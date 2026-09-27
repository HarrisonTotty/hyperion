//! The registry of catalogue classes: the one place a class value is allocated (plan 09, P09.T1).
//!
//! A catalogue class is a kind of rare host that the brainstorm carves out of the cells and places
//! on its own coarse grid under the `111` prefix ("Events in time": "Rare events are found by
//! carving out their hosts, not the events"). Plan 01 builds the bit layout
//! ([`CatalogueSystemId`]: a 6-bit class, so 64 values) and leaves the values to this registry.
//!
//! Values 0–8 are taken: this plan's core collapses, Type Ia supernovae and luminous blue
//! variables, plan 11's five binary classes, and the tidal-disruption victims, which exist only on
//! the galactic centre's feature-level list and never under the `111` prefix. Values 9 upward are
//! free. A value is never renumbered or reused: it is in every saved ID of its class. The golden
//! file `catalogue_classes/registry.golden` pins the table, so a renumbering shows as a changed
//! line.
//!
//! # Cell sizes
//!
//! A class's catalogue cells are 512 ly × 2ᵏ ([`ClassId::cell_log2_ly`]); a coarser class zeroes
//! the low bits of each 512 ly coordinate, which `resolve` checks with
//! [`CatalogueSystemId::is_aligned_to`]. The core collapses take 512 ly (P09.T33.a), plan 11's
//! accreting white dwarfs and X-ray binaries 512 ly and its two merger classes 4,096 ly (P11.T8).
//! The plan gives no size for the Type Ia hosts, the slow accreting white dwarfs or the luminous
//! blue variables; they take 512 ly, 512 ly and 4,096 ly here, **provisionally**, until the tasks
//! that build them (P09.T34, P11.T8, P09.T43) confirm or change them. No entry of any class exists
//! yet, so such a change moves nothing but this registry's golden file.

use std::fmt;

#[cfg(doc)]
use crate::id::CatalogueSystemId;

/// A catalogue class: the 6-bit class field of a `111` ID, and a class of the feature-level lists.
///
/// Only the named constants exist; [`ClassId::from_value`] returns `None` for a free value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClassId(u8);

/// One row of the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ClassEntry {
    class: ClassId,
    name: &'static str,
    /// `log₂` of the catalogue cell's edge in light-years, or `None` for a class that is never
    /// placed under the `111` prefix.
    cell_log2_ly: Option<u32>,
    /// The plan that owns the class's process.
    owner: &'static str,
}

impl ClassId {
    /// Core-collapse supernovae and their shells (plan 09, P09.T33).
    pub const CORE_COLLAPSE: Self = Self(0);
    /// Type Ia supernovae and their shells (plan 09, P09.T34).
    pub const TYPE_IA: Self = Self(1);
    /// Stellar mergers, reserved for plan 11 (P11.T8).
    pub const STELLAR_MERGER: Self = Self(2);
    /// Neutron-star mergers, reserved for plan 11 (P11.T8).
    pub const NEUTRON_STAR_MERGER: Self = Self(3);
    /// Luminous blue variables (plan 09, P09.T43).
    pub const LUMINOUS_BLUE_VARIABLE: Self = Self(4);
    /// X-ray binaries, reserved for plan 11 (P11.T8).
    pub const XRAY_BINARY: Self = Self(5);
    /// Accreting white dwarfs, the fast hosts, reserved for plan 11 (P11.T8).
    pub const ACCRETING_WHITE_DWARF: Self = Self(6);
    /// Stars torn apart by the central black hole: only on the centre's feature-level list, never
    /// under the `111` prefix (plan 09, P09.T30).
    pub const TIDAL_DISRUPTION_VICTIM: Self = Self(7);
    /// Accreting white dwarfs, the slow hosts, reserved for plan 11 (P11.T8).
    pub const ACCRETING_WHITE_DWARF_SLOW: Self = Self(8);

    /// The exclusive bound of a class value: 64, the 6-bit field of plan 01's layout.
    pub const LIMIT: u8 = 1 << 6;

    /// The registered class with this value, or `None` for a free value or one of 64 or more.
    #[must_use]
    pub fn from_value(value: u8) -> Option<Self> {
        REGISTRY
            .iter()
            .find(|entry| entry.class.0 == value)
            .map(|entry| entry.class)
    }

    /// The class's value, 0–63: the class field of its `111` IDs.
    #[must_use]
    pub const fn value(self) -> u8 {
        self.0
    }

    /// The class's name, in `SCREAMING_SNAKE_CASE` as its constant is named.
    #[must_use]
    pub fn name(self) -> &'static str {
        self.entry().name
    }

    /// `log₂` of the class's catalogue cell edge in light-years, 9 (512 ly) or more, or `None` for
    /// [`TIDAL_DISRUPTION_VICTIM`](Self::TIDAL_DISRUPTION_VICTIM), which is never placed under the
    /// `111` prefix.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::galaxy::catalogue_classes::ClassId;
    ///
    /// assert_eq!(ClassId::CORE_COLLAPSE.cell_log2_ly(), Some(9)); // 512 ly
    /// assert_eq!(ClassId::STELLAR_MERGER.cell_log2_ly(), Some(12)); // 4,096 ly
    /// assert_eq!(ClassId::TIDAL_DISRUPTION_VICTIM.cell_log2_ly(), None);
    /// ```
    #[must_use]
    pub fn cell_log2_ly(self) -> Option<u32> {
        self.entry().cell_log2_ly
    }

    /// The plan that builds the class's process: `"09"` or `"11"`.
    #[must_use]
    pub fn owner(self) -> &'static str {
        self.entry().owner
    }

    fn entry(self) -> &'static ClassEntry {
        REGISTRY
            .iter()
            .find(|entry| entry.class == self)
            .expect("a ClassId is only ever one of the registry's constants")
    }
}

impl fmt::Display for ClassId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Every registered class, in value order. The one table a value is allocated in.
const REGISTRY: [ClassEntry; 9] = [
    ClassEntry {
        class: ClassId::CORE_COLLAPSE,
        name: "CORE_COLLAPSE",
        cell_log2_ly: Some(9),
        owner: "09",
    },
    ClassEntry {
        class: ClassId::TYPE_IA,
        name: "TYPE_IA",
        cell_log2_ly: Some(9),
        owner: "09",
    },
    ClassEntry {
        class: ClassId::STELLAR_MERGER,
        name: "STELLAR_MERGER",
        cell_log2_ly: Some(12),
        owner: "11",
    },
    ClassEntry {
        class: ClassId::NEUTRON_STAR_MERGER,
        name: "NEUTRON_STAR_MERGER",
        cell_log2_ly: Some(12),
        owner: "11",
    },
    ClassEntry {
        class: ClassId::LUMINOUS_BLUE_VARIABLE,
        name: "LUMINOUS_BLUE_VARIABLE",
        cell_log2_ly: Some(12),
        owner: "09",
    },
    ClassEntry {
        class: ClassId::XRAY_BINARY,
        name: "XRAY_BINARY",
        cell_log2_ly: Some(9),
        owner: "11",
    },
    ClassEntry {
        class: ClassId::ACCRETING_WHITE_DWARF,
        name: "ACCRETING_WHITE_DWARF",
        cell_log2_ly: Some(9),
        owner: "11",
    },
    ClassEntry {
        class: ClassId::TIDAL_DISRUPTION_VICTIM,
        name: "TIDAL_DISRUPTION_VICTIM",
        cell_log2_ly: None,
        owner: "09",
    },
    ClassEntry {
        class: ClassId::ACCRETING_WHITE_DWARF_SLOW,
        name: "ACCRETING_WHITE_DWARF_SLOW",
        cell_log2_ly: Some(9),
        owner: "11",
    },
];

/// Every registered class, in value order.
pub const ALL_CLASSES: [ClassId; 9] = [
    ClassId::CORE_COLLAPSE,
    ClassId::TYPE_IA,
    ClassId::STELLAR_MERGER,
    ClassId::NEUTRON_STAR_MERGER,
    ClassId::LUMINOUS_BLUE_VARIABLE,
    ClassId::XRAY_BINARY,
    ClassId::ACCRETING_WHITE_DWARF,
    ClassId::TIDAL_DISRUPTION_VICTIM,
    ClassId::ACCRETING_WHITE_DWARF_SLOW,
];

/// The order of the classes on a feature-level member list (plan 09, Design note 16).
///
/// Index 0 of a list is member zero; from 1 upward come the list's classes, each with its own
/// Poisson candidate count, in this order, which is not the numeric order of [`ClassId`]: the
/// tidal-disruption victims (on the centre's list only), the core collapses, the Type Ia hosts, the
/// luminous blue variables, then plan 11's five in the order of P11.T8's subtasks. A class
/// registered later is appended, so that no earlier class's members move. A golden file pins it.
pub const FEATURE_LEVEL_LIST_ORDER: [ClassId; 9] = [
    ClassId::TIDAL_DISRUPTION_VICTIM,
    ClassId::CORE_COLLAPSE,
    ClassId::TYPE_IA,
    ClassId::LUMINOUS_BLUE_VARIABLE,
    ClassId::ACCRETING_WHITE_DWARF,
    ClassId::ACCRETING_WHITE_DWARF_SLOW,
    ClassId::XRAY_BINARY,
    ClassId::STELLAR_MERGER,
    ClassId::NEUTRON_STAR_MERGER,
];

#[cfg(test)]
mod tests {
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;

    use super::*;
    use crate::GENERATOR_VERSION;
    use crate::id::CatalogueSystemId;

    #[test]
    fn no_two_names_share_a_value_and_every_value_is_below_the_limit() {
        for (i, a) in REGISTRY.iter().enumerate() {
            assert!(a.class.value() < ClassId::LIMIT, "{}", a.name);
            assert!(
                a.class.value() < CatalogueSystemId::CLASS_LIMIT,
                "{}",
                a.name
            );
            for b in &REGISTRY[i + 1..] {
                assert_ne!(a.class, b.class, "{} and {} share a value", a.name, b.name);
                assert_ne!(a.name, b.name);
            }
        }
    }

    #[test]
    fn the_registry_is_in_value_order_and_all_classes_lists_it() {
        for (value, (entry, class)) in REGISTRY.iter().zip(ALL_CLASSES).enumerate() {
            assert_eq!(usize::from(entry.class.value()), value);
            assert_eq!(entry.class, class);
            assert_eq!(ClassId::from_value(class.value()), Some(class));
        }
        for free in 9..=u8::MAX {
            assert_eq!(ClassId::from_value(free), None, "{free} is free");
        }
    }

    #[test]
    fn values_zero_to_eight_are_the_plan_values() {
        assert_eq!(ClassId::CORE_COLLAPSE.value(), 0);
        assert_eq!(ClassId::TYPE_IA.value(), 1);
        assert_eq!(ClassId::STELLAR_MERGER.value(), 2);
        assert_eq!(ClassId::NEUTRON_STAR_MERGER.value(), 3);
        assert_eq!(ClassId::LUMINOUS_BLUE_VARIABLE.value(), 4);
        assert_eq!(ClassId::XRAY_BINARY.value(), 5);
        assert_eq!(ClassId::ACCRETING_WHITE_DWARF.value(), 6);
        assert_eq!(ClassId::TIDAL_DISRUPTION_VICTIM.value(), 7);
        assert_eq!(ClassId::ACCRETING_WHITE_DWARF_SLOW.value(), 8);
    }

    #[test]
    fn cell_sizes_are_512_ly_times_a_power_of_two_within_the_root_cube() {
        for class in ALL_CLASSES {
            match class.cell_log2_ly() {
                Some(log2) => assert!((9..=16).contains(&log2), "{class}: {log2}"),
                None => assert_eq!(class, ClassId::TIDAL_DISRUPTION_VICTIM),
            }
        }
    }

    #[test]
    fn the_list_order_holds_every_class_once() {
        for class in ALL_CLASSES {
            let n = FEATURE_LEVEL_LIST_ORDER
                .iter()
                .filter(|&&c| c == class)
                .count();
            assert_eq!(n, 1, "{class}");
        }
    }

    /// The registry and the list order, pinned: a renumbered value, a moved cell size or a
    /// reordered list shows as a changed line.
    #[test]
    fn registry_golden() {
        let mut w = GoldenWriter::new();
        w.header(GENERATOR_VERSION.get());
        for class in ALL_CLASSES {
            let size = class
                .cell_log2_ly()
                .map_or_else(|| "none".to_owned(), |log2| format!("{} ly", 1_u64 << log2));
            w.line(&format!(
                "{} = {} (cell {size}, plan {})",
                class.name(),
                class.value(),
                class.owner()
            ));
        }
        for (position, class) in FEATURE_LEVEL_LIST_ORDER.iter().enumerate() {
            w.line(&format!("list[{}] = {}", position + 1, class.name()));
        }
        golden!("catalogue_classes/registry", w.as_str());
    }
}
