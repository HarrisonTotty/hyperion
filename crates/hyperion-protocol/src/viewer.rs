//! The planet viewer's kinds (rendering plan R15): `viewer_planet`, one planet from the
//! generator's own inputs; `viewer_randomize`, a real planet's inputs drawn from the galaxy; and
//! `viewer_inputs`, a served planet's inputs read back.
//!
//! A planet the server generates is a system of its own, under a pinned ID of the viewer's half
//! (bit 57 of the pinned number set): `system_bodies`, `body_detail`, `surface_field`,
//! `survey_pass` and `sky`'s host discs answer it as they answer a galaxy system, for as long as
//! the server keeps it (its last 16 generations a universe), and `unknown_system` after, when the
//! client generates it again from its inputs. Additive under plan 04's convention:
//! `PROTOCOL_VERSION` stays 3.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::primitives::{BodyIdHex, GalacticPosition, SeedHex, SystemIdHex, UniverseIdHex};

/// A planet's inputs (`viewer_planet`): what the generator itself consumes of a planet and its
/// single host star, each in the unit its name states; an input that is `null` is drawn.
///
/// The server refuses an input outside its range with `bad_request` naming it: the host's mass
/// 0.08–150, \[Fe/H\] −3 to +1, the age 0.001–13.787, the mass 10⁻⁴–4,000, the semi-major axis
/// and the formation distance 10⁻³–10⁵, the eccentricity 0–0.9, the day 0.1–10⁶, the obliquity
/// 0–180 and each rank 0.001–0.999; and the semi-major axis of an orbit that leaves the system's
/// reach or enters its star, and the host's mass of a star with no disc.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PlanetInputsDto {
    /// The seed of every stream, in place of the universe's.
    pub seed: SeedHex,
    /// The host's initial mass, M☉.
    pub star_mass_msun: f64,
    /// The host's \[Fe/H\], dex.
    pub fe_h_dex: f64,
    /// The system's age at the epoch, Gyr.
    pub age_gyr: f64,
    /// The planet's primordial mass, M⊕.
    pub mass_mearth: f64,
    /// Its primordial semi-major axis, au.
    pub semi_major_axis_au: f64,
    /// Its primordial eccentricity.
    pub eccentricity: f64,
    /// How far from its host it formed, au; `null` for where placement would put it.
    pub formed_at_au: Option<f64>,
    /// Its primordial day, hours; `null` for its draw.
    pub day_hours: Option<f64>,
    /// Its primordial obliquity, degrees; `null` for its draw.
    pub tilt_deg: Option<f64>,
    /// Its water rank, 0–1; `null` for its draw.
    pub water_rank: Option<f64>,
    /// Its carbon rank, 0–1; `null` for its draw.
    pub carbon_rank: Option<f64>,
    /// Its nitrogen rank, 0–1; `null` for its draw.
    pub nitrogen_rank: Option<f64>,
}

/// What the generator drew for the inputs a `viewer_planet` left to it, and where the planet
/// formed: each its draw, or the input where one was given.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ViewerDrawnDto {
    /// Where it formed, au.
    pub formed_at_au: f64,
    /// Its primordial day, hours, floored at its break-up period; `null` where it has no rotation
    /// law.
    pub day_hours: Option<f64>,
    /// Its primordial obliquity, degrees; `null` where it has no rotation law.
    pub tilt_deg: Option<f64>,
    /// Its water rank.
    pub water_rank: f64,
    /// Its carbon rank.
    pub carbon_rank: f64,
    /// Its nitrogen rank.
    pub nitrogen_rank: f64,
}

/// Asks the server to generate one planet from its inputs (`viewer_planet`), answered with a
/// [`ViewerPlanetDto`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ViewerPlanetRequest {
    /// The universe whose galaxy gives the sky and the generator's rules.
    pub universe: UniverseIdHex,
    /// The planet's inputs.
    pub inputs: PlanetInputsDto,
}

/// The planet generated (`viewer_planet`): its system and body, which the system's kinds then
/// answer, its inputs as the server read them, and what was drawn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ViewerPlanetDto {
    /// The universe, echoed.
    pub universe: UniverseIdHex,
    /// The planet's system, a viewer system.
    pub system: SystemIdHex,
    /// The planet.
    pub body: BodyIdHex,
    /// Where the system stands at every time, its barycentre: the Sun-like point, 26,000 ly along
    /// +y.
    pub barycentre: GalacticPosition,
    /// The inputs as the server read them, a negative zero made positive.
    pub inputs: PlanetInputsDto,
    /// What the generator drew.
    pub drawn: ViewerDrawnDto,
}

/// Asks the server to draw a real planet's inputs from the galaxy (`viewer_randomize`), answered
/// with a [`ViewerDrawDto`]: the same `draw_seed` draws the same planet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ViewerRandomizeRequest {
    /// The universe whose galaxy the planet is drawn from.
    pub universe: UniverseIdHex,
    /// The draw's own seed.
    pub draw_seed: SeedHex,
}

/// A real planet's inputs as drawn (`viewer_randomize`): its host, orbit, mass and formation
/// distance, with its day, obliquity and ranks `null` (drawn), and a seed of the draw's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ViewerDrawDto {
    /// The universe, echoed.
    pub universe: UniverseIdHex,
    /// The draw's seed, echoed.
    pub draw_seed: SeedHex,
    /// The inputs.
    pub inputs: PlanetInputsDto,
    /// The real planet they were read from.
    pub source: BodyIdHex,
}

/// Asks the server for a served planet's inputs (`viewer_inputs`), answered with a
/// [`ViewerReadDto`]: `unknown_system` or `unknown_body` naming `body` as `body_detail` refuses
/// them, and `bad_request` naming `body` for a body that is not a planet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ViewerInputsRequest {
    /// The universe the planet is in.
    pub universe: UniverseIdHex,
    /// The planet.
    pub body: BodyIdHex,
}

/// A served planet's inputs, read back (`viewer_inputs`): its host's initial mass (its zone's, a
/// pair's total for a circumbinary planet), \[Fe/H\] and age, its primordial mass, orbit and
/// formation distance, its day and obliquity where it has a rotation law, its ranks, and its
/// universe's seed. They may lie outside the inputs' ranges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ViewerReadDto {
    /// The universe, echoed.
    pub universe: UniverseIdHex,
    /// The planet, echoed.
    pub body: BodyIdHex,
    /// Its inputs.
    pub inputs: PlanetInputsDto,
    /// Whether its system has more than one star, which the viewer's single host does not hold.
    pub multiple_stars: bool,
}

#[cfg(test)]
pub(crate) mod tests {
    use serde_json::json;

    use super::*;
    use crate::envelope::{RequestBody, ResponseBody};
    use crate::testing::assert_wire_form;

    /// An Earth about a Sun, its day and tilt given and its formation and ranks drawn.
    pub(crate) fn earth() -> PlanetInputsDto {
        PlanetInputsDto {
            seed: SeedHex::from_u64(7),
            star_mass_msun: 1.0,
            fe_h_dex: 0.0,
            age_gyr: 4.57,
            mass_mearth: 1.0,
            semi_major_axis_au: 1.0,
            eccentricity: 0.0167,
            formed_at_au: None,
            day_hours: Some(24.0),
            tilt_deg: Some(23.4),
            water_rank: None,
            carbon_rank: None,
            nitrogen_rank: None,
        }
    }

    /// The Earth's planet as generated.
    pub(crate) fn planet() -> ViewerPlanetDto {
        ViewerPlanetDto {
            universe: UniverseIdHex::from_u64(42),
            system: SystemIdHex::from_u64(0xfa41_3221_9330_884e),
            body: BodyIdHex::from_parts(0xfa41_3221_9330_884e, 0x0100),
            barycentre: GalacticPosition {
                cell_ly: [0, 26_000, 0],
                offset_m: [0.0, 0.0, 0.0],
            },
            inputs: earth(),
            drawn: ViewerDrawnDto {
                formed_at_au: 1.0,
                day_hours: Some(24.0),
                tilt_deg: Some(23.4),
                water_rank: 0.25,
                carbon_rank: 0.5,
                nitrogen_rank: 0.75,
            },
        }
    }

    fn earth_json() -> serde_json::Value {
        json!({
            "seed": "0000000000000007",
            "star_mass_msun": 1.0,
            "fe_h_dex": 0.0,
            "age_gyr": 4.57,
            "mass_mearth": 1.0,
            "semi_major_axis_au": 1.0,
            "eccentricity": 0.0167,
            "formed_at_au": null,
            "day_hours": 24.0,
            "tilt_deg": 23.4,
            "water_rank": null,
            "carbon_rank": null,
            "nitrogen_rank": null,
        })
    }

    #[test]
    fn viewer_planet_wire_forms() {
        assert_wire_form(
            &RequestBody::ViewerPlanet(ViewerPlanetRequest {
                universe: UniverseIdHex::from_u64(42),
                inputs: earth(),
            }),
            json!({ "kind": "viewer_planet", "universe": "000000000000002a", "inputs": earth_json() }),
        );
        assert_wire_form(
            &ResponseBody::ViewerPlanet(Box::new(planet())),
            json!({
                "kind": "viewer_planet",
                "universe": "000000000000002a",
                "system": "fa4132219330884e",
                "body": "fa4132219330884e.0100",
                "barycentre": { "cell_ly": [0, 26_000, 0], "offset_m": [0.0, 0.0, 0.0] },
                "inputs": earth_json(),
                "drawn": {
                    "formed_at_au": 1.0,
                    "day_hours": 24.0,
                    "tilt_deg": 23.4,
                    "water_rank": 0.25,
                    "carbon_rank": 0.5,
                    "nitrogen_rank": 0.75,
                },
            }),
        );
    }

    #[test]
    fn viewer_randomize_and_viewer_inputs_wire_forms() {
        let universe = UniverseIdHex::from_u64(42);
        let body = BodyIdHex::from_parts(0x0200_0800_2000_0000, 0x0100);
        assert_wire_form(
            &RequestBody::ViewerRandomize(ViewerRandomizeRequest {
                universe: universe.clone(),
                draw_seed: SeedHex::from_u64(9),
            }),
            json!({
                "kind": "viewer_randomize",
                "universe": "000000000000002a",
                "draw_seed": "0000000000000009",
            }),
        );
        assert_wire_form(
            &ResponseBody::ViewerRandomize(Box::new(ViewerDrawDto {
                universe: universe.clone(),
                draw_seed: SeedHex::from_u64(9),
                inputs: earth(),
                source: body.clone(),
            })),
            json!({
                "kind": "viewer_randomize",
                "universe": "000000000000002a",
                "draw_seed": "0000000000000009",
                "inputs": earth_json(),
                "source": "0200080020000000.0100",
            }),
        );
        assert_wire_form(
            &RequestBody::ViewerInputs(ViewerInputsRequest {
                universe: universe.clone(),
                body: body.clone(),
            }),
            json!({
                "kind": "viewer_inputs",
                "universe": "000000000000002a",
                "body": "0200080020000000.0100",
            }),
        );
        assert_wire_form(
            &ResponseBody::ViewerInputs(Box::new(ViewerReadDto {
                universe,
                body,
                inputs: earth(),
                multiple_stars: true,
            })),
            json!({
                "kind": "viewer_inputs",
                "universe": "000000000000002a",
                "body": "0200080020000000.0100",
                "inputs": earth_json(),
                "multiple_stars": true,
            }),
        );
    }
}
