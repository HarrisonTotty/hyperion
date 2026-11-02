//! The global list in the range query: plan 03's merge hook for the streams, the dwarf cores and
//! the galactic centre (plan 10, P10.T9; brainstorm, "Streams and accreted structure" and "The
//! range query").
//!
//! [`GlobalListSource`] answers a query sphere with the members of every entry of the galaxy's
//! [`GlobalList`] that can reach it:
//!
//! - **The shells first.** An entry whose bounding shell the sphere misses holds no member that
//!   can reach it, so a stream's is passed by without its tube table (module documentation of
//!   [`global_list`](super), "Bounding shells"). The census tests the shells against the
//!   unpadded radius at the epoch, the hits against the radius padded at
//!   [`UNBOUND_PAD_SPEED`], above every member's speed bound. A stream's shell bounds its members
//!   (from version 26, P10.T9), and a dwarf core's members lie within its extent of its centre,
//!   which its shell holds.
//! - **A stream**: its table through the caller's [`TubeLookup`], the tube cells touching the
//!   sphere ([`TubeTable::cells_touching`]) and their members ([`stream_cell_members`]). The
//!   census is the exact sum of the means ([`expected_members`]) of the cells touching the
//!   unpadded radius, band by band, which errs high, as [`SystemSource`]'s contract allows, since
//!   a cell can touch the sphere and hold no member inside it, and a member can lie outside the
//!   root cube or, from 26, its shell. The hits take the cells touching the radius padded band by
//!   band at the larger of the band's layer's [`pad_speed`](crate::galaxy::query::pad_speed) and
//!   the tube's own members' bound ([`TubeTable::member_speed_bound`]), plan 09's rule for a
//!   feature (P09.T23.b).
//! - **A dwarf core**: plan 09's nested-grid walk on its model ([`DwarfCoreModel`], P10.T7.b),
//!   as a catalogue feature's: the census sums the expected candidates of the owned cells touching
//!   the unpadded radius, and the hits read the cells within the padded one at its own bound
//!   ([`DwarfCoreModel::member_speed_bound`]). A core that fails
//!   [`DwarfCoreModel::check_index_headroom`] is not played, as
//!   [`resolve_with`](crate::galaxy::placement::resolve_with) refuses it.
//! - **The centre**: plan 09's [`CentreMemberSource`] (P09.T29), asked as it would be on its own,
//!   since its members move on orbits that the shells' pad does not bound.
//!
//! Every member is tested at the sphere's time, drifting in a straight line from its epoch
//! position at its own velocity (`Drift::of_global_member`), against the unpadded radius, as a
//! feature member is; each counts in the layer of its band, with
//! [`SystemOrigin::GlobalListMember`](crate::galaxy::placement::SystemOrigin::GlobalListMember).
//! Sums run in a fixed order, the list's entries in list order, then a stream's cells in
//! [`TubeCell`](super::tube::TubeCell)'s order or a core's in its grid's, band by band, so the
//! census is a function of the galaxy and the sphere alone, whatever the lookup has cached.
//!
//! A caller registers it with the range query from generator version 27
//! ([`GlobalListSource::is_registered_in`], P10.T10.b's registration member, with plan 09's
//! P09.T37.b), in place of the [`CentreMemberSource`] it holds, not beside it, since their IDs
//! would repeat. From the same version the galaxy stores the halo's discrete share, so the halo's
//! field gives up what the list's members stand for in the train that first serves them
//! (P10.T3.d.b).

use std::borrow::Cow;
use std::num::NonZeroU32;
use std::sync::OnceLock;

#[cfg(doc)]
use crate::galaxy::features::interior::MemberClassTable;

use crate::GeneratorVersion;
use crate::coords::GalacticPosition;
use crate::galaxy::Galaxy;
use crate::galaxy::features::centre::{BuildCentreError, CentreMemberSource, CentreOrbitCache};
use crate::galaxy::features::members::local_offset;
use crate::galaxy::features::members::source::{band_reach, hit_at, member_reach, part_of_key};
use crate::galaxy::imf::MassBand;
use crate::galaxy::placement::{Existence, ResolveSystemError, SystemRecord};
use crate::galaxy::query::{
    LayerCounts, LayerSet, QuerySphere, RangeQuery, SystemHit, SystemSource, UNBOUND_PAD_SPEED,
    pad_for,
};
use crate::id::{SystemId, SystemIdKind};
use crate::time::UniverseTime;
use crate::units::{LightYears, Seconds};

use super::dwarf_core::{DwarfCoreModel, resolve_dwarf_core_member};
use super::members::{
    GlobalMemberRecord, StreamBandShares, expected_members, resolve_stream_member,
    stream_cell_members,
};
use super::{
    BoundingShell, GlobalEntryKind, GlobalList, StreamNumber, TubeLookup, TubeTable,
    in_stream_shell,
};

/// The generator version from which the range query registers the global list's members, the
/// centre's among them, and a galaxy stores its halo's discrete share (plan 10, P10.T10.b's
/// registration member and P10.T3.d.b, riding with plan 09's P09.T37.b; the hold of
/// [`ListSourceRules`]).
///
/// Below it every galaxy built by `Galaxy::from_params` takes the held arm,
/// [`ListSourceRules::Unregistered`]: [`GlobalListSource::is_registered_in`] says no, so a served
/// query's census and hits are the grid's, the features' members' and the catalogue classes', and
/// the halo's discrete share stays zero, bit for bit. The tests reach the new arm through a galaxy
/// built under `GalaxyRules::every_27_hold()`. The bump to 27 deletes this hold,
/// [`ListSourceRules`] with its `Unregistered` arm, `GalaxyRules::list_source`, the `match`es in
/// [`GlobalListSource::is_registered_in`] (which then always says yes) and in the galaxy's build
/// (which then always stores the share), `Galaxy::with_rules`' check of it and the tripwire
/// (`the_list_source_is_held_only_until_the_27_bump`).
pub(crate) const LIST_SOURCE_VERSION: GeneratorVersion = GeneratorVersion::new(27);

/// Whether the range query registers the global list's source and the galaxy stores the halo's
/// discrete share (the hold of [`LIST_SOURCE_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum ListSourceRules {
    /// Version 26's and before: no range query places a stream's, a dwarf core's or the centre's
    /// member, and the halo's field keeps its discrete share.
    Unregistered,
    /// Version 27's: [`GlobalListSource`] beside the grid, and the halo's field gives up
    /// [`discrete_share`](super::discrete_share).
    Registered,
}

/// The first word of a stream's tube-cell keys for the sky's member parts ([`part_of_key`]),
/// `"stream"` in ASCII.
const STREAM_PART_WORD: u64 = 0x0000_7374_7265_616d;

/// The first word of a dwarf core's nested-cell keys for the sky's member parts
/// ([`part_of_key`]), `"core"` in ASCII.
const CORE_PART_WORD: u64 = 0x0000_0000_636f_7265;

/// The bands from the coarsest down, the order the query walks its layers.
const BANDS_COARSEST_FIRST: [MassBand; 5] = [
    MassBand::E,
    MassBand::D,
    MassBand::C,
    MassBand::B,
    MassBand::A,
];

/// What a global list's source reads of its galaxy that no query changes: each dwarf core's model,
/// and each stream's band shares once a query first reaches it (P09.T37's bench).
///
/// Both are functions of the galaxy and its list alone (a dwarf stream's shares read its core's
/// keyed metallicity stream), so which query makes a stream's shares first moves nothing, and a
/// caller may keep one for every query of the galaxy, lent through its [`TubeLookup::models`]. Made
/// for each source instead, the cores' models took 3.8 ms and the 121 streams' shares some 9.5 ms
/// of every Sun-like 50 ly query under every 27 hold, warm or cold (measured 2026-11-01, pod, 16
/// vCPU).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GlobalListModels {
    /// Each dwarf core's model in number order, `None` for one that fails its headroom check.
    cores: Vec<Option<DwarfCoreModel>>,
    /// Each stream's band shares in number order, made when a query first reaches it.
    shares: Vec<OnceLock<StreamBandShares>>,
}

impl GlobalListModels {
    /// The models of `list`, `galaxy`'s global list: every dwarf core's
    /// ([`DwarfCoreModel::from_spec`]), and no stream's shares until a query reaches it.
    ///
    /// The cores' models of the server tests' galaxy take 3.8 ms together (measured 2026-11-01,
    /// pod, 16 vCPU), so a server builds them off its runtime.
    ///
    /// # Panics
    ///
    /// As [`DwarfCoreModel::from_spec`], for a core no [`Debris`](super::Debris) plans.
    #[must_use]
    pub fn new(galaxy: &Galaxy, list: &GlobalList) -> Self {
        let cores = list
            .debris()
            .cores()
            .iter()
            .map(|spec| {
                let model = DwarfCoreModel::from_spec(galaxy, spec);
                model.check_index_headroom().is_ok().then_some(model)
            })
            .collect();
        let shares = list
            .debris()
            .streams()
            .iter()
            .map(|_| OnceLock::new())
            .collect();
        Self { cores, shares }
    }

    /// The bytes it owns on the heap, by its buffers' capacities and each core's class table's
    /// ([`MemberClassTable::heap_bytes`]): what a byte-bounded cache charges it.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.cores.capacity() * size_of::<Option<DwarfCoreModel>>()
            + self
                .cores
                .iter()
                .flatten()
                .map(|model| model.table().heap_bytes())
                .sum::<usize>()
            + self.shares.capacity() * size_of::<OnceLock<StreamBandShares>>()
    }

    /// Whether these are the models of `list`'s debris, by its counts of cores and streams.
    #[must_use]
    fn fit(&self, list: &GlobalList) -> bool {
        self.cores.len() == list.debris().cores().len()
            && self.shares.len() == list.debris().streams().len()
    }
}

/// The global list's members as a source of the range query (module documentation).
///
/// It holds the galaxy it is made for, the caller's [`TubeLookup`], whose list must be that
/// galaxy's, the centre's source and the list's models ([`GlobalListModels`]): the lookup's if it
/// lends them, otherwise its own. A caller registers it with the range query in place of the
/// [`CentreMemberSource`] it holds.
///
/// # Panics
///
/// Its [`SystemSource`] methods panic if asked about another galaxy than the one it was made for.
///
/// # Examples
///
/// A query of 50 ly in the Sun's neighbourhood, whose census takes the streams' shells first:
///
/// ```no_run
/// use std::collections::BTreeMap;
/// use std::sync::{Arc, Mutex};
///
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::features::centre::NoCentreOrbitCache;
/// use hyperion_sim::galaxy::global_list::{
///     GlobalList, GlobalListSource, StreamNumber, TubeLookup, TubeTable,
/// };
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::galaxy::placement::NoCache;
/// use hyperion_sim::galaxy::query::{RangeQuery, range_query};
/// use hyperion_sim::units::LightYears;
///
/// /// A lookup that keeps every table it builds.
/// struct Keep<'a> {
///     galaxy: &'a Galaxy,
///     list: GlobalList,
///     tubes: Mutex<BTreeMap<StreamNumber, Arc<TubeTable>>>,
/// }
///
/// impl TubeLookup for Keep<'_> {
///     fn global_list(&self) -> &GlobalList {
///         &self.list
///     }
///
///     fn tube(&self, stream: StreamNumber) -> Arc<TubeTable> {
///         let spec = self.list.stream(stream).expect("a stream of the list");
///         let mut tubes = self.tubes.lock().expect("not poisoned");
///         Arc::clone(tubes.entry(stream).or_insert_with(|| {
///             Arc::new(TubeTable::build(self.galaxy, spec).expect("the full potential"))
///         }))
///     }
/// }
///
/// let galaxy = Galaxy::from_params(Seed::new(7), GalaxyParams::milky_way_like())?
///     .with_full_potential();
/// let lookup = Keep { galaxy: &galaxy, list: GlobalList::build(&galaxy), tubes: Mutex::default() };
/// let source = GlobalListSource::new(&galaxy, &lookup, &NoCentreOrbitCache)?;
/// let here = GalacticPosition::from_light_years([26_000.0, 0.0, 20.0]).ok_or("out of range")?;
/// let query = RangeQuery::builder(here, LightYears::new(50.0)).build()?;
/// let result = range_query(&galaxy, &mut NoCache::new(), &[&source], &query);
/// # let _ = result;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone)]
pub struct GlobalListSource<'a> {
    galaxy: &'a Galaxy,
    lookup: &'a dyn TubeLookup,
    /// The centre's source, if the list holds the centre.
    centre: Option<CentreMemberSource<'a>>,
    /// The cores' models and the streams' band shares: the lookup's, or the source's own.
    models: Cow<'a, GlobalListModels>,
}

impl std::fmt::Debug for GlobalListSource<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GlobalListSource").finish_non_exhaustive()
    }
}

impl<'a> GlobalListSource<'a> {
    /// The source of `galaxy`'s global list, `lookup`'s, whose tube tables it reads through
    /// `lookup` and the centre's cells through `orbits`: the centre's source if the list holds the
    /// centre ([`CentreMemberSource::new`]), and the list's models, the lookup's
    /// ([`TubeLookup::models`]) or else its own ([`GlobalListModels::new`]).
    ///
    /// # Errors
    ///
    /// The [`BuildCentreError`] of a centre that cannot be built, which no galaxy the builder
    /// allows has ([`Galaxy::centre`]).
    ///
    /// # Panics
    ///
    /// As [`CentreMemberSource::new`], which no galaxy the builder allows reaches, and if the
    /// lookup lends models of another list than its own, by their counts of cores and streams.
    pub fn new(
        galaxy: &'a Galaxy,
        lookup: &'a dyn TubeLookup,
        orbits: &'a dyn CentreOrbitCache,
    ) -> Result<Self, BuildCentreError> {
        let list = lookup.global_list();
        let centre = list
            .kinds()
            .any(|kind| kind == GlobalEntryKind::Centre)
            .then(|| CentreMemberSource::new(galaxy, orbits))
            .transpose()?;
        let models = lookup.models().map_or_else(
            || Cow::Owned(GlobalListModels::new(galaxy, list)),
            Cow::Borrowed,
        );
        assert!(
            models.fit(list),
            "a lookup lends the models of its own global list"
        );
        Ok(Self {
            galaxy,
            lookup,
            centre,
            models,
        })
    }

    /// Whether a range query in `galaxy` registers this source beside the grid, in place of the
    /// centre's bare source: from generator version 27 (P10.T10.b's registration member, the hold
    /// of `LIST_SOURCE_VERSION`), and not below it, where a query takes no member of a stream, a
    /// dwarf core or the centre.
    #[must_use]
    pub const fn is_registered_in(galaxy: &Galaxy) -> bool {
        match galaxy.rules().list_source {
            ListSourceRules::Unregistered => false,
            ListSourceRules::Registered => true,
        }
    }

    /// The centre's source it holds and asks, if the list holds the centre.
    #[must_use]
    pub fn centre(&self) -> Option<&CentreMemberSource<'a>> {
        self.centre.as_ref()
    }

    /// The stream's or dwarf core's member `id` names, with what converting one of the source's
    /// hits needs beside its record: its class, composition and velocity. It is
    /// [`resolve_list_member_with`](crate::galaxy::placement::resolve_list_member_with)'s member,
    /// read through the source's lookup, band shares and cores' models rather than the galaxy's
    /// debris planned again.
    ///
    /// # Errors
    ///
    /// [`ResolveSystemError::KindNotGenerated`] for an ID of neither kind, and
    /// [`ResolveSystemError::NoSuchSystem`] for a stream or core the list lacks or does not play,
    /// and as [`resolve_stream_member`] and [`resolve_dwarf_core_member`] for a member, a stream's
    /// outside its shell among them under the galaxy's rules.
    ///
    /// # Panics
    ///
    /// If `galaxy` is not the source's own, and as the lookup's [`TubeLookup::tube`].
    pub fn member(
        &self,
        galaxy: &Galaxy,
        id: SystemId,
    ) -> Result<GlobalMemberRecord, ResolveSystemError> {
        self.check(galaxy);
        match id.kind() {
            SystemIdKind::Stream(member) => {
                let number = StreamNumber::from(member);
                let entry = self
                    .list()
                    .stream_entry(number)
                    .ok_or(ResolveSystemError::NoSuchSystem)?;
                let tube = self.lookup.tube(number);
                let found = resolve_stream_member(galaxy, &tube, self.shares(number), member)?;
                if in_stream_shell(&entry.shell(), found.record().epoch_position()) {
                    Ok(found)
                } else {
                    Err(ResolveSystemError::NoSuchSystem)
                }
            }
            SystemIdKind::DwarfCore(member) => {
                let core = self
                    .models
                    .cores
                    .get(usize::from(member.number()))
                    .and_then(Option::as_ref)
                    .ok_or(ResolveSystemError::NoSuchSystem)?;
                resolve_dwarf_core_member(galaxy, core, member)
            }
            SystemIdKind::Grid(_)
            | SystemIdKind::FeatureMember(_)
            | SystemIdKind::Centre(_)
            | SystemIdKind::Pinned(_)
            | SystemIdKind::Catalogue(_) => Err(ResolveSystemError::KindNotGenerated),
        }
    }

    /// Panics unless `galaxy` is the source's own.
    fn check(&self, galaxy: &Galaxy) {
        assert!(
            std::ptr::eq(galaxy, self.galaxy),
            "a global list source is for the galaxy it was made for"
        );
    }

    /// The list.
    #[must_use]
    fn list(&self) -> &GlobalList {
        self.lookup.global_list()
    }

    /// Stream `number`'s band shares, made on first use.
    ///
    /// # Panics
    ///
    /// If the list has no stream `number`.
    #[must_use]
    fn shares(&self, number: StreamNumber) -> &StreamBandShares {
        let index = usize::from(number.get());
        self.models.shares[index].get_or_init(|| {
            let spec = self.list().stream(number).expect("a stream of the list");
            StreamBandShares::of(self.galaxy, spec)
        })
    }

    /// The streams and the cores whose shells `sphere`, out to its padded radius, meets, in list
    /// order, each with its shell (module documentation).
    fn entries_near<'s>(
        &'s self,
        sphere: &'s QuerySphere,
    ) -> impl Iterator<Item = (GlobalEntryKind, BoundingShell)> + 's {
        entries_near(self.galaxy, self.list(), sphere)
    }

    /// The streams of `list`, `galaxy`'s global list, whose tube tables the source reads to answer
    /// a range query of `query`, in number order: those whose shells its sphere meets at its time,
    /// padded at [`UNBOUND_PAD_SPEED`] as its hits are, which holds every stream its census at the
    /// epoch reads too.
    ///
    /// A caller that builds tables off the query's own thread, as a server's pool does, builds
    /// these before it asks, so that the query reads no table cold.
    ///
    /// # Panics
    ///
    /// Never: a built query's radius is finite and positive, and its pad a few light-years.
    #[must_use]
    pub fn streams_read(
        galaxy: &Galaxy,
        list: &GlobalList,
        query: &RangeQuery,
    ) -> Vec<StreamNumber> {
        let padded = QuerySphere::new(
            *query.centre(),
            query.radius(),
            query.time(),
            pad_for(query.time(), UNBOUND_PAD_SPEED),
        )
        .expect("a built query's radius is finite and positive, and the pad a few light-years");
        entries_near(galaxy, list, &padded)
            .filter_map(|(kind, _)| match kind {
                GlobalEntryKind::Stream(number) => Some(number),
                GlobalEntryKind::DwarfCore(_) | GlobalEntryKind::Centre => None,
            })
            .collect()
    }

    /// Each stream's and dwarf core's member of `band` that may lie within `outer` of `centre` at
    /// some time of `window`, born by its end, given to `each` in the source's order: the list's
    /// entries in list order, a stream's tube cells or a core's nested cells, those of the part
    /// `part` of `parts` by their key ([`part_of_key`]), then members in index order (rendering
    /// plan R06, R06.T16.a). Each entry is padded by its own members' speed bound over the
    /// window's farthest time from the epoch, as a feature is, and the shells are tested at the
    /// unbound pad. It may give a member beyond the shell, never leave one out. The centre's
    /// members are its [`CentreMemberSource`]'s.
    ///
    /// # Panics
    ///
    /// If `outer` is not finite and positive, or `galaxy` is not the source's.
    pub(crate) fn members_in_shell_over(
        &self,
        galaxy: &Galaxy,
        centre: &GalacticPosition,
        window: (UniverseTime, UniverseTime),
        ((band, outer), (part, parts)): ((MassBand, LightYears), (u32, NonZeroU32)),
        each: &mut dyn FnMut(&GlobalMemberRecord),
    ) {
        self.check(galaxy);
        let to = window.1;
        let (padded, farthest) = seen_sphere(centre, window, outer);
        let band_word = u64::try_from(band.index()).expect("a band's index is below seven");
        let mut give = |member: &GlobalMemberRecord| {
            if member.record().existence_at(to) == Existence::Exists {
                each(member);
            }
        };
        let (mut members, mut core_members) = (Vec::new(), Vec::new());
        for (kind, shell) in self.entries_near(&padded) {
            match kind {
                GlobalEntryKind::Stream(number) => {
                    let tube = self.lookup.tube(number);
                    let shares = self.shares(number);
                    let within = member_reach(outer, farthest, tube.member_speed_bound());
                    for cell in tube.cells_touching(centre, within) {
                        let [a, b] = cell.across();
                        let key = [
                            STREAM_PART_WORD,
                            u64::from(number.get()),
                            u64::from(cell.along()),
                            u64::from(a),
                            u64::from(b),
                            band_word,
                        ];
                        if part_of_key(&key, parts) != part {
                            continue;
                        }
                        stream_cell_members(galaxy, &tube, shares, cell, band, &mut members);
                        for member in &members {
                            if in_stream_shell(&shell, member.record().epoch_position()) {
                                give(member);
                            }
                        }
                    }
                }
                GlobalEntryKind::DwarfCore(number) => {
                    let Some(core) = self.core(number) else {
                        continue;
                    };
                    let within = member_reach(outer, farthest, core.member_speed_bound());
                    let local = local_offset(&core_centre(core), centre);
                    for cell in core.grid().cells_touching(&local, within) {
                        let [x, y, z] = cell.cell();
                        let key = [
                            CORE_PART_WORD,
                            u64::from(number.get()),
                            band_word,
                            u64::from(cell.level()),
                            u64::from(x),
                            u64::from(y),
                            u64::from(z),
                        ];
                        if part_of_key(&key, parts) != part {
                            continue;
                        }
                        core.members_in_cell(galaxy, band, cell, &mut core_members);
                        for (member, _) in &core_members {
                            give(member);
                        }
                    }
                }
                GlobalEntryKind::Centre => {}
            }
        }
    }

    /// The streams of `list`, `galaxy`'s, whose tube tables the sky's census reads for a sphere of
    /// `outer` about `centre` over `window` (its [`SkySource`](crate::sky::census::SkySource) walk
    /// tests the same shells), in list order: so that a server can build them first, as
    /// [`streams_read`](Self::streams_read) lets it for a range query (rendering plan R06,
    /// R06.T16.a).
    ///
    /// # Panics
    ///
    /// If `outer` is not finite and positive.
    #[must_use]
    pub fn streams_seen(
        galaxy: &Galaxy,
        list: &GlobalList,
        centre: &GalacticPosition,
        window: (UniverseTime, UniverseTime),
        outer: LightYears,
    ) -> Vec<StreamNumber> {
        let (padded, _) = seen_sphere(centre, window, outer);
        entries_near(galaxy, list, &padded)
            .filter_map(|(kind, _)| match kind {
                GlobalEntryKind::Stream(number) => Some(number),
                GlobalEntryKind::DwarfCore(_) | GlobalEntryKind::Centre => None,
            })
            .collect()
    }

    /// Core `number`'s model, if it is played.
    #[must_use]
    fn core(&self, number: super::DwarfCoreNumber) -> Option<&DwarfCoreModel> {
        self.models
            .cores
            .get(usize::from(number.get()))
            .and_then(Option::as_ref)
    }
}

impl SystemSource for GlobalListSource<'_> {
    fn expected_in_sphere(&self, galaxy: &Galaxy, sphere: &QuerySphere) -> LayerCounts {
        self.check(galaxy);
        let mut counts = self
            .centre
            .as_ref()
            .map_or(LayerCounts::ZERO, |c| c.expected_in_sphere(galaxy, sphere));
        let radius = sphere.radius();
        let at_epoch = QuerySphere::new(
            *sphere.centre(),
            radius,
            UniverseTime::EPOCH,
            LightYears::ZERO,
        )
        .expect("a query sphere's radius is finite and positive");
        let mut add = |band: MassBand, expected: f64| {
            let layer = band.layer();
            counts.set(layer, counts.get(layer) + expected);
        };
        let (mut cells, mut core_cells) = (Vec::new(), Vec::new());
        for (kind, _) in self.entries_near(&at_epoch) {
            match kind {
                GlobalEntryKind::Stream(number) => {
                    let tube = self.lookup.tube(number);
                    let shares = self.shares(number);
                    cells.clear();
                    cells.extend(tube.cells_touching(sphere.centre(), radius));
                    for band in BANDS_COARSEST_FIRST {
                        let expected = cells.iter().fold(0.0, |sum, &cell| {
                            sum + expected_members(&tube, shares, cell, band)
                        });
                        add(band, expected);
                    }
                }
                GlobalEntryKind::DwarfCore(number) => {
                    let Some(core) = self.core(number) else {
                        continue;
                    };
                    let local = local_offset(&core_centre(core), sphere.centre());
                    core_cells.clear();
                    core_cells.extend(core.grid().cells_touching(&local, radius));
                    for band in BANDS_COARSEST_FIRST {
                        let expected = core_cells
                            .iter()
                            .fold(0.0, |sum, &cell| sum + core.expected_candidates(band, cell));
                        add(band, expected);
                    }
                }
                GlobalEntryKind::Centre => {}
            }
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
        self.check(galaxy);
        if let Some(centre) = &self.centre {
            centre.systems_in_sphere(galaxy, sphere, layers, out);
        }
        let t = sphere.time();
        let elapsed = Seconds::new(t.since_epoch().as_seconds_f64());
        let radius = sphere.radius();
        // Every stream's and core's members move below this (asserted in debug builds), so one
        // that can reach the sphere by `t` lies in an entry whose shell the sphere meets padded so.
        let padded = QuerySphere::new(*sphere.centre(), radius, t, pad_for(t, UNBOUND_PAD_SPEED))
            .expect(
                "a query sphere's radius is finite and positive, and the pad a few light-years",
            );
        let hit = |member: &GlobalMemberRecord, out: &mut Vec<SystemHit>| {
            if let Some(h) = hit_at(member.record(), member.velocity(), sphere, elapsed) {
                out.push(h);
            }
        };
        let (mut members, mut core_members) = (Vec::new(), Vec::new());
        for (kind, shell) in self.entries_near(&padded) {
            match kind {
                GlobalEntryKind::Stream(number) => {
                    let tube = self.lookup.tube(number);
                    let shares = self.shares(number);
                    let query = StreamQuery {
                        shell: &shell,
                        sphere,
                        layers,
                    };
                    stream_hits(galaxy, &tube, shares, &query, &mut members, &mut |m| {
                        hit(m, out);
                    });
                }
                GlobalEntryKind::DwarfCore(number) => {
                    let Some(core) = self.core(number) else {
                        continue;
                    };
                    let bound = core.member_speed_bound();
                    debug_assert!(
                        bound.value() < UNBOUND_PAD_SPEED.value(),
                        "dwarf core {}'s members move up to {} km/s",
                        number.get(),
                        bound.value()
                    );
                    let local = local_offset(&core_centre(core), sphere.centre());
                    for band in BANDS_COARSEST_FIRST {
                        if !layers.contains(band.layer()) {
                            continue;
                        }
                        let reach = band_reach(galaxy, radius, t, band.layer(), bound);
                        for cell in core.grid().cells_touching(&local, reach) {
                            core.members_in_cell(galaxy, band, cell, &mut core_members);
                            for (member, _) in &core_members {
                                hit(member, out);
                            }
                        }
                    }
                }
                GlobalEntryKind::Centre => {}
            }
        }
    }

    fn suppresses(&self, galaxy: &Galaxy, _record: &SystemRecord, _t: UniverseTime) -> bool {
        self.check(galaxy);
        false
    }
}

/// The sphere of `outer` about `centre` at the farthest time of `window` from the epoch, padded at
/// [`UNBOUND_PAD_SPEED`] to it, against which the sky's census tests the list's shells, and that
/// time.
///
/// # Panics
///
/// If `outer` is not finite and positive.
#[must_use]
fn seen_sphere(
    centre: &GalacticPosition,
    (from, to): (UniverseTime, UniverseTime),
    outer: LightYears,
) -> (QuerySphere, UniverseTime) {
    let farthest =
        if from.since_epoch().as_seconds_f64().abs() > to.since_epoch().as_seconds_f64().abs() {
            from
        } else {
            to
        };
    let padded = QuerySphere::new(
        *centre,
        outer,
        farthest,
        pad_for(farthest, UNBOUND_PAD_SPEED),
    )
    .expect("a shell's outer distance is finite and positive, and the pad a few light-years");
    (padded, farthest)
}

/// The streams and the cores of `list`, `galaxy`'s, whose shells `sphere`, out to its padded
/// radius, meets, in list order, each with its shell ([`GlobalListSource::entries_near`]).
fn entries_near<'s>(
    _galaxy: &Galaxy,
    list: &'s GlobalList,
    sphere: &'s QuerySphere,
) -> impl Iterator<Item = (GlobalEntryKind, BoundingShell)> + 's {
    list.entries()
        .iter()
        .filter(move |e| match e.kind() {
            GlobalEntryKind::Centre => false,
            GlobalEntryKind::Stream(_) | GlobalEntryKind::DwarfCore(_) => e.shell().touches(sphere),
        })
        .map(|e| (e.kind(), e.shell()))
}

/// What [`stream_hits`] asks of a stream: its shell, the query's sphere and the layers asked.
struct StreamQuery<'q> {
    shell: &'q BoundingShell,
    sphere: &'q QuerySphere,
    layers: LayerSet,
}

/// Calls `visit` with every member of `tube`'s stream, whose band shares are `shares`, inside
/// `query`'s shell (under the galaxy's rules) and in its layers' bands, in the cells the members
/// that can reach its sphere by its time lie in (module documentation): band by band, cell by
/// cell, each cell's members in index order. `members` is the caller's buffer.
///
/// # Panics
///
/// In debug builds, if the tube's members can move at [`UNBOUND_PAD_SPEED`] or faster.
fn stream_hits(
    galaxy: &Galaxy,
    tube: &TubeTable,
    shares: &StreamBandShares,
    query: &StreamQuery<'_>,
    members: &mut Vec<GlobalMemberRecord>,
    visit: &mut dyn FnMut(&GlobalMemberRecord),
) {
    let StreamQuery {
        shell,
        sphere,
        layers,
    } = *query;
    let t = sphere.time();
    let bound = tube.member_speed_bound();
    debug_assert!(
        bound.value() < UNBOUND_PAD_SPEED.value(),
        "stream {}'s members move up to {} km/s",
        tube.stream().get(),
        bound.value()
    );
    for band in BANDS_COARSEST_FIRST {
        let layer = band.layer();
        if !layers.contains(layer) {
            continue;
        }
        let reach = band_reach(galaxy, sphere.radius(), t, layer, bound);
        for cell in tube.cells_touching(sphere.centre(), reach) {
            stream_cell_members(galaxy, tube, shares, cell, band, members);
            let inside = members
                .iter()
                .filter(|m| in_stream_shell(shell, m.record().epoch_position()));
            for member in inside {
                visit(member);
            }
        }
    }
}

/// A core's centre at the epoch, in the galactic frame.
///
/// # Panics
///
/// Never: a core's centre lies in the root cube (`DwarfCoreModel::feature_level_list`'s
/// documentation).
#[must_use]
fn core_centre(core: &DwarfCoreModel) -> GalacticPosition {
    GalacticPosition::ORIGIN
        .translated(core.position())
        .expect("a core in the root cube is addressable")
}

#[cfg(test)]
mod tests;
