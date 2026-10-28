//! Catalogue classes: rare hosts carved out of the cells and placed on grids of their own under
//! the `111` prefix (brainstorm, "Events in time"; plan 09, phase 7).
//!
//! This module holds, so far, what phase 1 builds (P09.T1): the class registry ([`registry`]) and
//! the key of a catalogue-class cell ([`CatalogueCellKey`], in [`ids`]); what phase 4 builds
//! (P09.T17–T19): the supernova test with a clock and a supernova entry's state and light curve
//! ([`supernova`]), and the Type Ia's delay-time distribution, progenitors and leftovers
//! ([`type_ia`]); and the class grid of phase 7 (P09.T32, [`grid`]): the [`ClassProcess`] trait
//! every class implements, a class's cell ([`CatalogueClassCell`]), the resolve of a `111` ID
//! ([`resolve`]) and the cells of a class about a sphere ([`cells_in_sphere`]). The registry's,
//! the IDs' and the grid's items are re-exported here. No class's process is built yet, so the
//! production lookup ([`ProductionClasses`]) has none; the processes and the carve-out follow in
//! P09.T33–T35, P09.T43 and plan 11.

pub mod grid;
pub mod ids;
pub mod registry;
pub mod supernova;
pub mod type_ia;

pub use self::grid::{
    CatalogueClassCell, CatalogueEntry, CatalogueMember, CheckClassHeadroomError, ClassLookup,
    ClassProcess, ClassVisitor, MemberVisitor, ProductionClasses, cells_in_sphere,
    check_class_headroom, resolve, resolve_entry,
};
pub use self::ids::{BuildCatalogueCellKeyError, CatalogueCellKey};
pub use self::registry::{ALL_CLASSES, ClassId, FEATURE_LEVEL_LIST_ORDER};
