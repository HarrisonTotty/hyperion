//! A system's zones on the wire: where each host's planets may orbit, its snow line, its planets'
//! plane and its habitable zone (plan 14, P14.T35.b).
//!
//! Zones belong to the hosts, not to the bodies, so they are not sections and a detail level does
//! not withhold them: they follow from the stars that `system_summary` already describes. Every
//! distance is in metres from the zone's host, a star's centre or a pair's barycentre.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::planetary::OrbitHostDto;

/// Which architecture class a host's planetary system belongs to (plan 14, P14.T4), after the
/// placer's fallback: the class a readout names for the system as a whole (P14.T43.b).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ArchitectureClassDto {
    /// No body above 0.02 Earth masses; belts allowed.
    Barren,
    /// Rocky planets inside the snow line and ice-rich bodies beyond it, with no giant.
    TerrestrialOnly,
    /// A compact chain of super-Earths and sub-Neptunes, or its dynamically hot variant.
    CompactMulti,
    /// A compact chain with one or two cold giants beyond the snow line.
    CompactWithColdGiant,
    /// A Solar System analogue: rocky planets, cold giants, ice giants and both belts.
    SolarLike,
    /// One or two eccentric giants scattered inward, and at most one small survivor.
    EccentricGiant,
    /// A giant at 0.1–1 au × √(L ÷ L☉), with small companions in half of systems.
    WarmGiant,
    /// A hot Jupiter with nothing else inside 100 days, and often an outer giant.
    HotJupiter,
    /// For a host under 0.08 M☉: a compact chain of small bodies.
    SubstellarCompact,
}

/// The plane a host's planets share, from which each planet's own inclination is drawn (plan 14,
/// P14.T8.d's `SystemPlane`).
///
/// Its angles are in the system frame, whose axes are the galactic axes, with the conventions of
/// [`OrbitDto`](crate::OrbitDto): the inclination is that of the plane's normal to galactic north,
/// the frame's +z, and the ascending node is measured from +x towards +y. A close binary's
/// circumbinary planets share the binary's plane.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SystemPlaneDto {
    /// The inclination of the plane's normal to galactic north, in radians, in `[0, π]`.
    pub inclination_rad: f64,
    /// The longitude of the plane's ascending node, in radians, in `[0, 2π)`.
    pub ascending_node_rad: f64,
}

/// A host's habitable zone at the answer's time: the distances of Kopparapu et al.'s five limits
/// (2013, with their erratum's coefficients), in metres from the host, from the star outwards (plan
/// 14, P14.T12.b).
///
/// The conservative zone runs from the moist greenhouse to the maximum greenhouse, and the
/// optimistic one from recent Venus to early Mars. For a host that orbits a companion the
/// companion's light, averaged over its orbit, pushes every limit outwards; a limit is `null` where
/// the companions alone give more than its flux, so that every orbit about the host lies inside
/// it, since JSON has no infinity. Every limit is zero about a host that gives no light.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct HabitableZoneDto {
    /// Recent Venus, the optimistic inner edge, in metres; `null` beyond every orbit.
    pub recent_venus_m: Option<f64>,
    /// The runaway greenhouse, in metres; `null` beyond every orbit.
    pub runaway_greenhouse_m: Option<f64>,
    /// The moist greenhouse, the conservative inner edge, in metres; `null` beyond every orbit.
    pub moist_greenhouse_m: Option<f64>,
    /// The maximum greenhouse, the conservative outer edge, in metres; `null` beyond every orbit.
    pub maximum_greenhouse_m: Option<f64>,
    /// Early Mars, the optimistic outer edge, in metres; `null` beyond every orbit.
    pub early_mars_m: Option<f64>,
    /// Whether a shining host's effective temperature lay outside the fit's 2,600–7,200 K, where
    /// it was held to the nearer end, so that the zone is an extrapolation.
    pub extrapolated: bool,
}

/// One stable zone of a system and what belongs to its host: the limits within which the host's
/// planets stay, its snow line, its planets' plane, its architecture class and its habitable zone
/// (plan 14, P14.T35.b, from P14.T3, T4, T8.d, T9 and T12).
///
/// A single star has one zone. A multiple system has at most one about each star, each inner pair
/// and the whole system (plan 14, design note 10), each the host of its own disc and planets; a
/// star or a pair whose companions leave it no room has none.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ZoneDto {
    /// The zone's host: `star` about one star, `pair` about an inner pair, `barycentre` about the
    /// whole of a multiple system. Never `body`.
    pub host: OrbitHostDto,
    /// The zone's inner limit, in metres from the host, positive and below `outer_m`: a pair's
    /// circumbinary stability limit; `null` for a star's zone, which its disc's inner edge bounds.
    pub inner_m: Option<f64>,
    /// The zone's outer limit, in metres from the host, positive and above `inner_m`, which the
    /// host's companions set; `null` at the top of the hierarchy, where the disc bounds it.
    pub outer_m: Option<f64>,
    /// The host's snow line, in metres, positive: from its zero-age luminosity, the pair's summed
    /// for a circumbinary zone (plan 14, design note 6), and fixed for the system's life.
    pub snow_line_m: f64,
    /// The plane the host's planets share.
    pub plane: SystemPlaneDto,
    /// The host's architecture class.
    pub architecture: ArchitectureClassDto,
    /// The host's habitable zone at the answer's time; `null` when the host has none at that
    /// time, as before it forms.
    pub habitable_zone: Option<HabitableZoneDto>,
}

#[cfg(test)]
pub(crate) mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::testing::{assert_wire_form, assert_wire_strings};

    /// The zone of a Sun-like single star: no limit from a hierarchy, a snow line of 2.26 au
    /// (the zero-age Sun's, 0.7 L☉) and the Sun's habitable zone today, 0.75–1.77 au optimistic
    /// and 0.99–1.69 au conservative.
    pub(crate) fn solar_zone() -> ZoneDto {
        ZoneDto {
            host: OrbitHostDto::Star { body_index: 0 },
            inner_m: None,
            outer_m: None,
            snow_line_m: 337_938_907_867.118_65,
            plane: SystemPlaneDto {
                inclination_rad: 1.0,
                ascending_node_rad: 2.5,
            },
            architecture: ArchitectureClassDto::SolarLike,
            habitable_zone: Some(HabitableZoneDto {
                recent_venus_m: Some(1.12e11),
                runaway_greenhouse_m: Some(1.47e11),
                moist_greenhouse_m: Some(1.48e11),
                maximum_greenhouse_m: Some(2.53e11),
                early_mars_m: Some(2.65e11),
                extrapolated: false,
            }),
        }
    }

    pub(crate) fn solar_zone_json() -> Value {
        json!({
            "host": { "type": "star", "body_index": 0 },
            "inner_m": null,
            "outer_m": null,
            "snow_line_m": 337_938_907_867.118_65,
            "plane": { "inclination_rad": 1.0, "ascending_node_rad": 2.5 },
            "architecture": "solar_like",
            "habitable_zone": {
                "recent_venus_m": 1.12e11,
                "runaway_greenhouse_m": 1.47e11,
                "moist_greenhouse_m": 1.48e11,
                "maximum_greenhouse_m": 2.53e11,
                "early_mars_m": 2.65e11,
                "extrapolated": false,
            },
        })
    }

    #[test]
    fn zone_wire_form() {
        assert_wire_form(&solar_zone(), solar_zone_json());
    }

    #[test]
    fn a_circumbinary_zone_has_an_inner_limit_and_its_pair_s_plane() {
        assert_wire_form(
            &ZoneDto {
                host: OrbitHostDto::Pair { key_body_index: 1 },
                inner_m: Some(4.5e10),
                outer_m: Some(1.2e12),
                snow_line_m: 4.1e11,
                plane: SystemPlaneDto {
                    inclination_rad: 0.25,
                    ascending_node_rad: 0.0,
                },
                architecture: ArchitectureClassDto::Barren,
                habitable_zone: None,
            },
            json!({
                "host": { "type": "pair", "key_body_index": 1 },
                "inner_m": 4.5e10,
                "outer_m": 1.2e12,
                "snow_line_m": 4.1e11,
                "plane": { "inclination_rad": 0.25, "ascending_node_rad": 0.0 },
                "architecture": "barren",
                "habitable_zone": null,
            }),
        );
    }

    #[test]
    fn a_limit_beyond_every_orbit_is_null() {
        // A star 20 au from a B-type companion of some 200 L☉, which alone gives more than the
        // maximum-greenhouse and early-Mars fluxes everywhere about the star, so that every orbit
        // lies inside those limits; its 15,000 K lies outside the fit.
        assert_wire_form(
            &HabitableZoneDto {
                recent_venus_m: Some(2.0e11),
                runaway_greenhouse_m: Some(2.6e11),
                moist_greenhouse_m: Some(2.7e11),
                maximum_greenhouse_m: None,
                early_mars_m: None,
                extrapolated: true,
            },
            json!({
                "recent_venus_m": 2.0e11,
                "runaway_greenhouse_m": 2.6e11,
                "moist_greenhouse_m": 2.7e11,
                "maximum_greenhouse_m": null,
                "early_mars_m": null,
                "extrapolated": true,
            }),
        );
    }

    #[test]
    fn architecture_class_strings() {
        assert_wire_strings(&[
            (ArchitectureClassDto::Barren, "barren"),
            (ArchitectureClassDto::TerrestrialOnly, "terrestrial_only"),
            (ArchitectureClassDto::CompactMulti, "compact_multi"),
            (
                ArchitectureClassDto::CompactWithColdGiant,
                "compact_with_cold_giant",
            ),
            (ArchitectureClassDto::SolarLike, "solar_like"),
            (ArchitectureClassDto::EccentricGiant, "eccentric_giant"),
            (ArchitectureClassDto::WarmGiant, "warm_giant"),
            (ArchitectureClassDto::HotJupiter, "hot_jupiter"),
            (
                ArchitectureClassDto::SubstellarCompact,
                "substellar_compact",
            ),
        ]);
    }

    #[test]
    fn system_plane_wire_form() {
        assert_wire_form(
            &SystemPlaneDto {
                inclination_rad: 3.0,
                ascending_node_rad: 6.25,
            },
            json!({ "inclination_rad": 3.0, "ascending_node_rad": 6.25 }),
        );
    }
}
