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
//!   from version 26 (`STREAM_SHELL_VERSION`); below it the source
//!   visits every stream, since a member may lie outside its shell then. A dwarf core's members
//!   lie within its extent of its centre, which its shell holds at every version.
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
//! The source is not registered with the range query: P10.T10.b registers it (at 27 or later),
//! in place of the [`CentreMemberSource`] it holds, not beside it, since their IDs would repeat.

use std::sync::OnceLock;

use crate::coords::GalacticPosition;
use crate::galaxy::Galaxy;
use crate::galaxy::features::centre::{BuildCentreError, CentreMemberSource, CentreOrbitCache};
use crate::galaxy::features::members::local_offset;
use crate::galaxy::features::members::source::{band_reach, hit_at};
use crate::galaxy::imf::MassBand;
use crate::galaxy::placement::SystemRecord;
use crate::galaxy::query::{
    LayerCounts, LayerSet, QuerySphere, SystemHit, SystemSource, UNBOUND_PAD_SPEED, pad_for,
};
use crate::time::UniverseTime;
use crate::units::{LightYears, Seconds};

use super::dwarf_core::DwarfCoreModel;
use super::members::{GlobalMemberRecord, StreamBandShares, expected_members, stream_cell_members};
use super::{
    BoundingShell, GlobalEntryKind, GlobalList, StreamNumber, StreamShellRules, TubeLookup,
    TubeTable, in_stream_shell,
};

/// The bands from the coarsest down, the order the query walks its layers.
const BANDS_COARSEST_FIRST: [MassBand; 5] = [
    MassBand::E,
    MassBand::D,
    MassBand::C,
    MassBand::B,
    MassBand::A,
];

/// The global list's members as a source of the range query (module documentation).
///
/// It holds the galaxy it is made for, the caller's [`TubeLookup`], whose list must be that
/// galaxy's, the centre's source and the cores' models, and each stream's band shares once a query
/// first reaches it. A caller registers it with the range query in place of the
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
    /// Each dwarf core's model in number order, `None` for one that fails its headroom check.
    cores: Vec<Option<DwarfCoreModel>>,
    /// Each stream's band shares in number order, made when a query first reaches it. Each is a
    /// function of the galaxy and its stream alone (a dwarf's reads its core's keyed metallicity
    /// stream), so which query makes it first moves nothing.
    shares: Vec<OnceLock<StreamBandShares>>,
}

impl std::fmt::Debug for GlobalListSource<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GlobalListSource").finish_non_exhaustive()
    }
}

impl<'a> GlobalListSource<'a> {
    /// The source of `galaxy`'s global list, `lookup`'s, whose tube tables it reads through
    /// `lookup` and the centre's cells through `orbits`: the centre's source if the list holds the
    /// centre ([`CentreMemberSource::new`]) and every dwarf core's model
    /// ([`DwarfCoreModel::from_spec`]).
    ///
    /// # Errors
    ///
    /// The [`BuildCentreError`] of a centre that cannot be built, which no galaxy the builder
    /// allows has ([`Galaxy::centre`]).
    ///
    /// # Panics
    ///
    /// As [`CentreMemberSource::new`], which no galaxy the builder allows reaches.
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
        Ok(Self {
            galaxy,
            lookup,
            centre,
            cores,
            shares,
        })
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
        self.shares[index].get_or_init(|| {
            let spec = self.list().stream(number).expect("a stream of the list");
            StreamBandShares::of(self.galaxy, spec)
        })
    }

    /// The streams and the cores whose shells `sphere`, out to its padded radius, meets, in list
    /// order, each with its shell; under [`StreamShellRules::Unbounded`] every stream (module
    /// documentation).
    fn entries_near<'s>(
        &'s self,
        sphere: &'s QuerySphere,
    ) -> impl Iterator<Item = (GlobalEntryKind, BoundingShell)> + 's {
        let every_stream = self.galaxy.rules().stream_shell == StreamShellRules::Unbounded;
        self.list()
            .entries()
            .iter()
            .filter(move |e| match e.kind() {
                GlobalEntryKind::Centre => false,
                GlobalEntryKind::Stream(_) => every_stream || e.shell().touches(sphere),
                GlobalEntryKind::DwarfCore(_) => e.shell().touches(sphere),
            })
            .map(|e| (e.kind(), e.shell()))
    }

    /// Core `number`'s model, if it is played.
    #[must_use]
    fn core(&self, number: super::DwarfCoreNumber) -> Option<&DwarfCoreModel> {
        self.cores
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
                .filter(|m| in_stream_shell(galaxy, shell, m.record().epoch_position()));
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
