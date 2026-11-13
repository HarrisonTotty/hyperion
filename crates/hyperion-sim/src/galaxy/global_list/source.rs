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
//!   census is the exact sum of the means
//!   ([`expected_members`](super::members::expected_members)) of the cells touching the
//!   unpadded radius, band by band, which errs high, as [`SystemSource`]'s contract allows, since
//!   a cell can touch the sphere and hold no member inside it, and a member can lie outside the
//!   root cube or, from 26, its shell. The hits take the cells touching the radius padded band by
//!   band at the larger of the band's layer's [`pad_speed`](crate::galaxy::query::pad_speed) and
//!   the tube's own members' bound ([`TubeTable::member_speed_bound`]), plan 09's rule for a
//!   feature (P09.T23.b). A stream whose [`TubeEnvelope`] the list's models keep, and the sphere
//!   misses out to where its cells are read, yields no cell, and its table is not read
//!   ([`GlobalListModels`]).
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
//! field gives up what the list's members stand for (P10.T3.d.b).

use std::borrow::Cow;
use std::num::NonZeroU32;
use std::sync::{Arc, OnceLock};

#[cfg(doc)]
use crate::galaxy::features::interior::MemberClassTable;

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
use crate::units::{KilometresPerSecond, LightYears, Seconds};

use super::dwarf_core::{DwarfCoreModel, resolve_dwarf_core_member};
use super::members::{
    GlobalMemberRecord, StreamBandShares, expected_touching, resolve_stream_member,
    stream_cell_members,
};
use super::{
    BoundingShell, GlobalEntryKind, GlobalList, StreamNumber, TubeEnvelope, TubeLookup, TubeTable,
    in_stream_shell,
};

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
/// each stream's band shares once a query first reaches it (P09.T37's bench), and each stream's
/// tube envelope once its table is first read ([`TubeEnvelope`]).
///
/// All are functions of the galaxy and its list alone (a dwarf stream's shares read its core's
/// keyed metallicity stream, and an envelope is its table's, which is the galaxy's and the
/// stream's), so which query makes one first moves nothing, and a caller may keep one for every
/// query of the galaxy, lent through its [`TubeLookup::models`]. Made for each source instead, the
/// cores' models took 3.8 ms and the 121 streams' shares some 9.5 ms of every Sun-like 50 ly query
/// under every 27 hold, warm or cold (measured 2026-11-01, pod, 16 vCPU). A source passes by the
/// table of a stream whose envelope it holds and a sphere misses, so a caller that keeps the
/// envelopes when its tables are evicted builds none of those again (perf-range-census-streams).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GlobalListModels {
    /// Each dwarf core's model in number order, `None` for one that fails its headroom check.
    cores: Vec<Option<DwarfCoreModel>>,
    /// Each stream's band shares in number order, made when a query first reaches it.
    shares: Vec<OnceLock<StreamBandShares>>,
    /// Each stream's tube envelope in number order, kept when its table is first read.
    envelopes: Vec<OnceLock<TubeEnvelope>>,
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
        let streams = list.debris().streams();
        let shares = streams.iter().map(|_| OnceLock::new()).collect();
        let envelopes = streams.iter().map(|_| OnceLock::new()).collect();
        Self {
            cores,
            shares,
            envelopes,
        }
    }

    /// The bytes it owns on the heap, by its buffers' capacities and each core's class table's
    /// ([`MemberClassTable::heap_bytes`]): what a byte-bounded cache charges it. A stream's shares
    /// and envelope are held in place, so the count is the same before and after they are made:
    /// about 2.1 kB a stream, 683 kB for the server tests' galaxy's 321 (measured 2026-11-05).
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
            + self.envelopes.capacity() * size_of::<OnceLock<TubeEnvelope>>()
    }

    /// The envelope of stream `number`'s tube, if its table has been read through these models
    /// or kept with [`keep_envelope`](Self::keep_envelope).
    #[must_use]
    pub fn envelope(&self, number: StreamNumber) -> Option<&TubeEnvelope> {
        self.envelopes
            .get(usize::from(number.get()))
            .and_then(OnceLock::get)
    }

    /// Keeps the envelope of `table`, the tube of a stream of the list these are the models of,
    /// unless one is kept already: for a caller that builds a table outside a source, as a
    /// server's warm-up does, so that a query need not read it again. A table of a stream the list
    /// lacks is passed by; one of another galaxy's list would make a source pass by streams it
    /// should read, and is the caller's to keep out.
    pub fn keep_envelope(&self, table: &TubeTable) {
        if let Some(kept) = self.envelopes.get(usize::from(table.stream().get())) {
            kept.get_or_init(|| TubeEnvelope::of(table));
        }
    }

    /// Keeps `envelope` as stream `number`'s, unless one is kept already.
    ///
    /// For a caller that has rebuilt it from its stored parts ([`TubeEnvelope::from_parts`]), as
    /// a server reads a galaxy's from disk when it builds the list again after a restart, so that
    /// a cold query builds only the tables it may touch (perf-range-envelopes-on-disk). A number
    /// the list lacks is passed by. It must be the envelope of that stream's table in the galaxy of
    /// the list these are the models of, which the caller's keys keep apart: another's would make
    /// a source pass by a stream it should read.
    pub fn keep_stored_envelope(&self, number: StreamNumber, envelope: TubeEnvelope) {
        if let Some(kept) = self.envelopes.get(usize::from(number.get())) {
            // One kept already is the same table's envelope, or the caller's keys are wrong.
            let _ = kept.set(envelope);
        }
    }

    /// Whether these are the models of `list`'s debris, by its counts of cores and streams.
    #[must_use]
    fn fit(&self, list: &GlobalList) -> bool {
        self.cores.len() == list.debris().cores().len()
            && self.shares.len() == list.debris().streams().len()
            && self.envelopes.len() == list.debris().streams().len()
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
    /// centre's bare source: in every galaxy from generator version 27 (P10.T10.b's registration
    /// member).
    #[must_use]
    pub const fn is_registered_in(_galaxy: &Galaxy) -> bool {
        true
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

    /// Stream `number`'s table, read through the lookup with its envelope kept, or `None` if its
    /// envelope is kept already and misses `reach` about `centre`, where the table yields no cell
    /// ([`TubeEnvelope::may_touch`]).
    fn tube_near(
        &self,
        number: StreamNumber,
        centre: &GalacticPosition,
        reach: LightYears,
    ) -> Option<Arc<TubeTable>> {
        if let Some(envelope) = self.models.envelope(number)
            && !envelope.may_touch(centre, reach)
        {
            return None;
        }
        let tube = self.lookup.tube(number);
        self.models.keep_envelope(&tube);
        Some(tube)
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

    /// The streams of [`streams_read`](Self::streams_read) whose tables a source reads for `query`
    /// through `models`, `list`'s: those of them whose envelopes `models` does not keep, and those
    /// whose envelopes the query's sphere meets out to the farthest any stellar band's cells are
    /// read ([`TubeEnvelope`]).
    ///
    /// A caller that builds tables off the query's thread and keeps the envelopes beside the list
    /// builds these, so that the query reads no table cold and no table is built for a stream that
    /// holds nothing there. The Sun-like 50 ly query of the server tests' galaxy meets 121 streams'
    /// shells and touches none of their tubes (measured 2026-11-05, pod, 16 vCPU).
    ///
    /// # Panics
    ///
    /// As [`streams_read`](Self::streams_read), and if `models` are not `list`'s by their counts.
    ///
    /// # Examples
    ///
    /// Not run: the tables need the full potential and sprays of some seconds each.
    ///
    /// ```no_run
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::coords::GalacticPosition;
    /// use hyperion_sim::galaxy::Galaxy;
    /// use hyperion_sim::galaxy::global_list::{
    ///     GlobalList, GlobalListModels, GlobalListSource, TubeTable,
    /// };
    /// use hyperion_sim::galaxy::query::RangeQuery;
    /// use hyperion_sim::units::LightYears;
    ///
    /// let galaxy = Galaxy::new(Seed::new(0x4d2)).with_full_potential();
    /// let list = GlobalList::build(&galaxy);
    /// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).ok_or("out of range")?;
    /// let query = RangeQuery::builder(sun, LightYears::new(50.0)).build()?;
    /// // With no envelope kept, every stream whose shell the query meets: 121 of them.
    /// let models = GlobalListModels::new(&galaxy, &list);
    /// let shelled = GlobalListSource::streams_read(&galaxy, &list, &query);
    /// assert_eq!(GlobalListSource::streams_read_with(&galaxy, &list, &models, &query), shelled);
    /// // Once their tables have been built, as a server's warm-up builds them, and their envelopes
    /// // kept, only those the query may touch: 10 of them.
    /// for &number in &shelled {
    ///     let spec = list.stream(number).ok_or("a stream of the list")?;
    ///     models.keep_envelope(&TubeTable::build(&galaxy, spec)?);
    /// }
    /// let read = GlobalListSource::streams_read_with(&galaxy, &list, &models, &query);
    /// assert!(read.len() < shelled.len() && read.iter().all(|n| shelled.contains(n)));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn streams_read_with(
        galaxy: &Galaxy,
        list: &GlobalList,
        models: &GlobalListModels,
        query: &RangeQuery,
    ) -> Vec<StreamNumber> {
        assert!(models.fit(list), "the models of the list");
        let mut read = Self::streams_read(galaxy, list, query);
        read.retain(|&number| {
            models.envelope(number).is_none_or(|envelope| {
                let bound = envelope.member_speed_bound();
                let reach =
                    farthest_reach(query.radius(), query.time(), BANDS_COARSEST_FIRST, bound)
                        .expect("the stellar bands are five");
                envelope.may_touch(query.centre(), reach)
            })
        });
        read
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
                    if let Some(envelope) = self.models.envelope(number) {
                        let within = member_reach(outer, farthest, envelope.member_speed_bound());
                        if !envelope.may_touch(centre, within) {
                            continue;
                        }
                    }
                    let tube = self.lookup.tube(number);
                    self.models.keep_envelope(&tube);
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

    /// The streams of [`streams_seen`](Self::streams_seen) whose tables the sky's census reads
    /// through `models`, `list`'s, in list order: those whose envelopes `models` does not keep, and
    /// those whose envelopes the sphere meets out to where the census reads their cells, padded by
    /// their own members' speed bound over the window's farthest time from the epoch
    /// ([`TubeEnvelope`]), as [`streams_read_with`](Self::streams_read_with) names a range query's.
    ///
    /// A server builds these before a sky's members' jobs, so that it builds no table for a stream
    /// whose kept envelope the sky's reach misses, even one its cache has evicted
    /// (perf-range-cold-tables). The census reads none outside them: it passes by the same
    /// streams, and an envelope once kept is kept.
    ///
    /// # Panics
    ///
    /// As [`streams_seen`](Self::streams_seen), and if `models` are not `list`'s by their counts.
    #[must_use]
    pub fn streams_seen_with(
        galaxy: &Galaxy,
        list: &GlobalList,
        models: &GlobalListModels,
        centre: &GalacticPosition,
        window: (UniverseTime, UniverseTime),
        outer: LightYears,
    ) -> Vec<StreamNumber> {
        assert!(models.fit(list), "the models of the list");
        let (_, farthest) = seen_sphere(centre, window, outer);
        let mut seen = Self::streams_seen(galaxy, list, centre, window, outer);
        seen.retain(|&number| {
            models.envelope(number).is_none_or(|envelope| {
                let within = member_reach(outer, farthest, envelope.member_speed_bound());
                envelope.may_touch(centre, within)
            })
        });
        seen
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
        let mut core_cells = Vec::new();
        for (kind, _) in self.entries_near(&at_epoch) {
            match kind {
                GlobalEntryKind::Stream(number) => {
                    // A stream whose table yields no cell there sums no mean, zero in each band.
                    let expected =
                        self.tube_near(number, sphere.centre(), radius)
                            .map_or([0.0; 5], |tube| {
                                let shares = self.shares(number);
                                let sphere = (sphere.centre(), radius);
                                expected_touching(&tube, shares, sphere, BANDS_COARSEST_FIRST)
                            });
                    for (band, expected) in BANDS_COARSEST_FIRST.into_iter().zip(expected) {
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
                    if let Some(envelope) = self.models.envelope(number) {
                        let bands = BANDS_COARSEST_FIRST
                            .into_iter()
                            .filter(|band| layers.contains(band.layer()));
                        let reach = farthest_reach(radius, t, bands, envelope.member_speed_bound());
                        if !reach.is_some_and(|reach| envelope.may_touch(sphere.centre(), reach)) {
                            continue;
                        }
                    }
                    let tube = self.lookup.tube(number);
                    self.models.keep_envelope(&tube);
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
                        let reach = band_reach(radius, t, band.layer(), bound);
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

/// The farthest from a sphere's centre that [`stream_hits`] reads a stream's cells for `bands`, for
/// a sphere of `radius` at `t` and members below `bound`: the largest of their [`band_reach`]es,
/// none of which is below `radius`, or `None` for no band.
#[must_use]
fn farthest_reach(
    radius: LightYears,
    t: UniverseTime,
    bands: impl IntoIterator<Item = MassBand>,
    bound: KilometresPerSecond,
) -> Option<LightYears> {
    bands
        .into_iter()
        .map(|band| band_reach(radius, t, band.layer(), bound))
        .reduce(|far, reach| {
            if reach.value() > far.value() {
                reach
            } else {
                far
            }
        })
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
        let reach = band_reach(sphere.radius(), t, layer, bound);
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
