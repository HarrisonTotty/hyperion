# Plan R15: The Planet Viewer

- **Milestone:** none (the owner's request, 2026-11-04): a dev tool beside the consoles.
- **Depends on:** as built, [R07](07-lit-bodies-styles-and-main-screen.md) (`VIEW`'s body
  inspection, its camera and styles), [R08](08-atmospheres.md), [R10](10-terrain-on-generated-worlds.md)
  (the served terrain session, training surveys); galaxy plan 14 (the planetary generator) and
  plan 06 (the stellar stage).
- **What it is:** a `PLANET VIEWER` tab showing one planet, existing or generated from the
  generator's own inputs, from orbit down to the terrain on `VIEW`'s renderer.

## The owner's rulings (2026-11-04)

- Inputs are the generator's own; everything seen is derived as the galaxy derives it, with no
  appearance override. The same inputs give the same planet, and read back as one shareable string.
- Its own tab, in dev builds or behind a flag, held to `docs/frontend/ux-guidelines.md`.
- A camera from orbit down to the surface (pan, orbit, zoom), reusing `VIEW`'s renderer, with the
  sun's direction adjustable. Existing planets by body ID, and `OPEN IN PLANET VIEWER` from
  `SYSTEM`'s and `VIEW`'s body. `RANDOMIZE` draws every input from the generator's own
  distributions, repeatable by a seed of its own.
- No cost to the other displays while the tab is hidden.

## Where generation runs: the server, as additive request kinds

- `VIEW`'s served path (records, atmospheres, coarse fields, training surveys, host discs) asks
  the server by universe and body ID, through its caches. A generated planet the server holds is
  drawn by that path unchanged; the sim in a client worker would need the atmosphere and surface
  services moved into the client (the coarse pass peaks at 408 MB at level 8), and the client has
  no `hyperion-sim` WebAssembly build.
- Three request kinds, additive under plan 04's convention (`PROTOCOL_VERSION` stays 3):
  `viewer_planet {universe, inputs}` answers the system's and body's IDs and the inputs with every
  drawn value stated; `viewer_randomize {universe, draw_seed}` answers drawn inputs and the real
  planet they were read from; `viewer_inputs {universe, body}` reads an existing planet's inputs back.
- The server keeps the last 16 generations per galaxy key (`ViewerSystems`, by count). `bodies_of`,
  `system_bodies`' hosts and the sky's host discs read it first for a viewer ID, so every later
  request (`system_bodies`, `body_detail`, `surface_field`, `survey_pass` in training scope, `sky`)
  works unchanged. An evicted ID is refused `unknown_system`, and the client generates it again.
- **The ID** is a `PinnedId` whose number has bit 57 set (the viewer's half of the pinned space,
  lean) and a 57-bit hash of the inputs' canonical bits. Every stream is keyed on it, so every
  input changes every draw that isn't itself an input, as each galaxy system has its own draws.
- **The seed** is the input `SEED`: the stellar draws (`StarDraws::for_star`) and the planetary
  stage (`generate`) take it, not the universe's seed, so a string gives one planet in any universe
  of a generator version. The universe gives only the galaxy's rules and the sky.
- **The place** is the Sun-like point, 26,000 ly along +y (`SAMPLE_CENTRE_LY`), at the epoch; its
  sky is the galaxy's from there, the host excluded.

## The inputs (sim `planetary::viewer::PlanetInputs`)

| Panel name                    | Unit       | Range                            | What the generator consumes                   | `AUTO`                 |
| ----------------------------- | ---------- | -------------------------------- | --------------------------------------------- | ---------------------- |
| `SEED`                        | 16 hex     | any `u64`                        | `Seed` of every stream                        | no                     |
| `STAR MASS`                   | M☉ initial | 0.08–150                         | the single star's `StarModel` (ForcedSingle)  | no                     |
| `[FE/H]`                      | dex        | −3.0 to +1.0                     | `Composition::from_fe_h`, the disc, the class | no                     |
| `AGE`                         | Gyr        | 0.001–13.8 (`UNIVERSE_AGE`)      | the system's age at the epoch                 | no                     |
| `MASS`                        | M⊕         | 10⁻³ to 4,000 (13 MJ is 4,131)   | `PlacedPlanet`'s primordial mass              | no                     |
| `A`                           | au         | 0.005 to the strip radius        | the primordial orbit                          | no                     |
| `ECC`                         | none       | 0–0.9, periapsis past the star   | the primordial orbit                          | no                     |
| `FORMED AT`                   | au         | 0.005 to the strip radius        | formation distance, snow-line side            | a; giants as placement |
| `DAY`                         | h          | 0.5–100,000, floored at break-up | the primordial period (`SpinDraws`)           | drawn                  |
| `TILT`                        | degrees    | 0–180                            | the primordial obliquity (`SpinDraws`)        | drawn                  |
| `WATER`, `CARBON`, `NITROGEN` | rank       | 0.001–0.999                      | `VolatileDraws` words 0–2                     | drawn                  |

- Always drawn, never inputs: the star's own draws, the disc, the class (drawn, but only the given
  planet is placed), the planes and orbit angles, the radius rank, the formation time, moons, rings,
  belts, the halo, the pole's azimuth and phase, sulphur, chlorine, redox, C/O and Mg/Si, the
  surface and detail seeds. Locking, despinning and every later state are derived.
- **The string:** `PV1;` then `key=value` pairs in the table's order, `;`-separated, an `AUTO`
  input left out; numbers in shortest round-trip form, so a string parses to the same bits.
- **`RANDOMIZE`:** from its `DRAW SEED`, a point drawn from the galaxy's own number density of
  systems over the whole galaxy (a grid of 73,440 bins, constant within each), then the system
  nearest it. It keeps a single star with a present planet at the epoch (at most 64 tries), takes
  one of its present planets uniformly, and reads that planet's inputs: host initial mass, [Fe/H],
  age, primordial mass, a, e and formation distance, with `DAY`, `TILT` and the volatiles left
  `AUTO`, and a `SEED` from the draw.

## The tab

- `PLANET VIEWER`, `F5`, after `VIEW`. In the list only when the client isn't packaged, or with
  `--planet-viewer` on a packaged build (preload `planetViewer`).
- Left column, an `INPUTS` panel: `SOURCE` (`GENERATED`, or `BODY` with an ID field and `OPEN`);
  groups `STAR`, `ORBIT`, `BODY` and `COMPOSITION`; `SEED`; each `AUTO`-able field with its `AUTO`
  toggle showing the drawn value; `GENERATE`; `RANDOMIZE` with its `DRAW SEED`; `SHARE` (the
  string, `COPY`, and a paste field with `LOAD`); one status line for refusals.
- Right: `VIEW`'s display itself, under a nested `ViewSceneProvider` holding the viewer's scene as
  its only scene (no `SCENE` bar), opening on the `ORBIT` camera.
- Hidden, the tab costs nothing: `Activity` tears down `VIEW`'s engine, loop, terrain session and
  atmosphere cache, and the panel asks nothing.

## Reused from `VIEW`

- `inspectionScene` (a `BodyInspection` becomes a kept scene of served bodies), `ViewSceneProvider`,
  `ViewDisplay` (engine, terrain session, atmosphere cache, exposure, styles, instruments, plates),
  `surveyInspected` (the whole body, training scope), and the `system_bodies` adapters.
- New in `VIEW`: an `ORBIT` camera preset about the inspected body. Drag orbits, moving the
  sub-camera point by what the pointer moves on the ground. Shift-drag turns the heading and tilt
  toward the horizon. The wheel steps the altitude by a factor of 1.25, from 50 body radii down to
  10 m above the ground under the camera. Free and the other presets stay.
- New for the viewer: `SUN`, the subsolar point's `LAT` and `LON` on the body, made by turning the
  host about the body in the kept scene at its distance; `TIME` stays the inspection's.

## Tasks

### R15.T1 The planet from inputs, in the sim

Size M (sim only). `planetary::viewer`: `PlanetInputs` with its refusals by field; `viewer_id`;
`generate_viewer` (a synthetic record and single star, `generate` with one placed planet, the spin
and volatile overrides); `inputs_of` (a generated planet's inputs read back); `draw_inputs`
(`RANDOMIZE`). Accept: tests by name: two runs bit-identical; `inputs_of` of a real planet
regenerates its mass, orbit and host; a draw is repeatable by its seed and reads a real planet; each
refusal names its field; `golden_diff.py` reports no golden changed.

### R15.T2 The request kinds and the server's store

Size M. The protocol DTOs and bindings; `ViewerSystems`; the routing in `bodies_of`, `hosts_of` and
the sky's discs. Accept: a server test generates a planet, then answers `system_bodies`,
`body_detail`, `surface_field` and `sky`'s host discs for it; eviction is refused `unknown_system`;
`viewer_randomize` is repeatable.

### R15.T3 The tab and the inputs panel

Size L (client). The display, the panel with `RANDOMIZE` and the string, `VIEW` nested on the
viewer's scene. Accept: Vitest by name (the string round trip, the panel's requests, the hidden tab
asking nothing).

### R15.T4 The orbit camera and the sun

Size M (client). `ORBIT` in `VIEW`'s camera, and `SUN`. Accept: Vitest of the camera's pose, its
floor and its zoom steps.

### R15.T5 Existing planets

Size S (client). `BODY` with an ID; `OPEN IN PLANET VIEWER` on `SYSTEM`'s orbit map and on `VIEW`'s
body inspection. Accept: Vitest by name.

### R15.T6 The captures

Size S. A `--views-check --planet-viewer` run captures the tab at orbit, low orbit and the surface.
Accept: test-render, both variants, with no other display's capture changed; the three captures in
the lane's outbox.

## Generator version

No generated output moves: the viewer's path adds functions and pass-through overrides, and
every galaxy system takes the code it took before. No `GENERATOR_VERSION` change and no hold.

## Risks and open points

- R15.T1 (built): `planetary::viewer` holds `RawPlanetInputs` and `PlanetInputs` (`TryFrom`,
  `InputField::range`), `canonical()` and `system_id()`, `ViewerSystem` (`generate`, `summary_at`,
  `host_discs_at`, `drawn`, `into_parts`), `summary_of`, `host_discs_of_context`,
  `is_viewer_system`, `inputs_of`, `DensityGrid` and `draw_inputs(galaxy, grid, draw_seed)`.
- R15.T1: the ID hashes the inputs' canonical text (`PV1;name=value;…`, Rust's shortest decimal),
  not their bits, since the sim bans hashing float bits. The client's string should be that text.
- R15.T1: `generate`'s steps 6–10 are now `system::complete`. Also added: `system::generate_given`,
  `placement::place_given` with `GivenOrbit`, `SpinDraws::with_primordial_period` and
  `with_obliquity`, `sky::disc::host_discs_of`, `stellar::system::synthetic_summary_at`,
  `PlanetarySystem::seed()`, and the tag `viewer.draw` (`tags.golden` extended only).
- R15.T1 (lean, 2026-11-05, main's suggestion): `RANDOMIZE` draws its point by the whole galaxy's
  density, then takes the nearest system. A nearest pick keeps a sparse region from being
  under-drawn, as a fixed sphere would. Building the grid costs about 1 s (pod), so the server
  keeps one per galaxy.
- R15.T1 (lean, 2026-11-05): a lone given planet lies in its host's plane, with no mutual
  inclination; its host's class is kept as drawn.
- R15.T2 (built): the protocol's `PlanetInputsDto`, `ViewerPlanetRequest` and `ViewerPlanetDto`,
  `ViewerRandomizeRequest` and `ViewerDrawDto`, `ViewerInputsRequest` and `ViewerReadDto`, with
  `ViewerDrawnDto`. The server's `compute::ViewerSystems` keeps 16 systems a galaxy key and 4
  density grids. `bodies_of`, `hosts_of` (`convert::viewer_summary`), the sky's `check_exclude` and
  `discs`, and the surface service's seed check read a viewer ID from it.
- R15.T2: `system_bodies` takes `planets.seed()` in place of the universe's seed, which is the same
  for every galaxy system. A refused input names `inputs.<field>`.
- R15.T3 (built): `displays/planet/` holds `PlanetViewerDisplay` (F5, `lib/displays.ts`'s
  `PLANET_VIEWER_DISPLAY` and `stationDisplays`), `PlanetInputsPanel`, `PlanetViewerBar`,
  `SeedField`, `usePlanetViewer` and `planetInputs.ts`. `ViewDisplay` takes `sceneBar={false}`.
  The preload's `HyperionApi.planetViewer` is set by the `--hyperion-planet-viewer=` switch, which
  `main` turns on for an unpackaged client or `--planet-viewer`.
- R15.T3 (lean, 2026-11-05): `RANDOMIZE` and `SHARE` sit in a bar above the view, in the `SCENE`
  selector's place. `SEED` shares `GENERATE`'s row, and `VOLATILES · RANK` folds at mount, so the
  panel fits 1280×720. The keys are `G` and `N`, outside text fields and canvases. A viewer system's
  designation is `VIEWER` and its ID's last six digits. The tab always shows `TRAINING`.
- R15.T3 (lean, 2026-11-05): the field units hold: `SMA` and `FORMED AT` are always in AU, and
  `ROTATION` is in d, capped at 1,000 d for entry. The slot for T4's `SUN` is
  `PlanetInputsPanel`'s `sun` prop. The new names are in the guide's nomenclature list.
- R15.T4 (built): `view/camera/orbit.ts` holds the `ORBIT` camera's pose (`orbitStart`,
  `orbitDragged`, `orbitZoomed`, `orbitLooked`, `coRotated`). `CameraPreset` gains `orbit`, with
  `CameraScene.orbitBodies` and `orbitHome`, `ViewScene.orbitHome`, `KeptScene.startPreset`, the
  `zoom` command (`ViewCanvas`'s wheel) and `KeyLegend`'s `ORBIT` line. `displays/planet/viewerScene.ts`
  (`SunSetting`, `sunnedScene`, `viewerScene`) and `SunControl` fill the panel's `sun` slot.
- R15.T4 (lean, 2026-11-05): `ORBIT` is offered only by a scene that names its body
  (`ViewScene.orbitHome`, which only the viewer sets), so `VIEW`'s other scenes and their captures
  are unchanged. Its key is `0`.
- R15.T4 (lean, 2026-11-05): the floor is 10 m above the datum, the reference spheroid along the
  camera's line from the centre (`orbitRadiusM`), not above the ground. The camera reads no terrain
  height, so above the datum it can stand inside high ground. T6's first captures found that the
  equatorial radius held the camera kilometres off an oblate planet's ground away from its equator.
- R15.T4 (lean, 2026-11-05): tilt and heading are on `R`/`F` and `Q`/`E`, the free camera's keys,
  with no Shift-drag. The wheel and `W`/`S` zoom, and the arrows orbit at 0.5 rad/s, scaled as a
  drag is by altitude over distance.
- R15.T4 (lean, 2026-11-05): `SUN` is `ORBITAL` (where the records put it, as it opens) or `HELD`
  at a body-fixed `LATITUDE` and `LONGITUDE`, which turns every star about the planet at its own
  distance. In `ORBIT` the camera turns with the ground, so it hovers over one place.
- R15.T4 (lean, 2026-11-05): while `HELD`, the view's label block states `SUN HELD: LATITUDE … ·
LONGITUDE …`, since the light is not where the records put it. `MAXIMIZE VIEW`'s hidden keys
  name `0 ORBIT` where it is offered, and the key legend follows the `CONTROLS` view's preset.
- R15.T4 (lean, 2026-11-05): `ORBIT` also names a preset now, beside the `SYSTEM` panel and the
  legend entry of that name. The owner may want another name for the preset.
- R15.T5 (built): the panel's `BODY` and `OPEN BODY` (`BodyField`) ask `viewer_inputs`, then
  `resolve_system` and `system_bodies` at the epoch. `SYSTEM`'s orbit map has
  `P OPEN IN PLANET VIEWER` and `VIEW`'s `SCENE` bar has `OPEN IN PLANET VIEWER`, each handing `App`
  an inspection (`PlanetViewerOpening`) that the viewer opens with `viewer_inputs` alone, at that
  display's time. `ShownPlanet.source` tells a generated planet from the galaxy's.
- R15.T5 (lean, 2026-11-05): `OPEN BODY`, not the plan's `OPEN`, which already names an
  instrument view's command. It shows the galaxy's own planet, and its inputs, read back, go on the
  panel. `GENERATE` then makes a viewer planet of them, whose other draws differ. A system
  of more stars than one is noted `MULTIPLE STARS`, since the inputs hold one host.
- R15.T5 (lean, 2026-11-05): the viewer opens planets and dwarf planets. A moon's
  `OPEN IN PLANET VIEWER` is held back on `SYSTEM` (`NOT A PLANET`) and not offered on `VIEW`'s bar,
  which has no room for a reason. A belt member the server refuses shows its refusal.
- R15.T5 (lean, 2026-11-05): a hand-over counts as handled once its outcome is reported or a newer
  command cuts it short. One cut short by the display hidden is opened again when it is next shown.
- R15.T6 (built): `--views-check --planet-viewer` runs `planetViewerCheckRun.ts` (the CLI, the
  preload's launch, main's session with its captures under `target/views-check/planet-viewer/` and
  the inspection run's watchdog, `viewsCheck.sh --planet-viewer`). Such a launch opens on `GALAXY`
  and offers the viewer, and the viewer's `VIEW` draws through the views probe.
- R15.T6 (lean, 2026-11-05): the run opens the first universe the server lists and generates the
  panel's inputs at mount. It zooms by the wheel to 400 km and then 300 m above the ground at the
  line of sight, or above the datum where the ground is below it, and tilts to the horizon under
  `R`. Each capture is taken of the whole tab and again under `MAXIMIZE VIEW`.
- R15.T6 (lean, 2026-11-05): the first captures showed the tab overflowing 1280×720, so `SHOWN`
  (the `BODY` row) stands beside the title, folded at mount, `GENERATE`'s label keeps to one line,
  and the held-back reason stands in the bar. `SHOWN`, not `SOURCE`, which names the exposure
  meter's source on the same display.
- R15.T6 (lean, 2026-11-05): the run fails where the tab overflows the window (the console's frame
  taller than it), and it captures before it judges, so that a failed run keeps its picture.
- R15.T6 (lean, 2026-11-05): the inspection run's lit-body guard now counts the bodies pass's late
  raster (R14.T3.d). At the surface the guard is logged with what was drawn rather than failing the
  run, since close over a sea its materials may not be drawn; faults and lighting still fail it.
- R15.T6 (built, the fold): `inputsLayout.ts` (`inputsLayout`, `FULL_MIN_HEIGHT_REM` 59,
  `toggledInputsFold`), measured on `.planet-viewer__inputs`; below 59 rem one of `SHOWN`, `STAR`,
  `ORBIT`, `PLANET`, `VOLATILES · RANK` is open, `STAR` at mount; folded groups summarise rows.
- R15.T6 (built): `useEntryDraft`/`EntryRefusalContext` keep one entry refusal on the panel, and
  `BodyField` takes `folded`. A server refusal reads `SMA REJECTED: <reason>` under its field, and
  `GENERATE`'s outcome `see SMA` while that field stands open.
- R15.T6 (built): the run captures `planet-viewer-inputs-*` (`PLANET_VIEWER_INPUTS_CAPTURES`) and
  fails on `inputsCutPx`; the work area and `.planet-viewer` rows are held to the window.
- R15.T6 (lean, 2026-11-05): the folded summaries are running text, extending the guide's
  fixed-width exception; the full layout takes the summaries, the refusal wording and the one-refusal
  rule too.
- R15.T6 (built, infra): the run waits, within the draw's wait, for its sky's first reply
  (`skyAnswered`) before the ground's, and goes on past it; no limit changed. `--sky-qos background`
  would not help a pod (Linux's twins ignore it); the failure did not reproduce here (outbox).
