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
  `global_list::discrete_share(&GalaxyParams, &PotentialTables) -> f64`, which `Galaxy` stores
  through plan 09's `FeatureShares::set_halo_discrete`.
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
  gains `stream` and `dwarf_core` variants.
- Server: `TubeTableCache` (byte-bounded LRU implementing `TubeLookup`) and a background warm-up
  job. Client: `StreamTrackOverlay` for the galaxy map, `StreamReadout`.
- Test helpers: `global_list::testing::{pal5_like, gd1_like, sagittarius_like}` stream
  specifications at Milky Way parameters.
- Domain tags under `stream.` and `dwarf.`, each an entry of plan 01's registry `rng/tags.rs` under
  a "Plan 10" heading, added by the task that first draws on it.

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
  wasmtime, outside `just ci`) is what is left (see Risks).
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
- **Plan 03:** the merge hook `SystemSource` (`galaxy::query::SystemSource`: `expected_in_sphere`,
  `systems_in_sphere`, `suppresses`), `SystemRecord::from_parts` (it matches on the origin
  exhaustively, so `GlobalListMember` needs an arm there), `placement::SystemOrigin`
  (`#[non_exhaustive]`; this plan adds `GlobalListMember`, design note 13), the padding rule,
  `resolve`'s dispatch on `SystemIdKind`. `resolve(galaxy, id)` takes no context, and a stream
  member cannot be resolved without its tube table, nor a dwarf core's without the list's
  `DwarfCoreSpec`, so this plan adds `resolve_with(galaxy, id, &dyn TubeLookup)` beside it, and
  `resolve` goes on answering plan 03's `KindNotGenerated` for both kinds. The padding rule is
  `query::motion::pad_for(t, speed)`.
- **Plan 04/05:** request layer, CPU pool, byte-bounded caches; the galaxy map, the chart, the
  readout.
- **Plan 06:** stellar evaluation of members, lifetimes and the evolved mass function;
  `math::normal_quantile` (P06.T1.b), which the truncated normal of P10.T8.b inverts with. _As
  built:_ members' stars through `MemberRecord::stars`; lifetimes `galaxy::fates::StellarFates::
lifetime` (only `ProvisionalFates` implements it); no item is named the evolved mass function,
  the nearest being `galaxy::fates::{mean_present_mass, mean_present_mass_of_mixture, stars_below}`
  (P10.T8.a to choose).
- **Plan 08:** the halo components' velocity laws, from which orphan progenitors draw their orbits;
  straight-line drift. _As built:_ `kinematics::halo::HaloKinematics` through
  `KinematicTables::halo()`; `query::motion::{position_at, epoch_velocity}`.
- **Plan 15:** `tables::streams::ORPHAN_STREAMS_PER_GLOBULAR`, provisional 1.5, which P15.T11.b
  later fits with this plan's generator. _Not built at P10.T1:_ no `tables/streams.rs` exists, so
  P10.T3.b, its first reader, needs it first.
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

## Design notes

1. **The drawn discrete share is a ceiling, and the physical share is what is applied.** Plan 02
   draws a discrete share of 2–15% and leaves it unapplied. This plan derives the physical share:
   the expected systems in tubes and cores inside the cube ÷ the halo's budget, from 1.5 tubes per
   expected globular at the orphan law's mean mass, plus each recent progenitor's cold mass times
   the share of its orbital period spent inside 65,000 ly (a fixed quadrature of dr ÷ v_r in the
   mid-plane potential, no integration). φ for the halo is the smaller of the two. When the drawn
   share is the smaller, the dwarfs' cold masses are scaled down together to meet it; when the
   physical share is the smaller, the smooth components keep the difference. Both come from galaxy
   parameters alone, so `Galaxy` holds φ before any list is built. Reason: a halo of about 1% of the
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
   P10.T2.a's error test, not by this note.
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
    `ObjectKey::from(SystemId)`, as plans 03 and 09 do.

## Tasks

P10.T1 comes first; P10.T2 can run beside it. P10.T3 needs P10.T2. P10.T4 and P10.T5 follow P10.T3
in order. P10.T6 needs P10.T1 and P10.T5. P10.T7 needs P10.T1 and P10.T3 and can run in parallel
with P10.T4–T6. P10.T8 needs P10.T6 and P10.T7. P10.T9 follows P10.T8, P10.T10 follows P10.T9,
P10.T11 follows P10.T10, and P10.T12 closes the plan. New draws open `Stream::open(seed, tag, key)`
on domain tags under `stream.*` and `dwarf.*`, which each task adds to plan 01's registry
`rng/tags.rs` under a "Plan 10" heading, with the keys of design note 14.

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
built" in Risks. T3.d waits for v16._

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
  `FeatureShares::set_halo_discrete`; the halo's smooth components carry 1 − φ of its budget. Bump
  `GENERATOR_VERSION`, regenerate goldens. Tests: never above the drawn share; globular tubes alone
  are 0.3–1% of the halo; the distribution of the applied share over 2,000 seeds is recorded in a
  golden summary; evaluating it costs under a millisecond.

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

### P10.T5 The particle spray

- **P10.T5.a Release schedule.** About 2,000 tracers per stream, half from each Lagrange point,
  released at step boundaries by stratified sampling of the mass-loss history over [−T_s, 0]
  (constant fractional rate for a living globular, ending at the dissolution time for an orphan,
  pulsed at pericentres for a dwarf), each with weight = mass-loss rate × interval. Tests: weights
  sum to the tube's mass; an orphan releases nothing after its dissolution.
- **P10.T5.b Release and integration.** The progenitor is integrated backwards from its epoch state
  to −T_s and forwards again; at each release the tracer starts at the tidal radius from the
  progenitor along the galactocentric radius, with the offsets in position and velocity and their
  scatters of Fardal, Huang and Weinberg (2015), to be taken from the paper. Tracers feel the
  table's force plus a Plummer sphere for a living progenitor, with its mass following the history.
  Each body accumulates the angle it sweeps in its own orbital plane, step by step, with
  `math::atan2`; ψ is the tracer's angle minus the progenitor's over the same interval. Tests: ψ is
  positive for every leading and negative for every trailing tracer and monotone in release time on
  average; for `pal5_like` the tracers' mean track departs from the progenitor's orbit by more than
  six tidal radii within one wrap, which is the brainstorm's reason for measuring.

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
  `pal5_like`, targets about 0.2 s and about 80 kB.

### P10.T7 Dwarf cores

- **P10.T7.a The model.** `DwarfCoreModel::from_spec`: a Plummer profile flattened along an axis of
  its grid with a half-mass radius from the dwarfs' size–mass relation, up to 8,500 ly;
  `NestedGrid::new(w, 16, 8)` with w a power of two in 16–64 ly, chosen by plan 09's
  `MemberClassTable::grid_width` rule with that floor; a class table by plan 09's device with no
  segregation (q = 1) and no remnant losses; ages from a `StarFormationHistory` of the core's own
  (an old burst at 10–13 Gyr plus a tail that lasts longer the more massive the dwarf, ending at
  accretion); metallicity from the dwarfs' mass–metallicity relation, [Fe/H] = −1.69 + 0.30 ×
  log₁₀(M★ ÷ 10⁶ M☉) with 0.17 dex of scatter among members (Kirby et al. 2013, a source the
  brainstorm does not list; re-check); bulk motion from the core's integrated orbit at the epoch and
  the Plummer dispersion. Pure functions of a `DwarfCoreSpec`; nothing is placed yet. Tests: the age
  distribution of a 10⁸ M☉ core spreads over more than 2 Gyr; the profile integrates to the spec's
  system count; the extent stays under the brainstorm's 20,000 ly.
- **P10.T7.b Members.** Members are placed and resolved with plan 09's machinery (P09.T21, T22)
  under `DwarfCoreMemberId`, with records per design note 13 and a `FeatureLevelList` for catalogue
  classes, which holds the supernova classes of plan 09 (Type Ia from the delay-time distribution on
  the core's own history; core collapse only while the history still forms stars).
  `resolve_dwarf_core_member`, reached from `resolve_with`. Tests: members are not coeval; profile
  by Kolmogorov–Smirnov; no cell above 8,192 candidates; every member ID round-trips through
  `SystemId::from_raw` (no inner cell, band 7 only with level and cell zero); order independence;
  goldens.

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
  dispersions by band against the table.

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

### P10.T10 Protocol and server

- **P10.T10.a Protocol.** New kinds of plan 04's `RequestBody` and `ResponseBody`, added to
  `REQUEST_KINDS`: `GlobalFeatures { universe }` → `GlobalEntryWire` list (kind, number, mass,
  bounding radii, the living progenitor's feature ID as plan 09's `FeatureIdHex`,
  `TrackStatusWire`); `StreamTrack { universe, stream }` → per arm a polyline of at most 128 points
  with widths and mean speed, or `Pending`. `SystemOriginWire` gains `stream` and `dwarf_core`.
  `just gen-protocol`. Tests: wire forms for each message and variant.
- **P10.T10.b Server.** The server keeps the `GlobalList` with each universe (built on the CPU pool
  when the universe opens), serves tube tables from `TubeTableCache` over plan 04's `ByteLru`,
  builds on a miss inside the job that asked, registers `GlobalListSource` with the range-query
  handler in place of plan 09's bare `CentreMemberSource`, resolves IDs through `resolve_with`, and
  warms every table in the background at low priority after a universe opens (one to two
  CPU-minutes; about 20 MB of tables at Milky Way values and up to 120 MB for 1,500 tubes, inside
  the cache's byte bound, so eviction is normal and safe). Tests: an integration test that a range
  query inside a stream returns members with the `stream` origin; a `Pending` track becomes `Ready`;
  an evicted table is rebuilt to the same bits.

### P10.T11 Client

The galaxy map gains `StreamTrackOverlay`: tracks as thin lines in both views, dwarf cores as
feature marks, a toggle and a legend, and a stated count of tracks still pending, since unfetched
data must not look like absence. The chart's readout shows a member's stream or core, and
`StreamReadout` shows a selected stream's mass, length, width, dispersion and progenitor. The UX
guide gains the stream line style. Tests: projection of a track as a pure function; component tests
for the overlay's pending state and the readouts.

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
the plan's doc comment with the machine they ran on.

## Verification

- `just ci` green after every task, on both of plan 01's CI architectures; `just test-slow` holds
  P10.T3.c, T3.d and T12. _Until a workflow returns (ruling 146.5):_ `just ci` on x86-64, and
  `just test-wasm` on `hyperion-sim` at each orbit or tube re-bless and at the plan's close, its
  wasmtime version recorded (P10.T2.c).
- The brainstorm's Testing lines in scope: stream members stay in their tubes (P10.T12); budgets
  (P10.T3.d, T12); order independence and goldens (P10.T4, T6.c, T8.b); determinism of the leapfrog
  and the tube table on x86-64 and AArch64 (P10.T2.c, T6.c; AArch64 waits for a workflow, and
  `wasm32-wasip1` stands in, ruling 146.5).
- Cost figures as benchmarks, targets not promises: list build about 1 s; tube table about 0.2 s and
  80 kB; about 160 members generated for a 50 ly query on a globular stream.
- By eye in the `GALAXY` display: tracks crossing the halo in both map views, leading and trailing
  arms of unequal length, a gap where an orphan's progenitor was, a dwarf's wraps, and a chart on a
  stream's axis showing a thin line of old dwarfs.

## Generator version

One bump, in P10.T3.d, when the halo's field gives up the derived discrete share. Stream and core
members are additions and move nothing. Reserved: the `10` sub-kind `11`; the three unused bits of a
stream member's ID; stream numbers above the list's length; `dwarf.*` and `stream.*` domain tags.
Parameters that belong to the generator version and are named as such in code: the streams per
globular (1.5, from `tables::streams`), the orphan mass law, η's constant, the step rule, the tracer
count and kernel width, the gap noise's amplitude and wavelengths, the cut at four widths, the
dwarfs' stripping fraction, the shell-widening factor.

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
