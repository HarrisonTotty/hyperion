//! The extents of a system's populations on the wire: a planet's rings, a belt and the cometary
//! halo (plan 14, P14.T35.b's `BeltDto` and the halo section, from P14.T20–T21).
//!
//! Each is a body of its system's list (design note 3), whose record carries what it is as its
//! `population` section, a [`PopulationDto`]; every other body's is `not_applicable`. Every
//! distance is in metres and every field name carries its unit.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::planetary::{OrbitHostDto, SectionDto};
use crate::primitives::BodyIdHex;

/// What a population body is, and where it lies: a ring, a belt or a cometary halo, tagged by
/// `type` (`{"type": "belt", "host": …, …}`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum PopulationDto {
    /// A ring of a giant planet (P14.T20).
    Ring(RingDto),
    /// A belt of small bodies about an orbit host (P14.T21.a–c).
    Belt(BeltDto),
    /// A system's cometary halo (P14.T21.d).
    CometaryHalo(CometaryHaloDto),
}

/// Which of a giant's rings a ring is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum RingKindDto {
    /// The tenuous dusty ring every giant has, of optical depth under 10⁻³.
    Dusty,
    /// A massive ring system like Saturn's.
    Massive,
}

/// What a ring is made of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum RingMaterialDto {
    /// Porous water ice, 600 kg m⁻³: a massive ring of a giant colder than 170 K.
    PorousIce,
    /// Rock, 2,500 kg m⁻³: every dusty ring, and a massive ring of a hotter giant.
    Rock,
}

/// A gap a moon's resonance clears in a massive ring.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RingGapDto {
    /// The moon whose resonance clears it.
    pub moon: BodyIdHex,
    /// The resonance, as the pair of integers (p + 1, p) of the moon's period to the gap's:
    /// `[2, 1]` or `[3, 2]`.
    pub resonance: [u8; 2],
    /// Its distance from the planet's centre, in metres.
    pub radius_m: f64,
}

/// A giant's ring: its kind, material, edges and optical depth (P14.T20).
///
/// Its edges are distances from its planet's centre in the planet's equatorial plane, which this
/// generator version takes to be the planet's orbital plane until P14.T14 gives planets their
/// poles. The ring's record's `position_m` is its planet's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RingDto {
    /// Which of the planet's rings it is.
    pub ring_kind: RingKindDto,
    /// What it is made of.
    pub material: RingMaterialDto,
    /// The inner edge, in metres from the planet's centre, above the planet's radius.
    pub inner_edge_m: f64,
    /// The outer edge, in metres from the planet's centre, inside the fluid Roche limit for the
    /// ring's material.
    pub outer_edge_m: f64,
    /// The normal optical depth, dimensionless, positive.
    pub optical_depth: f64,
    /// The gaps the planet's moons clear in it, inside out; none in a dusty ring.
    pub gaps: Vec<RingGapDto>,
}

/// Where a belt lies, by the rule that placed it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum BeltSiteDto {
    /// Between the 4:1 and 2:1 resonances of the innermost giant beyond the snow line.
    InsideGiant,
    /// In a gap of more than 40 mutual Hill radii between two planets of a host without giants.
    Gap,
    /// Beyond the outermost planet, from its 3:2 to its 2:1 resonance, and scattered beyond.
    BeyondPlanets,
    /// The outer third of the disc of a host without planets.
    OuterDisc,
}

/// Which side of the snow line most of a belt's solids lie on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum BeltCompositionDto {
    /// Mostly rock.
    Rocky,
    /// Rock and ice.
    Icy,
}

/// One annulus of a belt: the belt proper, or a Kuiper-like belt's scattered component.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BeltComponentDto {
    /// The inner edge, in metres from the belt's host.
    pub inner_edge_m: f64,
    /// The outer edge, in metres from the belt's host.
    pub outer_edge_m: f64,
}

/// A gap a planet's resonance clears in an asteroid belt (the Kirkwood gaps).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BeltGapDto {
    /// The resonance, as the pair of integers of the giant's period to the gap's: `[3, 1]`,
    /// `[5, 2]` or `[7, 3]`.
    pub resonance: [u8; 2],
    /// Its distance from the belt's host, in metres.
    pub radius_m: f64,
}

/// A belt about an orbit host at the record's time (P14.T21.a–c): its extent, its population's
/// statistics and its largest members.
///
/// Its edges lie in its host's planetary plane, the plane of the host's zone, and have widened
/// with the host's mass loss by the time of the record; its mass is the record's `mass_kg`, worn
/// down by collisions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BeltDto {
    /// The orbit host the belt goes round, a zone's host.
    pub host: OrbitHostDto,
    /// The rule that placed it.
    pub site: BeltSiteDto,
    /// The inner edge of the whole belt, in metres from its host.
    pub inner_edge_m: f64,
    /// The outer edge of the whole belt, its scattered component included, in metres from its
    /// host.
    pub outer_edge_m: f64,
    /// The belt proper.
    pub main: BeltComponentDto,
    /// A Kuiper-like belt's scattered component, which has no mass of its own; `null` for every
    /// other belt.
    pub scattered: Option<BeltComponentDto>,
    /// The gaps its giant's resonances clear, inside out; none but in an asteroid belt inside a
    /// giant.
    pub gaps: Vec<BeltGapDto>,
    /// The size slope q of its population, N(> D) ∝ D^(−q), 2.5–3.5.
    pub size_slope: f64,
    /// The diameter of its largest body, in metres.
    pub largest_diameter_m: f64,
    /// Its composition class.
    pub composition: BeltCompositionDto,
    /// The mean eccentricity of the belt proper's population.
    pub mean_eccentricity: f64,
    /// The mean inclination of the belt proper's population to its host's plane, in radians.
    pub mean_inclination_rad: f64,
    /// Its dust's fractional luminosity `L_dust` ÷ L★ at the record's time, which an infrared
    /// sensor sees; 0 when its host has not formed or shines no more.
    pub fractional_luminosity: f64,
    /// Its largest members, the dwarf planets over 400 km across, by ID in index order (`bulk`):
    /// `not_resolved` below the `bulk` level, where a population seen as a whole does not resolve
    /// them, and `ok` with an empty list for a belt with none.
    pub members: SectionDto<Vec<BodyIdHex>>,
}

/// A system's cometary halo at the record's time (P14.T21.d): a statistical population and
/// nothing else.
///
/// A spherical shell about its host, whose radii have widened, and whose comets have thinned,
/// with its host's mass loss by the time of the record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CometaryHaloDto {
    /// What the halo surrounds: the primary star, or a pair about which the primary's planets
    /// go, or the whole system.
    pub host: OrbitHostDto,
    /// The inner radius, in metres from its host.
    pub inner_edge_m: f64,
    /// The outer radius, in metres from its host.
    pub outer_edge_m: f64,
    /// The number of comets over 1 km across, a statistical figure.
    pub comets: f64,
    /// The rate of long-period comets reaching perihelion inside 5 au × √(L ÷ L☉) of the host,
    /// new and returning, in comets per second at the record's time; 0 when the host has not
    /// formed or shines no more.
    pub comet_rate_per_s: f64,
}

#[cfg(test)]
pub(crate) mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::testing::{assert_wire_form, assert_wire_strings};

    const SYSTEM: u64 = 0x0200_0800_2000_0000;

    /// Saturn's main rings as the generator draws them: porous ice from 1.1 planetary radii to
    /// 0.8 of the Roche limit, with the Cassini Division at Mimas's 2:1 resonance.
    pub(crate) fn saturn_ring() -> PopulationDto {
        PopulationDto::Ring(RingDto {
            ring_kind: RingKindDto::Massive,
            material: RingMaterialDto::PorousIce,
            inner_edge_m: 66_000_000.0,
            outer_edge_m: 136_800_000.0,
            optical_depth: 0.6,
            gaps: vec![RingGapDto {
                moon: BodyIdHex::from_parts(SYSTEM, 0x0601),
                resonance: [2, 1],
                radius_m: 117_000_000.0,
            }],
        })
    }

    pub(crate) fn saturn_ring_json() -> Value {
        json!({
            "type": "ring",
            "ring_kind": "massive",
            "material": "porous_ice",
            "inner_edge_m": 66_000_000.0,
            "outer_edge_m": 136_800_000.0,
            "optical_depth": 0.6,
            "gaps": [{
                "moon": "0200080020000000.0601",
                "resonance": [2, 1],
                "radius_m": 117_000_000.0,
            }],
        })
    }

    /// The Solar System's main belt about star 0, at its resonances with Jupiter.
    pub(crate) fn main_belt() -> PopulationDto {
        PopulationDto::Belt(BeltDto {
            host: OrbitHostDto::Star { body_index: 0 },
            site: BeltSiteDto::InsideGiant,
            inner_edge_m: 3.089e11,
            outer_edge_m: 4.905e11,
            main: BeltComponentDto {
                inner_edge_m: 3.089e11,
                outer_edge_m: 4.905e11,
            },
            scattered: None,
            gaps: vec![BeltGapDto {
                resonance: [3, 1],
                radius_m: 3.743e11,
            }],
            size_slope: 3.0,
            largest_diameter_m: 9.4e5,
            composition: BeltCompositionDto::Rocky,
            mean_eccentricity: 0.125,
            mean_inclination_rad: 0.175,
            fractional_luminosity: 1e-9,
            members: SectionDto::Ok(vec![BodyIdHex::from_parts(SYSTEM, 0xe101)]),
        })
    }

    pub(crate) fn main_belt_json() -> Value {
        json!({
            "type": "belt",
            "host": { "type": "star", "body_index": 0 },
            "site": "inside_giant",
            "inner_edge_m": 3.089e11,
            "outer_edge_m": 4.905e11,
            "main": { "inner_edge_m": 3.089e11, "outer_edge_m": 4.905e11 },
            "scattered": null,
            "gaps": [{ "resonance": [3, 1], "radius_m": 3.743e11 }],
            "size_slope": 3.0,
            "largest_diameter_m": 9.4e5,
            "composition": "rocky",
            "mean_eccentricity": 0.125,
            "mean_inclination_rad": 0.175,
            "fractional_luminosity": 1e-9,
            "members": { "state": "ok", "value": ["0200080020000000.e101"] },
        })
    }

    #[test]
    fn ring_wire_form() {
        assert_wire_form(&saturn_ring(), saturn_ring_json());
    }

    #[test]
    fn belt_wire_form() {
        assert_wire_form(&main_belt(), main_belt_json());
    }

    #[test]
    fn a_kuiper_belt_carries_its_scattered_component_and_withholds_its_members_below_bulk() {
        let PopulationDto::Belt(main) = main_belt() else {
            unreachable!("the fixture is a belt")
        };
        let kuiper = PopulationDto::Belt(BeltDto {
            site: BeltSiteDto::BeyondPlanets,
            scattered: Some(BeltComponentDto {
                inner_edge_m: 7.1e12,
                outer_edge_m: 1.5e13,
            }),
            gaps: Vec::new(),
            composition: BeltCompositionDto::Icy,
            members: SectionDto::NotResolved,
            ..main
        });
        let wire = serde_json::to_value(&kuiper).unwrap();
        assert_eq!(
            wire["scattered"],
            json!({ "inner_edge_m": 7.1e12, "outer_edge_m": 1.5e13 })
        );
        assert_eq!(wire["members"], json!({ "state": "not_resolved" }));
        assert_eq!(
            serde_json::from_value::<PopulationDto>(wire).unwrap(),
            kuiper
        );
    }

    #[test]
    fn cometary_halo_wire_form() {
        assert_wire_form(
            &PopulationDto::CometaryHalo(CometaryHaloDto {
                host: OrbitHostDto::Star { body_index: 0 },
                inner_edge_m: 2.99e14,
                outer_edge_m: 1.496e16,
                comets: 7.5e11,
                comet_rate_per_s: 3.45e-7,
            }),
            json!({
                "type": "cometary_halo",
                "host": { "type": "star", "body_index": 0 },
                "inner_edge_m": 2.99e14,
                "outer_edge_m": 1.496e16,
                "comets": 7.5e11,
                "comet_rate_per_s": 3.45e-7,
            }),
        );
    }

    #[test]
    fn population_strings() {
        assert_wire_strings(&[
            (RingKindDto::Dusty, "dusty"),
            (RingKindDto::Massive, "massive"),
        ]);
        assert_wire_strings(&[
            (RingMaterialDto::PorousIce, "porous_ice"),
            (RingMaterialDto::Rock, "rock"),
        ]);
        assert_wire_strings(&[
            (BeltSiteDto::InsideGiant, "inside_giant"),
            (BeltSiteDto::Gap, "gap"),
            (BeltSiteDto::BeyondPlanets, "beyond_planets"),
            (BeltSiteDto::OuterDisc, "outer_disc"),
        ]);
        assert_wire_strings(&[
            (BeltCompositionDto::Rocky, "rocky"),
            (BeltCompositionDto::Icy, "icy"),
        ]);
    }
}
