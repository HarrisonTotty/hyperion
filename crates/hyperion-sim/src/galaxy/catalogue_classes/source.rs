//! The catalogue classes in the range query: plan 03's merge hook for the class grid (plan 09,
//! P09.T37.a; brainstorm, "The range query": "the catalogue classes … are merged in").
//!
//! From generator version 26 the grid gives the classes their hosts (P09.T35's carve-out), so a
//! range query that walks the grid alone misses them. [`CatalogueClassSource`] puts them back,
//! beside the grid's layers, for every class the lookup has a process for ([`ProductionClasses`]:
//! the core collapses, the Type Ia, the luminous blue variables and plan 11's five binary classes):
//!
//! - **Census.** A class's expected hosts in the sphere are its cells' bound × the volume each
//!   shares with the cube about the sphere's unpadded radius, which holds the sphere: the
//!   candidates' Poisson mean there, before the thinning and the class's test, so it errs high, as
//!   [`SystemSource`]'s contract allows, and needs no cell generated. It is counted in each layer
//!   its members' primaries' initial masses can give them (layer E for a core collapse, D for a
//!   Type Ia and A–D for its survivor, C and D for an accreting white dwarf), once for each member
//!   that layer can hold.
//! - **Hits.** The class's cells whose entries can lie in the sphere at any time
//!   ([`cells_in_sphere`], padded by the class's reach of 4,096 ly, which bounds every member's
//!   travel over its entry's lifetime, so no layer's walk pad applies: a Type Ia's survivor, which
//!   outruns layer D's, is found as its record asks), each candidate kept as the cell keeps it, and
//!   each member a system ([`ClassProcess::system`]) tested at the sphere's time: one that exists
//!   then and whose present position, the remnant's after an explosion, lies inside the unpadded
//!   radius is a hit, with its record of origin
//!   [`SystemOrigin::CatalogueSystem`](crate::galaxy::placement::SystemOrigin::CatalogueSystem).
//!   A further member is listed from its entry's break only: before it a Type Ia's survivor is
//!   its progenitor's companion, at member 0's place (P09.T34.a's lean).
//! - **No suppression.** A class's hosts are draws of its own, keyed on its own IDs, and from 26
//!   the grid keeps none of them: it gives up the records the core collapses and the luminous blue
//!   variables claim (P09.T35), never keeps a recent Type Ia (`keeps_a_recent_type_ia`) and
//!   redraws every binary of a carved class (P11.T7). So no grid system is a class host's twin, and
//!   the source suppresses nothing.
//!
//! A shell is an attribute of its system, not a hit: the hit's ID resolves to the entry that
//! carries it ([`system_of`](super::system_of)).
//!
//! Each class's process is built once a call, and each candidate is resolved once
//! ([`resolve_entry`]), never once a member read: a Type Ia's survivor alone is resolved again.
//! The source keeps nothing between calls, so a cold query generates every class cell about it
//! (P09.T40's `CatalogueClassCellCache` is the lever). Sums run in a fixed order: the classes in
//! [`ALL_CLASSES`] order, then their cells in the walk's order.

#[cfg(test)]
mod tests;

use super::grid::{ClassLookup, ClassProcess, ClassVisitor, ProductionClasses, candidate_count};
use super::{ALL_CLASSES, CatalogueCellKey, ClassId, cells_in_sphere, resolve_entry};
use crate::coords::ROOT_HALF_WIDTH_LY;
use crate::galaxy::Galaxy;
use crate::galaxy::placement::{Existence, SystemRecord};
use crate::galaxy::query::{LayerCounts, LayerSet, QuerySphere, SystemHit, SystemSource};
use crate::id::Layer;
use crate::time::UniverseTime;
use crate::units::LightYears;

/// The catalogue classes as a source of the range query (module documentation), over the
/// processes of `L`, the production lookup unless a test gives its own.
///
/// # Examples
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::catalogue_classes::{CatalogueClassSource, system_of};
/// use hyperion_sim::galaxy::placement::{NoCache, SystemOrigin};
/// use hyperion_sim::galaxy::query::{RangeQuery, SystemSource, range_query};
/// use hyperion_sim::id::SystemIdKind;
/// use hyperion_sim::units::LightYears;
///
/// let galaxy = Galaxy::new(Seed::new(9)).with_full_potential();
/// let here = GalacticPosition::from_light_years([0.0, 26_000.0, 20.0]).ok_or("in range")?;
/// let query = RangeQuery::builder(here, LightYears::new(50.0)).build()?;
/// let classes = CatalogueClassSource::new();
/// let sources: [&dyn SystemSource; 1] = [&classes];
/// let result = range_query(&galaxy, &mut NoCache::new(), &sources, &query);
/// // Each class host found is a catalogue system, whose ID resolves to its entry and its shell.
/// for hit in result.systems() {
///     if let SystemIdKind::Catalogue(id) = hit.id().kind() {
///         assert!(matches!(hit.record().origin(), SystemOrigin::CatalogueSystem(_)));
///         assert_eq!(system_of(&galaxy, id)?.record(), hit.record());
///     }
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CatalogueClassSource<L = ProductionClasses> {
    lookup: L,
}

impl CatalogueClassSource {
    /// The source over every class the production lookup has a process for.
    ///
    /// Before generator version 26's carve-out the grid holds the classes' hosts itself, so a
    /// query in such a galaxy that takes this source counts them twice.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            lookup: ProductionClasses,
        }
    }
}

impl<L: ClassLookup> CatalogueClassSource<L> {
    /// The source over the classes `lookup` has processes for: a test's or a bench's choice of
    /// classes.
    #[must_use]
    pub const fn with_lookup(lookup: L) -> Self {
        Self { lookup }
    }
}

impl<L: ClassLookup> SystemSource for CatalogueClassSource<L> {
    fn expected_in_sphere(&self, galaxy: &Galaxy, sphere: &QuerySphere) -> LayerCounts {
        let mut counts = LayerCounts::ZERO;
        for class in ALL_CLASSES {
            let visit = Expected {
                galaxy,
                sphere,
                counts: &mut counts,
            };
            // A class without a process (the tidal-disruption victims, a class not yet built) has
            // no hosts to count.
            let _ = self.lookup.visit_class(galaxy, class, visit);
        }
        counts
    }

    fn systems_in_sphere(
        &self,
        galaxy: &Galaxy,
        sphere: &QuerySphere,
        layers: LayerSet,
        out: &mut Vec<SystemHit>,
    ) {
        if layers.is_empty() {
            return;
        }
        for class in ALL_CLASSES {
            let visit = Hits {
                galaxy,
                sphere,
                layers,
                out: &mut *out,
            };
            // As in the census: a class without a process has no hosts to find.
            let _ = self.lookup.visit_class(galaxy, class, visit);
        }
    }

    /// Nothing: the grid keeps no twin of a class host (module documentation).
    fn suppresses(&self, _galaxy: &Galaxy, _record: &SystemRecord, _t: UniverseTime) -> bool {
        false
    }
}

/// The layers A to E as one set.
const STELLAR: LayerSet = LayerSet::EMPTY
    .with(Layer::A)
    .with(Layer::B)
    .with(Layer::C)
    .with(Layer::D)
    .with(Layer::E);

/// Layer E alone.
const E: LayerSet = LayerSet::EMPTY.with(Layer::E);

/// The layers each member of `class`'s entries can be counted in by its primary's initial mass,
/// member 0 first, one set for each member an entry can have; `None` for a class this source does
/// not know.
///
/// Each is its class's own statement: a core collapse's and a luminous blue variable's primary is
/// of band E (P09.T33.a, P09.T43); a Type Ia's of layer D, "every primary is of layer D", and its
/// survivor's its own star's, the donor's of A–D or a Type Iax's partly burnt dwarf's, D
/// (P09.T34.a); an accreting white dwarf's C or D, an X-ray binary's and a neutron-star merger's
/// E, the compact star's progenitor's, and a stellar merger's any (P11.T8). A test holds every
/// generated member to its set.
#[must_use]
fn member_layers(class: ClassId) -> Option<&'static [LayerSet]> {
    const D: LayerSet = LayerSet::EMPTY.with(Layer::D);
    const SURVIVOR: LayerSet = LayerSet::EMPTY
        .with(Layer::A)
        .with(Layer::B)
        .with(Layer::C)
        .with(Layer::D);
    const WHITE_DWARF: LayerSet = LayerSet::EMPTY.with(Layer::C).with(Layer::D);
    match class {
        ClassId::CORE_COLLAPSE
        | ClassId::LUMINOUS_BLUE_VARIABLE
        | ClassId::XRAY_BINARY
        | ClassId::NEUTRON_STAR_MERGER => Some(&[E]),
        ClassId::TYPE_IA => Some(&[D, SURVIVOR]),
        ClassId::ACCRETING_WHITE_DWARF | ClassId::ACCRETING_WHITE_DWARF_SLOW => {
            Some(&[WHITE_DWARF])
        }
        ClassId::STELLAR_MERGER => Some(&[STELLAR]),
        _ => None,
    }
}

/// [`CatalogueClassSource::expected_in_sphere`]'s visit of one class: its expected hosts added to
/// `counts`, in each layer of each of its members.
struct Expected<'a> {
    galaxy: &'a Galaxy,
    sphere: &'a QuerySphere,
    counts: &'a mut LayerCounts,
}

impl ClassVisitor for Expected<'_> {
    type Output = ();

    /// # Panics
    ///
    /// For a class with a process but no [`member_layers`], which the source's tests rule out.
    fn visit<P: ClassProcess>(self, process: &P) {
        let class = process.class();
        let members = member_layers(class).unwrap_or_else(|| {
            panic!("the {class} class has a process but no member layers in the range query")
        });
        let hosts = expected_hosts(self.galaxy, process, self.sphere);
        for layer in Layer::ALL {
            let slots = members.iter().filter(|set| set.contains(layer)).count();
            if slots > 0 {
                let weight = f64::from(u8::try_from(slots).expect("at most 16 members"));
                self.counts
                    .set(layer, self.counts.get(layer) + weight * hosts);
            }
        }
    }
}

/// `process`'s expected hosts about `sphere`: over each class cell that meets the cube of the
/// sphere's unpadded radius about its centre, in ascending x, then y, then z, its bound × the
/// volume it shares with the cube (module documentation).
#[must_use]
fn expected_hosts<P: ClassProcess>(galaxy: &Galaxy, process: &P, sphere: &QuerySphere) -> f64 {
    let class = process.class();
    let Some(log2) = class.cell_log2_ly() else {
        return 0.0;
    };
    let edge = f64::from(1_u32 << log2);
    let half = ROOT_HALF_WIDTH_LY >> log2;
    let centre = sphere.centre().to_light_years_f64();
    let radius = sphere.radius().value();
    // Each axis's cells and the length of each that lies inside [c − R, c + R].
    let spans = centre.map(|c| {
        let (low, high) = (c - radius, c + radius);
        let first = cell_index(low, edge).max(-half);
        let last = cell_index(high, edge).min(half - 1);
        (first..=last)
            .map(|i| {
                let start = f64::from(i) * edge;
                (i, high.min(start + edge) - low.max(start))
            })
            .filter(|&(_, length)| length > 0.0)
            .collect::<Vec<_>>()
    });
    let mut hosts = 0.0;
    for &(x, lx) in &spans[0] {
        for &(y, ly) in &spans[1] {
            for &(z, lz) in &spans[2] {
                let key = CatalogueCellKey::new(class, [x, y, z])
                    .expect("a cell clipped to the cube is a cell of the class");
                hosts += process.bound(galaxy, &key).value() * (lx * ly * lz);
            }
        }
    }
    hosts
}

/// The cell, `edge` light-years wide, that holds the coordinate `ly`, clamped to `i32`'s range:
/// the caller clips it to the cube.
#[must_use]
fn cell_index(ly: f64, edge: f64) -> i32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the floor of a coordinate within a few cube widths of the origin, over a cell of \
                  512 ly or more, is far inside i32, and the cast saturates beyond it"
    )]
    let index = (ly / edge).floor() as i32;
    index
}

/// [`CatalogueClassSource::systems_in_sphere`]'s visit of one class: its members in `layers`
/// inside the sphere at its time, appended to `out`.
struct Hits<'a> {
    galaxy: &'a Galaxy,
    sphere: &'a QuerySphere,
    layers: LayerSet,
    out: &'a mut Vec<SystemHit>,
}

impl ClassVisitor for Hits<'_> {
    type Output = ();

    fn visit<P: ClassProcess>(self, process: &P) {
        let Self {
            galaxy,
            sphere,
            layers,
            out,
        } = self;
        let class = process.class();
        let asked = |set: &LayerSet| layers.iter().any(|layer| set.contains(layer));
        if member_layers(class).is_some_and(|members| !members.iter().any(asked)) {
            // No member of the class can be in a layer asked for.
            return;
        }
        for key in cells_in_sphere(class, sphere) {
            let bound = process.bound(galaxy, &key);
            for index in 0..candidate_count(galaxy.seed(), &key, bound) {
                let id = key
                    .candidate_id(index, 0)
                    .expect("a cell's count is held under 2^24, so its candidates have IDs");
                let Ok(first) = resolve_entry(galaxy, process, id) else {
                    continue;
                };
                let system = process.system(galaxy, &first);
                push_hit(
                    system.record(),
                    system.position_at(sphere.time()),
                    sphere,
                    layers,
                    out,
                );
                if sphere.time() < system.motion().at() {
                    continue;
                }
                for member in 1..first.entry().members() {
                    let id = first
                        .entry()
                        .member_id(member)
                        .expect("a member under its entry's count");
                    let member = resolve_entry(galaxy, process, id)
                        .expect("an entry's further members resolve as the entry does");
                    let system = process.system(galaxy, &member);
                    push_hit(
                        system.record(),
                        system.position_at(sphere.time()),
                        sphere,
                        layers,
                        out,
                    );
                }
            }
        }
    }
}

/// Appends `record`'s hit to `out` if it is in `layers`, exists at the sphere's time and lies, at
/// `position`, inside its unpadded radius: [`hit_at`](crate::galaxy::query::hit_at)'s test, with
/// the position its class's motion gives.
fn push_hit(
    record: &SystemRecord,
    position: crate::coords::GalacticPosition,
    sphere: &QuerySphere,
    layers: LayerSet,
    out: &mut Vec<SystemHit>,
) {
    if !layers.contains(record.layer()) {
        return;
    }
    match record.existence_at(sphere.time()) {
        Existence::NoSystemYet => return,
        Existence::Exists => {}
    }
    let distance = LightYears::from(sphere.centre().distance_to(&position));
    if distance.value() <= sphere.radius().value() {
        out.push(SystemHit::new(*record, position, distance));
    }
}
