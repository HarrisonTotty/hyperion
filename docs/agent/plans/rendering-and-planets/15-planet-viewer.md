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
- **`RANDOMIZE`:** from its `DRAW SEED`, a uniform pick of the real systems within 10 ly of a point
  drawn uniformly within 100 ly of the Sun-like point. It keeps a single star with a present planet
  at the epoch (at most 64 tries), takes one of its present planets uniformly, and reads that
  planet's inputs: host initial mass, [Fe/H], age, primordial mass, a, e and formation distance,
  with `DAY`, `TILT` and the volatiles left `AUTO`, and a `SEED` from the draw.

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
