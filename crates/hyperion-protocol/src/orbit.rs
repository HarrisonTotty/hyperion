//! Orbits on the wire, and the hierarchy of a system's stars that they hold together (plan 11,
//! P11.T13, the part the `SYSTEM` display needs first).
//!
//! By ruling 33 of 2026-09-22 an orbit crosses the wire as its whole element set, with the
//! gravitational parameter, so that a client can place a body and propagate it itself for drawing
//! (plan 14, design note 18) with nothing else; every number a readout shows still comes from the
//! server. The same [`OrbitDto`] carries a pair of stars in a [`HierarchyDto`] and a planet or
//! moon in plan 14's [`BodyOrbitDto`](crate::BodyOrbitDto), so its shape is fixed here, before any
//! client code reads it. Every field name carries its unit.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A bound relative orbit: the Keplerian elements at the epoch and the gravitational parameter.
///
/// It is the orbit of a secondary relative to its primary: of a pair's outer member's barycentre
/// about its inner member's, or of a body about its parent. Angles are in radians in the frame the
/// server gives the orbit in: for stars and planets the system frame, whose axes are the galactic
/// axes; for a moon or a ring its planet's body frame, referred to the planet's equator (plan 14,
/// phase D). The inclination is measured from the frame's +z, the
/// ascending node from +x towards +y, and the argument of periapsis from the ascending node in the
/// direction of motion. The elements hold at the epoch, `UniverseTime` zero: at time t the mean
/// anomaly is `mean_anomaly_at_epoch_rad` plus 2π times the fraction of a period elapsed since
/// then. The period and the semi-major axis agree with `mu_m3_s2` by Kepler's third law, P = 2π
/// √(a³ ÷ μ), to the rounding of whichever of the two the server derived.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OrbitDto {
    /// The orbital period P, in seconds, finite and positive.
    pub period_s: f64,
    /// The semi-major axis a, in metres, finite and positive.
    pub semi_major_axis_m: f64,
    /// The eccentricity e, in `[0, 1)`. Bound orbits from e = 0.9999 up, where Kepler's equation
    /// is ill-conditioned, are carried in the open form of plan 14 instead (ruling 39 of
    /// 2026-09-22), so the elements here stay below that.
    pub eccentricity: f64,
    /// The inclination i, in radians, in `[0, π]`.
    pub inclination_rad: f64,
    /// The longitude of the ascending node Ω, in radians, in `[0, 2π)`.
    pub ascending_node_rad: f64,
    /// The argument of periapsis ω, in radians, in `[0, 2π)`.
    pub argument_of_periapsis_rad: f64,
    /// The mean anomaly at the epoch M₀, in radians, in `[0, 2π)`.
    pub mean_anomaly_at_epoch_rad: f64,
    /// The gravitational parameter μ = G (M₁ + M₂) of the two bodies, in m³ s⁻².
    pub mu_m3_s2: f64,
}

/// A system's stars and the orbits that hold them together, as a flat list of nodes.
///
/// The nodes are listed depth first from the root, node 0, which is the whole system: each pair's
/// inner member comes before its outer one, and the stars appear in body-index order. A single
/// star is one star node. A system that has not yet formed at the summary's time has no stars and
/// no nodes. A client places every star about the system's barycentre by walking the list from the
/// root: a pair's orbit gives its outer member's barycentre relative to its inner member's, r, and
/// each member sits about the pair's barycentre by the other member's share of the pair's mass M,
/// the inner one at −(M₂ ÷ M) r and the outer one at +(M₁ ÷ M) r, where M₁ and M₂ are the inner and
/// outer members' masses, each the sum of its stars' `mass_msun`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct HierarchyDto {
    /// Every node, depth first from the root.
    pub nodes: Vec<HierarchyNodeDto>,
}

/// One node of a [`HierarchyDto`]: a star, or a pair of nodes on a relative orbit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum HierarchyNodeDto {
    /// A star, or a brown-dwarf companion.
    Star {
        /// Its body index, 0 for the primary and 1–15 for companions (plan 14, design note 3),
        /// which is also the `body_index` of its entry in the summary's `stars`.
        body_index: u16,
        /// The mass the server places the star by about each barycentre, in M☉: its initial mass
        /// in this generator version, as the orbits' gravitational parameters are.
        mass_msun: f64,
    },
    /// Two nodes on a relative orbit: the outer member's barycentre about the inner member's.
    Pair {
        /// The inner member's index in the node list, which holds the pair's lower-indexed stars.
        inner: u8,
        /// The outer member's index in the node list.
        outer: u8,
        /// The relative orbit, whose `mu_m3_s2` is G times the pair's total mass.
        orbit: OrbitDto,
    },
}

#[cfg(test)]
pub(crate) mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::testing::assert_wire_form;

    /// An orbit like Alpha Centauri AB's (Pourbaix and Boffin 2016: 79.9 yr, 23.5 au): a = 23.5
    /// au and μ for 1.99 M☉, with the period that Kepler's third law gives them, 80.8 yr.
    pub(crate) fn wide_pair_orbit() -> OrbitDto {
        OrbitDto {
            period_s: 2_548_768_078.333_065_5,
            semi_major_axis_m: 3_515_625_000_000.0,
            eccentricity: 0.5,
            inclination_rad: 1.5,
            ascending_node_rad: 3.5,
            argument_of_periapsis_rad: 4.25,
            mean_anomaly_at_epoch_rad: 0.125,
            mu_m3_s2: 2.640_625e20,
        }
    }

    /// The wire form of [`wide_pair_orbit`].
    pub(crate) fn wide_pair_orbit_json() -> Value {
        json!({
            "period_s": 2_548_768_078.333_065_5,
            "semi_major_axis_m": 3_515_625_000_000.0,
            "eccentricity": 0.5,
            "inclination_rad": 1.5,
            "ascending_node_rad": 3.5,
            "argument_of_periapsis_rad": 4.25,
            "mean_anomaly_at_epoch_rad": 0.125,
            "mu_m3_s2": 2.640_625e20,
        })
    }

    #[test]
    fn orbit_wire_form() {
        assert_wire_form(&wide_pair_orbit(), wide_pair_orbit_json());
    }

    #[test]
    fn orbit_accepts_whole_numbers_for_its_floats() {
        // JavaScript writes 0.0 as `0`, so every element must accept an integer literal.
        let orbit: OrbitDto = serde_json::from_value(json!({
            "period_s": 86_400,
            "semi_major_axis_m": 3_000_000_000_u64,
            "eccentricity": 0,
            "inclination_rad": 0,
            "ascending_node_rad": 0,
            "argument_of_periapsis_rad": 0,
            "mean_anomaly_at_epoch_rad": 0,
            "mu_m3_s2": 132_712_440_041_279_419_u64,
        }))
        .unwrap();
        assert_eq!(orbit.period_s.to_bits(), 86_400.0_f64.to_bits());
        assert_eq!(orbit.eccentricity.to_bits(), 0.0_f64.to_bits());
    }

    #[test]
    fn a_single_star_is_one_star_node() {
        assert_wire_form(
            &HierarchyDto {
                nodes: vec![HierarchyNodeDto::Star {
                    body_index: 0,
                    mass_msun: 1.0,
                }],
            },
            json!({ "nodes": [{ "type": "star", "body_index": 0, "mass_msun": 1.0 }] }),
        );
    }

    #[test]
    fn a_hierarchical_triple_is_listed_depth_first() {
        // A close pair, stars 0 and 1, with star 2 on a wide orbit about it: the root pair (node
        // 0) holds the close pair (node 1, whose stars are nodes 2 and 3) and star 2 (node 4). The
        // close pair is 0.1 au apart with μ for its 1.7 M☉, so its period is 8.9 days, and the
        // wide orbit's μ is for all 1.99 M☉.
        let close = OrbitDto {
            period_s: 769_529.898_097_118_5,
            semi_major_axis_m: 1.5e10,
            eccentricity: 0.0,
            inclination_rad: 0.25,
            ascending_node_rad: 0.0,
            argument_of_periapsis_rad: 0.0,
            mean_anomaly_at_epoch_rad: 2.0,
            mu_m3_s2: 2.25e20,
        };
        assert_wire_form(
            &HierarchyDto {
                nodes: vec![
                    HierarchyNodeDto::Pair {
                        inner: 1,
                        outer: 4,
                        orbit: wide_pair_orbit(),
                    },
                    HierarchyNodeDto::Pair {
                        inner: 2,
                        outer: 3,
                        orbit: close,
                    },
                    HierarchyNodeDto::Star {
                        body_index: 0,
                        mass_msun: 1.2,
                    },
                    HierarchyNodeDto::Star {
                        body_index: 1,
                        mass_msun: 0.5,
                    },
                    HierarchyNodeDto::Star {
                        body_index: 2,
                        mass_msun: 0.29,
                    },
                ],
            },
            json!({
                "nodes": [
                    { "type": "pair", "inner": 1, "outer": 4, "orbit": wide_pair_orbit_json() },
                    {
                        "type": "pair",
                        "inner": 2,
                        "outer": 3,
                        "orbit": {
                            "period_s": 769_529.898_097_118_5,
                            "semi_major_axis_m": 1.5e10,
                            "eccentricity": 0.0,
                            "inclination_rad": 0.25,
                            "ascending_node_rad": 0.0,
                            "argument_of_periapsis_rad": 0.0,
                            "mean_anomaly_at_epoch_rad": 2.0,
                            "mu_m3_s2": 2.25e20,
                        },
                    },
                    { "type": "star", "body_index": 0, "mass_msun": 1.2 },
                    { "type": "star", "body_index": 1, "mass_msun": 0.5 },
                    { "type": "star", "body_index": 2, "mass_msun": 0.29 },
                ],
            }),
        );
    }

    #[test]
    fn a_system_not_yet_formed_has_no_nodes() {
        assert_wire_form(&HierarchyDto { nodes: Vec::new() }, json!({ "nodes": [] }));
    }

    #[test]
    fn an_unknown_node_type_is_rejected() {
        let error =
            serde_json::from_value::<HierarchyNodeDto>(json!({ "type": "triple" })).unwrap_err();
        assert!(
            error.to_string().contains("unknown variant `triple`"),
            "unexpected error: {error}"
        );
    }
}
