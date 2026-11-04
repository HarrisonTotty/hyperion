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
//!   Type Ia and A–D for its survivor, C and D for an accreting white dwarf, and E for an X-ray
//!   binary, or D and E under the fitted tables of the 27 hold), once for each member that layer
//!   can hold.
//! - **Hits.** The class's cells whose entries can lie in the sphere at any time
//!   ([`cells_in_sphere`], padded by the class's reach of 4,096 ly, which bounds every member's
//!   travel over its entry's lifetime, so no layer's walk pad applies: a Type Ia's survivor, which
//!   outruns layer D's, is found as its record asks) that draw any candidate
//!   ([`occupied_cells`](super::occupied_cells)), each candidate kept as the cell keeps it, and
//!   each member a system ([`ClassProcess::system`])
//!   tested at the sphere's time. A class that reads its candidates' place first
//!   ([`ClassProcess::epoch_place`], the grid's rule 11) has only those built whose place lies
//!   within the sphere padded at plan 03's [`PAD_SPEED`] (P09.T40): its members move from there
//!   below that, so no other can reach the sphere. Its places lie in their cells, so it walks only
//!   the cells that padded sphere meets. A member that exists
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
//! Sums run in a fixed order: the classes in [`ALL_CLASSES`] order, then their cells in the walk's
//! order.
//!
//! **The class-cell cache** (P09.T40). What a hit is made from in each cell, the places read first
//! and the other classes' members ([`ClassCellSystems`]), is epoch state, so a caller may keep it
//! between queries ([`CatalogueClassCellCache`]; [`CatalogueClassSource::with_cells`]). A warm
//! query then draws only the candidates whose place its sphere can reach. Without a cache
//! ([`NoClassCellCache`]) every cell's systems are made for the query and dropped. Each cell is a
//! pure function of the galaxy and its key, so a server may make a query's cells
//! ([`CatalogueClassSource::cells_read`]) on several threads before it asks for the hits
//! ([`CachedClassSource::read_ahead`]; P09.T37's bench).
//!
//! The range query registers the source beside the grid from generator version 27
//! ([`CatalogueClassSource::registered_in`]).

#[cfg(test)]
mod tests;

use core::num::NonZeroU32;
use std::sync::Arc;

use super::grid::{
    ClassLookup, ClassProcess, ClassVisitor, ProductionClasses, candidate_count, entry_systems_in,
};
use super::{
    ALL_CLASSES, CatalogueCellKey, CatalogueSystem, ClassId, cells_in_sphere, resolve_entry,
};
use crate::coords::{GalacticPosition, ROOT_HALF_WIDTH_LY};
use crate::galaxy::Galaxy;
use crate::galaxy::features::members::source::part_of_key;
use crate::galaxy::placement::SystemRecord;
use crate::galaxy::query::{
    LayerCounts, LayerSet, PAD_SPEED, QuerySphere, RangeQuery, SystemHit, SystemSource, pad_for,
    widest_sphere,
};
use crate::id::{CatalogueSystemId, Layer};
use crate::math;
use crate::time::{Span, UniverseTime};
use crate::units::LightYears;
use crate::units::consts::SPEED_OF_LIGHT;

/// The first word of a class cell's key for the sky's member parts ([`part_of_key`]), `"class"` in
/// ASCII.
const CLASS_PART_WORD: u64 = 0x0000_0063_6c61_7373;

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
/// // A server takes the source where the galaxy's rules register it, from generator version 27.
/// let classes = CatalogueClassSource::registered_in(&galaxy);
/// let sources: Vec<&dyn SystemSource> = classes.iter().map(|s| s as _).collect();
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
    /// The source over every class the production lookup has a process for, whatever a galaxy's
    /// rules: what a range query registers ([`registered_in`](Self::registered_in)).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            lookup: ProductionClasses,
        }
    }

    /// The source a range query in `galaxy` registers beside the grid: [`new`](Self::new)'s, in
    /// every galaxy from generator version 27 (P09.T37.a).
    #[must_use]
    pub const fn registered_in(_galaxy: &Galaxy) -> Option<Self> {
        Some(Self::new())
    }
}

impl<L: ClassLookup> CatalogueClassSource<L> {
    /// The source over the classes `lookup` has processes for: a test's or a bench's choice of
    /// classes.
    #[must_use]
    pub const fn with_lookup(lookup: L) -> Self {
        Self { lookup }
    }

    /// The same source, reading each class cell's systems through `cells` (P09.T40): what a server
    /// registers, over the cache it shares between queries. Its census and hits are this source's,
    /// bit for bit, whatever the cache holds.
    #[must_use]
    pub const fn with_cells(self, cells: &dyn CatalogueClassCellCache) -> CachedClassSource<'_, L> {
        CachedClassSource {
            source: self,
            cells,
        }
    }

    /// Appends the hits in `layers` inside `sphere` at its time, each class cell's systems read
    /// through `cells`.
    fn hits(
        &self,
        galaxy: &Galaxy,
        sphere: &QuerySphere,
        layers: LayerSet,
        cells: &dyn CatalogueClassCellCache,
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
                cells,
                out: &mut *out,
            };
            // As in the census: a class without a process has no hosts to find.
            let _ = self.lookup.visit_class(galaxy, class, visit);
        }
    }

    /// The class cells the source's hits read for `query` in `layers`: class by class in
    /// [`ALL_CLASSES`] order, and each class's in the order its hits read them (module
    /// documentation, "Hits"), about the sphere the range query asks its sources about.
    ///
    /// They are what a server makes ahead of the query on several threads
    /// ([`CachedClassSource::read_ahead`]; P09.T37's bench). A census that admits fewer layers
    /// reads only some of them, so the layers of a plan made without the sources, which hold the
    /// plan's with them (`RangePlan::with_sources`), give every cell the query reads.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::coords::GalacticPosition;
    /// use hyperion_sim::galaxy::Galaxy;
    /// use hyperion_sim::galaxy::catalogue_classes::{CatalogueClassSource, NoClassCellCache};
    /// use hyperion_sim::galaxy::query::{RangePlan, RangeQuery};
    /// use hyperion_sim::units::LightYears;
    ///
    /// let galaxy = Galaxy::new(Seed::new(9)).with_full_potential();
    /// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).ok_or("in range")?;
    /// let query = RangeQuery::builder(sun, LightYears::new(50.0)).build()?;
    /// let layers = RangePlan::new(&galaxy, &[], &query).census().layers();
    /// let classes = CatalogueClassSource::new();
    /// let cells = classes.cells_read(&galaxy, &query, layers);
    /// // Each share of the cells may be made on a thread of its own, into a cache that keeps them.
    /// for share in cells.chunks(256) {
    ///     classes.with_cells(&NoClassCellCache).read_ahead(&galaxy, share);
    /// }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn cells_read(
        &self,
        galaxy: &Galaxy,
        query: &RangeQuery,
        layers: LayerSet,
    ) -> Vec<CatalogueCellKey> {
        let mut cells = Vec::new();
        if layers.is_empty() {
            return cells;
        }
        let sphere = widest_sphere(query);
        for class in ALL_CLASSES {
            let visit = CellsRead {
                sphere: &sphere,
                layers,
                out: &mut cells,
            };
            // A class without a process reads no cell.
            let _ = self.lookup.visit_class(galaxy, class, visit);
        }
        cells
    }
}

/// A [`CatalogueClassSource`] whose hits read each class cell's systems through a
/// [`CatalogueClassCellCache`] ([`CatalogueClassSource::with_cells`]; P09.T40).
#[derive(Clone, Copy)]
pub struct CachedClassSource<'a, L = ProductionClasses> {
    source: CatalogueClassSource<L>,
    cells: &'a dyn CatalogueClassCellCache,
}

impl<L: ClassLookup> CachedClassSource<'_, L> {
    /// Makes the systems of each of `cells` through the source's cache, as its hits make them, one
    /// class's process for each run of cells of one class: what a server does on several threads
    /// for a query's [`cells_read`](CatalogueClassSource::cells_read) before the query asks for its
    /// hits, so that its lookups find the cells held. Each is a pure function of the galaxy and its
    /// key, so it changes no answer, and it is lost if the cache keeps nothing.
    pub fn read_ahead(&self, galaxy: &Galaxy, cells: &[CatalogueCellKey]) {
        for run in cells.chunk_by(|a, b| a.class() == b.class()) {
            let visit = ReadAhead {
                galaxy,
                keys: run,
                cells: self.cells,
            };
            // A class without a process has no cell to make.
            let _ = self
                .source
                .lookup
                .visit_class(galaxy, run[0].class(), visit);
        }
    }

    /// Each class host of `layer` of the classes `taken` holds that may lie between `inner` and
    /// `outer` of `centre` at some time of `window`, seen by its end, given to `each` in the
    /// source's order: the classes in [`ALL_CLASSES`] order, then the cells its hits would read for
    /// the sphere of `outer`
    /// about `centre` at the window's farthest time from the epoch, those of the part `part` of
    /// `parts` by their key ([`part_of_key`]), then each cell's systems in the hits' order
    /// (rendering plan R06, R06.T16.a). A placed class's candidate is built only if its place,
    /// padded by the farthest its hosts drift over the window, reaches the shell; and a host's
    /// broken line over the window, opened earlier by the light-cone step's residual
    /// ([`seen_in_shell`]), must come within `outer` and reach out to `inner`. It may give a host
    /// beyond the shell, never leave one out.
    ///
    /// # Panics
    ///
    /// If `outer` is not finite and positive.
    pub(crate) fn members_in_shell_over(
        &self,
        galaxy: &Galaxy,
        centre: &GalacticPosition,
        window: (UniverseTime, UniverseTime),
        ((layer, inner, outer), part): ((Layer, LightYears, LightYears), (u32, NonZeroU32)),
        taken: &dyn Fn(ClassId) -> bool,
        each: &mut dyn FnMut(&CatalogueSystem),
    ) {
        let (from, to) = window;
        let farthest = if from.since_epoch().as_seconds_f64().abs()
            > to.since_epoch().as_seconds_f64().abs()
        {
            from
        } else {
            to
        };
        let sphere = QuerySphere::new(*centre, outer, farthest, LightYears::ZERO)
            .expect("a shell's outer distance is finite and positive");
        for class in ALL_CLASSES.into_iter().filter(|&class| taken(class)) {
            let visit = ShellHosts {
                galaxy,
                sphere: &sphere,
                window,
                shell: (layer, inner),
                part,
                cells: self.cells,
                each: &mut *each,
            };
            // A class without a process has no hosts to give.
            let _ = self.source.lookup.visit_class(galaxy, class, visit);
        }
    }
}

impl<L: std::fmt::Debug> std::fmt::Debug for CachedClassSource<'_, L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CachedClassSource")
            .field("source", &self.source)
            .finish_non_exhaustive()
    }
}

impl<L: ClassLookup> SystemSource for CachedClassSource<'_, L> {
    fn expected_in_sphere(&self, galaxy: &Galaxy, sphere: &QuerySphere) -> LayerCounts {
        self.source.expected_in_sphere(galaxy, sphere)
    }

    fn systems_in_sphere(
        &self,
        galaxy: &Galaxy,
        sphere: &QuerySphere,
        layers: LayerSet,
        out: &mut Vec<SystemHit>,
    ) {
        self.source.hits(galaxy, sphere, layers, self.cells, out);
    }

    /// Nothing, as [`CatalogueClassSource`].
    fn suppresses(&self, _galaxy: &Galaxy, _record: &SystemRecord, _t: UniverseTime) -> bool {
        false
    }
}

/// What the range query keeps of one class cell between queries (P09.T40): its candidate count,
/// each place a class reads first (the grid's rule 11) with its candidate's index, and every
/// member of every entry of a class that reads none, in index and member order.
///
/// It is epoch state alone, a pure function of the galaxy and the cell, so a cache that lends it
/// changes no hit. A place is a few bytes and costs a stream's first three words, where a
/// candidate built costs its whole draw, so a placed class keeps its places and builds only the
/// candidates a query's sphere can reach.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassCellSystems {
    candidates: u32,
    places: Vec<(u32, GalacticPosition)>,
    members: Vec<CatalogueSystem>,
}

impl ClassCellSystems {
    /// The cell's candidate count.
    #[must_use]
    pub const fn candidates(&self) -> u32 {
        self.candidates
    }

    /// The places read first, each with its candidate's index, in index order.
    #[must_use]
    pub fn places(&self) -> &[(u32, GalacticPosition)] {
        &self.places
    }

    /// The members of the entries of the candidates that read no place, in index and member
    /// order.
    #[must_use]
    pub fn members(&self) -> &[CatalogueSystem] {
        &self.members
    }

    /// The bytes it owns on the heap, by its buffers' capacities: what a byte-bounded cache charges
    /// it.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.places.capacity() * size_of::<(u32, GalacticPosition)>()
            + self.members.capacity() * size_of::<CatalogueSystem>()
    }

    /// The systems of cell `key` of `process`'s class (module documentation, "Hits"): one bound
    /// for the cell, as [`occupied_cells`](super::occupied_cells) takes it, and each entry kept
    /// under it ([`entry_systems_in`]), as its members' resolves would keep it.
    #[must_use]
    fn of<P: ClassProcess>(galaxy: &Galaxy, process: &P, key: CatalogueCellKey) -> Self {
        assert_eq!(
            key.class(),
            process.class(),
            "a cell of the process's own class"
        );
        let bound = process.bound(galaxy, &key);
        let candidates = candidate_count(galaxy.seed(), &key, bound);
        let (mut places, mut members) = (Vec::new(), Vec::new());
        for index in 0..candidates {
            let id = key
                .candidate_id(index, 0)
                .expect("a cell's count is held under 2^24, so its candidates have IDs");
            let place = process.epoch_place(galaxy, &key, id);
            debug_assert_eq!(
                place.is_some(),
                process.reads_place_first(),
                "a class reads every candidate's place first or none (rule 11)"
            );
            match place {
                Some(place) => places.push((index, place)),
                None => members.extend(entry_systems_in(galaxy, process, &key, id, bound)),
            }
        }
        places.shrink_to_fit();
        members.shrink_to_fit();
        Self {
            candidates,
            places,
            members,
        }
    }
}

/// Where a caller keeps class cells' systems between range queries (P09.T40).
///
/// It takes `&self`, so an implementation that keeps them uses interior mutability. It must lend
/// for a cell exactly what `make` returns, which is a pure function of the galaxy and the cell.
/// **An implementation that keeps them between calls keys them by galaxy as well as by cell**, as
/// plan 03's `CellCache` and the features' `FeatureCellCache` document: the trait does not check
/// the galaxy.
pub trait CatalogueClassCellCache {
    /// The systems of class cell `key` in `galaxy`: what `make` returns, called now or for an
    /// earlier lookup of the same cell.
    fn systems(
        &self,
        galaxy: &Galaxy,
        key: CatalogueCellKey,
        make: &mut dyn FnMut() -> ClassCellSystems,
    ) -> Arc<ClassCellSystems>;
}

/// The cache that keeps nothing: every lookup makes its cell's systems.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NoClassCellCache;

impl CatalogueClassCellCache for NoClassCellCache {
    fn systems(
        &self,
        _galaxy: &Galaxy,
        _key: CatalogueCellKey,
        make: &mut dyn FnMut() -> ClassCellSystems,
    ) -> Arc<ClassCellSystems> {
        Arc::new(make())
    }
}

/// The systems of the members of candidate `id`'s entry, in member order, or none if its class does
/// not keep it.
///
/// # Panics
///
/// Never: an entry's further members resolve as the entry does.
fn entry_systems<P: ClassProcess>(
    galaxy: &Galaxy,
    process: &P,
    id: CatalogueSystemId,
) -> Vec<CatalogueSystem> {
    let Ok(first) = resolve_entry(galaxy, process, id) else {
        return Vec::new();
    };
    let mut systems = vec![process.system(galaxy, &first)];
    for member in 1..first.entry().members() {
        let id = first
            .entry()
            .member_id(member)
            .expect("a member under its entry's count");
        let member = resolve_entry(galaxy, process, id)
            .expect("an entry's further members resolve as the entry does");
        systems.push(process.system(galaxy, &member));
    }
    systems
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
        self.hits(galaxy, sphere, layers, &NoClassCellCache, out);
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
/// (P09.T34.a); an accreting white dwarf's C to E, the fitted rows giving layer E a share of the
/// hosts (P15.T10.b's refit at 27: a primary of a little over 8 M☉ whose transfer left a white
/// dwarf, as host 0xfc68bbc7f0000020's of 8.07 M☉); an X-ray binary's the compact star's
/// progenitor's, D or E under the fitted tables (P15.T10.b), whose engine-fitted progenitors reach
/// down to 10^0.54 M☉, 3.5 M☉, for a neutron star and 10^0.51 M☉, 3.2 M☉, for a black hole
/// (`XRB_RESIDUALS_26`'s lowest quantiles), the accretors of earlier transfer that grew past the
/// core-collapse mass; a neutron-star merger's E; and a stellar merger's any (P11.T8). A test holds
/// every generated member to its set.
#[must_use]
pub(crate) fn member_layers(class: ClassId) -> Option<&'static [LayerSet]> {
    const D: LayerSet = LayerSet::EMPTY.with(Layer::D);
    const SURVIVOR: LayerSet = LayerSet::EMPTY
        .with(Layer::A)
        .with(Layer::B)
        .with(Layer::C)
        .with(Layer::D);
    const WHITE_DWARF: LayerSet = LayerSet::EMPTY.with(Layer::C).with(Layer::D).with(Layer::E);
    const COMPACT_PROGENITOR: LayerSet = LayerSet::EMPTY.with(Layer::D).with(Layer::E);
    match class {
        ClassId::CORE_COLLAPSE | ClassId::LUMINOUS_BLUE_VARIABLE | ClassId::NEUTRON_STAR_MERGER => {
            Some(&[E])
        }
        ClassId::XRAY_BINARY => Some(&[COMPACT_PROGENITOR]),
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
    let spans = cube_spans(log2, sphere.centre(), sphere.radius());
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

/// Each axis's cells of `2^log2` ly that meet the cube of half-width `radius` about `centre`,
/// clipped to the root cube, in ascending order, each with the length of it inside the cube, ly.
#[must_use]
fn cube_spans(log2: u32, centre: &GalacticPosition, radius: LightYears) -> [Vec<(i32, f64)>; 3] {
    let edge = f64::from(1_u32 << log2);
    let half = ROOT_HALF_WIDTH_LY >> log2;
    let radius = radius.value();
    centre.to_light_years_f64().map(|c| {
        let (low, high) = (c - radius, c + radius);
        let first = cell_index(low, edge).max(-half);
        let last = cell_index(high, edge).min(half - 1);
        (first..=last)
            .map(|i| {
                let start = f64::from(i) * edge;
                (i, high.min(start + edge) - low.max(start))
            })
            .filter(|&(_, length)| length > 0.0)
            .collect()
    })
}

/// The margin on the cube a placed class's cells are walked over, ly: far above the rounding of a
/// place's light-year coordinate, some 10⁻¹¹ ly, and below any cell's 512 ly.
const CUBE_MARGIN_LY: f64 = 1.0;

/// `class`'s cells that meet the cube of half-width `radius` about `centre`, in ascending x, then
/// y, then z: every cell holding a point within `radius` of `centre`, and none for a class that is
/// never placed.
#[must_use]
fn cells_about(
    class: ClassId,
    centre: &GalacticPosition,
    radius: LightYears,
) -> Vec<CatalogueCellKey> {
    let Some(log2) = class.cell_log2_ly() else {
        return Vec::new();
    };
    let [xs, ys, zs] = cube_spans(log2, centre, radius);
    let mut keys = Vec::with_capacity(xs.len() * ys.len() * zs.len());
    for &(x, _) in &xs {
        for &(y, _) in &ys {
            for &(z, _) in &zs {
                keys.push(
                    CatalogueCellKey::new(class, [x, y, z])
                        .expect("a cell clipped to the cube is a cell of the class"),
                );
            }
        }
    }
    keys
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
/// inside the sphere at its time, appended to `out`, its cells' systems read through `cells`.
struct Hits<'a> {
    galaxy: &'a Galaxy,
    sphere: &'a QuerySphere,
    layers: LayerSet,
    cells: &'a dyn CatalogueClassCellCache,
    out: &'a mut Vec<SystemHit>,
}

impl ClassVisitor for Hits<'_> {
    type Output = ();

    fn visit<P: ClassProcess>(self, process: &P) {
        let Self {
            galaxy,
            sphere,
            layers,
            cells,
            out,
        } = self;
        let class = process.class();
        if !may_be_asked(class, layers) {
            return;
        }
        let reach = place_reach(sphere);
        for key in hit_cells(process, sphere) {
            let systems = cells.systems(galaxy, key, &mut || {
                ClassCellSystems::of(galaxy, process, key)
            });
            for &(index, place) in systems.places() {
                if LightYears::from(sphere.centre().distance_to(&place)).value() > reach.value() {
                    continue;
                }
                let id = key
                    .candidate_id(index, 0)
                    .expect("a cell's count is held under 2^24, so its candidates have IDs");
                for system in entry_systems(galaxy, process, id) {
                    debug_assert_eq!(
                        *system.record().epoch_position(),
                        place,
                        "the {class} candidate {id:?}'s place read first is its entry's (rule 11)"
                    );
                    push_hit(&system, sphere, layers, out);
                }
            }
            for system in systems.members() {
                push_hit(system, sphere, layers, out);
            }
        }
    }
}

/// Whether a member of `class`'s entries can be in a layer of `layers`: not where every member's
/// layers miss them all.
#[must_use]
fn may_be_asked(class: ClassId, layers: LayerSet) -> bool {
    let asked = |set: &LayerSet| layers.iter().any(|layer| set.contains(layer));
    member_layers(class).is_none_or(|members| members.iter().any(asked))
}

/// The cells of `process`'s class whose systems its hits about `sphere` read, in the order they
/// read them. A placed class's places lie in their cells, so only the cells its padded sphere meets
/// can hold one it reaches (the grid's rule 11); any other class's entries may lie anywhere in
/// their reach.
#[must_use]
fn hit_cells<P: ClassProcess>(process: &P, sphere: &QuerySphere) -> Vec<CatalogueCellKey> {
    let class = process.class();
    if process.reads_place_first() {
        cells_about(
            class,
            sphere.centre(),
            place_reach(sphere) + LightYears::new(CUBE_MARGIN_LY),
        )
    } else {
        cells_in_sphere(class, sphere).collect()
    }
}

/// [`CachedClassSource::members_in_shell_over`]'s visit of one class: its hosts of the shell's
/// layer that may lie in the shell over `window`, given to `each`, its cells' systems read through
/// `cells`.
struct ShellHosts<'a> {
    galaxy: &'a Galaxy,
    sphere: &'a QuerySphere,
    window: (UniverseTime, UniverseTime),
    /// The shell's layer and inner distance; its outer is the sphere's radius.
    shell: (Layer, LightYears),
    /// The part of the cells given, and of how many.
    part: (u32, NonZeroU32),
    cells: &'a dyn CatalogueClassCellCache,
    each: &'a mut dyn FnMut(&CatalogueSystem),
}

impl ClassVisitor for ShellHosts<'_> {
    type Output = ();

    fn visit<P: ClassProcess>(self, process: &P) {
        let Self {
            galaxy,
            sphere,
            window,
            shell: (layer, inner),
            part: (part, parts),
            cells,
            each,
        } = self;
        if !may_be_asked(process.class(), LayerSet::EMPTY.with(layer)) {
            return;
        }
        let reach = place_reach(sphere);
        // The farthest a placed class's host drifts from its place over the window, with
        // `place_reach`'s margins.
        let drift = reach.value() - sphere.radius().value();
        let span = (inner, sphere.radius());
        let mut offer = |system: &CatalogueSystem| {
            if seen_in_shell(system, layer, sphere.centre(), window, span) {
                each(system);
            }
        };
        for key in hit_cells(process, sphere) {
            if part_of_key(&[CLASS_PART_WORD, key.word()], parts) != part {
                continue;
            }
            let systems = cells.systems(galaxy, key, &mut || {
                ClassCellSystems::of(galaxy, process, key)
            });
            for &(index, place) in systems.places() {
                let d = LightYears::from(sphere.centre().distance_to(&place)).value();
                // Its hosts stay within `drift` of the place, so none from here meets the shell.
                if d > reach.value() || d + drift < inner.value() {
                    continue;
                }
                let id = key
                    .candidate_id(index, 0)
                    .expect("a cell's count is held under 2^24, so its candidates have IDs");
                for system in entry_systems(galaxy, process, id) {
                    offer(&system);
                }
            }
            for system in systems.members() {
                offer(system);
            }
        }
    }
}

/// Whether `system` may be of `layer` and between `inner` and `outer` of `centre` in light
/// emitted in `window`: of the layer, seen by the window's end ([`CatalogueSystem::shown_at`]),
/// and its broken line, the straight runs between the window's ends and the break within it,
/// coming within `outer` of `centre` and reaching out to `inner`, each with a margin over the
/// rounding. The line is opened before the window by twice the census's light-cone step's
/// residual, at the host's faster speed.
///
/// # Panics
///
/// If the window's start, so opened, is off the clock, which no cap's is.
#[must_use]
fn seen_in_shell(
    system: &CatalogueSystem,
    layer: Layer,
    centre: &GalacticPosition,
    (from, to): (UniverseTime, UniverseTime),
    (inner, outer): (LightYears, LightYears),
) -> bool {
    /// The margin on each distance, ly: far above the rounding of a point of the root cube about
    /// a line's ends, some 10⁻¹¹ ly.
    const MARGIN_LY: f64 = 1e-6;
    /// How many times the light-cone step's bound on its residual the line is opened by.
    const LEAD_FACTOR: f64 = 2.0;
    /// The lead over the emitted time's rounding to the nanosecond, s.
    const ROUNDING_S: f64 = 1.0;
    if system.record().layer() != layer || !system.shown_at(to) {
        return false;
    }
    let motion = system.motion();
    // The census's one fixed-point step on the light cone (`observe::retarded`) emits within (v ÷
    // c)² of the light time of the cone's emission, so a host it keeps may be seen in light from
    // before the window.
    let beta = motion.fastest().value() / SPEED_OF_LIGHT;
    let light_s = to
        .checked_since(from)
        .expect("a window's ends are a span apart")
        .as_seconds_f64();
    let lead = Span::from_seconds_f64(LEAD_FACTOR * beta * beta * light_s + ROUNDING_S)
        .expect("a lead of a few years is a span");
    let start = from
        .checked_sub(lead)
        .expect("a window's start less a few years is on the clock");
    let c = centre.to_light_years_f64();
    // A break outside the opened window repeats an end, a run of no length.
    let points = [start, motion.at().clamp(start, to), to]
        .map(|t| system.position_at(t).to_light_years_f64());
    // Each run's distance from the centre is convex along it, so its farthest is at an end.
    let farthest = points
        .iter()
        .map(|&p| length(core::array::from_fn(|k| p[k] - c[k])))
        .fold(0.0, f64::max);
    let nearest = points
        .windows(2)
        .map(|run| distance_to_run(c, run[0], run[1]))
        .fold(f64::INFINITY, f64::min);
    nearest <= outer.value() + MARGIN_LY && farthest + MARGIN_LY >= inner.value()
}

/// The distance from `c` to the straight run from `a` to `b`, all in light-years.
#[must_use]
fn distance_to_run(c: [f64; 3], a: [f64; 3], b: [f64; 3]) -> f64 {
    let ab: [f64; 3] = core::array::from_fn(|k| b[k] - a[k]);
    let ac: [f64; 3] = core::array::from_fn(|k| c[k] - a[k]);
    let along = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
    let s = if along > 0.0 {
        ((ac[0] * ab[0] + ac[1] * ab[1] + ac[2] * ab[2]) / along).clamp(0.0, 1.0)
    } else {
        0.0
    };
    length(core::array::from_fn(|k| ac[k] - s * ab[k]))
}

/// The length of `v`.
#[must_use]
fn length(v: [f64; 3]) -> f64 {
    math::hypot(math::hypot(v[0], v[1]), v[2])
}

/// [`CatalogueClassSource::cells_read`]'s visit of one class: the cells its hits in `layers` about
/// the sphere read, appended to `out`.
struct CellsRead<'a> {
    sphere: &'a QuerySphere,
    layers: LayerSet,
    out: &'a mut Vec<CatalogueCellKey>,
}

impl ClassVisitor for CellsRead<'_> {
    type Output = ();

    fn visit<P: ClassProcess>(self, process: &P) {
        if may_be_asked(process.class(), self.layers) {
            self.out.extend(hit_cells(process, self.sphere));
        }
    }
}

/// [`CachedClassSource::read_ahead`]'s visit of one class: the systems of each of `keys`, its
/// cells, made through `cells` and dropped.
struct ReadAhead<'a> {
    galaxy: &'a Galaxy,
    keys: &'a [CatalogueCellKey],
    cells: &'a dyn CatalogueClassCellCache,
}

impl ClassVisitor for ReadAhead<'_> {
    type Output = ();

    fn visit<P: ClassProcess>(self, process: &P) {
        let Self {
            galaxy,
            keys,
            cells,
        } = self;
        for &key in keys {
            drop(cells.systems(galaxy, key, &mut || {
                ClassCellSystems::of(galaxy, process, key)
            }));
        }
    }
}

/// How far from `sphere`'s centre a class host may lie at the epoch and still be inside the sphere
/// at its time, for a host moving straight below [`PAD_SPEED`] (the grid's rule 11): its radius
/// plus the farthest such a host drifts, with margins over the rounding of the drift and of the
/// distance, some 10⁻¹¹ ly about a point of the root cube, far below them.
#[must_use]
fn place_reach(sphere: &QuerySphere) -> LightYears {
    /// The relative margin on the drift.
    const DRIFT_MARGIN: f64 = 1e-9;
    /// The absolute margin, ly.
    const NEAR_MARGIN_LY: f64 = 1e-6;
    let drift = pad_for(sphere.time(), PAD_SPEED).value();
    LightYears::new(sphere.radius().value() + drift * (1.0 + DRIFT_MARGIN) + NEAR_MARGIN_LY)
}

/// Appends `system`'s hit to `out` if it is in `layers`, is seen at the sphere's time
/// ([`CatalogueSystem::shown_at`]: it exists then, a further member of an entry from its own break)
/// and lies, at its class's position then, inside the unpadded radius:
/// [`hit_at`](crate::galaxy::query::hit_at)'s test.
fn push_hit(
    system: &CatalogueSystem,
    sphere: &QuerySphere,
    layers: LayerSet,
    out: &mut Vec<SystemHit>,
) {
    let record = system.record();
    if !layers.contains(record.layer()) || !system.shown_at(sphere.time()) {
        return;
    }
    let position = system.position_at(sphere.time());
    let distance = LightYears::from(sphere.centre().distance_to(&position));
    if distance.value() <= sphere.radius().value() {
        out.push(SystemHit::new(*record, position, distance));
    }
}
