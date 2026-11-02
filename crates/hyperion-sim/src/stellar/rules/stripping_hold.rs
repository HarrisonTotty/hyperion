//! The tests of P11.T1.d's helium axis: a star with a helium excess reads its stripped share and
//! stage radii from [`HeliumStrippingTable`] at one choke point, and a star with none the
//! stripping table, bit for bit.
//!
//! The axis is fitted under the fitted helium table the tracks read
//! ([`StrippingNode::of_fitted_helium`]), as the stripping table at none is.

use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::GENERATOR_VERSION;
use crate::galaxy::displaced::binarity::{
    HeliumStrippingTable, STRIPPING_HELIUM_MAX, StrippingNode, StrippingTable,
    stripped_share_under, stripping_heliums, stripping_masses, stripping_metallicities,
    stripping_node_under,
};
use crate::stellar::Composition;
use crate::units::{Dex, HeliumExcess, SolarMasses};

/// A star of \[Fe/H\] `fe_h` with the helium excess `helium`.
fn composition(fe_h: f64, helium: f64) -> Composition {
    Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::new(helium))
}

/// Asserts that two nodes are the same bits.
fn assert_same_node(a: StrippingNode, b: StrippingNode) {
    let (a, b) = (a.to_row(), b.to_row());
    for (x, y) in [(a.0, b.0), (a.1, b.1), (a.2, b.2), (a.3, b.3)] {
        assert_same_bits(x, y);
    }
}

/// With no helium excess the axis is the stripping table, bit for bit, and every reader reads it.
#[test]
fn no_helium_reads_the_table() {
    let version = StellarRules::of_version(GENERATOR_VERSION);
    for (m, fe_h) in [
        (5.6, -2.3),
        (9.0, -1.0),
        (17.3, 0.1),
        (40.0, -1.0),
        (150.0, 0.0),
    ] {
        let (m, plain) = (SolarMasses::new(m), composition(fe_h, 0.0));
        let table = StrippingTable::generator().at(m, &plain).unwrap();
        assert_same_node(
            HeliumStrippingTable::generator().at(m, &plain).unwrap(),
            table,
        );
        assert_same_node(stripping_node_under(m, &plain, version), table);
        assert_same_bits(stripped_share_under(m, &plain, version), table.share());
    }
}

/// A helium-rich star reads the axis, which holds the exact nodes at its slices and is within the
/// fit's acceptance (a share within 0.07, the stripping table's own) of the exact path between
/// them; past its greatest helium excess the exact path answers.
#[test]
fn a_helium_rich_star_reads_the_axis() {
    let axis = HeliumStrippingTable::generator();
    let (masses, metallicities) = (stripping_masses(), stripping_metallicities());
    for (i, j, h) in [(4, 2, 0), (17, 6, 1), (33, 10, 2), (25, 0, 2)] {
        let m = SolarMasses::new(masses[i]);
        let comp = composition(metallicities[j], stripping_heliums()[h]);
        let (read, exact) = (
            axis.at(m, &comp).unwrap().to_row(),
            StrippingNode::of_fitted_helium(m, &comp).to_row(),
        );
        for (a, b) in [
            (read.0, exact.0),
            (read.1, exact.1),
            (read.2, exact.2),
            (read.3, exact.3),
        ] {
            assert!((a - b).abs() <= 1e-9 * b.abs().max(1.0), "{a} against {b}");
        }
    }
    for (m, fe_h, helium) in [(6.6, -0.65, 0.04), (9.0, -1.0, 0.09), (23.0, -1.6, 0.15)] {
        let (m, rich) = (SolarMasses::new(m), composition(fe_h, helium));
        let node = axis.at(m, &rich).unwrap();
        assert_same_node(
            stripping_node_under(m, &rich, StellarRules::of_version(GENERATOR_VERSION)),
            node,
        );
        assert_same_bits(
            stripped_share_under(m, &rich, StellarRules::of_version(GENERATOR_VERSION)),
            node.share(),
        );
        let exact = StrippingNode::of_fitted_helium(m, &rich);
        assert!(
            (node.share() - exact.share()).abs() < 0.07,
            "{node:?} against {exact:?}"
        );
    }
    let (m, past) = (
        SolarMasses::new(9.0),
        composition(-1.0, STRIPPING_HELIUM_MAX + 0.01),
    );
    assert!(axis.at(m, &past).is_none());
    assert_same_node(
        stripping_node_under(m, &past, StellarRules::of_version(GENERATOR_VERSION)),
        StrippingNode::of_fitted_helium(m, &past),
    );
}
