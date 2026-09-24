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
//! - [`fate`]: the states a body can be in at a time, which the record carries; the fate transform
//!   that produces them is P14.T28's.
//! - [`placement`]: placing planets: the Hill-spacing primitives and the spacing draw (P14.T6),
//!   the masses (P14.T7), the class placers that turn a host's class, disc and zone into planets
//!   on orbits with D5's second fallback (P14.T8), and the stable zones of multiple systems with
//!   their hosts and discs (P14.T9).
//! - [`params`]: the parameters that belong to the generator version, as named constants.
//! - [`context`] and [`system`]: what the stage reads from the stages above, and the assembled
//!   generator. Documentation only until their tasks (P14.T1.d and T30).
//!
//! The vertical slice to the `SYSTEM` display (ruling 33) builds these pieces ahead of the stages
//! that will feed them. Each takes what a later stage supplies as a plain argument: the disc takes
//! its host's mass, \[Fe/H\], zero-age luminosity and radius, its lifetime and its truncation
//! radii, not a [`context`] (P14.T1.d) or an orbit zone (P14.T9); the class draw takes the host's
//! mass and \[Fe/H\], its disc, the zone's outer limit if any and whether the host is in a close
//! binary.
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

pub mod architecture;
pub mod context;
pub mod derive;
pub mod disc;
pub mod error;
pub mod fate;
pub mod index;
pub mod params;
pub mod placement;
pub mod record;
pub mod system;

pub use error::{DecodeBodyIndexError, EncodeBodyIndexError, ResolveBodyError};
pub use index::{BodyIndex, BodySlot, BodySub};
