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
seat: a perspective view that is true to scale from a hull plate a metre away to a star system an
astronomical unit across, and eventually a planet flown from orbit to a landing without a seam or a
loading screen. The rendering must be defensible in the same way the simulation is: what is drawn is
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
range in _position_ is about 10¹⁶ between a hull rivet and an outer planet, and in _luminance_ about
10¹² between a dim star and the disc of the star itself, with a sunlit surface, at about
10⁴ cd/m², in between. Neither fits in a 32-bit float, and both have standard answers. They are in
[Real-scale foundations](#real-scale-foundations).

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

### Correcting the prior lean's reasoning

The single-player brainstorm leaned Babylon.js on two grounds: that it shipped large-world rendering
with a floating origin in 9.0, and that it commits to backward compatibility. Both were checked
directly, and the picture is more complicated:

- **Reversed-Z is not a differentiator.** Both engines have it. Babylon.js exposes
  `useReverseDepthBuffer`, which sets the depth function to `GEQUAL` and clears depth to 0 (verified
  in `Engines/thinEngine.pure.ts`). three.js exposes `reversedDepthBuffer` and
  `logarithmicDepthBuffer` as independent options on its WebGPU backend (verified in
  `WebGPUBackend.js`). The prior document's implicit contrast here does not exist.
- **"Large World Rendering" exists as a feature name and its content could not be verified.** It is
  listed in the Babylon.js 9.0.0 release notes of 26 March 2026, beside a geospatial camera and 3D
  Tiles support — a trio that shows the maintainers investing in globe-scale rendering, which is
  encouraging. The notes link its page (<https://aka.ms/babylon9LWDoc>), but `doc.babylonjs.com`
  serves its content through a client-side application that automated fetching cannot read, and
  nothing in the camera source or the changelog shows camera-relative rebasing for rendering. What
  is verifiable is a physics-side floating origin for Havok, whose own author notes it has cases
  where bodies in separate regions fail to interact. **The claim should not be leaned on until a
  human has read that page.**
- **The backward-compatibility commitment could not be verified at all.** No policy statement was
  found in the repository or the accessible release material. Meanwhile three.js's breaking changes
  are documented and routine: its migration guide records changes in essentially every release, the
  official advice is to upgrade in increments of ten because deprecations last that long, and some
  changes are _silent visual_ ones — physically-based brightness shifted in r181, an ambient
  occlusion effect darkened in r185.

That last point deserves weight beyond its size, because of what this renderer is for. A display
calibrated in absolute photometric units, whose numbers a console will state, cannot afford a
dependency that changes what a given radiance looks like between releases without saying so. In a
game that would be a nuisance. Here it silently falsifies an instrument.

### The decision does not rest on the engine's large-world feature

The more important realisation is that **the floating origin is ours whatever we choose.** It has to
be: it must agree with the frames `hyperion_sim::coords` already defines, change when the simulation
changes frame, and be unit-tested against the same `f64` arithmetic the server uses. That is a few
hundred lines of our own code — a differencing step, a per-patch origin, a rotation-only view matrix
— and an engine's opaque large-world system is as likely to fight it as to help. The same is true of
the depth policy and the exposure model.

So the engine is being hired for the ordinary parts: resource and state management, a render graph, a
shader pipeline, a material system, culling, and the tedious correctness of a WebGPU backend across
drivers. It is explicitly _not_ being hired to solve real scale.

| Option                         | For                                                                                                                                                                                        | Against                                                                                                                                                                |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Babylon.js 9.27**            | TypeScript-first, which suits a TS 7 workspace with type-aware lint. Weekly patch cadence and no visible migration guides. Reversed-Z present. Active investment in globe-scale rendering. | A quarter of three.js's community. Compatibility policy unverified. Documentation opaque to tooling. Larger package.                                                   |
| **three.js r186**              | The largest ecosystem by a factor of four, and the most published procedural-planet work. Reversed-Z and logarithmic depth both exposed on the WebGPU backend. Small package.              | Breaking changes in essentially every release, including silent visual ones. Upgrades must be taken in small steps forever. `react-three-fiber` pins to a React major. |
| **PlayCanvas 2.22**            | A mature WebGPU implementation with compute shaders, MIT-licensed.                                                                                                                         | Editor-centred workflow that a code-first console app would fight. Smallest community of the three.                                                                    |
| **Raw WebGPU, or wgpu → wasm** | Total control, and no engine to track.                                                                                                                                                     | Everything above becomes ours: culling, materials, resource lifetimes, driver workarounds. Months of work whose output is not gameplay.                                |

**Lean: Babylon.js, confirming the prior document's choice but not its reasoning.** The grounds are
narrower than before and should be stated as they are. TypeScript fit is verified. The case against
three.js is verified too: its churn is documented and includes silent visual changes. The case _for_
Babylon.js's stability is not: weekly patch releases and the absence of migration guides are
consistent with a compatibility commitment but do not establish one, and treating them as "release
discipline" would be the prior document's unverified claim under another name. The lean therefore
rests on TypeScript fit and on three.js's documented cost, not on an unverified large-world feature
— and on the understanding below that makes the choice cheap to reverse.

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
engine's scene graph, and it buys the reversibility that the unverifiable compatibility policy would
otherwise deny us.

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
and later (from Chrome 144) and for NVIDIA under Wayland with a driver of 535.183.01 or newer (from
Chrome 147). Everything else — including AMD, and including this machine's Gen9.5 UHD 620 — is behind
a flag with no announced date. The switch set reported to work with Mesa's ANV driver is
`--enable-unsafe-webgpu --use-angle=vulkan --enable-features=Vulkan,VulkanFromANGLE,DefaultANGLEVulkan`,
where `DefaultANGLEVulkan` is the one that avoids a hang in swap-chain acquisition on ANV. Electron
sets these from the main process.

Three consequences, which should be written down rather than discovered:

1. **Forcing WebGPU bypasses Chromium's GPU blocklist**, which exists because some driver and
   hardware combinations genuinely break. The client should therefore detect an adapter failure and
   report it as a ship system fault in the guide's language, not crash or silently fall back.
2. **The switches are a distribution problem, not just a development one.** Any Linux machine that
   is not Gen12 Intel or Wayland NVIDIA gets the forced path, so on Linux the forced path is the
   _normal_ path and must be the one that is tested. Windows and macOS enable WebGPU by default and
   should not be given the Linux switches, which name Linux's Vulkan path.
3. **Compute shaders are therefore available everywhere**, which is what makes the WebGPU-only ruling
   valuable: terrain, cloud and histogram passes can assume compute rather than emulating it in
   fragment shaders. That assumption should be used deliberately, because it is the whole return on
   the ruling.

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
sky-view and aerial-perspective lookup tables, which is this document's reserve model rather than its
lean, so a native renderer would either accept Hillaire or port Bruneton; its transforms are `f32`,
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
pre-build for it. The condition that would trigger it is narrow and measurable: the descent spike
fails on adequate hardware for reasons in the browser stack rather than in our own code.

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
  own size, which runs from metres at the finest level to thousands of kilometres at a root face,
  where the patch is seen from far enough away that the coarser precision is invisible. The
  precision tracks the level, which is what makes it enough at every scale.
- **The view matrix must not contain the translation.** Rotation-only view matrices, with
  translation folded into the per-object offset, keep the large numbers out of the matrix product
  entirely. Composing a rotation with a translation of 10¹¹ m in `f32` throws away the rotation.

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
zonal temperature crosses freezing) and cloud fraction; relief scaled as 20 km × (g⊕ ÷ g) × a
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
writing out, because it is easy to get wrong by orders of magnitude. An Earth-sized body has
5.1 × 10⁸ km² of surface: sampled at 100 km that is about 5 × 10⁴ cells, and at 10 km about 5 × 10⁶.
At a few tens of bytes per cell — elevation, plate identity, crustal type, temperature,
precipitation, drainage area, biome — the fine end is a hundred megabytes or more, which is neither a
cheap transfer nor a cheap simulation. **Lean:** level 7 or 8 of the cube-sphere quadtree, cells of
about 70 or 35 km, which is 98,304 or 393,216 cells and **roughly 2 to 15 MB** for an Earth. The
simulation that produces it is then of order 10⁷ to 10⁸ cell updates across its iterations, a
fraction of a second of Rust rather than minutes. Both figures need measuring rather than trusting,
but they are the right order for "computed when a ship arrives". Everything finer than a coarse cell
is the local synthesis's job.

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
   fraction rather than chosen by eye.
3. **Climate.** A two-dimensional energy-balance model gives temperature; there is one published
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
4. **Drainage.** Flow directions and accumulated drainage area on the coarse grid. This is the step
   that buys the realism, because it is computed globally where global is affordable, and everything
   local is then conditioned on it.
5. **Biomes**, from temperature and precipitation. Köppen–Geiger is the recommendation: it is
   temperature-and-precipitation driven, recognisable, and defensible in a way a hand-drawn biome map
   is not.
6. **Crater state**, from plan 14's crater density rather than recomputed from surface age, since
   that density already carries the belt-mass scaling and the loss of small craters under a thick
   atmosphere. The pass adds a saturation level and places the largest basins, which are coarse-scale
   features; the local pass turns the rest into actual craters. Mars, under a thin atmosphere, is as
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
cells, and an unsurveyed one does not arrive at all. A degraded copy — smoothed, quantised harder, or
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
4. **Erosional detail** conditioned on the coarse drainage area and slope. The physical statement to
   respect is the stream-power law, incision going as a power of drainage area and slope; there is
   also a published analytical erosion that substitutes a closed form for the iterative simulation,
   which is the most promising route to doing this properly rather than by analogy. **Lean:** start
   with drainage-conditioned noise, which is a heuristic and must be labelled one, and treat the
   analytical route as the upgrade that would make it physics.
5. **Craters**, wherever plan 14's crater density is not zero, evaluated per point: hash the cell,
   draw a crater count from that density, and for each crater draw a diameter by inverting the
   production function's cumulative size–frequency distribution, truncated below the smallest crater
   the atmosphere lets through, then a jittered position and a morphology by diameter — simple bowl,
   complex with a central peak, or multi-ring basin, at transitions the literature provides. Summing
   the nearby craters' profiles is O(craters within the search radius), which is why the crater
   field is baked into the patch's height texture rather than evaluated per frame.
6. **Band-limit.** Sum only those octaves the current level of detail can resolve.

That last step is not a performance trick. It is what makes collision agree with the picture, and it
has its own section below.

### The line between truth and decoration

GPU decoration is how a planet gets pebbles without the server knowing about pebbles, and the line has
to be drawn explicitly because safety in the fiction depends on it. Two words are kept apart from here
on: _amplification_ is the authoritative local synthesis above, run on both sides, and _decoration_
is what only the GPU draws.

**Lean:** the authoritative height function — sim code, run identically on both sides — owns everything
the ship can collide with, down to a stated wavelength. Call it a metre initially, to be tuned by what
the flight model can actually touch. Everything finer is **GPU-only decoration**: high-frequency
normal detail, sand ripples and small crater scars. It never displaces geometry the collision query
does not know about, so the surface a hull touches is always the surface both sides computed. Because
decoration is not the simulation's, the view says when it is on, as
[the guide's edits](#what-the-guide-must-gain) require. **Scatter** — rocks and vegetation as
instances — is a third category: placed deterministically, so that both sides could know where every
instance is, but decoration and not collidable at first ([open question 11](#open-questions)).

Two consequences worth stating. Normals should come from **analytic derivatives** of the height
function rather than finite differences: there is no arbitrary epsilon to tune, and they stay stable
across levels of detail, which is what stops shading from popping as patches subdivide. And scatter
placement must be hashed from integer cell coordinates with an integer generator even while scatter
is decoration, so that making it collidable later is a change of contract rather than of placement:
the server can then answer "is there a boulder here" without storing one.

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
appear at the level whose resolution can represent its diameter, and its profile must integrate to the
same displacement at every finer level.

The test is then simple to state and should be written early: for a sample of positions on a sample of
worlds, the height at level _n_ and the height at level _n + k_ differ by less than a stated bound that
depends only on _n_, and the collision query at the level the ship is using returns a value inside the
tolerance of what is drawn.

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
- **Fused multiply-add contraction.** A compiler may fuse a multiply and an add, changing the result in
  the last bits, and whether it does depends on target and flags. WebAssembly has no FMA, so a native
  build that fuses and a wasm build that cannot will disagree. Contraction must be off.
- **Summation order** in octave accumulation, which must be fixed and never reassociated.
- **Relaxed SIMD is banned outright.** Its whole premise is that an instruction may return different
  results on different hardware. Fixed-width 128-bit SIMD is IEEE-exact and welcome.
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

| Model                                                | Verdict                                                                                                                                                                                                                                                                                                                       |
| ---------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Bruneton's precomputed scattering, 2017 revision** | **Lean.** Multiple scattering precomputed into lookup tables, works inside and outside the atmosphere, handles aerial perspective, and — decisively — takes _custom density profiles and absorption_, which is exactly the generality an arbitrary gas mixture needs. Open reference implementation, including a WebGL build. |
| Hillaire 2020, the production LUT approach           | Cheaper and shipped in a major engine, and the right fallback if Bruneton's precompute proves too slow on arrival. Its published costs could not be extracted, so this is a measurement to make, not a claim to repeat.                                                                                                       |
| Nishita 1993, O'Neil (GPU Gems 2)                    | Single scattering only, with the known darkening artefacts and a phase function disabled to hide them. Too approximate for a display that claims physical units.                                                                                                                                                              |
| Hosek–Wilkie and other analytic sky models           | Fitted for ground-level daylight on Earth. No use from orbit.                                                                                                                                                                                                                                                                 |

**Parameterising from physics** is the part that must be built rather than borrowed, and it is
straightforward in outline. Rayleigh scattering follows from the number density and refractive index
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
both forward-scattering effects — would still come out wrong.

### Clouds

Volumetric clouds are the most expensive thing in this document and the first thing to cut. The
reference point is Guerrilla's Nubis, whose 2015 prototype ran in under 2 ms on a PlayStation 4 — and
that is _with_ the quarter-resolution raymarching and temporal reprojection such systems require, not
instead of them. The discrete estimate in [the budget](#performance-budget) is higher than that
figure on a GPU some eight times a PlayStation 4's, and the reason should be stated rather than
implied: Nubis draws a layer seen from the ground, and HYPERION's clouds are a shell seen from orbit,
from within and from below, which needs longer marches and more samples. A good implementation
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

Two honest gaps. A **spherical** ocean at planetary scale with level of detail is poorly covered in the
published work — the standard references are all flat-patch — so tiling tangent-plane patches over the
sphere with a radial height offset is our own construction and should be treated as a risk. And the
**shoreline**, where a wave field meets procedural terrain, is the classic hard case: it needs the
coarse ocean level, a depth-dependent wave amplitude, and foam driven by the terrain's slope.

### Rings

Rings are easy to make look wrong and the published technical accounts are thin — the sources sought
for this section were largely unreachable, so this is the least-researched part of the document and
should be taken as a sketch.

The physics to respect: a ring is an optical depth, not a surface, so it is rendered as transmittance
through a particle layer with a phase function that is strongly forward-scattering, which is why a
backlit ring looks utterly different from a front-lit one. It needs the planet's shadow cast on it and
its own shadow cast on the planet, both of which are strong, recognisable cues. Self-shadowing between
particles is a refinement to approximate rather than simulate. From an astronomical unit away a ring
is a textured annulus; from a kilometre away it is a field of individual bodies, and where that
transition happens is an open question.

### Knowledge, and the surface seed

The single-player brainstorm raised a problem and deferred it here: the client needs the surface to
draw it, but the rule is that the client holds only what the ship has seen, and a seed that reveals
the whole surface at once breaks that rule.

The [server-side coarse field](#who-computes-the-coarse-field-and-why-it-is-the-server) resolves most
of it. The client is given the regions of the coarse field the ship has surveyed — exact where it has
them, absent where it does not — and never the surface seed. The fine synthesis it runs locally takes
a **detail seed** instead: a one-way derivation from the surface seed, from which the coarse field
cannot be regenerated, so the client can only elaborate what the ship already established.

What remains is honest labelling, and it is a rendering requirement rather than a fiction one. Terrain
the ship has not surveyed at close range is an **approximation** — the synthesis is the model's
elaboration of an orbital survey, not a measurement of the ground — and the display must say so,
which is what the guide's estimated state is for, with the degraded-rendering state proposed under
[What the guide must gain](#what-the-guide-must-gain). A surface drawn from a coarse orbital survey
should not present itself as surveyed ground, any more than an unscanned contact's mass is shown
without its `~`. A region not yet surveyed at all is drawn as what the ship knows of it, the disc and
atmosphere from plan 14's figures, with no terrain claimed. **Lean:** the view carries the survey
coverage and resolution as a readout, the same way the star chart carries its census line.

## The sky

### The star field is the galaxy, not a photograph

Most space games paint a sky box. HYPERION should not, and for once the realistic answer is also the
cheap one: the galaxy model already knows where every star is, and once plan 06's stellar stage is
wired in, what its luminosity and effective temperature are. Two things stand between that and a sky
today. The range query returns each system's initial mass and age, not the luminosity and temperature
plan 06 derives from them. And the query is **volume-limited** — a radius, a mass-layer floor and a
`limit` — while a sky is **flux-limited**: a K giant 3,000 ly away can outshine a red dwarf at 10. The
sky therefore needs a magnitude-limited selection, built from range queries whose radius grows with
each mass layer's luminosity ([open question 13](#open-questions)). With those, the sky is
**generated from the same model the charts read**, which means the view out of the window and the
`GALAXY` display cannot disagree, and a star the player jumps to is the star they were looking at.

The practical form:

- **Apparent magnitude sets the flux**, from the star's luminosity and its distance, and flux converts
  to the absolute luminance units the rest of the pipeline uses. Nothing is authored by eye.
- **Colour from the effective temperature**, by evaluating the Planck function, integrating against the
  CIE colour matching functions and converting to the display primaries, desaturating towards the white
  point when a colour falls outside the gamut. Precomputed as a one-dimensional table against
  temperature, which is exact enough and costs a texture lookup.
- **Faint stars baked into a cubemap per location; bright ones drawn as sprites.** Tens of thousands
  of point sources are expensive to draw and, worse, they alias: a sub-pixel star flickers as the
  camera turns, which is the classic failure and it looks like a bug. Baking integrates each star's
  flux over the texel it falls in, which is correct antialiasing only while a texel is no larger than
  a pixel — and at 60° across 1080p that needs faces of about 2,900 texels, some 200 to 400 MB for
  the six in a floating-point format, and more if the view zooms. So the bake takes the faint,
  unresolved majority at a resolution the memory affords, where a faint star spread over a texel
  reads as the sky's grain, and the few thousand stars bright enough to be seen individually are
  drawn every frame as sprites whose point-spread function is integrated over the pixel, which keeps
  their antialiasing at any field of view.
- **Baked once per arrival, with parallax left to the sprites.** A star at distance _d_ shifts by
  206,265 × (baseline ÷ _d_) arcseconds, against roughly 110 arcseconds per pixel at 1080p across a
  60° field of view. In the solar neighbourhood, with the nearest star over a parsec away, a journey
  of a few astronomical units moves the sky by a few arcseconds, a few hundredths of a pixel. But the
  galaxy brainstorm places some fifteen million pairs of systems within 0.1 ly of each other, over
  half of them in the nuclear disc, and a star 0.1 ly away shifts by about 33 arcseconds per
  astronomical unit: crossing 30 au moves it nine pixels. **Lean:** a star that would shift by more
  than a tenth of a pixel across the system is drawn as a sprite at its true position every frame
  rather than baked, and everything else is baked once per arrival. The threshold is a number, and
  saying so with it is better than hoping nobody asks.
- **The galactic band from the galaxy's own model.** The Milky Way seen from inside is the light of
  the stars too faint or too many to draw individually, integrated along each line of sight outward
  from the ship and dimmed by the dust in front of it. That is related to what the `GALAXY` map shows
  but is not the same integral: the map's column density counts systems per square light-year along
  parallel lines through the whole galaxy, where the band needs luminosity along rays from a point,
  with extinction. It comes from the same density, stellar and dust fields, so it cannot disagree with
  the charts, and it must begin where the individually drawn stars end, or the stars near the ship are
  counted twice. Drawing the band from the model means a ship in the outer disc sees a thin bright line
  in one direction and a sparse sky in the other, _because the model says so_, and dust lanes appear
  when the dust field does. No other game can do this, because no other game generates the galaxy it
  is standing in.

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

| Pass                         | Discrete target, 1080p, 16.7 ms budget          | UHD 620 low setting, 720p, 33 ms budget                                                                                    |
| ---------------------------- | ----------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| Terrain geometry and patches | 3–5 ms                                          | 8–14 ms, a shallower quadtree away from the ship; full depth kept under it                                                 |
| Atmosphere, LUT lookups      | 0.5–1 ms                                        | 2–4 ms, lower-resolution tables, aerial perspective on terrain only                                                        |
| Atmosphere, LUT precompute   | Tens of ms, once per planet                     | Hundreds of ms to seconds, spread over frames on arrival — a spike item, and the reason a cheaper model is held in reserve |
| Volumetric clouds            | 1.5–3 ms at quarter resolution                  | **Cut.** Replaced by a two-dimensional layer at about 1 ms                                                                 |
| Ocean                        | 1–2 ms, Gerstner                                | 2–3 ms, fewer wave components, glint retained                                                                              |
| Shadows                      | 1.5–3 ms, cascaded, with cloud shadows          | 2–4 ms, one cascade or a horizon map for terrain self-shadowing                                                            |
| Star field and galactic band | Under 0.2 ms, a cubemap and bright-star sprites | Under 0.5 ms                                                                                                               |
| Exposure histogram           | 0.3–0.5 ms                                      | About 1 ms, over a quarter-resolution input                                                                                |
| Bloom and tone mapping       | Under 1 ms                                      | 2–3 ms, fewer bloom levels                                                                                                 |
| Scatter instances            | 1–2 ms                                          | Off, or a token density under 1 ms; the first thing after clouds to go                                                     |

The discrete column's lower ends sum to about 9 ms and its upper ends to about 18 ms, which is over
budget. That is recorded rather than tuned away: the frame fits only if the passes do not all land at
their worst, and the first measurements decide which one gives. The UHD 620 column sums to about 18
to 32 ms, inside its 33 ms frame with little to spare.

The ladder, stated as policy rather than as a list of numbers:

| Feature        | High                                                            | Low                                                                                  |
| -------------- | --------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| Clouds         | Raymarched volume with temporal reprojection                    | Two-dimensional layer, correct albedo and optical depth                              |
| Terrain detail | Full quadtree depth, GPU decoration                             | Shallower quadtree away from the ship, decoration off; full collision depth under it |
| Ocean          | Gerstner waves, shoreline foam, sun glint; spectral waves later | Fewer Gerstner components, sun glint retained                                        |
| Shadows        | Cascaded, with cloud shadows on terrain                         | One cascade or a horizon map: terrain self-shadowing only                            |
| Atmosphere     | Full tables, aerial perspective on everything                   | Smaller tables, aerial perspective on terrain only                                   |
| Scatter        | Instanced, filtered by biome and slope                          | Off, or a token density                                                              |
| Rings          | Transmittance with a forward-scattering phase, and both shadows | A textured annulus with the planet's shadow                                          |
| Resolution     | 1080p native                                                    | 720p, presented upscaled                                                             |

Three rules keep this honest. The renderer **states its setting on the display**, under the
degraded-rendering data state, so a player is never misled about whether they are seeing the real
surface. Whatever the setting, **the patches under the ship are drawn at the level the collision
query uses**: the low setting may coarsen what the ship is not touching, but never what it is, or the
guarantee of [Level-of-detail consistency](#level-of-detail-consistency-and-why-collision-agrees)
would hold only on the high setting. And the low setting is **built alongside the high one**, not
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
  for a height. Terrain belongs in a small crate — `hyperion-surface` — that depends on the sim's
  `math`, `rng` and `units` and nothing else, compiles to `wasm32-unknown-unknown` for the client and
  links natively into the server. The wasm bundle then holds the height function alone.
- **Both wasm targets join the checks, automatically.** `wasm32-unknown-unknown` is exercised with
  `wasm-bindgen-test` in a headless browser, the golden height files are asserted equal across
  native, wasip1 and the browser target, and both wasm runs move from the manual `just test-wasm`
  into a gate that runs without being remembered, whether `just ci` itself or a CI job beside it
  ([open question 12](#open-questions)). The existing pin of `libm` to `=0.2.16`, already documented
  as a generator-version change if bumped, is what makes that plausible; the same discipline extends
  to the surface crate.
- **Fixed-width SIMD is allowed; relaxed SIMD is banned.** WebAssembly's relaxed SIMD proposal
  permits two implementations to return different results for the same instruction, which is
  precisely what the determinism rules forbid. Fixed-width 128-bit SIMD is IEEE-exact and may be
  used, provided the code does not let the compiler reassociate a sum; the rule belongs in
  `.claude/rules/rust-dev.md` next to the existing numeric rules.
- **The renderer sits in the app, beside `spatial/`, not inside it.** A new `view/` directory holds
  the engine wrapper, the scene builder and the shaders. What the two share — `vec3`, `frame` and the
  direction conventions — moves to a common place rather than being duplicated or bent. The
  orthographic `Camera` is not generalised into a perspective one: they are different display classes
  and merging them would produce a type that means neither.
- **The engine is loaded lazily.** Consoles that do not draw a scene must not pay for the engine's
  bundle, which a dynamic import and a manual chunk achieve.
- **The scene arrives as a subscription.** The view needs the bodies, craft and stars near the ship
  at the tick rate, which the current protocol cannot express: it has six request kinds and no push.
  Plan 04 already reserved the mechanism — a request whose response carries a `subscription: u32`,
  pushes as `notification`, and binary frames for bulk payloads — so the view is the feature that
  finally builds it. **Lean:** JSON while the scene is bodies and craft, measured before anything
  binary is written, exactly as the single-player brainstorm concluded.

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
  [determinism rules](../../../.claude/rules/rust-dev.md) the sim already follows. The wasm runs
  need toolchains `just ci` does not install today, so whether they sit in `just ci` itself or in a
  job beside it is [open question 12](#open-questions); either way they are automatic.
- **Collision agrees with what is drawn.** The rendered surface at a given level of detail and the
  height the collision query returns must agree within a stated tolerance, and the tolerance must be
  a function of the level. This is the test that makes "the same terrain on both sides" a checkable
  claim instead of an intention.

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
  engine-agnostic, which is what makes the choice reversible.
- Reversed-Z with a floating-point depth buffer and an infinite far plane; no logarithmic depth.
- Camera-relative rendering with per-patch `f64` origins, from the frames the simulation already defines.
- Absolute photometric units throughout, with a photographic exposure model and AgX tone mapping.
- A cube-sphere quadtree with fixed-grid patches and vertex morphing.
- A coarse global field at cube-sphere level 7 or 8, computed **on the server** and sent to the
  client, with all fine detail synthesised locally; Knowledge gates its coverage but never its
  accuracy.
- A new `hyperion-surface` crate, and both wasm targets brought into the determinism checks
  automatically, before any terrain code.
- A low setting targeting 30 fps at 720p on the UHD 620, against a discrete reference of the RTX 4060
  class.

The edits this document asks of `docs/frontend/ux-guidelines.md`, collected under
[What the guide must gain](#what-the-guide-must-gain), are **proposals for the owner**, since guide
additions are the owner's call.

## Open questions

1. **What Babylon.js's "Large World Rendering" actually does.** The feature is named in the 9.0.0
   release notes, which link its page at <https://aka.ms/babylon9LWDoc>; that documentation is served
   by a client-side application that automated fetching cannot read. **Lean:** a human reads that page
   before any weight is put on it, and the design does not depend on it either way.
2. **Whether the engine survives the descent spike.** **Lean:** yes. The adapter makes a switch between
   JavaScript engines cheap but not a move to a native renderer, which is one more reason the spike
   comes before anything depends on its answer. The condition for reaching for a native renderer is
   narrow: the spike fails on adequate hardware for reasons inside the browser stack rather than in
   our own code.
3. **The atmosphere tables' precompute cost per planet**, particularly on the Intel part, where it may
   run to hundreds of milliseconds or seconds and must be spread over frames on arrival. **Lean:**
   reduced table sizes first; the cheaper production LUT model is held in reserve, and its published
   costs need measuring rather than quoting.
4. **Whether the coarse field is persisted.** It is a pure function, so it need not be. But it is also
   what the ship has surveyed, and survey results are player knowledge that ought to survive a save.
   **Lean:** recomputed and memory-cached for rendering, and recorded in the Knowledge overlay as a
   statement of _what has been surveyed_, not as a copy of the data.
5. **Erosion: heuristic or analytic.** Conditioning local noise on coarse drainage is a heuristic that
   will look plausible and is not physics. There is a published analytical erosion built on the
   stream-power law that would make it physics. **Lean:** ship the heuristic, label it as one, and treat
   the analytical route as the upgrade that closes the gap.
6. **The collision wavelength.** Where exactly the line falls between the authoritative height function
   and GPU-only decoration. **Lean:** a metre to begin with, tuned against what the flight model can
   actually touch, and stated in the surface crate's documentation because both sides depend on it.
7. **A spherical ocean at planetary scale.** The published work is all flat-patch; tiling tangent-plane
   patches over a sphere with a radial offset is our own construction. **Lean:** treat it as a genuine
   risk, and get sun glint and the coarse ocean level right before any wave geometry.
8. **Where a ring stops being an annulus and becomes particles**, and whether self-shadowing is worth
   approximating. This is the least-researched part of the document.
9. **Climate for worlds that are not Earth-like.** Energy-balance models and Köppen classification are
   Earth-centred; tidally locked "eyeball" worlds, high-obliquity worlds where the equator is the coldest
   place, airless bodies and Titan-like hydrocarbon cycles each break them differently. Automatically
   selecting the right model from stellar and orbital parameters alone is, as far as this research found,
   an open problem. **Lean:** an explicit regime classifier choosing among a few documented models, with
   non-Earth-like classes named separately rather than forced into Köppen's letters.
10. **Whether the scene subscription needs binary frames.** **Lean:** JSON until measurement says
    otherwise, as the single-player brainstorm concluded.
11. **Whether scatter is ever collidable.** A boulder a landing party can walk into is a different
    contract from a rock that is decoration. **Lean:** decoration first; collidable scatter only if a
    later phase needs it, and then through the same hashed placement so the server can answer for it.
12. **How the wasm targets join the determinism checks** — which harness runs the browser target, and
    whether both wasm runs go into `just ci` itself or into a CI job beside it, since each needs a
    toolchain `just ci` does not install today. **Lean:** `wasm-bindgen-test` in a headless browser,
    and both runs automatic before any terrain code is written, the descent spike's included, because
    they are the check the whole shared-terrain argument rests on.
13. **Where the sky's stellar figures come from, and how a flux-limited sky is queried.** The range
    query returns initial mass and age; the sky needs luminosity and effective temperature, and a
    selection limited by apparent brightness rather than by radius. **Lean:** the server runs plan
    06's stellar stage and the system record gains both figures, so the client never runs it; the
    magnitude-limited selection is a server request built from per-layer range queries, so that the
    radius for each mass layer follows from its luminosity.

## Suggested order of attack

Not a plan, only the dependency order a plan would follow. Steps 1 to 3 are the ones that retire risk;
everything after them is additive. Steps 5 to 9 also wait on plan 14, which has not started: they need
its radii, albedos, rotation, compositions and global figures. The descent spike does not, because it
can run on a hand-parameterised test planet.

1. **Foundations and the wireframe.** The engine adapter, camera-relative differencing, reversed-Z,
   frame-change rebasing, the photometric pipeline with exposure and tone mapping, and `VIEW` as a
   wireframe at real scale with stars at their true magnitudes. The scene subscription in the protocol.
   This is the precision test rig, and the point at which the depth and precision tests exist.
2. **The wasm targets in the determinism checks.** `wasm32-unknown-unknown` under `wasm-bindgen-test`,
   both wasm runs made automatic, and the client loading its first WebAssembly. Nothing terrain-shaped
   is built yet, but this is the check the shared terrain rests on, so it comes before the first line
   of terrain code, spike code included.
3. **The descent spike, which is the gate.** An Earth-sized test planet from orbit to a metre above the
   ground, terrain from sim code in WebAssembly workers. It passes at 1080p60 on the discrete target and
   at 30 fps at 720p on the UHD 620's low setting. The project has no discrete GPU today, so that half
   of the measurement needs one borrowed or rented; the UHD 620 half runs on the machine that exists,
   and a failure there is already an answer. The single-player brainstorm named this spike with a
   criterion of 1080p60 on the owner's machine, which the owner's ruling on the performance floor has
   since replaced. It decides whether the browser carries the planets, and it should happen before
   anything depends on the answer.
4. **The sky.** Plan 06's luminosities and temperatures in the system record, the magnitude-limited
   selection, faint stars baked and bright or near ones drawn as sprites, the galactic band from the
   model, the local star as a limb-darkened disc, blackbody colour. Cheap, highly visible, and it
   exercises the photometry.
5. **Lit bodies at real scale**, from plan 14's radii, albedos and rotation, with correct phase and the
   terminator. Still no surfaces.
6. **Atmospheres**, parameterised from plan 14's composition, pressure, temperature and gravity, with
   aerial perspective on terrain.
7. **The surface generator.** The `hyperion-surface` crate with its own `clippy.toml`, the coarse
   global pass on the server, its wire representation, and golden height files added to step 2's
   parity checks across native and both wasm targets. Nothing is drawn from it yet.
8. **Terrain.** The quadtree, patch streaming, height textures, morphing, materials — and the test that
   collision agrees with the picture.
9. **The rest of the surface**, in rough order of value: GPU decoration, scatter, clouds, ocean and
   glint, rings.
10. **The quality ladder and the guide edits**, made real: both settings of every feature, the
    degraded-rendering data state, and the recorded measurements on both GPUs.

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
  `packages/dev/core/src/Engines/thinEngine.pure.ts`. Babylon.js's own documentation site could not be
  read by automated fetching, so its large-world feature's content is **unverified**.
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
  draft of this document called it Bruneton-style); and the `big_space` crate for large worlds.
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
  generation_, Computer Graphics Forum — the closed-form substitute for iterative erosion.
- Schott, Paris, Fournier, Guérin et al. 2023, _Large-scale terrain authoring through interactive erosion
  simulation_, ACM TOG; and 2024, _Terrain Amplification using Multi Scale Erosion_, which names the
  boundary problems tiled erosion runs into.
- Génevaux et al. 2013, _Terrain generation using procedural models based on hydrology_, ACM TOG;
  Teoh 2009, _Riverland_; Derzapf, Ganster and Guthe 2011, _River networks for instant procedural
  planets_, Computer Graphics Forum — the network-first inversion.
- Guérin et al. 2016, _Sparse representation of terrains for procedural modeling_; Guérin et al. 2017,
  _Interactive example-based terrain authoring with conditional generative adversarial networks_, ACM TOG;
  Grenier et al. 2024, _Real-time Terrain Enhancement with Controlled Procedural Patterns_, reporting
  amplification up to thirty-two times — the amplification line of work.
- Paris, Galin, Peytavie, Guérin and Gain 2019, _Terrain Amplification with Implicit 3D Features_, ACM TOG.
- Argudo, Galin, Peytavie, Paris and Gain 2019, _Orometry-based terrain analysis and synthesis_, ACM TOG.
- Tucker and Whipple 2002, JGR; Harel, Mudd and Attal 2016, _Geomorphology_ — the stream-power law and its
  exponents.
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
- Checlair, Menou and Abbot 2017, ApJ (tidally locked "eyeball" states); Ferreira, Marshall, O'Gorman and
  Seager 2014, Icarus (at 90° obliquity the equator is the coldest region); Williams and Kasting 1997,
  Icarus; Lunine and Atreya 2008 and Hayes, Lorenz and Lunine 2018, Nature Geoscience (Titan's methane
  cycle).

**Atmosphere, clouds, ocean and imaging**

- Bruneton and Neyret 2008, and Bruneton's 2017 revision with its reference implementation, which adds
  custom density profiles and absorption. <https://ebruneton.github.io/precomputed_atmospheric_scattering/>
- Hillaire 2020, _A Scalable and Production Ready Sky and Atmosphere Rendering Technique_, EGSR. Its LUT
  sizes and per-frame costs were **not extracted**; only the abstract was readable.
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
