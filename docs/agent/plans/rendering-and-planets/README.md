# Rendering and Planets: Roadmap

Action plans for building what
[the rendering and planets brainstorm](../../brainstorming/rendering-and-planets.md) designs. This
file is the index: what each plan covers, the order they run in, what they ask of other plans, and
where they differ from the conventions the galaxy plans share. The brainstorm is the specification.
A plan says how to build a part of it, never what to build, and where a plan and the brainstorm
disagree the brainstorm wins until it is revised. The corrections the plans' research found are
collected [below](#brainstorm-corrections) so that the revision can be made in one pass.

## Outcome

The owner's goal, as the brainstorm's
[Suggested order of attack](../../brainstorming/rendering-and-planets.md#suggested-order-of-attack)
closes on it: a free camera that switches any view between the wireframe and the photorealistic
style, anywhere the ship's knowledge reaches, with still images on request.

- **The view.** A `VIEW` display draws the surroundings at real scale, in perspective, from a free
  camera with seat, chase and free presets, in two styles of one renderer over one server scene: a
  wireframe in the graphic language of an Artemis console, and a photorealistic image in absolute
  photometric units, exposed by a photographic model and tone-mapped by AgX. Several views share
  one device; a bridge's main screen shows a camera that is ship state.
- **The sky and the bodies.** The stars are the galaxy's own, each at its retarded time, to the
  naked-eye limit per direction; bodies are lit by their system's stars from plan 14's photometry,
  with their atmospheres computed from composition.
- **The planets.** Every solid body has a coarse field computed on the server and a shared height
  function synthesised on both sides, drawn from orbit to a metre above the ground on a cube-sphere
  quadtree, with authoritative materials and rocks, clouds, oceans and rings; what the ship has not
  surveyed stays undrawn as terrain, and every readout states its uncertainty.
- **The record.** Every figure of the brainstorm's performance budget has a measured counterpart on
  the UHD 620 and a discrete GPU of the RTX 4060 class, and the settings ladder follows them.

The app works at the end of every milestone. No plan leaves `just ci` failing.

## Plans

| Plan                                                   | Title                                                                    | Milestone | Depends on                                                   |
| ------------------------------------------------------ | ------------------------------------------------------------------------ | --------- | ------------------------------------------------------------ |
| [R01](01-graphics-platform-and-engine.md)              | Graphics platform and the engine adapter                                 | RM1       | none                                                         |
| [R02](02-real-scale-view-and-wireframe.md)             | Real-scale foundations and the wireframe `VIEW`                          | RM1       | R01; R03 for generated scenes (R02.T17 only)                 |
| [R03](03-scene-subscription-and-transport.md)          | The scene subscription and bulk transport                                | RM1       | galaxy 04, 12, 14                                            |
| [R04](04-cross-target-determinism.md)                  | Cross-target determinism and the crate split                             | RM1       | none                                                         |
| [R05](05-terrain-geometry-and-descent-spike.md)        | Terrain geometry and the descent spike (the gate)                        | RM2       | R01, R02, R04                                                |
| [R06](06-the-sky.md)                                   | The sky                                                                  | RM3       | R02, R03; galaxy 06, 09 (from P09.T40), 12                   |
| [R07](07-lit-bodies-styles-and-main-screen.md)         | Lit bodies, the photorealistic style, several views and the main screen  | RM3       | R02, R03, R06 (R05 read); galaxy 14; sessions (Phase C only) |
| [R08](08-atmospheres.md)                               | Atmospheres                                                              | RM4       | R05, R07; galaxy 14                                          |
| [R09](09-surface-generator.md)                         | The surface generator: coarse pass, detail synthesis, Knowledge coverage | RM5       | R03, R04, R05; galaxy 12 (P12.T7), 14                        |
| [R10](10-terrain-on-generated-worlds.md)               | Terrain on generated worlds                                              | RM5       | R05, R09 (R07 and R08 read for lit shading)                  |
| [R11](11-surface-detail-clouds-oceans-rings-stills.md) | Decoration, scatter, clouds, oceans, rings and still images              | RM6       | R08, R10; galaxy 14                                          |
| [R12](12-measurements-and-settings-ladder.md)          | Measurements and the settings ladder                                     | RM6       | all of R01–R11, as built                                     |

Each plan's header gives its own dependencies, which this table repeats. The plans were written in
parallel on 2026-09-29, against the tree at `4abab50` and each other's drafts; every later plan is
re-validated against the code (the `revalidate-plan` skill) when its turn comes, and only R01–R04
are expected to run close to as written.

The order is looser than the milestones. R03 and R04 need no other R-plan and can run beside R01
and R02; R04's first three tasks are the brainstorm's "done now" and run before anything else
([below](#three-things-done-now)). R09 needs only R03, R04 and R05, so the surface generator can
start once the gate has passed, beside RM3 and RM4. R07's Phase C (the main screen) waits on a
sessions plan that does not exist, and nothing else waits on Phase C except R12's main-screen run.

### Milestones

| Milestone | Plans   | What the app gains                                                                                                                                                                                                                                                                                                                                                              |
| --------- | ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| RM1       | R01–R04 | WebGPU on this machine's UHD 620, with graphics faults and a safe mode on a `GRAPHICS` panel; a `VIEW` display (`F4`) drawing a wireframe at real scale from the free camera, bodies as graticules, orbits, hulls, and stars at their true magnitudes from the range rows; generated scenes from the scene subscription; both WebAssembly targets in `just ci`, the crate split |
| RM2       | R05     | A planet's terrain from orbit to a metre above the ground on a hand-made Earth-sized test planet, with Earth's atmosphere, in a scripted descent; `TERRAIN: STREAMING` and `TERRAIN: DETAIL LIMITED`; the gate's recorded verdict                                                                                                                                               |
| RM3       | R06–R07 | The galaxy's own sky, each star at its retarded time, with the band and the host stars as limb-darkened discs; bodies lit at real scale; the photorealistic style and the style switch; instrument views beside it; once sessions exist, the main screen                                                                                                                        |
| RM4       | R08     | Every atmosphere plan 14 gives a body, computed from its composition, seen from the ground, from orbit and across the system, thick ones checked against a path tracer                                                                                                                                                                                                          |
| RM5       | R09–R10 | Generated worlds: the server's coarse field, surveyed coverage, the shared height function on three targets; surveyed terrain drawn in both styles with authoritative materials, horizon-map shadows and readouts that state their uncertainty                                                                                                                                  |
| RM6       | R11–R12 | Decoration, authoritative rocks, clouds, oceans with glint, rings close to, still images; the budget measured on both GPUs, the ladder adjusted and every low setting audited                                                                                                                                                                                                   |

## The gate

R05 is the gate: the brainstorm's descent spike (step 3), which decides whether the browser can
carry the planets before anything depends on the answer. The scripted, seeded descent passes, on
the criterion of [R05's Design note 21](05-terrain-geometry-and-descent-spike.md), at 1080p60 on a
discrete GPU of the RTX 4060 class and at 30 fps at 720p on the UHD 620's low setting, each as frame
intervals from presentation times over the whole descent and per segment, with headroom, the other
budget rows and resident memory counted.

What a failure means is open question 2's rule, operationalised by R05's Design note 22 and applied
by R05.T19:

- **A failure on the UHD 620 alone** redesigns the low setting. It never triggers a native
  renderer.
- **A failure on the discrete machine** is first priced: the descent is re-run with Dawn's safety
  checks off, and a captured span is replayed natively in wgpu. The rule fires only if the replay
  meets the budget where the browser misses it by more than a fifth, or if our CPU time and the
  GPU's pass time both meet the headroom row while the delivered frames miss. Then the owner is told
  before any later plan depends on the browser. Otherwise the failure is ours, and the verdict names
  the pass or the thread to fix.

The project has no discrete GPU. R05.T17 and R12.T9 need a physical desktop with a real monitor,
borrowed or bought; a cloud GPU's figures are advisory and cannot sign off the gate. Until then the
gate is half closed.

## Three things done now

The brainstorm has three things done now rather than in a step, because they cost little today and
a great deal later. All three are R04's, need nothing else, and should land first:

1. **R04.T3**: galaxy plan 14 is amended so that `BodyHooksDto` carries `detail_seed` in place of
   `surface_seed`, and the client's parser follows, **before P14.T23 lands**. The wire already has
   `surface_seed` (as `not_modelled`, in `crates/hyperion-protocol/src/planetary/record.rs`), so
   the amendment also changes the DTO and its client mirror. R09 later registers
   `body.surface.detail` and fills the field.
2. **R04.T1**: the `algebraic_*` float methods are banned in every `clippy.toml` (the root's, the
   sim's and the fitting crate's today; `hyperion-base`'s and `hyperion-surface`'s once R04.T4 and
   T5 create them), and a test holds all five to one list.
3. **R04.T2**: the sim-determinism skill and the `planetary_*golden.rs` headers stop claiming that
   CI checks 64-bit Arm and wasm32, and say what runs.

## Conventions

The galaxy roadmap's
[Conventions every plan follows](../galaxy-generation/README.md#conventions-every-plan-follows) bind
these plans: the ten-section plan layout, the code shape, the generator version, the tests and the
Figures rule. Only what differs is stated here.

- **Task IDs** are `R<nn>.T<n>`, with subtasks `R05.T3.a`. Galaxy tasks keep `P<nn>.T<n>` and are
  cited as "galaxy plan 14" or `P14.T23`. The `implement-task` and `validate` skills' scripts match
  only `P..` IDs today (`plan_task.py`'s `ID_RE`, `select_checks.py`'s `TASK_RE`), so they need the
  `R` prefix before they can pick up these plans' tasks (see
  [Open across plans](#open-across-plans)).
- **Two crates join the workspace** (R04). `hyperion-base`, beneath the sim, holds `math`, `rng`
  (with the part of `id` it needs), `units` and `version`; `libm` pinned with `=` becomes its one
  runtime dependency, and the sim re-exports every path it exposed, so `hyperion_sim::math` and the
  rest stay valid. `hyperion-surface` holds the shared height function (R05's provisional one, then
  R09's), the material classes (R10) and the rocks (R11), and builds for native and both wasm
  targets; relaxed SIMD is a `compile_error!` there. The sim depends on both. The galaxy
  convention's "the sim gains exactly one runtime dependency" now reads as base's.
- **Three tag registries.** Base, surface (`hyperion_surface::tags`, for `surface.*`) and sim each
  keep one, and one compile-time disjointness assertion in the sim covers all three, in place of
  the galaxy convention's single `rng/tags.rs`.
- **Five `clippy.toml` files**, the root's, the sim's, the fitting crate's, base's and the surface
  crate's, each self-contained and each banning the `algebraic_*` methods, held to one list by
  `crates/hyperion-testkit/tests/clippy_bans.rs`. The terrain hazards are a section of the
  sim-determinism skill, which reaches the new crates.
- **Goldens on three targets.** Native, `wasm32-wasip1` under wasmtime and `wasm32-unknown-unknown`
  under `wasm-bindgen-test` on Electron's own V8 (a `node` shim on `PATH`) all run their fast
  goldens in `just ci`, which fails and never skips when a tool is missing; the slow wasip1 suite
  runs in `just ci-slow`. Golden height files live under `crates/hyperion-surface/tests/golden/`.
- **GPU checks are by hand and recorded.** No test in `just ci` needs a GPU. The headless
  SwiftShader harness (`just test-render`, R01) renders every shader registered in `WGSL_CATALOGUE`
  and asserts properties of the read-back frames; it stays outside `just ci`, so every task that
  adds or changes a catalogued shader, `view/engine/` or `src/smoke/` runs it as part of its own
  gate. Checks on real GPUs (the three-canvas proof, the soak, the descent, frame-time benches) are
  run by hand and written into the plan's as-built notes or the results files.
- **No golden images.** Neither in CI nor by hand: images are checked by asserted properties
  (finiteness, flux, positions, colours at pixels), never compared with a stored picture.
- **Measurements on a quiet machine.** The development machine is shared with other agents' builds
  and tests, which moved every timing taken while these plans were written (load averages of 14 to
  21, the package at 96–97 °C). Every timing measured on 2026-09-29, in a plan or its research, is
  provisional. A bench or recorded run counts only when taken with no other agent, lane or test
  running, the load average under 1 at the start and recorded with the governor; a run under load is
  marked provisional and repeated (R05's Design note 27, R12's Design note 7). A missed budget
  target is a finding for R12, not a failure.
- **Results live under `docs/measurements/`.** R05's runs go to `docs/measurements/descent-spike/`,
  R12's consolidated record and generated budget table to `docs/measurements/rendering/`
  (`runs.v1.jsonl`, `budget.md`); every other plan records its benchmarks in its as-built notes,
  which R12 folds in. The directory is not yet in `.claude/CLAUDE.md`'s documentation layout; R12.T1
  adds it if it is still missing.
- **Low settings are built with the high ones**, in each feature's own plan (the budget's third
  rule); R12 audits them and builds none.
- **Guide edits are drafts.** Every edit of `docs/frontend/ux-guidelines.md` is drafted for the
  owner and ends in "the owner signs off"; the client is built to the draft meanwhile, as the galaxy
  slice was. R02.T2 drafts the brainstorm's nine items in one pass and absorbs R01's nomenclature;
  later plans add only their own entries.

## Asks of other plans

Every ask the R-plans make of galaxy plans, of the sessions plan that is not yet written, and of
each other. "Carried" means the owner plan's text already has it; "drafted" means the asking task
writes it into the owner plan for its owner to accept; "open" means the owner plan does not yet have
it. R-plans that ask each other were reconciled on 2026-09-29; the open ones are also listed in the
owner plan's Risks and open points.

### Of galaxy plans

| Owner   | Ask                                                                                                                                                                                               | Asked by                     | State                                                                      |
| ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------- | -------------------------------------------------------------------------- |
| Plan 04 | Rows in the reserved-kinds table: `scene_ship`, `scene_cameras`                                                                                                                                   | R03.T1                       | drafted                                                                    |
| Plan 04 | Row `sky`, large size class                                                                                                                                                                       | R06.T1                       | drafted                                                                    |
| Plan 04 | Rows `surface_field`, `survey_pass`                                                                                                                                                               | R09.T0.a                     | drafted                                                                    |
| Plan 06 | An optional `absolute_v_mag` on `StellarBriefDto`, from log L and T_eff                                                                                                                           | R02.T5                       | built by R02.T5 if plan 06 has not                                         |
| Plan 06 | A1: `BriefModel::new_for(galaxy, record, earliest_emitted)`, valid over the retarded interval (open question 17)                                                                                  | R06.T1                       | drafted; interim in R06                                                    |
| Plan 06 | A2: the Humphreys–Davidson ruling on cool supergiants above log L 5.8 (research lean: floor the LBV rate, add Hurley's LBV term; a bump)                                                          | R06.T1                       | drafted; a physics ruling for plan 06                                      |
| Plan 06 | A3: Class 0/I protostars dark in V                                                                                                                                                                | R06.T1                       | drafted; interim `sky::photometry::is_dark_in_v`                           |
| Plan 06 | A4: white dwarfs' absolute V (Montreal grids) and gravity-dependent bolometric corrections for late M giants; and a check of `photometry.rs`'s "Table III" citation of Straižys and Kuriliene     | R06.T1                       | drafted                                                                    |
| Plan 07 | `galaxy::gas::extinction::profile`, cumulative A_V along a ray, built in plan 07's module under its rules                                                                                         | R06.T9.a                     | built by R06; for plan 07's owner to review                                |
| Plan 03 | `galaxy::placement::generate_cell_where`, bit for bit `generate_cell` filtered by mass                                                                                                            | R06.T6.a                     | built by R06                                                               |
| Plan 09 | `FeatureMemberSource` registered in the server (P09.T40)                                                                                                                                          | R06.T16                      | carried by plan 09; R06.T16 waits                                          |
| Plan 11 | `stellar::multiplicity::star_states_at(h, t, out)`, each star's velocity beside `star_positions_at`                                                                                               | R03.T3                       | built by R03.T3 in plan 11's file if absent                                |
| Plan 12 | P12.T9's subscription envelope (`subscribe`, `unsubscribe`, `Notification`), to plan 12's design, with `Scene` as the first topic; a note in P12.T9                                               | R03.T1, R03.T5               | built by R03 if P12.T9 has not landed                                      |
| Plan 12 | `observe::{SystemTrajectory, retarded_in_system, SystemObserver, InSystemRetardation}` in `observe/in_system.rs`                                                                                  | R03.T2                       | built by R03 in plan 12's module                                           |
| Plan 12 | P12.T7's `KnowledgeStore`, on which `knowledge/surveys.v1.jsonl` is built                                                                                                                         | R09.T18                      | carried by plan 12, unbuilt; R09.T18 waits                                 |
| Plan 14 | The amendment of P14.T23 and `BodyHooksDto`: `detail_seed` on the wire, the surface seed server-only                                                                                              | R04.T3                       | drafted (edits plan 14 before P14.T23)                                     |
| Plan 14 | `body_fixed_at(body, t)` (P14.T14.c) returning R02's `coords::BodyFixedRotation`, one rotation type                                                                                               | R02 (Design note 14)         | open; graticules read `ROTATION NOT YET MODELLED`                          |
| Plan 14 | `PlanetarySystem::state_at(ctx, index, t)`, a body's velocity beside `position_at`                                                                                                                | R03.T3                       | built by R03.T3 in plan 14's file if absent                                |
| Plan 14 | A `photometry` section on `BodySummaryDto` at `Bulk` (p in B, V, R; phase template and s; L; the ratio p_V q_V ÷ A_Bond); two checks of the Bond albedo against p (the Moon, the Earth)           | R07.T1                       | drafted                                                                    |
| Plan 14 | A flattening by Darwin–Radau with C/MR² per composition class, capped at 0.2, skipped below about 250 km                                                                                          | R07.T1                       | drafted                                                                    |
| Plan 14 | P14.T24.c: the aerosol and absorber inventory with a shape class per material, and when ozone, haze, condensates and dust form                                                                    | R08.T1                       | drafted, shown to the owner before commit                                  |
| Plan 14 | P14.T24.d: a gas envelope's visible atmosphere (T_int, T_irr, Guillot profile to an adiabat, species by E(M) and [Fe/H], cloud decks by saturation)                                               | R08.T1                       | drafted                                                                    |
| Plan 14 | P14.T24.e: the vertical structure as (T_s, p_s, β, T_skin), with `temperature_at(p)` in the sim                                                                                                   | R08.T1                       | drafted                                                                    |
| Plan 14 | P14.T24.f: carbon speciation (CH₄ on cold worlds) and an abiotic O₂ source; P14.T35 carrying all of it on `BodySurfaceDto`                                                                        | R08.T1                       | drafted                                                                    |
| Plan 14 | σ_h by open question 20's ruling (σ_h² = σ_struct² + σ_crat² + σ_volc², no g in the structural share); greatest relief withdrawn as a published figure                                            | R09.T0.a                     | drafted                                                                    |
| Plan 14 | The volatile history (`WetEpoch`), the crater contract (N(>1 km), screening, g, k_target), classifier corrections for open question 9, a continental fraction, and the surface section's contents | R09.T0.a                     | drafted                                                                    |
| Plan 14 | Rotation, poles and `body_fixed_at` (P14.T14), and the surface section carried (P14.T24.a–b, T35): ocean, ice and cloud fractions, surface age, crater density, `SurfaceState`, `SurfaceMaterial` | R07, R08, R09, R10, R11      | carried by plan 14, unbuilt                                                |
| Plan 14 | A ring radial profile from processes (optical depth, albedo, spectral slope), the particle size distribution and vertical thickness on `RingDto`                                                  | R11 (Design notes 13, 16)    | open; provisional profile in R11                                           |
| Plan 14 | A cloud fraction that depends on the condensable's availability, not on the surface state alone (`cloud_fraction` gives every `Temperate` world 0.67, Mars included, and `Snowball` 0)            | R11 (Design note 9)          | open; a code finding for plan 14                                           |
| Plan 14 | Condensate cloud species in the aerosol inventory (R08's P14.T24.c)                                                                                                                               | R11.T7                       | drafted through R08.T1                                                     |
| Plan 14 | Pinnable bodies with ocean, cloud and ice fractions, an atmosphere and rings, through `body_detail`                                                                                               | R12.T5.b                     | carried once plan 14's sections exist                                      |
| Plan 14 | A zodiacal cloud, if zodiacal light is wanted in the sky's background                                                                                                                             | R06 (Risks)                  | open; not asked formally                                                   |
| Plan 15 | New kinds of `hyperion-fit` output, client fixtures rather than sim tables: `atmosphere-reference` (with `serde_json`), `ring-shadowing`; and the colour and limb-darkening tables                | R08.T12, R11.T9.b, R06.T3–T4 | built by the asking plans; plan 15's owner may move them to a sibling tool |

### Of the sessions plan and later plans (not written)

| Owner        | Ask                                                                                                                                                                                                                                                                                                                                                             | Asked by                                  |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------- |
| Sessions     | A session with a ship, its name and clock, replacing R03's per-universe ship stand-in and scene clock; craft through R03's `CraftSource`; `SceneCraftDto`, of which R03 has a draft                                                                                                                                                                             | R03.T6, R03 (Design note 4)               |
| Sessions     | For the main screen: station identity (`StationDto`, at least `captain` and `helm`), control arbitration, ship commands with closed-loop results (`PENDING` within 100 ms), input sent on change as well as at the 64 Hz tick, `session.json`, the replay log, the modes, ship alerts in the guide's four classes, a station-local predicted view for the flyer | R07.T22–T28 (Phase C)                     |
| Sessions     | The single-player pool cap and height-worker count the cockpit run supports                                                                                                                                                                                                                                                                                     | R12.T12                                   |
| Craft plan   | Hull definitions emitting R02's `HullOutline`; the ship adopting `planetary::frame::select_body_frame`                                                                                                                                                                                                                                                          | R02 (Design notes 6, 15)                  |
| Flight model | The consumer of `finest_surface_height`, R10's `ground_at` and R11's `surface_height_with_rocks`; the body-centred frame's tidal residual; drag through R08's vertical structure; landing hazard margins as multiples of R10's 1σ                                                                                                                               | R05, R10, R11, R02, R08                   |
| Sensors plan | The body-level overlay of which detail level the ship holds; survey passes from sensors (orbital, close-range, landed during a descent) instead of R09's explicit request; a server request for terrain readouts on consoles that draw no terrain                                                                                                               | R09 (Design note 16), R10 (Design note 7) |

### Between rendering plans

Asks met in the owner plan are listed there under Consumes or Design notes and are not repeated;
these are the ones the owner plan does not yet design, each also in its Risks and open points.

| Owner | Ask                                                                                                                                                             | Asked by                  | State                                 |
| ----- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------- | ------------------------------------- |
| R01   | A probe of each adapter's render-target rounding mode, in `GraphicsStatus` (Gen9 rounds toward zero)                                                            | R07 (Design note 12)      | open                                  |
| R02   | An oblate graticule for giants drawn oblate                                                                                                                     | R07 (Design note 19)      | open                                  |
| R04   | A `j0` wrapper over `libm::j0` in base's `math`, for R10's slope uncertainty                                                                                    | R10 (Design note 7)       | R04.T4.c                              |
| R05   | A `PlanetGeometry` of a reference spheroid at zero height, with no height worker, for R07's `mesh` regime                                                       | R07 (Design notes 3, 19)  | open                                  |
| R05   | `bake_patch`, `finest_surface_height` and selection's bound taking their height source as a trait or argument, not `&TestPlanet`                                | R10 (Design note 4)       | open                                  |
| R05   | Transmittance stored as optical depth, the ozone term through a curve of growth, the channel wavelengths refitted to 620, 540 and 445 nm                        | R08 (Design notes 5, 8)   | done by R08's own tasks in R05's code |
| R05   | `main/fdinfo.ts` parsing `drm-shared-system0` and the stolen keys; a versioned results-file schema; creation sites naming a memory item                         | R12.T1, R12.T3            | extended by R12's own tasks           |
| R06   | The disc pass writes meter weight 0 in the HDR alpha; eye views get `GlareSource`s for bright sources up to about 45° outside the frame                         | R07 (Design notes 10, 12) | open                                  |
| R06   | Each colour-table row's spectrum at R08's `BAKE_WAVELENGTHS_NM` (15 over 380–760 nm)                                                                            | R08 (Design note 5)       | open                                  |
| R07   | `body_brdf` fed per-texel lunar-Lambert parameters; the disc sampling R10's class-weights map; the horizon term taking its local horizon from R10's horizon map | R10 (Design notes 8, 10)  | open                                  |
| R07   | The instrument panels' sizes, stable `PassList` labels, the resolution controller's bounds as a setting value                                                   | R12 (Consumes)            | open                                  |
| R08   | The sun's refracted apparent elevation, where drawn, for R10's shadow test (R08 draws no refraction today)                                                      | R10 (Design note 10)      | open                                  |
| R09   | The datum on ocean worlds, agreed with R10's reference-sphere datum                                                                                             | R10 (Design note 14)      | open                                  |
| R09   | The terrain-independent zonal part of the precipitation heuristic, callable without the coarse field; whether `ClimateCell.wind` is seasonal or monthly         | R11 (Design note 8)       | open                                  |

## Awaiting the owner

Nothing below is assumed by any plan: each is drafted as a task that ends in "the owner signs off",
and the code built meanwhile is confined to a label, a constant or a gated task.

- **The Content Security Policy** (R04.T10.a). Research reframed the brainstorm's item: a `file://`
  or dev-server worker has no policy of its own and compiles WebAssembly under today's CSP; only the
  page's own thread needs `'wasm-unsafe-eval'`. Three options go to the owner: add
  `'wasm-unsafe-eval'` (the brainstorm's lean), change nothing, or serve through a custom scheme
  with a header CSP on workers. R04.T10.b, and through it R05's height workers, wait on the ruling.
- **The UX guide drafts.** R02.T2's nine items of "What the guide must gain", with R01.T5.c's
  `GRAPHICS` nomenclature absorbed; R06.T15's sky entries (`STARS`, `EYE`, `CAM`); R07.T16's
  photorealistic entries and phrases (`BODY ALBEDO: NOT YET MODELLED`,
  `ONE PHOTOREALISTIC VIEW ON LOW SETTING`, the meters); R08.T2's atmosphere labels; R10.T13's
  readout notation (`~2140 m ± 180 m`, `ELEVATION`, `SLANT RANGE`, `DATUM`, contours); and R11's
  reading of item 2, that "decoration" covers terrain micro-detail and decorative scatter but not
  clouds and waves (R11 Design note 2).
- **Data licences** before data are committed: the Karkoschka and Tomasko methane coefficients
  (Elsevier), Serdyuchenko's ozone data files (terms unstated), and whether raw tables may be
  committed beside the reduced values (R08.T4); H₂SO₄ (Palmer and Williams 1975), Mars dust (Wolff
  et al. 2009) and tholin (Khare et al. 1984), held until checked, and NH₄SH, which has no visible
  index (R08.T5); the stellar spectral libraries behind the colour table (ATLAS9, PHOENIX, TLUSTY,
  Koester), which state no licence and are committed as derived tables with citation, and the CIE
  data under CC BY-SA 4.0 (R06.T3). Filament's AgX is Apache-2.0 and gains a NOTICE entry (R07
  Design note 9).
- **The level-of-detail selection bound** (R10.T4): whether selection takes the hard bound or
  min(hard, 4σ), ruled with T4.a's recorded ratios and patch counts. Until then the hard bound
  selects.
- **Galaxy plan amendments** drafted by R-plans, each for the galaxy plan's owner to accept: R04.T3
  (P14.T23), R07.T1 (photometry, flattening), R08.T1 (P14.T24.c–f; shown to the owner before it is
  committed), R09.T0.a (σ_h, volatiles, craters, classifier), R06.T1 (plan 06's A1–A4), R03.T1 (plan
  04's rows, a note in P12.T9).
- **Brainstorm revisions** drafted by the plans: R05.T19's verdict on open question 2 and its
  findings; R11.T5's "ruled decoration" for open question 18 with corrected depths; R12.T10's
  measured budget tables; R04.T10.a's CSP ruling; and the corrections
  [below](#brainstorm-corrections).
- **The gate's verdict** (R05.T19): if open question 2's rule fires, the owner rules before any
  later plan depends on the browser.
- **A discrete GPU** for R05.T17 and R12.T9: a physical desktop of the RTX 4060 class with a real
  monitor.
- **Smaller rulings the plans leave open**: moving `just test-render` into `just ci` after one
  Electron upgrade passes (R01.T9.e); the safe mode drawing no view, since a Canvas 2D wireframe
  would be a second renderer (R01's Risks); keeping the main screen's pose interpolation, at about
  16 ms, against R07's extrapolation (R07.T28); a disc-statistical glint over unsurveyed ocean (R11
  Design note 11); `docs/measurements/` in `.claude/CLAUDE.md`'s layout (R12.T1); biotic O₂, which
  no plan owns, and the scalar-against-vector Rayleigh error R08.T12.a measures (R08's Risks).

## Brainstorm corrections

Errors and gaps the writers and their research agents found in the brainstorm on 2026-09-29, for
one revision pass. Each gives the brainstorm's section, what it says, the correct statement and the
plan whose design note or risk holds the source. None has been applied to the brainstorm.

### The engine and the platform

| Section                                               | The brainstorm says                                                                    | Correct                                                                                                                                                                                               | Source                                 |
| ----------------------------------------------------- | -------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------- |
| The graphics API, and the Intel problem; Decisions    | The main process sets `--ozone-platform=x11` before `ready`                            | An appended switch reaches only the GPU process; the browser picks Ozone from `XDG_SESSION_TYPE` first. A Wayland session needs a relaunch with the flag on the real command line, or a launcher flag | R01 Design note 3                      |
| The graphics API; Decisions                           | The crash-loop relaunch without Vulkan keeps a mode "without the photorealistic style" | Without the `Vulkan` feature there is no WebGPU adapter under X11 at all: the safe mode has no views                                                                                                  | R01 Design note 5                      |
| The graphics API                                      | Three GPU-process crashes disable GPU mode                                             | Vulkan drops after 3 crashes and compositing falls to software after 6; domain blocking cuts WebGPU after 2 unless `app.disableDomainBlockingFor3DAPIs()` is called                                   | R01 Design notes 6, 9                  |
| Decisions ("The Linux platform")                      | Loss is reported "as a ship-system fault"                                              | A console Fault or status (`GRAPHICS …`), never an Alert or Caution, which a console never invents                                                                                                    | R01 Design note 10                     |
| Runtime and code shape ("loaded lazily")              | A dynamic import and a manual chunk                                                    | Vite 8 / rolldown ignores `manualChunks` once `codeSplitting` is set; the named chunk is made another way                                                                                             | R01 Design note 14                     |
| Open question 15                                      | `_device` and `_disableEngineYFlip` as engine internals                                | `_disableEngineYFlip` is on `WebGPURenderTargetWrapper`; R06 adds a third pinned internal, `_hardwareTexture`                                                                                         | R01 Design note 13                     |
| Testing ("Golden images are rejected for CI")         | The headless run's switch list                                                         | Headless also needs `--ozone-platform=headless --use-angle=swiftshader --enable-features=Vulkan --use-vulkan=swiftshader` and `BrowserWindow({ show: false, webPreferences: { offscreen: true } })`   | R01 Design note 17                     |
| Suggested order of attack, step 3                     | Per-pass GPU time is available "uncoarsened" under the forced switches                 | Timestamps are quantised to 65,536 ns under them; `--disable-dawn-features=timestamp_quantization` (or `--enable-unsafe-webgpu`, as Sources says) lifts it; measurement runs use the narrow toggle    | R01 Design note 4; R05, R07, R11, R12  |
| Runtime and code shape; Decisions; Awaiting the owner | `script-src 'self'` blocks WebAssembly before the first height worker                  | A `file://` worker has no policy of its own and compiles freely; only the page's thread needs `'wasm-unsafe-eval'` (tested on Electron 44.4.3)                                                        | R04 Design note 16; R05 Design note 11 |
| Runtime and code shape                                | "Every ban's path changes in all three `clippy.toml` files"                            | The banned paths (`f64::sin`) do not change when `math` moves; only the `reason` strings do                                                                                                           | R04 Design note 9                      |
| Runtime and code shape; step 7                        | `math`, `rng` and `units` move                                                         | `version` must move too, so base's and the surface crate's goldens can write `GENERATOR_VERSION`                                                                                                      | R04 Design note 2                      |
| Testing (flush-to-zero probes)                        | `black_box(f64::MIN_POSITIVE) / 2.0` "is not zero"                                     | Under DAZ the float comparison reads its input as zero; the probe must compare the result's bits                                                                                                      | R04 Design note 15                     |
| Step 7 and the "done now" amendment                   | "The detail seed plan 14 by then sends"                                                | The DTO field exists from the amendment and is `not_modelled` until R09 registers `body.surface.detail` and computes it                                                                               | R04 Design note 17                     |

### Real-scale foundations, the free camera and the guide

| Section                                            | The brainstorm says                                                                          | Correct                                                                                                                                                                             | Source                                |
| -------------------------------------------------- | -------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------- |
| The floating origin; Decisions ("Positions")       | Drawn positions are apparent "except the camera's local body", drawn geometrically           | Only the ship's local body is drawn geometrically; a free camera's own local body is apparent, or a camera at Jupiter would sit some 80,000 km off its moons with the ship at Earth | R03 Design note 7; R02                |
| The floating origin                                | x_B(t − τ) − x_cam(t − τ)                                                                    | x_B(t − τ) − x_ship(t), aberrated by the ship's velocity at t (SPICE's `stelab_c`), by the exact Lorentz form; "above about 0.01c" is the 1080p threshold only                      | R03 Design note 7                     |
| The floating origin                                | Iterate "at most three times, as SPICE's converged 'CN+S' correction does"                   | Until a change of at most 1 ns: typically three, up to six at a system's reach (hot Jupiters at 100 au need four); cap 10                                                           | R03 Design note 7                     |
| The floating origin                                | The 10 m error of light time for a body 400 km below a ship at 7.7 km/s                      | 10 m is light time and aberration together; light time alone is frame-dependent (0 in the body frame, about 40 m in the system frame)                                               | R03 Design note 7                     |
| The floating origin                                | The non-rotating body frame "is inertial, so the flight model needs no fictitious forces"    | It is free-falling: no Coriolis or centrifugal terms, but the other bodies' tidal residual (the indirect term) remains                                                              | R02 Design note 6                     |
| The floating origin (frame table)                  | "About 1 mm at 50 au"                                                                        | Add: 2 m at a system's tidal radius (about 1 ly), so near views are differenced craft-relative                                                                                      | R02 Design note 22                    |
| The floating origin; Testing (frame-change scene)  | A frame change "with hysteresis"                                                             | The galaxy's rule has none at a sphere's boundary, only between rivals; a Schmitt band (enter at 0.9, leave at 1.0) is added                                                        | R02 Design note 6                     |
| Depth                                              | Depth comparison flips to "greater"                                                          | Greater-or-equal, as Babylon's reversed depth and R01's `DepthPolicy` use (harmless)                                                                                                | R02 Design note 4                     |
| Depth                                              | No global depth bias                                                                         | Hull edges over their own depth-only faces need a pipeline-local bias; worth a sentence                                                                                             | R02 Design note 5                     |
| Luminance in physical units                        | "A pixel is 3 × 10⁻⁷ sr" at 1080p over 60°; a mag 6.5 star in one pixel about 2 × 10⁻² cd/m² | That is the angular mean; the centre pixel is 3.62 × 10⁻⁷ sr, and the star 1.76 × 10⁻² cd/m²                                                                                        | R02 Design note 10                    |
| Luminance; Exposure and tone mapping               | The exposure equations cited from Lagarde and de Rousiers 2014 through bruop                 | Verified (§5.1, eqs. 67–75, pp. 83–85; K = 12.5, q = 0.65); Filament's "Physically based camera" is the better citation. The metered average lands 0.79 stop below AgX's grey       | R02 Design note 11; R07 Design note 9 |
| What the guide must gain, item 9                   | Main-screen text "at least 20′ … MIL-STD-1472"                                               | 20′ is for colour-coded characters (MIL-STD-1472H §5.17.25.14); text generally 10′ minimum, 15′ preferred (§5.17.18.2); warning and caution text 30–60′ (§5.7.3.6)                  | R02.T2.e; R07 Design note 23          |
| The free camera; Decisions ("The main screen")     | 100 ms from input "excluding the display device"                                             | MIL-STD-1472H §5.12.1.4.1.1 counts to the display of the result; television lag can cite RTINGS's method                                                                            | R07 Design note 22                    |
| The free camera                                    | The main screen interpolates between poses                                                   | Interpolation costs a tick (about 16 ms) of the 100 ms; R07 leans to extrapolation, with interpolation the fallback T28 compares                                                    | R07 Design note 22                    |
| Two deployments, one scene; Runtime and code shape | 250 to 300 bytes of JSON a body                                                              | A bare `OrbitDto` is about 280 bytes, a `BodyOrbitDto` about 340, a `BodySummaryDto` with its bulk section 700–900 (estimates; R03.T15 measures)                                    | R03 Design note 4                     |
| Runtime and code shape                             | Only craft pushed at the 64 Hz tick; every push states its time                              | No session, clock or ship exists to supply the time before the sessions plan; R03 adds a per-universe ship stand-in and scene clock                                                 | R03 Design note 2                     |

### Lighting, exposure and the sky

| Section                                   | The brainstorm says                                                                             | Correct                                                                                                                                                                                         | Source                    |
| ----------------------------------------- | ----------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------- |
| Exposure and tone mapping; Luminance      | "A frame holds AgX's 25 stops or so"                                                            | 25 stops is AgX Log's encoding; the formed image spans 16.5 stops (−10 to +6.5 about 0.18)                                                                                                      | R07 Design note 9         |
| Exposure and tone mapping                 | Metering "from the average scene luminance"                                                     | Right only as the arithmetic mean; a log average exposes a dark sky and burns a lit planet                                                                                                      | R07 Design note 10        |
| Luminance (formats)                       | `rg11b10ufloat` for bloom                                                                       | Gen9's render-target writes round toward zero, −0.8% to −1.6% a write; bloom stays `rgba16float` unless the adapter rounds to nearest                                                           | R07 Design note 12        |
| Exposure and tone mapping (glare)         | The CIE glare spread function as printed                                                        | Its first constant is 0.0046°, not the 0.046° of the open copies (normalisation shows it)                                                                                                       | R07 Design note 12        |
| The local star as a disc                  | Eclipses "sampled at a handful of points"                                                       | 13–26% wrong for small occluders at 4 to 8 samples, and bands; limb-darkened annuli with exact circle overlaps, and Howell's closed form for the horizon                                        | R07 Design note 6         |
| The local star as a disc                  | The limb at about 30% of the central intensity                                                  | The power-2 fit gives 0.22 at μ = 0 and 30% at μ ≈ 0.05–0.1; the polynomial cited is poor at the extreme limb                                                                                   | R06 Design note 16        |
| The local star as a disc; Luminance       | Eclipses and the terminator in the sky's section; no planetshine, oblateness, bands, magma glow | Eclipses and the terminator are lighting (R07); planetshine is the dominant night-side light (earthshine about 15 lx); Saturn is 10% oblate; giants' bands and magma oceans' glow have no owner | R07 Design notes 6, 7, 19 |
| The sky (threshold table)                 | Bulge 1.5 kpc from the centre, away: 6.5 at μ 22; a globular core like 47 Tuc: about 5 at 17.7  | 6.43 (6.4) and about 5.3                                                                                                                                                                        | R06 Design note 2         |
| The sky                                   | Backgrounds brighter than μ ≈ 18.9 are mesopic, where eq. 34 applies                            | Eq. 53 already fails below μ ≈ 16.7; eq. 34 is the better single formula over the whole range                                                                                                   | R06 Design note 2         |
| The sky; Decisions; open question 16      | F = 1.4, "Crumey's value for an experienced, dark-adapted observer"                             | 1.4 is the keen end of Crumey's real-world 1.4–2.4 range (1.378 fitted for M33), not a named observer                                                                                           | R06 Design note 2         |
| The sky                                   | M stars 0.3–0.4 mag brighter, O stars about 0.4 fainter (Crumey eq. 18)                         | Eq. 18 through B − V gives about 0.2–0.3 and 0.26; the brainstorm's figures are the blackbody S/P of eq. 7                                                                                      | R06 Design note 3         |
| The sky (glare)                           | "Plus the glare of the brightest resolved stars", as Crumey's                                   | Crumey defers glare to Adrian 1989; CIE 146:2002 general disability glare, θ ≥ 0.1°                                                                                                             | R06 Design note 4         |
| The sky; Decisions; open question 16      | "About V 10 for a video camera"                                                                 | The limit depends on the field: about V 10 at 13°, 9.5 at 60° with a full-frame sensor, about 7 with a small sensor                                                                             | R06 Design note 18        |
| Performance budget (memory, star cubemap) | Baking is correct antialiasing while a texel is no larger than a pixel                          | The low setting's 1,024² faces (6.7′ texels) are larger than a 1080p pixel at 60° (3.3′), so the low setting breaks the rule by design                                                          | R06 Design note 22        |

### Atmospheres

| Section                     | The brainstorm says                                                          | Correct                                                                                                                                                                         | Source                                     |
| --------------------------- | ---------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------ |
| Atmosphere                  | Tables "rebuilt whenever the atmosphere or the sun changes", "built once"    | Transmittance and multiple scattering depend on the atmosphere alone and are shared by every view; sky-view and aerial perspective are per view and per sun                     | R05 Design note 16; R08 Design notes 7, 14 |
| Atmosphere                  | Earth's reference atmosphere at Hillaire's values                            | Hillaire's reference aerosol is tuned, 20–40 times cleaner than Earth's typical sky (τ 5.3 × 10⁻³); Earth's measured aerosol is τ 0.1 at 550 nm, α 1.3, ω 0.92                  | R05 Design note 16                         |
| Atmosphere                  | The 32 km aerial-perspective reach                                           | Hillaire 2020's (Table 2, §5.4) and Bevy's figure; sebh's reference code reaches 128 km                                                                                         | R05 Design note 16; R08                    |
| Atmosphere                  | Render channels at 680, 550 and 440 nm                                       | Bruneton's code constant, not a physical choice; used as channels it makes low suns up to 60% too bright; a fitted 620, 540, 445 nm triple, with bakes solved at 15 wavelengths | R08 Design note 5                          |
| Atmosphere                  | H₂ and He from Dalgarno's cross-sections                                     | Dalgarno and Williams 1962 runs about 6.5% below measured H₂ refractivity: Peck and Huang 1977 with Hohm's King factor; He from Mansfield and Peck 1969                         | R08 Design note 4                          |
| Atmosphere                  | The Rayleigh sources named                                                   | They do not cover O₂, Ar, H₂O or NH₃: Zhang et al. 2008, Peck and Fisher 1964, Ciddor 1996, Cuthbertson 1914                                                                    | R08 Design note 4                          |
| Atmosphere; open question 3 | Venus's cloud optical depth "near 30"; Rayleigh "near 15"                    | About 25 at 0.63 µm (Tomasko et al. 1980; 25–40 across probes); Rayleigh near 15 at 550 nm but about 41 in the blue channel                                                     | R08 Design note 4; its research            |     |
| Atmosphere; open question 3 | Where Hillaire's term drifts, "that table is replaced"                       | The replacement must depend on view direction, J_ms(h, μ₀, μ_v, m); Hillaire's isotropic term is inadequate at Venus and Titan depths                                           | R08 Design note 9                          |
| Atmosphere                  | The scale height "follows from temperature, mean molecular mass and gravity" | Only with a vertical structure: an isothermal column is 3 times wrong at Venus's cloud tops; R08 asks plan 14 for (T_s, p_s, β, T_skin)                                         | R08 Design note 3                          |
| Atmosphere                  | "Baked offline … cached per world"                                           | Generated worlds cannot be baked at development time: the bakes run at arrival, in workers                                                                                      | R08 Design note 1                          |
| Atmosphere                  | Mie theory for every aerosol                                                 | Mars dust and ice are not spherical; sphere Mie puts a spurious rainbow and glory in their skies; literature phase functions for non-spheres, mean-field aggregates for haze    | R08 Design note 6                          |
| Atmosphere                  | Methane haze, ozone only with O₂; one sun                                    | Plan 14 produces only H₂O, CO₂, N₂ and Ar (its Titan has no methane, no world O₂); several suns, a gas envelope's visible atmosphere, refraction and scintillation are unowned  | R08 Design notes 13, 16                    |

### Terrain, the surface and Knowledge

| Section                                             | The brainstorm says                                                                        | Correct                                                                                                                                                                                                                    | Source                     |
| --------------------------------------------------- | ------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------- |
| The line between truth and decoration; The geometry | 0.5 m spacing keeps a 2 m wavelength within about a third                                  | True in one dimension only; on triangles the largest spacing must be at most 0.375 m: Earth's finest level is 19 (17.7 m patches, not 32 m), the demand cap about 89 m, not 160 m; "a quarter of the band limit" is a mean | R05 Design note 3          |
| Performance budget (patch demand)                   | (200 · v + 290 · \|ḣ\|) ÷ h; the cap as 1/τ                                                | 200 re-derives as 8k² at k = 5 (300 without cached parents); 290 as about 340 (3πk² ÷ ln 2); the cap moves as 1 ÷ (τ θ_px)                                                                                                 | R05 Design note 19         |
| Performance budget; Two styles of one renderer      | The low setting's demand "about a ninth"; the wireframe "about a sixteenth" of the patches | The quadtree's floor of about 36 patches a ring makes the low setting's counts about a quarter, the wireframe's about a fifth; the ninth holds for demand only while k exceeds about 3                                     | R10 Design note 15; R05    |
| Performance budget (shadows)                        | A horizon map "needs rebaking only as the sun moves, 15° an hour"                          | A horizon map is sun-independent (Max 1988) and bakes once with its patch; at half-float it is about 68 kB a patch, which the height-texture row does not count                                                            | R10 Design note 10         |
| The line between truth and decoration; memory table | Normals always at twice the mesh's resolution                                              | The memory table gives the low setting mesh resolution; on the target, twice quadruples the gradients the 40 ms, 10 µs-a-point bake budget counts                                                                          | R05 Design note 25; R10    |
| Two deployments, one scene                          | The station wireframe's 4–9 ms                                                             | Holds only with analytic line antialiasing; 4× MSAA takes it to 10–15 ms                                                                                                                                                   | R10 (T11)                  |
| The coarse global pass; Decisions                   | D_b "about 100 km on an Earth at level 8"                                                  | 84.9 km (S2's `kMaxEdge` 1.7049); Mars 90.3, the Moon 92.6, Ceres 50.0 km                                                                                                                                                  | R09 Design note 4          |
| The coarse global pass                              | The margin is "about D_b or two coarse cells"                                              | Five cells, set by the interpolant at the corners; the crater term alone needs four by the smallest edge                                                                                                                   | R09 Design note 15         |
| The coarse global pass (step 2)                     | Relief 20 km × (g⊕ ÷ g) × a lithosphere factor                                             | A 1/g envelope (Johnson and McGetchin 1973), not a prediction; σ_h² = σ_struct² + σ_crat² + σ_volc², with no g in the structural share (open question 20's ruling, for plan 14)                                            | R09 Design note 3          |
| The coarse global pass (craters)                    | Screening "about 5 m on Earth, 0.5 km on Venus and 8 cm on Mars"                           | Those are projectile sizes; the crater cutoffs are about 20 times larger (about 100 m, 10 km, 1.6–2 m)                                                                                                                     | R09 Design note 10         |
| The coarse global pass (craters)                    | Neukum's coefficients and transition diameters "not extracted"                             | Extracted (Craterstats; Pike 1980 Table 3); the published a0 of −3.0876 is a misprint of −3.0768                                                                                                                           | R09 Design note 10         |
| The coarse global pass; open question 5             | Hack's law with C = 1.5, h = 0.6                                                           | Hack's 1.4 is in miles; in SI C = 0.320 m^−0.2, and 1.5 makes streams 4.7 times too long                                                                                                                                   | R09 Design note 9          |
| The coarse global pass (step 2, step 4)             | Craters of D_b and wider before erosion on a Mars                                          | Craters younger than the wet epoch's end go on after erosion                                                                                                                                                               | R09 Design note 5          |
| The per-query evaluation                            | Local synthesis holds "0.1–0.2%" of the variance, the Moon about 2%                        | Those hold at a 35 km wavelength; at the coarse cell it is 0.6% (Earth, Mars), 4% Mercury, 5% Ceres, 7% the Moon; Earth at 35 km is about 0.27%                                                                            | R09 Design note 7          |
| The per-query evaluation; open question 5           | Dendry's published network has four levels; 1 to 3 µs a point                              | The paper's figures use up to four, its code allows six; 1–3 µs holds for one network, not the stacked fourteen levels (about 4 µs native, 5–6 µs in wasm)                                                                 | R09 Design notes 13, 14    |
| The per-query evaluation; Knowledge                 | A crater function shared "in the sim"; an `ObjectKey` of body, face, level and cell        | The fine pass is in `hyperion-surface`, which cannot depend on the sim, so the function lives there and plan 14 calls it; the body is carried by the detail seed, not the key                                              | R09 Design notes 2, 12     |
| Open question 9                                     | Ramirez 2024's seasonal energy-balance model                                               | Not reimplementable from the paper (radiation from unpublished tables), and its explicit six-hour step fails for slow rotators; an implicit seasonal moist model                                                           | R09 Design note 8          |
| Open question 9                                     | Slow rotators stay temperate at nearly twice the flux; 54° obliquity                       | Right, but the onset is a solar day of 16–48 d rising with flux; 54° is confirmed (53.9°), but Kilic et al.'s ice belt has hysteresis, so obliquity alone cannot decide it                                                 | R09 Design note 3          |
| Open question 5 (Mars check)                        | At least about a metre of rock incised                                                     | Confirmed (1.2 m, Luo et al. 2017), but most of it is below the coarse cell, so a coarse-only check falls short                                                                                                            | R09's research             |
| Open question 18                                    | A crater 1–2 m across is some 0.2–0.4 m deep                                               | The bound for brand-new Martian craters (d/D 0.23); on mature ground 0.05–0.2 m (lunar fresh d/D 0.10, mature ≤ 0.05); proposed status "ruled decoration"                                                                  | R11 Design note 20; R09    |
| The line between truth and decoration               | "Some tens" of rocks 0.2 m tall in a 32 m patch                                            | Some tens at Viking 1's abundance to about 200 at Viking 2 and Pathfinder; heights average 0.29–0.41 of the diameter, 0.5 being the hazard convention, so authority is by D ≥ 0.4 m                                        | R11 Design note 4          |
| Materials, and what a surface looks like            | "The shader blends the class's textures"                                                   | "The class's photometric parameters": any texture beyond the baked data is decoration by the brainstorm's own rule; a class across levels is not addressed (a coarse slope loses every cliff)                              | R10 Design notes 3, 9      |
| Knowledge, and the surface seed                     | `ELEV ~2140 m ±180 m`; "band-limited noise of known amplitude"                             | `ELV` already names a camera's elevation, so `ELEVATION`, written `~2140 m ± 180 m` (NIST SP 811 §7.7); the fine synthesis also holds channels and craters, which need their own closed forms                              | R10.T13; R10 Design note 6 |

### Clouds, rings, stills and the budget

| Section                          | The brainstorm says                                                            | Correct                                                                                                                                                               | Source                 |
| -------------------------------- | ------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------- |
| Rings                            | Close to at 9 km "for plan 14's 5 m top particle size", 18 km for 10 m         | Plan 14's 5 m is a radius (`RING_PARTICLE_RADII`); the top particle is 10 m across and fills a 1080p pixel at about 18 km                                             | R11 Design note 16     |
| Rings; Testing                   | A particle slab at a filling factor near 0.05                                  | Cannot reproduce the B ring's tilt brightening; Salo and French 2010's fields have central filling factors of 0.32–0.38, vertically non-uniform                       | R11 Design note 15     |
| Rings; open question 8           | Hapke's shadow-hiding term on the low setting                                  | It depends on phase only and gives no tilt effect; the low setting can sample the same table, with Hapke a measured fallback                                          | R11 Design note 15     |
| Rings (the profile's gaps)       | Cassini-like gaps at resonances                                                | The resonance is the gap's inner edge, and the Division holds τ ≈ 0.05–0.12 (for plan 14's profile)                                                                   | R11 Design note 13     |
| Clouds; Oceans; Knowledge        | Unsurveyed regions drawn "as what the ship knows of it"                        | Says nothing of clouds and seas: clouds are drawn everywhere from a terrain-free zonal climatology, refined over surveyed ground; no sea over unsurveyed ground       | R11 Design notes 8, 11 |
| What the guide must gain, item 2 | The view states when decoration is on                                          | Whether clouds and Gerstner waves count is unsaid; R11 reads decoration as terrain micro-detail and decorative scatter only (for the owner)                           | R11 Design note 2      |
| Still images                     | Windows' `TdrDelay` and i915's 640 ms                                          | Both stand; Chromium's 15 s GPU watchdog, on the GPU process's main thread, is the realistic hazard for a still (a synchronous pipeline compile) and is not mentioned | R11 Design note 17     |
| Performance budget; Sources      | RTX 4060 "270 GB/s" (text) and "272 GB/s" (Sources)                            | 272 GB/s                                                                                                                                                              | R12 Design note 11     |
| Performance budget               | UHD 620 "about 1.1 GHz", "a few tens" and "20 to 25 GB/s"                      | 1.15 GHz (0.44 TFLOP/s at 24 EU); 37.5 GB/s shared with the CPU                                                                                                       | R12 Design note 11     |
| Performance budget               | A discrete memory ceiling of "2 to 3 GB"; "replace them with measured figures" | Read as a finding above 2 GB and a failure above 3 GB; the brainstorm does not say whether its own tables are edited (R12 keeps a results file and drafts a revision) | R12 Design notes 2, 9  |

## Open across plans

Questions the reconciliation raised that need research, a physics ruling or an owner's decision,
recorded here rather than settled.

- **Gas giants' cloud bands have no owner.** R07, R08 and R11 each exclude them and no plan
  generates a band structure. The research lean (R11's research) is R07: a band texture driven by zonal jets
  over R08's deck photometry, with the structure asked of plan 14.
- **Visible thermal emission of magma oceans** is owned by no plan (R07's Risks; neither R08 nor R10
  takes it).
- **Refraction and scintillation.** R06 leaves scintillation to R08, and R08 draws neither; R10's
  shadow test would read a refracted sun if R08 drew one. Whether the realism ruling wants them is a
  question for the owner and a research agent.
- **Terrain shadows on the high setting** were assigned to no plan by the brief; R10 takes the
  cascades (its Design note 11, R10.T9). Confirmed here as R10's unless the owner moves them.
- **Plan 14's Venus at 58 bar** against the real 92 (R08 Design note 16) moves its Rayleigh depth
  from about 15.5 to 9.8. It is a physics discrepancy in a galaxy plan, for a research agent and
  plan 14's owner.
- **Plan 14's cloud fraction** gives every `Temperate` world 0.67 and `Snowball` 0 (R11's finding in
  `planetary/derive/atmosphere.rs`), and its Bond albedos disagree with the geometric albedos R07
  derives (airless rock 0.11 against the Moon's A_V of about 0.06; Earth's p_V 0.434 giving A_V
  0.57 against 0.294): both need plan 14's owner and a research agent before R07 and R11 depend on
  them.
- **H₂'s Rayleigh cross-section** between Peck and Huang with Hohm's King factor and Ford and Browne
  1973 or an ab initio value (R08's Risks).
- **The skills' task-ID patterns.** `implement-task`'s `plan_task.py` and `validate`'s
  `select_checks.py` match only `P..` IDs. They need the `R` prefix before the first rendering task
  is implemented through them. R04.T7.c owns the change, beside its own edit of `select_checks.py`.
- **The brainstorm's "done now" clippy scope.** The brainstorm names three `clippy.toml` files for
  the ban; R04 bans in five, since base and the surface crate must carry it from their creation.
  Consistent, but the brainstorm's sentence should say five once they exist.
