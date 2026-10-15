//! Tests of the satellite assembly (P14.T22.a) on whole systems of the planetary sample.

use super::*;
use crate::planetary::index::BodySlot;
use crate::planetary::system::tests::{SEED, whole};
use crate::units::consts::RADIANS_PER_DEGREE;

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

/// Ruling 112.5: a moon's radius rank is keyed by its candidate (its kind and number within the
/// kind), not by the dense sub-index, so that a moon pruned before it moves nothing of it; and no
/// two moons of a parent share a key.
#[test]
fn a_moon_draws_by_its_candidate_not_its_sub_index() {
    let (mut moons, mut shifted) = (0, 0);
    for (_, system) in whole() {
        for found in system.satellites() {
            let parent = found.parent_index();
            let mut keys = Vec::new();
            for moon in found.moons() {
                let key = candidate_key(parent, moon.moon()).unwrap_or(moon.index());
                let expected =
                    crate::planetary::system::radius_rank(SEED, key.body_id(system.system()));
                assert_eq!(moon.radius_rank(), expected, "{:?}", moon.index());
                shifted += usize::from(key != moon.index());
                keys.push(key);
                moons += 1;
            }
            let before = keys.len();
            keys.sort_unstable();
            keys.dedup();
            assert_eq!(keys.len(), before, "keys shared about {parent:?}");
        }
    }
    assert!(
        moons > 100 && shifted > 20,
        "{moons} moons, {shifted} keyed apart from their index"
    );
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

// P14.T56.a: the Laplace surface.

/// A unit vector of inclination `inclination` and node `node` from galactic north, rad.
fn axis(inclination: f64, node: f64) -> [f64; 3] {
    let (sin_i, cos_i) = crate::math::sin_cos(inclination);
    let (sin_n, cos_n) = crate::math::sin_cos(node);
    [sin_i * sin_n, -sin_i * cos_n, cos_i]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The angle between two unit vectors, rad.
fn angle(a: [f64; 3], b: [f64; 3]) -> f64 {
    let c = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    crate::math::atan2(dot(c, c).sqrt(), dot(a, b))
}

/// A frame about `parent` whose spin pole is `spin`, whose J₂ is `j2` at its radius, read at
/// `read_at`.
fn frame_of(parent: &MoonParent, spin: [f64; 3], j2: f64, read_at: Years) -> LaplaceFrame {
    LaplaceFrame::new(&laplace::LaplaceFrameParts {
        spin_pole: spin,
        orbit_normal: parent.orbit().orientation().normal(),
        j2,
        equatorial_radius: parent.radius(),
        parent_orbit: *parent.orbit(),
        parent_mass: Kilograms::from(parent.mass()),
        host_mass: parent.host_mass(),
        read_at,
    })
    .unwrap()
}

/// A Jupiter in slot 5 of the tests' system `index`, with its index.
fn jupiter_at(index: u32) -> (BodyIndex, MoonParent) {
    use crate::planetary::moons::testing::{jupiter, planet_id};
    let at = BodyIndex::new(BodySlot::Planet(5), BodySub::Primary).unwrap();
    (at, jupiter(planet_id(index, 5)))
}

/// Its rings' parent: a giant of Jupiter's mass and radius at 125 K, in 1 ÷ 27 of Earth's flux.
fn jupiter_rings(at: BodyIndex, parent: &MoonParent) -> RingParent {
    RingParent::new(
        at,
        crate::planetary::derive::PlanetClass::GasGiant,
        Kilograms::from(parent.mass()),
        parent.radius(),
        crate::units::Kelvin::new(125.0),
        crate::units::EarthFluxes::new(1.0 / 27.0),
    )
    .unwrap()
}

#[test]
fn a_parent_of_zero_obliquity_gives_today_s_orientation() {
    let mut moons = 0;
    for (_, system) in whole().iter().take(100) {
        for found in system.satellites() {
            let Some(parent) = found.parent() else {
                continue;
            };
            let normal = parent.orbit().orientation().normal();
            let frame = frame_of(parent, normal, 0.01, Years::new(1e9));
            for moon in found.moons() {
                let local = moon.local_orbit_at(parent, Years::new(1e9));
                let today = in_body_frame(parent, &MoonPlane::OrbitalPlane, &local);
                for sense in [PoleSense::Spin, PoleSense::Orbit] {
                    let rl = frame.laplace_radius([]);
                    let plane = MoonPlane::Laplace(frame.local(rl, sense));
                    let laid = in_body_frame(parent, &plane, &local);
                    let (a, b) = (today.orientation(), laid.orientation());
                    assert!(angle(a.normal(), b.normal()) < 1e-12, "{:?}", moon.index());
                    let (p, q) = (a.periapsis_direction(), b.periapsis_direction());
                    assert!(angle(p, q) < 1e-12, "{:?}", moon.index());
                    assert_eq!(today.semi_major_axis(), laid.semi_major_axis());
                    assert_eq!(today.eccentricity(), laid.eccentricity());
                }
                moons += 1;
            }
        }
    }
    assert!(moons > 50, "{moons} moons");
}

#[test]
fn a_parent_on_its_side_keeps_its_regular_moons_prograde_about_its_spin() {
    let (mut regular, mut kept_captures) = (0, 0);
    for index in 0..40 {
        let (at, parent) = jupiter_at(index);
        let spin = axis(98.0 * RADIANS_PER_DEGREE, 0.4);
        let frame = frame_of(&parent, spin, 0.0147, Years::new(4.5e9));
        let rings = jupiter_rings(at, &parent);
        let found = satellites_of(
            SEED,
            at,
            &parent,
            Some(&rings),
            None,
            Years::new(4.5e9),
            SatelliteFrame::Laplace(&frame),
        );
        let rl = frame.laplace_radius([]);
        for moon in found.moons() {
            let SatelliteMoon::Regular(local) = moon.moon() else {
                continue;
            };
            let MoonPlane::Laplace(plane) = moon.plane() else {
                panic!("a moon on a frame has a Laplace plane");
            };
            assert_eq!(plane.sense(), PoleSense::Spin);
            let normal = moon
                .orbit_at(&parent, Years::new(4.5e9))
                .orientation()
                .normal();
            assert!(dot(normal, spin) > 0.0, "{:?} is retrograde", moon.index());
            // Its tilt to its local plane is its drawn inclination.
            let tilt = angle(normal, plane.pole_at(local.orbit().semi_major_axis()));
            assert!((tilt - local.orbit().inclination().value()).abs() < 1e-7);
            // Well inside the Laplace radius the local plane is the equator.
            if local.orbit().semi_major_axis().value() < 0.3 * rl.value() {
                assert!(angle(normal, spin) < 3.0 * RADIANS_PER_DEGREE);
            }
            regular += 1;
        }
        for ring in found.rings() {
            assert!(dot(ring.normal(), spin) > 0.999_999, "{:?}", ring.index());
        }
        // A capture's sense is drawn about the orbit, so its pole is on the orbit normal's side,
        // which about this parent is not the spin's.
        let normal = parent.orbit().orientation().normal();
        for moon in found.moons() {
            let (SatelliteMoon::Captured(captured), MoonPlane::Laplace(plane)) =
                (moon.moon(), moon.plane())
            else {
                continue;
            };
            assert_eq!(plane.sense(), PoleSense::Orbit);
            let pole = plane.pole_at(captured.orbit().semi_major_axis());
            assert!(dot(pole, normal) > 0.0, "{:?}", moon.index());
            kept_captures += 1;
        }
    }
    assert!(
        regular > 40 && kept_captures > 40,
        "{regular} regular moons, {kept_captures} captures"
    );
}

#[test]
fn a_regular_moon_in_the_band_is_dropped_with_every_regular_moon_beyond_it() {
    use crate::planetary::moons::regular_moons;
    let mut cut = 0;
    for index in 0..60 {
        let (at, parent) = jupiter_at(index);
        let moons = regular_moons(SEED, &parent);
        let [first, second, ..] = moons.moons() else {
            continue;
        };
        // A J₂ that puts the second moon at its own Laplace radius, the first counted: r_L⁵ =
        // (J₂R² + ½ (m₁ ÷ M_p) a₁²) T, with T read off a unit quadrupole.
        let tide = laplace::laplace_radius(
            1.0,
            parent.orbit(),
            Kilograms::from(parent.mass()),
            parent.host_mass(),
        )
        .value();
        let tide = crate::math::powi(tide, 5);
        let (a1, a2) = (
            first.orbit().semi_major_axis().value(),
            second.orbit().semi_major_axis().value(),
        );
        let inner = 0.5 * (first.mass().value() / parent.mass().value()) * a1 * a1;
        let j2 = (crate::math::powi(a2, 5) / tide - inner)
            / (parent.radius().value() * parent.radius().value());
        // The first moon, with nothing inside it, must lie inside the band's inner edge, 0.922
        // of its own Laplace radius on a parent on its side.
        let first_rl = crate::math::powf(
            j2 * parent.radius().value() * parent.radius().value() * tide,
            0.2,
        );
        if j2 <= 0.0 || a1 > 0.9 * first_rl {
            continue;
        }
        let spin = axis(90.0 * RADIANS_PER_DEGREE, 1.0);
        let frame = frame_of(&parent, spin, j2, Years::new(4.5e9));
        let rings = jupiter_rings(at, &parent);
        let alone = satellites_of(
            SEED,
            at,
            &parent,
            Some(&rings),
            None,
            Years::new(4.5e9),
            SatelliteFrame::OrbitalPlane,
        );
        let found = satellites_of(
            SEED,
            at,
            &parent,
            Some(&rings),
            None,
            Years::new(4.5e9),
            SatelliteFrame::Laplace(&frame),
        );
        let kept: Vec<u8> = found
            .moons()
            .iter()
            .filter_map(|moon| match moon.moon() {
                SatelliteMoon::Regular(regular) => Some(regular.ordinal()),
                SatelliteMoon::GiantImpact(_) | SatelliteMoon::Captured(_) => None,
            })
            .collect();
        assert_eq!(kept, vec![first.ordinal()], "{index}");
        // Its rings, far inside, are made as before, on the equator.
        assert_eq!(found.rings().len(), alone.rings().len());
        for (laid, flat) in found.rings().iter().zip(alone.rings()) {
            assert_eq!(laid.outer_edge(), flat.outer_edge());
            assert!(angle(laid.normal(), spin) < 1e-6);
            let normal = parent.orbit().orientation().normal();
            for (x, y) in flat.normal().into_iter().zip(normal) {
                hyperion_testkit::float::assert_same_bits(x, y);
            }
        }
        cut += 1;
    }
    assert!(cut > 10, "{cut} parents cut");
}
