//! Planetary wire types (plan 14, P14.T35): the detail levels, the tagged sections every optional
//! part of a record is carried in and a body's orbit (T35.a, here), the records of bodies and
//! zones (T35.b, in `record` and `zones`), and the `system_bodies`, `body_detail` and
//! `body_events` requests with their answers (T35.c, in `requests`).
//!
//! They mirror the simulation's `planetary::record` without depending on it; body IDs are
//! [`BodyIdHex`](crate::BodyIdHex), beside the system IDs.

mod population;
mod record;
mod requests;
mod zones;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub use population::{
    BeltComponentDto, BeltCompositionDto, BeltDto, BeltGapDto, BeltSiteDto, CometaryHaloDto,
    PopulationDto, RingDto, RingGapDto, RingKindDto, RingMaterialDto,
};
pub use record::{
    BeltKindDto, BodyDetailDto, BodyHooksDto, BodyKindDto, BodyRecordDto, BodyStateDto,
    BodySummaryDto, BodySurfaceDto, BulkPropertiesDto, DestructionCauseDto, MassFractionsDto,
    MoonOriginDto, PlanetClassDto,
};
pub use requests::{
    BodyDetailRequest, BodyEventDto, BodyEventsDto, BodyEventsRequest, SystemBodiesDto,
    SystemBodiesRequest,
};
pub use zones::{ArchitectureClassDto, HabitableZoneDto, SystemPlaneDto, ZoneDto};

#[cfg(test)]
pub(crate) use record::tests as record_fixtures;
#[cfg(test)]
pub(crate) use requests::tests as requests_fixtures;

use crate::orbit::OrbitDto;
use crate::primitives::{BodyIdHex, UniverseTime};

/// How much of a body a record carries, from least to most (plan 14, design note 16).
///
/// Each level is a prefix of the next: a record at a level carries every section of the levels
/// below it. The levels cross the wire as names, never as numbers, so that a level can be added
/// between two without renumbering; they compare in the order listed. Until the knowledge overlay
/// exists the server grants whatever level a request asks for, and says which it granted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum DetailLevelDto {
    /// An unresolved contact: the body's ID and position, its kind unknown and its label withheld.
    Contact,
    /// Its mass and orbit only.
    MassAndOrbit,
    /// Its bulk: radius, density, class and equilibrium temperature.
    Bulk,
    /// Its surface: atmosphere, surface conditions, rotation and global figures.
    Surface,
    /// Everything, the hooks included: detail seed, composition, habitability and resources.
    Full,
}

/// An optional section of a body or system record, tagged with its state (ruling 34 of
/// 2026-09-22; plan 14, P14.T34's `Section<T>` on the wire).
///
/// The server sets every tag, since it is the authority on what its generator version computes
/// and on what a detail level withholds; the client never infers one. On the wire the tag is
/// `state`, and only `ok` carries a `value`: `{"state": "ok", "value": …}`,
/// `{"state": "not_modelled"}`. "None" is data, not a state: an airless world's atmosphere is `ok`
/// with no gas in it. A single value the generator does not compute inside a section it otherwise
/// models is not a state either: the section is `ok` and the value is absent.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(tag = "state", content = "value", rename_all = "snake_case")]
#[ts(export)]
pub enum SectionDto<T> {
    /// The section, with its value.
    Ok(T),
    /// The granted detail level withholds it; the display reads `NOT RESOLVED`.
    NotResolved,
    /// This generator version does not compute it, which must never be taken for "none"; the
    /// display reads `NOT YET MODELLED`, once per section.
    NotModelled,
    /// It has no meaning for this body's kind, such as a gas giant's surface; the display omits
    /// the row.
    NotApplicable,
}

/// A body's orbit: what it orbits, the orbit, and until when the elements hold.
///
/// The elements are those of plan 11's [`OrbitDto`], which carries the whole set and the
/// gravitational parameter, so that a client can propagate the body for drawing with nothing else
/// (plan 14, design note 18); every number a readout shows still comes from the server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BodyOrbitDto {
    /// What the body orbits, whose position the orbit is relative to.
    pub parent: OrbitHostDto,
    /// The orbit about it: in the system frame for a planet, and for a moon or a ring in its
    /// planet's body frame, referred to the planet's equator (plan 14, phase D).
    pub orbit: OrbitDto,
    /// The last instant at which the elements hold: the next change of the body's state or orbit
    /// that the server knows of (a host's sudden death, the body destroyed or unbound), or the
    /// end of the current drift cell of an evolving orbit, past which a display that has moved
    /// its time asks the server again and a scene re-sends the body; `null` when no change falls
    /// inside the clock window, 1,000 years either side of the epoch.
    pub valid_until: Option<UniverseTime>,
    /// For an orbit whose elements evolve (a moon's tidal recession, circularisation, a host's
    /// mass loss), how they change through the current drift cell; absent for an orbit that holds
    /// (plan 14, P14.T45.b).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub drift: Option<OrbitDriftDto>,
}

/// How an evolving orbit's elements change through one drift cell (plan 14, P14.T45).
///
/// `orbit` holds exactly at `reference`, the cell's start. At a time t up to `valid_until`, with
/// Δt = t − `reference` in seconds (the difference of the whole seconds plus that of the
/// nanoseconds ÷ 10⁹): the semi-major axis is a + ȧ Δt, the eccentricity e + ė Δt, the mean
/// anomaly the elements' own plus ½ ṅ Δt², and the mean motion in the velocity 2π ÷ P + ṅ Δt. The
/// simulation places the body by the same formula, so a client that applies it agrees with the
/// server as for an orbit that holds.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OrbitDriftDto {
    /// The instant the elements hold at exactly.
    pub reference: UniverseTime,
    /// ȧ, m s⁻¹.
    pub semi_major_axis_rate_m_per_s: f64,
    /// ė, s⁻¹.
    pub eccentricity_rate_per_s: f64,
    /// ṅ, rad s⁻².
    pub mean_motion_rate_rad_per_s2: f64,
}

/// What a body orbits: a star, the barycentre of a pair of stars or of the whole system, or
/// another body (ruling 53 of 2026-09-22; the simulation's `planetary::placement::OrbitHost`).
///
/// A circumstellar planet orbits its star, a circumbinary planet a pair, which is no body, and a
/// moon or a ring its planet. The stars and pairs are those of the system's
/// [`HierarchyDto`](crate::HierarchyDto), whose positions a client already computes to place the
/// stars: a pair is the hierarchy's pair node whose outer member's first star has the given body
/// index.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum OrbitHostDto {
    /// A star of the system, or a brown-dwarf companion: a circumstellar orbit.
    Star {
        /// The star's body index, 0–15 (plan 14, design note 3).
        body_index: u16,
    },
    /// A pair of the system's stars, orbited about its barycentre: a circumbinary orbit.
    Pair {
        /// The body index k, 1–15, of the star the pair is keyed by: the first star of its outer
        /// member, the one its orbit brought in (plan 11, design note 5).
        key_body_index: u16,
    },
    /// The barycentre of the whole system, for what is bound to the system but to no star or pair,
    /// such as the cometary halo of a multiple system.
    Barycentre,
    /// Another body, for a moon's or a ring's planet.
    Body {
        /// The body.
        id: BodyIdHex,
    },
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::orbit::tests::{wide_pair_orbit, wide_pair_orbit_json};
    use crate::testing::{assert_wire_form, assert_wire_strings};

    const SYSTEM: u64 = 0x0200_0800_2000_0000;

    fn planet_orbit() -> BodyOrbitDto {
        BodyOrbitDto {
            parent: OrbitHostDto::Star { body_index: 0 },
            orbit: wide_pair_orbit(),
            valid_until: None,
            drift: None,
        }
    }

    fn planet_orbit_json() -> Value {
        json!({
            "parent": { "type": "star", "body_index": 0 },
            "orbit": wide_pair_orbit_json(),
            "valid_until": null,
        })
    }

    #[test]
    fn body_orbit_wire_form() {
        assert_wire_form(&planet_orbit(), planet_orbit_json());
    }

    #[test]
    fn an_orbit_whose_elements_change_says_until_when_they_hold() {
        assert_wire_form(
            &BodyOrbitDto {
                valid_until: Some(UniverseTime {
                    seconds: 9_467_280_000,
                    nanos: 500_000_000,
                }),
                ..planet_orbit()
            },
            json!({
                "parent": { "type": "star", "body_index": 0 },
                "orbit": wide_pair_orbit_json(),
                "valid_until": { "seconds": 9_467_280_000_i64, "nanos": 500_000_000 },
            }),
        );
    }

    #[test]
    fn an_evolving_orbit_carries_its_drift() {
        assert_wire_form(
            &BodyOrbitDto {
                valid_until: Some(UniverseTime {
                    seconds: 33_554_432,
                    nanos: 0,
                }),
                drift: Some(OrbitDriftDto {
                    reference: UniverseTime {
                        seconds: 0,
                        nanos: 0,
                    },
                    semi_major_axis_rate_m_per_s: 2.5e-9,
                    eccentricity_rate_per_s: -1.5e-20,
                    mean_motion_rate_rad_per_s2: -1.25e-22,
                }),
                ..planet_orbit()
            },
            json!({
                "parent": { "type": "star", "body_index": 0 },
                "orbit": wide_pair_orbit_json(),
                "valid_until": { "seconds": 33_554_432, "nanos": 0 },
                "drift": {
                    "reference": { "seconds": 0, "nanos": 0 },
                    "semi_major_axis_rate_m_per_s": 2.5e-9,
                    "eccentricity_rate_per_s": -1.5e-20,
                    "mean_motion_rate_rad_per_s2": -1.25e-22,
                },
            }),
        );
    }

    #[test]
    fn an_orbit_without_a_drift_key_reads_as_one_that_holds() {
        let read: BodyOrbitDto = serde_json::from_value(planet_orbit_json()).unwrap();
        assert_eq!(read.drift, None);
    }

    #[test]
    fn a_circumstellar_orbit_names_its_star() {
        assert_wire_form(
            &OrbitHostDto::Star { body_index: 2 },
            json!({ "type": "star", "body_index": 2 }),
        );
    }

    #[test]
    fn a_circumbinary_orbit_names_its_pair_by_its_key_star() {
        assert_wire_form(
            &OrbitHostDto::Pair { key_body_index: 1 },
            json!({ "type": "pair", "key_body_index": 1 }),
        );
    }

    #[test]
    fn an_orbit_about_the_system_names_its_barycentre() {
        assert_wire_form(&OrbitHostDto::Barycentre, json!({ "type": "barycentre" }));
    }

    #[test]
    fn a_moon_orbits_its_planet() {
        assert_wire_form(
            &OrbitHostDto::Body {
                id: BodyIdHex::from_parts(SYSTEM, 0x0300),
            },
            json!({ "type": "body", "id": "0200080020000000.0300" }),
        );
    }

    #[test]
    fn an_orbit_host_with_a_malformed_body_id_is_rejected() {
        let error = serde_json::from_value::<OrbitHostDto>(json!({
            "type": "body",
            "id": "0200080020000000.301",
        }))
        .unwrap_err();
        assert!(
            error.to_string().contains("a full stop and 4 lowercase"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn section_ok_wire_form() {
        assert_wire_form(
            &SectionDto::Ok(planet_orbit()),
            json!({ "state": "ok", "value": planet_orbit_json() }),
        );
    }

    #[test]
    fn section_ok_carries_a_list_as_its_value() {
        // A planet's moons are a list, which only an adjacent tag can carry beside `state`.
        assert_wire_form(
            &SectionDto::Ok(vec![BodyIdHex::from_parts(SYSTEM, 0x0101)]),
            json!({ "state": "ok", "value": ["0200080020000000.0101"] }),
        );
        assert_wire_form(
            &SectionDto::<Vec<BodyIdHex>>::Ok(Vec::new()),
            json!({ "state": "ok", "value": [] }),
        );
    }

    #[test]
    fn section_not_resolved_wire_form() {
        assert_wire_form(
            &SectionDto::<BodyOrbitDto>::NotResolved,
            json!({ "state": "not_resolved" }),
        );
    }

    #[test]
    fn section_not_modelled_wire_form() {
        assert_wire_form(
            &SectionDto::<BodyOrbitDto>::NotModelled,
            json!({ "state": "not_modelled" }),
        );
    }

    #[test]
    fn section_not_applicable_wire_form() {
        assert_wire_form(
            &SectionDto::<BodyOrbitDto>::NotApplicable,
            json!({ "state": "not_applicable" }),
        );
    }

    #[test]
    fn section_ok_without_its_value_is_rejected() {
        let error = serde_json::from_value::<SectionDto<BodyOrbitDto>>(json!({ "state": "ok" }))
            .unwrap_err();
        assert!(
            error.to_string().contains("missing field `value`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn section_with_an_unknown_state_is_rejected() {
        let error = serde_json::from_value::<SectionDto<BodyOrbitDto>>(json!({ "state": "none" }))
            .unwrap_err();
        assert!(
            error.to_string().contains("unknown variant `none`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn detail_level_strings() {
        assert_wire_strings(&[
            (DetailLevelDto::Contact, "contact"),
            (DetailLevelDto::MassAndOrbit, "mass_and_orbit"),
            (DetailLevelDto::Bulk, "bulk"),
            (DetailLevelDto::Surface, "surface"),
            (DetailLevelDto::Full, "full"),
        ]);
    }

    #[test]
    fn detail_levels_order_from_least_to_most() {
        let levels = [
            DetailLevelDto::Contact,
            DetailLevelDto::MassAndOrbit,
            DetailLevelDto::Bulk,
            DetailLevelDto::Surface,
            DetailLevelDto::Full,
        ];
        assert!(levels.windows(2).all(|pair| pair[0] < pair[1]));
    }
}
