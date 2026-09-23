# Rendering and Planets

Brainstorm for the rendering engine and for what it draws: the view out of the window at real
astronomical scale, and the planets the ship will one day descend to. This is a design exploration,
not a plan. Decisions are marked **Lean** where there is a recommendation, the open ones are
collected under [Open questions](#open-questions), and what has been settled is under
[Decisions](#decisions).

It carries forward two threads already begun. The single-player brainstorm chose the engine's
requirements and leaned towards Babylon.js under
[The rendering engine](single-player-experience.md#the-rendering-engine); that lean is re-examined
here against what the engines actually shipped. Its open question 7 asked when the planets brainstorm
would be written, and this document answers the rendering half of it: planet geometry, surfaces and
atmospheres as the renderer needs them, including the surface generator's architecture, because
nothing about planet rendering can be settled without it. Planet _content_ — biomes as places, life,
resources, what a landing party finds — is still owed a document of its own.

## Goal and scope

Given the galaxy the [galaxy brainstorm](galaxy-generation.md) generates, draw it from the pilot's
seat: a perspective view that is true to scale from a hull plate a metre away to a star system tens
of astronomical units across, and eventually a planet flown from orbit to a landing without a seam
or a loading screen. The rendering must be defensible in the same way the simulation is: what is drawn is
what the numbers say, and where the picture is an approximation, the display says so.

In scope:

- The rendering technology: what runs inside the Electron client, and what would replace it.
- The real-scale foundations: floating origin from the galaxy's frames, the depth buffer, and
  luminance in physical units.
- `VIEW` as a wireframe first, then as a lit, textured scene.
- Planet rendering: geometry and level of detail, the surface height function and how client and
  server agree on it, materials and scatter, atmospheres, clouds, oceans and rings.
- The sky: stars at their true magnitudes, the local star as a disc, the galaxy from inside it.
- Exposure and tone mapping, which is what makes a physically-lit space scene readable.
- The performance budget, on the hardware that exists and the hardware that is assumed.
- How all of this fits the consoles: the UX guide's edits, accessibility, and testing.

Out of scope, each owed its own document or already settled elsewhere:

- Flight dynamics, controls, sound, and the rest of the cockpit: the
  [single-player brainstorm](single-player-experience.md) has them.
- Planet content: biomes as habitats, life, resources, ruins, and anything a landing party would
  interact with. This document generates a _surface_; it does not populate it.
- Ship and station models as art: the hull definitions are data the flight model already needs, and
  what they look like is a later question. Wireframes come from the same definitions.
- Interiors. The view is from a seat, through a window.
- LLM-generated description text, which never reaches the renderer.

## The scales the view spans

The reason this is hard is in one table. Each row is a real situation the player will be in, and the
renderer has to handle all of them without changing its mind about what a metre is.

| Situation                  | Distance to what matters | What dominates the picture                                 | What dominates the cost           |
| -------------------------- | ------------------------ | ---------------------------------------------------------- | --------------------------------- |
| Docked, hull a metre away  | 1 m – 100 m              | Hull plates, the base, the docking port                    | Nothing; it is one model          |
| Low orbit, 400 km up       | 10⁵ – 10⁶ m              | The planet filling the window, terrain detail, the horizon | Terrain level of detail, clouds   |
| Orbit, a few radii out     | 10⁷ – 10⁸ m              | The whole planet as a disc, its atmosphere's limb          | Atmosphere, cloud layer, the ring |
| Moving between planets     | 10⁹ – 10¹² m             | Bodies as discs and points, the star, the star field       | Almost nothing                    |
| Across the system          | 10¹¹ – 10¹³ m            | The star, a few discs, the star field                      | Almost nothing                    |
| Interstellar, after a jump | 10¹⁶ m and out           | Stars as points, the galaxy as a band                      | Almost nothing                    |

Two things follow. First, the expensive cases are the near ones, and they are the ones with a planet
in them; everything beyond a million kilometres or so is cheap because it is points and small discs
— though a gas giant, at 7 × 10⁷ m in radius, still fills the window from 10⁸ m. Second, the dynamic
range in _position_ is about 10¹⁵ between a centimetre of hull and an outer planet, and in
_luminance_ about 10¹² between a dim star and the disc of the star itself, with a sunlit surface, at
about 10⁴ cd/m², in between. Neither fits in a 32-bit float, and both have standard answers. They
are in [Real-scale foundations](#real-scale-foundations).

## Constraints that decide the design

The engine choice is usually argued on features. Here it is decided by five constraints that come
from outside rendering, and they narrow the field before any feature list is opened.

1. **Real scale, to the centimetre.** A GPU works in 32-bit floats. At Earth's surface radius,
   6,371 km, consecutive `f32` values are **0.5 m** apart, and at 1 au they are **16 km** apart. A
   planet's surface simply cannot be expressed in world coordinates on the GPU. The renderer must
   therefore be camera-relative in a specific, checkable way, and the engine must not fight it.
2. **The same terrain on both sides.** Anything the ship can collide with must be identical in the
   client and the server, bit for bit. `hyperion-sim` already compiles to WebAssembly, so the terrain
   generator can be sim code that the server runs natively and the client runs in workers — but only
   once the browser's WebAssembly target is checked for determinism, which today it is not (see
   [Runtime and code shape](#runtime-and-code-shape)). This is the single strongest architectural
   constraint in the document, and it reaches into the choice of noise functions and even of SIMD
   instructions.
3. **The consoles come first.** HYPERION is a bridge simulator whose displays are flight hardware.
   `docs/frontend/ux-guidelines.md` requires that text which must be read stays in the DOM, set in
   B612 and B612 Mono, contrast-checked by the project's own tooling, with every canvas paired to a
   DOM list that the keyboard can reach. A renderer that wants to own the whole window, draw its own
   text and swallow input is a worse fit than a less capable one that sits inside a React display.
4. **The hardware that exists.** The development machine has no discrete GPU: an Intel UHD Graphics
   620 (Whiskey Lake-U GT2, Gen9.5), on Mesa 26.2.3 with the ANV Vulkan driver. The owner has ruled
   that a modern discrete GPU is the design target at 1080p60, but that **every feature needs a
   documented low setting that stays playable on the UHD 620** so that the real renderer can always
   be developed and tested locally. Gen9.5 also sits below the Intel generation at which Chromium
   enables WebGPU by default on Linux, which is why the API floor needed a ruling of its own; see
   [Decisions](#decisions).
5. **Longevity.** The project is long-lived and the simulation beneath it is meant to outlast
   several rendering fashions. An engine that breaks its API every release is a standing tax, and one
   that is abandoned is a rewrite. This constraint is what the engine comparison below actually turns
   on, more than any feature.

## What to take from the references

Every game listed here has solved some part of this problem, and the differences between them are
instructive because they are forced by scale.

| Reference                 | Take                                                                                                                                                                                                          | Leave                                                                                                                                                                                                                                                                                                                                           |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| No Man's Sky              | The feel of the descent, and the proof that one continuous motion from space to the ground is the thing worth building. Fine detail synthesised from a coarse authoritative field.                            | Its planets are small — on the order of a kilometre in radius, not thousands — which is exactly why it has no precision problem. HYPERION cannot take that shortcut.                                                                                                                                                                            |
| Elite Dangerous           | Real-scale bodies flown to the surface, with terrain generated from seeds rather than stored. Proof that the realism ruling is technically feasible.                                                          | Its planets are airless or thin-atmosphere by design, which sidesteps the hardest rendering problem. HYPERION's planetary stage will produce thick atmospheres.                                                                                                                                                                                 |
| Outerra                   | The depth-buffer work: the clearest published account of what precision each scheme actually gives, with measured costs. Camera-relative rendering.                                                           | Its fragment-written logarithmic depth, whose early-Z cost its own measurements show; reversed-Z makes it unnecessary. And its terrain is Earth's, from elevation datasets, where ours must come from the seed.                                                                                                                                 |
| SpaceEngine               | One hierarchy from galaxy to moon with seamless scale changes, and procedural height cached into per-patch GPU textures rather than re-evaluated every frame.                                                 | A free camera that goes anywhere. HYPERION looks through a window from a seat.                                                                                                                                                                                                                                                                  |
| Cesium                    | The best public write-up of planetary precision: per-patch `f64` origins, and the honest finding that one logarithmic frustum still fights at extreme range. A globe renderer held to surveying standards.    | Its problem is one known Earth with measured tiles streamed from a server. Ours is 10¹¹ unvisited worlds computed on arrival. And its two-float encoding, which emulates the `f64` difference on the GPU: per-patch origins differenced on the CPU make it unnecessary, and one rule for where differencing happens is easier to test than two. |
| Star Citizen              | That the industry answer to jitter at kilometre scale was to widen world coordinates to 64 bits, which corroborates that 32-bit world space is not merely inconvenient but unusable.                          | Widening everything is not available to a web renderer: a GPU vertex buffer has no `f64`. We narrow late instead.                                                                                                                                                                                                                               |
| Kerbal Space Program      | A quadtree on a cube sphere with a stack of height modifiers, and a floating origin that rebases discretely rather than every frame.                                                                          | Scaled-down planets and a rescaled solar system.                                                                                                                                                                                                                                                                                                |
| Horizon Zero Dawn (Nubis) | Volumetric clouds that read correctly from below and above, at a measured cost — under 2 ms on a PlayStation 4 for the 2015 prototype — which is the number to beat and the reason clouds need a low setting. | A single authored sky for one Earth-like world. Ours are parameterised by composition and pressure.                                                                                                                                                                                                                                             |

The common lesson is the mirror of the galaxy brainstorm's. There, the galaxy is a function rather
than a database. Here, **the picture is a measurement rather than an illustration**: every quantity
that reaches a pixel should be traceable to something the simulation computed, and the few places
where that is not true should be labelled on the display.

## The engine

### Checking the prior lean's reasoning

The single-player brainstorm leaned Babylon.js on two grounds: that it shipped large-world rendering
with a floating origin in 9.0, and that it commits to backward compatibility. Both were checked,
first through Babylon's documentation site, which automated fetching cannot read, and then through
the documentation's markdown source on GitHub and the engine's own code. Both hold, with
qualifications:

- **Reversed-Z is not a differentiator.** Both engines have it. Babylon.js exposes
  `useReverseDepthBuffer`, which sets the depth function to `GEQUAL` and clears depth to 0 (verified
  in `Engines/thinEngine.pure.ts`). three.js exposes `reversedDepthBuffer` and
  `logarithmicDepthBuffer` as independent options on its WebGPU backend (verified in
  `WebGPUBackend.js`). The prior document's implicit contrast here does not exist.
- **"Large World Rendering" is camera-relative rendering, and experimental.** It is listed in the
  Babylon.js 9.0.0 release notes of 26 March 2026, beside a geospatial camera and 3D Tiles support,
  and its page (<https://aka.ms/babylon9LWDoc>) says what it does. `useLargeWorldRendering` turns on
  64-bit matrices on the CPU and a floating-origin mode that overrides `Effect.setMatrix` and
  `UniformBuffer._updateMatrixForUniform` globally, rewriting uniforms by name: the eye position is
  subtracted from `world`, the translation is zeroed in `view`, and the combined matrices are
  decomposed, offset and recomposed. It handles WebGPU's uniform buffers and never touches the
  projection, so it should coexist with reversed-Z. It began as an experiment in 8.28.3, and the
  maintainer's announcement still calls it experimental, with limits on shadows and billboards.
- **The backward-compatibility commitment is written down.** The first golden rule of Babylon.js's
  contributing guide is "You cannot add code that will break backward compatibility", with
  exceptions for performance and bugs, and a breaking-changes log records what does change,
  including changes to how things look, each with a flag that restores the old look. Meanwhile
  three.js's breaking changes are documented and routine: its migration guide records changes in
  essentially every release, the official advice is to upgrade in increments of ten because
  deprecations last that long, and some changes are _silent visual_ ones — physically-based
  brightness shifted in r181, an ambient occlusion effect darkened in r185.

That last point deserves weight beyond its size, because of what this renderer is for. A display
calibrated in absolute photometric units, whose numbers a console will state, cannot afford a
dependency that changes what a given radiance looks like between releases without saying so. In a
game that would be a nuisance. Here it silently falsifies an instrument. Babylon.js's visual changes
are the contrast that matters: logged, and each with a way back.

### The decision does not rest on the engine's large-world feature

The more important realisation is that **the floating origin is ours whatever we choose.** It has to
be: it must agree with the frames `hyperion_sim::coords` already defines, change when the simulation
changes frame, and be unit-tested against the same `f64` arithmetic the server uses. That is a few
hundred lines of our own code — a differencing step, a per-patch origin, a rotation-only view matrix
— and an engine's opaque large-world system is as likely to fight it as to help. The same is true of
the depth policy and the exposure model.

Babylon.js's own feature shows why. It works in one 64-bit world frame, whose spacing at galactic
distances is about 130 km, with no hierarchy of frames and no rebasing, so it cannot replace the
simulation's frames. With the camera already at the origin it would do nothing useful, and because it
rewrites uniforms by name across every shader, it could silently rewrite our own. **Lean:** it stays
off, and the adapter supplies camera-relative transforms itself.

So the engine is being hired for the ordinary parts: resource and state management, a render graph, a
shader pipeline, a material system, culling, and the tedious correctness of a WebGPU backend across
drivers. It is explicitly _not_ being hired to solve real scale.

| Option                         | For                                                                                                                                                                                                                                                                                              | Against                                                                                                                                                                |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Babylon.js 9.27**            | TypeScript-first, which suits a TS 7 workspace with type-aware lint. A written backward-compatibility rule, and a breaking-changes log whose visual changes each come with a flag to restore the old look. Weekly patch cadence. Reversed-Z present. Active investment in globe-scale rendering. | A quarter of three.js's community. Documentation site opaque to tooling, though its source is on GitHub. Larger package.                                               |
| **three.js r186**              | The largest ecosystem by a factor of four, and the most published procedural-planet work. Reversed-Z and logarithmic depth both exposed on the WebGPU backend. Small package.                                                                                                                    | Breaking changes in essentially every release, including silent visual ones. Upgrades must be taken in small steps forever. `react-three-fiber` pins to a React major. |
| **PlayCanvas 2.22**            | A mature WebGPU implementation with compute shaders, MIT-licensed.                                                                                                                                                                                                                               | Editor-centred workflow that a code-first console app would fight. Smallest community of the three.                                                                    |
| **Raw WebGPU, or wgpu → wasm** | Total control, and no engine to track.                                                                                                                                                                                                                                                           | Everything above becomes ours: culling, materials, resource lifetimes, driver workarounds. Months of work whose output is not gameplay.                                |

**Lean: Babylon.js, confirming the prior document's choice and, once checked, most of its
reasoning.** TypeScript fit is verified. The compatibility commitment is verified: it is a written
rule, and the changes it allows are logged with a way back. The case against three.js is verified
too: its churn is documented and includes silent visual changes. The large-world feature is real but
is not what the lean rests on, for the reasons above — and the understanding below makes the choice
cheap to reverse whatever the engine does next.

### The engine is kept at arm's length

This is the load-bearing decision of the section, and it matters more than which engine wins.

**Lean:** everything that is HYPERION-specific lives behind a small interface that the engine
implements, and no engine type appears outside it. Concretely, the following are ours, engine-agnostic
and unit-tested without a GPU:

- the camera-relative differencing and frame rebasing;
- the projection and depth policy;
- patch selection, level-of-detail choice, culling, and streaming priority;
- the scene description that arrives from the server, and its translation into draw submissions;
- photometry, exposure and the tone-mapping curve's parameters.

What the engine supplies is the submission of buffers and draws, the shader compilation, and the
swap chain. Under that arrangement, switching to another JavaScript engine is a re-implementation of
one adapter rather than of the renderer. It costs perhaps a week more than binding directly to the
engine's scene graph, and it buys the reversibility that even a written compatibility policy does
not give.

It does less for a native renderer than it might seem. A Rust renderer cannot implement a TypeScript
interface, and would have to rewrite everything in the list above. What makes a native renderer
possible is a different property: that the renderer is a [display sink](#runtime-and-code-shape) fed
by the protocol, holding nothing the server has not sent. If the native route ever becomes likely,
the mechanisms above that are pure arithmetic — differencing, patch selection, photometry — are
candidates for Rust compiled to WebAssembly beside the height function, so that both renderers share
them.

### The graphics API, and the Intel problem

The owner has ruled that **WebGPU is required and Electron forces the switches**, rather than
maintaining a WebGL2 fallback. What that means in practice on this machine:

Electron 44 carries Chromium 152. Chromium enables WebGPU by default on Linux only for Intel Gen12
and later with Mesa 22.0 or newer (from Chrome 144) and for NVIDIA with a driver of 535.183.01 or
newer (from Chrome 147). The implementation-status wiki says NVIDIA's enablement is under Wayland,
but Chromium's blocklist entry carries no such condition. Everything else — including AMD, and
including this machine's Gen9.5 UHD 620 — is behind a flag with no announced date. The switch set
reported to work with Mesa's ANV driver is
`--enable-unsafe-webgpu --use-angle=vulkan --enable-features=Vulkan,VulkanFromANGLE,DefaultANGLEVulkan`,
where `DefaultANGLEVulkan` is the one that avoids a hang in swap-chain acquisition on ANV. Electron
sets these from the main process.

Three consequences, which should be written down rather than discovered:

1. **Forcing WebGPU bypasses Chromium's GPU blocklist**, which exists because some driver and
   hardware combinations genuinely break. The entry that gates Linux blocks WebGPU's display path
   through Vulkan and GL interop, so the Gen9.5 risk is in presenting and compositing frames rather
   than in compute. The client should therefore detect an adapter failure and report it as a ship
   system fault in the guide's language, not crash or silently fall back.
2. **The switches are a distribution problem, not just a development one.** Any Linux machine that
   is not Gen12 Intel or NVIDIA with a recent driver gets the forced path, so on Linux the forced
   path is the _normal_ path and must be the one that is tested. Windows and macOS enable WebGPU by
   default and should not be given the Linux switches, which name Linux's Vulkan path.
3. **Compute shaders are therefore available everywhere**, which is what makes the WebGPU-only ruling
   valuable: terrain, cloud and histogram passes can assume compute rather than emulating it in
   fragment shaders. That assumption should be used deliberately, because it is the whole return on
   the ruling. Subgroups are the exception: Dawn refuses them on Gen9 unless a toggle is set, so a
   reduction such as the exposure histogram needs a path that does without them.

WebGL2's absence from the plan costs little that matters: it has no compute shaders, no storage
buffers and no indirect draw, so every compute pass this document leans on would need a second,
fragment-shader implementation, which is precisely the second renderer the ruling declined to
maintain. Reversed-Z is not the obstacle it might seem. WebGL's `[-1, 1]` clip range defeats it by
default, but the `EXT_clip_control` extension, Community Approved by the WebGL working group in
November 2023, switches the range to `[0, 1]`. Whether Chromium 152 exposes it on ANV was not
checked, and under the ruling it does not need to be.

### If the browser cannot carry it

The fallback remains a native renderer in Rust, joining the session as another protocol client, and
the research sharpened what that would cost. Bevy is at 0.19 with breaking changes every five to six
months; its built-in atmosphere implements Hillaire 2020, with transmittance, multiple-scattering,
sky-view and aerial-perspective lookup tables, which is this document's lean too, and its medium is
a list of terms each with its own density and phase function; its transforms are `f32`,
so large-world support means the `big_space` crate, which tracks Bevy a version behind. Godot's
double-precision build converts vectors and physics but leaves shaders single precision, and
requires maintaining custom export templates.

But the cost is not the renderer. **It is the split client**, and on Wayland it is worse than it
looks: there is no cross-process surface embedding, so a native view cannot be placed inside an
Electron window as it could under X11's XEmbed. The options reduce to two top-level windows, a
transparent always-on-top overlay whose layering is compositor-dependent, or streaming frames into
Electron and paying encode-plus-decode latency that a hand on a stick would feel. Add to that a
single input authority for the HOTAS, and two GPU consumers in one process tree.

**Lean:** keep it as a fallback, keep it possible by keeping the renderer a display sink, and do not
pre-build for it. The condition that would trigger it is narrow, and
[open question 2](#open-questions) states it in a form that can be measured: the descent spike fails
on the discrete reference machine for reasons in the browser stack rather than in our own code.

## Real-scale foundations

Three mechanisms have to be right before anything is drawn, because every later decision assumes
them. They are cheap to build and expensive to retrofit, which is why the wireframe `VIEW` is worth
building on the real foundations rather than on a toy camera.

### The floating origin is already in the simulation

The galaxy brainstorm's [Coordinates](galaxy-generation.md#coordinates) gave the simulation three
nested frames, and they are exactly what a large-world renderer needs:

| Frame    | Representation                                      | Resolution                  |
| -------- | --------------------------------------------------- | --------------------------- |
| Galactic | Integer 1 ly cell (`i32` per axis) + `f64` m offset | about 2 m anywhere          |
| System   | `f64` m from the system barycentre                  | about 1 mm at 50 au         |
| Body     | `f64` m from the body's centre                      | sub-micrometre in low orbit |

The renderer adds one rule to them: **nothing reaches the GPU in world coordinates.** Every position
is differenced against the camera's position in `f64`, in whichever frame the camera is in, and only
the difference — which is small, because it is a distance from the eye — is narrowed to `f32` and
uploaded. The camera sits at the origin of the rendering space by construction, so precision is
spent where the eye is, which is where it is needed. At 1 km from the camera an `f32` offset is
precise to 0.06 mm; at 1,000 km, to 6 cm.

This is the standard technique, and its name in the literature is camera-relative rendering or
rendering relative to eye. Two details are where implementations go wrong:

- **The difference must happen in `f64` on the CPU, never on the GPU.** Uploading two `f32` world
  positions and subtracting them in a shader has already lost the precision; the subtraction must
  happen before narrowing. Where a mesh's vertices are themselves at planetary distance from the
  body's centre — every terrain patch is — the patch carries its own `f64` origin, its vertices are
  stored relative to that origin, and the shader is handed the single `f64`-differenced
  patch-origin-minus-camera vector as an `f32`. Vertex coordinates then never exceed the patch's
  own size, which runs from tens of metres at the finest level to thousands of kilometres at a
  root face, where the patch is seen from far enough away that the coarser precision is invisible.
  The precision tracks the level, which is what makes it enough at every scale.
- **The view matrix must not contain the translation.** Rotation-only view matrices, with
  translation folded into the per-object offset, keep the large numbers out of the matrix product
  entirely. Composed with a translation of 10¹¹ m in `f32`, every transformed vertex lands on the
  16 km spacing of an `f32` at that magnitude.

Because the simulation already hands out positions as frame-plus-offset, and already changes frames
with hysteresis at defined boundaries, the renderer inherits a floating origin rather than inventing
one. What it must add is a rule for what happens at a frame change: the camera's frame changes under
it, every cached patch origin is now expressed against a different centre, and a naive
implementation will visibly jump. **Lean:** frame changes are a renderer event, the scene's `f64`
origins are rebased in one operation, and a test asserts that a body's projected position is
continuous across the boundary to within a pixel, in the same way the flight model's test asserts
position and velocity are continuous.

### Depth: reversed-Z, and no logarithmic depth

A single perspective projection has to resolve a hull plate at 1 m and a planet at 10¹² m. The
classic answers are three:

| Approach                         | How it works                                                              | Verdict                                                                                                       |
| -------------------------------- | ------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| **Reversed-Z, float depth**      | Near plane maps to 1 and the far plane to 0, into a `depth32float` buffer | The float's dense values near zero cancel the projection's hyperbolic crowding. Free. **This is the answer.** |
| **Logarithmic depth**            | The fragment shader writes a logarithm of view depth                      | Works, but writing `gl_FragDepth`/`@builtin(frag_depth)` disables early depth rejection, which is a real cost |
| **Split frusta, several passes** | Draw near and far in separate depth ranges                                | Costs a pass per range and complicates every effect. A fallback if one buffer is ever not enough              |

**Lean: reversed-Z with a floating-point depth buffer and an infinite far plane**, which removes the
far plane as a tuning parameter altogether and is numerically the best-behaved combination. Depth
comparison flips to "greater", the depth buffer clears to 0, and any hand-written shader has to know
it. This is a decision that touches every shader, so it belongs in the first week of the renderer
and not in an optimisation pass later. Logarithmic depth is rejected on the early-Z cost, which the
Intel GPU can least afford.

### Luminance in physical units, and an exposure model

A space scene has no ambient light and a dynamic range no display can show: the Sun's disc is of
order 10⁹ cd/m² and a dim star's contribution is of order 10⁻³ cd/m². A renderer that works in
arbitrary units will either blow out the sunlit side of every planet or lose the star field
entirely, and the usual game trick of authoring light values by eye is not available, because the
simulation knows each star's luminosity and every body's albedo and will happily state them on a
console.

**Lean:** the renderer works in **absolute photometric units** end to end. Stellar luminosities and
albedos come from the simulation, irradiance at the body follows from the inverse square law, and
surfaces are shaded in cd/m². The camera then has a real exposure model — an EV100 derived from an
aperture, shutter and sensitivity triple — and what reaches the display is the tone-mapped result.
This is more work than it sounds only once: after it, sunrise on a planet, a dim red dwarf's light
and a magnitude-6 star all come out at the right relative brightness without tuning, which no amount
of per-scene tweaking would achieve.

It also gives the cockpit something real to show. Exposure is a _camera_ property, so it belongs on
the display as a control with a value, which is exactly the kind of instrument the UX guide wants,
and it explains to the player why the night side of a planet is black when the camera is exposed for
the day side.

## The view before the planets

The owner has already ruled that `VIEW` starts as a wireframe, and the single-player brainstorm's
[The view outside](single-player-experience.md#the-view-outside) says what it draws: bodies as
graticule spheres true to scale, rings as ellipses, orbits on request, stars as points by apparent
magnitude, craft as wireframe hulls, and the guide's symbology over it all, with the scene supplied by
the server at the retarded time. None of that needs restating. What this document adds is why the
wireframe is worth building on the full foundations rather than on a quick camera.

The wireframe is the **precision test rig**. It exercises camera-relative differencing, reversed-Z,
frame changes, the scene subscription and the depth-buffer policy at real scale, on a scene so cheap
that any wrongness is obviously wrongness rather than a performance artefact. If a moon jitters at
10⁸ m, or a body's position jumps when the ship crosses a sphere of influence, the wireframe shows it
against a grid with nothing else to blame. Every one of those failures is much harder to diagnose once
there is terrain, an atmosphere and a cloud layer in front of it.

So the order is: foundations and wireframe first, proven at real scale, and only then anything lit.
This is also the order the single-player brainstorm's plan of attack implies, and it is the reason its
step 2 is "the rendering engine, and `VIEW` as a wireframe at real scale" rather than a lit scene.

One addition to what it draws, which follows from having a photometric pipeline from the start: the
wireframe should already be **exposed**, not drawn at arbitrary brightness. Stars plotted at their
true apparent magnitudes through a real exposure model, even on a wireframe, immediately tell you
whether the photometry is right — and a star field with the local star's disc in frame is the
cheapest possible test of a 10¹² dynamic range. The field alone spans only some five orders of
magnitude between its brightest and faintest stars; it is the disc that takes it to twelve.

## Planets

### What the generator already owes us

Plan 14 is not yet built, but it is specified, and the surface generator should be written against
what it promises rather than inventing its own global statistics. From its `BodyRecord` and hooks:
radius and mass, hence surface gravity; bulk composition as iron, rock, water and envelope fractions;
atmospheric composition as ordered gas fractions with a surface pressure; equilibrium and surface
temperatures with day–night and equator–pole contrasts; Bond albedo, iterated against the surface and
cloud state; rotation period, obliquity and rotation phase at the epoch, with a `BodyFixedFrame` of
pole, prime-meridian angle and rate; ocean fraction, ice fraction (from the latitude at which the
zonal temperature crosses freezing, which [open question 9](#open-questions) finds wrong for several
classes of world) and cloud fraction; relief scaled as 20 km × (g⊕ ÷ g) × a
lithosphere factor; heat flow, and from it a tectonic regime and a volcanism level; surface age, and
from it a crater density by the Neukum–Ivanov–Hartmann chronology, scaled by the system's belt masses
and zeroed for small craters under a thick atmosphere; rings and belts as bodies in their own right.

And, crucially, a **`surface_seed`**, specified as a block output of the universe seed and the body's
ID alone, depending on nothing else, so that no later change to any other derivation can alter a
world's seed. That protects the random draws behind a map, not the map itself: the map also realises
plan 14's global figures, so a later change to how relief or ocean fraction is derived changes the map
too, and is a generator-version change like any other.

The surface generator is therefore a _consumer_, and its contract is narrow: given the surface seed
and those global figures, produce a height and a material at any point on the sphere, plus the coarse
fields the renderer and the climate need. It does not decide how much relief a world has or whether it
has an ocean — plan 14 already did, from physics — nor whether it has plates, how cold its poles are,
how much of it is ice, or how cratered it is. Wherever the coarse pass computes one of those figures
for itself, as the climate model does, the result is constrained to plan 14's value rather than
allowed to drift from it. This division is what keeps the terrain defensible: the numbers a console
states come from the astrophysics, and the terrain merely realises them.

### The geometry: a quadtree on a cube sphere

The choice is narrowed sharply by one fact about the API: **WebGPU has no tessellation stage**, and no
mesh shaders. Hardware subdivision is simply unavailable, so any scheme that depends on it is out
before its merits are weighed.

| Scheme                                            | Verdict                                                                                                                                                                      |
| ------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Quadtree on a cube sphere, fixed-grid patches** | **The answer.** Six root quadtrees, one per cube face; each leaf a fixed vertex grid displaced along the radial direction. Instances batch, and the topology is predictable. |
| CDLOD (Strugar 2009)                              | Its contribution is kept: continuous morphing between levels in the vertex shader, which removes cracks without skirts or stitching.                                         |
| Geometry clipmaps (Losasso and Hoppe 2004)        | Fixed memory and few draws, but camera-centred rather than body-centred, which is awkward when several bodies are in view. Rejected.                                         |
| Hardware tessellation                             | Not available in WebGPU or WebGL2. Rejected by the platform.                                                                                                                 |
| Mesh shaders, Nanite-style virtual geometry       | Not available. Also the wrong tool: terrain's structure is known, and its height is cheap.                                                                                   |
| ROAM                                              | Historical. CPU-bound triangle bintrees lost to GPU throughput two decades ago.                                                                                              |

Cube-sphere distortion — cells grow uneven towards the face corners — is the known cost, and it is
accepted because the compensation is what we want everywhere else: square parameter domains, trivial
quadtree refinement, and square textures. Its distortion is well characterised, in the
planetary-rendering literature and in Google's S2 geometry library, which indexes the Earth on a cube
sphere.

**Lean:** cube-sphere quadtree; leaf patches of a fixed grid (64 × 64 is the figure to start from and
measure); displacement along the radial direction in the vertex shader from a per-patch height
texture; vertex morphing between levels for continuity; per-patch `f64` origin as in
[Real-scale foundations](#real-scale-foundations). Height textures are generated once per patch and
cached, not evaluated per frame — the same trick SpaceEngine uses.

### Where the height comes from: the central tension

This is the hardest problem in the document, and it deserves to be stated plainly before any solution
is offered.

HYPERION's rule is that the universe is a pure function: generated on demand, never stored,
bit-identical everywhere. But the processes that make terrain look _real_ are global and iterative.
Plate tectonics moves whole plates and deforms shared crust. Hydraulic erosion carves a valley because
of the size of the basin uphill of it — drainage area is an integral over everything above a point, not
a local quantity. Noise alone gives the familiar fractal blobbiness: mountains with no coherent belts,
valleys with no drainage, coastlines that are just a contour of a random field. The realism ruling
wants the geology; the determinism rules want point evaluation. They genuinely conflict.

Two findings from the literature bound the answer. First, tiled iterative erosion does not simply
work: the published attempts name the two failures explicitly, propagation of drainage across patch
boundaries and seamless blending afterwards, and both are consequences of the non-locality, not
implementation slips. Error at a seam re-injects on every iteration. Second, the escape is well
trodden in another direction — **amplification**: a coarse field computed globally, and all fine
detail synthesised locally and conditioned on it. That literature is substantial, from sparse
dictionary representations through conditional generative models to real-time pattern libraries
reporting amplification factors up to thirty-two times.

### The coarse global pass, once per planet

So: run the global processes, but only at a resolution where global is cheap.

The resolution is set by what the transfer and the compute can afford, and the arithmetic is worth
writing out, because it is easy to get wrong by orders of magnitude. An Earth-sized body has 5.1 ×
10⁸ km² of surface: sampled at 100 km that is about 5 × 10⁴ cells, and at 10 km about 5 × 10⁶. At a
few tens of bytes per cell — elevation, plate identity, crustal type, temperature, precipitation,
prevailing wind, drainage area, ice, biome and crater state — the fine end is a hundred megabytes or
more, which is neither a cheap transfer nor a cheap simulation. **Lean:** level 7 or 8 of the
cube-sphere quadtree, cells of about 70 or 35 km, which is 98,304 or 393,216 cells and **roughly 2
to 15 MB** for an Earth. The simulation that produces it is then of order 10⁷ to 10⁸ cell updates
across its iterations, a fraction of a second of Rust rather than minutes. Both figures need
measuring rather than trusting, but they are the right order for "computed when a ship arrives".
Everything finer than a coarse cell is the local synthesis's job.

What the pass does, in order:

1. **Plates, where plan 14's tectonic regime has them.** A mobile-lid world is tessellated into some
   tens of plates, each given a motion. Spherical Voronoi is the standard construction, and its
   canonical algorithm was itself written for plate tectonics. Advance the plates enough to produce
   the landforms that read as geology — mountain belts at convergent boundaries, ridges at divergent
   ones, island arcs at subduction — following the procedural-tectonics line of work rather than a
   full geodynamic simulation. A stagnant-lid world, as Mars and Venus are, gets no plate boundaries:
   its large-scale structure is one lid, with volcanic provinces sized by plan 14's volcanism level
   and impact basins where its crater density puts them.
2. **Coarse elevation**, with continental and oceanic crust distinguished, scaled to the relief plan 14
   already computed from gravity and lithosphere, and with the ocean surface placed to match its ocean
   fraction rather than chosen by eye. The largest impact basins are placed here, from plan 14's
   crater density, because they are coarse-scale features that climate and drainage must see.
3. **Climate**, by the model the world's regime calls for ([open question 9](#open-questions)). For
   the common case a seasonal two-dimensional energy-balance model gives temperature — seasonal
   because the climate classes below are defined on monthly climatology — and there is one published
   specifically for the climates of rapidly and slowly rotating _terrestrial planets_, which is
   exactly the generality needed. Precipitation is the weak point and the literature says so: the
   standard reference text on these models warns outright that precipitation cannot be solved by
   simple models. So precipitation is a documented heuristic — zonal bands from the energy budget,
   plus an orographic correction with rain shadows from the coarse elevation and the prevailing wind —
   and it is labelled as a heuristic wherever a console shows it. The model's temperatures are
   normalised to plan 14's mean surface temperature and its day–night and equator–pole contrasts, so
   that the map and the console state one climate, and ice is placed where the model is coldest until
   its area matches plan 14's ice fraction, as the ocean surface is placed to match the ocean
   fraction.
4. **Drainage and erosion.** The stream-power law, solved by the analytical method of Tzathas et
   al. It is closed-form in time but not in space: it needs a receiver for every cell, a sort from
   ridges down to the outlets to accumulate drainage area, a pass back upstream, and iteration to a
   fixed point sped by multigrid, which is exactly why it runs here, on the coarse grid on the
   server, and never per point. Ocean cells are the base level, uplift comes from the plates, and the
   result is rescaled to plan 14's relief. It gives the final coarse elevation, flow directions,
   drainage area and a channel-steepness index. Its published timings, 1.8 s at 512² cells and
   8.2 s at 1024² in Python, bracket level 8's 393,216. This is the step that buys the realism,
   because it is computed globally where global is affordable, and everything local is then
   conditioned on it.
5. **Climate classes, and biomes where there is life.** Köppen–Geiger for the seasonal water-cycle
   regimes: it is temperature-and-precipitation driven, recognisable, and defensible in a way a
   hand-drawn biome map is not. It classifies climate, not life. Plan 14 says nothing about life, so
   vegetation follows only where a later biosphere says it exists, and a lifeless Earth-like world
   keeps its class with bare ground. The other regimes classify surface state instead — liquid, ice
   or frost of a named species, rock, regolith, melt, organic sediment — with the regime's named
   zones, such as a locked world's substellar ocean and nightside glacier.
6. **Crater state**, from plan 14's crater density rather than recomputed from surface age, since
   that density already carries the belt-mass scaling and the loss of small craters under a thick
   atmosphere. The pass adds a saturation level; the local pass turns everything smaller than the
   basins of step 2 into actual craters. Mars, under a thin atmosphere, is as
   cratered as its surface age says; Venus loses only its small craters.

**The grid.** HEALPix is equal-area with isolatitude rings and subdivides hierarchically, which is the
better physics grid; the cube sphere is the better rendering grid and is already the geometry. **Lean:**
the coarse field lives on the cube-sphere quadtree at a fixed shallow level, the 7 or 8 chosen
above, so that no second spherical indexing scheme exists and a patch's lookup into the coarse field
is an ancestor lookup. Equal-area sampling is a real loss for the climate model and the honest
mitigation is to weight cells by their true solid angle, which is computable in closed form.

**And this is not stored state.** The coarse field is a memoised pure function of the surface seed
and plan 14's global figures: recomputed on arrival, cached while the ship is there, discarded after,
and identical every time. The galaxy brainstorm's rule survives intact — it is the same relationship
a system's star already has to its seed, only with a larger intermediate result.

### Who computes the coarse field, and why it is the server

There is a choice here that looks like an optimisation and is actually the key to two other problems.

Both sides could run the coarse pass from the seed, since it is the same code. **Lean: the server
computes it and sends it, and the client never runs it.** Three reasons, in increasing order of
importance:

1. It is roughly 2 to 15 MB, once per planet, at the level chosen above. That is an affordable
   transfer for something a player will orbit for many minutes.
2. **It shrinks the determinism surface enormously.** An iterative plate-and-flow simulation is
   exactly the kind of floating-point code most at risk of diverging between native x86-64 and
   WebAssembly: accumulation order, transcendental functions, fused multiply-add. If only the server
   ever runs it, that risk disappears from the client-server agreement problem entirely. What must
   then match bit-for-bit is only the **local** synthesis — a pure function of the coarse field, a
   position and a detail seed, with no iteration — which is a far smaller and more testable surface.
3. **It is the Knowledge overlay's natural answer.** The coarse field is precisely "what the ship has
   established about this world from orbit". It arrives because the ship surveyed it, region by
   region, through the Knowledge overlay. The alternative — handing the client a seed from which it
   can generate the whole planet — is the awkwardness the single-player brainstorm flagged and left
   to this document.

One constraint follows from reasons 2 and 3 together, and it must not be lost. The local synthesis
agrees between client and server only if its inputs do, so **Knowledge gates how much of the coarse
field the client holds, never how accurate it is.** A surveyed region arrives as the server's exact
cells, whole, with a margin of neighbouring cells as wide as anything the per-query evaluation reads
— the interpolation, the river network's neighbourhood, and crater cells larger than a coarse cell —
so that the synthesis at a region's edge has the same inputs on both sides; an unsurveyed one does
not arrive at all. A degraded copy — smoothed, quantised harder, or
resampled at a sensor-limited resolution — would feed the client's synthesis different inputs, and
the ground it drew would stop being the ground the server collides with. If the field is quantised
for the wire, the server's own synthesis reads the same quantised values.

### The per-query evaluation

Per height query, on both sides, with no iteration and no global state:

1. Find the coarse cell and interpolate the fields across the sphere.
2. **Base elevation** from the interpolated coarse elevation.
3. **Structural detail** conditioned on crustal type and the distance to a plate boundary: ridged
   multifractal for a mountain belt, low-amplitude for an abyssal plain, domain-warped where a
   boundary is oblique.
4. **Channels below the coarse cell.** A point given only its cell's drainage area cannot know its
   own path downstream, so the channels come from a Dendry-style network (Gaillard et al. 2019):
   locally computable, jittered points joined to their lowest neighbour level by level, anchored to
   the server's flow directions and hashed from integer cells, with the reference code's
   standard-library generator replaced by the integer hash. The profiles follow the steady-state
   stream-power slope, S = k_s·A^−θ, with the drainage area A from Hack's law and the steepness k_s
   from the server's solution. The network's geometry is procedural and labelled so; the profiles
   are physics. Incision only ever cuts down, so each coarse cell's mean incision is subtracted — the
   server computes it and sends it with the cell — or the rule of
   [Level-of-detail consistency](#level-of-detail-consistency-and-why-collision-agrees) breaks.
5. **Craters**, wherever plan 14's crater density is not zero, by sparse convolution. A single hashed
   cell holding every diameter cannot work: at a cumulative size–frequency slope of −2 or steeper, a
   fixed cell holds some 10⁹ times more 1 m craters than 35 km ones, and a crater that crosses a cell
   edge is found only if the search reaches its rim and ejecta, out to about twice its radius. So
   each diameter octave has its own quadtree level, with cells at least as large as that octave's
   reach, and a query searches the 3 × 3 neighbourhood at each octave's level, across face edges,
   where a cube corner has seven neighbours. Each cell draws its count from the density at its own
   canonical point, weighted by its true area and capped at saturation, and each crater a diameter
   within the octave by inverting the production function's cumulative size–frequency distribution,
   truncated below the smallest crater the atmosphere lets through, then a jittered position and a
   morphology by diameter — simple bowl, complex with a central peak, or multi-ring basin, at
   transitions the literature provides. The cost is about nine cells per octave, which is still why
   the crater field is baked into the patch's height texture rather than evaluated per frame.
6. **Band-limit.** Sum only those octaves the current level of detail can resolve.

That last step is not a performance trick. It is what makes collision agree with the picture, and it
has its own section below.

### The line between truth and decoration

GPU decoration is how a planet gets pebbles without the server knowing about pebbles, and the line has
to be drawn explicitly because safety in the fiction depends on it. Two words are kept apart from here
on: _amplification_ is the authoritative local synthesis above, run on both sides, and _decoration_
is what only the GPU draws.

**Lean:** the authoritative height function — sim code, run identically on both sides — owns everything
the ship can collide with, down to a fixed band limit of **1 m**, the scale of a landing-gear
footpad: Apollo's lunar module pad was about 0.9 m across, and its gear tolerated 0.6 m of relief
within the footprint. The limit is one generator constant, not tied to the size of the body that
touches the ground, since two bodies on the same spot must meet the same surface, and it changes
only with the generator version. Everything finer is **GPU-only decoration**: high-frequency normal
detail, sand ripples and small crater scars. It never displaces geometry the collision query does
not know about, so the surface a hull touches is always the surface both sides computed. Because
decoration is not the simulation's, the view says when it is on, as
[the guide's edits](#what-the-guide-must-gain) require.

**Scatter** — rocks and vegetation as instances — splits at the same limit. Rigid instances larger
than it, boulders and trunks, are authoritative from the first phase that draws them: discrete
features in hashed cells per size octave, which the server answers for at contact points. A boulder
drawn but not collidable would be exactly the geometry the rule above forbids, and at the gear's
0.6 m tolerance it is exactly the landing hazard. Smaller scatter is decoration, flagged as such,
and culled where it intersects a grounded body ([open question 11](#open-questions)).

Two consequences worth stating. Normals should come from **analytic derivatives** of the height
function rather than finite differences: there is no arbitrary epsilon to tune, and they stay stable
across levels of detail, which is what stops shading from popping as patches subdivide. And scatter
placement is hashed from integer cell coordinates with an integer generator, which is what lets the
server answer "is there a boulder here" without storing one.

### Level-of-detail consistency, and why collision agrees

If the rendered surface at one level and the collision height at another disagree, a ship either
floats or sinks into the ground, and the bug is intermittent and miserable. The fix is a property of
the height function rather than a reconciliation step: **a coarse evaluation must be a smooth
low-pass approximation of a fine one.**

Summed-octave noise gives this for free when it is built for it — each octave is a frequency band, so
truncating the sum is exactly a low-pass filter, which is the observation the original eroded-fractal
terrain work rests on. The rule that follows: every contribution to height must be band-limited and
attributable to a level, and nothing may be added at a fine level that changes the mean at a coarse
one. Craters are the awkward case, because a crater is not a noise band; a crater must therefore
appear at the level whose resolution can represent its diameter, with its rim smoothed to that
level's resolution so that it sharpens as it refines, and its profile must integrate to the same
displacement at every finer level. Channels are the other, because incision only removes material,
which is why each cell's mean incision is subtracted.

The test is then simple to state and should be written early: for a sample of positions on a sample of
worlds, the height at level _n_ and the height at level _n + k_ differ by less than a stated bound that
depends only on _n_. Collision needs no tolerance at all: it reads the interpolated authoritative
heights of the finest level, whose vertex spacing is at most half the band limit, and that level is
drawn around every grounded body in view, so where anything touches the ground, collision and the
picture are the same mesh. Coarser levels serve sensors and distant views, with the level-_n_ bound
as their stated error.

### Determinism hazards specific to terrain

The sim's rules already cover streams, word consumption and iteration order. Terrain adds hazards of
its own, and they are the ones that bite across architectures:

- **Transcendental functions.** `sin`, `cos`, `exp` and friends are not bit-identical across libm
  implementations and targets. The repository already knows this: `libm` is pinned to `=0.2.16` with a
  comment that a bump is a generator-version change, and Clippy's `disallowed-methods` routes
  every transcendental function through `hyperion_sim::math`, in the sim's own `clippy.toml` and,
  since 22 September 2026, in a workspace-root one that binds the server, protocol and testkit
  crates. Terrain must live inside that discipline, and the surface crate needs a `clippy.toml` of
  its own modelled on the sim's rather than inheriting the root file, because the root file
  deliberately omits the sim's ban on float-bit conversions. That ban exists to stop floats being
  hashed, which is exactly the mistake a noise function is tempted to make.
- **Fused multiply-add, where the code asks for it.** rustc never fuses a multiply and an add on its
  own, so there is no contraction to turn off; the hazards are the explicit forms. `f64::mul_add`
  is a call to a software `fma` on wasm32 and a single instruction with the `fma` target feature,
  and it is already banned. The `algebraic_*` float methods, stable since Rust 1.98, license the
  compiler to fuse and reassociate, so they fuse under `+fma` and not on wasm; no `clippy.toml`
  bans them yet, and all three should. Core WebAssembly has no FMA at all; relaxed SIMD's
  `relaxed_madd` is the exception.
- **Summation order** in octave accumulation, which must be fixed and never reassociated.
- **Relaxed SIMD is banned outright, and mechanically.** Its whole premise is that an instruction
  may return different results on different hardware. A `compile_error!` under
  `target_feature = "relaxed-simd"` in the surface crate makes the ban a build failure. Fixed-width
  128-bit SIMD is IEEE-exact and welcome.
- **`min` and `max` are not exact at zero.** For equal inputs such as +0 and −0, Rust documents that
  either may be returned, and a flipped zero sign propagates through `1/x`, `atan2` and `copysign`.
  Both targets return the second operand today, but that is how the code compiles, not a guarantee,
  so the height path uses a `min` and `max` of its own that fix the sign.
- **A NaN's sign is nondeterministic on every target.** The sim already refuses NaN in golden files
  and bans reading float bits, but the sign still leaks through `total_cmp`, which the rules
  recommend, through `is_sign_*`, and through `copysign`. Heights are asserted finite before they
  are sorted, compared or emitted.
- **Flush-to-zero from outside.** Neither target flushes subnormals to zero under Rust, but a C
  library built with `-ffast-math` turns flushing on for the whole process when it loads. That
  matters once the server embeds an LLM runtime: such a runtime stays out of the server process, or
  the server checks the floating-point control register.
- **Seed derivation should be integer**, hashing integer cell coordinates rather than floats. The
  hash-based approach — deriving noise from a cheap cryptographic hash of the coordinates rather than
  from tables or state — is the technique that makes a noise function depend on nothing but its inputs.
- **`f32` anywhere in the authoritative path is a mistake.** Heights are `f64` in the generator and
  narrow only when they reach the GPU.

### Materials, and what a surface looks like

Kept brief, because it is the least uncertain part and the most easily changed:

- **Triplanar projection** for rock and detail textures, blended by the surface normal, which avoids
  the UV distortion any spherical parameterisation produces. Biplanar is the cheaper variant if three
  samples per fragment costs too much on the Intel part.
- **Splatting by slope, altitude and climate**, computed in the shader from height, normal and the
  coarse field rather than stored in splat maps: cliffs above a slope threshold are always rock, and
  ice appears where the coarse pass placed it, from the temperature field and plan 14's ice fraction,
  rather than by latitude alone, so a warm pole has no ice cap and a tidally locked world's ice sits
  where the climate model puts it.
- **Repetition hidden** by blending two scales with a per-patch rotation and offset, which is two
  samples rather than the four or more that stochastic approaches need.
- **Instanced scatter** from hashed cells, filtered by biome and slope, with the instance count falling
  off with distance.

### Atmosphere

The requirement is unusual and it decides the choice: HYPERION needs atmospheres for **arbitrary
compositions**, seen from the ground, from orbit and from outside, through the terminator, with
correct fog on terrain at every distance.

| Model                                            | Verdict                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| ------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Hillaire 2020, the production LUT approach**   | **Lean.** Four small tables — transmittance, multiple scattering, sky-view and aerial perspective — cheap enough to rebuild whenever the atmosphere or the sun changes: 0.31 ms for all four on a GTX 1080 at 720p, 0.5 ms with the per-pixel ray march it uses for views from space, and under a millisecond for the two per-planet tables on an iPhone 6s, which is roughly the UHD 620's class. It takes Bruneton's material model — his density profiles, ozone layer and Cornette–Shanks aerosol — and Bevy 0.19's version generalises it to any number of terms, each with its own density and phase function. RGB rather than spectral. |
| Bruneton's precomputed scattering, 2017 revision | Multiple scattering precomputed into four-dimensional tables, inside and outside the atmosphere, with aerial perspective, and spectral at no runtime cost. But an update takes 250 ms on the same GTX 1080, about 150 ms on the discrete target and seconds on the UHD 620, and its density profiles are limited to two layers, with one aerosol and one absorbing layer. Its WebGL demo loads tables precomputed offline. The reference if Hillaire's RGB approximation proves visibly wrong.                                                                                                                                                 |
| Nishita 1993, O'Neil (GPU Gems 2)                | Single scattering only, with the known darkening artefacts and a phase function disabled to hide them. Too approximate for a display that claims physical units.                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| Hosek–Wilkie and other analytic sky models       | Fitted for ground-level daylight on Earth. No use from orbit.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |

**Parameterising from physics** is the part that must be built rather than borrowed, and it is
straightforward in outline. The medium is a list of terms, each a density profile, a scattering and
absorption coefficient and a phase function, as Bevy's is, so a new gas is a new term rather than a
new model. Rayleigh scattering follows from the number density and refractive index
of the mixture, falling as the inverse fourth power of wavelength; the scale height follows from
temperature, mean molecular mass and gravity, all of which plan 14 provides. Earth's reference values
anchor the implementation: a Rayleigh scale height near 8 km, an aerosol scale height near 1.2 km, an
aerosol asymmetry parameter about 0.76, and Rayleigh coefficients of order 10⁻⁵ m⁻¹ rising towards the
blue. One caution for whoever writes the code: **published values for these coefficients differ** —
sources consulted here gave both 3.8 × 10⁻⁶ and 5.8 × 10⁻⁶ m⁻¹ for the red end, a 50% difference.
The λ⁻⁴ law takes 5.8 × 10⁻⁶ at 680 nm to 3.8 × 10⁻⁶ near 755 nm, so the difference may be a
different choice of "red" rather than a disagreement about physics. Either way the constants must be
derived from the formula at stated wavelengths, with a cited source, rather than copied from a
tutorial, exactly as the project's rules already require of physical constants.

Absorption and aerosols are where character comes from: ozone on an Earth-like world, suspended dust
on a Mars-like one, hydrocarbon haze on a Titan-like one, and skipping them cannot be patched over by
tuning the Rayleigh terms. Mars' butterscotch sky and blue sunsets are both dust effects: without the
dust, its thin carbon dioxide would give a dark, faintly blue sky. Tinting the Rayleigh coefficients
can make the daytime colour right for the wrong reason, but Rayleigh scattering is nearly symmetric
while dust scatters strongly forward, so the glow around the sun and the blue of a Martian sunset —
both forward-scattering effects — would still come out wrong. Bevy's Mars preset takes its dust's
phase function from Mie theory, after Schneegans et al. 2024, which is the precedent to follow.

Thick atmospheres are where both models are unproven. Bruneton's multiple-scattering iterations fail
to converge and then diverge at 40 orders, Hillaire's colour can drift at very high scattering
coefficients, and Bruneton's table layout mishandles small bodies with thick atmospheres, which is
Titan's case. Venus, at a Rayleigh optical depth near 15 and a cloud optical depth near 30, is a
diffusion regime that neither was built for. **Lean:** Hillaire's tables, regenerated when the
atmosphere or the sun changes, with a ray march for views from orbit, where the aerial-perspective
volume's 32 km reach runs out; and Venus- and Titan-class atmospheres validated against a
path-traced reference before they ship ([open question 3](#open-questions)).

### Clouds

Volumetric clouds are the most expensive thing in this document and the first thing to cut. The
reference point is Guerrilla's Nubis, whose 2015 prototype ran in under 2 ms on a PlayStation 4 — and
that is _with_ the quarter-resolution raymarching and temporal reprojection such systems require, not
instead of them. The discrete estimate in [the budget](#performance-budget) is about that figure on
a GPU some eight times a PlayStation 4's, where scaling alone would predict a quarter of a
millisecond, and the reason should be stated rather than implied: Nubis draws a layer seen from the
ground, and HYPERION's clouds are a shell seen from orbit, from within and from below, which needs
longer marches and more samples. A good implementation
should bring the estimate down.

**Lean:** a raymarched volumetric layer on a shell around the planet, at reduced resolution with
temporal reprojection, on the high setting only, driven by plan 14's cloud fraction and the climate
field so that cloud sits where the precipitation is. Cloud shadows on terrain come from the same
volume. On the low setting it becomes a **two-dimensional animated layer with the correct albedo and
optical depth** — which still reads correctly from orbit, where clouds matter most for recognition, and
degrades gracefully from below. This is the clearest example of the quality ladder the owner's ruling
requires, and it should be built with both settings from the start rather than retrofitted.

### Oceans and ice

An ocean from orbit is recognisable by exactly one thing: **specular sun glint**. It is worth getting
that right before any wave geometry, because it is what tells a player at a glance that a world has
liquid on it. The glint's shape comes from the statistics of wave slopes, which depend on wind, so a
sea state from the climate field feeds it directly.

For the surface itself, **Gerstner waves** are the lean: cheap enough to have run on shader model 1.0
hardware, with analytic normals, and displacement that sharpens crests the way real waves do. Their
one trap is documented and easy to test for — if the summed steepness exceeds a limit the normals
invert and the waves visibly loop. A spectral FFT ocean with a real wave spectrum is the WebGPU
upgrade, and it is genuinely better, but it wants compute and it is not where the first effort goes.

Two gaps, one smaller than it first looked. A **spherical** ocean at planetary scale is published.
Bruneton, Neyret and Holzschuch 2010 draw the ocean from space as a sphere whose reflectance comes
from the slope statistics of the waves too small to resolve — a Cox–Munk distribution, which is
exactly the glint-first path above — and switch below 20 km to a projected grid on the sphere.
Proland's ocean module implements it, working in a frame tangent to the sphere under the camera, and
Scatterer carries it into Kerbal Space Program; Outerra's blog describes Gerstner waves on terrain
patches. What remains our own is an ocean fixed to the planet on the terrain quadtree, consistent
across levels of detail, rather than a projected grid that follows the camera, and a wave spectrum
driven by the climate field. And the **shoreline**, where a wave field meets procedural terrain, is
the classic hard case: it needs the coarse ocean level, a depth-dependent wave amplitude, and foam
driven by the terrain's slope, and its only published account is Outerra's, a distance map with
waves chosen by depth.

Ice is the coarse pass's, not the ocean's. Where the coarse field's ice covers the sea, the ocean
surface gives way to sea ice drawn as a terrain material, with no glint and no waves, so that a
frozen ocean reads as ice from orbit and the ice cap a console states is the one the window shows.

### Rings

Rings are easy to make look wrong and the published rendering accounts are thin, so what follows
rests more on planetary photometry than on graphics papers.

The physics to respect: a ring is an optical depth, not a surface, so it is rendered as transmittance
through a particle layer. Its phase function has two parts. The main rings' icy particles, from
centimetres to metres, scatter light _back_ towards the sun, with an opposition surge at small phase
angles, and the rings darken markedly as the phase angle grows; the dusty rings and the spokes
scatter forward. That is why a backlit ring looks utterly different from a front-lit one: the main
rings fade and the dusty ones appear. The unlit face is lit by diffuse transmission. It needs the
planet's shadow cast on it and its own shadow cast on the planet, both of which are strong,
recognisable cues. Shadowing between particles is worth approximating, since it accounts for about a
fifth of the B ring's brightening with elevation, and it is an analytic function of phase angle,
elevation and optical depth rather than something to simulate. Björn Jónsson's published radial
profiles — backscattered, forward-scattered, unlit side and transparency — are ready-made data for the
annulus.

Where the annulus gives way follows from the pixel. At 1080p and a 60° field of view a pixel is
about 1.07 mrad, so a body of size _D_ fills one at about 935 × _D_: 9.4 km for a 10 m particle, and a
10 m-thick layer seen edge-on reaches a pixel at the same range. **Lean:** the ring is an
optical-depth slab while the camera is further from the ring plane than the largest particle's
one-pixel range, about 10 km for Saturn-like rings. Below that, only bodies larger than a pixel are
instanced, from the top size decade, and their share of the optical depth is removed from the slab,
so that total extinction is conserved. Within a few layer thicknesses the view becomes a local
particle field inside volumetric extinction, where sideways visibility is only some ten metres.
SpaceEngine does the same in outline, without publishing its criterion.

### Knowledge, and the surface seed

The single-player brainstorm raised a problem and deferred it here: the client needs the surface to
draw it, but the rule is that the client holds only what the ship has seen, and a seed that reveals
the whole surface at once breaks that rule.

The [server-side coarse field](#who-computes-the-coarse-field-and-why-it-is-the-server) resolves most
of it. The client is given the regions of the coarse field the ship has surveyed — exact where it has
them, absent where it does not — and never the surface seed. The fine synthesis it runs locally takes
a **detail seed** instead: a one-way derivation from the surface seed, from which the coarse field
cannot be regenerated, so the client can only elaborate what the ship already established.

What remains is honest labelling, and it is a rendering requirement rather than a fiction one. The
amplified terrain is not an approximation: the server collides with the same function, so it is the
ground itself, and the view band-limits it to what can be resolved from where the ship is, which is
what an eye at the window would see. What the display must label is what is _not_ the ground: GPU
decoration, a setting that draws the surface coarser than the ship's position warrants, which is the
degraded-rendering state proposed under [What the guide must gain](#what-the-guide-must-gain), and
the edge of what has been surveyed. A console readout that quotes the ground — an elevation or a
slope at a distant landing site — is a different matter, because it states a measurement: it quotes
the coarse survey, under the guide's estimated state, until the ship's sensors have seen the point,
in the same way an unscanned contact's mass carries its `~`.

A region not yet surveyed at all is drawn as what the ship knows of it, the disc and atmosphere from
plan 14's figures, with no terrain claimed. That cannot extend to ground the ship can reach. A ship
descending over an unsurveyed region surveys it with its own sensors on the way down, so the server
sends the cells under and ahead of it before it could touch them, and no hull ever meets terrain the
client was not given. **Lean:** the view carries the survey coverage and resolution as a readout,
the same way the star chart carries its census line.

## The sky

### The star field is the galaxy, not a photograph

Most space games paint a sky box. HYPERION should not, and for once the realistic answer is also the
one this project is built for: the galaxy model already knows where every star is, and plan 06's
stellar stage knows what each one is. Plan 06's task T33 puts a stellar brief — kind, class,
luminosity and effective temperature — on each row of the range query when the request asks for it,
which is what a star field needs. The harder problem is selection. The range query is
**volume-limited** — a radius, a mass-layer floor and a `limit` — while a sky is **flux-limited**: a
K giant 3,000 ly away can outshine a red dwarf at 10. Range queries cannot serve it. To naked-eye
magnitude 6.5 the brightest stars of the heavier mass layers — supergiants, O stars, and giants from
the 0.75–2.5 M☉ layer — are visible thousands of light-years away, and the spheres that reach them
hold some 3.6 × 10⁷ systems: far past the server's cap of 20,000 expected systems per query, and a
few minutes of one core per arrival merely to place them. The sky therefore needs a request of its
own ([open question 13](#open-questions)). With it, the sky is **generated from the same model the
charts read**, which means the view out of the window and the `GALAXY` display cannot disagree, and a
star the player jumps to is the star they were looking at.

The practical form:

- **Apparent magnitude sets the flux**, from the star's luminosity, its distance, and the extinction
  along the line of sight from plan 07's dust field, and flux converts to the absolute luminance
  units the rest of the pipeline uses. Nothing is authored by eye. The sky's limit is magnitude 6.5,
  which gives about 9,100 stars across the whole sky and some 450 in a 60° view at 1080p.
- **Colour from the effective temperature**, by evaluating the Planck function, integrating against the
  CIE colour matching functions and converting to the display primaries, desaturating towards the white
  point when a colour falls outside the gamut. Precomputed as a one-dimensional table against
  temperature, which is exact enough and costs a texture lookup, and reddened by the same dust that
  dims the star.
- **Faint stars baked into a cubemap per location; bright ones drawn as sprites.** Tens of thousands
  of point sources are expensive to draw and, worse, they alias: a sub-pixel star flickers as the
  camera turns, which is the classic failure and it looks like a bug. Baking integrates each star's
  flux over the texel it falls in, which is correct antialiasing only while a texel is no larger than
  a pixel — and at 60° across 1080p that needs faces of about 2,900 texels, some 200 to 400 MB for
  the six in a floating-point format, and more if the view zooms. So the bake takes the faint,
  unresolved majority at a resolution the memory affords, where a faint star spread over a texel
  reads as the sky's grain, and the roughly 1,600 stars brighter than magnitude 5 are drawn every
  frame as sprites whose point-spread function is integrated over the pixel, which keeps their
  antialiasing at any field of view. Light fainter than the limit belongs to the galactic band and
  to a statistical layer of unresolved stars, labelled as such. A deep exposure that wants fainter
  individual stars asks for a narrow cone rather than the whole sky: a 1° field is 2 × 10⁻⁵ of it.
- **Baked once per arrival, with parallax left to the sprites.** A star at distance _d_ shifts by
  206,265 × (baseline ÷ _d_) arcseconds, against roughly 110 arcseconds per pixel at 1080p across a
  60° field of view. In the solar neighbourhood, with the nearest star over a parsec away, a journey
  of a few astronomical units moves the sky by a few arcseconds, a few hundredths of a pixel. But the
  galaxy brainstorm places some fifteen million pairs of systems within 0.1 ly of each other, over
  half of them in the nuclear disc, and a star 0.1 ly away shifts by about 33 arcseconds per
  astronomical unit: crossing 30 au moves it nine pixels. **Lean:** a star that would shift by more
  than a tenth of a pixel across the system is drawn as a sprite at its true position every frame
  rather than baked, and everything else is baked once per arrival. The threshold is a number, and
  saying so with it is better than hoping nobody asks. How many sprites it makes depends on where
  the ship is. For a 30 au journey it takes every star within about 9 ly, which near the Sun, at
  about 0.002 systems per cubic light-year, is a handful. In the nuclear disc, at some 16, it is
  about 50,000 systems, of which roughly a third are bright enough to be in the sky at all, and in
  the nuclear cluster nearly every star. There the bake is instead redone as
  the ship moves, whenever the nearest baked star's accumulated shift reaches the threshold, and the
  budget's star-field figure is the solar neighbourhood's.
- **The galactic band from the galaxy's own model.** The Milky Way seen from inside is the light of
  the stars too faint or too many to draw individually, integrated along each line of sight outward
  from the ship and dimmed by the dust in front of it. That is related to what the `GALAXY` map
  shows but is not the same integral: the map's column density counts systems per square light-year
  along parallel lines through the whole galaxy, where the band needs luminosity along rays from a
  point, with extinction. It comes from the same density, stellar and dust fields, so it cannot
  disagree with the charts, and it must take exactly the light the selection leaves out — stars
  below the limit, and those beyond each layer's capped radius — or the stars near the ship are
  counted twice. Drawing the band from the model means a ship in the outer disc sees a thin bright
  line in one direction and a sparse sky in the other, _because the model says so_, and dust lanes
  appear when the dust field does. No other game can do this, because no other game generates the
  galaxy it is standing in.

### The local star as a disc

Close to, the star is a disc with structure, and two details do most of the work. **Limb darkening** —
the disc is brighter at the centre than at the edge, because a sightline near the limb passes through
cooler, less emissive layers. The standard form is a polynomial in the cosine of the emission angle;
for the Sun at 550 nm the coefficients give a limb at about 30% of the central intensity and a
disc-averaged intensity of about 80% of centre. It is a few lines in a fragment shader and it is the
difference between a star and a white circle. **Angular diameter** is twice the arctangent of radius
over distance — about half a degree for the Sun at 1 au — and it must come from the real radius, since
plan 06 computes it.

Eclipses and the terminator follow from the disc having a size: a shadow is soft because the star is
not a point, so the penumbra comes from integrating visibility across the disc, sampled at a handful of
points. A ship crossing a moon's shadow should see a partial eclipse because the geometry produces one.

### Exposure and tone mapping

The pipeline works in absolute luminance, so something must map 10⁻³ to 10⁹ cd/m² onto a display. The
photographic model is the one to use, because it is the one that makes the camera's behaviour explicable
to the player:

- **Metering** gives an exposure value from the average scene luminance, with the standard reflected-light
  calibration constant, and the maximum luminance that maps to white follows from it. With the
  conventional constants this reduces to a pleasingly simple relation: the white point sits at roughly
  **9.6 times the average scene luminance**. That white point belongs to a linear camera that clips.
  Under AgX, which compresses highlights over several stops instead, the figure is the exposure's
  calibration, fixing where the metered average lands, and not a hard white: brighter values roll
  off rather than clip.
- **The average comes from a histogram**, computed on the GPU over log-luminance and smoothed over time.
  A histogram rather than a downsampled average, because a single extreme value — a star's disc — drags
  a mean badly, which is precisely the failure mode of a space scene. Compute shaders make this easy,
  and this is one of the places the WebGPU-only ruling pays for itself. If it proves too slow on the
  Intel part, the fallback is a coarser histogram over a smaller input, never a mean.
- **The star's disc is excluded from metering.** Otherwise the exposure hunts every time the star enters
  frame, and everything else goes black. This is what real cameras do badly and real pilots complain
  about, so an override that lets the operator meter on the sunlit or the dark side is both realistic
  and useful — and it is an instrument with a value, which is what the guide wants.
- **Tone mapping: AgX.** It is a full colour pipeline rather than a curve — a log encoding across roughly
  25 stops, a three-dimensional lookup, and a conversion to the display space — and it is now the
  default in Blender, having displaced the previous generation. Its behaviour in extreme highlights is
  the reason to choose it here: hues hold instead of sliding towards white, which matters when the
  brightest thing in frame is a star. ACES is the credible alternative and is better supported for
  high-dynamic-range output; the Khronos neutral operator exists but has little visible adoption. All
  three are a lookup at runtime, so the choice is reversible.
- **Bloom is veiling glare**, a lens effect on real light sources, and it is the thing that stops a star
  from being a hard-edged clipped disc. It is allowed on the image and forbidden on symbology, as
  [the guide's edits](#what-the-guide-must-gain) set out.

## Performance budget

The owner's ruling gives this section its shape: a modern discrete GPU is the design target at 1080p60,
and **every feature needs a documented low setting that stays playable on the UHD 620**. Playable, not
60 fps — the point is that the real renderer can always be developed and tested on the machine that
exists.

What that machine is, stated plainly because every estimate below depends on it: 24 execution units at
about 1.1 GHz, so nominally of order 0.4 TFLOP/s of 32-bit arithmetic, with no dedicated video memory —
bandwidth is system memory shared with the CPU, a few tens of gigabytes per second. The design target
needs a name for the comparison to mean anything, and this section assumes **a current mid-range part
of the RTX 4060 class**, about 15 TFLOP/s and 270 GB/s: some 35 times the arithmetic and 7 times the
bandwidth. Rendering at 720p rather than 1080p takes back a factor of 2.25, so a pass that costs 1 ms
on the target costs of order 3 to 15 ms on the UHD 620 at the same quality, depending on whether it is
bound by bandwidth or by arithmetic. It is not a small difference to be tuned away; it decides which
features exist at all on the low setting. **Lean:** the low setting's target is **30 fps at 720p**, a
33 ms frame.

**Every number in the table below is an estimate, not a measurement.** They are recorded so that the
first real measurement has something to contradict, and the plan that implements this should replace
them with measured figures and keep them under version control.

| Pass                          | Discrete target, 1080p, 16.7 ms budget                                       | UHD 620 low setting, 720p, 33 ms budget                                                 |
| ----------------------------- | ---------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| Terrain geometry and patches  | 3–5 ms                                                                       | 8–14 ms, a shallower quadtree away from the ship; full depth kept under grounded bodies |
| Atmosphere, per frame         | 0.5–1 ms: sky-view and aerial-perspective tables, and a ray march from orbit | 2–4 ms, smaller tables, aerial perspective on terrain only                              |
| Atmosphere, per-planet tables | Under 0.1 ms, when the atmosphere or the sun changes                         | About 1 ms, on the same occasions                                                       |
| Volumetric clouds             | 1.5–3 ms at quarter resolution                                               | **Cut.** Replaced by a two-dimensional layer at about 1 ms                              |
| Ocean                         | 1–2 ms, Gerstner                                                             | 2–3 ms, fewer wave components, glint retained                                           |
| Shadows                       | 1.5–3 ms, cascaded, with cloud shadows                                       | 2–4 ms, one cascade or a horizon map for terrain self-shadowing                         |
| Star field and galactic band  | Under 0.2 ms, a cubemap and bright-star sprites                              | Under 0.5 ms                                                                            |
| Exposure histogram            | 0.3–0.5 ms                                                                   | About 1 ms, over a quarter-resolution input                                             |
| Bloom and tone mapping        | Under 1 ms                                                                   | 2–3 ms, fewer bloom levels                                                              |
| Scatter instances             | 1–2 ms                                                                       | Off, or a token density under 1 ms; the first thing after clouds to go                  |
| Rings, when in view           | 0.5–1 ms                                                                     | Under 0.5 ms, a textured annulus                                                        |

The discrete column's lower ends sum to about 9 ms and its upper ends to about 18 ms, which is over
budget. That is recorded rather than tuned away: the frame fits only if the passes do not all land at
their worst, and the first measurements decide which one gives. The UHD 620 column sums to about 18
to 32 ms, inside its 33 ms frame with little to spare. The sums leave out the rings, which are in
view only near a ringed body, and the per-planet atmosphere tables, which are not rebuilt every
frame.

The table is GPU time for the view alone, and two costs sit outside it. The consoles beside the view
share the same GPU for their own canvases and for compositing, which on the UHD 620 is not free. And
the descent's heaviest cost is on the CPU: every patch's heights come from the height function in
WebAssembly workers, a descent streams patches faster than any other situation, and in
single-player the same processor also runs the server. That cost has no row because it is not a
frame cost, but it is what [the descent test](#testing) watches for as the moment streaming cannot
keep up, and the spike measures it.

The ladder, stated as policy rather than as a list of numbers:

| Feature        | High                                                                      | Low                                                                                     |
| -------------- | ------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| Clouds         | Raymarched volume with temporal reprojection                              | Two-dimensional layer, correct albedo and optical depth                                 |
| Terrain detail | Full quadtree depth, GPU decoration                                       | Shallower quadtree away from the ship, decoration off; full depth under grounded bodies |
| Ocean          | Gerstner waves, shoreline foam, sun glint; spectral waves later           | Fewer Gerstner components, sun glint retained                                           |
| Shadows        | Cascaded, with cloud shadows on terrain                                   | One cascade or a horizon map: terrain self-shadowing only                               |
| Atmosphere     | Full tables, aerial perspective on everything                             | Smaller tables, aerial perspective on terrain only                                      |
| Scatter        | Instanced, filtered by biome and slope                                    | Off, or a token density                                                                 |
| Rings          | Transmittance with a two-part phase, both shadows, and particles close to | A textured annulus with the planet's shadow                                             |
| Resolution     | 1080p native                                                              | 720p, presented upscaled                                                                |

Three rules keep this honest. The renderer **states its setting on the display**, under the
degraded-rendering data state, so a player is never misled about whether they are seeing the real
surface. Whatever the setting, **the patches under every grounded body in view are drawn at the
finest level**, which is the one collision reads: the low setting may coarsen what nothing is
touching, but never what something is, or the guarantee of
[Level-of-detail consistency](#level-of-detail-consistency-and-why-collision-agrees) would hold only
on the high setting. And the low setting is **built alongside the high one**, not
retrofitted: a two-dimensional cloud layer written after the volumetric one exists will never be
tested and will rot.

## Fit with the consoles

`docs/frontend/ux-guidelines.md` was written for instruments: thin vector marks on `--surface-0`,
reserved colours, no gradients, redraw on demand. A view out of the window breaks nearly every one of
those sentences, and it is right to. The guide's own principle is that colour, motion and light are
spent on what needs attention — and a photometrically correct image of a planet is not decoration, it
is the most information-dense thing on the ship. What matters is that the guide gains the exception
_explicitly_, with its boundary written down, rather than being quietly ignored by one display. The
boundary is this: **the image is data and obeys physics; everything drawn over the image is
symbology and obeys the guide.**

### The contrast problem, which is the real one

The guide requires 6:1 contrast for any mark that carries meaning. That is a promise about marks on
`--surface-0` at `#05080d`. Over a sunlit cloud deck the background is at the top of the display's
range, and `--text` at `#c8d6e5` against it fails badly — a flight-path marker that vanishes over a
bright planet is a safety defect in the fiction and a usability defect outside it.

The fix is one this project already uses. The galaxy map's cursor is 1.5 px of `--accent` cased in
3.5 px of `--surface-0`, because `--accent` falls to 1.3:1 over the top of the density ramp. (An
optical head-up display cannot do this: its combiner only adds light, so it manages contrast with
brightness instead, and it is the wrong precedent for a view drawn on a screen.) **Lean:** every
symbology mark is stroked twice — a wider `--surface-0` stroke beneath a thin coloured stroke — so
that each mark carries its own dark surface with it and the guide's pairing holds against any
background. That is an outline, not a glow or a drop shadow, and the distinction needs to be in the
guide so the two are not confused. Text over the image needs the same protection and cannot get it
from a canvas stroke: a DOM readout positioned over the canvas sits on a `--surface-0` plate of its
own, which is chrome and obeys the guide. The alternative, dimming the image under the symbology, is
rejected: it falsifies the image, and the honest-data principle forbids it.

A second consequence is subtler. A real image contains real colours, so a rusty planet fills the
window with something close to `--status-warning` and a chlorophyll-green one with
`--status-nominal`. The guide's reservation of the status colours cannot apply to photons. It applies
to symbology and chrome, which is why symbology must be told from the image by _shape and outline_
first — a rule the guide already states in another form, and which now has teeth.

### What the guide must gain

Collected here so a plan can make the edits in one pass:

1. **A class of display: the view.** Perspective, redrawn every frame, always labelled as a view, and
   **not a spatial display**. The conventions that
   [three-dimensional spatial displays](../../frontend/ux-guidelines.md#graphs-schematics-and-spatial-displays)
   carry do not apply to it as a set, because a perspective image breaks nearly all of them: it is
   not orthographic and has no single scale, its camera rolls with the ship, it has no reference
   plane or stalks, and physics dims and shrinks what is far away, where the guide says nothing is
   dimmed by depth. What it keeps is stated instead: the contact symbol set with shape for type, the
   bracket reticle for the selection and `--target` for a commanded destination, predicted paths
   dashed, the frame name and the time always shown, and the canvas paired with a DOM list. This was
   already leaned in the single-player brainstorm's open question 11; this document supplies the
   wording it needs.
2. **The rendered image is data.** The ban on gradients, glows, blurs and transparency governs
   console chrome, and a photometric image is not chrome. This is a new exception, not an instance of
   the raster-field one: that exception is single-hue and forbids "smoothing or interpolation that
   invents values", and the view is full colour and carries GPU decoration, which is invented detail
   by construction. The exception therefore comes with its own honesty rule: what the simulation did
   not compute is labelled, so the view states when decoration is on, beside its survey coverage and
   its quality setting. Symbology over it is unaffected.
3. **Outlines for symbology, and only for symbology.** The two-stroke rule above, stated as the way
   contrast is met over an image, with a note that it is not the banned glow, and the `--surface-0`
   plate for DOM text over the image.
4. **Glare as a physical effect.** Bloom and veiling glare around a real light source are camera
   optics and are allowed on the image. They are never applied to symbology or chrome, which keeps
   them clear of the guide's ban on neon glow.
5. **Scale, when there is no scale bar.** A perspective image cannot carry the 1-2-5 scale bar the
   guide demands, because it has no single scale. The view instead states its **field of view in
   degrees** and its reference frame, and every target carries a range readout. That substitution
   should be written down rather than left as an omission.
6. **Exposure is an instrument.** The camera's exposure is shown as a value with its unit and its
   automation level (`AUTO` or `MAN`), under the guide's existing rule that automation always shows
   who is in control.
7. **Degraded rendering is a data state.** If the renderer is not drawing the surface at the detail
   the ship's position warrants — because the hardware cannot, or because terrain has not streamed in
   yet — the display says so. This is the honest-data principle applied to pixels: an approximated
   surface must not present itself as a surveyed one. It is also the hook by which the low settings
   of [the performance budget](#performance-budget) stay honest.
8. **Motion, for a display that never stops.** The view redraws continuously. The guide already lets
   gauges and plots move at frame rate, but it has no display whose whole picture moves by itself,
   and it requires three-dimensional displays to redraw on demand only. Under
   `prefers-reduced-motion` the world cannot be frozen, but every _non-physical_ motion must stop:
   camera easing, preset transitions, idle drift, and any animation not caused by the simulation.
   Readouts stay at the guide's 4 Hz.

### Accessibility, which a canvas threatens

The guide's existing answer applies unchanged and is worth restating because it is easy to lose in a
3D display: the canvas is focusable, carries an accessible name, and is **paired with a DOM list** of
what is in view — contacts, bodies and the selected target — from which selection works by keyboard.
Readouts go in `output` elements in B612 Mono, on their `--surface-0` plates, positioned over the
canvas rather than drawn into it. The star chart already works this way, and the view inherits the
pattern rather than inventing one. Every camera control is reachable from the keyboard, and none
depends on hover or a right click.

## Runtime and code shape

What exists today, checked against the tree rather than assumed:

- **The frames are built.** `hyperion_sim::coords` provides `GalacticPosition` (an `LyCell` of `i32`
  plus an `f64` metre offset), `SystemPosition`, `BodyPosition` and a `Frame` enum of
  `Galactic | System(SystemId) | Body(BodyId)`. `GalacticPosition::displacement_to` subtracts integer
  cells before offsets and is precisely the floating-origin primitive the renderer needs; the client
  already has the same operation as `galacticDeltaLy` in `packages/protocol/src/position.ts`. Frame
  selection with hysteresis is implemented in `hyperion_sim::galaxy::frame`.
- **The spatial view is orthographic and pure.** `apps/hyperion/src/renderer/src/spatial/` holds a
  declarative scene (`marks.ts`), a pure orthographic projection (`camera.ts`), a pure draw list with
  its own painter's-algorithm ordering (`drawList.ts`), pure picking (`pick.ts`) and one
  canvas-bound painter (`paint.ts`). Its `Camera` carries a single `pxPerUnit`, because the guide
  requires one scale for the whole picture. There is no perspective projection, no field of view and
  no view matrix anywhere in the client.
- **Nothing planetary exists.** `hyperion-sim` has no `planetary` module, no body record, no orbit
  propagator and no `surface_seed`; plan 14 specifies all of them and has not started. Plan 12's
  retarded-time and Knowledge machinery is likewise unbuilt.
- **The client loads no WebAssembly at all**, and the sim's second-architecture check runs on
  `wasm32-wasip1` under wasmtime, by hand, outside `just ci`. The browser target,
  `wasm32-unknown-unknown`, is in no check.

That last point sharpens a sentence in the single-player brainstorm. It says "`hyperion-sim` already
compiles to WebAssembly, and CI checks it bit for bit there", and rightly makes the shared terrain
conditional on the browser target joining those checks. What it overstates is the check itself: the
WASI run is a manual gate outside `just ci`, not CI. So there are two gaps rather than one — the
browser target is in no check, and the check that exists is not automatic — and both are
load-bearing for the whole shared-terrain argument. They should be closed before any terrain code is
written, the descent spike's included, rather than after.

**Lean, for the shape:**

- **A crate of its own for the surface.** The client should not have to ship galaxy generation to ask
  for a height. Terrain belongs in a small crate — `hyperion-surface` — that compiles to
  `wasm32-unknown-unknown` for the client and links natively into the server. The wasm bundle then
  holds the height function alone. It cannot depend on `hyperion-sim`: the flight model is sim code,
  and collision needs heights, so the sim depends on the surface crate and a dependency back would be
  a cycle. The sim's `math`, `rng` and `units` modules therefore move into a crate beneath both, which
  each depends on, and their discipline — the `libm` pin, the Clippy bans — moves with them. The
  coarse pass, which only the server runs, lives in the sim beside plan 14's planetary stage and hands
  its field to the surface crate as data.
- **Both wasm targets join the checks, automatically.** `wasm32-unknown-unknown` is exercised with
  `wasm-bindgen-test` in its Node mode, with Electron's own binary standing in for Node through
  `ELECTRON_RUN_AS_NODE=1`, so that the check runs on the V8 the client ships — 15.2 in Electron 44,
  where the system's Node carries 12.4 and its Chromium 15.3. The golden height files are asserted
  equal across native, wasip1 and the browser target. The fast goldens under both wasm targets join
  `just ci`, which fails with a pointer to the recipe that installs the tools when one is missing,
  and never skips; the slow wasip1 suite joins `just ci-slow`. There is no hosted CI, so a job
  beside `just ci` would be a gate nobody runs ([open question 12](#open-questions)). The existing
  pin of `libm` to `=0.2.16`, already documented as a generator-version change if bumped, is what
  makes that plausible; the same discipline extends to the surface crate.
- **Fixed-width SIMD is allowed; relaxed SIMD is banned.** WebAssembly's relaxed SIMD proposal
  permits two implementations to return different results for the same instruction, which is
  precisely what the determinism rules forbid. Fixed-width 128-bit SIMD is IEEE-exact and may be
  used, provided the code does not let the compiler reassociate a sum; the rule belongs in
  `.claude/rules/rust-dev.md` next to the existing numeric rules, and the `compile_error!` of
  [Determinism hazards](#determinism-hazards-specific-to-terrain) enforces it.
- **The renderer sits in the app, beside `spatial/`, not inside it.** A new `view/` directory holds
  the engine wrapper, the scene builder and the shaders. What the two share — `vec3`, `frame` and the
  direction conventions — moves to a common place rather than being duplicated or bent. The
  orthographic `Camera` is not generalised into a perspective one: they are different display classes
  and merging them would produce a type that means neither.
- **The engine is loaded lazily.** Consoles that do not draw a scene must not pay for the engine's
  bundle, which a dynamic import and a manual chunk achieve.
- **The scene arrives as a subscription.** The view needs the bodies and craft near the ship, which
  the current protocol cannot express: it has six request kinds and no push. Plan 04 already
  reserved the mechanism — a request whose response carries a `subscription: u32`, pushes as
  `notification`, and binary frames for bulk payloads — so the view is the feature that finally
  builds it. Bodies are on rails, so they travel as orbital elements, on arrival and when Knowledge
  changes, and only craft are pushed at the 64 Hz tick rate. Stars are not in it: the sky arrives
  once per arrival, by a request of its own. **Lean:** JSON for the scene, as the single-player
  brainstorm concluded: at some 250 to 300 bytes a body and 400 a craft, that is about 0.25 MB/s,
  against some 2 MB/s if everything were pushed every tick.
- **Bulk payloads travel as binary frames.** The coarse field, 2 to 15 MB a planet, goes in
  surveyed-region chunks of at most 1 MB, the first use of plan 04's reservation. Sent as base64 in
  JSON, the way the density map travels today, a 15 MB field would become a 20 MB frame: over the
  server's 16 MiB outbound budget, past its 10 s write timeout on a slow link, and holding every scene
  push behind it. The sky's list of about 9,000 stars, some 2 MB as JSON, can stay JSON. Plan 04
  requires new kinds to enter its table of reserved kinds first, so the scene topic, the sky request
  and the coarse-field chunk go there before any is built.
- **Workers hand heights to the render thread.** A `GPUDevice` cannot be shared between threads, so
  height workers transfer their results to the thread that owns the device, or the whole renderer
  runs in a worker on an `OffscreenCanvas`. Each worker runs its own WebAssembly instance, which
  avoids `SharedArrayBuffer` and the cross-origin isolation it needs — isolation that pages loaded
  with `loadFile`, as the client's are, cannot declare.

The renderer is a **display sink**: it is handed a scene and draws it, and it holds no truth the
server has not sent. This is not an aesthetic preference. It is what keeps the Knowledge overlay
honest, and it is also what makes a native renderer a possible future rather than a rewrite — see
[If the browser cannot carry it](#if-the-browser-cannot-carry-it).

## Testing

A renderer is where test discipline usually collapses, because the output is a picture. Most of this
one is testable anyway, because most of it is arithmetic.

**Without a GPU, and automatic:**

- **Projection and camera.** The perspective projection, the camera-relative differencing, the
  rotation-only view matrix and the frame-change rebasing are pure functions over `f64` and `f32`.
  The existing `spatial/` code is the precedent and the standard to match.
- **Level-of-detail selection.** Which patches a given camera position and field of view select, and
  that the set is a function of position alone — not of history, not of arrival order. A wrong answer
  here is a visible pop, and it is entirely testable on the CPU.
- **Culling.** Frustum and horizon culling as predicates, including the cases that catch people out:
  a patch larger than the frustum, a camera inside a patch's bounding volume, and the horizon test
  at grazing altitude.
- **Precision, as arithmetic.** That differencing in `f64` then narrowing keeps a 1 cm feature
  distinct at a planetary radius, asserted numerically rather than by eye. This is the test that
  would have caught the naive implementation.
- **Exposure and photometry.** Magnitude to luminance, luminance to display value, and the exposure
  triple, each against hand-computed values with a cited reference.
- **The height function's determinism**, which is the important one: golden height files asserted
  equal on native, `wasm32-wasip1` and `wasm32-unknown-unknown`, under the
  [determinism rules](../../../.claude/rules/rust-dev.md) the sim already follows, in `just ci`
  as [open question 12](#open-questions) settles. The golden loader reads files with `std::fs`,
  which `wasm32-unknown-unknown` does not have, so it gains an arm that embeds them with
  `include_str!`, and the golden tests carry the `wasm_bindgen_test` attribute on that target.
- **Collision agrees with what is drawn.** The collision query and the finest level's mesh must
  return the same height, and each coarser level must stay inside the stated bound for its level.
  This is the test that makes "the same terrain on both sides" a checkable claim instead of an
  intention.

**With a GPU, by hand, and recorded:**

- **The precision scene.** A hull plate at 1 m, a moon at 10⁸ m and a planet at 1 au in one frame,
  with no depth fighting and no jitter as the camera translates and rotates. The single-player
  brainstorm already names this test; it belongs in a scene that is kept, not a throwaway.
- **The frame-change scene.** Crossing a system and a body boundary while watching a body's projected
  position, asserting continuity to within a pixel.
- **The descent.** Orbit to a metre above the ground in one continuous motion, watching for pops,
  cracks between levels of detail, and the moment streaming cannot keep up.
- **The performance runs** of [the budget](#performance-budget), on both GPUs, recorded with their
  settings so that a regression is visible as a number.

**Golden images are rejected for CI.** They differ across drivers, across Mesa versions and between
software and hardware rasterisation, and a test that fails for reasons unrelated to the change is
worse than no test. Headless software rendering exists — SwiftShader through ANGLE, or Mesa's
lavapipe — and is fine for a smoke test that answers "did every shader compile and did a frame
complete", which is worth having and is all it is worth having. Image comparison stays a local,
deliberate act with a human looking at it.

## Decisions

Settled with the project owner on 2026-09-22:

- **The performance floor.** A modern discrete GPU is the design target at 1080p60, and every rendering
  feature must have a documented low setting that stays playable — not necessarily at 60 fps — on the
  development machine's Intel UHD 620. The real renderer must always be runnable locally.
- **WebGPU is required.** Electron's main process sets the switches needed to force it on hardware
  Chromium blocklists, and no WebGL2 fallback is maintained. Compute shaders may therefore be assumed.
- **This document's scope.** It covers the engine, the real-scale view, and planet rendering including
  the deterministic surface-generation architecture that rendering depends on. Planet content — biomes as
  habitats, life, resources, what a landing party finds — remains for a later planets document. This
  partly answers the single-player brainstorm's open question 7.

Inherited from the earlier brainstorms and unchanged:

- **The realism ruling applies**: where a choice is open, the most realistic answer wins unless it is
  technically infeasible.
- **The view starts as a wireframe**, and full 3D planets follow, as in No Man's Sky but at real scale.
- **Travel within a system is open**, the whole of a system's space including its star.

Recommended here, as technical choices rather than rulings:

- Babylon.js in the renderer, **behind an adapter** that keeps every HYPERION-specific mechanism
  engine-agnostic, which is what makes the choice reversible, with its large-world feature off.
- Reversed-Z with a floating-point depth buffer and an infinite far plane; no logarithmic depth.
- Camera-relative rendering with per-patch `f64` origins, from the frames the simulation already defines.
- Absolute photometric units throughout, with a photographic exposure model and AgX tone mapping.
- A cube-sphere quadtree with fixed-grid patches and vertex morphing.
- Hillaire 2020 atmospheres from a list of physically parameterised terms, with thick atmospheres
  validated against a path tracer.
- A coarse global field at cube-sphere level 7 or 8, computed **on the server** and sent to the
  client as binary frames, with all fine detail synthesised locally; Knowledge gates its coverage but
  never its accuracy. Erosion runs in the coarse pass, and a collision band limit of 1 m covers
  terrain and scatter alike.
- A sky request of its own, magnitude-limited at 6.5, fed by plan 06's stellar brief.
- A new `hyperion-surface` crate, with the sim's `math`, `rng` and `units` moved into a crate beneath
  it and the sim, and both wasm targets brought into `just ci` under Electron's own V8, before any
  terrain code.
- A low setting targeting 30 fps at 720p on the UHD 620, against a discrete reference of the RTX 4060
  class.

The edits this document asks of `docs/frontend/ux-guidelines.md`, collected under
[What the guide must gain](#what-the-guide-must-gain), are **proposals for the owner**, since guide
additions are the owner's call.

## Open questions

Two of these are closed by the research recorded above and kept, with their answers, so that the
numbers cited elsewhere stay put.

1. **What Babylon.js's "Large World Rendering" does.** **Closed:** camera-relative rendering, through
   64-bit CPU matrices and a global override that subtracts the eye position from uniforms matched
   by name, in one 64-bit frame with no rebasing (see
   [The engine](#the-decision-does-not-rest-on-the-engines-large-world-feature)). It stays off; the
   adapter supplies camera-relative transforms itself.
2. **Whether the engine survives the descent spike.** **Lean:** yes. The adapter makes a switch between
   JavaScript engines cheap but not a move to a native renderer, which is one more reason the spike
   comes before anything depends on its answer. The condition for reaching for a native renderer is
   narrow, and stated so that it can be measured: it fires only on the discrete reference machine,
   and only when a native wgpu replay of the same WGSL passes and tile data meets the budget where
   the browser misses it by more than a fifth, or when our CPU time and the GPU's pass time fit with
   headroom while the frames actually delivered miss it. Dawn's robustness and validation toggles are
   compared on and off, to price the browser's safety checks. A failure on the UHD 620 alone
   redesigns the low setting rather than triggering a native renderer.
3. **Whether Hillaire's model holds for thick atmospheres.** The per-planet precompute this question
   once asked about is gone: Hillaire's tables rebuild in under a millisecond. What remains is that
   neither model is validated at Venus's or Titan's optical depths. **Lean:** validate both classes
   against a path-traced reference before they ship, and fall back to Bruneton's tables, with more
   scattering orders, only where Hillaire's colour visibly drifts.
4. **Whether the coarse field is persisted.** **Lean:** not persisted. The server recomputes it and
   caches it keyed by seed, generator version and body. Knowledge records coverage only: for each
   body, the coarse cells surveyed, with the time and the source — orbital or close range — in a new
   versioned JSON-lines record beside plan 12's contacts, at most a 48 KB bitset per body at level 8.
   A save made under another generator version already refuses to open, so surveyed ground cannot
   move under a save; the real risk is changing the coarse pass, its wire quantisation or the local
   synthesis without bumping `GENERATOR_VERSION`, so all three belong to it.
5. **Erosion: where the physics runs.** **Lean:** the stream-power law runs where it can. In the
   coarse pass, Tzathas et al.'s analytical solution produces the coarse elevation, flow directions,
   drainage area and steepness index. Below a coarse cell, a Dendry-style network anchored to those
   flow directions supplies the channels, with stream-power profiles. The network's geometry is a
   heuristic and labelled so; its per-point cost, about 150 µs in the unoptimised reference code, is
   to be measured.
6. **The collision wavelength.** **Lean:** a fixed band limit of 1 m, the scale of a landing-gear
   footpad, changed only with the generator version and stated in the surface crate's documentation
   because both sides depend on it. Collision reads the finest level, which is drawn around every
   grounded body in view. A finer tier, nested inside this one, is added only if crews on foot ever
   touch terrain.
7. **A planet-fixed ocean.** The spherical ocean is published, as a projected grid that follows the
   camera and a reflectance model from space; an ocean on the terrain quadtree, consistent across
   levels and meeting the shoreline, is our own construction. **Lean:** start from the published
   sphere and glint, and treat the quadtree ocean and the shoreline as the risk.
8. **Rings close to.** The criterion is now stated under [Rings](#rings). **Lean:** adopt it, with
   analytic shadowing between particles, and measure the instancing. SpaceEngine's volumetric rings
   run at 150 fps and more at 1080p on an RTX 2080, which suggests the discrete target can afford
   particles and the UHD 620 cannot, so the low setting keeps the annulus.
9. **Climate for worlds that are not Earth-like.** Energy-balance models and Köppen classification are
   Earth-centred; tidally locked "eyeball" worlds, high-obliquity worlds where the equator is the
   coldest place, airless bodies and Titan-like hydrocarbon cycles each break them differently.
   **Lean:** the regime classifier belongs to plan 14's surface conditions, not to this document,
   because the coarse pass is constrained to plan 14's figures and those figures are wrong without
   one. Plan 14's ice fraction, taken from the latitude at which the zonal temperature crosses
   freezing, assumes the poles are coldest and the ice is water, which fails for a locked world's
   nightside ice, for obliquities between 54° and 126°, and for Titan and Pluto, where water ice is
   bedrock; and its equator–pole contrast needs a sign and, for a locked world, the substellar axis.
   The classifier has three fields rather than one list: a thermal regime from Koll's (2022)
   redistribution index on optical depth, surface pressure and equilibrium temperature; a forcing from
   the locking state, the length of the solar day and the obliquity; and a condensable from the
   retained species against their phase diagrams. Each regime names its coarse model — radiative
   equilibrium with thermal inertia for airless and thin-atmosphere worlds, a seasonal or body-fixed
   energy-balance model (Ramirez 2024) for the rest, an isothermal surface for a Venus — and this
   document then says only what each one runs and classifies. The index's thresholds and the onset
   of slow-rotator climates still need checking against the papers.
10. **Whether the scene subscription needs binary frames.** **Closed** for the scene: JSON, with
    bodies as orbital elements and only craft pushed at the tick rate. Bulk payloads, the coarse
    field first, travel as binary frames, as [Runtime and code shape](#runtime-and-code-shape) sets
    out.
11. **Whether scatter is ever collidable.** **Lean:** yes, above the band limit, from the first phase
    that draws it: boulders and trunks are discrete features in hashed cells per size octave, which
    the server answers for. Smaller scatter is decoration, culled where it intersects a grounded
    body.
12. **How the wasm targets join the determinism checks.** **Lean:** `wasm-bindgen-test` in Node mode
    with Electron's own binary as Node, so that the check exercises the V8 the client ships; the fast
    goldens under both wasm targets in `just ci`, which fails and never skips when a tool is missing;
    the slow wasip1 suite in `just ci-slow`; all of it before any terrain code, the descent spike's
    included. What needs a short trial is whether the runner accepts Electron in Node's place, and
    what the extra runs add to `just ci`'s time.
13. **How the sky is selected.** **Lean:** plan 06's stellar brief (P06.T33) supplies luminosity and
    effective temperature, and the sky is a request kind of its own rather than a set of range
    queries. Its census counts stars brighter than magnitude 6.5, not systems. Each mass layer's
    radius comes from the brightest absolute magnitude it reaches in any phase of its life, dimmed by
    the least extinction in any direction and capped — about 2,000 ly for the 0.75–2.5 M☉ layer,
    4,300 for 2.5–8 and 10,000 for 8–150 — and beyond the caps the light belongs to the band. The two
    lightest layers never evolve and need only tens of light-years. Candidates are skipped by mass
    and age, which are drawn on their own streams, before any density is evaluated, and each cell's
    bright subset is cached, so that a jump of up to 1,000 ly reuses most of the cells. That still
    leaves some millions of cell bounds, about 5 s of the server's pool per arrival. The magnitude
    bounds need checking by the science checker, and the saving needs a benchmark.

## Suggested order of attack

Not a plan, only the dependency order a plan would follow. Steps 1 to 3 are the ones that retire risk;
everything after them is additive. Steps 5 to 9 also wait on plan 14, which has not started: they need
its radii, albedos, rotation, compositions and global figures. Steps 1 and 3 do not, because they run
on scenes built by hand — the precision scenes of [Testing](#testing) and a hand-parameterised test
planet — and the wireframe draws generated bodies once plan 14 supplies them. Until plan 12's
retarded-time machinery exists, the scene the server sends is the present state, which within a
system differs from the retarded one by seconds to hours of light-time. Each feature's low setting is
built in that feature's own step rather than at the end, as the budget's third rule requires, and
the first one brings the degraded-rendering data state with it.

1. **Foundations and the wireframe.** The engine adapter, camera-relative differencing, reversed-Z,
   frame-change rebasing, the photometric pipeline with exposure and tone mapping, and `VIEW` as a
   wireframe at real scale with stars at their true magnitudes. Those magnitudes need plan 06's
   stellar brief on the range rows (P06.T33), so this step waits on plan 06's protocol work, whose
   stellar system module is still a stub; until step 4 the stars are the range query's
   volume-limited set, and the view says so. The scene subscription in the protocol.
   The guide edits go to the owner here, since the wireframe already needs the view as a display
   class, the exposure instrument and the motion rule. This is the precision test rig, and the point
   at which the depth and precision tests exist.
2. **The wasm targets in the determinism checks.** `wasm32-unknown-unknown` under `wasm-bindgen-test`,
   both wasm runs made automatic, and the client loading its first WebAssembly. Nothing terrain-shaped
   is built yet, but this is the check the shared terrain rests on, so it comes before the first line
   of terrain code, spike code included.
3. **The descent spike, which is the gate.** An Earth-sized test planet with Earth's reference
   atmosphere, from orbit to a metre above the ground, terrain from sim code in WebAssembly workers
   and the atmosphere drawn every frame, since after terrain it is one of the heaviest passes on the
   Intel part. The descent is scripted and seeded, identical every run, and records frame intervals
   at the 50th, 95th and 99th percentiles, main-thread time split between our code, the engine and
   idle, GPU time per pass — which the forced switches make available, uncoarsened, on Linux —
   tile demand against worker throughput, upload bytes, pipeline-creation stalls and
   garbage-collection pauses. It passes at 1080p60 on the discrete target and at 30 fps at
   720p on the UHD 620's low setting. The project has no discrete GPU today, so that half of the
   measurement needs one borrowed or rented; the UHD 620 half runs on the machine that exists, and a
   failure there is already an answer. The single-player brainstorm's statement of the spike carries
   the same criterion. It decides whether the browser carries the planets, and it should happen
   before anything depends on the answer.
4. **The sky.** The sky request with its magnitude-limited census, faint stars baked and bright or
   near ones drawn as sprites, the galactic band from the model, the local star as a limb-darkened
   disc, blackbody colour. Cheap, highly visible, and it exercises the photometry.
5. **Lit bodies at real scale**, from plan 14's radii, albedos and rotation, with correct phase and the
   terminator. Still no surfaces.
6. **Atmospheres**, parameterised from plan 14's composition, pressure, temperature and gravity, with
   aerial perspective on terrain, and the thick ones checked against a path tracer.
7. **The surface generator.** The shared `math`, `rng` and `units` moved into the crate beneath the
   sim, the `hyperion-surface` crate with its own `clippy.toml`, the coarse global pass in the sim on
   the server, its binary wire chunks with Knowledge's coverage gating and coverage record, and
   golden height files
   added to step 2's parity checks across native and both wasm targets. Nothing is drawn from it yet.
8. **Terrain.** The quadtree, patch streaming, height textures, morphing, materials, the survey
   coverage readout — and the test that collision agrees with the picture.
9. **The rest of the surface**, in rough order of value: GPU decoration with its label on the view,
   scatter, clouds, ocean and glint, rings.
10. **The measurements.** The budget's estimates replaced by figures measured on both GPUs and kept
    under version control, and the ladder's settings adjusted to what they show.

## Sources

Figures above are rounded, and several are explicitly estimates rather than measurements. Everything
here should be re-checked against these when it becomes code, in the same way the galaxy brainstorm's
figures are. Where a source could not be reached, it is marked, because the alternative is a document
that reads as more certain than the research was.

**Engines and the platform** (all verified 2026-09-22 unless marked)

- Babylon.js releases, including the 9.0.0 notes of 26 March 2026 naming Large World Rendering, a
  geospatial camera and 3D Tiles support, each linked to its documentation page (Large World
  Rendering at <https://aka.ms/babylon9LWDoc>); 9.27.1 of 18 September 2026.
  <https://github.com/BabylonJS/Babylon.js/releases>. An earlier draft of this document gave 9.0.0
  the date of 8.0.0, 27 March 2025.
- Babylon.js reversed-Z: `useReverseDepthBuffer` setting `GEQUAL` and `clearDepth(0.0)`, in
  `packages/dev/core/src/Engines/thinEngine.pure.ts`.
- Babylon.js Large World Rendering (verified 2026-09-23): the documentation source
  `content/features/featuresDeepDive/scene/large_world.md` in the BabylonJS/Documentation repository,
  to which <https://aka.ms/babylon9LWDoc> redirects; the override in
  `packages/dev/core/src/Materials/floatingOriginMatrixOverrides.ts` and the eye position in
  `scene.pure.ts`, at 9.27.1; its origin as an experiment in 8.28.3 (pull request 17183); and the
  maintainer's announcement, calling it experimental. <https://forum.babylonjs.com/t/61114>. An
  earlier draft of this document, unable to read the documentation site, called the feature
  unverified.
- Babylon.js compatibility (verified 2026-09-23): golden rule 1 of `contributing.md` in the engine
  repository, and `content/breaking-changes.md` in the documentation repository, which records visual
  changes such as PBR rough metals in 7.45.0, each with a flag to restore the old look. An earlier
  draft of this document found no policy.
- three.js r186 of 8 September 2026. <https://threejs.org/> and
  <https://github.com/mrdoob/three.js/releases>
- three.js migration guide, recording breaking changes in essentially every release and advising upgrades
  in increments of ten. <https://github.com/mrdoob/three.js/wiki/Migration-Guide>
- three.js `WebGPUBackend.js` (`reversedDepthBuffer` and `logarithmicDepthBuffer` as independent
  options) and `logdepthbuf_fragment.glsl.js` (writes `gl_FragDepth`, so early depth rejection is lost).
  Whether the two options may be combined is **unverified**.
- PlayCanvas 2.22.3 of 21 September 2026. <https://github.com/playcanvas/engine/releases>
- WebGPU implementation status: Linux default enablement for Intel Gen12+ from Chrome 144 and NVIDIA
  under Wayland from Chrome 147, everything else behind a flag.
  <https://github.com/gpuweb/gpuweb/wiki/Implementation-Status>
- Chromium's `software_rendering_list.json`, entry 186, blocking WebGPU's Vulkan-through-GL-interop
  display path on Linux except for Intel Gen12+ with Mesa 22.0 or newer and NVIDIA 535.183.01 or
  newer, with no Wayland condition; timestamp coarsening lifted by `--enable-unsafe-webgpu` in
  `webgpu_decoder_impl.cc`; and WebGPU exposed to workers (`WorkerNavigator includes NavigatorGPU`).
  Read on Chromium's main branch (verified 2026-09-23), **not the 152 branch**.
- Dawn's Vulkan backend, `PhysicalDeviceVk.cpp`: subgroups refused on Gen9 unless the
  `enable_subgroups_intel_gen9` toggle is set (verified 2026-09-23).
  <https://dawn.googlesource.com/dawn>
- WebGPU on Mesa's ANV driver, and the switch set including `DefaultANGLEVulkan` that avoids a
  swap-chain hang. <https://github.com/gpuweb/gpuweb/issues/5022>
- Electron 44 bundling Chromium 152. <https://github.com/electron/electron/releases>
- W3C WebGPU specification: depth range, `depth32float`, `depthCompare`, and the absence of a
  tessellation stage. <https://www.w3.org/TR/webgpu/>
- Khronos WebGL extension registry, `EXT_clip_control`: extension 51, written against WebGL 1.0 and
  promoted to Community Approved on 2 November 2023.
  <https://registry.khronos.org/webgl/extensions/EXT_clip_control/>. An earlier draft of this
  document, relying on MDN's WebGL page, said the extension did not exist. Whether Chromium 152
  exposes it on ANV is **unverified**.
- WebAssembly relaxed SIMD, whose instructions may return different results on different hardware.
  <https://github.com/WebAssembly/relaxed-simd>
- Electron multithreading, for workers and `nodeIntegrationInWorker`.
  <https://www.electronjs.org/docs/latest/tutorial/multithreading>
- Chromium SwiftShader, for headless software rendering.
  <https://chromium.googlesource.com/chromium/src/+/main/docs/gpu/swiftshader.md>
- Bevy 0.19.1 of 13 August 2026; its atmosphere settings, whose four lookup tables are Hillaire's,
  and its atmosphere module, whose documentation states that it implements Hillaire 2020 (an earlier
  draft of this document called it Bruneton-style); its medium in `crates/bevy_light/src/atmosphere.rs`,
  any number of terms each with its own density and phase function, and a Mars preset whose dust
  follows Schneegans et al. 2024 (doi:10.1111/cgf.15010, **not read**: it returned 403); and the
  `big_space` crate for large worlds.
  <https://github.com/bevyengine/bevy/releases>, <https://docs.rs/bevy/0.19.1/bevy/pbr/struct.AtmosphereSettings.html>,
  <https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_pbr/src/atmosphere/mod.rs>,
  <https://github.com/aevyrie/big_space>
- Godot's large world coordinates, converting vectors and physics but leaving shaders single precision.
  <https://docs.godotengine.org/en/stable/tutorials/physics/large_world_coordinates.html>
- wgpu 30.0.1. <https://github.com/gfx-rs/wgpu/releases>
- Hardware figures for the budget: the UHD 620's 24 execution units at about 1.1 GHz, the RTX 4060
  class's roughly 15 TFLOP/s and 272 GB/s, and the PlayStation 4's 1.84 TFLOP/s. **Cited from memory
  and not re-checked**; they set the scale of estimates, not any measured figure.

**Precision and depth**

- Matt Pharr, _Rendering in Camera Space_, 2018.
  <https://pharr.org/matt/blog/2018/03/02/rendering-in-camera-space>
- Bruce Dawson, _Don't Store That in a Float_, 2012, for precision by magnitude.
  <https://randomascii.wordpress.com/2012/02/13/dont-store-that-in-a-float/>
- Cesium's `EncodedCartesian3.js`, the two-float encoding of a `f64` position for shader use.
  <https://github.com/CesiumGS/cesium>
- Cesium, _Logarithmic Depth Buffer_, 2018, including the finding that one logarithmic frustum still
  fights at extreme range — two surfaces 300 m apart seen from 6.4 × 10⁷ m — which is why Cesium combines
  multiple frusta with logarithmic depth. It does not discuss reversed-Z.
  <https://cesium.com/blog/2018/05/24/logarithmic-depth/>
- Outerra, _Maximizing Depth Buffer Range and Precision_, November 2012, with measured costs for
  fragment-written depth. <https://outerra.blogspot.com/2012/11/maximizing-depth-buffer-range-and.html>
- Nathan Reed, _Depth Precision Visualized_ (NVIDIA), the origin of the reversed-Z error figures.
  **Unverified**: the page 404s; the figures are cited here through Outerra.

**Terrain geometry and level of detail**

- Musgrave, Kolb and Mace 1989, _The synthesis and rendering of eroded fractal terrains_, SIGGRAPH — the
  summed-octave construction that makes a coarse evaluation a low-pass of a fine one.
- Strugar 2009, _Continuous Distance-Dependent Level of Detail for Rendering Heightmaps_ (CDLOD).
  **Unverified**: the original site is defunct; the technique is widely cited.
- Losasso and Hoppe 2004, _Geometry clipmaps: terrain rendering using nested regular grids_.
  **Unverified** fetch; widely documented.
- Dimitrijević, Lambers et al. 2016, _Comparison of spherical cube map projections used in planet-sized
  terrain rendering_.
- Westerteiger et al. 2012, _Spherical Terrain Rendering using the hierarchical HEALPix grid_.
- Scholz, Bender and Dachsbacher 2013, _Level of Detail for Real-Time Volumetric Terrain Rendering_;
  Dachsbacher 2006, _Interactive terrain rendering: towards realism with procedural models and graphics
  hardware_.

**Surface generation**

- Cortial, Peytavie, Galin and Guérin 2019, _Procedural Tectonic Planets_, Computer Graphics Forum.
- Renka 1997, _Algorithm 772: STRIPACK, Delaunay triangulation and Voronoi diagram on the surface of a
  sphere_, ACM TOMS — written for a model of plate tectonics.
- Tzathas, Gailleton, Steer et al. 2024, _Physically-based analytical erosion for fast terrain
  generation_, Computer Graphics Forum — closed-form in time but evaluated numerically over space,
  with receivers, a topological sort, an upstream pass and multigrid fixed-point iteration; 1.8 s at
  512² and 8.2 s at 1024² in Python. Read from the authors' PDF (verified 2026-09-23).
  <https://www-sop.inria.fr/reves/Basilic/2024/TGSC24/Analytical_Terrains_EG.pdf>. An earlier
  draft of this document called it a closed-form substitute that could be evaluated per point.
- Gaillard, Benes, Guérin, Galin and Peytavie 2019, _Dendry: A Procedural Model for Dendritic
  Patterns_, I3D (doi:10.1145/3306131.3317020; **the paper returned 403**), read through its
  reference code, which seeds each cell from integer coordinates but draws with `mt19937_64` and
  standard-library distributions. <https://github.com/mgaillard/Noise>
- NASA TN D-6850, _Apollo Experience Report: Lunar Module Landing Gear Subsystem_, 1972: a footpad
  about 0.9 m across, and 0.6 m of relief tolerated within the footprint.
  <https://ntrs.nasa.gov/citations/19720018253>
- Schott, Paris, Fournier, Guérin et al. 2023, _Large-scale terrain authoring through interactive erosion
  simulation_, ACM TOG; and 2024, _Terrain Amplification using Multi Scale Erosion_, which names the
  boundary problems tiled erosion runs into.
- Génevaux et al. 2013, _Terrain generation using procedural models based on hydrology_, ACM TOG;
  Teoh 2009, _Riverland_; Derzapf, Ganster, Guthe and Klein 2011, _River networks for instant
  procedural planets_, Computer Graphics Forum — the network-first inversion. Derzapf et al. is
  closed access and was read only in abstract.
- Guérin et al. 2016, _Sparse representation of terrains for procedural modeling_; Guérin et al. 2017,
  _Interactive example-based terrain authoring with conditional generative adversarial networks_, ACM TOG;
  Grenier et al. 2024, _Real-time Terrain Enhancement with Controlled Procedural Patterns_, reporting
  amplification up to thirty-two times — the amplification line of work.
- Paris, Galin, Peytavie, Guérin and Gain 2019, _Terrain Amplification with Implicit 3D Features_, ACM TOG.
- Argudo, Galin, Peytavie, Paris and Gain 2019, _Orometry-based terrain analysis and synthesis_, ACM TOG.
- Tucker and Whipple 2002, JGR; Harel, Mudd and Attal 2016, _Geomorphology_ — the stream-power law and its
  exponents. Flint's law and Hack's law, for channel slope and drainage area, are **cited from
  memory**, and Hack's constants (C = 1.5, h = 0.6) through Tzathas et al.
- Lagae et al. 2009, on sparse convolution noise, and Worley 1996, on cellular noise — the
  per-cell, neighbourhood-searched construction the crater step uses. **Cited from memory.**
- Zafar, Olano and Curtis 2010, _GPU random numbers via the tiny encryption algorithm_, HPG — hash-based
  noise that depends on nothing but its inputs.
- FastNoise2, for SIMD noise, and its warning that a compiler's SIMD bugs can change generated output.
  <https://github.com/Auburns/FastNoise2>
- Arteaga, Fuhrer and Hoefler 2014, _Designing bit-reproducible portable high-performance applications_,
  IPDPS; Sawaya, Bentley, Briggs et al. 2017, _FLiT_ — cross-platform floating-point reproducibility,
  including fused multiply-add and library functions as the principal hazards.

**Climate, biomes and craters**

- Ramirez 2024, _A new 2D energy balance model for simulating the climates of rapidly and slowly rotating
  terrestrial planets_, Planetary Science Journal.
- North, Cahalan and Coakley 1981, _Energy balance climate models_, Reviews of Geophysics; North and
  Kim 2017, _Energy Balance Climate Models_ — the latter cautioning that precipitation cannot be solved by
  simple models.
- Siler, Roe and Armour 2018, Journal of Climate; O'Gorman, Allan, Byrne and Previdi 2012, Surveys in
  Geophysics — precipitation from energy budgets.
- Roe 2005, _Orographic precipitation_, Annual Review of Earth and Planetary Sciences; Roe and Baker 2006 —
  rain shadows.
- Köppen–Geiger, Holdridge life zones and the Whittaker diagram, as candidate biome classifications.
- Neukum, Ivanov and Hartmann 2001, _Cratering records in the inner solar system in relation to the lunar
  reference system_, Space Science Reviews; Michael and Neukum 2010, EPSL, for fitting the cumulative
  production function; Croft 1985, JGR, and Krüger and Hergarten 2018, JGR, for the simple-to-complex
  transition diameter. Specific transition diameters were **not extracted** and must come from these
  papers directly.
- Checlair, Menou and Abbot 2017, ApJ (tidally locked "eyeball" states, with no snowball
  bifurcation), and Checlair et al. 2019, finding that ocean heat transport does not restore it;
  Ferreira, Marshall, O'Gorman and Seager 2014, Icarus (at 90° obliquity the equator is the coldest
  region; only its bibliography was confirmed); Williams and Kasting 1997, Icarus; Lunine and Atreya
  2008 and Hayes, Lorenz and Lunine 2018, Nature Geoscience (Titan's methane cycle).
- Koll 2022, ApJ 924 (arXiv:1907.13145): the day–night redistribution index for locked rocky planets,
  in optical depth, surface pressure and equilibrium temperature. Wordsworth 2015
  (arXiv:1412.5575): the pressure below which carbon dioxide collapses on a locked world's night side.
  Yang et al. 2014, ApJL 787 L2: slow rotators stay temperate at nearly twice the flux. Kilic et al.
  2018, ApJ 864, 106: stable equatorial ice belts at high obliquity. Barnes et al. 2025, the FILLET
  protocol (arXiv:2511.11957), naming four energy-balance climate states by their ice edges. All
  verified 2026-09-23. The 54° obliquity threshold, after Ward 1974 and Williams and Kasting 1997, is
  **cited from memory**.

**Atmosphere, clouds, ocean and imaging**

- Bruneton and Neyret 2008, and Bruneton's 2017 revision with its reference implementation, whose
  defaults are a 256 × 64 transmittance table, a 32 × 128 × 32 × 8 scattering table, a 64 × 16
  irradiance table and four scattering orders, with density profiles of at most two layers; its WebGL
  demo loads tables precomputed offline. <https://ebruneton.github.io/precomputed_atmospheric_scattering/>
- Hillaire 2020, _A Scalable and Production Ready Sky and Atmosphere Rendering Technique_, EGSR, Table
  2 and section 7: 0.31 ms for all four tables on a GTX 1080 at 720p and 0.5 ms with per-pixel ray
  marching from space, the per-planet tables under a millisecond on an iPhone 6s, and 250 ms for
  Bruneton's update on the same GTX 1080; figures 11 and 12 for both models' behaviour at high
  optical depth (verified 2026-09-23). <https://sebh.github.io/publications/egsr2020.pdf>. An earlier
  draft of this document could read only the abstract.
- Bruneton, Neyret and Holzschuch 2010, _Real-time Realistic Ocean Lighting using Seamless Transitions
  from Geometry to BRDF_, Computer Graphics Forum, section 6 on planet-scale rendering.
  <https://inria.hal.science/inria-00443630>. Proland's ocean module, drawing flat or spherical
  oceans. <https://github.com/csbrandt/proland-4.0>. Scatterer's port for Kerbal Space Program.
  <https://github.com/LGhassen/Scatterer>. Outerra, _Ocean rendering_, 18 February 2011.
  <https://outerra.blogspot.com/2011/02/ocean-rendering.html>. An earlier draft of this document said
  the published work was all flat-patch.
- Rings: Björn Jónsson's ring model and radial profiles, <http://mmedia.is/bjj/data/s_rings/>; Salo
  and French 2010 (arXiv:1007.0349), on shadowing between particles and the opposition surge; Déau
  et al. 2009 (arXiv:0902.0289), on the surge from 0.001° to 25° phase; SpaceEngine's volumetric
  rings, <https://spaceengine.org/news/blog210611>; ring thicknesses and optical depths from
  Wikipedia's _Rings of Saturn_, a secondary source. Dones et al. 1993, on main-ring particles
  backscattering, and Zebker et al. 1985, on the size distribution, are **cited from memory**. An
  earlier draft of this document called the main rings forward-scattering.
- Nishita et al. 1993; O'Neil, _Accurate Atmospheric Scattering_, GPU Gems 2 chapter 16.
  <https://developer.nvidia.com/gpugems/gpugems2/part-ii-shading-lighting-and-shadows/chapter-16-accurate-atmospheric-scattering>
- Rayleigh and Mie coefficients, scale heights and the Cornette–Shanks phase function, from Scratchapixel's
  _Simulating the Colors of the Sky_ and Zucconi's _Atmospheric Scattering_. **These two differ** on the
  red-end Rayleigh coefficient (3.8 × 10⁻⁶ against 5.8 × 10⁻⁶ m⁻¹), possibly through different choices of
  red wavelength, so the constants must be derived from the formula at stated wavelengths with a cited
  source rather than copied.
- Schneider and Vos 2015, _The Real-time Volumetric Cloudscapes of Horizon Zero Dawn_ (**PDF unreadable**);
  Guerrilla, _Nubis: Authoring Real-Time Volumetric Cloudscapes with the Decima Engine_, 2017, the source of
  the under-2 ms figure on PlayStation 4.
  <https://www.guerrilla-games.com/read/nubis-authoring-real-time-volumetric-cloudscapes-with-the-decima-engine>
- Finch, _Effective Water Simulation from Physical Models_, GPU Gems chapter 1, for Gerstner waves and the
  steepness limit beyond which normals invert.
  <https://developer.nvidia.com/gpugems/gpugems/part-i-natural-effects/chapter-1-effective-water-simulation-physical-models>
- Tessendorf 2004, _Simulating Ocean Water_, SIGGRAPH course notes, for spectral ocean surfaces.
  **PDF unreadable**; cited bibliographically.
- The exposure model — exposure value from average luminance, the reflected-light calibration constant of
  12.5 and the lens factor of 0.65, giving a white point near 9.6 times average luminance — and the GPU
  histogram. <https://bruop.github.io/exposure/>. It derives from Lagarde and de Rousiers 2014,
  _Moving Frostbite to Physically Based Rendering_, **cited from memory and not re-checked**.
- Tone mapping operators, and Blender's colour-management configuration as evidence of AgX's adoption and
  its roughly 25-stop range. <https://64.github.io/tonemapping/> and
  <https://github.com/blender/blender/blob/main/release/datafiles/colormanagement/config.ocio>
- Limb darkening, with the polynomial law and solar coefficients at 550 nm of 0.3, 0.93 and −0.23, giving a
  limb at 30% of centre and a disc average of about 80%. Via
  <https://en.wikipedia.org/wiki/Limb_darkening>, which cites Cox 2000, _Allen's Astrophysical Quantities_ —
  **the primary source was not read**.
- Walker, _Colour Rendering of Spectra_, for blackbody to RGB through Planck's law, the CIE matching
  functions and gamut clamping. <https://www.fourmilab.ch/documents/specrend/>
