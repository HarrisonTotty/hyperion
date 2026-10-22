//! P14.T24.h.a's floor over the passes (decision-p14-t54a-passes §G3.5): the Solar System's table
//! bodies under the floor's rules against the area mix's alone.

use hyperion_testkit::float::assert_same_bits;

use crate::planetary::derive::photometry::area_mix::{RayleighFloorRules, with_rayleigh_floor};
use crate::planetary::derive::solar::historic_table;
use crate::units::Pascals;

/// P14.T24.h.a's third test: Earth keeps its Bond albedo to the bit, since the floor does not bind
/// there (the table's Earth, 0.2798 at 23 under its 0.77 bar; the reference Earth's 0.294, which
/// calibrates the water cloud, reads no floor: `the_reference_earth_takes_0_294_by_its_cloud_s_
/// calibration`); the Moon, Mars, Venus and Titan move by under 0.005, stated, as does every other
/// body of the table; and a body whose Bond albedo the floor leaves keeps its equilibrium
/// temperature to the bit.
#[test]
fn earth_keeps_its_bond_to_the_bit_and_the_other_measured_bodies_move_by_under_0_005() {
    let held = historic_table();
    let floored = with_rayleigh_floor(RayleighFloorRules::Floored, historic_table);
    assert_eq!(held.len(), floored.len());
    for ((name, before), (again, after)) in held.iter().zip(&floored) {
        assert_eq!(name, again);
        let (a, b) = (before.albedo().value(), after.albedo().value());
        eprintln!(
            "{name}: Bond {a:.6} → {b:.6} ({:+.2e}), T_s {:.3} → {:.3} K, p_s {:.4e} Pa",
            b - a,
            before.atmosphere().surface_temperature().value(),
            after.atmosphere().surface_temperature().value(),
            after
                .atmosphere()
                .surface_pressure()
                .map_or(0.0, Pascals::value),
        );
        assert!((b - a).abs() < 0.005, "{name} moves {a} → {b}");
        if a.total_cmp(&b).is_eq() {
            assert_same_bits(
                before.equilibrium_temperature().value(),
                after.equilibrium_temperature().value(),
            );
        }
        if *name == "Earth" {
            assert_same_bits(a, b);
        }
    }
    for name in ["Moon", "Mars", "Venus", "Titan"] {
        assert!(
            held.iter().any(|(n, _)| *n == name),
            "{name} is a table body"
        );
    }
}
