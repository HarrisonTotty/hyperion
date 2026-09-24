//! Everything the planetary stage reads from the stages above: a system's [`SystemContext`] (plan
//! 14, P14.T1.d).
//!
//! A context holds a system's ID and [`HostKind`], its stars as plan 06's [`StarModel`]s (ruling
//! 34: the stage reads every star through them, never a track), plan 11's [`SystemHierarchy`] with
//! the stable zones it gives (P14.T9), the system's composition and \[Fe/H\], its age at the epoch,
//! its sphere of influence and the strip radius cut from it (design note 14), and its encounter
//! environment. Every generator of plan 14 is a pure function of a seed and a context.
//!
//! There are two ways to make one:
//!
//! - [`SystemContext::for_system`] resolves a system ID through plan 03 and builds the rest from
//!   the system's record: its stars through [`SystemStars`], its \[Fe/H\] through
//!   [`draw_metallicity`](crate::stellar::system::draw_metallicity), and its sphere of influence
//!   from plan 02's potential at its epoch position.
//! - [`SystemContext::builder`] makes synthetic hosts for tests and tools: a star or a binary of
//!   chosen masses, \[Fe/H\] and age, under a caller-supplied [`SystemId`] so that the stage's
//!   streams differ from one sample to the next. The test helpers of `planetary::testing` (behind
//!   the crate's `testing` feature) wrap it, and sample real systems.
//!
//! # What the vertical slice leaves out
//!
//! Ruling 33's slice builds the context before the plans that complete it, and each gap is a
//! documented value rather than code to replace:
//!
//! - **Binary evolution.** A real system's stars are [`SystemStars`]' primary and companions, with
//!   the hierarchy plan 11's draw gives a grid system (P11.T2.c), but each evolves as a single star
//!   until plan 11's P11.T4–T11, so the zones are the zones at birth.
//! - **\[α/Fe\]** is absent, `None`, not zero: P14.T1.a's closed form of \[Fe/H\] and population
//!   lands with its consumers (P14.T13.b and phase E).
//! - **The sphere of influence** is the galactic tidal radius alone,
//!   [`PotentialTables::tidal_radius`](crate::galaxy::potential::PotentialTables::tidal_radius) of
//!   the system's mass at its epoch position. Plan 09's rule, the smaller of that and a feature's,
//!   with the pericentre rule of P09.T28.c in the central black hole's Kepler regime, changes it
//!   only for feature members and for systems within about 10 ly of the centre.
//! - **The encounter environment** is `None`: only plan 09's feature members have one.
//! - **Every host is [`HostKind::Stellar`]**: plan 13 places the free-floating brown dwarfs and
//!   rogue planets, and P14.T27 gives them contexts.

use std::error::Error;
use std::fmt;

use super::params::SATELLITE_STABILITY_FRACTION;
use super::placement::{OrbitZone, ZoneHierarchy, ZoneStar, stable_zones};
use crate::Seed;
use crate::galaxy::imf::MASS_LIMIT_LO;
use crate::galaxy::placement::{Existence, ResolveSystemError, SystemRecord, resolve};
use crate::galaxy::{Galaxy, PointLy};
use crate::id::SystemId;
use crate::math;
use crate::orbit::{BuildOrbitError, Eccentricity, OpenOrbit};
use crate::stellar::draws::StarDraws;
use crate::stellar::multiplicity::{
    LOG_PERIOD_MAX, LOG_PERIOD_MIN, MIN_COMPANION_MASS, MIN_SUBSTELLAR_COMPANION_MASS,
    MultiplicityModel, SlotKind, SystemHierarchy, TIDAL_CUT_SHARE,
};
use crate::stellar::sse::{ZCoeffs, zams};
use crate::stellar::system::{MAX_STAR_MASS, StarModel, SystemStars};
use crate::stellar::{Composition, substellar};
use crate::time::{CLOCK_WINDOW_H, UniverseTime};
use crate::units::consts::{GM_SUN, METRES_PER_KILOPARSEC};
use crate::units::{
    Days, Dex, HeliumExcess, KilometresPerSecond, Metres, PerCubicLightYear, SolarMasses, Years,
};

/// The age of the universe, 13.787 Gyr: the oldest age at the epoch a context takes.
///
/// Planck Collaboration (2020, A&A 641, A6, Table 2, TT,TE,EE+lowE+lensing+BAO): 13.787 ± 0.020
/// Gyr. The oldest system the fields place, a halo star, is 13 Gyr old at the epoch
/// ([`HALO_AGES`](crate::galaxy::ages::HALO_AGES)).
pub const UNIVERSE_AGE: Years = Years::new(13.787e9);

/// Oort's constant A at the Sun, km s⁻¹ kpc⁻¹ (Bovy 2017, MNRAS 468, L63, eq. 5: 15.3 ± 0.4).
const OORT_A_KM_S_KPC: f64 = 15.3;

/// Oort's constant B at the Sun, km s⁻¹ kpc⁻¹ (Bovy 2017, eq. 6: −11.9 ± 0.4).
const OORT_B_KM_S_KPC: f64 = -11.9;

/// The age of a brown dwarf's disc in design note 6: a substellar host, which has no main
/// sequence, reads its snow line from its cooling fit's luminosity at 10 Myr.
const SUBSTELLAR_DISC_AGE: Years = Years::new(1.0e7);

/// The tidal (Jacobi) radius of a system of mass `m` (M☉, positive and finite) in the solar
/// neighbourhood, m: the sphere of influence of a synthetic host, which has no place in a galaxy.
///
/// `r_J` = (G m ÷ (4Ω² − κ²))^⅓, King's (1962, AJ 67, 471, eq. 24) limiting radius in the
/// epicyclic approximation, which Jiang and Tremaine (2010, MNRAS 401, 977, eq. 20) write as
/// (G m ÷ 4ΩA)^⅓; with Oort's constants Ω = A − B and κ² = −4B(A − B), so 4Ω² − κ² = 4A(A − B).
/// Bovy's (2017) A and B give 1.372 pc (2.83 × 10⁵ au) for 1 M☉, growing as m^⅓. Bovy notes that
/// the local velocity field is not quite axisymmetric, so reading his A and B as the potential's
/// frequencies is an approximation, good to the ±0.02 pc their errors give.
///
/// Plan 02's [`tidal_radius`](crate::galaxy::potential::PotentialTables::tidal_radius) gives a
/// real system's; this is the same formula with the measured frequencies at the Sun.
///
/// # Examples
///
/// ```
/// use hyperion_sim::planetary::context::solar_neighbourhood_tidal_radius;
/// use hyperion_sim::units::{AstronomicalUnits, SolarMasses};
///
/// let sun = AstronomicalUnits::from(solar_neighbourhood_tidal_radius(SolarMasses::new(1.0)));
/// assert!((sun.value() - 2.83e5).abs() < 0.01e5);
/// ```
#[must_use]
pub fn solar_neighbourhood_tidal_radius(m: SolarMasses) -> Metres {
    let per_second = 1e3 / METRES_PER_KILOPARSEC;
    let a = OORT_A_KM_S_KPC * per_second;
    let b = OORT_B_KM_S_KPC * per_second;
    let denominator = 4.0 * a * (a - b);
    Metres::new(math::cbrt(GM_SUN * m.value() / denominator))
}

/// What a system's host is (plan 14's Provides): a star, with or without companions, or one of
/// plan 13's free-floating objects.
///
/// By ruling 34 a star needs no host enum of its own: whatever it is, the context reads it
/// through its [`StarModel`]. This tells the stellar path from P14.T27's substellar ones.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HostKind {
    /// A star and its companions, if any: every system of plan 03's grid.
    #[default]
    Stellar,
    /// A free-floating brown dwarf, which runs the stellar path with the substellar class row
    /// (design note 13). Plan 13 places them.
    BrownDwarf,
    /// A free-floating planet, which runs the satellite path only (design note 13). Plan 13 places
    /// them.
    RoguePlanet,
}

/// The encounter environment of a feature member: what sets the second cut of design note 14,
/// where the time to a disrupting encounter equals the system's age (P14.T29).
///
/// Plan 09 supplies it from the feature's class profiles at the member's place; until then no
/// system has one. The three quantities are those of T29's cross-section,
/// 1 ÷ (n π a² (1 + 2 G M ÷ (a σ²)) σ).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EncounterEnvironment {
    number_density: PerCubicLightYear,
    velocity_dispersion: KilometresPerSecond,
    mean_member_mass: SolarMasses,
}

impl EncounterEnvironment {
    /// The environment of `number_density` systems per cubic light-year moving with a
    /// one-dimensional velocity dispersion `velocity_dispersion`, whose mean mass is
    /// `mean_member_mass`.
    ///
    /// # Errors
    ///
    /// [`BuildEncounterEnvironmentError::NumberDensityNotPositive`],
    /// [`VelocityDispersionNotPositive`](BuildEncounterEnvironmentError::VelocityDispersionNotPositive)
    /// or [`MeanMassNotPositive`](BuildEncounterEnvironmentError::MeanMassNotPositive) unless
    /// that quantity is positive and finite.
    pub fn new(
        number_density: PerCubicLightYear,
        velocity_dispersion: KilometresPerSecond,
        mean_member_mass: SolarMasses,
    ) -> Result<Self, BuildEncounterEnvironmentError> {
        if !is_positive(number_density.value()) {
            return Err(BuildEncounterEnvironmentError::NumberDensityNotPositive);
        }
        if !is_positive(velocity_dispersion.value()) {
            return Err(BuildEncounterEnvironmentError::VelocityDispersionNotPositive);
        }
        if !is_positive(mean_member_mass.value()) {
            return Err(BuildEncounterEnvironmentError::MeanMassNotPositive);
        }
        Ok(Self {
            number_density,
            velocity_dispersion,
            mean_member_mass,
        })
    }

    /// The number density of the member's neighbours, per cubic light-year.
    #[must_use]
    pub const fn number_density(&self) -> PerCubicLightYear {
        self.number_density
    }

    /// Their one-dimensional velocity dispersion, km s⁻¹, as the feature's profiles give it; the
    /// relative speed of an encounter is P14.T29's to form from it.
    #[must_use]
    pub const fn velocity_dispersion(&self) -> KilometresPerSecond {
        self.velocity_dispersion
    }

    /// Their mean mass, M☉.
    #[must_use]
    pub const fn mean_member_mass(&self) -> SolarMasses {
        self.mean_member_mass
    }
}

/// An [`EncounterEnvironment`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildEncounterEnvironmentError {
    /// The number density was not positive and finite.
    NumberDensityNotPositive,
    /// The velocity dispersion was not positive and finite.
    VelocityDispersionNotPositive,
    /// The mean member mass was not positive and finite.
    MeanMassNotPositive,
}

impl fmt::Display for BuildEncounterEnvironmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NumberDensityNotPositive => "the number density is not positive and finite",
            Self::VelocityDispersionNotPositive => {
                "the velocity dispersion is not positive and finite"
            }
            Self::MeanMassNotPositive => "the mean member mass is not positive and finite",
        })
    }
}

impl Error for BuildEncounterEnvironmentError {}

/// Everything plan 14's generators read about one system (P14.T1.d): its ID and host kind, its
/// stars and their hierarchy, its composition, its age, its sphere of influence and its
/// encounter environment.
///
/// The [module documentation](self) says how the two constructors fill it and what the vertical
/// slice leaves out. A context is immutable and a pure function of what built it, so two built
/// from the same galaxy and ID, or from the same builder, are equal, whatever was built before.
///
/// # Examples
///
/// A real system's context, from its ID, and what the planetary stage reads of it:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::placement::{CellKey, generate_cell};
/// use hyperion_sim::id::Layer;
/// use hyperion_sim::planetary::{HostKind, SystemContext};
///
/// let galaxy = Galaxy::new(Seed::new(11));
/// let mut cell = Vec::new();
/// generate_cell(&galaxy, CellKey::new(Layer::C, [0, 812, 0])?, &mut cell);
/// let id = cell.first().ok_or("layer C is not empty at the solar circle")?.id();
///
/// let context = SystemContext::for_system(&galaxy, id)?;
/// assert_eq!(context.host_kind(), HostKind::Stellar);
/// assert_eq!(context.stars().len(), usize::from(context.hierarchy().star_count()));
/// // Nothing is generated beyond 0.49 of the sphere of influence (design note 14).
/// assert!(context.strip_radius() < context.tidal_radius());
/// // [α/Fe] is not modelled yet, which is not the same as solar.
/// assert_eq!(context.alpha_fe(), None);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SystemContext {
    id: SystemId,
    host_kind: HostKind,
    stars: Vec<StarModel>,
    hierarchy: SystemHierarchy,
    composition: Composition,
    alpha_fe: Option<Dex>,
    age_at_epoch: Years,
    tidal_radius: Metres,
    encounter_environment: Option<EncounterEnvironment>,
}

impl SystemContext {
    /// The context of the system `id` of `galaxy`, resolved through plan 03.
    ///
    /// [`from_record`](Self::from_record) of the record [`resolve`] returns, so a system that
    /// resolves always has a context, whether it is born at the epoch or not.
    ///
    /// # Errors
    ///
    /// Whatever [`resolve`] refuses: [`ResolveSystemError::NoSuchSystem`] for an ID that names no
    /// system, [`LayerNotGenerated`](ResolveSystemError::LayerNotGenerated) for a brown-dwarf or
    /// rogue-planet ID until plan 13 places them, and
    /// [`KindNotGenerated`](ResolveSystemError::KindNotGenerated) for the kinds plans 09 and 10
    /// add.
    pub fn for_system(galaxy: &Galaxy, id: SystemId) -> Result<Self, ResolveSystemError> {
        let record = resolve(galaxy, id)?;
        Ok(Self::from_record(galaxy, &record))
    }

    /// The context of the grid system `record` of `galaxy`, for a caller that holds the record
    /// already (a cell, a range query's hit).
    ///
    /// - The stars are [`SystemStars::generate`]'s, primary first, and the composition theirs:
    ///   [`draw_metallicity`](crate::stellar::system::draw_metallicity) of the record.
    /// - The hierarchy is [`SystemStars`]' own, the one
    ///   [`draw_hierarchy`](crate::stellar::multiplicity::draw_hierarchy) draws for a grid system
    ///   under [`MultiplicityContext::Free`](crate::stellar::multiplicity::MultiplicityContext::Free)
    ///   (P11.T2.c), so the stars are the primary and
    ///   its companions.
    /// - The sphere of influence is plan 02's
    ///   [`tidal_radius`](crate::galaxy::potential::PotentialTables::tidal_radius) of the
    ///   hierarchy's [`system_mass`](SystemHierarchy::system_mass) at the record's epoch position,
    ///   zero for a system at the galactic centre itself.
    /// - The age at the epoch is the record's, [\[α/Fe\]](Self::alpha_fe) and the encounter
    ///   environment are `None`, and the host is [`HostKind::Stellar`].
    ///
    /// # Panics
    ///
    /// As [`SystemStars::generate`] does, for a record of another galaxy.
    #[must_use]
    pub fn from_record(galaxy: &Galaxy, record: &SystemRecord) -> Self {
        let stars = SystemStars::generate(galaxy, record);
        let hierarchy = stars.hierarchy().clone();
        let tidal_radius = galaxy.potential().tidal_radius(
            hierarchy.system_mass(),
            &PointLy::from(record.epoch_position()),
        );
        Self {
            id: record.id(),
            host_kind: HostKind::Stellar,
            composition: *stars.primary().composition(),
            // `SystemStars` lends its models; the copy keeps their built tracks, so nothing is
            // evolved twice.
            stars: stars.stars().to_vec(),
            hierarchy,
            alpha_fe: None,
            age_at_epoch: record.age_at_epoch(),
            tidal_radius,
            encounter_environment: None,
        }
    }

    /// A builder of synthetic hosts, for tests and tools.
    #[must_use]
    pub fn builder() -> SystemContextBuilder {
        SystemContextBuilder::default()
    }

    /// The system's ID, which keys every stream of the planetary stage (design note 4).
    #[must_use]
    pub const fn id(&self) -> SystemId {
        self.id
    }

    /// What the host is: [`HostKind::Stellar`] for every context of this generator version.
    #[must_use]
    pub const fn host_kind(&self) -> HostKind {
        self.host_kind
    }

    /// The stars, by body index, the primary first: one for each of the hierarchy's
    /// [`stars`](SystemHierarchy::stars), with the same initial mass.
    #[must_use]
    pub fn stars(&self) -> &[StarModel] {
        &self.stars
    }

    /// Plan 11's hierarchy of the stars, which bounds the stable zones.
    #[must_use]
    pub const fn hierarchy(&self) -> &SystemHierarchy {
        &self.hierarchy
    }

    /// The stable zones, each an orbit host with its limits (design note 10): P14.T9's
    /// [`stable_zones`] of [`ZoneHierarchy::from`] the hierarchy, recomputed on each call.
    ///
    /// They are the zones at birth, all the slice has: design note 10 intersects them with the
    /// zones of the evaluated state once plan 11 evolves binaries (P11.T4).
    #[must_use]
    pub fn zones(&self) -> Vec<OrbitZone> {
        stable_zones(&ZoneHierarchy::from(&self.hierarchy))
    }

    /// What each component's disc reads (P14.T9.c), by body index: its zero-age luminosity and
    /// radius, and its `star.disc_lifetime` rank.
    ///
    /// - A star takes plan 06's zero-age main sequence
    ///   ([`zams::luminosity`] and [`zams::radius`]) at its initial mass and the system's
    ///   composition (design note 6). Tout et al.'s (1996) fits are calibrated from 0.1 M☉; a star
    ///   of 0.08–0.1 M☉ takes them extrapolated, which they call "inaccurate but still reasonable"
    ///   in mass (their §2).
    /// - A brown-dwarf companion takes its cooling fit's luminosity and radius at 10 Myr, the
    ///   substellar rule of design note 6 ([`substellar::cooling`]).
    ///
    /// # Panics
    ///
    /// Never for a context this module builds: a brown-dwarf slot's mass lies inside the cooling
    /// fits.
    #[must_use]
    pub fn zone_stars(&self) -> Vec<ZoneStar> {
        let coeffs = ZCoeffs::new(self.composition.z_fit());
        self.hierarchy
            .stars()
            .iter()
            .zip(&self.stars)
            .map(|(slot, star)| {
                let mass = slot.initial_mass();
                let (zams_luminosity, zams_radius) = match slot.kind() {
                    SlotKind::Star => {
                        (zams::luminosity(mass, &coeffs), zams::radius(mass, &coeffs))
                    }
                    SlotKind::BrownDwarf => {
                        let state =
                            substellar::cooling(mass, SUBSTELLAR_DISC_AGE, &self.composition)
                                .expect(
                                    "a brown-dwarf companion's mass lies inside the cooling fits",
                                );
                        (state.luminosity(), state.radius())
                    }
                };
                ZoneStar {
                    zams_luminosity,
                    zams_radius,
                    disc_lifetime_rank: star.draws().disc_lifetime(),
                }
            })
            .collect()
    }

    /// The composition every star of the system shares.
    #[must_use]
    pub const fn composition(&self) -> &Composition {
        &self.composition
    }

    /// The system's iron abundance \[Fe/H\], dex: its [`composition`](Self::composition)'s.
    #[must_use]
    pub const fn fe_h(&self) -> Dex {
        self.composition.fe_h()
    }

    /// The system's \[α/Fe\], dex: `None`, because this generator version does not model it.
    ///
    /// `None` is "not modelled", never "solar" (ruling 33). P14.T1.a's closed form of \[Fe/H\] and
    /// population (the thin-disc and thick-disc sequences) lands with its consumers, P14.T13.b and
    /// phase E.
    #[must_use]
    pub const fn alpha_fe(&self) -> Option<Dex> {
        self.alpha_fe
    }

    /// The system's age at the epoch, Julian years: negative for a system that forms during play.
    #[must_use]
    pub const fn age_at_epoch(&self) -> Years {
        self.age_at_epoch
    }

    /// The system's age at `t`, Julian years, in the arithmetic of [`SystemRecord::age_at`] and
    /// [`StarModel::age_at`].
    #[must_use]
    pub fn age_at(&self, t: UniverseTime) -> Years {
        Years::new(self.age_at_epoch.value() + t.since_epoch().as_julian_years_f64())
    }

    /// Whether the system exists at `t`: once its [`age_at`](Self::age_at) is positive, as
    /// [`SystemRecord::existence_at`] says.
    #[must_use]
    pub fn existence_at(&self, t: UniverseTime) -> Existence {
        if self.age_at(t).value() > 0.0 {
            Existence::Exists
        } else {
            Existence::NoSystemYet
        }
    }

    /// The system's sphere of influence, m: the galactic tidal radius of its mass at its epoch
    /// position for a real system, and until plan 09 for every system (module documentation).
    #[must_use]
    pub const fn tidal_radius(&self) -> Metres {
        self.tidal_radius
    }

    /// The radius inside which the stage generates bodies, m: design note 14's
    /// [`SATELLITE_STABILITY_FRACTION`] (0.4895) of the [`tidal_radius`](Self::tidal_radius).
    ///
    /// P14.T29 adds here the encounter cut of a feature member, the smaller of this and the radius
    /// at which a disrupting encounter is expected within the system's age, so that the stage has
    /// one strip radius. Until then an [`encounter_environment`](Self::encounter_environment) a
    /// synthetic host was given is carried but not applied; no real system of this generator
    /// version has one.
    #[must_use]
    pub fn strip_radius(&self) -> Metres {
        self.tidal_radius * SATELLITE_STABILITY_FRACTION
    }

    /// The system's encounter environment: `None` except for a feature member, which plan 09
    /// places, or a synthetic host given one.
    #[must_use]
    pub const fn encounter_environment(&self) -> Option<&EncounterEnvironment> {
        self.encounter_environment.as_ref()
    }
}

/// How a synthetic host's stars take their plan 06 draws ([`StarDraws`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum SyntheticDraws {
    /// Every star the median star ([`StarDraws::median`]): its disc lifetime, rotation and
    /// remnant draws at their medians, whatever the ID. The default.
    #[default]
    Median,
    /// Every star the draws plan 06 gives the same body of the same system in the universe of
    /// this seed ([`StarDraws::for_star`]), so that they vary from one ID to the next as a real
    /// system's do.
    OfUniverse(Seed),
}

/// The stars a builder was given.
#[derive(Debug, Clone, Copy, PartialEq)]
enum SyntheticStars {
    /// A single star of this initial mass.
    Single(SolarMasses),
    /// A primary and a companion on a relative orbit.
    Binary {
        primary: SolarMasses,
        companion: SolarMasses,
        a: Metres,
        e: Eccentricity,
    },
}

/// Builds a [`SystemContext`] for a synthetic host: a star or a binary with chosen masses,
/// \[Fe/H\] and age (P14.T1.d).
///
/// The ID, the stars and the age are required. The rest default: \[Fe/H\] 0 with no helium
/// excess, the median star's draws ([`SyntheticDraws::Median`]), the sphere of influence of a field
/// system at the Sun's galactocentric radius ([`solar_neighbourhood_tidal_radius`] of the system's
/// mass), and no encounter environment. [\[α/Fe\]](SystemContext::alpha_fe) is `None` and the host
/// [`HostKind::Stellar`], as for a real system.
///
/// A binary's orbit lies in the reference plane with the companion at periapsis at the epoch. Its
/// companion is a star from [`MIN_COMPANION_MASS`] (0.08 M☉) and a brown dwarf below it, down to
/// [`MIN_SUBSTELLAR_COMPANION_MASS`] (13 Jupiter masses), as plan 11 numbers them. The hierarchy
/// is one plan 11's draw could give, except that the draw makes brown-dwarf companions only from
/// P11.T2.d: no companion outweighs the primary, the period lies inside plan 11's range
/// ([`LOG_PERIOD_MIN`] to [`LOG_PERIOD_MAX`], 0.1 to 10¹¹ days), the eccentricity inside Moe and
/// Di Stefano's envelope at that period (zero below 12 days,
/// [`EccentricityDistribution::e_max`](crate::stellar::multiplicity::EccentricityDistribution::e_max))
/// and below the open orbits' [`OpenOrbit::MIN_ECCENTRICITY`], and the apocentre inside
/// [`TIDAL_CUT_SHARE`] of the sphere of influence.
///
/// # Examples
///
/// Two synthetic Suns with different IDs have the same stars but open different planetary
/// streams, and a binary's stars bound each other's zones:
///
/// ```
/// use hyperion_sim::galaxy::placement::CellKey;
/// use hyperion_sim::id::Layer;
/// use hyperion_sim::orbit::Eccentricity;
/// use hyperion_sim::planetary::SystemContext;
/// use hyperion_sim::units::{AstronomicalUnits, Dex, Metres, SolarMasses, Years};
///
/// let cell = CellKey::new(Layer::C, [0, 812, 0])?;
/// let id = |i| cell.candidate_id(i).ok_or("inside the cell's index field");
/// let sun = SystemContext::builder()
///     .star(SolarMasses::new(1.0))
///     .fe_h(Dex::ZERO)
///     .age_at_epoch(Years::new(4.57e9));
/// let (a, b) = (sun.clone().system(id(0)?).build()?, sun.system(id(1)?).build()?);
/// assert_eq!(a.stars(), b.stars());
/// assert_ne!(a.id(), b.id());
///
/// let binary = SystemContext::builder()
///     .system(id(2)?)
///     .binary(
///         SolarMasses::new(1.1),
///         SolarMasses::new(0.9),
///         Metres::from(AstronomicalUnits::new(20.0)),
///         Eccentricity::new(0.3)?,
///     )
///     .age_at_epoch(Years::new(5.0e9))
///     .build()?;
/// // A zone about each star and one about the pair.
/// assert_eq!(binary.zones().len(), 3);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SystemContextBuilder {
    id: Option<SystemId>,
    stars: Option<SyntheticStars>,
    fe_h: Dex,
    age_at_epoch: Option<Years>,
    draws: SyntheticDraws,
    tidal_radius: Option<Metres>,
    encounter_environment: Option<EncounterEnvironment>,
}

impl SystemContextBuilder {
    /// The host's ID, which keys the planetary stage's streams: give each synthetic sample its
    /// own.
    #[must_use]
    pub const fn system(mut self, id: SystemId) -> Self {
        self.id = Some(id);
        self
    }

    /// A single star of initial mass `mass`, M☉ (0.08–150), replacing any stars given before.
    #[must_use]
    pub const fn star(mut self, mass: SolarMasses) -> Self {
        self.stars = Some(SyntheticStars::Single(mass));
        self
    }

    /// A binary, replacing any stars given before: the primary of initial mass `primary` M☉
    /// (0.08–150), and a companion of `companion` M☉ (13 Jupiter masses up to the primary's) on a
    /// relative orbit of semi-major axis `a` and eccentricity `e` about their total mass.
    #[must_use]
    pub const fn binary(
        mut self,
        primary: SolarMasses,
        companion: SolarMasses,
        a: Metres,
        e: Eccentricity,
    ) -> Self {
        self.stars = Some(SyntheticStars::Binary {
            primary,
            companion,
            a,
            e,
        });
        self
    }

    /// The system's \[Fe/H\], dex (default 0): its composition is plan 06's
    /// [`Composition::from_fe_h`] with no helium excess, as a grid system's is.
    #[must_use]
    pub const fn fe_h(mut self, fe_h: Dex) -> Self {
        self.fe_h = fe_h;
        self
    }

    /// The system's age at the epoch, Julian years: from −H, a system that forms during play, to
    /// [`UNIVERSE_AGE`].
    #[must_use]
    pub const fn age_at_epoch(mut self, age: Years) -> Self {
        self.age_at_epoch = Some(age);
        self
    }

    /// How the stars take their plan 06 draws (default [`SyntheticDraws::Median`]).
    #[must_use]
    pub const fn star_draws(mut self, draws: SyntheticDraws) -> Self {
        self.draws = draws;
        self
    }

    /// The system's sphere of influence, m, in place of the solar neighbourhood's.
    #[must_use]
    pub const fn tidal_radius(mut self, radius: Metres) -> Self {
        self.tidal_radius = Some(radius);
        self
    }

    /// An encounter environment, as a feature member would have (P14.T29).
    #[must_use]
    pub const fn encounter_environment(mut self, environment: EncounterEnvironment) -> Self {
        self.encounter_environment = Some(environment);
        self
    }

    /// The context.
    ///
    /// # Errors
    ///
    /// - [`BuildSystemContextError::MissingSystem`], [`MissingStars`] or [`MissingAge`] if the ID,
    ///   the stars or the age was not given.
    /// - [`BuildSystemContextError::MassOutsideRange`] for a primary outside 0.08–150 M☉ or a
    ///   companion outside 13 Jupiter masses–150 M☉, a negative mass or NaN included.
    /// - [`BuildSystemContextError::CompanionOutweighsPrimary`] for a companion heavier than its
    ///   primary.
    /// - [`BuildSystemContextError::AgeOutsideRange`] for an age before −H, beyond
    ///   [`UNIVERSE_AGE`], or not finite.
    /// - [`BuildSystemContextError::MetallicityNotFinite`] if \[Fe/H\] is not finite.
    /// - [`BuildSystemContextError::TidalRadiusNotPositive`] for a sphere of influence that is not
    ///   positive and finite.
    /// - [`BuildSystemContextError::Orbit`] for a binary's semi-major axis that is not positive and
    ///   finite, or whose period overflows or underflows.
    /// - [`BuildSystemContextError::PeriodOutsideRange`] for a binary's period outside plan 11's
    ///   0.1 to 10¹¹ days.
    /// - [`BuildSystemContextError::EccentricityAboveEnvelope`] for a binary's eccentricity above
    ///   plan 11's envelope at its period.
    /// - [`BuildSystemContextError::BeyondTidalCut`] for a binary whose apocentre lies beyond
    ///   [`TIDAL_CUT_SHARE`] of the sphere of influence.
    ///
    ///
    /// # Panics
    ///
    /// Never: every mass and age that reaches plan 06's [`StarModel::new`] has been checked
    /// against its ranges.
    ///
    /// [`MissingStars`]: BuildSystemContextError::MissingStars
    /// [`MissingAge`]: BuildSystemContextError::MissingAge
    pub fn build(self) -> Result<SystemContext, BuildSystemContextError> {
        let id = self.id.ok_or(BuildSystemContextError::MissingSystem)?;
        let stars = self.stars.ok_or(BuildSystemContextError::MissingStars)?;
        let age = self
            .age_at_epoch
            .ok_or(BuildSystemContextError::MissingAge)?;
        let youngest = -CLOCK_WINDOW_H.as_julian_years_f64();
        if !(youngest..=UNIVERSE_AGE.value()).contains(&age.value()) {
            return Err(BuildSystemContextError::AgeOutsideRange(age));
        }
        if !self.fe_h.value().is_finite() {
            return Err(BuildSystemContextError::MetallicityNotFinite(self.fe_h));
        }
        let hierarchy = synthetic_hierarchy(id, stars)?;
        let tidal_radius = match self.tidal_radius {
            Some(radius) if is_positive(radius.value()) => radius,
            Some(radius) => return Err(BuildSystemContextError::TidalRadiusNotPositive(radius)),
            None => solar_neighbourhood_tidal_radius(hierarchy.system_mass()),
        };
        if let Some((_, orbit)) = hierarchy.pairs().next() {
            let limit = tidal_radius * TIDAL_CUT_SHARE;
            let apoapsis = orbit.apoapsis();
            if apoapsis > limit {
                return Err(BuildSystemContextError::BeyondTidalCut { apoapsis, limit });
            }
        }
        let composition = Composition::from_fe_h(self.fe_h, HeliumExcess::ZERO);
        let models = hierarchy
            .stars()
            .iter()
            .map(|slot| {
                let draws = match self.draws {
                    SyntheticDraws::Median => StarDraws::median(),
                    SyntheticDraws::OfUniverse(seed) => StarDraws::for_star(seed, slot.body()),
                };
                StarModel::new(slot.initial_mass(), composition, draws, age)
                    .expect("the masses and the age were checked against the model's ranges")
            })
            .collect();
        Ok(SystemContext {
            id,
            host_kind: HostKind::Stellar,
            stars: models,
            hierarchy,
            composition,
            alpha_fe: None,
            age_at_epoch: age,
            tidal_radius,
            encounter_environment: self.encounter_environment,
        })
    }
}

/// The hierarchy of a synthetic host's `stars`, with its masses checked.
fn synthetic_hierarchy(
    id: SystemId,
    stars: SyntheticStars,
) -> Result<SystemHierarchy, BuildSystemContextError> {
    let primary = match stars {
        SyntheticStars::Single(mass) | SyntheticStars::Binary { primary: mass, .. } => mass,
    };
    if !(MASS_LIMIT_LO..=MAX_STAR_MASS.value()).contains(&primary.value()) {
        return Err(BuildSystemContextError::MassOutsideRange {
            star: 0,
            mass: primary,
        });
    }
    match stars {
        SyntheticStars::Single(mass) => Ok(SystemHierarchy::single(id, mass, SlotKind::Star)),
        SyntheticStars::Binary {
            primary,
            companion,
            a,
            e,
        } => {
            let m = companion.value();
            if !(MIN_SUBSTELLAR_COMPANION_MASS.value()..=MAX_STAR_MASS.value()).contains(&m) {
                return Err(BuildSystemContextError::MassOutsideRange {
                    star: 1,
                    mass: companion,
                });
            }
            if companion > primary {
                return Err(BuildSystemContextError::CompanionOutweighsPrimary {
                    primary,
                    companion,
                });
            }
            let kind = if companion < MIN_COMPANION_MASS {
                SlotKind::BrownDwarf
            } else {
                SlotKind::Star
            };
            let hierarchy = SystemHierarchy::binary(id, primary, (companion, kind), a, e)
                .map_err(BuildSystemContextError::Orbit)?;
            if let Some((_, orbit)) = hierarchy.pairs().next() {
                check_orbit(Days::from(orbit.period()), e)?;
            }
            Ok(hierarchy)
        }
    }
}

/// Checks a synthetic binary's orbit of period `period` and eccentricity `e` against plan 11's
/// draw: its period range and its eccentricity envelope.
fn check_orbit(period: Days, e: Eccentricity) -> Result<(), BuildSystemContextError> {
    let log_period = math::log10(period.value());
    if !(LOG_PERIOD_MIN..=LOG_PERIOD_MAX).contains(&log_period) {
        return Err(BuildSystemContextError::PeriodOutsideRange(period));
    }
    let e_max = MultiplicityModel::default_v1()
        .eccentricity_distribution(period)
        .e_max();
    if e.value() > e_max || e.value() >= OpenOrbit::MIN_ECCENTRICITY {
        return Err(BuildSystemContextError::EccentricityAboveEnvelope {
            eccentricity: e,
            e_max,
        });
    }
    Ok(())
}

/// Whether `x` is positive and finite.
#[must_use]
fn is_positive(x: f64) -> bool {
    x.is_finite() && x > 0.0
}

/// A synthetic host's [`SystemContext`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildSystemContextError {
    /// No ID was given.
    MissingSystem,
    /// No star was given.
    MissingStars,
    /// No age was given.
    MissingAge,
    /// A star's initial mass was outside its range: 0.08–150 M☉ for the primary, 13 Jupiter masses
    /// to 150 M☉ for a companion.
    MassOutsideRange {
        /// The star's body index: 0 for the primary, 1 for the companion.
        star: u8,
        /// The mass given, M☉.
        mass: SolarMasses,
    },
    /// The companion was heavier than the primary, which plan 11 never draws.
    CompanionOutweighsPrimary {
        /// The primary's initial mass, M☉.
        primary: SolarMasses,
        /// The companion's initial mass, M☉.
        companion: SolarMasses,
    },
    /// The age at the epoch was before −H, beyond [`UNIVERSE_AGE`], or not finite.
    AgeOutsideRange(Years),
    /// \[Fe/H\] was not finite.
    MetallicityNotFinite(Dex),
    /// The sphere of influence given was not positive and finite.
    TidalRadiusNotPositive(Metres),
    /// The binary's orbit could not be built.
    Orbit(BuildOrbitError),
    /// The binary's period lay outside plan 11's range, 0.1 to 10¹¹ days.
    PeriodOutsideRange(Days),
    /// The binary's eccentricity lay above plan 11's envelope at its period, which is zero below
    /// the circularisation period of 12 days, or at the open orbits' 0.9999.
    EccentricityAboveEnvelope {
        /// The eccentricity given.
        eccentricity: Eccentricity,
        /// The envelope at the binary's period.
        e_max: f64,
    },
    /// The binary's apocentre lay beyond [`TIDAL_CUT_SHARE`] of the sphere of influence, which
    /// plan 11's tidal cut refuses.
    BeyondTidalCut {
        /// The apocentre, m.
        apoapsis: Metres,
        /// The limit, m.
        limit: Metres,
    },
}

impl fmt::Display for BuildSystemContextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSystem => f.write_str("no system id was given"),
            Self::MissingStars => f.write_str("no star was given"),
            Self::MissingAge => f.write_str("no age was given"),
            Self::MassOutsideRange { star, mass } => {
                write!(
                    f,
                    "star {star}'s initial mass {} M_sun is out of range",
                    mass.value()
                )
            }
            Self::CompanionOutweighsPrimary { primary, companion } => write!(
                f,
                "the companion of {} M_sun outweighs the primary of {} M_sun",
                companion.value(),
                primary.value()
            ),
            Self::AgeOutsideRange(age) => write!(
                f,
                "age at the epoch {} yr is outside -H to the universe's age",
                age.value()
            ),
            Self::MetallicityNotFinite(fe_h) => write!(f, "[Fe/H] {} is not finite", fe_h.value()),
            Self::TidalRadiusNotPositive(radius) => {
                write!(
                    f,
                    "tidal radius {} m is not positive and finite",
                    radius.value()
                )
            }
            Self::Orbit(_) => f.write_str("the binary's orbit cannot be built"),
            Self::PeriodOutsideRange(period) => write!(
                f,
                "the binary's period of {} d is outside 0.1 to 1e11 d",
                period.value()
            ),
            Self::EccentricityAboveEnvelope {
                eccentricity,
                e_max,
            } => write!(
                f,
                "the binary's eccentricity {} is above the envelope's {e_max} at its period",
                eccentricity.value()
            ),
            Self::BeyondTidalCut { apoapsis, limit } => write!(
                f,
                "the binary's apocentre of {} m lies beyond the tidal cut at {} m",
                apoapsis.value(),
                limit.value()
            ),
        }
    }
}

impl Error for BuildSystemContextError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Orbit(error) => Some(error),
            Self::MissingSystem
            | Self::MissingStars
            | Self::MissingAge
            | Self::MassOutsideRange { .. }
            | Self::CompanionOutweighsPrimary { .. }
            | Self::AgeOutsideRange(_)
            | Self::MetallicityNotFinite(_)
            | Self::TidalRadiusNotPositive(_)
            | Self::PeriodOutsideRange(_)
            | Self::EccentricityAboveEnvelope { .. }
            | Self::BeyondTidalCut { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::order::assert_order_independent;

    use super::*;
    use crate::coords::GalacticPosition;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::placement::{CellKey, generate_cell};
    use crate::id::{BodyId, Layer};
    use crate::orbit::BuildOrbitError;
    use crate::planetary::placement::{OrbitHost, ZoneNode};
    use crate::stellar::multiplicity::{MultiplicityContext, RedrawAttempt, draw_hierarchy};
    use crate::stellar::system::draw_metallicity;
    use crate::units::consts::METRES_PER_PARSEC;
    use crate::units::{AstronomicalUnits, LightYears};

    const SEED: u64 = 0x0e14_0001_d000_5eed;

    fn galaxy() -> Galaxy {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral")
    }

    /// The cell of `layer` at the solar circle, 26,000 ly out along +y in the plane.
    fn solar_cell(layer: Layer) -> CellKey {
        let size = i32::try_from(layer.cell_size_ly()).unwrap();
        CellKey::new(layer, [0, 26_000 / size, 0]).unwrap()
    }

    /// The first few systems of layers A, C and E at the solar circle.
    fn records(galaxy: &Galaxy) -> Vec<SystemRecord> {
        let mut out = Vec::new();
        let mut cell = Vec::new();
        for layer in [Layer::A, Layer::C, Layer::E] {
            generate_cell(galaxy, solar_cell(layer), &mut cell);
            assert!(
                !cell.is_empty(),
                "layer {layer:?} is empty at the solar circle"
            );
            out.extend(cell.iter().take(4));
        }
        out
    }

    fn id(index: u32) -> SystemId {
        solar_cell(Layer::C).candidate_id(index).unwrap()
    }

    fn au(x: f64) -> Metres {
        Metres::from(AstronomicalUnits::new(x))
    }

    fn sun(index: u32) -> SystemContextBuilder {
        SystemContext::builder()
            .system(id(index))
            .star(SolarMasses::new(1.0))
            .age_at_epoch(Years::new(4.57e9))
    }

    fn binary(m1: f64, m2: f64, a_au: f64, e: f64) -> SystemContextBuilder {
        SystemContext::builder()
            .system(id(7))
            .binary(
                SolarMasses::new(m1),
                SolarMasses::new(m2),
                au(a_au),
                Eccentricity::new(e).unwrap(),
            )
            .age_at_epoch(Years::new(2.0e9))
    }

    // P14.T1.d: `for_system`.

    #[test]
    fn an_id_that_names_no_system_has_no_context() {
        let galaxy = galaxy();
        let key = solar_cell(Layer::C);
        // Far above any cell's candidate count.
        let unplaced = key.candidate_id(key.index_capacity() - 1).unwrap();
        assert_eq!(
            SystemContext::for_system(&galaxy, unplaced),
            Err(ResolveSystemError::NoSuchSystem)
        );
    }

    #[test]
    fn a_real_system_s_context_joins_its_stages() {
        let galaxy = galaxy();
        for record in records(&galaxy) {
            let context = SystemContext::for_system(&galaxy, record.id()).unwrap();
            assert_eq!(context, SystemContext::from_record(&galaxy, &record));
            assert_eq!(context.id(), record.id());
            assert_eq!(context.host_kind(), HostKind::Stellar);
            // The stars are plan 06's with plan 11's companions (P11.T2.c).
            let stars = SystemStars::generate(&galaxy, &record);
            assert_eq!(context.stars(), stars.stars());
            assert_eq!(context.composition(), &draw_metallicity(&galaxy, &record));
            assert_same_bits(
                context.fe_h().value(),
                draw_metallicity(&galaxy, &record).fe_h().value(),
            );
            assert_eq!(context.alpha_fe(), None);
            assert_same_bits(
                context.age_at_epoch().value(),
                record.age_at_epoch().value(),
            );
            // The hierarchy is the one the multiplicity draw gives a grid system, and each slot is
            // its model's star.
            let drawn = draw_hierarchy(
                &galaxy,
                &record,
                MultiplicityContext::Free,
                RedrawAttempt::FIRST,
            );
            assert_eq!(context.hierarchy(), &drawn);
            assert_eq!(context.hierarchy(), stars.hierarchy());
            assert_eq!(
                usize::from(context.hierarchy().star_count()),
                context.stars().len()
            );
            for (slot, star) in context.hierarchy().stars().iter().zip(context.stars()) {
                assert_eq!(slot.body().system(), record.id());
                assert_same_bits(slot.initial_mass().value(), star.initial_mass().value());
            }
            // The sphere of influence is the galactic tidal radius at the epoch position.
            let tidal = galaxy
                .potential()
                .tidal_radius(drawn.system_mass(), &PointLy::from(record.epoch_position()));
            assert_same_bits(context.tidal_radius().value(), tidal.value());
            assert_same_bits(
                context.strip_radius().value(),
                tidal.value() * SATELLITE_STABILITY_FRACTION,
            );
            assert_eq!(context.encounter_environment(), None);
            // A single star has one zone; a multiple's narrow zones may be dropped (P14.T9.b), but
            // its circumbinary zone never is.
            if drawn.star_count() == 1 {
                assert_eq!(context.zones().len(), 1);
            } else {
                assert!(!context.zones().is_empty());
            }
        }
    }

    #[test]
    fn a_context_is_the_same_in_every_run_and_every_order() {
        let (first, second) = (galaxy(), galaxy());
        let ids: Vec<SystemId> = records(&first).iter().map(SystemRecord::id).collect();
        for &id in &ids {
            assert_eq!(
                SystemContext::for_system(&first, id),
                SystemContext::for_system(&second, id)
            );
        }
        assert_order_independent(&ids, |&id| SystemContext::for_system(&first, id));
        let builders: Vec<SystemContextBuilder> = (0..6)
            .map(|i| sun(i).star_draws(SyntheticDraws::OfUniverse(Seed::new(SEED))))
            .collect();
        assert_order_independent(&builders, |b| b.clone().build());
    }

    #[test]
    fn age_and_existence_follow_the_record() {
        let galaxy = galaxy();
        let record = records(&galaxy)[0];
        let context = SystemContext::from_record(&galaxy, &record);
        for years in [-1_000, -1, 0, 1, 500, 1_000] {
            let t = UniverseTime::from_julian_years(years).unwrap();
            assert_same_bits(context.age_at(t).value(), record.age_at(t).value());
            assert_eq!(context.existence_at(t), record.existence_at(t));
        }
        // A synthetic system that forms 300 years after the epoch.
        let unborn = sun(0).age_at_epoch(Years::new(-300.0)).build().unwrap();
        let at = |y| UniverseTime::from_julian_years(y).unwrap();
        assert_eq!(unborn.existence_at(at(0)), Existence::NoSystemYet);
        assert_eq!(unborn.existence_at(at(300)), Existence::NoSystemYet);
        assert_eq!(unborn.existence_at(at(301)), Existence::Exists);
        assert_eq!(unborn.stars()[0].state_at(at(0)), None);
    }

    // P14.T1.d: the builder.

    #[test]
    fn the_builder_refuses_a_negative_mass() {
        assert_eq!(
            sun(0).star(SolarMasses::new(-1.0)).build(),
            Err(BuildSystemContextError::MassOutsideRange {
                star: 0,
                mass: SolarMasses::new(-1.0)
            })
        );
        assert_eq!(
            binary(1.0, -0.5, 10.0, 0.1).build(),
            Err(BuildSystemContextError::MassOutsideRange {
                star: 1,
                mass: SolarMasses::new(-0.5)
            })
        );
        assert_eq!(
            binary(-1.0, 0.5, 10.0, 0.1).build(),
            Err(BuildSystemContextError::MassOutsideRange {
                star: 0,
                mass: SolarMasses::new(-1.0)
            })
        );
        // The stellar range's ends: a primary below the hydrogen-burning grid's 0.08 M☉ is a brown
        // dwarf, a host of plan 13's, and none is placed above 150 M☉.
        for mass in [0.0, 0.079, 150.1, f64::NAN] {
            assert!(matches!(
                sun(0).star(SolarMasses::new(mass)).build(),
                Err(BuildSystemContextError::MassOutsideRange { star: 0, .. })
            ));
        }
        for mass in [0.08, 150.0] {
            assert!(sun(0).star(SolarMasses::new(mass)).build().is_ok());
        }
    }

    #[test]
    fn the_builder_refuses_an_age_beyond_the_universe_s() {
        for age in [13.788e9, 1e11, f64::INFINITY, f64::NAN, -1_000.5, -1e6] {
            assert!(
                matches!(
                    sun(0).age_at_epoch(Years::new(age)).build(),
                    Err(BuildSystemContextError::AgeOutsideRange(_))
                ),
                "{age}"
            );
        }
        for age in [13.787e9, 0.0, -1_000.0] {
            assert!(
                sun(0).age_at_epoch(Years::new(age)).build().is_ok(),
                "{age}"
            );
        }
    }

    #[test]
    fn the_builder_names_what_is_missing_or_invalid() {
        assert_eq!(
            SystemContext::builder().build(),
            Err(BuildSystemContextError::MissingSystem)
        );
        assert_eq!(
            SystemContext::builder().system(id(0)).build(),
            Err(BuildSystemContextError::MissingStars)
        );
        assert_eq!(
            SystemContext::builder()
                .system(id(0))
                .star(SolarMasses::new(1.0))
                .build(),
            Err(BuildSystemContextError::MissingAge)
        );
        assert!(matches!(
            sun(0).fe_h(Dex::new(f64::NAN)).build(),
            Err(BuildSystemContextError::MetallicityNotFinite(_))
        ));
        for radius in [0.0, -1.0, f64::INFINITY] {
            assert!(matches!(
                sun(0).tidal_radius(Metres::new(radius)).build(),
                Err(BuildSystemContextError::TidalRadiusNotPositive(_))
            ));
        }
        assert_eq!(
            binary(0.6, 0.9, 10.0, 0.1).build(),
            Err(BuildSystemContextError::CompanionOutweighsPrimary {
                primary: SolarMasses::new(0.6),
                companion: SolarMasses::new(0.9)
            })
        );
        assert!(matches!(
            binary(1.0, 0.5, 0.0, 0.1).build(),
            Err(BuildSystemContextError::Orbit(
                BuildOrbitError::SemiMajorAxisNotPositive { .. }
            ))
        ));
        // A companion lighter than plan 11's substellar floor of 13 Jupiter masses.
        assert!(matches!(
            binary(1.0, 0.01, 10.0, 0.1).build(),
            Err(BuildSystemContextError::MassOutsideRange { star: 1, .. })
        ));
    }

    #[test]
    fn a_binary_plan_11_would_not_draw_is_refused() {
        // 0.001 au about 1.5 M☉ is a period of 0.0094 d, under plan 11's 0.1 d.
        assert!(matches!(
            binary(1.0, 0.5, 0.001, 0.0).build(),
            Err(BuildSystemContextError::PeriodOutsideRange(p)) if p.value() < 0.1
        ));
        // 0.05 au about 2 M☉ is 2.9 days, under the circularisation period of 12.
        let close = |e| binary(1.0, 1.0, 0.05, e).build();
        assert!(close(0.0).is_ok());
        assert_eq!(
            close(0.1),
            Err(BuildSystemContextError::EccentricityAboveEnvelope {
                eccentricity: Eccentricity::new(0.1).unwrap(),
                e_max: 0.0
            })
        );
        // At 1 au (258 days) the envelope is 1 − (129)^(−2/3) = 0.961.
        assert!(binary(1.0, 1.0, 1.0, 0.95).build().is_ok());
        assert!(matches!(
            binary(1.0, 1.0, 1.0, 0.97).build(),
            Err(BuildSystemContextError::EccentricityAboveEnvelope { e_max, .. })
                if (e_max - 0.961).abs() < 1e-3
        ));
        // A wide orbit's envelope passes 0.9999, where open orbits begin.
        assert!(matches!(
            binary(1.0, 1.0, 5_000.0, 0.99995)
                .tidal_radius(au(1e9))
                .build(),
            Err(BuildSystemContextError::EccentricityAboveEnvelope { .. })
        ));
    }

    #[test]
    fn a_binary_beyond_the_tidal_cut_is_refused() {
        // 1.3722 pc for 1 M☉, times (1.5)^⅓ for the pair, halved: about 1.2e5 au.
        let limit = solar_neighbourhood_tidal_radius(SolarMasses::new(1.5)) * TIDAL_CUT_SHARE;
        let inside = AstronomicalUnits::from(limit).value() / 1.5 * 0.999;
        assert!(binary(1.0, 0.5, inside, 0.5).build().is_ok());
        let error = binary(1.0, 0.5, inside * 1.01, 0.5).build();
        let Err(BuildSystemContextError::BeyondTidalCut {
            apoapsis,
            limit: cut,
        }) = error
        else {
            panic!("{error:?}");
        };
        assert_same_bits(cut.value(), limit.value());
        assert!(apoapsis > cut);
        // Against a sphere of influence given, the cut is half of it.
        assert!(matches!(
            binary(1.0, 0.5, 100.0, 0.0).tidal_radius(au(199.0)).build(),
            Err(BuildSystemContextError::BeyondTidalCut { .. })
        ));
        assert!(
            binary(1.0, 0.5, 100.0, 0.0)
                .tidal_radius(au(201.0))
                .build()
                .is_ok()
        );
    }

    #[test]
    fn a_synthetic_star_is_plan_06_s_star_under_its_own_id() {
        let fe_h = Dex::new(-0.3);
        let a = sun(3).fe_h(fe_h).build().unwrap();
        let composition = Composition::from_fe_h(fe_h, HeliumExcess::ZERO);
        let model = StarModel::new(
            SolarMasses::new(1.0),
            composition,
            StarDraws::median(),
            Years::new(4.57e9),
        )
        .unwrap();
        assert_eq!(a.id(), id(3));
        assert_eq!(a.stars(), std::slice::from_ref(&model));
        assert_eq!(a.composition(), &composition);
        assert_eq!(a.alpha_fe(), None);
        assert_eq!(a.host_kind(), HostKind::Stellar);
        assert_eq!(
            a.hierarchy(),
            &SystemHierarchy::single(id(3), SolarMasses::new(1.0), SlotKind::Star)
        );
        assert_eq!(a.hierarchy().stars()[0].body(), BodyId::new(id(3), 0));
        assert_same_bits(
            a.tidal_radius().value(),
            solar_neighbourhood_tidal_radius(SolarMasses::new(1.0)).value(),
        );
        assert_eq!(a.encounter_environment(), None);
        let zones = a.zones();
        assert_eq!(zones.len(), 1);
        assert_eq!(zones[0].host(), OrbitHost::Star(0));
        assert_eq!((zones[0].inner(), zones[0].outer()), (None, None));
        // Another ID: the same star, other streams.
        let b = sun(4).fe_h(fe_h).build().unwrap();
        assert_eq!(a.stars(), b.stars());
        assert_ne!(a.id(), b.id());
    }

    #[test]
    fn drawn_synthetic_stars_take_plan_06_s_draws_of_their_bodies() {
        let seed = Seed::new(SEED);
        let drawn = |i| {
            binary(1.0, 0.7, 30.0, 0.2)
                .system(id(i))
                .star_draws(SyntheticDraws::OfUniverse(seed))
                .build()
                .unwrap()
        };
        let (a, b) = (drawn(10), drawn(11));
        for context in [&a, &b] {
            for (n, star) in (0_u16..).zip(context.stars()) {
                let body = BodyId::new(context.id(), n);
                assert_eq!(star.draws(), &StarDraws::for_star(seed, body));
                assert_eq!(context.hierarchy().stars()[usize::from(n)].body(), body);
            }
        }
        assert_ne!(a.stars()[0].draws(), b.stars()[0].draws());
        assert_ne!(a.stars()[0].draws(), a.stars()[1].draws());
    }

    #[test]
    fn a_synthetic_binary_is_a_hierarchy_the_zones_read() {
        let context = binary(1.1, 0.9, 20.0, 0.3).build().unwrap();
        let h = context.hierarchy();
        assert_eq!(h.star_count(), 2);
        assert_eq!(context.stars().len(), 2);
        let (node, orbit) = h.pairs().next().unwrap();
        assert_eq!(node, h.root());
        assert_same_bits(orbit.semi_major_axis().value(), au(20.0).value());
        assert_same_bits(orbit.eccentricity().value(), 0.3);
        assert_same_bits(h.system_mass().value(), 1.1 + 0.9);
        assert_same_bits(
            orbit.gravitational_parameter().value(),
            GM_SUN * (1.1 + 0.9),
        );
        assert_eq!(h.first_star(h.root()).get(), 0);
        assert_eq!(h.pair_key(h.root()), Some(BodyId::new(id(7), 1)));
        for (slot, mass) in h.stars().iter().zip([1.1, 0.9]) {
            assert_eq!(slot.kind(), SlotKind::Star);
            assert_same_bits(slot.initial_mass().value(), mass);
        }
        // The zones are those of the same pair built for the zones' own tests.
        let star = |i: u8, m: f64| ZoneNode::component(i, SolarMasses::new(m), SlotKind::Star);
        let by_hand = ZoneHierarchy::new(ZoneNode::pair(
            star(0, 1.1),
            star(1, 0.9),
            au(20.0),
            Eccentricity::new(0.3).unwrap(),
        ))
        .unwrap();
        assert_eq!(context.zones(), stable_zones(&by_hand));
        let hosts: Vec<OrbitHost> = context.zones().iter().map(OrbitZone::host).collect();
        assert_eq!(
            hosts,
            [
                OrbitHost::Star(0),
                OrbitHost::Star(1),
                OrbitHost::Barycentre
            ]
        );
        assert_same_bits(
            context.tidal_radius().value(),
            solar_neighbourhood_tidal_radius(SolarMasses::new(2.0)).value(),
        );
    }

    #[test]
    fn a_companion_below_the_stellar_floor_is_a_brown_dwarf() {
        let dwarf = binary(0.9, 0.05, 5.0, 0.1).build().unwrap();
        assert_eq!(dwarf.hierarchy().stars()[1].kind(), SlotKind::BrownDwarf);
        let floor = binary(0.9, MIN_COMPANION_MASS.value(), 5.0, 0.1)
            .build()
            .unwrap();
        assert_eq!(floor.hierarchy().stars()[1].kind(), SlotKind::Star);
        assert!(
            binary(0.9, MIN_SUBSTELLAR_COMPANION_MASS.value(), 5.0, 0.1)
                .build()
                .is_ok()
        );
    }

    #[test]
    fn each_component_s_disc_reads_its_zero_age_state_and_its_own_rank() {
        let context = binary(0.9, 0.05, 5.0, 0.1)
            .fe_h(Dex::new(0.2))
            .star_draws(SyntheticDraws::OfUniverse(Seed::new(3)))
            .build()
            .unwrap();
        let zone_stars = context.zone_stars();
        assert_eq!(zone_stars.len(), 2);
        let coeffs = ZCoeffs::new(context.composition().z_fit());
        let star = zone_stars[0];
        let mass = SolarMasses::new(0.9);
        assert_same_bits(
            star.zams_luminosity.value(),
            zams::luminosity(mass, &coeffs).value(),
        );
        assert_same_bits(
            star.zams_radius.value(),
            zams::radius(mass, &coeffs).value(),
        );
        let dwarf = zone_stars[1];
        let young = substellar::cooling(
            SolarMasses::new(0.05),
            Years::new(1e7),
            context.composition(),
        )
        .unwrap();
        assert_same_bits(dwarf.zams_luminosity.value(), young.luminosity().value());
        assert_same_bits(dwarf.zams_radius.value(), young.radius().value());
        for (zone_star, model) in zone_stars.iter().zip(context.stars()) {
            assert_eq!(zone_star.disc_lifetime_rank, model.draws().disc_lifetime());
        }
    }

    #[test]
    fn the_solar_neighbourhood_s_tidal_radius_is_king_s_with_oort_s_constants() {
        let one = solar_neighbourhood_tidal_radius(SolarMasses::new(1.0));
        // Jiang and Tremaine's (G m ÷ 4ΩA)^⅓, with Ω = A − B.
        let (a, b) = (
            15.3e3 / METRES_PER_KILOPARSEC,
            -11.9e3 / METRES_PER_KILOPARSEC,
        );
        let expected = math::cbrt(GM_SUN / (4.0 * (a - b) * a));
        assert!((one.value() / expected - 1.0).abs() < 1e-15);
        assert!((one.value() / METRES_PER_PARSEC - 1.3722).abs() < 1e-4);
        let eight = solar_neighbourhood_tidal_radius(SolarMasses::new(8.0));
        assert!((eight.value() / one.value() - 2.0).abs() < 1e-14);
        // Plan 02's model of the Milky Way at the Sun-like point gives 1.296 pc, 5.5% less.
        let galaxy = galaxy();
        let here = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap();
        let model = galaxy
            .potential()
            .tidal_radius(SolarMasses::new(1.0), &PointLy::from(&here));
        let ratio = model.value() / one.value();
        assert!(
            (0.9..1.1).contains(&ratio),
            "{ratio}, {:?}",
            LightYears::from(model)
        );
    }

    #[test]
    fn an_encounter_environment_is_positive_and_finite() {
        let ok = EncounterEnvironment::new(
            PerCubicLightYear::new(1e4),
            KilometresPerSecond::new(10.0),
            SolarMasses::new(0.5),
        )
        .unwrap();
        assert_same_bits(ok.number_density().value(), 1e4);
        assert_same_bits(ok.velocity_dispersion().value(), 10.0);
        assert_same_bits(ok.mean_member_mass().value(), 0.5);
        let with = sun(0).encounter_environment(ok).build().unwrap();
        assert_eq!(with.encounter_environment(), Some(&ok));
        let (n, s, m) = (
            PerCubicLightYear::new(1.0),
            KilometresPerSecond::new(1.0),
            SolarMasses::new(1.0),
        );
        assert_eq!(
            EncounterEnvironment::new(PerCubicLightYear::new(0.0), s, m),
            Err(BuildEncounterEnvironmentError::NumberDensityNotPositive)
        );
        assert_eq!(
            EncounterEnvironment::new(n, KilometresPerSecond::new(f64::NAN), m),
            Err(BuildEncounterEnvironmentError::VelocityDispersionNotPositive)
        );
        assert_eq!(
            EncounterEnvironment::new(n, s, SolarMasses::new(-1.0)),
            Err(BuildEncounterEnvironmentError::MeanMassNotPositive)
        );
    }

    #[test]
    fn error_messages_are_lower_case_without_trailing_punctuation() {
        let messages = [
            BuildSystemContextError::MissingSystem.to_string(),
            BuildSystemContextError::MissingStars.to_string(),
            BuildSystemContextError::MissingAge.to_string(),
            BuildSystemContextError::MassOutsideRange {
                star: 0,
                mass: SolarMasses::new(-1.0),
            }
            .to_string(),
            BuildSystemContextError::CompanionOutweighsPrimary {
                primary: SolarMasses::new(0.5),
                companion: SolarMasses::new(0.6),
            }
            .to_string(),
            BuildSystemContextError::AgeOutsideRange(Years::new(2e10)).to_string(),
            BuildSystemContextError::MetallicityNotFinite(Dex::new(f64::NAN)).to_string(),
            BuildSystemContextError::TidalRadiusNotPositive(Metres::ZERO).to_string(),
            BuildSystemContextError::Orbit(BuildOrbitError::SemiMajorAxisNotPositive {
                metres: 0.0,
            })
            .to_string(),
            BuildSystemContextError::BeyondTidalCut {
                apoapsis: Metres::new(2.0),
                limit: Metres::new(1.0),
            }
            .to_string(),
            BuildSystemContextError::PeriodOutsideRange(Days::new(0.01)).to_string(),
            BuildSystemContextError::EccentricityAboveEnvelope {
                eccentricity: Eccentricity::new(0.5).unwrap(),
                e_max: 0.0,
            }
            .to_string(),
            BuildEncounterEnvironmentError::NumberDensityNotPositive.to_string(),
            BuildEncounterEnvironmentError::VelocityDispersionNotPositive.to_string(),
            BuildEncounterEnvironmentError::MeanMassNotPositive.to_string(),
        ];
        for message in messages {
            let first = message.chars().next().unwrap();
            assert!(!first.is_uppercase(), "{message}");
            assert!(!message.ends_with(['.', '!', '?']), "{message}");
        }
        let orbit = BuildSystemContextError::Orbit(BuildOrbitError::SemiMajorAxisNotPositive {
            metres: 0.0,
        });
        assert!(orbit.source().is_some());
        assert!(BuildSystemContextError::MissingAge.source().is_none());
    }
}
