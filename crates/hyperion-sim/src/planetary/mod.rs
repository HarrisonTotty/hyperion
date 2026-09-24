//! Planetary systems (plan 14): what orbits each host, and where it is at any time.
//!
//! A system's planets come from a disc derived from its host, an architecture class whose
//! frequency depends on the host's mass and metallicity, and placement under dynamical
//! constraints; everything else about a body is computed from those (the brainstorm's "Planetary
//! systems", approach C). Draws are primordial: a system is generated as it was born, from its
//! hosts' zero-age properties, and time enters only through a closed-form fate transform (plan 14,
//! design note 1). Draws that are joint properties of a system (disc, class, counts, spacings,
//! masses) are made on system-level streams with the orbit host and slot in the draw number, and
//! everything that belongs to one body on that body's own streams (design note 4).
//!
//! # What is built
//!
//! - [`architecture`]: the architecture classes, their frequency model and their templates, the
//!   written definition the brainstorm asks for (P14.T4, T5).
//! - [`index`]: [`BodyIndex`], the layout of the 16-bit body index inside plan 01's
//!   [`BodyId`](crate::id::BodyId) (design note 3), decodable without generating anything.
//! - [`error`]: the errors of encoding, decoding and resolving a body.
//! - [`disc`]: the protoplanetary disc of one orbit host, its budget and ruler (design note 5).
//! - [`derive`](mod@derive): the derivation of a body's properties; so far its radius and
//!   composition (P14.T11.a–c, with rocky compositions from the observed spread, ruling 53), giant
//!   planets' radii and compositions (T11.d), irradiation and habitable zone (T12), and limits
//!   (Roche, Hill and satellite stability, T15), with their assembly `derive_body` (T16.a).
//! - [`record`]: what a query returns and how it degrades, the body record and the system snapshot
//!   with their sections' four states (P14.T34, ruling 34).
//! - [`fate`]: the fate transform, a body's state and orbit at a time (P14.T28.a–c): formation on
//!   young hosts, expansion and engulfment on evolved ones, and supernovae with a zero kick until
//!   P06.T19 (ruling 33); [`hosts`] holds its closed forms.
//! - [`placement`]: placing planets: the Hill-spacing primitives and the spacing draw (P14.T6),
//!   the masses (P14.T7), the class placers that turn a host's class, disc and zone into planets
//!   on orbits with D5's second fallback (P14.T8), and the stable zones of multiple systems with
//!   their hosts and discs (P14.T9).
//! - [`params`]: the parameters that belong to the generator version, as named constants.
//! - [`context`]: [`SystemContext`], everything the stage reads from the stages above, for a real
//!   system ([`SystemContext::for_system`]) or a synthetic host ([`SystemContext::builder`])
//!   (P14.T1.d). The crate's `testing` feature adds `testing`: synthetic hosts and samples of real
//!   systems for the statistical tests.
//! - [`system`]: the assembled generator, [`generate`] and [`generate_planets`], and the
//!   [`PlanetarySystem`] they return with its queries at a time, `body_at`, `snapshot_at`,
//!   `position_at` and `habitable_zone_at` (P14.T30.a–b).
//! - [`label`]: bodies' labels for people, `A b` onwards (design note 22, P14.T30.c).
//!
//! The vertical slice to the `SYSTEM` display (ruling 33) builds these pieces ahead of the stages
//! that will feed them. Each takes what a later stage supplies as a plain argument: the disc takes
//! its host's mass, \[Fe/H\], zero-age luminosity and radius, its lifetime and its truncation
//! radii, not a [`context`] (P14.T1.d) or an orbit zone (P14.T9); the class draw takes the host's
//! mass and \[Fe/H\], its disc, the zone's outer limit if any and whether the host is in a close
//! binary. The assembled generator (P14.T30) passes them from the context and its zones.
//!
//! # Consumed items, by their paths in the code
//!
//! The items of plan 14's "Consumes" that the pieces built so far use (P14.T1.a records the rest
//! as they are wired):
//!
//! | Plan | Item | Path |
//! | ---- | ---- | ---- |
//! | 01 | streams, keys, tags | [`rng::Stream::open`](crate::rng::Stream::open), [`rng::ObjectKey`](crate::rng::ObjectKey) (`From<SystemId>`), [`rng::tags::PLANET_DISC`](crate::rng::tags::PLANET_DISC), [`rng::tags::PLANET_SPACING`](crate::rng::tags::PLANET_SPACING), [`rng::tags::PLANET_CLASS`](crate::rng::tags::PLANET_CLASS), [`Seed`](crate::Seed) |
//! | 01 | integer-threshold decisions | [`rng::Mark::pick_weighted`](crate::rng::Mark::pick_weighted) (weights and a bound), the same answer as [`rng::Thresholds::from_weights`](crate::rng::Thresholds::from_weights) with [`rng::Mark::pick`](crate::rng::Mark::pick) |
//! | 02 | the stellar mass function's lower limit | [`galaxy::imf::MASS_LIMIT_LO`](crate::galaxy::imf::MASS_LIMIT_LO), below which a host takes the substellar class row |
//! | 01 | IDs | [`id::SystemId`](crate::id::SystemId), [`id::BodyId`](crate::id::BodyId) (every `u16` a valid index there; its meaning is [`index`]'s) |
//! | 01 | maths | [`math`](crate::math): `sqrt` is IEEE; `cbrt`, `exp`, `exp_m1`, `exp10` through the pinned `libm` |
//! | 01 | units and constants | [`units`](crate::units) (`SolarMasses`, `EarthMasses`, `Metres`, `Megayears`, `Dex`, `SolarLuminosities`, `SolarRadii`, and `KilogramsPerSquareMetre` and `KilogramsPerCubicMetre`, added by this plan); [`units::consts`](crate::units::consts) (`GM_SUN`, `GRAVITATIONAL_CONSTANT`, `METRES_PER_AU`, `SOLAR_RADIUS_M`, `SOLAR_MASS_KG`, `EARTH_MASS_KG`); μ is `GM_SUN` × m until `units::GravitationalParameter` exists |
//! | 06 | zero-age luminosity and radius (design note 6) | [`stellar::sse::zams::luminosity`](crate::stellar::sse::zams::luminosity) and [`radius`](crate::stellar::sse::zams::radius), which take `(SolarMasses, &ZCoeffs)`; the caller passes their values to [`disc::DiscHost::new`] |
//! | 06 | the disc's lifetime (ruling 33) | [`stellar::premain::disc_lifetime`](crate::stellar::premain::disc_lifetime) of [`StarDraws::disc_lifetime`](crate::stellar::draws::StarDraws::disc_lifetime), a [`UnitUniform`](crate::stellar::draws::UnitUniform); the caller passes the result to [`disc::derive`] |
//! | 06 | \[Fe/H\] | [`Composition::fe_h`](crate::stellar::Composition::fe_h), as drawn |
//! | 11 | stars in slot `0x00` | [`stellar::multiplicity::STAR_BODY_INDEX_END`](crate::stellar::multiplicity::STAR_BODY_INDEX_END) = 16; [`index::STELLAR_SUB_END`] states the same bound |
//! | 11 | a planet's orbit (P14.T8) | [`orbit::KeplerElements::from_semi_major_axis`](crate::orbit::KeplerElements::from_semi_major_axis) with [`orbit::Orientation`](crate::orbit::Orientation) and μ = G (M★ + m) as [`units::GravitationalParameter`](crate::units::GravitationalParameter); [`rng::tags::PLANET_COUNT`](crate::rng::tags::PLANET_COUNT), [`PLANET_PLANE`](crate::rng::tags::PLANET_PLANE) and [`PLANET_ORBIT`](crate::rng::tags::PLANET_ORBIT) |
//! | 11 | a pair's orbit | [`orbit::Eccentricity`](crate::orbit::Eccentricity) and a semi-major axis in [`Metres`](crate::units::Metres), as `KeplerElements` gives them |
//! | 11 | the hierarchy | [`stellar::multiplicity::SystemHierarchy`](crate::stellar::multiplicity::SystemHierarchy), read through [`placement::ZoneHierarchy`]'s `From<&SystemHierarchy>`: each star's index, initial mass and [`SlotKind`](crate::stellar::multiplicity::SlotKind), and each pair's members and orbit at birth |
//! | 02 | the sphere of influence (P14.T1.d, until plan 09) | [`PotentialTables::tidal_radius`](crate::galaxy::potential::PotentialTables::tidal_radius) `(SolarMasses, &PointLy) -> Metres` of [`Galaxy::potential`](crate::galaxy::Galaxy::potential), at [`PointLy::from`](crate::galaxy::PointLy) the record's epoch position |
//! | 03 | resolving a system (P14.T1.d) | [`galaxy::placement::resolve`](crate::galaxy::placement::resolve) and [`ResolveSystemError`](crate::galaxy::placement::ResolveSystemError) (`NoSuchSystem`, `LayerNotGenerated`, `KindNotGenerated`); [`SystemRecord`](crate::galaxy::placement::SystemRecord)'s `id`, `epoch_position`, `primary_initial_mass` and `age_at_epoch`, and the arithmetic of its `age_at` and `existence_at` ([`Existence`](crate::galaxy::placement::Existence)) |
//! | 06 | the stars (P14.T1.d, ruling 34) | [`stellar::system::SystemStars`](crate::stellar::system::SystemStars)'s `generate` and `stars`; [`StarModel`](crate::stellar::system::StarModel), `new` for synthetic hosts with [`MAX_STAR_MASS`](crate::stellar::system::MAX_STAR_MASS); [`StarDraws::for_star`](crate::stellar::draws::StarDraws::for_star) and [`StarDraws::median`](crate::stellar::draws::StarDraws::median) |
//! | 06 | \[Fe/H\] of a real system (P14.T1.d) | [`stellar::system::draw_metallicity`](crate::stellar::system::draw_metallicity), through `SystemStars`; [`Composition::from_fe_h`](crate::stellar::Composition::from_fe_h) with no helium excess for a synthetic host; \[α/Fe\] is in no plan's code yet and is `None` |
//! | 06 | a brown dwarf's zero-age state (design note 6) | [`stellar::substellar::cooling`](crate::stellar::substellar::cooling) at 10 Myr, a [`StarState`](crate::stellar::StarState) |
//! | 11 | a real system's hierarchy (P14.T1.d) | [`SystemStars::hierarchy`](crate::stellar::system::SystemStars::hierarchy), the one [`draw_hierarchy`](crate::stellar::multiplicity::draw_hierarchy) draws under [`MultiplicityContext::Free`](crate::stellar::multiplicity::MultiplicityContext::Free) (P11.T2.c) |
//! | 11 | a synthetic binary (P14.T1.d) | [`TIDAL_CUT_SHARE`](crate::stellar::multiplicity::TIDAL_CUT_SHARE), [`MIN_COMPANION_MASS`](crate::stellar::multiplicity::MIN_COMPANION_MASS) and [`MIN_SUBSTELLAR_COMPANION_MASS`](crate::stellar::multiplicity::MIN_SUBSTELLAR_COMPANION_MASS); the hierarchy through `SystemHierarchy`'s crate-private `single` and `binary` |
//! | 01 | time and the clock window (P14.T1.d) | [`time::UniverseTime`](crate::time::UniverseTime) (`since_epoch`), [`time::CLOCK_WINDOW_H`](crate::time::CLOCK_WINDOW_H) for the youngest age, and [`units`](crate::units)' `Years`, `Days`, `HeliumExcess`, `PerCubicLightYear` and `KilometresPerSecond`, with [`units::consts::METRES_PER_KILOPARSEC`](crate::units::consts::METRES_PER_KILOPARSEC) |
//! | 02, 03 | the sample of real systems (`testing`, P14.T1.d) | [`Galaxy::from_params`](crate::galaxy::Galaxy::from_params) with [`GalaxyParams::milky_way_like`](crate::galaxy::params::GalaxyParams::milky_way_like); [`galaxy::placement::generate_cell`](crate::galaxy::placement::generate_cell), [`STELLAR_LAYERS`](crate::galaxy::placement::STELLAR_LAYERS) and each layer's [`MassBand`](crate::galaxy::imf::MassBand); [`galaxy::query::QuerySphere`](crate::galaxy::query::QuerySphere), [`cells_in_sphere`](crate::galaxy::query::cells_in_sphere) and [`count_cells_in_sphere`](crate::galaxy::query::count_cells_in_sphere) |
//! | 11 | a synthetic binary's period and eccentricity (P14.T1.d) | [`LOG_PERIOD_MIN`](crate::stellar::multiplicity::LOG_PERIOD_MIN), [`LOG_PERIOD_MAX`](crate::stellar::multiplicity::LOG_PERIOD_MAX), [`MultiplicityModel::eccentricity_distribution`](crate::stellar::multiplicity::MultiplicityModel::eccentricity_distribution)'s `e_max`, and [`orbit::OpenOrbit::MIN_ECCENTRICITY`](crate::orbit::OpenOrbit::MIN_ECCENTRICITY) |
//! | 09 | sphere of influence and encounter environment | not built: T1.d's interim rule, the galactic tidal radius and `None`; [`context::EncounterEnvironment`] holds what P14.T29 reads of a feature member |
//! | 13 | free-floating hosts, `SystemRecord::kind` | not built: every context is [`HostKind::Stellar`] |
//! | 06 | a star at any time (ruling 34) | [`stellar::system::StarModel`](crate::stellar::system::StarModel): `state_at`, `age_at`, `max_radius_until`, `death`, `remnant` and `natal_kick`, which [`fate`] reads |
//! | 06 | how a star dies | [`stellar::remnant::Death`](crate::stellar::remnant::Death), its [`DeathKind::is_sudden`](crate::stellar::remnant::DeathKind::is_sudden) and [`ProgenitorAtDeath`](crate::stellar::remnant::ProgenitorAtDeath), and [`NatalKick`](crate::stellar::remnant::NatalKick) (`None` until P06.T19) |
//! | 11, 14 | an orbit's expansion and its inverse | [`orbit::KeplerElements::scaled`](crate::orbit::KeplerElements::scaled) (P14.T2.a) and [`orbit::elements_from_state`](crate::orbit::elements_from_state), whose [`orbit::Orbit`](crate::orbit::Orbit) is bound or open (P14.T2.c) |
//! | 11 | where the stars are (P14.T30.b) | [`stellar::multiplicity::star_positions_at`](crate::stellar::multiplicity::star_positions_at)'s walk of [`SystemHierarchy`](crate::stellar::multiplicity::SystemHierarchy) (`root`, `node`, `node_mass`, `pairs` and each pair's [`orbit::KeplerElements::relative_state_at`](crate::orbit::KeplerElements::relative_state_at)), which `position_at` follows to a star or a pair's barycentre |
//! | 01 | a planet's radius rank (P14.T30.a) | [`rng::tags::PLANET_RADIUS`](crate::rng::tags::PLANET_RADIUS) (`Body`), opened with `ObjectKey::from(BodyId)` |

pub mod architecture;
pub mod context;
pub mod derive;
pub mod disc;
pub mod error;
pub mod fate;
pub mod hosts;
pub mod index;
pub mod label;
pub mod params;
pub mod placement;
pub mod record;
pub mod system;
#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use context::{HostKind, SystemContext};
pub use error::{DecodeBodyIndexError, EncodeBodyIndexError, ResolveBodyError};
pub use index::{BodyIndex, BodySlot, BodySub};
pub use label::BodyLabel;
pub use system::{Body, PlanetaryHost, PlanetarySystem, generate, generate_planets};
