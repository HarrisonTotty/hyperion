# Plan 10: The global list: streams and dwarf cores

- **Milestone:** M3.
- **Depends on:** 09, and through it 01–08 and 15.
- **Brainstorm sections covered:** "Streams and accreted structure" in full; the second bullet of
  the lean under "Large features" (the global list); the `10` row of the prefix table in "Dense
  features" for streams and dwarf cores; the "Features and streams" row of the velocity table in
  "Orbits and time"; the tube tables as caller-owned caches in "Runtime and code shape"; of
  "Testing", "stream members stay in their tubes" and the streams' part of "Budgets"; the orphan
  multiplier under "Open questions"; step 8 of "Suggested order of attack".

## Goal

When this plan is done the halo is no longer smooth. One global list per galaxy holds the galactic
centre (from plan 09), 150–1,500 stream tubes and 0–3 dwarf cores, built once in about a second,
each with a bounding shell from one integrated orbit. A stream's track is measured, not assumed:
lazily, as a pure function of seed and stream number, about 2,000 tracers are sprayed from the
progenitor's Lagrange points, integrated with a fixed-step leapfrog in the tabulated potential and
reduced to a tube table of about 256 knots per arm, which the caller caches. Members are a Poisson
process in the tube's own coordinates, placed by inverse transform with no thinning, under the `10`
prefix. Dwarf cores are features with nested grids, a star formation history and a metallicity of
their own. Every range query tests the list, the halo's field gives up the discrete share, and the
map and chart show streams and cores.

## Scope and non-goals

In scope: the global list and its construction; which debris gets a tube; globular streams, orphans
and dwarf streams; the orbit integrator; the particle spray and the tube table; stream members,
their IDs, band shares and kinematics; dwarf cores; the query path; the halo's discrete share;
protocol, server cache and display additions; the tests and cost figures the brainstorm states.

Not in scope:

- The halo's smooth components and their marks (plan 02). This plan only supplies the discrete share
  they leave room for.
- The galactic centre (plan 09), which this plan adopts as the list's first entry.
- A cluster's near tails (plan 09's tail class). The disc gets no tubes.
- Satellites beyond the root cube, shells as a feature kind, and a rotating-bar potential for the
  integrator: tubes are given only to progenitors that stay outside the bar's corotation.
- Catalogue-class hosts inside streams and dwarf cores beyond the device plan 09 provides: a dwarf
  core carries a feature-level list, a stream does not (see design note 9).

## Provides

All Rust paths are under `hyperion_sim::galaxy` unless they start with another module.

- Plan 01 already builds `id::{StreamMemberId, DwarfCoreMemberId}` (prefix `10`; stream: number 12,
  band 3, along 14, across 6 + 6, index 13; dwarf core: number 2, then band, level, cell and index
  as in a catalogue feature). This plan adds `global_list::{StreamNumber, DwarfCoreNumber}` and the
  numbering rule that gives them meaning.
- `global_list::orbit::{OrbitState, Leapfrog, OrbitSummary, BuildLeapfrogError}`:
  `Leapfrog::new(&PotentialTables, step) -> Result<Leapfrog, BuildLeapfrogError>` (an error when the
  tables lack the (R, z) grid of `Galaxy::with_full_potential`), `Leapfrog::step(&mut OrbitState)`,
  `integrate(state, steps) -> OrbitSummary` (pericentre, apocentre, radial period, mean angular
  frequency, bounding radii); `FixedStep::new(pericentre, pericentre_speed, span)`, design note 4's
  step (as built, ruling 146).
- `global_list::{GlobalList, GlobalEntry, GlobalEntryKind, BoundingShell}`,
  `global_list::{StreamSpec, StreamOrigin, DwarfCoreSpec}`:
  `GlobalList::build(&Galaxy) -> GlobalList`,
  `GlobalList::entries_touching(sphere) -> impl Iterator<Item = &GlobalEntry>`,
  `GlobalList::entries_with_class_members()` for plan 12's alerts,
  `global_list::discrete_share(Seed, &GalaxyParams, &PotentialTables) -> f64` (P10.T3.d.a; the
  seed for the dwarfs' stripping fractions, re-validated), which `Galaxy` construction stores
  through plan 09's `FeatureShares::set_halo_discrete` (P10.T3.d.b).
- `global_list::Debris` (`generate(&Galaxy)`, `streams()`, `cores()`, `cores_beyond_limit()`), the
  numbered stream and core specifications P10.T4 builds the list from and plan 15's P15.T11
  observes (P10.T3, as built).
- `global_list::spray::{Tracer, SprayParams, spray}` and
  `global_list::tube::{TubeTable, TubeKnot, TubeArm, KnotHash}` (as built in P10.T6.c, no
  `KnotHash`, a scan instead, and `TubeCell` added: Risks, "P10.T6.c as built"):
  `TubeTable::build(&Galaxy, &StreamSpec) -> TubeTable` (as built,
  `-> Result<TubeTable, BuildTubeTableError>`, with
  `TubeTable::from_tracers(&StreamSpec, &[Tracer])` for a pool's tracers: Risks, "P10.T6.a as
  built"; as built in P10.T6.b, `from_tracers(Seed, SolarMasses, &StreamSpec, &[Tracer])`, the
  seed and the halo's mean system mass first, with `TubeTable::along_cells()` and
  `TubeTable::noise(cell)`: Risks, "P10.T6.b as built"), `TubeTable::byte_size()`,
  `TubeTable::locate(u, a, b) -> GalacticPosition`, `TubeTable::mean_velocity(u)`,
  `TubeTable::cells_touching(sphere) -> impl Iterator<Item = TubeCell>` (as built,
  `cells_touching(&GalacticPosition, LightYears)`, a centre and a radius).
- `global_list::tube::TubeLookup`, the trait a caller's cache implements:
  `fn global_list(&self) -> &GlobalList` and `fn tube(&self, StreamNumber) -> Arc<TubeTable>`.
- `placement::resolve_with(galaxy, id, &dyn TubeLookup) -> Result<SystemRecord, ResolveSystemError>`
  beside plan 03's `resolve`, which keeps answering `KindNotGenerated` for stream and dwarf-core IDs
  because it has no list to read.
- `global_list::members::{stream_cell_members, resolve_stream_member, StreamBandShares}`.
- `global_list::dwarf_core::{DwarfCoreModel, StarFormationHistory, mass_metallicity}` and
  `resolve_dwarf_core_member`.
- `global_list::GlobalListSource`, implementing plan 03's `SystemSource` and wrapping plan 09's
  `CentreMemberSource` as entry 0. It owns a reference to the caller's `TubeLookup`.
- Protocol: request kinds `GlobalFeatures` and `StreamTrack` (on the wire `global_features` and
  `stream_track`, reserved by plan 04; not plan 09's `galaxy_features`), each a new variant of plan
  04's `RequestBody` and `ResponseBody` with an entry in `REQUEST_KINDS`, payload types
  `GlobalEntryWire`, `StreamTrackWire`, `TrackStatusWire { Ready, Pending }`; `SystemOriginWire`
  gains `stream` and `dwarf_core` variants (`SystemOriginWire` is P09.T39's, not built). The
  `GlobalFeatures` reply also carries the accreted progenitors' list, and `galaxy_parameters`
  gains the drawn `halo.discrete_share` and a derived key for the applied share (P10.T10.a, as
  ruled there).
- Server: `TubeTableCache` (byte-bounded LRU implementing `TubeLookup`) and a background warm-up
  job. Client: `StreamTrackOverlay` for the galaxy map, `StreamReadout`.
- Test helpers: `global_list::testing::{pal5_like, gd1_like, sagittarius_like}` stream
  specifications at Milky Way parameters, added by their first user, P10.T5.b.
- Domain tags under `stream.` and `dwarf.`, each an entry of the sim's registry
  `crates/hyperion-sim/src/rng/tags.rs` (one of three since R04.T4.d) under its "Plan 10" heading,
  added by the task that first draws on it (five so far, P10.T3).

## Consumes

- **Plan 01:** `math` (including `atan2` and `erf`), `rng`
  (`Stream::open(Seed, DomainTag, ObjectKey)`, the `domain_tags!` registry in `rng/tags.rs`),
  `units`, `time`, `coords`, `id::{StreamMemberId, DwarfCoreMemberId, SystemIdKind}` under the `10`
  prefix, whose decode already rejects band 7 for a stream, sub-kind `11`, set padding bits and a
  dwarf core's inner cells, the golden harness and statistical helpers of `hyperion-testkit`,
  slow-test marking, `just bench`, and the second CI architecture of P01.T12 (`rust-aarch64`, or its
  recorded fallback), which this plan's goldens run on without a job of their own. _As found at
  P10.T1 (2026-09-29):_ `.github/workflows/ci.yml`, with its `rust-aarch64` and `rust-wasm32` jobs,
  was deleted in `751bad8` and no workflow exists; `just test-wasm` (`wasm32-wasip1` under
  wasmtime, outside `just ci`) is what is left (see Risks). Since R04.T4.d, `math`, `rng`, `units`
  and `version` are `hyperion-base`'s, re-exported by the sim (`crates/hyperion-sim/src/lib.rs`
  about 82): `math::{atan2, erf, normal_quantile}` are `crates/hyperion-base/src/math.rs` about 96,
  285 and 586, and `Stream::open` is base's `rng/stream.rs` about 89. Plan 10's tags are in the
  sim's registry, `crates/hyperion-sim/src/rng/tags.rs`, one of three that its
  `assert_registries_disjoint` covers (R04's design note 4), under its "Plan 10" heading (about
  727–749). P01.T12 is dropped and no workflow returns (plan 01's Risks, "T12 is dropped"); since
  R04.T7, `just ci` runs the sim's fast suites, goldens included, under `wasm32-wasip1` too, and
  `just test-wasm` (in `ci-slow` as `test-wasm-slow`) the slow ones (re-validated).
- **Plan 02:** `Galaxy`, `GalaxyParams` (`AccretionHistory`: the time of the last major merger, the
  recent progenitors, Poisson with mean 8, with masses, times and provisional orbits that this plan
  revalidates, the globular count; the halo's drawn discrete share of 2–15%, carried but not
  applied; the bar's corotation radius; the dark halo), `PotentialTables` from
  `Galaxy::with_full_potential` (`potential(r_cyl, z)` on the (R, z) grid, `v_circ`, `omega`,
  `bar_corotation`; plan 02 tabulates no gradient, so design note 3 derives one), the halo's marked
  mixture and its budget, `imf`, `Galaxy::mean_system_mass`, the root cube. _As built:_
  `GalaxyParams::accretion()` → `AccretionHistory::{last_major_merger, progenitors, globular_count}`,
  the recent progenitors being the `ProgenitorKind::Recent(j)` entries of `progenitors()`, each with
  `mass`, `accreted` and a provisional `Orbit`; `GalaxyParams::halo().discrete_share()`;
  `GalaxyParams::dark_halo()` and `MassModel::dark_halo()`; the halo's budget
  `GalaxyParams::population_mass(Population::Halo)`; `Galaxy::mean_system_mass(population)`; the
  root cube `coords::ROOT_HALF_WIDTH_LY`. **The tables now have a gradient:** plan 08 added
  `PotentialTables::force(r_cyl, z) -> Option<CylindricalForce>`, the exact gradient of the bicubic
  interpolant of `potential` with the spherical components in closed form (ruling 101.4), and
  P10.T2 steps in it (design note 3, as built in Risks). P10.T2's far field reads the tables, not the stars' and gas's masses (Risks).
  Since P02.T6.f (2026-10-12) the tables' own force beyond the grid's box is the exact gradient of
  their continuation, and the global list keeps its own far field, since replacing it would move
  output (plan 02's "P02.T6.f as built"; `global_list/orbit.rs`'s "The far field").
  `Galaxy::from_params(seed, params) -> Result` builds the in-plane tables and a mutable
  `FeatureShares` (`galaxy/mod.rs` about 251–286), `Galaxy::potential()` lends the tables, and
  `PotentialTables::potential` returns an `Option` (`potential/tables.rs` about 384). The recent
  progenitors' apocentres come from `ORBIT_APOCENTRE` (`params/draws.rs` about 277, `pub(super)`),
  log-uniform on 20,000–200,000 ly for every progenitor, the dominant merger's too. The drawn
  `HaloParams::discrete_share()` (`params/halo.rs` about 161) is still applied nowhere
  (re-validated).
- **Plan 03:** the merge hook `SystemSource` (`galaxy::query::SystemSource`: `expected_in_sphere`,
  `systems_in_sphere`, `suppresses`), `SystemRecord::from_parts` (it matches on the origin
  exhaustively, so `GlobalListMember` needs an arm there), `placement::SystemOrigin`
  (`#[non_exhaustive]`; this plan adds `GlobalListMember`, design note 13), the padding rule,
  `resolve`'s dispatch on `SystemIdKind`. `resolve(galaxy, id)` takes no context, and a stream
  member cannot be resolved without its tube table, nor a dwarf core's without the list's
  `DwarfCoreSpec`, so this plan adds `resolve_with(galaxy, id, &dyn TubeLookup)` beside it, and
  `resolve` goes on answering plan 03's `KindNotGenerated` for both kinds. The padding rule is
  `query::motion::pad_for(t, speed)`. _As built:_ each of `SystemSource`'s methods also takes the
  `&Galaxy`, `systems_in_sphere` a `LayerSet` and an output `Vec`, and `suppresses` the time
  (`query/source.rs` about 114); `range_query` takes the sources as `&[&dyn SystemSource]`
  (`query/mod.rs` about 113). `SystemOrigin` (`placement/record.rs` about 87) is `Grid`,
  `FeatureMember { feature, attempt }` and `CentreMember { attempt }`; `GlobalListMember` is still
  only named in its doc comment (about 83), and the variant needs an arm in each exhaustive match
  on the origin: `SystemRecord`'s `from_parts`, `layer` and `component` (about 178, 229 and 288),
  and others in `features/members.rs`, `stellar/{system,brief}.rs`, `observe/retarded.rs`,
  `observe/stars_cache.rs` and `lensing/walk.rs`. `resolve_with` is `placement/resolve.rs` about
  130 and does not read its lookup yet. `pad_for` is `query/motion.rs` about 134, with
  `pad_speed(layer)` about 69. `query::motion::{position_at, epoch_velocity}` draw a grid record's
  velocity and panic for any other origin in a galaxy with kinematic tables (about 150–196), so a
  member drifts at its own velocity, as plan 09's `FeatureMemberSource` moves its members
  (`members/source.rs`'s `hit_at`) (re-validated).
- **Plan 04/05:** request layer, CPU pool, byte-bounded caches; the galaxy map, the chart, the
  readout. _As built:_ `REQUEST_KINDS` (`crates/hyperion-protocol/src/envelope.rs` about 368) holds
  20 kinds, and neither `global_features` nor `stream_track` appears in any code yet;
  `RequestBody` and `ResponseBody` are the same file's (about 267 and 315). The server's caches are
  `ByteLru` and `SharedByteLru` (`crates/hyperion-server/src/cache/byte_lru.rs`), its pool
  `CpuPool::try_submit(Priority, CancelToken, job)` (`compute/pool.rs`, `Priority::Bulk` for
  background work), each universe's galaxy `GalaxyCache` (`compute/galaxies.rs`). The
  systems-in-range handler (`requests/galaxy.rs`, `systems`, about 225–241) registers no source:
  it calls `range_query(&job_galaxy, &mut cells, &[], &query)`. IDs resolve through plan 03's
  `resolve` in `compute/systems.rs`, `requests/observe.rs` (`resolve_system`) and `convert.rs`.
  `galaxy_parameters` (`convert.rs` about 810) leaves out `halo.discrete_share` and
  `accretion.progenitors` as plan 10's (`EXCLUDED_PARAMETERS`, test-only, about 727; T10.a). The
  client's galaxy map is `displays/galaxy/GalaxyMapView.tsx` (its one overlay the dust's) with its
  toggle in `GalaxyMapPanel.tsx`, the chart's readout `SystemReadout.tsx`, and the request hook
  `lib/useServerRequest.ts`, under `apps/hyperion/src/renderer/src/` (re-validated).
- **Plan 06:** stellar evaluation of members, lifetimes and the evolved mass function;
  `math::normal_quantile` (P06.T1.b), which the truncated normal of P10.T8.b inverts with. _As
  built:_ members' stars through `MemberRecord::stars`; lifetimes `galaxy::fates::StellarFates::
lifetime` (only `ProvisionalFates` implements it); no item is named the evolved mass function,
  the nearest being `galaxy::fates::{mean_present_mass, mean_present_mass_of_mixture, stars_below}`
  (P10.T8.a to choose). Now `ProvisionalFates`, `CensusFates`, `TrackFates` and
  `MultiplicityFates` implement `StellarFates` (`galaxy/fates.rs` about 73), and still nothing is
  named the evolved mass function: the globulars' "evolved Schechter function"
  (`kinds/globular.rs` about 87) is the clusters' mass function, not the stars'.
  `MemberRecord::stars(&self, &Galaxy) -> SystemStars` is `features/members.rs` about 152, and
  `normal_quantile` base's `math.rs` about 586 (re-validated).
- **Plan 08:** the halo components' velocity laws, from which orphan progenitors draw their orbits;
  straight-line drift. _As built:_ `kinematics::halo::HaloKinematics` through
  `KinematicTables::halo()`; `query::motion::{position_at, epoch_velocity}`. `HaloKinematics` is
  `kinematics/halo.rs` about 157 and `KinematicTables::halo` `kinematics/mod.rs` about 346;
  `kinematics::draw_on_from` (`kinematics/draw.rs` about 142, crate-visible) is the draw from a
  given word that P10.T3.b added. `position_at` and `epoch_velocity` serve grid records only (plan
  03's entry above), and plan 12's `observe::Drift::of_record` (`observe/retarded.rs` about 230)
  matches on the origin for a member's own velocity (re-validated).
- **Plan 15:** `tables::streams::ORPHAN_STREAMS_PER_GLOBULAR`, provisional 1.5, which P15.T11.b
  later fits with this plan's generator. _Not built at P10.T1:_ no `tables/streams.rs` exists, so
  P10.T3.b, its first reader, needs it first. P10.T3.b committed it by hand, and P15.T11.a
  (`deb01729`, 2026-10-23) registered it as the fit task `streams`, its body unchanged and still
  provisional at 1.5: `crates/hyperion-fit/src/tasks/streams.rs`, `manifests/streams.toml`, its
  `tables::MANIFEST` row and its `tables.lock` entry (plan 15's "T11.a as built"). P15.T11.b fits
  it once P10.T5–T6 exist, with `fingerprint()` probes of this plan's generator (the same record's
  "For T11.b") (re-validated).
- **Plan 09:** `FeatureCatalogue::walk_process(FeatureProcess::Globular)`, `GlobularMarks` with the
  orbit and history of P09.T13, `ClusterModel` (present mass function, first-population share),
  `features::nested::NestedGrid`, `features::interior::{MemberClassTable, ClassProfile}`,
  `features::members::{FeatureLevelList, MemberRecord}` and the member placement of P09.T21,
  `CentreModel::as_global_entry`, `CentreMemberSource`, `FeatureShares::set_halo_discrete`, the
  reserved variant name `placement::SystemOrigin::GlobalListMember`,
  `FeatureKind::{Stream, DwarfCore}`, the globulars' metallicity and age laws of P09.T12.b. The
  sub-kind of a `10` ID is plan 01's `SystemIdKind`; plan 09 adds no type for it. _As built at
  P10.T1:_ `GlobularMarks` carries no orbit or history; P09.T13 is the free functions
  `features::kinds::globular::{orbit, history, history_on_orbit}` and `GlobularHistory` (pericentre,
  apocentre, eccentricity, dissolution time, initial mass, mass-loss rate); the present mass
  function is `interior::counts::depleted_slope(&ClusterModel)` and the first population's share
  `interior::abundances::first_population_share`; the globulars' laws are the constants
  `METALLICITY_LAWS`, `IN_SITU_AGE`, `ACCRETED_AGES` of `features::kinds::globular`;
  `FeatureShares::set_halo_discrete` exists but a built `Galaxy` has no way to call it (P10.T3.d
  adds one); `CentreModel` and `resolve_centre_member` exist since P09.T24–T27 (`d5330c7`), but
  `as_global_entry` and `CentreMemberSource` (P09.T28–T31) do not; `SystemOrigin::GlobalListMember` is still only reserved in a doc comment.
  _At `e8b5c6f3`:_ `set_halo_discrete` (`features/shares.rs` about 478) is still called only by its
  own test, and nothing outside `shares.rs` reads `phi` or `field_factor` (about 453 and 469):
  P09.T2.c, which applies `field_factor` to the fields, is not built and rides 26 (plan 09's
  Generator version). `as_global_entry`, `CentreMemberSource` and `Galaxy::centre()` are still not
  built. `CentreModel::new(&Galaxy) -> Result<CentreModel, BuildCentreError>` and `from_params`
  are `features/centre/mod.rs` about 81 and 95; the model cost 91 ms in release at three
  inversions (plan 09's "Phase 6, T24–T27 as built"), and takes four since ruling 144, not
  re-benched. By `decision-galaxy-25-train` (2026-10-27, §2.2), P10.T4 adds `as_global_entry` at
  plan 09's Provides signature if P09.T29 has not, and P10.T9 still needs P09.T29's
  `CentreMemberSource`, which P09.T29 does not register with the range query: P09.T37 does (train
  27), or a census's own registration. P09.T28.b splits into T28.b.a, pure, and T28.b.b, behind
  `KEPLER_REGIME_VERSION` = 25, and P09.T29 starts once T28.b.a has landed;
  `motion::{regime_of, position_velocity_at, plunge_pad, full_scan_radius}` are not built. Of the
  supernova classes, only `catalogue_classes::{registry, supernova, type_ia}` exist: the processes
  of P09.T33, T34 and T36 are not built. `MemberClassTable::new` takes a `&ClusterModel`, and
  `grid_width` starts from the constant floor `GRID_WIDTH_MIN_LOG2` (`features/interior/table.rs`
  about 164, 567 and 710; plan 09's "Phase 5 as built", "Plan 10 (P10.T7.a)").
  `GlobularHistory`'s figures are public fields (`kinds/globular.rs` about 552).
  `FeatureLevelList::with_classes` takes a `ListIndex` since P11.T8.e.b, which widened the
  centre's list index to 16 bits; a dwarf core's stays 13 (re-validated).
- _Re-validated at `e8b5c6f3` (2026-10-27, `GENERATOR_VERSION` 23):_ every item above was checked
  against its definition in the code, and the corrected ones are marked "(re-validated)" (Risks,
  "Re-validated at `e8b5c6f3`").

## Design notes

1. **The drawn discrete share is a ceiling, and the physical share is what is applied.** Plan 02
   draws a discrete share of 2–15% and leaves it unapplied. This plan derives the physical share:
   the expected systems in tubes inside 65,000 ly and in cores, counted whole, which exist only
   inside the cube, ÷ the halo's budget, from 1.5 tubes per
   expected globular at the orphan law's mean mass and the measured share of their mass inside
   65,000 ly, plus each recent progenitor's cold mass times
   the share of its orbital period spent inside 65,000 ly (a fixed quadrature of dr ÷ v_r in the
   mid-plane potential, no integration). φ for the halo is the smaller of the two, unless the
   globular tubes alone pass the drawn share, when φ is theirs (Risks, "P10.T3.d.a's globular
   part"). When the drawn
   share is the smaller, the dwarfs' cold masses are scaled down together to meet it; when the
   physical share is the smaller, the smooth components keep the difference. Both come from the
   galaxy's parameters, its seed (the dwarfs' stripping fractions, as built: Risks, "T3.a–c as
   built") and its in-plane tables, so `Galaxy` holds φ before any list is built. Reason: a halo
   of about 1% of the
   galaxy is a few 10⁸ M☉, while progenitors run to 10⁹·⁵ M☉, so a drawn share and drawn progenitors
   cannot agree unless one yields. The ceiling is the brainstorm's 15%; its floor of 2% is not
   enforced, because globular tubes alone come to under 1% of the halo at Milky Way values,
   about 0.1–4% over drawn galaxies, and raising them to 2% would
   break the brainstorm's own figures for their masses and their contrast (25–45 against the old
   r^−3.5 halo, roughly 7–15 against the slopes ruled on 2026-09-21). See Risks.
2. **Axisymmetric forces.** The integrator reads the (R, z) potential table only. The bar is left
   out because no progenitor with a tube comes inside corotation.
3. **Force interpolation.** Plan 02's tables give Φ(R, z) and no gradient. `Leapfrog::new` samples
   `PotentialTables::potential` on a logarithmic grid of its own, 128 × 128 nodes in (ln R, ln |z|)
   from 100 ly to the outer edge of plan 02's table at 2¹⁸ ly with the plane as a row of its own,
   takes central differences in the two logarithms at the nodes, once per galaxy, and interpolates
   the two force components bilinearly, with a linear patch through the plane where the vertical
   force changes sign. Beyond 2¹⁸ ly, where dwarfs and some orphans have their apocentres, the force
   is the NFW halo's closed form plus a point mass of the galaxy's stars and gas, from
   `GalaxyParams`; inside 100 ly no tube progenitor ever comes. There is no transcendental call in
   the inner loop except the two logarithms of the index, through `math`. The grid's size is set by
   P10.T2.a's error test, not by this note. _As built, this note is replaced:_ the integrator steps
   in plan 08's `PotentialTables::force` with a far field of its own (Risks, "T1 and T2 as built",
   T2.a), and P10.T5's tracers take ruling 146.3's bicubic Hermite table of the total potential
   (Risks, "The force costs about 640 ns").
4. **The step is fixed per stream:** 1 ⁄ 256 of the progenitor's radial period, held to at most 2
   Myr, with a whole number of steps over the stripping time. Tracers are released on step
   boundaries, so every body shares one time grid. _As built (ruling 146.1 of 2026-09-22, lane
   `int10b`):_ the step is `h = min(2 Myr, τ_p ÷ 64)`, with the pericentre crossing time
   `τ_p = r_p ÷ v_p` taken from the progenitor's spec, still one fixed step per stream and a whole number of
   steps over T_s. The P ⁄ 256 term is dropped: the eccentric orbits that missed 10⁻⁴ ran at the 2
   Myr cap, not at P ⁄ 256, and the energy error goes as `(h ÷ τ_p)²`. Adaptive and time-transformed
   schemes are rejected, since per-body clocks break releases on step boundaries and the landing on
   t = 0. The same rule serves dwarfs, the cores' single orbits and the shell orbits (P10.T4).
5. **Stripping time.** T_s = min(π ÷ (η Ω̄), time since the last major merger), with Ω̄ the orbit's
   mean angular frequency and η the fractional spread in frequency across the debris, r_t ÷ r_peri
   times a constant of the generator version. For a dwarf, T_s also stops at its accretion time.
6. **Knots by kernel.** Tracers are sorted by gained angle ψ within each arm; knots sit at equal
   steps of cumulative weight, and every knot quantity is a Gaussian-kernel average over the 48
   nearest tracers in rank. Four tracers a knot would be noise.
7. **Cells along are equal in line density.** The along coordinate is u = Λ(ψ) ÷ Λ_total, cut into a
   power-of-two number of cells, at most 16,384, chosen so that one cross-section of cells expects
   about 64 members over all bands. Every along cell then has the same mean before noise, and the
   inverse transform along the tube is a uniform draw mapped back through Λ. Across, 64 cells per
   axis span ±4 local widths.
8. **Gap noise is constant within an along cell.** The noise is a mean-preserving log-normal of
   one-dimensional lattice noise in ψ with wavelengths no shorter than eight cells, evaluated at the
   cell's middle and divided by its exact mean over all cells at build time, so a cell's mean count
   stays a closed form and the total is preserved exactly.
9. **No feature-level list for streams.** Streams hold white dwarfs and low-mass stars and no
   neutron stars or black holes. Their Type Ia rate is a few per 10⁸ years each, under one entry per
   galaxy inside the interval. Plan 11 decides where a stream's accreting white dwarfs go.
10. **Orphans.** The brainstorm's 1.5 streams per globular is
    `tables::streams::ORPHAN_STREAMS_PER_GLOBULAR`, the multiplier that is a parameter of the
    generator version, and counts all globular tubes. Orphans number Poisson(max(0, 1.5 × expected
    globulars − living globulars with a tube)), so most tubes are orphans. Orphan masses are
    log-normal about 10⁴ M☉ within 10³–10⁵, and their progenitors dissolved at a time drawn
    uniformly within T_s, which leaves the gap.
11. **Dwarf stripping.** A dwarf loses a drawn fraction of its bound mass (0.1–0.5) at each
    pericentre since accretion. What is lost within T_s is the tube, what is left is the core, and
    the rest is already in the smooth components. A core exists if at least 10³ systems remain and
    the progenitor is inside the cube at the epoch.
12. **Numbering.** Streams are numbered in the order: living globulars in the catalogue's walk
    order, orphans, dwarfs by accretion time. The number is a pure function of the seed.
13. **A member's record.** Plan 03's `SystemRecord` carries a `placement::SystemOrigin` and no
    mandatory component (plan 03, design note 18). This plan adds the variant `GlobalListMember`,
    whose name plan 09 reserved; a member's `component()` is `None`, its population is the halo's,
    and age, metallicity and velocity come from the stream or core, as for plan 09's members (its
    design note 20).
14. **Stream keys.** A stream's specification and spray draw on
    `ObjectKey::galaxy_item(stream number)`, a tube cell on `ObjectKey::cell(word)` with the word
    the ID of the cell's member 0 with its index zeroed, and a member on
    `ObjectKey::from(SystemId)`, as plans 03 and 09 do. _As built for T3.b–c,_ an orphan's draws
    are keyed by its place among the orphans and a dwarf's by its progenitor number (Risks, "T3.a–c
    as built", "Keys, against Design note 14"); the spray still keys by stream number.

## Tasks

P10.T1 comes first; P10.T2 can run beside it. P10.T3 needs P10.T2. P10.T1, T2.a–c and T3.a–c are
built. P10.T3.d.a comes next (`decision-galaxy-25-train`, 2026-10-27, §2.2); P10.T3.d.b, the one
call in `Galaxy` construction, lands with P10.T10.b's registration member (the desktop's rulings
on this re-validation, issue #123, 2026-10-27, points 1 and 2). P10.T4 follows P10.T3.a–c and
P10.T3.d.a: not P10.T3.d.b, since it reads no stored φ, and not P09.T29, since it adds
`CentreModel::as_global_entry` itself if P09.T29 has not (the same ruling). P10.T5 needs P10.T2's
leapfrog and P10.T3's specifications, not P10.T4's list, so it can run beside P10.T4. P10.T6 needs
P10.T1 and P10.T5. P10.T7 needs P10.T1 and P10.T3, P10.T3.d.a included, whose scaling reaches the
dwarfs' specifications, and can run in parallel with P10.T4–T6; P10.T7.b's supernova classes need
P09.T33, T34 and T36. P10.T8 needs P10.T6 and P10.T7. P10.T9 follows P10.T4 and P10.T8 and needs
P09.T29's `CentreMemberSource`. P10.T10 follows P10.T9: P10.T10.a needs P09.T39's `FeatureIdHex`
and `SystemOriginWire`, and P10.T10.b's registration with the range-query handler is a member of
its own with P09.T37 (27, else 28), which carries P10.T3.d.b's call (the same rulings). P10.T11
follows P10.T10 and needs P09.T41.a–b's feature marks and map overlay. P10.T12 closes the plan:
its budget test compares the sampled halo field plus the discrete members with the halo's budget,
so that half waits for the member that carries P10.T3.d.b and P10.T10.b's registration, and for
P09.T2.c's `field_factor` in the fields (the same rulings, point 1). New draws open
`Stream::open(seed, tag, key)` on domain tags under `stream.*` and `dwarf.*`, which each task adds
to the sim's registry `crates/hyperion-sim/src/rng/tags.rs` under its "Plan 10" heading, with the
keys of design note 14. Each task's acceptance is `just ci` green and its slow tests by name under
`just test-slow`; its fast tests run with `cargo test -p hyperion-sim --lib -- galaxy::global_list`
and `--test` for each `tests/global_list_*.rs` it adds or edits, since a bare `global_list` filter
misses tests outside that module path and integration tests whose names lack the word (Risks, "T1
and T2 as built").

### P10.T1 Numbers, kinds and reconciliation

_Built (lane `int10`, 2026-09-29, rebased onto `d5330c7` at `GENERATOR_VERSION` 15); see "T1 and T2 as built" in Risks._

`StreamNumber` (under 2¹²) and `DwarfCoreNumber` (under 4) over plan 01's `StreamMemberId` and
`DwarfCoreMemberId`; `GlobalEntryKind`; `StreamOrigin` (`LivingGlobular(FeatureId)`, `Orphan`,
`Dwarf(progenitor index)`); `TubeLookup`; `resolve_with`, which forwards every other kind to
`resolve` and until P10.T7.b and P10.T8.b answers `KindNotGenerated` for the two kinds of its own.
The task also reconciles this plan's "Consumes" with what plans 01–09 built. Files:
`galaxy/global_list/mod.rs`, `galaxy/placement.rs`. Tests: numbers out of range rejected; `resolve`
answers `KindNotGenerated` for a stream ID and a dwarf-core ID, and `resolve_with` agrees with
`resolve` on a sample of grid, feature-member and centre IDs. Acceptance:
`cargo test -p hyperion-sim global_list` passes.

### P10.T2 The orbit integrator

_T2.a–c built (lane `int10`, 2026-09-29, rebased onto `d5330c7` at `GENERATOR_VERSION` 15); see "T1 and T2 as built" in
Risks. The step rule, T2.b's energy tests and T2.c's acceptance follow ruling 146 (lane `int10b`,
2026-09-29, on `2b2683f`), recorded there too._

- **P10.T2.a Forces from the tables.** `Leapfrog::new` tabulates the gradient of Φ as design note 3
  describes, and returns `BuildLeapfrogError::NoVerticalGrid` for in-plane tables. Tests: against
  the analytic force of a Plummer sphere and of an NFW halo loaded into a table, error under 10⁻³
  over 1–300 kpc; circular speed from the force matches `PotentialTables::v_circ` to 10⁻³; the
  vertical force matches plan 02's `MassModel::vertical_force` to 10⁻³ at twenty points off the
  plane.
- **P10.T2.b Fixed-step leapfrog.** Kick–drift–kick in Cartesian galactic coordinates, in SI through
  unit newtypes, using only `+ − × ÷`, `sqrt` and `math`. `integrate` returns an `OrbitSummary` from
  the recorded radial turning points. Tests: energy conserved to 10⁻⁴ over ten radial periods at the
  step of design note 4; L_z conserved to 10⁻¹²; time reversal returns the start to 10⁻⁹ relative; a
  circular orbit stays circular. _Restated by ruling 146.2:_ 10⁻⁴ means "the pericentre passage is
  resolved"; the bound is 2 × 10⁻⁴ over ten radial periods at the step rule, with each pinned
  orbit's measured drift pinned too, and the pinned starts include one of e ≈ 0.9 with its
  apocentre near 10⁶ ly. Added: halving the step cuts the drift by 3.5–4.5 times (h²); over fifty
  radial periods the worst drift in the last five is at most twice the worst in the first five;
  optionally, two starts whose energies differ by 2 × 10⁻³ of |E| keep the difference to 1 % over
  ten periods; and (ruling 146.4) the far field's fitted monopole is the Gaussian components' mass
  from `GalaxyParams` to about 1 %. The heavy tests are slow tests.
- **P10.T2.c Determinism across platforms.** A golden file of the exact bit patterns of an orbit
  after 10⁴ steps for three pinned starts. Plan 01's second-architecture job (P01.T12,
  `rust-aarch64`, or the `wasm32-wasip1` fallback it recorded) already runs
  `cargo test -p hyperion-sim` with every golden, so this task adds no CI job and touches no
  workflow file; it only checks that the job is still there and runs this golden. A unit test reads
  the module's source with `include_str!` and fails on `mul_add` and on float methods of Clippy's
  disallowed list, so that a local `#[expect]` cannot hide one. Acceptance: both architectures green
  on the same goldens. _Restated by ruling 146.5 (the workflow is gone, Risks):_ acceptance is (a)
  `just ci` plus the slow orbit tests green on x86-64; (b) `just test-wasm` green on `hyperion-sim`,
  `global_list_orbit`'s golden and determinism test included, run by hand at each bless or re-bless
  of the orbit or tube goldens and at least at plan 10's close, recorded here with its wasmtime
  version; (c) the `include_str!` source scan. AArch64 waits until a workflow returns (P01.T12's
  owner item); a run under `qemu-aarch64` is optional.

### P10.T3 Which debris gets a tube

_T3.a–c built (lane `int10c`, 2026-09-30, on `62cbe2f` at `GENERATOR_VERSION` 15); see "T3.a–c as
built" in Risks. T3.d is split (`decision-galaxy-25-train`, 2026-10-27, §2.2, as amended by the
desktop's rulings on this plan's re-validation, issue #123, 2026-10-27, points 1 and 2): T3.d.a
lands at the version in force, pure; T3.d.b, the one call in `Galaxy` construction, rides with
P10.T10.b's registration member (below)._

- **P10.T3.a Living globulars.** For each record of `walk_process(Globular)`: pericentre from
  P09.T13; no tube if it lies inside the bar's corotation; T_s per design note 5; tube mass = the
  mass lost within T_s from the history of P09.T13. Tests: at Milky Way parameters about a fifth
  (0.12–0.30) of globulars qualify.
- **P10.T3.b Orphans.** Count, masses and dissolution times per design note 10; metallicity and age
  from the accreted globulars' laws of P09.T12.b, since an orphan is a destroyed cluster of the same
  system; all first population but for the share P10.T8.a sets. The progenitor's orbit is drawn from
  the globular system's radial law and the globular-born debris component's velocity law, redrawn on
  the same stream until the pericentre is outside corotation and the apocentre inside 10⁶ ly. Tests:
  masses within 10³–10⁵ M☉ with a median near 10⁴; every orphan's pericentre outside corotation.
- **P10.T3.c Dwarfs and their cores.** The recent progenitors are plan 02's (Poisson with mean 8,
  stellar masses on M^−1.45 over 10⁵–10⁹·⁵ M☉, accreted within 6 Gyr). Their provisional orbits are
  revalidated: apocentres of 100,000–500,000 ly, pericentres outside corotation for a tube, bound in
  the tabulated potential; a range that fails is corrected in plan 02's parameter table with a
  version bump, not patched here. Stripping per design note 11; `StreamSpec` for the tube and
  `DwarfCoreSpec` for the core. Tests over 2,000 seeds: a progenitor of at least 10⁸ M☉ with a live
  tube in 15–25% of galaxies; 0–3 cores, none in at least a third of galaxies.
- **P10.T3.d The discrete share.** `discrete_share` per design note 1, stored through
  `FeatureShares::set_halo_discrete`; the halo's smooth components carry 1 − φ of its budget. Split
  by `decision-galaxy-25-train` (2026-10-27, §2.2) into a pure part and the stored share, as
  amended by the desktop's rulings on this plan's re-validation (issue #123, 2026-10-27, points 1
  and 2); both parts' records are T3.d's.
  - **P10.T3.d.a The share, pure.** `global_list::discrete_share` per design note 1, as
    `discrete_share(seed: Seed, params: &GalaxyParams, tables: &PotentialTables) -> f64`, in a
    module of its own, `galaxy/global_list/share.rs`, re-exported from `global_list`. It takes the
    `Seed` with the in-plane tables, as `dwarfs::dwarfs(seed, params, tables)` does
    (`global_list/dwarfs.rs` about 89), because a dwarf's cold mass depends on its stripping
    fraction drawn on `dwarf.stripping` (Risks, "T3.a–c as built", "Orbits by quadrature"), and so
    that `Galaxy::from_params`, which holds all three before the galaxy exists, can call it
    (T3.d.b). Design note 1's other half, the dwarfs' cold masses scaled down together when the
    drawn share is the smaller, goes where `Debris::generate` collects the dwarfs
    (`global_list/debris.rs` about 77). It is computed from `discrete_share`'s own figures, not
    from a stored φ, so it is the same whether or not a galaxy stores φ, and `dwarfs::dwarfs`
    and its golden `galaxy/global_list/dwarfs.golden` keep the unscaled masses. Nothing generated
    reads `Debris` (only the global list's tests call `Debris::generate`), so the scaling moves no
    wired output. Nothing stores φ yet. Tests: never above the greater of the drawn share and the
    globular tubes' share; at the Milky Way fixture, the globular tubes inside 65,000 ly are 0.3–1%
    of the halo, and over the 2,000 galaxies their share is recorded in the summary and pinned;
    the distribution of the applied share over 2,000 seeds is recorded in
    a golden summary, a new golden under `tests/golden/galaxy/global_list/` written by a slow
    test, since 2,000 seeds' in-plane tables take minutes, as T3.c's sweep does; evaluating it
    costs about 10 ms for a drawn galaxy and 1.3 ms for the fixture (pod, 4 vCPU; restated from
    "under a millisecond", research-p10's item 8, Risks, "P10.T3.d.a's cost"). It lands now, at
    the version in force, with no hold and no train.
    Acceptance: `just ci`; `cargo test -p hyperion-sim --lib -- galaxy::global_list`; the
    summary's slow test by name under `just test-slow`; `golden_diff.py` lists the new golden
    alone.
  - **P10.T3.d.b The share stored, with T10.b's registration.** `Galaxy` construction calls
    `FeatureShares::set_halo_discrete` with `discrete_share(seed, &params, &potential)` after
    `set_class_shares` in `Galaxy::from_params` (`galaxy/mod.rs` about 266–272), where the seed,
    the parameters and the in-plane tables exist and `feature_shares` is still mutable: the
    `FeatureShares` the construction builds and owns, as plan 09's "T1–T6, as built" asks
    (`Galaxy` lends its own immutably). By the desktop's ruling (point 1) it has no hold, no
    `GalaxyRules` field and no tripwire: nothing reads `FeatureShares::phi` or `field_factor`
    until P09.T2.c applies them to the field at 26, so a hold would hold nothing. Nor does the
    call land on its own. If it did, P09.T2.c's bump would take φ out of the halo's field at 26
    while the stream and core members it stands for are not served before P10.T10.b's
    registration, and the halo's budget would be short across that gap, which P09.T38's budget
    test at 27 would see. So T3.d.b, the one call, rides with P10.T10.b's registration member
    (point 2), with P09.T37 at 27 or the first train after both are ready, and the halo's field
    gives up φ in the train where its members are first served, the rule R06.T16.a already
    applies to the features. Until then no built galaxy calls it, and P09.T2.c's bump leaves the
    halo's discrete part at zero. Its census goes with that member's: the applied φ per seed, the
    halo layer's counts in three cells before and after, and every density finite
    (`the_discrete_share_census`, a unit test in `galaxy::global_list::share::tests`, slow if
    long, by name under `just test-slow`). Nothing of T3.d lands inside the potential freeze.

    _Built behind the registration's hold (lane `p10-list-registration`, 2026-11-01, at
    `GENERATOR_VERSION` 24, no output moved): see Risks, "P10.T3.d.b as built"._

### P10.T4 The global list and its bounding shells

`GlobalList::build(&Galaxy)`: entry 0 is `CentreModel::as_global_entry()`; then one entry per stream
and per core. For each, one orbit integrated over T_s (a core: one radial period either way) gives
the radii between which the debris can lie, widened by four times the expected width and by the
spread in apocentre of the spray (a constant factor of the generator version, checked against built
tubes in P10.T9). `BoundingShell` is a spherical shell about the origin, clipped to the root cube;
`entries_touching` tests a sphere against it. The list is a plain value the caller keeps with the
galaxy. Tests: 150–1,500 tubes over seeds and about 240 at Milky Way parameters; order independence
of `StreamSpec` generation; golden of the first ten entries. Bench: the build, target about one
second.

_The centre's entry (`decision-galaxy-25-train`, 2026-10-27, §2.2)._ T4 reads only
`CentreModel::as_global_entry()` from the centre, and that needs no motion: the entry's kind
(`GlobalEntryKind::Centre`), its mass and its bounding shell, the members' reach about the origin
(`CentreProfile::reach`, the 128 ly of `features/centre/profile.rs`'s `REACH`, about 90 and 755).
At the base nothing defines it, and `Galaxy` builds no `CentreModel`: `Galaxy::centre()` is not
built (plan 09's "Phase 6, T24–T27 as built", which left the choice to P09.T28.b, now T28.b.a).
If P09.T29 has not added it, T4 adds `as_global_entry()` at plan 09's Provides signature (its
"The galactic centre and motion"), in `galaxy/features/centre/mod.rs`, with no output moved. So T4
waits on P10.T3, not on P09.T29. `GlobalList::build` builds the model with `CentreModel::new`
(`features/centre/mod.rs` about 81), a `Result` whose `BuildCentreError` no galaxy the builder
allows reaches (its documentation and tests), so `build` expects it with that reason or returns
the error, and records which. **The centre's cost:** `CentreModel::new` took 91 ms in release
with three inversions (plan 09's "Phase 6, T24–T27 as built", under its design note 15's 100 ms),
takes four since ruling 144, not re-benched (plan 09's "Ruling 144 as built", _Cost_), and some
seconds in the debug profile. It is inside T4's "about one second" bench and its golden of the
first ten entries, and T4's tests build one list per fixture galaxy and share it, as
`tests/global_list_orbit.rs` shares one build of its tables. Files: `galaxy/global_list/mod.rs`
(`GlobalList`, `GlobalEntry`, `BoundingShell`, `build`, `entries_touching`,
`entries_with_class_members`), `galaxy/features/centre/mod.rs` (`as_global_entry`, unless
P09.T29 added it), a new golden under `tests/golden/galaxy/global_list/`, and the bench. The
stream count: T3.b's record measured 260–310 streams per fixture galaxy against "about 240",
which T4 checks (Risks, "T3.a–c as built").

### P10.T5 The particle spray

- **P10.T5.a Release schedule.** About 2,000 tracers per stream, half from each Lagrange point,
  released at step boundaries by stratified sampling of the mass-loss history over [−T_s, 0]
  (constant rate (`MassLoss::Steady`, as P10.T3.a built it) for a living globular, ending at the
  dissolution time for an orphan, pulsed at pericentres for a dwarf), each with weight = mass-loss
  rate × interval. Tests: weights sum to the tube's mass; an orphan releases nothing after its
  dissolution. The schedule reads the spec's `StreamSpec::mass_loss()`, a `MassLoss`
  (`global_list/spec.rs` about 374): `Steady`, a living globular's constant rate from its history
  (P09.T13) over the whole stripping time, `Dissolved { ago }` for an orphan and
  `Pulsed { fraction, last_pericentre, pericentres }` for a dwarf (Risks, "T3.a–c as built").
  _Deviation from the plan's text (the desktop's ruling on this re-validation, issue #123,
  2026-10-27, point 5):_ "constant fractional rate" became "constant rate (`MassLoss::Steady`, as
  P10.T3.a built it)", so the tracers' weights sum to the tube's mass, rate × T_s, exactly; a
  fractional rate would change both the release schedule and what the tube's mass means.
- **P10.T5.b Release and integration.** The progenitor is integrated backwards from its epoch state
  to −T_s and forwards again; at each release the tracer starts at the tidal radius from the
  progenitor along the galactocentric radius, with the offsets in position and velocity and their
  scatters of Fardal, Huang and Weinberg (2015), to be taken from the paper. Tracers feel the
  table's force alone. _Deviation from the plan's text (main's ruling of 2026-10-29, after the
  research lane `research-galaxy-leans`, items 10 and 13):_ the text had "plus a Plummer sphere
  for a living progenitor, with its mass following the history", which counts the progenitor's
  pull twice, since Fardal et al.'s release offsets already hold it (their §6), and the offsets'
  Gaussians, cut at ±2.5σ, are widened so that they keep the fitted dispersions (Risks, "P10.T5.b,
  the spray's fix, as built").
  Each body accumulates the angle it sweeps in its own orbital plane, step by step, with
  `math::atan2`; ψ is the tracer's angle minus the progenitor's over the same interval. Tests: ψ is
  positive for every leading and negative for every trailing tracer and monotone in release time on
  average; for `pal5_like` the tracers' mean track departs from the progenitor's orbit by more than
  six tidal radii within one wrap, which is the brainstorm's reason for measuring. The step is the
  spec's, `SphericalOrbit::fixed_step(span)` (`global_list/spec.rs` about 344). Ruling 146.3
  binds the force: one bicubic Hermite table of the total potential on the tables' (ln R, ln |z|)
  nodes out to 2²⁰ ly with its exact gradient, and the tracers run in parallel on plan 04's pool,
  gathered by index (Risks, "The force costs about 640 ns"). The task adds `global_list::spray`
  (`galaxy/global_list/spray.rs`) and, as their first user, the test helpers
  `global_list::testing::{pal5_like, gd1_like, sagittarius_like}` (Provides).

### P10.T6 The tube table

_T6.a–c built (lanes `p10-t6a`, `p10-t6b` and `p10-t6c`, 2026-10-29 and 2026-10-30, at
`GENERATOR_VERSION` 23), and the arms split by energy (lane `p10-spray-arms`, 2026-10-30, at 24);
see "P10.T6.a as built", "P10.T6.b as built", "P10.T6.c as built" and "P10.T6.a's arms by energy,
as built" in Risks._

- **P10.T6.a Reduction.** Per arm about 256 knots per design note 6: centre line (as integer
  light-years plus offset), a local frame carried along the track by parallel transport so that it
  never flips, mean velocity, two widths, three velocity dispersions in the frame, and the
  cumulative line density Λ from the tracer weights. Linear interpolation between knots. `locate`,
  `mean_velocity`, the inverse of Λ. Tests: `pal5_like` gives a length near 82,000 ly, widths of
  120–380 ly and dispersions of 0.9–3.5 km/s; `gd1_like` near 159,000 ly, 100–260 ly and 0.7–1.3
  km/s; an orphan shows the central gap; a dwarf shows the pile-up at apocentre; frames orthonormal
  to 10⁻¹². _Deviation from the plan's text (main's rulings of 2026-10-29, after the research
  lane `research-galaxy-leans`, items 11 and 15 and its findings for T6.a):_ each arm is the
  tracers of one sign of the energy offset at release (main's ruling of 2026-10-30, in place of
  ψ's sign), not of one Lagrange point; `TubeTable::from_tracers` reduces the
  tracers a caller's pool gathered by index, and `build` sprays them in turn; the length tests are
  restated, since the helpers' stripping times (Design note 5) make `pal5_like` about 230,000 ly
  and `gd1_like` about 117,000 ly through their tracers: each tube's knots' arc length lies within
  5 % of its own tracers' path through 48-tracer bins over the same stretch, and each length is
  recorded against the brainstorm's 30,000–160,000 ly for globular streams; the dwarf test is
  reworded: for each pulse's tracers separately the line density per unit arc length is higher
  near the centre line's apocentric points (local maxima of galactocentric radius) than near its
  pericentric points; a test that every tracer whose Lagrange point's arm is not its energy's lies
  inside its energy arm's tube at its ψ, within its four-width cut or within two tidal radii of the
  progenitor at the epoch, where the arms' roots meet, is added; and widths and dispersions that
  miss their brackets are recorded, not tuned (Risks, "P10.T6.a as built" and "P10.T6.a's arms by
  energy, as built").
- **P10.T6.b Gaps.** The noise of design note 8 with an amplitude of the generator version. Tests:
  the total expected count is unchanged to 10⁻¹²; the power of the density contrast along `gd1_like`
  peaks at scales of a few thousand light-years.
- **P10.T6.c Knot hash, size and cost.** `KnotHash`: a `BTreeMap` from 1,024 ly cells to the knot
  segments whose tube, at four widths, touches them. `byte_size`. Goldens: a hash of the table's bit
  patterns for the three test streams, run on both CI architectures. Bench: `TubeTable::build` for
  `pal5_like`, targets about 0.2 s and about 80 kB. With no CI workflow, "both CI architectures"
  is x86-64 and `wasm32-wasip1`: `just ci` runs the sim's fast goldens under both (R04.T7), and
  `just test-wasm` runs at each re-bless of the tube goldens (ruling 146.5, P10.T2.c). T6 fills
  `tube::TubeTable` (`galaxy/global_list/tube.rs`, a placeholder since P10.T1).

### P10.T7 Dwarf cores

_T7.a–b built (lanes `p10-share` and `p10-t6c`, 2026-10-28 and 2026-10-30); see "Deviations in
P10.T7.a, as built" and "P10.T7.b as built" in Risks._

- **P10.T7.a The model.** `DwarfCoreModel::from_spec`: a Plummer profile flattened along an axis of
  its grid with a half-mass radius from the dwarfs' size–mass relation, up to 8,500 ly;
  `NestedGrid::new(w, 16, 8)` with w a power of two in 16–64 ly, chosen by plan 09's
  `MemberClassTable::grid_width` rule with that floor; a class table by plan 09's device with no
  segregation (q = 1) and no remnant losses; ages from a `StarFormationHistory` of the core's own
  (an old burst at 10–13 Gyr plus a tail that lasts longer the more massive the dwarf, ending at
  accretion); metallicity from the dwarfs' mass–metallicity relation, [Fe/H] = −1.69 + 0.30 ×
  log₁₀(M★ ÷ 10⁶ M☉) (Kirby et al. 2013, ApJ 779, 102, eq. 4, the mean ⟨[Fe/H]⟩ of a galaxy's
  stars; a source the brainstorm does not list), the cores' means 0.17 dex about Eq. 4 (the rms
  between galaxies), and members σ(M★) about their core's mean (below); bulk motion from the
  core's integrated orbit at the epoch and
  the Plummer dispersion. Pure functions of a `DwarfCoreSpec`; nothing is placed yet. Tests: the age
  distribution of a 10⁸ M☉ core spreads over more than 2 Gyr; the profile integrates to the spec's
  system count; the extent stays under the brainstorm's 20,000 ly. _In the code's names:_ the grid
  is `features::nested::NestedGrid::new(w, FEATURE_GRID_CELLS, FEATURE_GRID_LEVELS)`, 16 cells and
  8 levels (`features/interior/table.rs` about 704–707), whose reach is 1,024 w. `grid_width`
  (about 567) takes the least power of two from `GRID_WIDTH_MIN_LOG2`'s 1 ⁄ 64 ly whose reach holds
  the table's tidal radius, then doubles it while the doubled grid keeps every cell and band at or
  under `CELL_CANDIDATE_TARGET` (2,000) and has not reached the tails' extent. "That floor" is 16 ly
  (2⁴) in place of 1 ⁄ 64 ly, and a core has no tails, so w is 16 ly for an extent to 16,384 ly
  and 32 ly to 32,768 ly, never 64 ly under the 20,000 ly extent. As built the floor is a constant
  and the table takes a `ClusterModel` (`MemberClassTable::new` about 164, `from_counts` about
  176), so T7.a adds a floor argument and a constructor for a core without a cluster model in plan
  09's `features/interior/table.rs`, leaving every cluster's grid as it is, bit for bit (plan
  09's "Phase 5 as built", "Plan 10 (P10.T7.a)"). Files: `galaxy/global_list/dwarf_core.rs`
  (new) and `galaxy/features/interior/table.rs`. The orbit's step is the spec's
  (`SphericalOrbit::fixed_step`). _The metallicity, as ruled (the desktop's ruling on this
  re-validation, issue #123, 2026-10-27, point 4):_ a core's mean is Eq. 4 at the progenitor's
  stellar mass at infall (`DwarfCoreSpec::accreted_mass`, not the stripped remnant's `mass`),
  plus one Gaussian offset of 0.17 dex, drawn on the core's own stream under a new `dwarf.*` tag
  that T7.a registers. The paper's sample ends at 5 × 10⁸ M☉, so above that the doc comment says
  Eq. 4 is extrapolated; it gives −0.64 at 10⁹·⁵ M☉. Members are drawn about the core's mean (in
  T7.b) as a Gaussian in [Fe/H] with σ = max(0.45 − 0.06 log₁₀(M★ ÷ 1.6 × 10⁵ M☉), 0.32), Kirby et
  al. 2011's eq. 13, an error-corrected trend with luminosity, at the M★ ÷ L_V of 1.6 that Kirby
  et al. 2013 use (Woo et al. 2008). It gives 0.46 at 10⁵ M☉, 0.40 at 10⁶ and 0.34 at 10⁷, and
  is held at 0.32 from about 10⁷·⁴ M☉, where the dSph data end; the hold is marked as an
  extrapolation. _Deferred findings (2026-10-13 rule), from P10.T7.a's ruling, recorded and not
  built:_ the metal-poor skew of the luminous dwarfs (−0.7 to −1.4), which a Gaussian lacks; the
  dIrrs' 0.37–0.49 near 10⁸ M☉, against the 0.32 held; the stripped core's possibly more
  metal-rich inner stars; and the Leaky Box (Kirby et al. 2013, eq. 5) as a sourced skewed
  alternative, too broad for the luminous dSphs. None is a blocker.
- **P10.T7.b Members.** Members are placed and resolved with plan 09's machinery (P09.T21, T22)
  under `DwarfCoreMemberId`, with records per design note 13 and a `FeatureLevelList` for catalogue
  classes, which holds the supernova classes of plan 09 (Type Ia from the delay-time distribution on
  the core's own history; core collapse only while the history still forms stars).
  `resolve_dwarf_core_member`, reached from `resolve_with`. Each member's [Fe/H] is drawn about
  its core's mean with T7.a's σ(M★) (as ruled there). Tests: members are not coeval; profile
  by Kolmogorov–Smirnov; no cell above 8,192 candidates; every member ID round-trips through
  `SystemId::from_raw` (no inner cell, band 7 only with level and cell zero); order independence;
  goldens. _At the base:_ plan 09's supernova processes (P09.T33, T34, T36) are not built, so the
  feature-level classes wait on them, and `registered_classes` is empty until they register
  (`features/members/level_list.rs` about 203). The list is `FeatureLevelList::with_classes`
  (about 243), which takes a `ListIndex` since P11.T8.e.b; a dwarf core's index stays 13 bits,
  under `MemberSlot::INDEX_LIMIT` (8,192). T7.b or T8.b, whichever lands first, adds
  `SystemOrigin::GlobalListMember` with an arm in each exhaustive match on the origin (Consumes,
  plan 03), `observe::Drift::of_record`'s among them, and each adds its members' row to plan 12's
  `tests/source_horizon.rs::source_horizon_sweep` (plan 12's "T0, T1, T2 and T4 as built", T0:
  "the task that places each adds its row to the sweep").

### P10.T8 Stream members

_T8.a–c built (lane `p10-t6c`, 2026-10-30, at `GENERATOR_VERSION` 24); see "P10.T8.a as built",
"P10.T8.b as built" and "P10.T8.c as built" in Risks._

- **P10.T8.a Band shares.** `StreamBandShares`: what the cluster lost = the evolved canonical mass
  function minus the cluster's present one (plan 09's depleted slope; for an orphan the slope of a
  cluster at the end of its life), white dwarfs kept, neutron stars and black holes zero, and the
  first population's share raised as plan 09's tails are. A dwarf's tube takes the core's own shares
  and history. Tests: shares sum to 1; band A's share above the canonical one; no member of a
  globular stream resolves to a neutron star or black hole.
- **P10.T8.b The Poisson process in tube coordinates.** For a `TubeCell` (along, across, across) and
  band: mean = Λ_total ÷ (mean system mass × along cells) × noise × band share × the two differences
  of the normal cumulative function, normalised to the ±4 width cut; a Poisson count on the cell's
  stream; per member a stream keyed by its `StreamMemberId`, u uniform in the cell, a and b by
  inverse transform of the truncated normal through plan 06's `math::normal_quantile`, position from
  `locate`; members outside the root cube are dropped, the index kept. Nothing is thinned.
  `resolve_stream_member`. Tests: across-tube distribution normal to ±4 widths (Kolmogorov–Smirnov);
  counts along against Λ × noise by chi-square; order independence; the fullest cell of
  `sagittarius_like` expects about 180 M-dwarf members and none exceeds 8,192; goldens.
- **P10.T8.c Kinematics.** Velocity = the tube's mean velocity at u plus a normal draw with the
  three dispersions in the local frame, on the member's velocity stream; straight-line drift. Tests:
  dispersions by band against the table. The drift is the member's own velocity, since plan 03's
  `query::motion::{position_at, epoch_velocity}` serve grid records only (Consumes, plan 03).

### P10.T9 The query path

_Built (lane `p10-t9`, 2026-10-31, at `GENERATOR_VERSION` 24); see "P10.T9 as built" in Risks._

`GlobalListSource`: bounding shells → for a stream, `TubeLookup::tube`, the knot hash, then the tube
cells touching the sphere padded by plan 03's speed × |t| → members, distances tested at the query's
time; for a core, plan 09's nested-grid walk; for the centre, `CentreMemberSource`. The expected
count for the census is the exact sum of the cells' means. Each member counts in the layer of its
band, with `SystemOrigin::GlobalListMember`. Tests: a query on the axis of `gd1_like` equals a
brute-force enumeration of the tube; a query that touches no shell builds no table (asserted with a
counting `TubeLookup`); every member of every test tube lies inside its entry's bounding shell; the
census is independent of the cache. Bench: a 50 ly query on a globular stream's axis, recording
members generated per member returned, about 160 to find two.

_In the code's names:_ `global_list::GlobalListSource` implements `query::SystemSource`, whose
methods take the `&Galaxy` too (Consumes, plan 03). The pad is
`query::motion::pad_for(t, pad_speed(layer))`; plan 09 pads each feature by the greater of that
and its members' own speed bound (plan 09's "P09.T23.b as built", `band_reach`), and the census
sums the cells touching the unpadded radius, the hits the padded one (plan 09's "Phase 5 as
built", plan 03's contract). Members are tested at the query's time at their own velocity. The
centre's part waits on P09.T29's `CentreMemberSource`, which P09.T28.b.a must precede
(`decision-galaxy-25-train`, §2.2).

### P10.T10 Protocol and server

_T10.a built (lane `p10-t9`, step 3, 2026-10-31, at `GENERATOR_VERSION` 24); see "P10.T10.a as built"
in Risks. T10.b's additions built (lane `p10-t10b-t11`, 2026-11-01, at 24); see "P10.T10.b's additions
as built". Its registration member built behind its hold (lane `p10-list-registration`, 2026-11-01,
at 24, no output moved); see "P10.T10.b's registration member as built". Its placement by ID built
(lane `p10-placed-motion-27`, 2026-11-01, at 24, no output moved); see "P10.T10.b's placement as
built"._

- **P10.T10.a Protocol.** New kinds of plan 04's `RequestBody` and `ResponseBody`, added to
  `REQUEST_KINDS`: `GlobalFeatures { universe }` → `GlobalEntryWire` list (kind, number, mass,
  bounding radii, the living progenitor's feature ID as plan 09's `FeatureIdHex`,
  `TrackStatusWire`); `StreamTrack { universe, stream }` → per arm a polyline of at most 128 points
  with widths and mean speed, or `Pending`. `SystemOriginWire` gains `stream` and `dwarf_core`.
  It is a growing enum (plan 04's reserved list), so this needs no bump; whichever of P10.T7.b,
  T8.b and T10.a comes first adds the two variants, and they are sent only from P10.T10.b's
  registration. `just gen-protocol`. Tests: wire forms for each message and variant. _At the base:_
  `FeatureIdHex` and `SystemOriginWire` exist nowhere: they are P09.T39's, which T10.a waits on.
  `REQUEST_KINDS`, `RequestBody` and `ResponseBody` are `crates/hyperion-protocol/src/envelope.rs`
  (about 368, 267 and 315); a new kind or an optional field needs no `PROTOCOL_VERSION` bump (plan
  04's design note 15). _The two parameters plan 04 leaves to this plan (the desktop's ruling on
  this re-validation, issue #123, 2026-10-27, point 3):_
  - `halo.discrete_share` is the drawn ceiling (0.085 at Milky Way values); what is held at zero
    is the applied share. T10.a sends the drawn value as a new entry in the existing grouped
    `galaxy_parameters` list, origin drawn: plan 04's design note 11's growing key set, so no
    bump under its design note 15. It sends the applied share as a second, derived key, first in
    the build that carries P10.T3.d.b's member, since before then the applied share is zero by
    construction and a non-zero key would misstate the model.
  - `accretion.progenitors` is a list, not a key and value, so it does not go in
    `galaxy_parameters`. The `GlobalFeatures` reply carries the accreted progenitors as a list of
    their own beside the entries: the last major merger and the recent progenitors, each with its
    stellar mass, its accretion time, its orbit's pericentre and apocentre, and its entry number
    if it has a tube or a core. It is a new type inside a new request kind, so there is no bump.
  - When T10.a sends them, it removes both keys from the server's `EXCLUDED_PARAMETERS`
    (`crates/hyperion-server/src/convert.rs` about 727) and raises the parameter-count test's
    `PARAMETERS_ACCOUNTED_FOR` (about 795) to match.
- **P10.T10.b Server.** The server keeps the `GlobalList` with each universe (built on the CPU pool
  when the universe opens), serves tube tables from `TubeTableCache` over plan 04's `ByteLru`,
  builds on a miss inside the job that asked, registers `GlobalListSource` with the range-query
  handler in place of plan 09's bare `CentreMemberSource`, resolves IDs through `resolve_with`, and
  warms every table in the background at low priority after a universe opens (one to two
  CPU-minutes; about 20 MB of tables at Milky Way values and up to 120 MB for 1,500 tubes, inside
  the cache's byte bound, so eviction is normal and safe). Tests: an integration test that a range
  query inside a stream returns members with the `stream` origin; a `Pending` track becomes `Ready`;
  an evicted table is rebuilt to the same bits. _At the base:_ the systems-in-range handler
  (`crates/hyperion-server/src/requests/galaxy.rs`, `systems`, about 225–241) registers no source
  (`range_query(&job_galaxy, &mut cells, &[], &query)`); plan 09's sources and caches are
  P09.T40.a's and the merge P09.T37's, neither built. So "in place of plan 09's bare
  `CentreMemberSource`" holds only once P09.T40 has registered it. _T10.b splits (the desktop's
  ruling on this re-validation, issue #123, 2026-10-27, point 2):_ registering `GlobalListSource`
  with the range-query handler moves the range query's census and registers the centre's source
  ahead of P09.T37, so that registration, with the list's registration wherever the sky census by
  then reads feature sources (R06.T16.a's pattern), is a member of its own with P09.T37 (27, else
  28), and it carries P10.T3.d.b's call. The rest lands as additions: the list kept per universe,
  `TubeTableCache`, the warm-up, `resolve_with` at the server's resolve call sites, and the
  `GlobalFeatures` and `StreamTrack` handlers; its integration test runs the sim's `range_query`
  with the source passed in until the registration lands. The galaxies are
  `compute/galaxies.rs`'s `GalaxyCache`, the pool `compute/pool.rs`'s `CpuPool` (the warm-up at
  `Priority::Bulk`), the byte bound `cache/byte_lru.rs`'s `SharedByteLru`; the IDs that go through
  `resolve_with` are resolved today by `resolve` in `compute/systems.rs`, `requests/observe.rs`
  (`resolve_system`) and `convert.rs`.

### P10.T11 Client

_Built (lane `p10-t10b-t11`, 2026-11-01, at `GENERATOR_VERSION` 24); see "P10.T11 as built" in Risks._

The galaxy map gains `StreamTrackOverlay`: tracks as thin lines in both views, dwarf cores as
feature marks, a toggle and a legend, and a stated count of tracks still pending, since unfetched
data must not look like absence. The chart's readout shows a member's stream or core, and
`StreamReadout` shows a selected stream's mass, length, width, dispersion and progenitor. It
follows the UX guide's stream line style, which the guide has had since `9d955b26`
(`docs/frontend/ux-guidelines.md` about 1058: a solid `1px` `--text-muted` line labelled at one
end, cased as any line over a raster, and a dwarf core in the feature mark), its `STREAM` and
`DWARF CORE` terms (about 1250) and its rule that a layer still being computed says so
(`FEATURES PENDING`, about 1056); a phrase of its own for the pending tracks would be a guide edit
for the owner. Tests: projection of a track as a pure function; component tests for the overlay's
pending state and the readouts. _At the base:_ the map's one overlay is the dust's
(`apps/hyperion/src/renderer/src/displays/galaxy/GalaxyMapView.tsx`, its toggle in
`GalaxyMapPanel.tsx`), no feature mark is drawn yet (P09.T41.a–b), and the chart's readout is
`SystemReadout.tsx`, which needs P09.T39's `SystemOriginWire` to name a member's stream or core.
T11 also shows T10.a's list of the accreted progenitors in the galaxy panel, under the UX guide's
list and readout rules, and its labels go to a sign-off agent (the desktop's ruling on this
re-validation, issue #123, 2026-10-27, point 3).

### P10.T12 Statistical tests and cost figures

_Built but for the budget's second half (lane `p10-t9`, step 4, 2026-10-31, at `GENERATOR_VERSION`
24); see "P10.T12 as built" in Risks._

Slow tests. **Tubes over time:** for the three test streams, 10⁴ members each at t = ±1,000 yr lie
within 4 widths plus 0.01 ly of the centre line, have moved 0.3–0.8 ly along it, and the error from
neglected curvature is under 10⁻⁴ ly. **Budgets:** expected members inside 65,000 ly of every
tube, and every core member, against `discrete_share` × the halo's budget, counted as the share
counts them (Risks, "Research-p10's rulings for P10.T12"): (i) per galaxy, the dwarfs' part (their
tubes inside 65,000 ly by the spray, and their cores) within 5% of the share's dwarf part, the
tolerance of design note 1's period-share estimate; (ii) the globular tubes' part within 5% of the
share's globular term in the mean over 64 sweep galaxies, and per galaxy within the band that sweep
measures (0.71–1.33 of the term at `GLOBULAR_TUBES_INSIDE`'s 0.55; Risks, "P10.T3.d.a's globular
part"); (iii) the applied share never below the share's globular term. T12 measures the band (ii) again
with its spray's members. And the sampled halo field
plus the discrete members against the halo's budget within Poisson error in eight volumes. **Contrast:** on its axis at 15–18 kpc a globular stream is
about a thousandth of the reference density and roughly 7–15 times the smooth halo; the figure is
recorded, and a value outside it is a finding, since the brainstorm's range is an estimate (see
Risks). **Figures:** globular tubes 30,000–160,000 ly long, 100–400 ly wide, 0.5–3 km/s; dwarf tubes
1,000–6,000 ly wide, 10–25 km/s, several wraps. The benches of P10.T4, T6.c and T9 are recorded in
the plan's doc comment with the machine they ran on. The budget test's second half, the sampled
halo field plus the discrete members, waits for the member that carries P10.T3.d.b's call and
P10.T10.b's registration, and for P09.T2.c's `field_factor` in the fields, not just for P09.T2.c
(the desktop's ruling on this re-validation, issue #123, 2026-10-27, point 1).

## Verification

- `just ci` green after every task; `just test-slow` holds P10.T3.c, T3.d.a's 2,000-seed summary,
  T3.d.b's census if it is slow, and T12. _No workflow returns (plan 01's "T12 is dropped";
  ruling 146.5):_ `just ci` on x86-64, which runs the sim's fast suites under `wasm32-wasip1` too
  (R04.T7), and `just test-wasm` on `hyperion-sim` at each orbit or tube re-bless, at T3.d.b's
  landing and at the plan's close, its wasmtime version recorded (P10.T2.c).
- The brainstorm's Testing lines in scope: stream members stay in their tubes (P10.T12); budgets
  (P10.T3.d, T12); order independence and goldens (P10.T4, T6.c, T8.b); determinism of the leapfrog
  and the tube table on x86-64 and AArch64 (P10.T2.c, T6.c; nothing checks AArch64, and
  `wasm32-wasip1` stands in, ruling 146.5).
- Cost figures as benchmarks, targets not promises: list build about 1 s; tube table about 0.2 s and
  80 kB; about 160 members generated for a 50 ly query on a globular stream.
- By eye in the `GALAXY` display: tracks crossing the halo in both map views, leading and trailing
  arms of unequal length, a gap where an orphan's progenitor was, a dwarf's wraps, and a chart on a
  stream's axis showing a thin line of old dwarfs.

## Generator version

Plan 10's one change to generated output is one train member: P10.T3.d.b's call in `Galaxy`
construction, which stores the derived discrete share, with P10.T10.b's registration of
`GlobalListSource` with the range-query handler (and with the sky census's feature sources,
R06.T16.a's pattern). It rides with P09.T37 at 27, or the first train after both are ready, and
no longer rides 25 (the desktop's rulings on this re-validation, issue #123, 2026-10-27, points 1
and 2, amending `decision-galaxy-25-train` §2.2). P10.T9's stream-shell hold rides 26
(`STREAM_SHELL_VERSION`, Risks, "P10.T9 as built"); it moves only what `resolve_with` and the
unregistered source give. Until then no built galaxy stores φ, and
P09.T2.c's bump at 26, which applies `field_factor` to the fields, leaves the halo's discrete part
at zero; the halo's field gives up φ in the train where its members are first served. Nothing of
T3.d lands inside the potential freeze. P10.T3.d.a and the rest of P10.T4–T12 add output on new
tags and kinds and move nothing that existed, so they land at the version in force with their new
goldens at its header (decision-bump-plan-after-22, 2026-10-14, §3, amended to 2026-10-27).
Until P10.T3.d.b stores φ, the share's and `Debris`' own unwired goldens
(`galaxy/global_list/discrete_share.golden` and those of the list's) are re-blessed by name at
the version in force when a fix to them moves them, with no bump (main's ruling, 2026-10-29, on
`research-p10`'s fixes: Risks, "P10.T3.d.a's globular part"); from T3.d.b on, a change to φ moves
stored output and needs a bump. Stream and core members are additions in the sim, and their
registration is what moves the census. Reserved: the `10` sub-kind `11`; the three unused bits of a stream member's ID; stream
numbers above the list's length; `dwarf.*` and `stream.*` domain tags. Parameters that belong to
the generator version and are named as such in code: the streams per globular (1.5, from
`tables::streams`), the globular tubes' share inside 65,000 ly (`GLOBULAR_TUBES_INSIDE`, 0.55,
measured), the orphan mass
law, η's constant, the step rule, the tracer count and kernel width, the gap noise's amplitude and
wavelengths, the cut at four widths, the dwarfs' stripping fraction, the shell-widening factor.

## Risks and open points

- **Re-validated at `941d80b` for P10.T1 (lane `int10`, 2026-09-29, `GENERATOR_VERSION` 14).**
  Consumes is reconciled with plans 01–09 and 15 as built, in place (the _As built_ and _As found_
  notes there). Not built, and so not consumable yet: of plan 09's centre, `CentreModel::as_global_entry` and
  `CentreMemberSource` (P09.T28–T31; P10.T4 and T9 wait for them), though `CentreModel` and
  `resolve_centre_member` arrived with P09.T24–T27 at `d5330c7`; plan 15's
  `tables::streams` (P10.T3.b's first reader must add the provisional table); a way for a built
  `Galaxy` to call `FeatureShares::set_halo_discrete` (P10.T3.d adds it); `SystemOrigin::
GlobalListMember` (P10.T7.b or T8.b adds it, with its arm in `SystemRecord::from_parts`); a
  function named the evolved mass function (P10.T8.a picks among `galaxy::fates`); and the second
  CI architecture (below).
- **T1 and T2 as built (lane `int10`, 2026-09-29, rebased onto `d5330c7` at `GENERATOR_VERSION` 15).** No output moved:
  nothing draws, and no domain tag is added. New golden `galaxy/global_list/orbit.golden`
  (P10.T2.c). Its bits depend on the potential tables, so **any change to the mass model or its
  tables moves it, and it is re-blessed then**: it was first blessed at 14 on `941d80b` and
  re-blessed at 15 on `d5330c7`, where v15's new potential (`b2ceb62`) moved every value; the
  figures below are v15's, within a few 10⁻⁴ of v14's, and the `ENERGY_DRIFT` pins still hold.
  `resolve_with` forwards the centre's IDs, which plan 09 resolves since P09.T27 (`d5330c7`). T1's
  acceptance command, `cargo test -p hyperion-sim global_list`, misses the two `resolve_with`
  tests in `placement::resolve`; run `cargo test -p hyperion-sim --lib -- global_list placement::resolve` and `--test
global_list_orbit`. **Slow tests:** every orbit test that builds the (R, z) grid for more than
  the golden file and the determinism check, and `resolve_with`'s feature-member check (the
  kinematic tables, and an interior rebuilt per resolve, seconds each), are `#[ignore = "slow:
…"]` and run under `just test-slow`; a first version ran the feature-member check in `just ci`
  and held the shared heavy-test lock for 90 minutes.
  - _T1._ `global_list::{StreamNumber, DwarfCoreNumber}` (`new`, `get`, `LIMIT`, `TryFrom` the raw
    integer, `From` the member ID) with `BuildEntryNumberError::{StreamNumberTooLarge,
DwarfCoreNumberTooLarge}`; `GlobalEntryKind::{Centre, Stream(StreamNumber),
DwarfCore(DwarfCoreNumber)}` with `feature_kind()`; `StreamOrigin::Dwarf(u32)` holds `j` of plan
    02's `ProgenitorKind::Recent(j)`. `GlobalList` holds only its entries' kinds until P10.T4, and
    its `Default` is the empty list; `tube::TubeTable` is declared with no constructor until P10.T6;
    `TubeLookup` is in `tube.rs` and re-exported from `global_list`. `placement::resolve_with(galaxy,
id, &dyn TubeLookup)` is in `placement/resolve.rs` (plan 03's `placement.rs` is a directory
    now); it does not read the lookup yet. Its test samples grid IDs (systems, thinned and empty
    indices), the centre's black hole and an in-cell centre ID, and an open cluster's members.
  - _T2.a, design note 3 replaced._ Its premise, that plan 02 tabulates no gradient, no longer
    holds: plan 08 added `PotentialTables::force`, the exact gradient of the bicubic interpolant of
    `potential` (ruling 101.4), which is conservative where a bilinear table of central differences
    would not be. So `Leapfrog` builds no grid of its own and steps in `force`, as plan 15's
    displaced-form integrator does. **Beyond the grid** the tables' own continuation, `1 ÷ h` from the
    clamped edge point, is not the gradient of their potential (up to 3 × 10⁻⁴ of the force outside
    the grid's box), so the integrator continues the galaxy itself: the spherical components in
    closed form (the NFW halo among them) and, **in place of the note's point mass of the stars
    and gas from `GalaxyParams`**, the Gaussian components as a monopole plus the discs'
    quadrupole, matched to the tables at 2¹⁸ ly in the plane and on the axis so that the potential
    is continuous with the tables (a point mass from the parameters would not be),
    blended into the tables by a smooth step between spherical radii of 2¹⁷·⁵ and 2¹⁸ ly with the
    blend's exact gradient. Added to plan 02's `potential/tables.rs` for it, crate-visible only:
    `PotentialTables::spherical_at` and `GRID_EDGE_LY`. The plan's Plummer and NFW profiles
    "loaded into a table" cannot be, since tables are built from a `MassModel` only; the test
    instead holds the force to `MassModel::force` to 10⁻³ over 1–300 kpc at nine latitudes (3.6 ×
    10⁻⁴ at worst in a scratch sweep), and a second test holds it to the central differences of
    the potential to 10⁻⁶ through the blend and beyond. The twenty vertical-force points sit at
    latitudes of 10–70°: within about 2° of the plane the tables' `K_z` departs from the model's
    by up to 1.9 × 10⁻³ of itself (a plan 02 figure, where `K_z` is small against the radial
    force). Within 1 ⁄ 16 ly of the plane the tables' force reads their in-plane `v_c²` table,
    which differs from the derivative of their in-plane potential by about 2 × 10⁻⁵ at 150,000 ly.
  - _T2.b._ `orbit::Leapfrog<'a>` borrows the tables: `new(&PotentialTables, Seconds)`, `with_step`,
    `time_step`, `state(GalacticDisplacement, GalacticVelocity)` (position about the galactic
    centre, SI), `acceleration_m_s2` and `potential_j_kg` (taking a `GalacticDisplacement`) and
    `energy_j_kg`, `step`, and `integrate(&mut
OrbitState, steps: u32) -> OrbitSummary`, which leaves the state at the end. `OrbitState`
    carries the acceleration at its position, so a step costs one force and `n` steps equal one
    `integrate` of `n` bit for bit; `reversed()`, `radius()`, `acceleration_m_s2()`,
    `angular_momentum_z_m2_s()`. Acceleration, specific energy and angular momentum are plain SI
    `f64`s with the unit in the name, since the crate has no newtype for them. A negative
    step integrates backwards. `OrbitSummary`'s pericentre, apocentre and radial period are
    `Option`s (none before a turning point, or two), refined by the parabola through three samples;
    the radial period is `2 (t_last − t_first) ÷ (n − 1)` over all turning points; `bounding_radii`
    and `turning_points` added. `orbit::FixedStep::new(radial_period, span)` was Design note 4, with
    `STEPS_PER_RADIAL_PERIOD` (256) and `MAX_STEP` (2 Myr); `BuildLeapfrogError::InvalidStep` added.
    _Superseded by "T2 after ruling 146" below_, which changes the step and the figures that
    follow; they are kept as the evidence for the ruling.
    Measured on the Milky Way fixture over ten radial periods at that step: energy 8.8 × 10⁻⁵ for an
    inclined orbit with Pal 5's pericentre (23,300–38,400 ly; Pal 5's own reaches 16–19 kpc),
    9.0 × 10⁻⁶ for a GD-1-like one, **2.3 × 10⁻⁴ for one with radii near Sagittarius's
    (41,200–203,100 ly, e 0.66, inclined 62°) and 2.1 × 10⁻³ for e 0.8 (30,100–263,800
    ly)**, below; L_z to 5 × 10⁻¹⁴ at worst; time reversal over 5,000 steps within 10⁻⁹; a circular
    orbit's radius keeps within 1.3–1.7 × 10⁻⁴ (the leapfrog's epicycle at 256 steps a period,
    not growing over ten periods).
  - _T2.c._ The golden pins three starts after 10⁴ steps of 0.5 Myr, the eccentric one crossing
    the blend and the grid's edge. The source check reads `orbit.rs` and the force path
    (`potential/{force,tables,nfw,spherical}.rs`) up to their test modules. **No second
    architecture ran it:** the CI workflow is gone (below), and `just test-wasm` was not run,
    since wasmtime is not installed on the lane's machine (since run by lane `int10b`, below).
- **T2 after ruling 146 (lane `int10b`, 2026-09-29, on `2b2683f` at `GENERATOR_VERSION` 15).**
  Ruling 146's points 1, 2, 4 and 5 (research `r-int10`); point 3, the integrator's force grid,
  goes with P10.T5. No output moved but the unwired orbit golden, re-blessed at 15.
  - _The step (146.1)._ `FixedStep::new(pericentre, pericentre_speed, span)` (a `Metres`, a
    `MetresPerSecond` and a `Seconds`) takes `τ_p = r_p ÷ v_p` from the spec and steps at `min(τ_p ÷ 64, 2 Myr)`, shortened to
    a whole number of steps over the span; a crossing time that is not finite and positive is
    `InvalidStep` carrying it. `STEPS_PER_RADIAL_PERIOD` is replaced by
    `STEPS_PER_PERICENTRE_CROSSING` (64); `MAX_STEP` stays. Where v_p comes from (the epoch state's
    energy, `√(2 [E − Φ(r_p)])`), or P10.T4's shell orbit) is the caller's, at P10.T4 and T5.
  - _The tests (146.2, 146.4)._ The pinned starts carry their spec, pericentre and speed there
    as a probe at 0.25 Myr measures them (the energy test checks the pins against the probe to
    1 %), and gain a fifth, "wide": e 0.90, 53,200 ly to 10⁶ ly, started at apocentre 30° above the
    plane. Over ten radial periods at the rule's step (slow tests, `tests/global_list_orbit.rs`,
    v15, x86-64): energy 3.0 × 10⁻⁵ (inner, e 0.24, 0.44 Myr steps), 5.8 × 10⁻⁶ (retrograde, e
    0.08, 1.24 Myr), 3.0 × 10⁻⁵ (Sagittarius-like, e 0.66, 0.56 Myr), 5.3 × 10⁻⁵ (outer, e 0.8,
    0.36 Myr) and 2.3 × 10⁻⁵ (wide, 0.58 Myr); L_z to 2.1 × 10⁻¹³ at worst, each pinned at about 1.5 times that
    (`Start::drift`) under the ruled bound of 2 × 10⁻⁴ (`ENERGY_DRIFT`), against 2.3 × 10⁻⁴ and 2.1
    × 10⁻³ for the eccentric two at the old step. The fixture does better than the research's toy
    (1.1–1.7 × 10⁻⁴ at e 0.8–0.9). **Convergence:** halving the step cuts the worst drift by 4.21,
    4.62, 4.05, 3.20 and 3.92 times; the ruling's 3.5–4.5 holds for three orbits of five, since the
    worst drift is read at the steps and scatters with where they fall about pericentre (at steps
    0.3 % different, from the probe's spec rather than the pins, the same five read 4.21, 4.57,
    3.30, 3.49 and 4.47). The test
    holds the geometric mean of the five (3.97) to 3.5–4.5 and each ratio to 3–5, a reading of the
    ruling recorded for the owner. **Secular drift:** over fifty radial periods the worst drift in
    the last five periods is 0.71–1.16 times the first five's (bound 2). **Energy difference
    (optional):** two starts 2 × 10⁻³ of |E| apart keep their difference to 0.04–0.27 % over ten
    periods for four orbits, taken as the difference of energies averaged over each radial
    period; **the outer orbit's grows steadily to 1.04 % by the tenth period** and is pinned at
    1.5 %, the others at the ruled 1 %. The outer orbit's apocentre lies at the far field's blend
    and the grid's edge, which may be why; not investigated. The figure is sensitive to the step:
    at a step 0.3 % shorter it read 0.19 %. Read instantaneously the difference holds only 1.2–4.2
    % for the four eccentric orbits (0.2 % for the retrograde one), because the two orbits dephase
    and their step errors, each up to the drift above, stop cancelling (a step of τ_p ÷ 128 brings
    it to 0.3–0.8 %). Both are recorded for the owner. The
    near-circular retrograde orbit takes 340 steps a radial period, not the research's "over 430
    for every orbit": for e near 0 `τ_p` is about the period ÷ 2π. **Monopole (146.4):** a slow
    unit test holds the far field's fitted `−A ÷ G` to the Gaussian components' mass from
    `GalaxyParams` (thin, thick and nuclear discs, bar, bulge and gas) to 1 %: 5.8921 × 10¹⁰ M☉ against 5.8919 × 10¹⁰ M☉, 2.7 × 10⁻⁵ apart (the Gaussians' own
    total equals the parameters').
    Costs in the slow profile: ten periods of all five starts take about 1.3 s after the grid.
  - _The golden (T2.c)._ `orbit.golden` now pins each of the inner, retrograde and outer starts
    over 4 Gyr at the rule's step from its spec (9,114, 3,235 and 11,209 steps, recorded with the
    step), not 10⁴ steps of 0.5 Myr: the golden moves with the rule, which is a parameter of the
    generator version.
  - _Acceptance (146.5)._ Restated in P10.T2.c. **Run at this re-bless (2026-09-29, wasmtime
    48.0.2, `e9f1ea232`, from the `calib` lane's scratch tools via `PATH`):** (a) `just ci` and the
    seven slow tests above by name on x86-64, green; (b) `just test-wasm`'s first command,
    `cargo test --target wasm32-wasip1 -p hyperion-sim -p hyperion-testkit`, with the golden, the
    determinism check and the orbit unit tests green, and the seven slow orbit tests by name under
    wasm in the slow-test profile, green, with the same figures to the printed digits. The recipe's
    second command, every slow test of the two crates under wasm, was not run: the lane runs only
    the slow tests it wrote or edited. **`just test-wasm` is not green at `2b2683f`, for reasons
    outside plan 10:** `snr::caps::tests::cut_ends_a_window_at_its_cap` uses `catch_unwind`, which
    aborts the whole lib binary under wasm's `panic = abort` (so the recipe stops there, before
    any integration test; the run above skipped it with `--skip`), `planetary_properties`'s
    `satellites::moons_inside_hill_spheres_and_rings_inside_roche_limits` spawns scoped threads,
    which `wasm32-wasip1` lacks, and `planetary_rocky_properties`'s
    `rocky_core_mass_fractions_follow_plotnykov_and_valencia` hits a multiply overflow under
    wasm32 (not traced). For the owner. (c) The source scan passes.
- **T3.a–c as built (lane `int10c`, 2026-09-30, on `62cbe2f` at `GENERATOR_VERSION` 15; T3.d
  deferred until after v16).** No output moved: nothing reads the debris yet, and the one golden
  touched, `rng/tags.golden`, gains only the five appended tags. **Every figure below is v15's and
  moves with the potential, which pot02 revises for v16: the pins are provisional and are
  re-measured on the v16 commit.**
  - _Items._ `global_list::Debris` (new: `generate(&Galaxy)`, `streams()`, `cores()`,
    `cores_beyond_limit()`), the numbered list P10.T4 builds from; `global_list::spec::{StreamSpec,
DwarfCoreSpec, SphericalOrbit, MassLoss, ClusterStars, FREQUENCY_SPREAD}`, the five re-exported
    from `global_list`; `orphans::{MASS_LAW, MassLaw, APOCENTRE_LIMIT_LY, ORBIT_ATTEMPTS,
ORBIT_WORDS}`; `dwarfs::{STRIPPING_FRACTION, CORE_MIN_SYSTEMS, APOCENTRE_WINDOW_LY}`; plan 15's
    `tables::streams::ORPHAN_STREAMS_PER_GLOBULAR`, provisional 1.5 with no manifest entry, for
    P15.T11.a to take over; crate-visible `kinematics::draw_on_from`, plan 08's draw from a given
    word (`draw_on` is it at word 0, bit for bit). Tags, all `Galaxy`-scoped:
    `stream.orphan.{count, marks, orbit}`, `dwarf.{orbit, stripping}`. `Debris::generate` panics
    without the kinematic tables. `DwarfCoreNumber`'s doc still says "0–3 cores"; see the cores below.
  - _Orbits by quadrature, not integration._ A spec's orbit is `SphericalOrbit`: in the mid-plane
    potential taken as spherical (P09.T13's approximation), the turning points, `E` and `L` from
    them, and by a 16-panel, 32-point Gauss–Legendre quadrature of `dr ÷ v_r` in θ (`r = r̄ + ½ (r_a
− r_p) sin θ`) the radial period and the angle swept in it, whose ratio is Ω̄. Reason: the
    dwarfs' figures then need only the parameters, the seed (for `dwarf.orbit` and
    `dwarf.stripping`) and the in-plane tables, as Design note 1 needs of P10.T3.d, and the
    2,000-seed sweep needs no (R, z) grid. **So P10.T3.d's `discrete_share` must take the `Seed`
    (or the `Galaxy`) as well as Provides' `(&GalaxyParams, &PotentialTables)`**: a dwarf's cold
    mass depends on its drawn stripping fraction. The leapfrog integrates the specs
    later (P10.T4, T5). **Cross-check** (slow test): every fourth spec of one Milky Way galaxy,
    integrated at ruling 146's step for three radial periods, gives Ω̄ 0.866–1.000 of the
    quadrature's (median 0.991) and a pericentre 1.000–1.148 of the spherical one (median 1.006),
    none inside corotation (asserted).
  - _Where the step rule's `v_p` comes from (int10b's open point)._ `SphericalOrbit::
pericentre_speed()`, `|L| ÷ r_p` of the spherical approximation, which is `√(2 [E − Φ(r_p)])` with
    the epoch state's `E`; `SphericalOrbit::fixed_step(span)` is `FixedStep::new(r_p, v_p, span)`.
    P10.T4 and T5 take the step from the spec.
  - _Design note 5._ η's constant `FREQUENCY_SPREAD` is 2, not 1: a star released at `r_p + r_t`
    with the progenitor's angular speed differs in energy by `2 v_c² r_t ÷ r_p` in a flat curve
    (science check of Johnston 1998, ApJ 495, 297, §2.2 eqs. 4, 8, 11, 13, and Johnston, Sackett and
    Bullock 2001, ApJ 557, 137, eqs. 3 and 5). `T_s` also stops at the cluster's age. The living
    globulars' median `T_s` on the fixture is the last major merger's 10 Gyr (the science check
    puts a 10⁴–10⁵ M☉ cluster's wrap time at 30 kpc at 40–180 Gyr).
  - _Keys, against Design note 14._ An orphan's draws are keyed by `galaxy_item` of its place among
    the orphans and a dwarf's by its progenitor number `6 + j`, not by stream number, so that
    neither moves when the count of streams before it changes. P10.T5's spray still keys by stream
    number. Dwarf streams are numbered most recent first, ties by `j`.
  - _T3.a._ `bulk_velocity`, `history` (P09.T13), then a tube if the pericentre lies outside
    corotation (19,840 ly on the fixture); its mass is the history's rate × `T_s`. **Finding:
    0.036 of globulars qualify** (46 of 1,264 over eight fixture seeds), against the plan's
    0.12–0.30. The Baumgardt–Hilker catalogue in `features::testing` gives 0.18 of its 165
    clusters above 6.08 kpc, and 0.10 of the 141 inside 20 kpc, so plan 09's globular orbits run
    more radial than the Milky Way's (P09.T14's median pericentre of 1–2.5 kpc holds, but its tail
    above corotation is thin). Pinned at 0.02–0.06: provisional; ruling deferred. Living tubes' median mass 1.8 ×
    10⁴ M☉.
  - _T3.b._ The mass law's 0.5 dex matches the known thin streams (Bonaca and Price-Whelan 2024,
    arXiv:2405.19410, Table A.1: median 7 × 10³ M☉, 0.53 dex). Radii follow the metal-poor
    globulars' cored law alone, since an orphan is an accreted cluster, out to 10⁶ ly and uncut at
    plan 09's 65,000 ly placement cut, because a progenitor's apocentre may lie far beyond it
    (the plan says only "the globular system's radial law"). Velocities follow the globular-born
    debris law through `draw_on_from`, each attempt in its own 67-word block of the one stream.
    After 256 failed attempts an orphan is dropped; none is at the fixture. Measured: 251–300
    orphans per fixture galaxy, so 260–310 streams against P10.T4's "about 240", which P10.T4
    checks. Median 10^4.002 M☉ over 2,136 orphans; the test allows
    0.1 dex, about five times the sample median's error.
  - _T3.c._ Plan 02's orbital phase is read as the radial phase since pericentre. The argument of
    pericentre, which plan 02 does not draw, is drawn on `dwarf.orbit`. `r_t` is taken at the
    stellar mass at accretion. **Finding (science check):** a dwarf's tidal radius is its total
    bound mass's, 2–10 times larger for halo-to-stellar ratios of 10–1,000, which would shorten
    its wrap time; plan 02 draws no halo mass; ruling deferred. Cores are
    kept in the dwarfs' order up to plan 01's four, and the rest are counted in
    `cores_beyond_limit`.
  - _Finding: plan 02's recent-progenitor orbits fail the revalidation._ Their apocentres are
    log-uniform on 20,000–200,000 ly (`ORBIT_APOCENTRE`, shared by all non-dominant progenitors),
    against the plan's 100,000–500,000: 0.302 of 16,054 progenitors over 2,000 seeds lie in the
    window, 0.492 have pericentres outside corotation, and all are bound. As drawn, a ≥ 10⁸ M☉
    progenitor has a tube in 0.127 of galaxies, 0.017 have no core, and up to 12 cores meet the
    conditions. Not patched here: plan 02's table is pot02's file set; provisional; ruling deferred.
    The sweep pins these provisionally, and asserts the plan's windows on the same orbits with
    their apocentres moved to the same quantile of 1–5 × 10⁵ ly. Those give a tube in 0.203 of
    galaxies and no core in 0.337, both inside the windows, but **51 of 2,000 galaxies (0.026)
    meet a core's conditions more than three times**, 9 of them more than four times. "0–3 cores"
    is pinned at 3% above three: provisional; ruling deferred. The test's moved orbits (`revalidated`) assume plan
    02's log-uniform 2 × 10⁴–2 × 10⁵ ly and must be revised with it. Whether "0–3" is the count or the number field's
    range, and which cores to keep when more qualify, is for the orchestrator.
  - _Tests._ Fast: `spec` (Kepler and harmonic orbits in closed form, the half-orbit time, the
    stripping time), `orphans` (the mass law and the radial law by Kolmogorov–Smirnov, the count),
    `dwarfs` (pericentre counting, the epoch state on its orbit, one drawn galaxy twice), and
    `rng::tags`'s plan 10 registration, and a new unwired golden, `galaxy/global_list/dwarfs.golden`
    (two drawn galaxies' dwarf debris and a Kepler orbit's quadrature, on in-plane tables; it
    moves with the potential and is re-blessed then). Slow, run by name:
    `tests/global_list_debris.rs`'s `milky_way_debris_is_numbered_bounded_and_measured` (T3.a–b,
    the numbering, determinism, the leapfrog cross-check; about 40 s) and
    `galaxy::global_list::dwarfs::tests::dwarf_tubes_and_cores_meet_plan_10_s_windows_on_revalidated_orbits`
    (T3.c; about 8 minutes on the loaded lane machine, almost all of it the in-plane tables).
    P10.T4 still owes the order-independence test of the specs.
- **The second CI architecture is gone.** `.github/workflows/ci.yml`, with the `rust-aarch64` and
  `rust-wasm32` jobs of P01.T12, was deleted in `751bad8` ("checkpoint"), and no workflow exists at
  `941d80b`. P10.T2.c's "check that the job is still there" fails, and its acceptance, "both
  architectures green on the same goldens", cannot be met until a job returns or `just test-wasm`
  is run by hand. For the owner (P01.T12 is gated on them). _Ruled (146.5):_ P10.T2.c's acceptance
  is restated around `just test-wasm` by hand; AArch64 waits for a workflow. wasmtime has no
  installed home on the dev machine: the owner's call where (v48.0.2 sits in the `calib` lane's
  scratch tools today).
- **`integrate` calls `math::atan2` once a step**, for Ω̄; `step` does not, so tracers that need
  no summary should be stepped with `step`.
- **The force costs about 640 ns** (a release build on the loaded lane machine; the tables'
  bicubic lookup, two logarithms, the NFW halo's and the nuclear cluster's closed forms, the
  latter with `powf`). About 2,000 tracers over a few thousand steps each is some seconds, against
  P10.T6.c's 0.2 s for a tube: P10.T5 and T6 should measure first. A grid of the total potential
  of the integrator's own, bicubic like the tables', would drop the closed forms from the inner
  loop. _Ruled (146.3), for P10.T5:_ at τ_p ÷ 64 a Pal 5-like stream takes about 9,000 steps, so
  the grid is needed: one bicubic Hermite table of the total potential on the tables' (ln R, ln
  |z|) nodes out to 2²⁰ ly, with its exact gradient, and the tracers run in parallel on plan 04's
  pool, gathered by index; the 0.2 s is stated as parallel wall time. Fallback if the budget still
  slips: τ_p ÷ 32 with a bound of 5 × 10⁻⁴.
- **Updated for the 2026-09-21 density rulings.** The halo's smooth components now have inner slopes
  of 2.2–2.8, and the dominant merger breaks at 16–28 kpc, steepening by 1.5–2.5 beyond. At 15–18
  kpc that halo is three to four times denser than the r^−3.5 halo the brainstorm's 25–45 was worked
  on, so P10.T12's contrast becomes roughly 7–15. The streams' masses and widths are unchanged. The
  new range is an estimate from spherical profiles, so the first run records the contrast rather
  than tuning streams to it.
- **The discrete share against the dwarf mass range.** The mixture table gives the discrete share
  2–15% of a halo that holds a few 10⁸ M☉, while dwarf progenitors run to 10⁹·⁵ M☉. Resolved by
  design note 1: the drawn share is a ceiling and scales down the cold debris of the largest dwarfs;
  the 2% floor is not enforced, since a galaxy with no recent dwarf has under 1% in tubes at Milky
  Way values. This
  departs from the letter of the mixture table's "2–15%" at the low end and is reported to the
  brainstorm's owner, who should either widen the row to "under 1% to 15%" or say what fills the
  gap. Until then the applied share is recorded per seed in P10.T3.d's golden summary.
- **"The multiplier is a parameter of the generator version"** follows the sentence on a stream's
  density contrast in the brainstorm. It is read as the streams-per-globular multiplier, as "Open
  questions" and plan 15 have it, and the contrast is tested as an outcome.
- **`resolve` needs a tube table** for a stream member, 0.2 s on a cold cache. `resolve_with`
  carries the `TubeLookup`; IDs from saves then resolve at that cost once.
- **Living globulars' tube masses** come from individual cluster histories, while φ uses the orphan
  law's mean for all tubes. The budget test's 5% tolerance covers it; if it does not, φ takes the
  mean over the living globulars' closed form.
- **Bounding shells are estimated before the spray.** A tracer outside its shell would make a query
  miss members. P10.T9 asserts containment for the test streams and P10.T12 over a sample of seeds;
  the widening factor is raised until it holds.
- **Fardal et al.'s constants and Kirby et al.'s relation** are quoted from memory in the research
  behind the brainstorm. P10.T5.b and P10.T7 re-check them against the papers.
- **The second architecture is plan 01's** (P01.T12). If it fell back to `wasm32-wasip1`, this
  plan's bit-pattern goldens run there instead, and the rule that the integrator uses only
  IEEE-exact operations and `libm`, which P10.T2.c's source check enforces, carries the rest.
- **Members as `SystemRecord`s** (design note 13): plan 03's record carries a `SystemOrigin` and no
  mandatory component, so a stream or core member needs no placeholder. P10.T9 tests that no member
  reaches a component's laws.
- **Dwarf cores resolve through the list.** `resolve_with` reads a core's `DwarfCoreSpec` from the
  caller's `GlobalList`, because rebuilding it needs the progenitor's integrated orbit. A caller
  without a list cannot resolve a core's member, as it cannot a stream's.
- **Re-validated at `e8b5c6f3` (2026-10-27).** Docs only, at `GENERATOR_VERSION` 23, swept against
  `origin/rendering-and-planets` and anchored on `9d955b26`, where the plan arrived on this branch
  (the record "Re-validated at `941d80b`" was made on the old galaxy-generation history). Nothing
  was run but listing and reading. P10.T1, T2.a–c and T3.a–c are built; the rest is checked as
  far as its inputs allow. The lane then merged `f7b6f6e7`, which splits plans R09's and R10's
  records and touches nothing the sweep read.
  - _What changed, and why._ Names, signatures, files and commands are corrected to the code, and
    the ruled split and the desktop's five rulings on this re-validation's open points (below) are
    written in:
    - Provides: `discrete_share` takes the `Seed` (T3's record: a dwarf's cold mass depends on its
      drawn stripping fraction, and `dwarfs::dwarfs` takes the seed, the parameters and the
      in-plane tables); `Debris` is listed, which T4 builds from and P15.T11 observes; the test
      helpers name their first user, T5.b; the tags are in the sim's registry.
    - Consumes: every plan's entry gains the corrected items, marked "(re-validated)", and a
      closing line names this sha.
    - Design notes 1, 3 and 14: their references to code, by the records ("T3.a–c as built",
      "T1 and T2 as built"). No substance changed.
    - The ordering paragraph: T3.d's split by the ruling; T4 after T3.a–c and T3.d.a, not
      P09.T29; T5 beside T4, since it needs T2's leapfrog and T3's specifications, not the list;
      T7 after T3.d.a, whose scaling reaches the dwarfs' specifications; each task's dependencies
      on plan 09 (T7.b on P09.T33, T34 and T36, T9 on P09.T29, T10.a on P09.T39, T11 on P09.T41.a–b
      and P09.T39); T3.d.b and T10.b's registration as one member with P09.T37; T12's budget half
      after that member. A common acceptance is added, since no unbuilt task had one and a bare
      `global_list` filter misses tests (T1's record).
    - Tasks: T3's status line; T3.d.a and T3.d.b; T4's note; the code's names, files and binding
      records in T5.a–b, T6.c, T7.a–b, T8.c, T9, T10.a–b, T11 and T12.
    - Verification and Generator version: by the ruling, and with no CI workflow (plan 01's "T12
      is dropped"; R04.T7 runs the sim's fast suites under `wasm32-wasip1` in `just ci`).
  - _The ruled split and T4's note_ (`decision-galaxy-25-train`, 2026-10-27, §2.2, as amended by
    the desktop's rulings below). T3.d.a, `discrete_share` with its tests and the 2,000-seed golden
    summary, is pure and lands now. Design note 1's scaling of the dwarfs' cold masses goes with
    it, in `Debris::generate`, from the share's own figures, since nothing generated reads
    `Debris` and `dwarfs.golden` keeps the unscaled masses: no wired output moves. T3.d.b, the one
    call that stores the share at `Galaxy` construction, has no hold and rides with T10.b's
    registration member, with P09.T37 at 27 or the first train after both are ready. T4 reads only
    `CentreModel::as_global_entry()`, adds it at plan 09's Provides signature if P09.T29 has not,
    moves no output, and waits on P10.T3, not on P09.T29.
  - _Pending re-validation:_ T7.b waits on P09.T33, T34 and T36 for its feature-level classes;
    T9 on P09.T29's `CentreMemberSource`, which waits on P09.T28.b.a; T10.a on P09.T39's
    `FeatureIdHex` and `SystemOriginWire`; T10.b on P09.T40 and, for its registration member,
    P09.T37; T11 on P09.T41.a–b and P09.T39; T12's budget half on that member and on P09.T2.c.
    T5.b, T6 and T8 use this plan's own sketches (the spray, the tube table, the members),
    re-checked as each earlier task lands.
  - _Records now stale in part, left as written:_
    - "T3.a–c as built" defers T3.d "until after v16" and says its figures are re-measured on the
      v16 commit. The code still pins v15's, marked provisional until the potential of v16 lands:
      `tests/global_list_debris.rs` holds the living globulars' share to 0.02–0.06 and the
      orphans' median to 0.1 dex of 10⁴ M☉ (the 251–300 orphans a galaxy are printed, not
      asserted); `dwarfs.rs`'s slow sweep holds a ≥ 10⁸ M☉ tube to 0.15–0.25 of galaxies, no core
      to at least a third and more than three cores to 3% on the moved orbits, and 0.10–0.16,
      under 0.05 and more than four on the orbits as drawn; `tests/global_list_orbit.rs` keeps
      v15's `drift` pins under `ENERGY_DRIFT` = 2 × 10⁻⁴ and the outer orbit's 1.5%. No commit
      since `9d955b26` changed a pin. The two goldens' values moved once, at P11.T4.h's last-bit
      shift of the mean system mass (`c07edb86`), and their headers at each bump to 23. Whether the
      slow pins hold at 23 is unknown here; T3.d.a's lane runs them.
    - "T1 and T2 as built", T2.a: the tables' continuation beyond the grid is now the exact
      gradient of their potential (plan 02's "P02.T6.f as built", "The global list keeps its own
      far field"). T2.c's "`just test-wasm` is not green at `2b2683f`": R04.T7.a fixed the three
      failures it names, and `just ci` runs the sim's fast suites under `wasm32-wasip1`.
    - "The second CI architecture is gone" and "The second architecture is plan 01's": P01.T12 is
      dropped (plan 01's "T12 is dropped", 2026-09-30).
    - "Re-validated at `941d80b` for P10.T1": `tables::streams` exists and is registered
      (P15.T11.a); `set_halo_discrete`'s caller is T3.d.b's; `GlobalListMember` and an evolved
      mass function still do not exist.
    - "Fardal et al.'s constants and Kirby et al.'s relation": Kirby et al.'s is re-checked at
      source and ruled (ruling 4, in T7.a); Fardal et al.'s stay T5.b's to check.
  - _Brainstorm drift:_ four commits touch the brainstorm since `9d955b26`. `27e90fa0`
    (P11.T8.e.b) gives the centre's feature-level list a 16-bit index under the spare band value;
    the stream and dwarf-core layouts that Provides and T7.b's ID test name are unchanged, a dwarf
    core's list index staying 13 bits (`id/global.rs`'s `the_centre_s_list_index_is_sixteen_bits`).
    `0c115993`, `78a8c520` and `5cf09bbf` touch contact binaries, planetary composition and a
    citation's year, none of them this plan. Nothing in the plan changed for them.
  - _Numbers re-checked:_ the potential grid's edge at 2¹⁸ ly (`potential/tables.rs` about 22);
    the 8,192 cap (`MemberSlot::INDEX_LIMIT`, and a stream member's 13-bit index); 4,096 streams
    and 4 cores (`StreamNumber::LIMIT`, `DwarfCoreNumber::LIMIT`); `GENERATOR_VERSION` 23. Not
    re-measured, since nothing ran: corotation (19,840 ly on the fixture at v15), the stream count
    (260–310 against T4's "about 240") and `CentreModel::new`'s cost at four inversions.
  - _The six misses, awaiting a ruling_ (deferred 2026-10-07, "Galaxy physics and calibration",
    under the owner's rule), unchanged:
    - the discrete share's 2% floor against under 1% ("The discrete share against the dwarf mass
      range"), which T3.d.a's golden summary records per seed;
    - 0.036 of globulars with a tube against 0.12–0.30 (T3.a), from plan 09's globular orbits,
      pinned at v15;
    - a dwarf's tidal radius at its stellar mass, not its bound mass (T3.c), since plan 02 draws
      no halo mass;
    - plan 02's recent-progenitor apocentres, `ORBIT_APOCENTRE`'s 2–20 × 10⁴ ly against 1–5 × 10⁵
      (T3.c): a change to plan 02's table with a bump, of drawn galaxy parameters whose
      per-progenitor list plan 04 leaves to this plan to send (plan 04, about 947; ruling 3),
      and the tests' `revalidated` orbits change with it;
    - "0–3 cores" (T3.c), 2.6% of galaxies over three on the moved orbits, which follows the
      apocentres and the reading of "0–3";
    - the energy test's convergence reading and the outer orbit's energy difference ("T2 after
      ruling 146"), which P02.T6.f leaves as they were, since the global list keeps its own far
      field.

    None is on the owner's list of blockers.

  - _Open points, ruled_ (raised on issue #123 and ruled by the desktop the same day, 2026-10-27;
    each is written into the plan as ruled, and is closed):
    - _1. T3.d.b's hold would hold nothing._ Nothing outside `features/shares.rs` reads `phi` or
      `field_factor` until P09.T2.c applies them at 26. _Amended:_ T3.d.b has no hold, no
      `GalaxyRules` field and no tripwire, and does not land on its own: stored now, P09.T2.c's
      bump would take φ out of the halo's field before the members it stands for are served, a
      shortfall P09.T38's budget test at 27 would see. It rides with T10.b's registration member,
      its census goes with that member's, and T12's budget half waits for it (T3, T3.d.b, the
      ordering paragraph, T12, Generator version).
    - _2. T10.b's registration moves the range query._ _Accepted, with point 1 folded in:_ the
      registration, with the sky census's feature sources where they are read by then, is a
      member of its own with P09.T37 (27, else 28) and carries T3.d.b's call; the rest of T10.b
      lands as additions (T10.b).
    - _3. Plan 04's two parameters._ _Amended:_ T10.a sends the drawn `halo.discrete_share` as a
      grouped `galaxy_parameters` entry, and the applied share as a derived key from the build that
      carries point 1's member; `GlobalFeatures`' reply carries the accreted progenitors' list, and
      T11 shows it, its labels to a sign-off agent; both keys leave `EXCLUDED_PARAMETERS` then.
      Plan 04's two reasons are corrected in main's next docs batch (T10.a, T11, Provides).
    - _4. Kirby et al.'s 0.17 dex_ is the rms of the galaxies' means about eq. 4, not a spread
      among one dwarf's stars. _Amended:_ a core's mean is eq. 4 at its infall mass plus one 0.17
      dex offset on a new `dwarf.*` tag, and members spread by Kirby et al. 2011's σ(M★) about
      it, held at 0.32 dex as an extrapolation; four deferred findings are recorded, none a
      blocker (T7.a, T7.b).
    - _5. T5.a's "constant fractional rate"._ _Accepted:_ T5.a releases by the spec's `MassLoss`
      as built, a constant rate, and the plan-text deviation is recorded there (T5.a).
  - _Lead 5's risk, settled:_ the potential freeze from 25 to 26 covers the halo mixture
    (decision-bump-plan-after-22 §2.3), and by ruling 1 nothing of T3.d lands inside it.
- **Deviations in P10.T3.d.a, as built (lane `p10-share`, 2026-10-28, on `9f4a60da` at
  `GENERATOR_VERSION` 23).** No output moved: nothing stores φ (T3.d.b), nothing generated reads
  `Debris`, and `dwarfs::dwarfs` and `dwarfs.golden` keep the unscaled masses. One new unwired
  golden, `galaxy/global_list/discrete_share.golden`, at 23's header; `golden_diff.py` lists it
  alone.
  - _Items._ `global_list::share` (new), with
    `discrete_share(Seed, &GalaxyParams, &PotentialTables) -> f64` at Provides' signature,
    re-exported from `global_list`, and `INSIDE_LY` (65,000 ly, the halo's cut, tested against its
    components' largest cut radius). Crate-visible:
    `ShareFigures` (`of`, `physical`, `applied`, `dwarf_scale`), `scale_cold_masses`, and
    `debris::in_list_order`, Design note 12's dwarf order taken out of `Debris::generate` so that
    both count the same cores. `orphans::MassLaw::mean_solar_masses`, the cut log-normal's mean in
    closed form: 16,284 M☉ for `MASS_LAW`. The bench `global_list/discrete_share` in
    `benches/galaxy.rs`.
  - _The globular term._ 1.5 × plan 02's `AccretionHistory::globular_count` (untruncated, the
    orphans' "expected globulars") × the orphan law's mean mass. Every globular tube counts whole:
    no 65,000 ly cut, though the orphans' radial law reaches 10⁶ ly (T3.b), and no living
    globular's history (Risks, "Living globulars' tube masses").
  - _The dwarf term, against Design note 1's "each recent progenitor's cold mass times the share
    of its orbital period spent inside 65,000 ly" (lean, 2026-10-28)._ A tube counts at its mass ×
    the share of its radial period inside 65,000 ly, by its spec's own quadrature of `dr ÷ v_r` in
    the mid-plane potential (Kepler's `(E − e sin E) ÷ π` to 10⁻⁹ in a test). A core counts whole,
    not by its period share: a core exists only where its progenitor is inside the root cube at
    the epoch (Design note 11), and P10.T12 counts members inside the cube. Only the four cores
    `Debris` numbers (the first in list order) count; cores beyond the limit count nowhere.
  - _The scaling._ `Debris::generate` multiplies each dwarf's tube mass, the tube's progenitor
    (bound) mass, and its core's mass and systems by `(drawn × budget − globular) ÷ dwarfs`,
    clamped to [0, 1], from the `ShareFigures` of the same dwarfs, so it equals what
    `discrete_share` gives whether or not φ is stored (the full tables keep the in-plane ones bit
    for bit; a slow test checks the two shares agree). The orbits, the tidal radius, the stripping
    fraction, the mass and time at accretion and the `MassLoss` stay the progenitor's. Then
    (review fix, lean 2026-10-28) a tube scaled to nothing is dropped, and a core scaled under
    `CORE_MIN_SYSTEMS` too, without letting a fifth core take its number: before the fix a scale
    of 0 left zero-mass cores, on which P10.T7.a's model panics. The share still counts a dropped
    core, so the list serves less than φ says by those cores alone, under 10³ systems each.
  - _Finding: globular tubes are 0.23–7.0% of the halo_ over the 2,000 summary galaxies (median
    1.0%; the Milky Way fixture 0.84%), against T3.d.a's 0.3–1% and Design note 1's "under 1%":
    1,024 of 2,000 lie outside 0.3–1%. Plan 02's globular count follows the dark halo's mass, the
    halo's budget the stars', so a galaxy of low stellar-to-halo mass has many globulars for a
    small halo. **In 14 of 2,000 the globular tubes alone reach the drawn share**: φ is then the
    drawn share, the dwarfs are scaled to nothing, and φ is below what the globular tubes hold, so
    P10.T12's 5% budget test cannot hold there. Pinned provisionally at 0.2–8% for every galaxy
    (`GLOBULAR_SHARE_PIN`, the fast test's sixteen and the slow test's 2,000), the median at
    0.8–1.2%. Ruling deferred.
  - _The summary (v23, seeds `0x0a10_3d00_0000_0000 | n`, n < 2,000)._ Applied share 0.26–14.7%,
    median 1.80%, mean 2.58%: 388 galaxies under 1%, 719 at 1–2%, 661 at 2–5%, 177 at 5–10% and 55
    at 10–15%, so 1,107 of 2,000 fall under the brainstorm's 2% floor, not only those with no
    dwarf (Risks, "The discrete share against the dwarf mass range"). The drawn share binds in 251;
    10 have no dwarf debris inside 65,000 ly. The physical share reaches 405% of the halo (99th
    percentile 72%), Design note 1's reason. The golden holds the 0–100th percentiles and means of
    the applied, physical, globular and drawn shares, those counts, and a digest of every seed's
    applied share in seed order, not 2,000 readable lines: Risks' "recorded per seed" is met in
    bits only.
  - _Cost (finding)._ `global_list/discrete_share` on the Milky Way fixture: 7.5 ms (bench
    profile, pod, 4 vCPU), against "under a millisecond". Nearly all of it is `dwarfs::dwarfs`'s
    epoch states, 60 bisections of the time from pericentre per progenitor, which `Debris` shares;
    T3.d.b's call in `Galaxy::from_params` (about 100 ms) would carry it.
  - _Tests._ Fast, `cargo test -p hyperion-sim --lib -- galaxy::global_list`: in
    `share::tests`, Kepler's time inside a radius, `INSIDE_LY` against the halo's cut, the cap and
    the scale on the fixture and sixteen drawn galaxies, the scale's arithmetic; in
    `orphans::tests`, the mean mass against the quantile's average. Slow, by name:
    `galaxy::global_list::share::tests::the_applied_share_over_2000_galaxies_is_summarised` (the
    golden; 15 minutes in the slow-test profile, pod, 4 vCPU) and
    `galaxy::global_list::share::tests::debris_carries_the_scaled_dwarfs_and_drops_what_the_scale_empties`
    (one galaxy whose drawn share binds and one whose globular tubes reach it: `Debris` against
    `dwarfs` × the scale, the two tables' shares bit for bit; 1 minute: scales 0.58 and 0, the second leaving no dwarf tube or core). The
    slow pins of T3.a–c's record were run at 23 on this lane: `dwarfs.rs`'s sweep gives v15's
    figures (a ≥ 10⁸ M☉ tube in 0.127 of galaxies as drawn and 0.203 on the moved orbits, no core
    in 0.017 and 0.337) and passes, as does the orbit's far-field monopole..
- **Deviations in P10.T7.a, as built (lane `p10-share`, 2026-10-28, on `9f4a60da` at
  `GENERATOR_VERSION` 23).** No output moved: nothing places a core's members (T7.b). The one
  golden touched, `rng/tags.golden`, gains only the appended `dwarf.core.metallicity`; T7.a names
  no golden of its own, so the model's figures are pinned by tests, not bits (T7.b's goldens).
  - _Items._ `global_list::dwarf_core` (new): `DwarfCoreModel::from_spec(&Galaxy, &DwarfCoreSpec)`
    with `number`, `systems`, `position`, `velocity`, `plummer_scale`, `half_mass_radius`,
    `extent`, `dispersion`, `dispersion_at(LightYears)`, `history`, `fe_h`, `fe_h_spread`,
    `table`, `grid`; `StarFormationHistory::{of(mass, accreted), old_share, tail, age_quantile}`;
    `mass_metallicity`, `metallicity_spread`, `half_light_radius`, `velocity_dispersion`, and the
    laws' constants (`SIZE_MASS`, `VELOCITY_DISPERSION`, `MASS_METALLICITY`,
    `MEAN_METALLICITY_SCATTER_DEX`, `METALLICITY_SPREAD`, `METALLICITY_SPREAD_FLOOR_DEX`,
    `MASS_TO_LIGHT_V`, `OLD_BURST_AGES_YR`, `OLD_SHARE`, `FLATTENING`, `HALF_MASS_RADIUS_LIMIT_LY`,
    `EXTENT_LIMIT_LY`, `CORE_GRID_WIDTH_MIN_LOG2`). In plan 09's `features/interior/table.rs`:
    `MemberClassTable::from_classes(&[ClassCount], tidal, flattening)` (no tails, depletion or
    runaways) and crate-visible `grid_width_from` and `grid_from(floor_log2)`, which `grid_width`
    and `grid` call at `GRID_WIDTH_MIN_LOG2`, so every cluster's grid is as it was, bit for bit.
    Made crate-visible: `interior::counts::moments`, `potential::TIDAL_FLOOR`.
  - _`from_spec` takes the `Galaxy`_ with the spec: its in-plane tables (the extent), its mass
    function (the classes) and its seed (the offset).
  - _Size and shape (the plan names no relation; science check, 2026-10-28)._ The Plummer scale
    is the projected half-light radius of `SIZE_MASS`, `log₁₀(R_e ÷ pc) = 0.855 + 0.267 log₁₀ M★`,
    a fit (ours, rms 0.21 dex, not drawn) to Woo, Courteau and Dekel 2008's Table 1 scale lengths
    × 1.678 (Wolf et al. 2010, appendix B), at the core's present mass, held so that the half-mass
    radius, 1.305 a, stays at or under 8,500 ly (reached near 10⁹·¹⁶ M☉). The flattening is 0.67,
    one less McConnachie 2012's median ellipticity (Table 3), a projected figure taken as the
    intrinsic one.
  - _The extent (the plan gives no rule)._ The Jacobi radius at the core's present radius for a
    bound mass `max(M★, 2 σ² r ÷ G)`, the stars' or an isothermal halo's of the observed σ, the
    denominator floored as plan 02's tidal radius floors it, held to the brainstorm's 20,000 ly. A
    dwarf is dark-matter dominated: the stars alone give a radius 2.5 times smaller at 10⁶ M☉,
    inside its own half-light radius; above a few 10⁸ M☉ they give the larger (review fix). With
    no tails the grid is the least power of two from 16 ly that holds the extent: 16 ly up to
    16,384 ly, 32 ly above. Measured on hand-made specs at 20,000–65,000 ly from the fixture's
    centre: a 2 × 10³ M☉ core reaches 491 ly (a 178 ly), 10⁶ M☉ 1,733 ly (934 ly), 10⁸ M☉ 5,302 ly
    (3,195 ly), 3 × 10⁹ M☉ 6,231 ly (6,515 ly, held), every grid 16 ly.
  - _The dispersion, against "the Plummer dispersion" (lean, 2026-10-28)._ A Plummer sphere of the
    stars alone falls 1.8 (NGC 205) to 15 (Canes Venatici I) times short of measured dispersions.
    So σ is `VELOCITY_DISPERSION`, `log₁₀(σ ÷ km/s) = −0.045 + 0.159 log₁₀ M★`, a fit (ours, rms
    0.14 dex) to McConnachie 2012's Table 4 (10⁵·⁴–10⁸·⁷ M☉, extrapolated outside), and
    `dispersion_at` keeps a Plummer sphere's radial shape, `σ₀² (1 + r² ÷ a²)^(−½)` with
    `σ₀² = 16 σ² ÷ 3π`, so that its mass-weighted mean square is σ². Measured dwarf spheroidals'
    profiles are nearer flat (Walker et al. 2007, ApJ 667, L53), a finding for T7.b.
  - _Classes, by plan 09's device but not its `class_counts`_, which is for one age: per band, the
    living stars below each age's turn-off (`stellar::sse::turn_off_mass` at the core's mean
    [Fe/H]) and the white dwarfs, over the history's 64 Gauss–Legendre nodes; band E's neutron
    stars and black holes per primary formed (`retention_tabulated`'s, retained or not), over the
    ages at which an 8 M☉ star has died, so a core accreted in the last 40 Myr leaves out its
    youngest remnants. Nothing is lost. Every class is single and first population and shares the
    one profile: with q = 1 a binary class changes no profile, and multiplicity is the member's
    (T7.b, plan 11). The classes are scaled together to the spec's systems.
  - _Ages, against "a tail that lasts longer the more massive the dwarf" (lean, 2026-10-28)._ The
    burst is uniform over 10–13 Gyr ago; the tail uniform from 10 Gyr ago to the accretion at every
    mass, holding `1 − clamp(0.8 − 0.25 (log₁₀ M★ − 5), 0.3, 0.8)` of the stars at the infall mass
    (Weisz et al. 2014, §4.2: "≳80%" before z ≈ 2 below 10⁵ M☉, "only ~30%" above 10⁷). So the
    tail's weight grows with mass, not its length. _Deferred finding (2026-10-13 rule):_ satellites
    go on forming stars after infall, longer the more massive (Wetzel, Tollerud and Weisz 2015,
    Fig. 3: under 1.5 Gyr below 10⁷ M☉, 2–3 Gyr at 10⁷–10⁸, about 5 Gyr at 10⁸–10⁹), which the
    plan's "ending at accretion" leaves out. _Superseded by main's ruling of 2026-10-30:_ the tail
    ends a quenching time after the accretion ("P10.T7.b as built").
  - _Metallicity, as ruled (issue #123, point 4)._ Eq. 4 at `accreted_mass` plus one 0.17 dex
    normal on `dwarf.core.metallicity`; the science check found eq. 4 fitted over 10³·⁷–10⁸·⁰ M☉
    (Coma Berenices to IC 1613, Table 4), the paper's "unbroken … up to 5 × 10⁸ M☉ (NGC 205)" not
    in the fit, so the doc says it is extrapolated above 5 × 10⁸. Members' σ is Kirby et al. 2011's
    eq. 13 at the infall mass (lean, 2026-10-28: the stars formed before the stripping), fitted
    over 15 dwarf spheroidals of 10³·⁶–10⁷·³ L☉ (10³·⁸–10⁷·⁵ M☉ at 1.6), held at 0.32 from
    10⁷·³⁷ M☉.
  - _Which mass._ Size, extent and dispersion read the present mass (`DwarfCoreSpec::mass`, after
    T3.d.a's scaling: a scaled core is a smaller dwarf, lean 2026-10-28); ages, mean [Fe/H] and
    σ([Fe/H]) read `accreted_mass`. _Finding:_ the core's extent assumes a dark-dominated bound
    mass, while T3.c's tube tidal radius still uses the stellar mass at accretion (T3.c's deferred
    finding); one ruling should cover both.
  - _The tag and a lint._ `dwarf.core.metallicity` (Galaxy): two words, keyed `galaxy_item(6 + j)`
    as `dwarf.orbit` and `dwarf.stripping` are; appended after `class.cell`, since the macro's
    order fixes `ALL` (Provides' "five so far, P10.T3" is now six). It took the registry past
    rustc's deny-by-default `long_running_const_eval` step count: `domain_tags!`'s
    `assert_tag_names` hashes every earlier name again for each name, quadratic in the registry. A
    module-level `#![expect(long_running_const_eval, reason = …)]` on `rng/tags.rs` stands in. The
    fix is in `hyperion-base`, outside this lane: compare each `DomainTag`'s stored hash and the
    names only on equal hashes, then drop the `expect`. **For main**: every lane that adds a tag
    after this needs it or the fix. `McConnachie` is added to the sim's `clippy.toml`.
  - _Tests._ Fast, by
    `cargo test -p hyperion-sim --lib -- galaxy::global_list galaxy::features::interior::table rng::tags`,
    since a bare `galaxy::global_list` filter misses the last two: in `dwarf_core::tests`, the
    metallicity laws, the size and dispersion fits, a 10⁸ M☉
    core's 5–95% ages over more than 2 Gyr for accretion at 0–6 Gyr, the density integrated over
    space (spherical quadrature) equal to the spec's systems to 10⁻³ inside an extent under 20,000
    ly, every remnant kept with one profile and the dispersion's mean square, every core of six
    drawn galaxies modelled, purity, the offset's own stream bit for bit and order independence;
    in `table.rs`, `a_grid_tried_from_a_floor_is_the_rule_s_from_there`.
- **T5.a–b as built (lane `p10-spray`, 2026-10-28, on `9f4a60da` at `GENERATOR_VERSION` 23; T5.a
  at `75ab070` before landing).** No output moved: nothing places the spray's tracers yet (T6,
  T8). Two tags, `stream.spray.release` and `stream.spray.offsets` (`Galaxy`, keyed by
  `ObjectKey::galaxy_item` of the stream number, Design note 14), are appended after P09.T32's
  `class.cell`, so `rng/tags.golden` is extended only; new unwired golden
  `galaxy/global_list/spray.golden` (the spray table at three points, the three test streams'
  specifications and sprays of four tracers an arm), which moves with the potential, the step
  rule, the release offsets and the table, and is re-blessed then. Every figure below is v23's,
  x86-64, measured in the Lightchord pod (4 vCPU).
  - _Items._ `global_list::spray::{SprayParams (MAX_TRACERS_PER_ARM), TRACERS_PER_ARM (1,000),
ReleaseSchedule, Release, Spray, Tracer, Arm, SprayForce, NODES (70), spray, RADIAL_OFFSET,
TANGENTIAL_OFFSET, VERTICAL_SPREAD, IN_PLANE_SPREAD, FILLING_FACTOR, OFFSET_CUT, PLUMMER_SCALE,
OFFSET_WORDS}`, beyond Provides' three. `spray(&Galaxy, &StreamSpec) -> Result<Vec<Tracer>,
BuildLeapfrogError>` builds the galaxy's `SprayForce` each call; `Spray::new(seed, &SprayForce,
&PotentialTables, &StreamSpec, &SprayParams)` integrates the progenitor and holds the schedule,
    and `Spray::tracer(index)` integrates one tracer, a pure function of its index (tracer `2r` is
    release `r`'s leading one, `2r + 1` its trailing one), so a caller's pool runs them and gathers
    them by index (ruling 146.3). The sim spawns no threads: no pool runs them yet, and
    `TubeTable::build(&Galaxy, &StreamSpec)` (T6) runs them in turn unless its sketch takes a pool.
    `global_list::testing::{pal5_like, gd1_like, sagittarius_like}(&PotentialTables)`, stream
    numbers 0–2, behind `cfg(any(test, feature = "testing"))`.
  - _The registry's lint._ `stream.spray.release`, the registry's 193rd tag, pushed the
    `domain_tags!` name check past rustc's deny-by-default `long_running_const_eval`, since it
    hashes every earlier name again for each name. T5.a first made `hyperion-base`'s
    `assert_tag_names` hash each name once, with a 400-name `const` test that the old check fails;
    by main's ruling of 2026-10-29 that change is reverted (a fix comparing stored hashes is queued
    separately), and the registry takes P10.T7.a's module attribute instead, word for word:
    `#![expect(long_running_const_eval, reason = "the registry's name check is finite; it only
grows with the registry")]`.
  - _Outside plan 10's files._ Plan 02's `potential/tables.rs`: `grid_point` (now to index
    255), `hermite`, `hermite_slope`, `FIRST`, `LN_FIRST` and `STEP` crate-visible, re-exported
    as `GRID_*`; `orbit.rs`: `potential` split into `potential_ly` (the same operations, so
    `orbit.golden` holds) and `Leapfrog::potential_and_gradient`, `dot`, `cross` and the SI factors
    `pub(super)`; `dwarfs.rs`: `epoch_state`, `pericentres_within` and `remaining` `pub(super)`.
    T2.c's source scan reads `spray.rs` and `spray/force.rs` too.
  - _T5.a, the schedule._ Stratified in mass: release `i` of `N` at the time the mass lost reaches
    `(i + Uᵢ) ÷ N` of the tube's, `Uᵢ` word `i` of `stream.spray.release`, rounded to the nearest
    step boundary of the spec's `fixed_step` over `T_s` (an orphan's no later than the last before
    its dissolution); one tracer from each Lagrange point a release, each `M ÷ 2N`, so the weights
    sum to the tube's mass to 10⁻¹². The bound mass at release `i` is `m₀ + M (1 − uᵢ)`, so an
    orphan's cluster shrinks to nothing as it dissolves. _Deviation (lean, 2026-10-28):_ a dwarf's
    pulses fall on the integrated orbit's pericentres (the boundaries nearer the centre than both
    neighbours on the forward pass), not on the spec's spherical times, since the spherical period
    drifts from the integrated one (T3's cross-check, Ω̄ 0.87–1.00 of the quadrature's); the
    spec's count of the most recent carry `(1 − f)^−(k+1)` each, and with none inside the
    stripping time the spherical ones stand in. Each pulse is released at its pericentre's boundary
    (Design note 11's "at each pericentre"), not spread over the passage.
  - _T5.b, the release._ Fardal, Huang and Weinberg 2015 (MNRAS 452, 301), checked at source:
    eq. 1's tidal radius, which is plan 02's `(G m ÷ (4Ω² − κ²))^⅓`, at the bound mass and radius
    of the release; eqs. 3–9 with `k_φ = k_vr = 0`, Ω the circular frequency at the progenitor's
    radius, and eq. 7's printed `k_z r_tidal ÷ r` read as `k_z r_tidal` (gala reads it so);
    `k̄_r = 2.0`, `k̄_vφ = 0.3` (eqs. 19–20); `σ(k_z) = σ(k_vz) = 0.5` (eqs. 22–24);
    `σ(k_r) = σ(k_vφ) = min(0.15 f_t² R_acc^⅔, 0.4)` (eq. 21) with `R_acc` from the spec's turning
    points (0.278, 0.249 and the cap 0.4 for the three test streams). _Leans (2026-10-28):_
    `f_t` = 0.8, the five suffix-less runs of their Table 1, since no progenitor has a King
    radius; each offset cut at ±2.5σ (their Gaussians are "unlikely to be accurate in the
    extremes"), which keeps `k_r + k_vφ ≥ 0.3`; the Plummer scale 0.11 of the spec's tidal radius
    (Hénon's maximum `r_h ÷ r_J` of 0.145, Gieles, Heggie and Zhao 2011, §2.2, over Plummer's
    1.305), a choice with no source for a dwarf; the Plummer sphere acts while the bound mass is
    above zero, which reads "a living progenitor" as a living globular always, an orphan until it
    dissolves and a dwarf whose core remains, its mass the stellar mass (P10.T3.c's finding).
    _Finding (science check):_ Fardal et al. fitted their offsets to tracers in the host's
    potential alone (§5.2), from satellites of about 3.5 × 10⁻⁶ of the host's mass, so the
    sphere moves the releases away from their calibration, most for a massive dwarf.
  - _T5.b, the force (ruling 146.3)._ `SprayForce`: the leapfrog's potential, far field and blend
    included, on the tables' 64 nodes a log axis continued at their spacing to 70, the last at
    2^20.095 ly, the first node past 2²⁰ ly; `Φ`, `R ∂Φ ÷ ∂R`, `z ∂Φ ÷ ∂z` from the leapfrog and
    the cross derivative by central differences in the logarithms (2⁻¹² each way); bicubic
    Hermite with its exact gradient. _Lean:_ held flat within 2⁻⁴ ly of the plane and the axis
    (the vertical force there is 9 × 10⁻⁵ of its value 1,000 ly up), and continued beyond the last
    node as `A ÷ h` from the clamped point, as plan 02 continues its grid (P02.T6.f). Against the
    leapfrog over 1–300 kpc at nine latitudes: the potential to 1.05 × 10⁻⁵ and the force to
    2.54 × 10⁻⁴ (asserted at 2 × 10⁻⁵ and 10⁻³); its own gradient to 10⁻⁶, beyond the last node
    included. Build 25 ms, 157 kB; a lookup 260 ns against the leapfrog's 466 ns (the dev
    profile's `opt-level` 2).
  - _T5.b, the integration._ The progenitor goes back over `T_s` and forward again in the table
    (alone, no Plummer sphere), returning to its epoch state to 6 × 10⁻¹² of its radius
    (`pal5_like`); tracers step in the table plus the Plummer sphere at each boundary's progenitor;
    ψ is each body's summed `math::atan2` swept angle, the tracer's less the progenitor's over the
    same steps.
  - _The test streams._ `pal5_like`: the Baumgardt–Hilker catalogue's Pal 5 (1.34 × 10⁴ M☉, born
    10^4.78, 8.88–17.54 kpc), Harris's [Fe/H] −1.41, 11.5 Gyr, losing `4.1 × 10⁻⁶` M☉ a year,
    just past apocentre on a plane inclined 70°: `T_s` 10 Gyr (the last major merger; its wrap time
    is longer), 20,474 steps of 0.49 Myr, a tube of 4.1 × 10⁴ M☉. `gd1_like`: Koposov, Rix and Hogg
    2010 (14–26 kpc, retrograde at 39°, 9 Gyr, [Fe/H] −1.9), 7 × 10³ M☉ (Bonaca and Price-Whelan
    2024), dissolved halfway through `T_s` = 9 Gyr (its age), 10,858 steps. `sagittarius_like`: Law
    and Majewski 2010 (19–59 kpc, a plane at 76°, 6.4 × 10⁸ M☉ taken as the stellar mass at
    accretion 6 Gyr ago), 0.3 lost a pericentre: `T_s` 2.75 Gyr (its wrap time), three pulses,
    a core of 5.3 × 10⁷ and a tube of 1.0 × 10⁸ M☉. The stripping times are Design note 5's, so
    `pal5_like`'s ψ runs to ±3.5 rad: **its tube will be far longer than P10.T6.a's "near 82,000
    ly"** (the review estimates 300,000 ly), which looks set for a stripping time near 3 Gyr, the
    `FixedStep` example's; T6 measures it.
  - _Tests._ Fast: `spray::tests` (the schedule's weights, the orphan's last release, the steady
    and pulsed histories, determinism; on the Milky Way fixture, one test, the golden, the table's
    gradient and accuracy, `assert_order_independent` over the three streams' tracers, the weights
    and the pairs) and `spray::force::tests`, in the lib so that the fast suite compares the
    golden on `wasm32-wasip1`, whose run turns no feature on. Slow, in `tests/global_list_spray.rs`
    (`testing`), by name:
    `the_test_streams_arms_lead_and_trail_and_pal5_s_track_leaves_its_orbit` (about 20 s) and
    `a_drawn_galaxy_s_streams_keep_nearly_every_tracer_on_its_arm_s_side` (about 60 s). ψ grows
    with the time since release in every arm (groups of a tenth, a pulse one group);
    `pal5_like`'s mean track departs from its orbit by 39.0 tidal radii within one wrap (bound
    6), `gd1_like`'s by 12.2, `sagittarius_like`'s by 9.0. Commands:
    `cargo test -p hyperion-sim --lib -- galaxy::global_list rng::tags`,
    `cargo test -p hyperion-sim --test foundation_golden`, and
    `cargo test -p hyperion-sim --features testing --test global_list_spray -- --ignored`.
  - _Finding: not every tracer is on its arm's side._ T5.b's "positive for every leading and
    negative for every trailing tracer" holds for `pal5_like` and `gd1_like`; `sagittarius_like`
    has 27 of 2,000 on the other side (39 under another seed and plane) and none with the Plummer
    sphere left out, so it is the 10⁸ M☉ progenitor's pull on tracers released near its Jacobi
    radius; a drawn Milky Way galaxy's streams (five orphans at the step counts' quantiles and its
    one dwarf) have 13, 0, 24, 20, 13 and 32 of 2,000, from eccentric orbits, where eq. 21's
    dispersion reaches its cap and a trailing tracer released near pericentre with `k_vφ` low in
    its range leaves with less energy than the progenitor. Pinned as found: 60 for
    `sagittarius_like`, 2.5 % a stream. P10.T6.a sorts each arm by ψ, so these tracers sit at
    their arm's root unless T6 splits the arms by ψ's sign; for the owner, with whether the
    Plummer sphere stays for dwarfs.
  - _Finding: the cost._ A spray costs about 0.32 µs a tracer-step, about 1,000 × the step count
    for a stream. A drawn Milky Way galaxy's 309 streams take 5,000–49,120 steps (median 12,969;
    4.96 × 10⁶ in all, nearly every one at the 10 Gyr cap of `T_s`), so a stream takes 1.7–29 s
    alone (median 4.6 s) and 0.7–7.7 s on four threads (median 1.9 s), and `pal5_like` 6.6 s alone,
    against P10.T6.c's 0.2 s of parallel wall time for a tube: 4–40 times over on four threads, 10
    at the median.
    Ruling 146.3's fallback, `τ_p ÷ 32`, would halve it; not applied. For the owner, with T6.c's
    bench.
- **P10.T5.b, the spray's fix, as built (lane `p10-t6a`, 2026-10-29, on `7a674ea` at
  `GENERATOR_VERSION` 23).** Main's rulings of 2026-10-29, after the research lane
  `research-galaxy-leans` (items 10, 12 and 13). No output moved but the unwired `spray.golden`,
  re-blessed by name (its 168 tracer values; the table's, the specifications' and the releases'
  lines are unchanged; x86-64), which `golden_diff` names alone. Every figure is v23's, x86-64,
  measured in a pod (4 vCPU).
  - _The Plummer sphere is dropped, for every progenitor (item 10)._ Fardal, Huang and Weinberg
    2015, §6: the progenitor "can be omitted with an appropriate choice of release constants
    (equations 19–24); the time-integrated effect of the progenitor is encapsulated by our larger
    action offset", so their `k̄_r` = 2 and `k̄_vφ` = 0.3 with a sphere counted its pull twice; their
    §5.2 tested the host-only recipe on a Sagittarius of 8.5 × 10⁻⁴ of its host's mass ("the
    agreement is extremely good"), and the brainstorm has no sphere. `PLUMMER_SCALE` and the
    track's bound mass for the force are removed; the bound mass still sets each release's tidal
    radius (eq. 1). This supersedes "T5.a–b as built"'s leans on the Plummer scale and on "a living
    progenitor", and its science check's finding.
  - _The cut keeps the fitted dispersions (item 13)._ `spray::cut_widening()`,
    `1 ÷ √(1 − 2a φ(a) ÷ (2Φ(a) − 1))` at `a` = 2.5, 1.04756: each offset's Gaussian is widened by
    it, so that cut at ±2.5 of its dispersions (which drops 1.24 % of draws and leaves 0.9113 of
    the variance; ±2.619 of the fitted dispersions) it keeps eq. 21's and eqs. 23–24's
    dispersions; `k_r + k_vφ` stays at 0.204 or more.
    `Spray::in_plane_spread()` is still eq. 21's σ. `f_t` = 0.8 stays (item 12), its documentation
    citing §5.2's Sagittarius run beside Table 1. Tests:
    `spray::tests::the_widening_restores_a_cut_gaussian_s_variance` (the cut distribution's
    variance by a stratified midpoint rule, to 10⁻⁸), and, in the Milky Way fixture's test, the
    variances of 20,000 tracers' drawn offsets to 3 % and every offset within the cut;
    `spray.golden` pins `cut_widening`'s bits.
  - _Tracers on the other arm's side, measured again._ `pal5_like` 0, `gd1_like` 0 and
    `sagittarius_like` 1 of 2,000 (27 with the sphere); the drawn galaxy's six sampled streams 0
    each (13, 0, 24, 20, 13 and 32 with it). So the drawn orphans' strays, which "T5.a–b as built"
    put down to eccentric orbits, came from the sphere too, its pull integrated over the long time
    a slowly drifting tracer spends near its progenitor. The slow tests now allow 10 for
    `sagittarius_like` (was 60) and 0.5 % of a drawn stream's tracers (was 2.5 %). The tracks
    depart from their orbits within one wrap by 35.3, 11.2 and 9.4 tidal radii (were 39.0, 12.2
    and 9.0).
  - _For the owner._
    - Chen, Valluri, Gnedin and Ash 2025 (ApJS, doi:10.3847/1538-4365/ad9904; arXiv:2408.01496)
      find host-only Fardal streams 15–50 % too narrow for globulars (15–30 % at `e` = 0, 30–50 %
      at `e` = 0.6), with `f_t` = 1, and their own host-only variant fails to separate the arms
      in R and μ_φ1, "highlighting the importance of including the progenitor's potential". They
      give a release distribution calibrated with the progenitor's gravity for 10⁴–10⁶ M☉ and
      `e` ≤ 0.6 (gala's `ChenStreamDF`): the globulars' option, a new release model.
    - Fardal et al. calibrated on satellites of 10⁶ M☉, 3.5 × 10⁻⁶ of their host (§3, Table 1),
      and tested beyond that range on a Sagittarius of 8.5 × 10⁻⁴ (§5.2); their §6 says that at
      very large satellite masses the satellite's force must be included explicitly. Dwarfs above
      about 10⁻³ of the host's mass, up to 10⁹·⁵ M☉, lie beyond every test of the recipe, and
      beyond the linear frequency expansion (§2.2, after Bovy 2014). The ruling drops the sphere
      for them too: a conflict between the source's caveat and the ruling, recorded, not acted on.
    - The Fardal quotes and numbers are checked against arXiv:1410.1861v1; the refereed text was
      out of the pod's reach.
- **P10.T6.a as built (lane `p10-t6a`, 2026-10-29, on `7a674ea` at `GENERATOR_VERSION` 23).** No
  output moved: nothing reads a tube before P10.T8, and the tube goldens are T6.c's, so no golden
  is added. Main's rulings of 2026-10-29 (items 11 and 15 and the findings for T6.a, after the
  research lane `research-galaxy-leans`) are in the task's text. Every figure is v23's, x86-64,
  measured in a pod (4 vCPU).
  - _Items._ In `global_list::tube`: `TubeTable`, `TubeArm`, `TubeKnot`, `BuildTubeTableError`,
    `KNOTS_PER_ARM`, `KERNEL_TRACERS` and `KERNEL_WIDTH`. `TubeTable`'s `build`, `from_tracers`,
    `stream`, `mass`, `arm`, `knots`, `length`, `locate`, `mean_velocity` and `psi`;
    `TubeArm::{knots, tracers, mass}`; `TubeKnot`'s `centre`, `frame`, `velocity`, `widths`,
    `dispersions`, `cumulative_mass` and `psi`;
    `BuildTubeTableError::{Spray, TooFewTracers}`. `spray::Arm` names a tube's arms too.
    _Deviations from Provides:_ `TubeTable::build(&Galaxy, &StreamSpec)` returns
    `Result<TubeTable, BuildTubeTableError>`, since the spray fails on a galaxy without the full
    potential; `from_tracers(&StreamSpec, &[Tracer])`, the pool's entry, takes no `&Galaxy` (T6.b
    adds what its noise needs). `KnotHash`, `byte_size` and `cells_touching` stay T6.c's and T8's.
    _For T9 and T10.b:_ `TubeLookup::tube` cannot fail, so `TubeTableCache` must `expect` the
    error, which is safe for a galaxy with the full potential, as the server builds it (no stream
    of a drawn galaxy's 309 met `TooFewTracers`), or the trait returns a `Result`. `locate` takes
    across coordinates within `ACROSS_LIMIT`, 1,000 widths, and makes the interpolated normal and
    binormal unit vectors again, as the widths' own measurement does (on `sagittarius_like`'s
    54° turns the plain interpolation would shorten them to 0.89 at mid-segment).
    `tube::tests::tracers_of_equal_psi_keep_their_order` pins the stable sort, which a spray's
    last release on the epoch's boundary needs (its pair has ψ of +0.0 both). For T6.c: a tube
    golden must be a lib unit test (the `wasm32-wasip1` run turns `testing` off), at 48 tracers an
    arm or more (`KERNEL_TRACERS`), or on the synthetic tracers of `tube::tests`; and its
    order-independence test is still to come.
  - _The arms (item 11)._ Superseded by the energy's sign ("P10.T6.a's arms by energy, as built").
    ψ ≥ 0, either zero, is the leading arm's, ψ < 0 the trailing arm's;
    the tracers are sorted by ψ with `total_cmp`, stably, so the table is a pure function of the
    list in index order. Each arm's knots run in increasing ψ, the along coordinate's order: the
    trailing arm's from its tip to its root.
  - _Knots (Design note 6)._ 256 an arm at equal steps of its cumulative weight, counted to each
    tracer's middle, the end knots at the arm's ends; the kernel is the 48 tracers nearest in
    rank, Gaussian with σ = 12 ranks. _Leans (2026-10-29):_ σ = 12, so that the 48 span ±2σ (the
    plan makes the width a parameter of the generator version without a value); the centre line
    and the mean velocity are the kernel's local-linear averages (the intercept of the
    kernel-weighted line in rank, its mean where the tracers lie evenly about the knot), since at
    an arm's end the plain mean lies inside the end and turns the mean velocity there: on a
    synthetic circle it put 2.4 times the dispersion along the normal at the tips; ψ at a knot is
    the tracers' own at its rank, the inverse of their Λ(ψ), not a kernel mean, so the knots keep
    their order and their arm's sign. The widths and dispersions are kernel means of the squared
    offsets about the centre line and the mean velocity interpolated at each tracer's own rank.
    The reduction costs 2.8 ms a stream in release, against the spray's 4.30 s serial for
    `pal5_like` (210 ns a tracer-step; `gd1_like` 3.44 s, `sagittarius_like` 0.54 s; a scratch
    probe, not committed), so T6.c's budget is the spray's.
  - _Frame._ The tangent is the kernel fit's direction (on the synthetic stream the binormal
    strays 2.8° from the pole at most, against 9° with the chord through the neighbours, the worst
    at the arms' ends); the normal is carried by the double reflection of Wang, Jüttler, Zheng and
    Liu (2008, ACM Trans. Graph. 27(1), article 2, §4: fourth order for exact tangents, but noisy
    tangents set its error here) from the trailing arm's root, where the binormal starts on the
    orbit's pole, and made perpendicular to each tangent; orthonormal to 10⁻¹². From knot to knot
    the normal turns by at most 6.4° on `pal5_like` and 7.5° on `gd1_like`, but by 54° on
    `sagittarius_like`, inside its most recent pulse, 448 tracers some 1,500 ly wide over 22,000
    ly of track, where the fitted direction is noise; it never flips. The binormal strays from each
    knot's own orbital pole by at most 11.7° on `pal5_like` and 6.9° on `gd1_like` (their planes
    precess), so the two widths are near the in-plane and vertical ones; the cross-section's tilt
    is not stored. The local-linear fit is Fan and Gijbels's (1992, Ann. Statist. 20, 2008–2036),
    an end knot's fit resting on an effective 7 tracers against an inner knot's 39.
  - _Lengths (the restated test)._ The knots' path over the bins' stretch against the tracers'
    48-tracer bins: `pal5_like` 217,410 against 218,861 ly (−0.66 %), `gd1_like` 112,540 against
    113,681 ly (−1.00 %), `sagittarius_like` 724,505 against 711,460 ly (+1.83 %). From tip to
    tip, which the bins stop about 24 tracers short of where the oldest tracers lie sparse:
    281,431, 135,899 and 814,020 ly, and over the central 90 % of the mass 157,363, 91,641 and
    609,015 ly. _Finding, for the owner:_ against the brainstorm's 30,000–160,000 ly for globular
    streams, `pal5_like` lies above from tip to tip (its stripping time is the 10 Gyr cap, Design
    note 5) and inside over its central 90 %, and `gd1_like` inside; the detected Pal 5 tails span
    some 20° (about 25,000 ly) and GD-1 some 100°, so a length by a mass quantile is the fairer
    comparison; a drawn Milky Way galaxy's 308 globular
    streams (seed `0x0a10_3000_0000_0000`) measure 16,641–590,021 ly from tip to tip, median
    108,500 ly, 226 inside the bracket, 78 above and 4 below.
  - _Widths and dispersions (recorded, not tuned)._ The medians over the knots: `pal5_like`'s
    widths 156 and 252 ly, inside 120–380 ly (asserted), and dispersions along the tangent, the
    normal and the binormal 0.55, 0.81 and 1.11 km/s against 0.9–3.5 km/s; `gd1_like`'s widths 52
    and 56 ly against 100–260 ly and dispersions 0.16, 0.49 and 0.24 km/s against 0.7–1.3 km/s.
    The misses are pinned as found, to a quarter; `f_t` = 1 is not pulled. _Finding, for the
    owner_ (the science check, with sources read through archived copies): a width here is the
    rms offset, a Gaussian σ, as observers fit one. GD-1's observed σ is about 65–114 ly (Koposov,
    Rix and Hogg 2010; Valluri et al. 2025, arXiv:2407.06336), so the plan's 100 ly floor sits
    above two of three readings, and `gd1_like`'s 52–56 ly is 0.7–0.85 of it, as Chen et al.'s
    15–50 % narrow host-only streams predict ("P10.T5.b, the spray's fix, as built"); GD-1's
    dispersion is an upper limit, about 1 km/s or less (Bonaca et al. 2020, arXiv:2001.07215),
    so the plan's 0.7 km/s floor has no detection behind it; Pal 5's widths match (σ about
    0.1–0.15°, Bonaca et al. 2020, arXiv:1910.00592), its observed dispersion is 2.1 ± 0.4 km/s
    (Kuzma et al. 2015). The dispersions agree with the widths through the epicyclic and vertical
    frequencies (`pal5_like` 0.85 and 0.97 km/s predicted, 0.81 and 1.11 measured), so the misses
    are the spray's, not the reduction's. Read as a full width at half maximum (2.355σ),
    `gd1_like`'s widths would lie inside 100–260 ly and `pal5_like`'s second above 380: which width
    the brackets mean is the owner's.
  - _The gap and the pile-up._ `gd1_like`'s central 5 % of ψ holds 0.0033 of its mass against
    0.05 in an even tube (asserted under 0.005), `pal5_like`'s 0.198. `sagittarius_like`'s
    centre line has one apocentric point, at about 162,000 ly, and two pericentric, at about
    66,000 and 60,000 ly; its oldest pulse, 914 tracers, spans them all, with 4.0 × 10⁻⁶ of its
    tracers a light-year within 20,000 ly of the apocentre against 5 × 10⁻⁸ and 3 × 10⁻⁸ near the
    pericentres. The two later pulses span no apocentric point, so the test compares the pulses
    that span both, and requires one: so one pulse of three, not "each". Both pericentric points
    lie within 0.7 rad of the progenitor, between the oldest pulse's two clumps (drifted some 1.6
    and 2.4 rad from it), so the contrast, some 100 times against an r_a ÷ r_p of about 2.5, is
    mostly the pulse's own clumping, though its leading clump peaks at the apocentre. For the
    owner: a dwarf with a longer stripping history would test the pile-up itself; `pal5_like`'s
    steady release cannot, its track running from a pericentre at one tip through its one
    apocentre to a pericentre at the other, outside the central 90 % of its tracers.
  - _Strays (item 11's test)._ `sagittarius_like` has one stray under the tests' seed and one
    under eight more seeds, at 4.78 and 4.53 widths out: beyond the tube's cut at four widths,
    where 1–3 of each spray's 2,000 tracers lie, and within the reach of its other tracers (8.3
    and 14.2 widths). _Main's ruling (2026-10-30, after the research lane `research-p10-t6a`):_
    the test reads "inside the other arm's tube at its ψ" as within its four-width cut there or
    within two tidal radii of the progenitor at the epoch, where the arms' roots meet; both
    strays lie at 0.80 and 0.88 tidal radii, and the strays weigh at most 0.5 % of a tube
    (asserted); `pal5_like` and `gd1_like` have none.
  - _Finding: ψ's sign fails for eccentric orphans near apocentre._ A census of the same drawn
    galaxy, every stream sprayed (6 minutes on four threads in release-like `slow-test`, a scratch
    probe, not committed): 40 of its 309 streams have strays, 17 more than 2 %, up to 1,219 of
    2,000. Every one of the 17 is an orphan with `e` 0.79–0.96, 410,000–960,000 ly out at the
    epoch, near apocentre, the central 90 % of its tracers within |ψ| of 0.004–0.055 rad: they
    have hardly drifted (T3's frequency spread is about 2 r_t ÷ r_p), and ψ's swing through the
    orbit hides the sign of the drift. Their tubes are blobs ordered by ψ, 45,000–330,000 ly of
    knots for debris some 10⁴ ly across. None reaches the root cube (118 of the 309 tubes do), so
    no member would be placed in one, and nothing is shown before P10.T11. Not a blocker; for the
    owner and for T6.c and T11: such a tube needs another along coordinate (the tracers' orbital
    phase or energy), or the list keeps it unreduced until it shears. Resolved for the split by
    main's ruling of 2026-10-30 ("P10.T6.a's arms by energy, as built"); such tubes stay blobs.
  - _Tests._ Fast, `tube::tests` on a synthetic stream on a circle (`Tracer::from_parts`,
    test-only): the split by ψ's sign with strays and zeros of both signs, the centre line, Λ and
    the length, the frames, the widths and dispersions, `locate`, `mean_velocity` and `psi`
    against the knots, purity and order, and the error. Slow, in `tests/global_list_tube.rs`
    (`testing`), by name, about 16 s in the `slow-test` profile after its build:
    `the_test_streams_tubes_follow_their_tracers`,
    `the_globular_streams_widths_and_dispersions_are_pinned`,
    `an_orphan_s_tube_holds_its_cluster_s_gap`, `a_dwarf_s_pulses_pile_up_at_apocentre` and
    `the_strays_lie_inside_the_other_arm_s_tube`. Commands:
    `cargo test -p hyperion-sim --lib -- galaxy::global_list` and
    `cargo test -p hyperion-sim --features testing --test global_list_tube -- --ignored`.
- **P10.T3.d.a's cost, cut bit for bit (lane `p10-fixes`, 2026-10-29, on `13b8b05` at
  `GENERATOR_VERSION` 23).** Research `research-p10`'s item 8, as main ruled it (2026-10-29). No
  output moved. Every golden is unchanged and passes, and a release probe, run by this lane over
  the change, gave the same bits before and after for the shares of the first 100 sweep galaxies
  and for the whole `Debris` (every field, by its `Debug` form) of the Milky Way fixture and sweep
  galaxies 3 and 11: 789 streams and 7 cores.
  - _What changed._ `PotentialTables::potential_in_plane` sums Φ alone, in `totals`' order, through
    a new private `InPlaneTable::potential_at` (`at`'s three branches for Φ only): it had also
    evaluated `v_c²` and its slope for the in-plane table and all three spherical components.
    `SphericalOrbit::state_after_pericentre`'s bisection integrates the time alone (a new private
    `panel_time`, `panel_integrals(…).0`'s arithmetic) and leaves once the midpoint equals an end
    of the interval, after which every later step keeps it; `share_inside` calls a time-only
    `radial_time` (crate-visible). An orphan's attempt tests its turning points before it builds
    the orbit (`SphericalOrbit::between` is pure). Two fast tests hold each copy to what it copies,
    bit for bit (review fix): `potential::tables`'s
    `the_potential_in_the_plane_is_the_totals_bit_for_bit` in all three branches, and `spec`'s
    `the_time_alone_is_the_integrals_time_bit_for_bit`, against the old 60 full bisections.
  - _Costs (release, pod, 4 vCPU)._ `discrete_share`: the fixture 7.50 → 1.33 ms; 40 sweep
    galaxies a mean of 54.7 → 9.3 ms (median 56.0 → 9.4, at most 115 → 20); 7.2 → 1.2 ms a recent
    progenitor, of which the fixture has one and a drawn galaxy eight on average. One
    `potential_in_plane` 1.2 → 0.31 µs (`research-p10`'s figure). `Debris::generate`: the fixture
    (264 streams) 1.56 → 0.16 s, sweep galaxy 3 (155) 0.89 → 0.11 s, sweep galaxy 11 (370) 2.22 →
    0.24 s; `Galaxy::with_full_potential` 7.5 s each, unchanged. `Debris` no longer threatens T4's
    "about one second" for the list.
  - _The target._ T3.d.a's "evaluating it costs under a millisecond" is restated as about 10 ms for
    a drawn galaxy and 1.3 ms for the fixture. Under a millisecond would need a Newton inversion
    of the time from pericentre or fewer panels, either of which moves `dwarfs.golden`: left to the
    bump that corrects `ORBIT_APOCENTRE`. The bench group `global_list` gains
    `discrete_share (sweep galaxy 0)`, the summary's first seed, which has eight recent
    progenitors (9.7 ms in the probe).
  - _Tests run (pod, x86_64)._ Fast: `cargo test -p hyperion-sim --lib -- galaxy::global_list galaxy::potential
galaxy::kinematics galaxy::params galaxy::features::interior::table rng::tags` (115 passed,
    `dwarfs.golden` among them), `--test global_list_orbit` (`orbit.golden`) and `--test
galaxy_potential`. Slow, by name, in the slow-test profile:
    `the_applied_share_over_2000_galaxies_is_summarised` (`discrete_share.golden` unchanged, 683 s
    against the 902 s recorded before), `debris_carries_the_scaled_dwarfs_and_drops_what_the_scale_empties`
    (44 s), `tests/global_list_debris.rs` (65 s) and `tests/global_list_orbit.rs`' eleven.
- **P10.T3.b's orphans keep their radius (lane `p10-fixes`, 2026-10-29, on `13b8b05` at
  `GENERATOR_VERSION` 23).** Research `research-p10`'s item 6, change 1, as main ruled it
  (2026-10-29). An orphan keeps the first radius it draws outside corotation and redraws only
  its direction and velocity after it, which is T3.b's "drawn from the globular system's radial
  law … redrawn on the same stream until the pericentre is outside corotation" read as written.
  Each attempt still reads its radius word first, so every block of `ORBIT_WORDS` keeps its
  layout, and an orphan accepted at the attempt that first keeps a radius is unchanged. Only
  unwired output moved, `Debris`' orphans, and no golden: `dwarfs.golden` and
  `discrete_share.golden` read no orphan, and the orphans have none.
  - _Why._ As first built the radius was redrawn with the velocity, so the radii kept were those
    where a radially biased velocity cleared corotation, mostly far out. In the Milky Way fixture
    and sweep galaxies 0–11 (a release probe, pod, 4 vCPU) 0.13–0.31 of the orphans' tube mass
    (mass-weighted share of their radial periods, median 0.23) lay inside 65,000 ly, and
    0.10–0.22 of them at the epoch, against 0.50 of the radial law outside corotation (0.70 of
    the whole law) and 0.82 of the Milky Way's metal-poor globulars inside 20 kpc (Harris 2010,
    as `POOR_PLACED_SHARE` cites it).
  - _Measured, the same thirteen galaxies._ The orphans' tube mass inside 65,000 ly 0.43–0.59
    (median 0.53); 0.43–0.58 of them inside at the epoch; all the globular tubes, the living
    included, 0.53 in the mean, from 0.24. `discrete_share` is unchanged: it draws no orphan
    (its globular term follows in the next record).
  - _Finding: about 1% of orphans are dropped._ A radius kept just outside corotation leaves only
    nearly circular velocities to clear it, which 256 attempts may not find: 46 of 4,844 orphans
    over the thirteen galaxies (0.95%; 0.4–1.7% a galaxy, the most where corotation lies
    farthest out, 23,393 ly in sweep galaxy 6), 2 of 259 in the fixture, where none were before.
    The share's globular term, 1.5 tubes per expected globular, then stands about 1% above the
    tubes drawn, inside P10.T12's 5%. Not a blocker; recorded.
  - _Tests._ `tests/global_list_debris.rs`'s slow test gains a Kolmogorov–Smirnov test of the
    orphans' radii at the epoch against the radial law outside corotation (α = 10⁻³), which the
    first rule fails. Over the eight fixture seeds (slow-test profile, pod, x86_64): D 0.019 over 2,104 orphans, p
    0.41, 1,066 inside 65,000 ly at the epoch; the first rule, the same test, fails it at D 0.378
    over 2,136 (p 2 × 10⁻²⁶⁶), 272 inside. The test's other pins hold: 0.036 of the globulars
    qualify, the orphans' median mass 10^4.001 M☉, and the leapfrog's cross-check, now on orphans
    nearer corotation, gives Ω̄ 0.762–1.000 of the quadrature's (median 0.961) and a pericentre
    0.968–1.185 of the spherical one (median 1.009), none inside corotation. Fast: `cargo test -p
hyperion-sim --lib -- galaxy::global_list`, 35 passed. For P10.T4: no golden pins the orphans'
    bits, and T4's golden of the first ten entries may hold none (the living globulars come first),
    so T4's golden and its order-independence test should cover orphans (determinism review).
- **P10.T3.d.a's globular part and the ceiling (lane `p10-fixes`, 2026-10-29, on `13b8b05` at
  `GENERATOR_VERSION` 23).** Research `research-p10`'s item 6, changes 2–4, as main ruled it
  (2026-10-29), after T3.b's fix above. It supersedes the T3.d.a record's "Every globular tube
  counts whole" and its finding on the globular tubes' 0.23–7.0%. Only unwired output moved:
  `discrete_share.golden`, re-blessed by name at 23 on x86_64 (main's ruling: nothing stores or
  reads φ until T3.d.b, with P10.T10.b's registration at 27 or later), and `Debris`' dwarf scale.
  - _The globular term._ `ShareFigures::of` multiplies it by a new `share::GLOBULAR_TUBES_INSIDE`,
    0.55: the mass-weighted share of the globular tubes' radial periods inside 65,000 ly, living
    and orphan together as `Debris` draws them, a parameter of the generator version. Over the
    first 64 sweep galaxies it is 0.41–0.67 a galaxy (median 0.551, mean 0.549), and
    `the_globular_tubes_inside_share_is_the_sweep_s_mean` (slow, 535 s in the slow-test profile,
    pod) holds the mean to the constant within 0.03. The share cannot draw the tubes (their orbits
    need the kinematic tables), so a measured constant stands for "the catalogue's rates". It
    moves with the orphans' orbits and the potential.
  - _The ceiling's guard._ `ShareFigures::applied` is `physical.min(drawn.max(globular))`: the
    globular tubes are never scaled, since P15.T11.b fits their 1.5 per globular, so the ceiling
    yields to them. `dwarf_scale` keeps its form; it is already 0 there. Design note 1 says so,
    and gives the range over drawn galaxies as "about 0.1–4%", from the summary's 0.13–3.86%,
    where the research had "about 0.1–3.5%" (review fix).
  - _Where the guard acts._ None of the 2,000 summary galaxies' globular tubes reach the drawn
    share now (14 before). Over the first 40,000 sweep seeds, by their parameters alone, 9 do at
    0.55 (6 at 0.5), the widest by 1.33 times (sweep galaxy 7,805). The slow test
    `debris_carries_the_scaled_dwarfs_and_drops_what_the_scale_empties` took its emptied galaxy
    from the first 2,000, of which none qualifies now; it takes the first with dwarf debris of
    sweep galaxies 7,805, 39,409 and 38,661 (`GLOBULAR_TUBES_PASS`) and checks that the share
    applied there is the globular tubes', above the drawn share.
  - _The summary (v23, the same 2,000 seeds)._ Applied share 0.14–14.7%, median 1.23%, mean
    2.11% (from 0.26–14.7%, 1.80%, 2.58%): 812 under 1%, 592 at 1–2%, 403 at 2–5%, 144 at 5–10%
    and 49 at 10–15%, so 1,404 fall under the brainstorm's 2% floor (from 1,107). The drawn share
    binds in 215 (from 251); 10 have no dwarf debris inside 65,000 ly. The globular tubes inside
    65,000 ly are 0.13–3.86% of the halo, median 0.55%, mean 0.65%; 645 galaxies lie outside
    0.3–1% (from 1,024). The Milky Way fixture: globular tubes 0.46% (from 0.84%), applied 2.79%
    (from 3.17%), drawn 8.5%.
  - _The tests, restated._ "Never above the drawn share" is now "never above the greater of the
    drawn share and the globular tubes' share", in the fast test (renamed
    `the_share_is_capped_by_the_drawn_or_globular_share_and_the_scale_meets_it`) and the summary.
    "Globular tubes alone are 0.3–1% of the halo" is now held at the fixture (a new assertion),
    and over drawn galaxies `GLOBULAR_SHARE_PIN` is 0.1–4.5% (from 0.2–8%) and the summary's
    median 0.4–0.7% (from 0.8–1.2%). A new fast test,
    `the_ceiling_yields_to_globular_tubes_that_pass_it_alone`, checks the guard on made-up figures
    and on the three `GLOBULAR_TUBES_PASS` galaxies, on in-plane tables (review fix). T3.d.a's
    text says so.
  - _For P10.T12 (ii)._ Each sweep galaxy's globular tube mass inside 65,000 ly, as drawn, is
    0.71–1.33 of the share's globular term (mean 1.00 over the 64): the per-galaxy band is about
    ±30%, a little wider than the research's ±25%. The mean over 64 galaxies is the term's to 1%.
  - _Findings (science check, 2026-10-29), none a blocker._ (1) The orphans' radial law is the
    globulars' `r^−3.5`, which the Harris 2010 catalogue measures over 3–40 kpc and which steepens
    beyond (plan 09's records); taken out to 10⁶ ly it puts 0.18 of its mass beyond 40 kpc, where
    the catalogue's 105 clusters of [Fe/H] < −1 have 7 (0.07, Poisson P(≤ 7 | 19.3) ≈ 10⁻³), and
    0.30 beyond 65,000 ly against the living system's 0.18 (`POOR_PLACED_SHARE`). Destroyed
    clusters should sit at least as far in as survivors, so `GLOBULAR_TUBES_INSIDE` may be low:
    with the catalogue's 0.18 beyond the cut, or the law cut at 40 kpc, the share of the law
    outside corotation that lies inside 65,000 ly would be 0.67–0.72 against 0.50, and the
    constant about 0.65–0.7 (a scaling estimate, not a measurement); the fixture's globular
    tubes would stay inside 0.3–1%. Recorded in `orphans.rs`; the fix (plan 09's normalisation
    for the orphans, or a law steepened beyond 40 kpc) is a ruling for main, and re-measures the
    constant (lean, 2026-10-29: leave it until P10.T4 or T5 next touch the orphans, at 23 while
    unwired). (2) The 0.3–1% is of the fixture's halo budget, about 5.2 × 10⁸ M☉; against the
    Milky Way's measured stellar halo, 1.4 ± 0.4 × 10⁹ M☉ (Deason, Belokurov and Sanders 2019,
    MNRAS 490, 3426), the fixture's 2.4 × 10⁶ M☉ of globular tubes inside 65,000 ly would be about
    0.2%. The known thin streams inside 20 kpc hold about 6.8 × 10⁵ M☉ (Bonaca and Price-Whelan
    2024's table, as the science check summed it), so the model has about 3.5 times the known
    mass, plausible for an incomplete census.
- **Research-p10's rulings for P10.T12 (lane `p10-fixes`, 2026-10-29).** Research
  `research-p10`'s items 1 and 6 (change 5), as main ruled them (2026-10-29). Docs only. It
  supersedes the T3.d.a record's "P10.T12 counts members inside the cube" ("The dwarf term").
  - _P10.T12 counts as the share does (item 1)._ A tube counts by its members inside 65,000 ly and
    a core by every member, as `discrete_share` counts a dwarf's tube by its period share inside
    65,000 ly and its core whole (T3.d.a's record). T12 first counted "tubes and cores inside the
    cube", whose corners reach 113,500 ly, while the share is reckoned inside the 65,000 ly sphere,
    the halo's smooth components' cut and so the region φ takes from: a cube count would compare
    different regions. A core is one bound object, which exists only where its centre lies in the
    root cube at the epoch (Design note 11), with its members within its extent (at most 4,524 ly in
    the research's probe of 40 cores over the fixture and 12 sweep galaxies); cutting it at a sphere
    would split it. In that probe 4 cores lie across or beyond 65,000 ly, the largest 9.3 × 10⁶ M☉
    centred at 69,780 ly in sweep galaxy 0, 1.7% of that halo.
  - _The globular part is held in the mean (item 6, change 5)._ φ takes the globular tubes in
    expectation ("φ is the expected share … computed once per galaxy from the catalogue's rates and
    never from individual clusters", the brainstorm), so a galaxy's drawn tubes scatter about the
    term, 0.71–1.33 of it inside 65,000 ly over the first 64 sweep galaxies. A 5% test per galaxy
    would fail on that noise alone, so T12 (ii) holds the mean over 64 galaxies to 5% and each
    galaxy to the sweep's band; the dwarfs, whose cold masses φ reads one by one, stay per galaxy.
    T12's "Budgets" says so.
- **Finding for P10.T7.b, from research-p10's item 3 (lane `p10-fixes`, 2026-10-29).** The
  research put it in T7.a's record and the cap as T7.b's lean; it is filed here for T7.b. In the
  research's probe, before these fixes, 11 of the 40 cores have an extent (T7.a's Jacobi radius)
  inside their own Plummer half-mass radius, 1.305 a, and 6 inside a: cores at 10,000–35,000 ly from the centre, such as one at 10,159 ly with r_J =
  365 ly against a = 449 ly. Such a core is disrupting, and tapering a Plummer sphere at r_J < a
  packs its systems into a nearly uniform ball. The research's lean (low confidence), for T7.b:
  cap the scale at a ≤ r_J ÷ 2 for such cores, a guard and not the physics, since tidal stripping
  shrinks a core's radius only by a factor of about 2 even after 99% of its stars are lost
  (Peñarrubia, Navarro and McConnachie 2008, ApJ 673, 226; abstract only). Not a blocker; T7.b
  measures it again on these fixes' scaled cores.
- **P10.T6.b as built (lane `p10-t6b`, 2026-10-30, at `GENERATOR_VERSION` 23).** No output
  moved: nothing reads a tube before P10.T8. One tag, `stream.tube.gaps` (`Galaxy`), keyed by
  `ObjectKey::galaxy_item` of the stream number; `rng/tags.golden` is extended only.
  - _Items._ `TubeTable::from_tracers(seed, mean_system_mass, spec, tracers)` (`Seed`,
    `SolarMasses`); `build` passes `galaxy.seed()` and `galaxy.mean_system_mass(Population::Halo)`.
    `TubeTable::along_cells` and `TubeTable::noise`; in `tube`, the constants
    `CROSS_SECTION_MEMBERS`, `MIN_ALONG_CELLS`, `MAX_ALONG_CELLS`, `CELLS_PER_GAP_NODE` and
    `GAP_NOISE_AMPLITUDE`. The table stores the seed, the cell count and the noise's mean (16
    bytes more for T6.c's `byte_size`), no per-cell values and not the mean system mass.
  - _For T8.b._ Read `along_cells()`, never recompute it. Cell `i` holds the along coordinates
    from `i ÷ cells` to `(i + 1) ÷ cells`. Before band shares its mean count is
    `Λ_total ÷ (m̄ × cells) × noise(i)`, with `m̄` the mean system mass `from_tracers` was given
    (the halo's, in `build`). `noise` reopens the stream and reads two node normals at each call.
  - _For T6.c._ The tube golden must pin `along_cells()` and some cells' `noise(cell)` bits, not
    only the stored fields: no golden pins the noise yet, so `wasm32-wasip1` does not compare it.
  - _Leans (2026-10-30)._ "The power of two nearest" is nearest in the logarithm, a geometric
    middle rounding up; the floor of 16 cells; the nodes run from the trailing tip's ψ to the
    leading tip's, `cells ÷ 8` intervals, so Design note 8's "eight cells" is eight mean cells in
    ψ, and where the tracers lie sparse a node spacing covers fewer cells (about 2.5 over
    `gd1_like`'s outer 10 % of mass); the amplitude, σ = 0.3 of the logarithm.
  - _Tests._ Fast, `tube::tests`: `the_gap_noise_keeps_the_expected_total`,
    `the_gap_noise_is_a_pure_function_of_the_seed_and_the_stream`,
    `the_along_cells_follow_the_rule_and_its_clamp`,
    `a_mean_system_mass_of_zero_is_a_broken_invariant`. Slow, by name, in
    `tests/global_list_tube.rs`: `gd1_like_s_gap_noise_peaks_at_a_few_thousand_light_years`, the
    peak read at the scale `1 ÷ k` (Bovy, Erkal and Sanders 2017's axis) in `k P(k)` (this task's
    choice, so that a peak exists), and
    `build_cuts_the_tube_with_the_galaxy_s_seed_and_its_halo_s_mean_mass`.
- **P10.T6.c as built (lane `p10-t6c`, 2026-10-30, at `GENERATOR_VERSION` 23).** No output
  moved: nothing reads a tube before P10.T8. New unwired golden `galaxy/global_list/tube.golden`
  (a lib unit test, so `wasm32-wasip1` compares it), blessed by name.
  - _Items._ In `tube`: `TubeCell` (`along`, `across`, `across_widths`, `From<StreamMemberId>`),
    `CUT_WIDTHS` (4) and `ACROSS_CELLS` (64); `TubeTable::byte_size` and
    `TubeTable::cells_touching(&GalacticPosition, LightYears)`, a centre and a radius as plan 09's
    `NestedGrid::cells_touching` takes, so that T9 asks once for the census's unpadded radius and
    once for the hits' padded one. `TubeKnot` keeps the tangent and the normal only; `frame()`
    recomputes the binormal to the same bits and is no longer `const`.
  - _No `KnotHash` (main's fallback, ruling of 2026-10-30)._ Each arm stores its segments'
    reaches, and `cells_touching` scans the 510 padded segment boxes. The per-tube hash, built in a
    scratch probe and not committed, was no faster, larger (5.6–64 kB a tube against 4 kB), and
    over the lean's 2,000 entries for `pal5_like` and `sagittarius_like` and for 152 of a drawn
    galaxy's 306 tubes (module documentation, "Finding the segments"). The brainstorm's "small
    spatial hash of knots" is now this scan; T9's "the knot hash" is `cells_touching`'s.
  - _For T8.b and T9._ `cells_touching` is conservative: it yields every cell holding a point
    `locate` puts within the radius, and may yield cells holding none, so T8.b must place members
    with `locate`. Once T9's census sums the yielded cells' means, the set is output, and with it
    the test's constants (`MARGIN_LY`, `EDGE`, `PIECE_CHORD`, `MAX_PIECES`). Across cell `j` spans
    `−4 + j ÷ 8` to `−4 + (j + 1) ÷ 8` widths (`across_widths`).
  - _For T10.b._ `build` sprays serially: 2.12 s for `pal5_like`, or 0.138 s with its tracers on a
    16-thread pool through `from_tracers` (pod, 16 vCPU). A table holds 90,256 bytes, not 80 kB,
    so T10.b's cache budget is about an eighth more a tube. Ruling 146.3's `τ_p ÷ 32` is not
    applied (lean, 2026-10-30): the pool meets 0.2 s.
  - _Tests._ Fast, `tube::tests`: the golden and order independence
    (`milky_way::the_test_streams_tubes_are_pinned_and_order_independent`), the scan, the reach,
    `cells_touching` against points drawn in spheres, the cells, `byte_size` and the frame's bits.
    Slow, by name, in `tests/global_list_tube.rs` (`testing`, `--ignored`):
    `cells_touching_holds_every_point_of_the_test_tubes_in_a_sphere`. Bench:
    `benches/global_list.rs` (`testing`).
- **P10.T7.b as built (lane `p10-t6c`, 2026-10-30, at `GENERATOR_VERSION` 24).** No served
  output moved: no source places a core's members yet (T9, T10.b). New unwired golden
  `galaxy/global_list/dwarf_core_members.golden`; tags `dwarf.core.age` and `dwarf.core.abundance`
  (`System`) appended, so `rng/tags.golden` is extended only.
  - _Items._ `global_list::members::GlobalMemberRecord` (record, class, composition, velocity,
    `attempt`, `stars`); on `DwarfCoreModel`: `member_id`, `expected_candidates`,
    `candidate_count`, `candidate`, `members_in_cell`, `feature_level_list`, `member_speed_bound`,
    `peak_candidates`, `check_index_headroom`; in `dwarf_core`: `resolve_dwarf_core_member`,
    `ExceedDwarfCoreIndexError`, `quenching_time`, `QUENCHING_TIME_YR`.
    `SystemOrigin::GlobalListMember { attempt }`, with an arm in every match on the origin;
    `TraceMotionError::GlobalListMember` and `Drift::of_global_member`.
  - _`resolve_with`_ resolves a core's member by drawing the galaxy's `Debris` (about 0.2 s in
    release) until P10.T4's list holds the cores' specifications, and refuses a core that fails
    `check_index_headroom`; `resolve` still refuses it.
  - _Research-p10's notes, as ruled (2026-10-30)._ `dispersion_at` is flat; the tail ends
    `quenching_time` of the infall mass after the accretion (1 Gyr to 10⁷ M☉, 2 at 10⁸, 9.5 from
    10⁹, linear in log M★), so the class tables move (T7.a's tests restated).
  - _Members._ Plan 09's placement on `member.cell`, `member.position` and `member.accept`; the star
    by attempts as the centre's (age per attempt on `dwarf.core.age`; a living star kept below its
    age's turn-off at its own [Fe/H]; a remnant kept if of its class's kind, with no kick cut, as
    T7.a keeps every remnant); [Fe/H] the mean plus the spread times one normal; velocity the bulk
    plus N(0, σ) on each axis with no escape cut (lean, 2026-10-30: a cut at the truncated
    isothermal halo's escape speed would take a quarter of the members at the extent, against the
    flat profile); multiplicity `Free`. The feature-level list is empty until P09.T33, T34 and T36.
  - _Capacity._ The in-cell index widened to 24 bits (research-leans-3; "P10.T7.b's core index
    widened, as built"), in place of the lean of 2026-10-30 that refused cores past 13 bits. A
    core's fullest cell holds about 10⁻³ of its systems whatever the grid's width;
    `check_index_headroom` still guards the index as a typed error, and whoever plays a core calls
    it first.
  - _For T8.a._ A dwarf's tube takes the core's history only to the tube's own stripping, not the
    stars the core forms after it (research-p10 §4).
  - _Research-p10's item 3, measured again._ Of the 252 drawn cores, 59 have their extent inside
    their half-mass radius and 35 inside their Plummer scale; the research's cap, `a ≤ r_J ÷ 2`,
    is not applied (low confidence, not ruled).
  - _Tests._ Fast, `dwarf_core::members::tests`: the laws, the profiles by KS, every ID's round
    trip and resolving, stars of their class, the speed bound, the headroom, order independence,
    the golden; `dwarf_core::tests::the_tail_ends_a_mass_dependent_quenching_time_after_the_accretion`;
    `placement::resolve::tests::stream_and_dwarf_core_members_are_not_generated_by_resolve`. Slow,
    by name: `resolve_with_reaches_a_drawn_core_s_members`, and plan 12's
    `source_horizon_sweep_every_source_answers_at_the_horizons_start` gains the cores' row.
- **P10.T8.a as built (lane `p10-t6c`, 2026-10-30, at `GENERATOR_VERSION` 24).** No served
  output moved. Tags `stream.member.age` and `stream.member.abundance` (`System`) appended.
  - _Items._ `global_list::members::StreamBandShares`: `of(&Galaxy, &StreamSpec)`, `share`,
    `living`, `white_dwarfs`, `mean_member_mass`, `composition(&Galaxy, SystemId)`, `draw_star`;
    `ClusterStars::{kept, depleted_slope}`, set from plan 09's `class_counts` for a living
    globular (`Debris::generate` now builds each tubed globular's `ClusterModel`), 0 and −1.5 for
    an orphan; `StarFormationHistory::no_younger_than` (crate-visible); `dwarf_core::classes`
    crate-visible.
  - _What a globular lost._ Per system formed: `φ(m) (1 − K d(m))` for a living star below the
    turn-off and `φ(m) (1 − K)` for a white dwarf's progenitor to 8 M☉, NS and BH zero. An orphan
    keeps nothing (lean, 2026-10-30: `K = 0`), so its stream is the evolved canonical population
    and "the slope of a cluster at the end of its life" changes nothing. Every member is first
    population (lean, 2026-10-30, as plan 09's tails).
  - _A dwarf's tube._ The core's classes over `StarFormationHistory::of(infall, accreted)` cut at
    the tube's last pericentre, at the core's mean [Fe/H] and spread; the infall mass and time are
    the galaxy's recent progenitor `j`'s.
  - _For T8.b._ `draw_star` (a `MemberStar`) keeps a mass by its weight's odds over the band's
    greatest, a living star below its age's turn-off and a dead one only if its remnant is a white
    dwarf; as in plan 09's clusters, a star past the turn-off but not yet a remnant is of the
    white-dwarf class.
  - _Test restated._ An orphan's band A share equals the canonical evolved share (`K = 0`); only a
    living globular's lies above it, so most streams, orphans, are not bottom-heavy (a question for
    main).
  - _Tests._ Fast, `members::tests`: the shares, no NS or BH, a dwarf's tube's ages and [Fe/H];
    `dwarf_core::tests::a_history_cut_at_an_age_keeps_the_older_stars_only`. Slow, by name:
    `a_living_globular_s_stream_carries_what_its_cluster_keeps` (`tests/global_list_debris.rs`).
- **P10.T8.b as built (lane `p10-t6c`, 2026-10-30, at `GENERATOR_VERSION` 24).** No served
  output moved. New unwired golden `galaxy/global_list/stream_members.golden`; tags `stream.cell`
  (`Cell`) and `stream.member.position` (`System`) appended.
  - _Items._ In `global_list::members`: `expected_members`, `stream_member_id`,
    `stream_member_count`, `stream_member`, `stream_cell_members` and `resolve_stream_member`,
    each taking the `TubeTable` and the stream's `StreamBandShares`
    (`StreamBandShares::mean_system_mass`). `resolve_with` resolves a stream's member with the
    lookup's tube and the spec drawn from the galaxy's `Debris` (as for cores, until P10.T4's list).
  - _The mean._ The tube's mass over the halo's mean system mass, companions included (Design note
    7, as the cores and the share count), and its along cells, times the noise, the band's share
    and the two normal masses over the cut's. The means sum to the tube's expected members to 10⁻⁹.
  - _The fullest cell._ `sagittarius_like`'s axis cell at its greatest noise expects 42.5 M dwarfs
    against the plan's "about 180" (along cells held at 16,384; the cell about 0.0025 of a
    cross-section); pinned as found at 30–90.
  - _A member._ `u` uniform in its along cell, `a` and `b` by the truncated normal's inverse
    transform, its position `locate`'s, so `cells_touching` finds it; its velocity, for T8.c, the
    tube's mean velocity.
  - _For T9._ A cell's members are `stream_cell_members` over `cells_touching`'s cells, each
    tested at the query's time; the census sums `expected_members` over them.
  - _Tests._ Fast, `members::tests`: the counts along by chi-square, the across coordinates by KS,
    IDs and resolving, order independence, members outside the root cube dropped, the golden.
    Slow, by name: `the_fullest_cell_of_sagittarius_like_s_tube_stays_far_under_the_index` and
    `resolve_with_reaches_a_drawn_stream_s_members` (`tests/global_list_tube.rs`); plan 12's sweep
    gains the streams' row.
- **P10.T8.c as built (lane `p10-t6c`, 2026-10-30, at `GENERATOR_VERSION` 24).** No served
  output moved; T8.b's unwired `stream_members.golden` re-blessed by name, its velocities alone
  moving. Tag `stream.member.velocity` (`System`) appended.
  - _Items._ `TubeTable::frame_at(u)` and `TubeTable::dispersions_at(u)`, read as `locate` reads
    the knots.
  - _The draw._ The tube's mean velocity at `u` plus three normals, a Box–Muller pair and one
    more, times the dispersions there along the local tangent, normal and binormal. Every band
    takes the tracers' dispersions (lean, 2026-10-30: the spray's tracers carry no mass, so no
    equipartition is drawn). The member drifts on its own straight line
    (`Drift::of_global_member`).
  - _For T9._ No speed bound is given yet for the query's pad: a member's internal speed is at most
    8.58 √2 times its tube's largest dispersion, plan 09's `INTERNAL_SPEED_SIGMAS`.
  - _Tests._ Fast: `members::tests::stream_members_move_with_their_tube_s_dispersions_in_every_band`
    and `tube::tests::the_frame_and_the_dispersions_at_an_along_coordinate_are_the_knots`.
- **P10.T4 as built (lane `p10-t4`, 2026-10-30, at `GENERATOR_VERSION` 23).** No output moved:
  nothing generated reads the list. New unwired golden `galaxy/global_list/list.golden` (a lib
  test, so `wasm32-wasip1` runs it), re-blessed by name at the version in force while unwired.
  - _Items._ `global_list::{GlobalList, GlobalEntry, BoundingShell, CUT_WIDTHS, SHELL_WIDENING}`;
    `GlobalList::{build, entries, kinds, debris, stream(StreamNumber), core(DwarfCoreNumber),
entries_touching(&QuerySphere), entries_with_class_members}`; `GlobalEntry::{kind, mass, shell,
has_class_members}`; `BoundingShell::{new, inner, outer, is_empty, contains, touches}`;
    `CentreModel::as_global_entry()` (mass inside the 128 ly reach, shell 0–128 ly; the name is
    plan 09's Provides', though it builds an owned value). `GlobalList` is `PartialEq`, not `Eq`;
    `Debris` gains `Default`. Crate-visible: `spray::radii_over_ly`, `dwarf_core::extent_of`.
  - _Contracts._ `build` reads `Galaxy::centre()` and `expect`s it (no galaxy the builder allows
    fails it) and panics without the kinematic tables, as `Debris::generate` does.
    `entries_touching` tests the sphere's padded radius, so the caller pads for its members'
    motion (T9: the centre's members need plan 09's own speed bound). `entries_with_class_members`
    is the centre and every core.
  - _Shells (deviation)._ Orbits step in the spray's `SprayForce`, not `Leapfrog`, so a stream's
    backward leg over `T_s` is its spray progenitor's; forwards over `T_s` too when `T_s` is under
    a radial period. A core goes one radial period each way and is widened by its extent, not by
    widths. A stream's radii widen by `(CUT_WIDTHS + 2 SHELL_WIDENING) r_t ÷ r_p` of themselves,
    `r_t` at the epoch mass plus the tube's or the spec's, the larger; `SHELL_WIDENING` = 4.
  - _For T6 and T9._ The tubes' own four-width cut is not inside the shells: end knots' widths run
    to about ten times the median, so a member cut there could lie far beyond every tracer (the
    lane's report has the figures). `stream_shell` is private to `global_list`; its child modules can
    call it.
  - _Counts (lean, 2026-10-30)._ "About 240" is held as the mean over eight Milky Way seeds within
    200–300; "150–1,500" is held on 24 galaxies, though some drawn galaxies expect fewer than 150
    (a finding).
- **P10.T4's list build, faster (a performance lane, 2026-10-30).** No output moved.
  `spray::radii_over_ly(force, &[Leg]) -> Vec<[f64; 2]>` steps every shell orbit of a list
  together, in lanes, so `GlobalList::build` gathers its shells' legs first; `stream_shell` is
  test-only (T9's tests still call it). `SprayForce`'s lookup is split into `locate`,
  `Located::logs` and `acceleration_at` (`acceleration_ly` is gone), and `globular::RadialMotion`
  gives an orbit's turning points one at a time, which the orphans' draws use. If the spray's
  steps change, `radii_over_ly_alone` (test-only) holds the lanes to them bit for bit.
- **P10.T5.b's spray, faster (a performance lane, 2026-10-30).** No output moved.
  `Spray::tracers` steps its tracers side by side in lanes, earliest releases first, and gives
  them in index order through `spray::step_in_lanes`, which the shell orbits share (a `Lane` trait
  over a `Walker`). `Spray::tracer(i)` stays the scalar path a pool calls; the spray's lib test
  holds the two bit for bit.
- **P10.T5.b on the server's pool (lane `perf-list-pool`, 2026-10-31; no output moves).**
  - Sim: `Debris::plan` gives a `DebrisPlan` (`orphan_count`, `orphan_room`, `orphan(galaxy, k)`
    pure in `k`, `finish`). `GlobalList::plan` gives a `ListPlan` (`of_debris`, `legs`, `finish`).
    `spray::Leg` (with `steps`) and `spray::radii_over_ly` are public; `Spray::tracers_in(range)`.
  - Server: `compute::{global_list_on, tube_table_on}` with `SharedSprayForces` (a
    galaxy's spray table) and `Shares`. They are not yet called from a request; P10.T10.b keeps the
    list per universe and caches the tables, and calls them.
- **P10.T6.a's arms by energy, as built (lane `p10-spray-arms`, 2026-10-30, at
  `GENERATOR_VERSION` 24).** Main's ruling of 2026-10-30, after the research lane
  `research-spray-arms`: a tube's arms split by the sign of each tracer's energy offset at
  release, ordered by ψ within each arm, with no hold, since nothing served reads a tube before
  T10.b (27 or later). No served output moved. `tube.golden`, `stream_members.golden` and
  `dwarf_core_members.golden` are unchanged; `spray.golden` gained one `energy_offset` label a
  tracer ("extended only"), and `tube_arms.golden` is new, both blessed by name.
  - _Items._ `Tracer::energy_offset_j_kg()`: the tracer's `½|v|² + Φ` less the progenitor's, both
    at the release boundary, Φ `SprayForce::potential_j_kg`, J/kg. `Tracer::energy_arm()`:
    `Leading` below zero, `Trailing` otherwise, a zero of either sign included. `arm()` stays the
    Lagrange point, a diagnostic. `Tracer::from_parts` (test-only) takes the offset last. No draw,
    tag or word added.
  - _The split._ `from_tracers` partitions by `energy_arm()`, trailing first, then sorts each arm
    by ψ, stably. The gap noise's span (a private `psi_span`) runs from the lesser of the arms'
    first knots' ψ to the greater of their last knots', chosen with `<`. `cells_touching` is
    unchanged: its bound is per segment.
  - _What it replaces._ In a cloud an arm's ψ may cross zero, so Design note 7's `u = Λ(ψ) ÷
Λ_total` holds within each arm and ψ may step back between the roots; T6.a's knots keep their
    arm's order, not its sign; T6.b's noise nodes span the knots' least ψ to their greatest, not
    tip to tip.
  - _Unserved bits that moved._ Every tube in which a tracer's two signs disagree: the clouds, and
    a living globular's whose last release rounds to the epoch's boundary, whose trailing tracer
    at ψ +0.0 was the leading arm's (a chance of N ÷ 2 steps, 2.5 % at 20,000 steps).
  - _Counts (x86-64, pod, 16 vCPU)._ Three drawn galaxies' 306, 258 and 275 streams at 2,000
    tracers (seeds `0x0a10_3000_0000_000{0,1,2}`, a scratch probe, not committed): 1, 3 and 1
    tracers with the other arm's energy, at most 1 a stream, where ψ's sign put 1,960, 2,733 and
    971 on the other side, 7, 11 and 7 streams over 0.5 %. Streams 153, 183 and 244 of the first
    go from 1,244, 424 and 161 to 0. `sagittarius_like`'s stray under the tests' seed and seed 7
    is the same tracer by both rules; seeds 5 and 6 have one each by the energy alone, at the
    root, inside the cut.
  - _Limit, and for T10.a and T11._ "ψ's sign fails for eccentric orphans near apocentre" is
    resolved for the split, but such a tube is still a blob, each arm one radial band, outside the
    root cube. Its track and `length()`, which cross the roots, are no stream's for T10.a's
    `StreamTrack` or T11's `StreamReadout`.
  - _Cost._ Two potential evaluations a tracer at release. `TubeTable::build(pal5_like)` 2.123 s
    alone and 138.2 ms on 16 threads, `from_tracers` 1.300 ms, against T6.c's bench before the
    change: no change past 0.12 %.
  - _Tests._ Fast: `spray::tests`' `the_energy_at_release_names_each_tracer_s_arm`, in the pinned
    group (the test streams' 6 tracers an arm keep their Lagrange point's energy; a release on the
    epoch's boundary leaves both its tracers at ψ +0.0, in their own arms);
    `tube::tests::the_arms_are_split_by_the_energy_s_sign`,
    `the_gap_noise_s_span_covers_both_arms` and `tubes_whose_arms_cross_zero_are_pinned` (the
    synthetic cloud and `pal5_like` stripped over 20 Myr). Slow, by name, restated:
    `a_drawn_galaxy_s_tracers_keep_their_lagrange_point_s_energy`, renamed from
    `a_drawn_galaxy_s_streams_keep_nearly_every_tracer_on_its_arm_s_side`, which failed at the
    base on stream 153: at most 2 of 2,000 a stream, on the step quantiles, the dwarfs and up to
    three clouds chosen by rule (orphans, `e ≥ 0.85`, at 0.85 of apocentre or more, `T_s` of four
    radial periods or fewer; the fewest periods first, then by number; at least one asserted);
    `the_test_streams_arms_lead_and_trail_and_pal5_s_track_leaves_its_orbit` (energy strays at
    most 2, `sagittarius_like`'s 1); and in `tests/global_list_tube.rs`
    `the_strays_lie_inside_the_other_arm_s_tube`, `the_test_streams_tubes_follow_their_tracers`
    and `an_orphan_s_tube_holds_its_cluster_s_gap`, by `energy_arm()`, each stray's ψ found in its
    energy arm. Commands, `<file>` each of `global_list_spray` and `global_list_tube`:
    `cargo test -p hyperion-sim --features testing --profile slow-test --test <file> -- --ignored`.
- **P10.T7.b's core index widened, as built (lane `p10-spray-arms`, 2026-10-31, at
  `GENERATOR_VERSION` 24).** Research `research-leans-3` overturned T7.b's capacity lean (main,
  2026-10-30): a refused core showed no members, and the refused cores are the most luminous
  satellites. No version: nothing places core members yet (T9, T10.b), and no ID or count of a core
  that passed moves. `dwarf_core_members.golden` is unchanged; `layouts.golden` gained two wide IDs
  ("extended only") and `dwarf_core_index.golden` is new, both blessed by name.
  - _Layout._ The dwarf-core sub-kind's `[43:33]` hold the in-cell index's high bits, above the
    slot's `[12:0]`, for an index below `DwarfCoreMemberId::INDEX_LIMIT`, 2²⁴; `DWARF_PADDING` is
    `[56:44]`. Band 7 keeps 13 bits, a high bit under it `PaddingBitsSet`. An extension, as
    decision-p11-centre-index was: the wire and `PROTOCOL_VERSION` are unchanged.
  - _Types._ `MemberSlot::InCell`'s `index` is `u32`; every other layout still refuses 8,192 or
    more, and features and the centre keep `u16` counts. `DwarfCoreModel`'s `member_id`,
    `candidate` and `candidate_count` take and give `u32`; `count_from_mean` clamps at 2²⁴ on the
    same Poisson word. `SystemId::cell_word` zeroes a core's whole index. `parse_slot` takes named
    limits from its caller (the `DWARF` arm 2²⁴); the formatter is unchanged.
  - _The guard._ `check_index_headroom` stays, at 8σ against 2²⁴, and `resolve_with` still refuses
    a core that fails it. Twice the fullest cell of plan 02's largest progenitor after the least
    stripping (`RECENT_MASS`, `STRIPPING_FRACTION`), 1.14 × 10⁷, fits; the index overflows near
    1.1 × 10¹⁰ M☉. Census and cost: the outbox.
  - _Tests._ Fast: `id::global::tests::a_dwarf_core_s_index_in_a_cell_is_24_bits`,
    `every_padding_bit_is_rejected`, `id::system::tests::cell_word_zeroes_the_index_and_equals_candidate_zero`,
    `id::designation::tests::a_dwarf_core_s_cells_number_past_8191`, the `DwarfCoreMemberId`
    doctest, and `dwarf_core::members::tests`'
    `no_cell_of_a_core_under_the_headroom_expects_the_index` (restated, with the tripwire),
    `the_fullest_cell_of_a_massive_core_resolves_past_8191` (every 97th member) and
    `the_massive_core_s_members_past_8191_are_pinned`.
- **P10.T9 as built (lane `p10-t9`, 2026-10-31, at `GENERATOR_VERSION` 24).** No output or
  golden moved: only tests call the source. The tube cut's cap was tried and dropped: it put no
  member back in its shell.
  - _Items._ `global_list::GlobalListSource::new(galaxy, lookup, orbits)`, a `SystemSource`;
    `GlobalList::stream_entry`, `GlobalList::of_parts` (testing); `TubeTable::member_speed_bound`.
    Crate-visible: `features::members::source::{band_reach, hit_at}`.
  - _Deviation._ `tube::MEMBER_SPEED_SIGMAS` is 8.58 (1 + √2), not T8.c's 8.58 √2: the frame's
    vectors are interpolated and normalised apart, so they need not be orthogonal.
  - _The shell hold (main's ruling, 2026-10-31, option A, reversible)._ The tips' members lie
    outside their shells. From `STREAM_SHELL_VERSION` = 26 (`GalaxyRules::stream_shell`) such a
    member is none, in the source and in `resolve_with`; below 26 the source visits every stream.
    Tripwire `the_stream_shell_is_held_only_until_the_26_bump`. Options B and C wait on research.
  - _For T10.b._ Register the source in place of `CentreMemberSource`, never beside it: their IDs
    would repeat.
  - _For T12._ From 26 containment holds by construction: test the members the source keeps.
  - _Tests._ Fast: `global_list::source::tests` (seven) and
    `rules::tests::each_hold_turns_on_at_its_version`. Slow: the brute force,
    `a_query_on_a_test_stream_s_axis_equals_every_member_of_its_tube`, and three more, all by
    `cargo test -p hyperion-sim --profile slow-test --lib -- --ignored global_list::source`.
- **P10.T6.a's tips in the orbital plane, as built (lane `p10-t9`, step 2, 2026-10-31, at
  `GENERATOR_VERSION` 24).** Main's brief after research-leans-10: option B, with P10.T9's option
  A kept as its guard and C rejected. No version: nothing served reads a tube before T10.b.
  - _The fit._ `Reduced::new` fits the centre line, its slope and the mean velocity in the
    kernel's orbital plane about the galactic centre (`tube::polar_fit`: R, φ and z, and the
    velocity's radial, azimuthal and vertical parts), with the same ranks, kernel and weights.
    The widths and dispersions are measured as before.
  - _The fallback (lean, 2026-10-31)._ A knot keeps the Cartesian fit where a kernel tracer lies
    more than 3π ÷ 4 from the plane's first axis, or within a tenth of the kernel's mean R of the
    pole's line. The brief's π ÷ 2 refused `pal5_like`'s trailing tips (tracers to 114°) in 3 of
    8 seeds, which kept 6, 54 and 11 members outside the shell; at 3π ÷ 4, 0–2 a seed.
  - _Goldens._ `tube.golden` (now with each test tube's fallback knots), `tube_arms.golden` and
    `stream_members.golden` re-blessed by name at 24 (main's allowance); no other golden moved.
  - _Cost._ `from_tracers` +25% (a finding; the outbox).
  - _Tests._ Fast: `tube::tests`' `on_a_sparse_arc_every_knot_lies_on_the_arc` and two of the
    fallback; the frame test allows the arms' junction cos 0.9985 (measured 0.99865, 3.0°); P10.T9's
    order test centres its spheres on members, as the centre line moved. Slow:
    `source::tests::over_eight_seeds_the_test_tubes_lie_within_their_tracers_and_shells`.
- **P10.T10.a as built (lane `p10-t9`, step 3, 2026-10-31, at `GENERATOR_VERSION` 24).** No
  generated output moved; no `PROTOCOL_VERSION` bump (new kinds and a growing enum's variants).
  - _Wire._ `global_features` answers `GlobalFeatures { universe, entries, progenitors }`:
    `GlobalEntryWire`, tagged `kind` (`galactic_centre`, `stream` with `number`, `progenitor` and
    `track: TrackStatusWire`, `dwarf_core` with `number`), each with `mass_msun` and a `ShellWire`
    (empty, inner past outer, for an entry wholly beyond the cube's corner); and
    `AccretedProgenitorWire`s, the dominant merger then the recent ones, with the `stream` and
    `core` each left. `stream_track` answers `StreamTrackWire { universe, stream, track }`,
    `TrackWire` tagged `status`: `ready` (`trailing` and `leading`, at most 128 `TrackPointWire`s
    each, ends kept) or `pending`. A stream the list lacks, 4,096 or more among them, is
    `unknown_feature` naming `stream`. `SystemOriginWire` gains `stream { stream }` and
    `dwarf_core { core }`. No dispersion on a track point: a later optional field.
  - _Server, taken from T10.b (lean, 2026-10-31)._ The two handlers. `compute::GlobalListService`
    builds each galaxy's list on its first request, not when the universe opens, through P10.T5.b's
    `global_list_on` at bulk priority (`GLOBAL_LIST_CACHE_BYTES`, 4 MiB; the spray tables in a
    `SharedSprayForces` of `SPRAY_FORCE_CACHE_BYTES`, 1 MiB). With no tube cache, every stream's
    `track` is `pending`, and `stream_track` has its tube built for it through `tube_table_on` at
    interactive priority (`GlobalListService::tube`), not kept; T10.b's cache, warm-up and
    open-time build make them `ready`. A cloud's tube (an
    eccentric orphan near apocentre) is sent `ready` like any other. `convert::system_origin` has
    no arm for `GlobalListMember`, whose number is in its ID: T10.b's.
  - _Parameters._ `halo.discrete_share` is sent drawn in the `halo` group; both keys are out of
    `EXCLUDED_PARAMETERS`, and `PARAMETERS_ACCOUNTED_FOR` is 115, since the progenitors now go in
    `global_features`. `galaxy_parameters.golden` re-blessed by name. The applied share's derived
    key waits for T3.d.b.
  - _For T11._ The client reads `stream` and `dwarf_core` origins as `unknown` (`featureWire.ts`).
    `halo.discrete_share` needs its label in `parameterLabels.ts` and its row in
    `galaxyFixtures.ts`'s parameter list.
  - _Tests._ `hyperion-server`'s `tests/global_list.rs`, and slow:
    `convert::global_answers::tests::global_features_lists_every_entry_and_the_accreted_progenitors`
    and `a_stream_s_track_is_its_tube_s_knots_thinned_to_128_an_arm`.
- **P10.T12 as built (lane `p10-t9`, step 4, 2026-10-31, at `GENERATOR_VERSION` 24).** Test-only;
  no output moved. The slow tests are `global_list::statistics`' four, by name, the millennium's on
  three threads and the others on eight (`.config/nextest.toml`'s slow profile). Every figure is
  pinned as measured; the misses are findings (the lane's outbox).
  - _Still to do._ The budget's second half, the sampled halo field plus the discrete members in
    eight volumes, waits for T3.d.b's call, T10.b's registration and P09.T2.c's `field_factor`.
  - _The tripwire (research-leans-10)._ `option_a_drops_a_bounded_share_over_the_sweep` stands in
    for the shells' containment over a sample of seeds; the widening is not raised (option C,
    rejected). Over the first 8 sweep galaxies (a tube's spray costs some 340 CPU-seconds a
    galaxy), it bounds the members outside their shells (`DROPPED_BOUNDS`), of a galaxy's and of
    one tube's members placed. It enumerates only the along cells whose cut can reach past the
    shell, by the triangle inequality about each segment's chord (`dropped_of`).
  - _For the bump._ Nothing reads these tests' figures; a change to the spray, the tubes or the
    share re-pins them by name.
- **P10.T10.b's additions as built (lane `p10-t10b-t11`, 2026-11-01, at `GENERATOR_VERSION` 24).**
  No generated output moved. The registration member (with P09.T37) is left.
  - _Items._ `compute::{TubeTableCache, ListedTubes, Listing, TubeKey}`;
    `GlobalListService::{tube, is_ready, listing, tube_counters}` (`prefetch`, `stop_warming`
    crate-only);
    `TUBE_TABLE_CACHE_BYTES` (64 MiB); `GlobalListWarmUp { Off, AtOpen }` (off in test servers);
    `ServerStats::tube_tables`; sim `placement::resolve_list_member_with`.
  - _Deviation._ `TubeLookup` is implemented by `ListedTubes`, one galaxy's view of the cache, not
    by `TubeTableCache`, since `global_list()` is one galaxy's. `listing` builds a stream member's
    table on the pool before the job, so a table that cannot be built is refused, not a panic; a
    miss inside a job is built serially. The warm-up stops before a table that would overflow the
    budget, so an open again rebuilds none (some 1.8 CPU-s a table at Milky Way values).
  - _For the registration._ Take the lookup from `GlobalListService::listing` before the job (the
    list is async). Below 26 the source reads every table (option A's hold). `convert::placed_by`
    still has no arm for `GlobalListMember`: a row needs the member's velocity and class.
  - _Resolve sites (lean, 2026-11-01)._ `system_summary` and `resolve_system` go through
    `resolve_with` (`convert.rs` has no production `resolve` left). Extinction's line ends and the
    scene's frame stay on `resolve`, since both refuse a list member's motion whichever resolver
    finds it; `system_bodies` and `body_detail` too, so a list member has no planets yet.
- **P10.T11 as built (lane `p10-t10b-t11`, 2026-11-01, at `GENERATOR_VERSION` 24).** No
  generated output moved; no `PROTOCOL_VERSION` bump.
  - _Wire._ Optional fields: a dwarf core's `position` and `extent_ly`, a ready track's
    `length_ly`, and a track point's `dispersion_km_s` (one axis's rms, as T12 measures it).
  - _Client._ `useGlobalList` asks the list again every 10 s while a track is missing and fetches
    the tracks held, four at a time (`StreamTrackFetcher`); `StreamTrackOverlay` draws under the
    feature overlay; `StreamReadout` and `useStreamReadout`; the progenitors on `PARAMETERS`.
  - _Leans (2026-11-01)._ `TRACKS PENDING <n> OF <m>` is a draft guide row for the owner, standing
    under the overlay's toggles (lean, 2026-11-01). `STREAM` and `DWARF CORE` toggle with
    `FEATURES` (lean, 2026-11-01). `StreamReadout` is the selected stream member's rows in the
    system readout, since the overlay takes no pointer (lean, 2026-11-01).
  - _For the registration member._ The readout's rows and `ORIGIN` `STREAM <n>` appear once a range
    row carries the `stream` origin, which needs `convert::placed_by`'s arm.
- **P10.T10.b's warm-up, perf (lane `perf-stream-warmup`, 2026-11-01, at `GENERATOR_VERSION` 24).**
  No output moved: the slow warm-up test pins the 261 Milky Way tables' digest and two range
  answers at their base values.
  - _Items._ Sim `spray::SprayPlan` (`new`, `schedule`) and `Spray::with_plan`; `Spray::new` makes
    its own plan. `tracers_on` makes one plan a table and lends it to every share; `Lists::warm`
    keeps `tables_in_flight(workers)` tables building (2 on 8 workers, 3 on 15), started in number
    order, one alone until a charge is known.
  - _Not met._ The one to two CPU-minutes: the warm-up still costs some 8 CPU-minutes, all of it
    the spray's step, 98.7 % of it the orphans' (a finding). Held at 27 if wanted: ruling 146.3's `τ_p ÷ 32` and
    500 tracers an arm, each about halving it.
- **P10.T10.b's orphan spray, held at 27 (lane `perf-stream-warmup`, 2026-11-01, at
  `GENERATOR_VERSION` 24).** Through 26 nothing moved: the 24 warm-up digest and range answers
  still pass. From 27 an orphan's spray releases 500 tracers an arm at ruling 146.3's `τ_p ÷ 32`.
  - _Items._ `spray::ORPHAN_TRACERS_PER_ARM`, `spray::ORPHAN_STEPS_PER_CROSSING`,
    `ORPHAN_SPRAY_VERSION` (27), `OrphanSprayRules`, `GalaxyRules::orphan_spray`;
    `SprayParams::with_steps_per_crossing`, `steps_per_crossing` and `of_stream`;
    `FixedStep::with_steps_per_crossing`, `SphericalOrbit::fixed_step_at`. The generator's spray is
    `SprayParams::of_stream` (`spray`, `TubeTable::build`, the server's pool); `SprayParams::default`
    stays the exact one, which hyperion-fit's streams fit keeps (lean, 2026-11-01).
  - _For the bump._ No golden file moves (checked with the hold at 24). Re-pin by name the server's
    24 warm-up test and P10.T12's `a_drawn_galaxy_s_tubes_and_contrast_keep_their_measured_figures`
    (the globular tubes' median length falls 6.1 %); the tripwire lists both.
  - _Not met._ About 1–2 CPU-minutes: the warm-up at 27 costs some 200 CPU-s, since 27's fitted k
    gives the fixture 355 orphans against 256 (a finding).
- **P10.T3.d.b as built (lane `p10-list-registration`, 2026-11-01, at `GENERATOR_VERSION` 24,
  behind `LIST_SOURCE_VERSION` = 27; no output moved).**
  - _The hold (lean, 2026-11-01)._ The share is stored through a `GalaxyRules` field, not with
    none as the desktop's point 1 has it, since it lands with a 27 member:
    `GalaxyRules::list_source` (`global_list::source::{LIST_SOURCE_VERSION, ListSourceRules}`), in
    `every_27_hold()`; tripwire `the_list_source_is_held_only_until_the_27_bump`;
    `Galaxy::with_rules` refuses to change it. P10.T10.b's registration reads the same field
    (`GlobalListSource::is_registered_in`), so the halo's field gives up φ in the train that
    serves its members, and P09.T2.c's `field_factor` takes it out at once.
  - _The call._ `Galaxy::build_field` stores `share::discrete_share_under` (new, crate-only) of
    the seed, the parameters, the in-plane tables and `rules.orphan_streams`, after
    `set_class_shares` and before the field share is built; `discrete_share` is it at the
    version's streams per globular.
  - _Test._ Slow, by name: `galaxy::global_list::share::tests::the_discrete_share_census`.
- **P10.T10.b's registration member as built (lane `p10-list-registration`, 2026-11-01, at
  `GENERATOR_VERSION` 24, behind `LIST_SOURCE_VERSION` = 27, with P09.T37.b; no output moved).**
  - _The hold._ `GalaxyRules::list_source`, P10.T3.d.b's ("P10.T3.d.b as built"), read here by
    `GlobalListSource::is_registered_in`.
  - _The sources._ `compute::range::beside_the_grid` takes the features' source and the class
    cells as a pair and the list's reads (`ListReads`): from 27, with the list taken, the sources
    are `[features, GlobalListSource, classes]`, the centre's source inside the list's and never
    beside it; its `f` gets the list's source too.
    `ServedQuery::with_list(ListedTubes)`; `served_range_query`'s answer gets `RowReads` (the
    interiors and the list's source), and `with_row_reads` gives them to the chunked conversion.
  - _Before the job._ `requests::galaxy::list_for` awaits the list and builds, on the pool, the
    tables of `GlobalListSource::streams_read` (new): an answer waits for the list rather than
    leaving it out, so it never depends on what is built (lean, 2026-11-01).
  - _Rows._ `convert::placed_by` resolves a list member through `GlobalListSource::member` (new;
    its velocity and member class), a centre member (P09.T37.b) and a catalogue host
    (`catalogue_system`, its class's velocity at the row's time, no class; `system_origin` gains
    the arm). Centre and list rows carry no brief yet (lean, 2026-11-01). `AppState::tube_tables`
    holds the service's cache.
  - _Left._ `convert::system_motion` and observed mode refuse centre and list members; the sky's
    census takes no list source.
  - _Test._ Slow, by name, on eight workers:
    `requests::galaxy::registration::from_27_a_range_query_serves_the_list_s_members_and_the_centre_s`.
- **P10.T10.b's placement as built (lane `p10-placed-motion-27`, 2026-11-01, at
  `GENERATOR_VERSION` 24, behind the 27 holds, with P09.T37; no output moved, no wire change).**
  The registration member's _Left_, but the sky's census.
  - _Items._ Server `convert::{SystemLine, MotionReads, ListMembers}`; `system_motion` takes a
    `MotionReads` and is `SystemLine::of(..)?.at(galaxy, t)`; `RowReads::motion`;
    `SystemStarsHandle::with_stars_of`; `SharedBriefCache::get_or_build_listed`; `Ship::frame_line`,
    `ShipStandIn::with_frame`; `requests::scene::frame_line`. Sim `features::centre::CentrePath`,
    `GlobalMemberRecord::position_at`, `Trajectory` for `CatalogueMotion`,
    `observe::{observe_hit_on, summary_observed_on}` (`observe_on_line` folded in),
    `BriefModel::of_global_member`, `TraceMotionError::Disrupted`.
  - _Gates._ Centre and list members on `list_source` (`GlobalListSource::is_registered_in`),
    hosts on `class_source`; below 27 the old refusals at every time.
  - _Contract._ The three kinds sit at their rows' positions bit for bit; a centre member's velocity
    is its orbit's at `t`, its row's the epoch's. `scene_ship` places the frame at the pose's time
    and at `ClockWindow::END` (a victim disrupted inside the window is refused); the core reads the
    frame's path from the stand-in, with no cache.
  - _Recent entries (lean, 2026-11-01)._ A centre recent entry drifts straight at its velocity, as
    `hit_at` rounds it (`CentrePath`'s `Straight`); `fix-p09-recent-dead`'s placement must agree.
  - _Briefs._ List rows carry their member record's; centre rows none: the server generates no
    centre member's stars (no multiplicity context is ruled for the centre's cells).
  - _Test._ Slow, by name: `requests::galaxy::placed::from_27_every_system_a_query_returns_is_placed_by_its_id`.
    Rows at the epoch and ±10³ yr; at ±10⁴ yr against the sources, since no query reaches past ±H.
