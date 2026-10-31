//! The tests of P11.T1.d's helium axis, the hold of [`STRIPPING_HELIUM_VERSION`]: from 26 a star
//! with a helium excess reads its stripped share and stage radii from
//! [`HeliumStrippingTable`] at one choke point, and below it the exact path, as it did.
//!
//! - **D3.** The version's rules keep the exact path, and a star with no helium excess reads the
//!   stripping table under either arm, bit for bit.
//! - **D2.** Under the axis's arm a helium-rich star's share and node are the axis's, within the
//!   fit's acceptance of the exact path under the fitted helium table.
//! - **D1** is the goldens as they stand: no version takes the axis's arm.
//!
//! The axis is fitted under the fitted helium table and the version's other rules
//! ([`StrippingNode::of_fitted_helium`]), as the stripping table at none is under the version's,
//! so the hard limit's arm (`hd_limit_hold`) moves neither; the bump refits both. It is read only
//! under the fitted helium table: the axis's arm with the identity's takes the exact path.

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

/// The version's rules with the axis's arm turned on, and the fitted helium table it was built
/// under and is read under.
const TABULATED: StellarRules = StellarRules {
    stripping: StrippingRules::HeliumTabulated,
    helium: HeliumRules::Fitted,
    ..StellarRules::of_version(GENERATOR_VERSION)
};

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

#[test]
fn the_stripping_helium_axis_is_held_only_until_the_26_bump() {
    assert!(
        GENERATOR_VERSION < STRIPPING_HELIUM_VERSION,
        "the 25 → 26 bump puts the stripping table's helium axis on the record: a star with a \
         helium excess reads its stripped share and stage radii from `HeliumStrippingTable`. \
         Delete `STRIPPING_HELIUM_VERSION`, `StrippingRules` with its `HeliumExact` arm, \
         `StellarRules::stripping`, the `match` in `binarity::tabulated_node` and this module, \
         and refit the stripping table and its helium axis under 26's rules, in the bump's own \
         commit"
    );
    for v in 1..=64 {
        let expected = if v >= 26 {
            StrippingRules::HeliumTabulated
        } else {
            StrippingRules::HeliumExact
        };
        assert_eq!(
            StellarRules::of_version(GeneratorVersion::new(v)).stripping,
            expected,
            "version {v}"
        );
    }
    assert_eq!(
        StellarRules::every_25_hold().stripping,
        StrippingRules::HeliumExact
    );
    assert_eq!(
        StellarRules::every_26_hold().stripping,
        StrippingRules::HeliumTabulated
    );
}

/// D3: with no helium excess the axis is the stripping table, bit for bit, and every reader reads
/// the table under either arm; with one the version's rules take the exact path.
#[test]
fn the_version_keeps_the_exact_path_and_no_helium_reads_the_table() {
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
        for rules in [version, TABULATED] {
            assert_same_node(stripping_node_under(m, &plain, rules), table);
            assert_same_bits(stripped_share_under(m, &plain, rules), table.share());
        }
    }
    let (m, rich) = (SolarMasses::new(9.0), composition(-1.0, 0.09));
    let exact = StrippingNode::of(m, &rich);
    assert_same_node(stripping_node_under(m, &rich, version), exact);
    assert_same_bits(stripped_share_under(m, &rich, version), exact.share());
}

/// D2: under the axis's arm a helium-rich star reads the axis, which holds the exact nodes at its
/// slices and is within the fit's acceptance (a share within 0.07, the stripping table's own) of
/// the exact path between them; past its greatest helium excess the exact path answers.
#[test]
fn a_helium_rich_star_reads_the_axis_under_its_arm() {
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
        assert_same_node(stripping_node_under(m, &rich, TABULATED), node);
        assert_same_bits(stripped_share_under(m, &rich, TABULATED), node.share());
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
        stripping_node_under(m, &past, TABULATED),
        StrippingNode::of_fitted_helium(m, &past),
    );
    let (m, rich) = (SolarMasses::new(9.0), composition(-1.0, 0.09));
    let identity = StellarRules {
        helium: HeliumRules::Identity,
        ..TABULATED
    };
    assert_same_node(
        stripping_node_under(m, &rich, identity),
        StrippingNode::of(m, &rich),
    );
}
