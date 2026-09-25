//! Tests of the satellite assembly (P14.T22.a) on whole systems of the planetary sample.

use super::*;
use crate::planetary::index::BodySlot;
use crate::planetary::system::tests::{SEED, whole};

#[test]
fn two_calls_agree_bit_for_bit() {
    for (ctx, system) in whole().iter().take(120) {
        for planet in system.planets() {
            let belt = system.nearest_belt(planet.index());
            let a = generate_satellites(SEED, ctx, planet, belt);
            assert_eq!(a, generate_satellites(SEED, ctx, planet, belt));
        }
    }
}

#[test]
fn sub_indices_are_unique_decode_and_belong_to_their_parent() {
    let (mut moons, mut rings) = (0, 0);
    for (_, system) in whole() {
        for found in system.satellites() {
            let parent = found.parent_index();
            let mut indices: Vec<BodyIndex> = found
                .moons()
                .iter()
                .map(Satellite::index)
                .chain(found.rings().iter().map(Ring::index))
                .collect();
            for index in &indices {
                assert_eq!(
                    BodyIndex::decode(index.get()),
                    Ok((index.slot(), index.sub()))
                );
                assert_eq!(index.slot(), parent.slot());
            }
            for (n, moon) in (1_u8..).zip(found.moons()) {
                let expected = match parent.sub() {
                    BodySub::Primary => BodySub::Moon(n),
                    BodySub::Member(k) => BodySub::Member(k + MEMBER_MOON_OFFSET),
                    other => panic!("a satellite of {other:?}"),
                };
                assert_eq!(moon.index().sub(), expected);
                moons += 1;
            }
            for ring in found.rings() {
                assert!(matches!(ring.index().sub(), BodySub::Ring(0 | 1)));
                assert!(matches!(parent.slot(), BodySlot::Planet(_)));
                rings += 1;
            }
            let before = indices.len();
            indices.sort_unstable();
            indices.dedup();
            assert_eq!(indices.len(), before);
        }
    }
    assert!(moons > 100 && rings > 50, "{moons} moons, {rings} rings");
}

#[test]
fn a_planet_with_no_moons_has_an_empty_set_not_an_error() {
    let mut empty = 0;
    for (ctx, system) in whole().iter().take(200) {
        for planet in system.planets() {
            let found = generate_satellites(SEED, ctx, planet, system.nearest_belt(planet.index()));
            assert_eq!(found.parent_index(), planet.index());
            if found.is_empty() {
                assert!(found.moons().is_empty() && found.rings().is_empty());
                assert_eq!(system.children(planet.index()).count(), 0);
                empty += 1;
            }
        }
    }
    assert!(empty > 50, "{empty} planets with nothing");
}

#[test]
fn every_giant_has_a_dusty_ring_and_regular_moons_only_orbit_giants() {
    let mut giants = 0;
    for (_, system) in whole() {
        for found in system.satellites() {
            let Some(parent) = found.parent() else {
                continue;
            };
            let giant = matches!(
                parent.class(),
                crate::planetary::derive::PlanetClass::GasGiant
                    | crate::planetary::derive::PlanetClass::IceGiant
            );
            // A dusty ring needs the Roche limit for rock beyond 1.1 planetary radii: a giant
            // denser than about 225 kg m⁻³ (P14.T20).
            if giant
                && found.parent_index().sub() == BodySub::Primary
                && parent.density().value() > 230.0
            {
                giants += 1;
                assert!(!found.rings().is_empty(), "{:?}", found.parent_index());
            }
            for moon in found.moons() {
                if let SatelliteMoon::Regular(_) = moon.moon() {
                    assert!(giant);
                }
            }
        }
    }
    assert!(giants > 20, "{giants} giants");
}

#[test]
fn a_moon_in_its_parent_s_body_frame_keeps_its_shape_and_its_tilt_to_the_orbital_plane() {
    for (_, system) in whole().iter().take(100) {
        for found in system.satellites() {
            let Some(parent) = found.parent() else {
                continue;
            };
            let normal = parent.orbit().orientation().normal();
            for moon in found.moons() {
                let local = moon.local_orbit_at(parent, Years::new(1e9));
                let framed = moon.orbit_at(parent, Years::new(1e9));
                assert_eq!(local.semi_major_axis(), framed.semi_major_axis());
                assert_eq!(local.eccentricity(), framed.eccentricity());
                let n = framed.orientation().normal();
                let cos = n[0] * normal[0] + n[1] * normal[1] + n[2] * normal[2];
                let tilt = crate::math::acos(cos.clamp(-1.0, 1.0));
                // An arccosine near 1 keeps about √ε of its argument's precision.
                assert!((tilt - local.inclination().value()).abs() < 1e-7);
            }
        }
    }
}
