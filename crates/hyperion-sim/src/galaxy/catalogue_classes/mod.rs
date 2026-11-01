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
//! the IDs' and the grid's items are re-exported here. Of the classes' processes, phase 7's first
//! three are built: the core collapses (P09.T33.a, [`core_collapse`]), the Type Ia (P09.T34.a,
//! [`type_ia::TypeIaProcess`]) and the luminous blue variables (P09.T43, [`lbv`]). With plan 11's
//! accreting white dwarfs, fast and slow (P11.T8.a, [`binary`]), its X-ray binaries (P11.T8.b) and
//! its stellar and neutron-star mergers (P11.T8.c–d), they are the production lookup's
//! ([`ProductionClasses`]) arms, whose members are catalogue systems ([`system`]) that plan 03's
//! resolve returns. The other processes and the carve-out follow in P09.T35, and the classes'
//! source of the range query in P09.T37.a ([`source`], [`CatalogueClassSource`]).

pub mod binary;
pub mod core_collapse;
pub mod grid;
pub mod ids;
pub mod lbv;
pub mod registry;
pub mod source;
pub mod supernova;
pub mod system;
pub mod type_ia;

pub use self::grid::{
    CatalogueClassCell, CatalogueEntry, CatalogueMember, CheckClassHeadroomError, ClassLookup,
    ClassProcess, ClassVisitor, MemberVisitor, ProductionClasses, cells_in_slab, cells_in_sphere,
    check_class_headroom, class_slabs, entries_when, occupied_cells, resolve, resolve_entry,
};
pub use self::ids::{BuildCatalogueCellKeyError, CatalogueCellKey};
pub use self::registry::{ALL_CLASSES, ClassId, FEATURE_LEVEL_LIST_ORDER};
pub use self::source::CatalogueClassSource;
pub use self::system::{CatalogueMotion, CatalogueSystem, system_of, system_of_record};
