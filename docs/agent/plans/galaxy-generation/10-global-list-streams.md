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
  `global_list::tube::{TubeTable, TubeKnot, TubeArm, KnotHash}`:
  `TubeTable::build(&Galaxy, &StreamSpec) -> TubeTable`, `TubeTable::byte_size()`,
  `TubeTable::locate(u, a, b) -> GalacticPosition`, `TubeTable::mean_velocity(u)`,
  `TubeTable::cells_touching(sphere) -> impl Iterator<Item = TubeCell>`.
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
   the expected systems in tubes and cores inside the cube ÷ the halo's budget, from 1.5 tubes per
   expected globular at the orphan law's mean mass, plus each recent progenitor's cold mass times
   the share of its orbital period spent inside 65,000 ly (a fixed quadrature of dr ÷ v_r in the
   mid-plane potential, no integration). φ for the halo is the smaller of the two. When the drawn
   share is the smaller, the dwarfs' cold masses are scaled down together to meet it; when the
   physical share is the smaller, the smooth components keep the difference. Both come from the
   galaxy's parameters, its seed (the dwarfs' stripping fractions, as built: Risks, "T3.a–c as
   built") and its in-plane tables, so `Galaxy` holds φ before any list is built. Reason: a halo
   of about 1% of the
   galaxy is a few 10⁸ M☉, while progenitors run to 10⁹·⁵ M☉, so a drawn share and drawn progenitors
   cannot agree unless one yields. The ceiling is the brainstorm's 15%; its floor of 2% is not
   enforced, because globular tubes alone come to under 1% of the halo and raising them to 2% would
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
    wired output. Nothing stores φ yet. Tests: never above the drawn share; globular tubes alone
    are 0.3–1% of the halo; the distribution of the applied share over 2,000 seeds is recorded in
    a golden summary, a new golden under `tests/golden/galaxy/global_list/` written by a slow
    test, since 2,000 seeds' in-plane tables take minutes, as T3.c's sweep does; evaluating it
    costs under a millisecond. It lands now, at the version in force, with no hold and no train.
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
  table's force plus a Plummer sphere for a living progenitor, with its mass following the history.
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

- **P10.T6.a Reduction.** Per arm about 256 knots per design note 6: centre line (as integer
  light-years plus offset), a local frame carried along the track by parallel transport so that it
  never flips, mean velocity, two widths, three velocity dispersions in the frame, and the
  cumulative line density Λ from the tracer weights. Linear interpolation between knots. `locate`,
  `mean_velocity`, the inverse of Λ. Tests: `pal5_like` gives a length near 82,000 ly, widths of
  120–380 ly and dispersions of 0.9–3.5 km/s; `gd1_like` near 159,000 ly, 100–260 ly and 0.7–1.3
  km/s; an orphan shows the central gap; a dwarf shows the pile-up at apocentre; frames orthonormal
  to 10⁻¹².
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

- **P10.T10.a Protocol.** New kinds of plan 04's `RequestBody` and `ResponseBody`, added to
  `REQUEST_KINDS`: `GlobalFeatures { universe }` → `GlobalEntryWire` list (kind, number, mass,
  bounding radii, the living progenitor's feature ID as plan 09's `FeatureIdHex`,
  `TrackStatusWire`); `StreamTrack { universe, stream }` → per arm a polyline of at most 128 points
  with widths and mean speed, or `Pending`. `SystemOriginWire` gains `stream` and `dwarf_core`.
  `just gen-protocol`. Tests: wire forms for each message and variant. _At the base:_
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

Slow tests. **Tubes over time:** for the three test streams, 10⁴ members each at t = ±1,000 yr lie
within 4 widths plus 0.01 ly of the centre line, have moved 0.3–0.8 ly along it, and the error from
neglected curvature is under 10⁻⁴ ly. **Budgets:** expected members of tubes and cores inside the
cube against `discrete_share` × the halo's budget, within 5% (the tolerance of design note 1's
inside-the-cube estimate), and sampled halo field plus discrete members against the halo's budget
within Poisson error in eight volumes. **Contrast:** on its axis at 15–18 kpc a globular stream is
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
and 2, amending `decision-galaxy-25-train` §2.2). Until then no built galaxy stores φ, and
P09.T2.c's bump at 26, which applies `field_factor` to the fields, leaves the halo's discrete part
at zero; the halo's field gives up φ in the train where its members are first served. Nothing of
T3.d lands inside the potential freeze. P10.T3.d.a and the rest of P10.T4–T12 add output on new
tags and kinds and move nothing that existed, so they land at the version in force with their new
goldens at its header (decision-bump-plan-after-22, 2026-10-14, §3, amended to 2026-10-27).
Stream and core members are additions in the sim, and their registration is what moves the
census. Reserved: the `10` sub-kind `11`; the three unused bits of a stream member's ID; stream
numbers above the list's length; `dwarf.*` and `stream.*` domain tags. Parameters that belong to
the generator version and are named as such in code: the streams per globular (1.5, from
`tables::streams`), the orphan mass
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
  the 2% floor is not enforced, since a galaxy with no recent dwarf has under 1% in tubes. This
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
