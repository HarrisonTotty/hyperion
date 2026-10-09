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

Given the galaxy the [galaxy brainstorm](galaxy-generation.md) generates, draw it through a
free-flying camera, as a wireframe or a photorealistic image
([Render styles and multiple views](#render-styles-and-multiple-views)): a perspective view true to
scale from a hull plate a metre away to a star system tens of astronomical units across, and
eventually a planet flown from orbit to a landing without a seam
or a loading screen. The rendering must be defensible in the same way the simulation is: what is drawn is
what the numbers say, and where the picture is an approximation, the display says so.

In scope:

- The rendering technology: what runs inside the Electron client, and what would replace it.
- The real-scale foundations: floating origin from the galaxy's frames, the depth buffer, and
  luminance in physical units.
- `VIEW` in two styles, both kept: a wireframe first, then a lit, photorealistic scene, from a free
  camera, for a bridge's main screen and stations and for the single-player cockpit, with several
  views at once and still images on request.
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
- Interiors. The cameras look out through a window or fly outside the hull, never into its rooms.
- LLM-generated description text, which never reaches the renderer.

## The scales the view spans

The reason this is hard is in one table. Each row is a real situation the player will be in, and the
renderer has to handle all of them without changing its mind about what a metre is.

| Situation                  | Distance to what matters | What dominates the picture                                              | What dominates the cost               |
| -------------------------- | ------------------------ | ----------------------------------------------------------------------- | ------------------------------------- |
| Docked, hull a metre away  | 1 m – 100 m              | Hull plates, the base, the docking port                                 | Nothing; it is one model              |
| Low orbit, 400 km up       | 4 × 10⁵ – 2 × 10⁶ m      | The planet filling the window, terrain detail, the horizon 2,300 km off | Terrain level of detail, clouds       |
| Orbit, a few radii out     | 2 × 10⁷ – 10⁸ m          | The whole planet as a disc, its atmosphere's limb                       | Atmosphere, cloud layer, the ring     |
| Moving between planets     | 10⁹ – 10¹² m             | Bodies as discs and points, the star, the star field                    | Almost nothing, away from a gas giant |
| Across the system          | 10¹¹ – 10¹³ m            | The star, a few discs, the star field                                   | Almost nothing                        |
| Interstellar, after a jump | 10¹⁵ m and out           | Stars as points, the galaxy as a band                                   | Almost nothing                        |

Two things follow. First, the expensive cases are the near ones, and they are the ones with a planet
in them; everything beyond a million kilometres or so is cheap because it is points and small discs.
The exception is a gas giant. At 7 × 10⁷ m in radius it fills the window from inside about
1.4 × 10⁸ m and is still some 8°, about 250 px, across from 10⁹ m, and a ringed one's rings span
about twice its diameter. Its atmosphere needs the full passes while the limb shell of about ten
scale heights spans two pixels, out to about 2.5 × 10⁸ m for Jupiter and 5.5 × 10⁸ m for Saturn, so
10⁹ m is a safe boundary for them. Beyond it, while the disc is at least three pixels across (to
about 9 × 10¹⁰ m for Jupiter), the body is an analytic disc with a baked reflectance table and an
analytic ring annulus, which costs one quad; below three pixels it is a point. The interstellar row
starts at 10¹⁵ m, 0.1 ly, because that is how close a neighbour can be in the nuclear disc; near the
Sun it is a few light years.

Second, the dynamic range in _position_ is about 10¹⁵ between a centimetre of hull and an outer
planet, and in _luminance_ about 10¹¹ between the faintest star drawn, magnitude 6.5 with its light
in one 1080p pixel at about 2 × 10⁻² cd/m² (four times that at 4K), and a Sun-like star's disc,
about 2 × 10⁹ cd/m², with a sunlit surface, about 10⁴ cd/m², in between. Across every scene the
renderer draws, from the faintest galactic background to the centre of an O star's disc, luminance
spans some 16 decades. Position does not fit in a 32-bit float; luminance fits in one easily, but
not in a display or in the half-float render targets it passes through. Both have standard answers,
and they are in [Real-scale foundations](#real-scale-foundations).

## Constraints that decide the design

The engine choice is usually argued on features. Here it is decided by five constraints that come
from outside rendering, and they narrow the field before any feature list is opened.

1. **Real scale, to the centimetre where the camera is.** A GPU works in 32-bit floats. At Earth's
   surface radius, 6,371 km, consecutive `f32` values are **0.5 m** apart, and at 1 au they are
   **16 km** apart. A planet's surface simply cannot be expressed in world coordinates on the GPU.
   The renderer must therefore be camera-relative in a specific, checkable way, and the engine must
   not fight it. The centimetre is a claim about the hull and body frames the camera works in; an
   absolute galactic position resolves only about 2 m, which matters only between the stars, where
   nothing drawn is nearer than 10¹⁵ m.
2. **The same terrain on both sides.** Anything the ship can collide with must be identical in the
   client and the server, bit for bit. `hyperion-sim` already compiles to `wasm32-wasip1` and is
   checked there by hand (`just test-wasm`), so the terrain generator can be sim code that the
   server runs natively and the client runs in workers — but only once the browser's target,
   `wasm32-unknown-unknown`, builds and is checked for determinism, which today it is not (see
   [Runtime and code shape](#runtime-and-code-shape)). This is the single strongest architectural
   constraint in the document, and it reaches into the choice of noise functions and even of SIMD
   instructions.
3. **The consoles come first.** HYPERION is a bridge simulator whose displays are flight hardware.
   `docs/frontend/ux-guidelines.md` requires that text which must be read stays in the DOM, set in
   B612 and B612 Mono, contrast-checked by the project's own tooling, with every spatial display's
   canvas paired to a DOM list of its marks that the keyboard can reach (a rule this document
   extends to views; see
   [Accessibility, which a canvas threatens](#accessibility-which-a-canvas-threatens)). A renderer
   that wants to own the whole window, draw its own text and swallow input is a worse fit than a
   less capable one that sits inside a React display.
4. **The hardware that exists.** The development machine has no discrete GPU: an Intel UHD Graphics
   620 (Whiskey Lake-U GT2, Gen9.5), on Mesa 26.2.3 with the ANV Vulkan driver. The owner has ruled
   that a modern discrete GPU is the design target at 1080p60, but that **every feature needs a
   documented low setting that stays playable on the UHD 620** so that the real renderer can always
   be developed and tested locally. Gen9.5 also sits below the Intel generation at which Chromium
   enables WebGPU by default on Linux, which is why the API floor needed a ruling of its own; see
   [Decisions](#decisions).
5. **Longevity.** The project is long-lived and the simulation beneath it is meant to outlast
   several rendering fashions. An engine that breaks its API every release is a standing tax, and
   one that is abandoned is a rewrite. This constraint is what the engine comparison below actually
   turns on, more than any feature.

## What to take from the references

Every game listed here has solved some part of this problem, and the differences between them are
instructive because they are forced by scale.

| Reference                 | Take                                                                                                                                                                                                                                                                 | Leave                                                                                                                                                                                                                                                                                                                                                            |
| ------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| No Man's Sky              | The feel of the descent, and the proof that one continuous motion from space to the ground is the thing worth building. A density function evaluated on demand, per region and level of detail, straight from the seed, with no stored world (McKendrick, GDC 2017). | Its planets are small — tens of kilometres in radius by community measurement, and some hundreds for the giants added in 2025, not thousands — which is why it has no precision problem. And it has no authoritative coarse field: every client regenerates the whole world from the seed, which Knowledge forbids here. HYPERION cannot take either shortcut.   |
| Artemis                   | The owner's reference for the wireframe style: the graphic language of its consoles, with thin vector hulls and radar symbology on black, read at a glance.                                                                                                          | Its flat sector with no bodies or orbits. The graticule bodies, the orbits and the perspective view are HYPERION's own, drawn in that language. Nothing of how Artemis draws is assumed.                                                                                                                                                                         |
| Elite Dangerous           | Real-scale bodies flown to the surface, with terrain generated from seeds rather than stored. Proof that the realism ruling is technically feasible.                                                                                                                 | Its landable planets are airless or thin-atmosphere by design, which sidesteps the hardest rendering problem. HYPERION's planetary stage will produce thick atmospheres.                                                                                                                                                                                         |
| Outerra                   | The depth-buffer work: the clearest published account of what precision each scheme actually gives, with measured costs. Camera-relative rendering.                                                                                                                  | Its fragment-written logarithmic depth, whose early-Z cost its own measurements show; reversed-Z makes it unnecessary. And its terrain is Earth's, from elevation datasets, where ours must come from the seed.                                                                                                                                                  |
| SpaceEngine               | One hierarchy from galaxy to moon with seamless scale changes, and procedural height cached into per-patch GPU textures rather than re-evaluated every frame. The free camera, bounded here by Knowledge.                                                            | A camera that sees whatever the generator can make. HYPERION's sees only what the ship knows, within the current system.                                                                                                                                                                                                                                         |
| Cesium                    | The best public write-up of planetary precision: per-patch `f64` origins, and the honest finding that one logarithmic frustum still fights at extreme range. A globe renderer held to surveying standards.                                                           | Its problem is one known Earth with measured tiles streamed from a server. Ours are the worlds of 10¹¹ unvisited systems, computed on arrival. And its two-float encoding, which emulates the `f64` difference on the GPU: per-patch origins differenced on the CPU make it unnecessary, and one rule for where differencing happens is easier to test than two. |
| Star Citizen              | That the industry answer to jitter at kilometre scale was to widen world coordinates to 64 bits, which corroborates that 32-bit world space is not merely inconvenient but unusable.                                                                                 | Widening everything is not available to a web renderer: a GPU vertex buffer has no `f64`. We narrow late instead.                                                                                                                                                                                                                                                |
| Kerbal Space Program      | A quadtree on a cube sphere with a stack of height modifiers, and a floating origin that rebases discretely rather than every frame.                                                                                                                                 | Scaled-down planets and a rescaled solar system.                                                                                                                                                                                                                                                                                                                 |
| Horizon Zero Dawn (Nubis) | Volumetric clouds that read correctly from below and above, at a measured cost — under 2 ms on a PlayStation 4 for the 2015 prototype — which is the number to beat and the reason clouds need a low setting.                                                        | A single authored sky for one Earth-like world. Ours are parameterised by composition and pressure.                                                                                                                                                                                                                                                              |

The common lesson is the mirror of the galaxy brainstorm's. There, the galaxy is a function rather
than a database. Here, **the picture is a measurement rather than an illustration**: every quantity
that reaches a pixel should be traceable to something the simulation computed, and the few places
where that is not true should be labelled on the display.

## The engine

### Checking the prior lean's reasoning

An earlier version of the single-player brainstorm leaned Babylon.js on two grounds: that it shipped
large-world rendering with a floating origin in 9.0, and that it commits to backward compatibility.
Both were checked, first through Babylon's documentation site, which automated fetching cannot read,
and then through the documentation's markdown source on GitHub and the engine's own code. Both hold,
with qualifications:

- **Reversed-Z is not a differentiator.** Both engines have it. Babylon.js exposes
  `useReverseDepthBuffer`, which sets the depth function to `GEQUAL` and clears depth to 0 (verified
  in `Engines/thinEngine.pure.ts`). three.js exposes `reversedDepthBuffer` and
  `logarithmicDepthBuffer` as options of its shared `Renderer`, so on both its WebGPU and WebGL
  renderers, the latter through `EXT_clip_control` (verified in `renderers/common/Renderer.js` and
  `WebGLRenderer.js` at r186); whether the two combine is unverified. The contrast that earlier
  version implied does not exist, and it has since been corrected.
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
  essentially every release, and the official advice is to upgrade in increments of ten because
  deprecations last that long — more than two years at the current cadence of about four releases a
  year. Some of the changes are visual, recorded in the guide but with no flag to restore the old
  look: physically-based brightness shifted in r181, and an ambient occlusion effect darkened in
  r185.

That last point deserves weight beyond its size, because of what this renderer is for. A display
calibrated in absolute photometric units, whose numbers a console will state, cannot afford a
dependency that changes what a given radiance looks like between releases with no way back. In a
game that would be a nuisance. Here it falsifies an instrument.
Babylon.js's visual changes are the contrast that matters: logged, and each with a way back.

### The decision does not rest on the engine's large-world feature

The more important realisation is that **the floating origin is ours whatever we choose.** It has to
be: it must agree with the frames `hyperion_sim::coords` already defines, change when the simulation
changes frame, and be unit-tested against the same `f64` arithmetic the server uses. That is a few
hundred lines of our own code — a differencing step, a per-patch origin, a rotation-only view matrix
— and an engine's opaque large-world system is as likely to fight it as to help. The same is true of
the depth policy and the exposure model.

Babylon.js's own feature shows why. It works in one 64-bit world frame, whose spacing at galactic
distances is tens of kilometres — 33 km at the Sun's 26,700 ly from the centre, 130 km at
100,000 ly — with no hierarchy of frames and no rebasing, so it cannot replace the
simulation's frames. With the camera already at the origin it would do nothing useful, and because it
rewrites uniforms by name across every shader, it could silently rewrite our own. **Lean:** it stays
off, and the adapter supplies camera-relative transforms itself.

So the engine is being hired for the ordinary parts: resource and state management, a render graph, a
shader pipeline, a material system, culling of ordinary meshes (terrain culling is ours), and the
tedious correctness of a WebGPU backend across drivers. It is explicitly _not_ being hired to solve
real scale.

| Option                         | For                                                                                                                                                                                                                                                                                                                             | Against                                                                                                                                                                                                         |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Babylon.js 9.28**            | TypeScript-first, which suits a TS 7 workspace with type-aware lint. A written backward-compatibility rule, and a breaking-changes log whose visual changes each come with a flag to restore the old look. Weekly minor releases. Reversed-Z present. A render graph (Frame Graph). Active investment in globe-scale rendering. | A quarter of three.js's community. Documentation site opaque to tooling, though its source is on GitHub. Larger package.                                                                                        |
| **three.js r186**              | The largest ecosystem by a factor of four, and the most published procedural-planet work. Reversed-Z and logarithmic depth both exposed as renderer options. Small package.                                                                                                                                                     | Breaking changes in essentially every release, including visual ones with no way back. Upgrades must be taken in small steps forever, at about four releases a year. `react-three-fiber` pins to a React major. |
| **PlayCanvas 2.22**            | A mature WebGPU implementation with compute shaders, MIT-licensed.                                                                                                                                                                                                                                                              | Editor-centred workflow that a code-first console app would fight. Smallest community of the three.                                                                                                             |
| **Raw WebGPU, or wgpu → wasm** | Total control, and no engine to track.                                                                                                                                                                                                                                                                                          | Everything above becomes ours: culling, materials, resource lifetimes, driver workarounds. Months of work whose output is not gameplay.                                                                         |

**Lean: Babylon.js, confirming the prior document's choice and, once checked, most of its
reasoning.** TypeScript fit is verified. The compatibility commitment is verified: it is a written
rule, and the changes it allows are logged with a way back. The case against three.js is verified
too: its churn is documented and includes visual changes with no way back. The large-world feature
is real but is not what the lean rests on, for the reasons above — and the understanding below makes
the choice cheap to reverse whatever the engine does next.

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

What the engine supplies is resource and state management, the render graph, materials, shader
compilation, the submission of buffers and draws, and the swap chain. Under that arrangement,
switching to another JavaScript engine is a re-implementation of one adapter rather than of the
renderer. It costs perhaps a week more than binding directly to the engine's scene graph, and it
buys the reversibility that even a written compatibility policy does not give.

The adapter's shaders are WGSL only. Babylon's WebGPU engine compiles GLSL through glslang and twgsl
WebAssembly, which it fetches from `cdn.babylonjs.com` the first time a GLSL shader appears; its
built-in materials already use WGSL on WebGPU, and only an explicit `forceGLSL` or a GLSL-only custom
shader or post-process would draw the compilers in. The adapter therefore makes any GLSL compile an
error that names the shader, rather than a fetch, and a test renders every material and
post-process with the network disabled. The game then runs offline and the Content Security Policy
gains no external origin. Self-hosting the two compilers, some 1–2 MB of WebAssembly, is the
fallback if a Babylon feature the renderer needs proves GLSL-only. ~~The `'wasm-unsafe-eval'` that
our own workers need (see [Runtime and code shape](#runtime-and-code-shape)) is a CSP change that
awaits the owner's sign-off.~~ Superseded 2026-09-30 (R04.T10.a): our same-origin module workers
compile WebAssembly under today's policy, which does not change (see "Awaiting the owner").

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

Electron 44 carries Chromium 152. On Linux, Chromium's default display path for WebGPU is Vulkan
through GL interop, and only its Wayland backend offers that path; there, blocklist entry 186
enables it for Intel Gen12 and later with Mesa 22.0 or newer (from Chrome 144) and for NVIDIA with a
driver of 535.183.01 or newer (from Chrome 147), which is what the implementation-status wiki means
by placing NVIDIA's enablement under Wayland. Under X11, Dawn falls back to its null backend and
WebGPU gets no adapter on any GPU. This machine runs Xorg with i3, and a probe of Electron 44.4.3 on
its Gen9.5 UHD 620 (Mesa 26.2.3 ANV) found no adapter without switches. Electron's main process
therefore sets, before `ready` and on every Linux machine,
`--ozone-platform=x11 --use-angle=vulkan --enable-features=Vulkan,VulkanFromANGLE,DefaultANGLEVulkan`.
The `Vulkan` feature moves Chromium's compositor and rasteriser to Vulkan, which lets Dawn use its
Vulkan backend without GL interop; `--use-angle=vulkan` with `VulkanFromANGLE` keeps ANGLE, and so
WebGL and Skia's GL, on the same Vulkan device. This is the implementation-status wiki's set
without its `--enable-unsafe-webgpu`, plus `DefaultANGLEVulkan`, which a report on gpuweb issue
5022, naming no GPU, found stopped a swap-chain acquisition hang when added with `VulkanFromANGLE`.
The probe gave a hardware adapter with correct output with and without `DefaultANGLEVulkan`, and
with `Vulkan` alone, in runs of a few seconds in which the hang did not appear. `DefaultANGLEVulkan`
is kept because it costs nothing, and a soak on this machine settles whether it is needed.
`--enable-unsafe-webgpu` is not set. It is not needed; it also turns on experimental WebGPU
features and lifts Chromium's blocklist on CPU adapters, so that a failed hardware path silently
hands out SwiftShader, which the probe saw both with that flag alone and with
`ForceEnableWebGpuInterop`, a switch that looks like a way to keep GL compositing but whose interop
path X11 disables. None of these switches is set today.

Three consequences, which should be written down rather than discovered:

1. **The switches move the whole window onto a path Chromium does not ship.** Chromium does not
   enable Vulkan compositing on desktop Linux, so the Gen9.5 risk is in compositing and rasterising
   the entire window, DOM consoles included, not only in presenting WebGPU frames. Swap-chain
   acquisition hangs, video and overlay faults, resize flicker and GPU-process restart loops are
   the known and plausible failures; the probe saw none, but did not look for them. The client
   therefore refuses to draw the photorealistic style on a fallback adapter
   (`adapter.info.isFallbackAdapter`, or an architecture of `swiftshader`) and reports that, any
   adapter or device loss, and any GPU-process crash as a ship-system fault in the guide's
   language, rather than crashing or drawing at a few frames a second. After a crash loop it
   relaunches once without the Vulkan features, into a declared mode without the photorealistic
   style.
2. **The switches are a distribution problem, not just a development one.** The main process sets
   them on every Linux machine, so on Linux the forced path is the _only_ path and must be the one
   that is tested. Electron 38 and later run natively under a Wayland session, but Chromium's
   Wayland backend refuses Vulkan compositing, so `--ozone-platform=x11` runs the client through
   XWayland there. Native Wayland would give WebGPU only on the machines entry 186 allows, and one
   tested path is worth more than that; the cost is XWayland's handling of fractional scaling and
   per-monitor DPI, which a console application can bear. Windows on x64 and macOS enable WebGPU by
   default and get no switches; Windows on Arm64 does not enable it.
3. **Compute shaders are therefore available everywhere**, which is what makes the WebGPU-only ruling
   valuable: terrain, cloud and histogram passes can assume compute rather than emulating it in
   fragment shaders. That assumption should be used deliberately, because it is the whole return on
   the ruling. Subgroups are not part of it. They are optional, and Dawn refuses them on Gen9 unless
   a toggle is set, because `subgroupBroadcast` of f16 fails in some cases there
   (crbug.com/391680973). Every subgroup kernel, the exposure histogram's reduction among them,
   therefore has a no-subgroup twin that is the reference, and the client chooses between them by
   `adapter.features.has("subgroups")`, never by GPU identity. On Linux the main process also passes
   `--enable-dawn-features=enable_subgroups_intel_gen9`, which Dawn documents as polyfilling that
   one defect, so that the development machine exercises the path the target uses; subgroup
   operations never take f16 operands. A subgroup reduction changes summation order, so anything
   read back to the CPU must agree bit for bit between the two paths or be declared
   presentation-only.

WebGL2's absence from the plan costs little that matters: it has no compute shaders, no storage
buffers and no indirect draw, so every compute pass this document leans on would need a second,
fragment-shader implementation, which is precisely the second renderer the ruling declined to
maintain. Reversed-Z is not the obstacle it might seem. WebGL's `[-1, 1]` clip range defeats it by
default, but the `EXT_clip_control` extension, Community Approved by the WebGL working group in
November 2023, switches the range to `[0, 1]`. Chromium 152 exposes it on this machine, through
ANGLE over Vulkan and through ANGLE's GL backend alike, so three.js's WebGL reversed-Z would work
here; under the ruling it is moot.

### If the browser cannot carry it

The fallback remains a native renderer in Rust, joining the session as another protocol client, and
the research sharpened what that would cost. Bevy is at 0.19, with 0.20 in release candidates, and
breaks its API every three and a half to five months; its built-in atmosphere implements Hillaire
2020, with transmittance, multiple-scattering, sky-view and aerial-perspective lookup tables, which
is this document's lean too, and its medium is a list of terms each with its own density and phase
function; its transforms are `f32`, so large-world support means the `big_space` crate, which tracks
Bevy a version behind and will be two behind once 0.20 ships. Godot's double-precision build
converts vectors and physics and emulates double precision in the standard vertex transform, but
not in custom world-space shaders, and requires custom editor and export-template builds.

But the cost is not the renderer. **It is the split client**, and on Wayland it is worse than it
looks: there is no cross-process surface embedding, so a native view cannot be placed inside an
Electron window as it could under X11's XEmbed (xdg-foreign can parent a native top-level window to
Electron's, not embed it). The options reduce to two top-level windows, a transparent always-on-top
overlay whose layering is compositor-dependent, or streaming frames into Electron and paying
encode-plus-decode latency that a hand on a stick would feel. Add to that a single input authority
for the HOTAS, and two GPU consumers in one process tree.

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

| Frame    | Representation                                      | Resolution                               |
| -------- | --------------------------------------------------- | ---------------------------------------- |
| Galactic | Integer 1 ly cell (`i32` per axis) + `f64` m offset | 1–2 m anywhere                           |
| System   | `f64` m from the system barycentre                  | about 1 mm at 50 au                      |
| Body     | `f64` m from the body's centre                      | sub-micrometre (about 1 nm) in low orbit |

All three are along the galactic axes (`coords/frames.rs`), so none of them rotates with a body.
Terrain needs one that does, and it is added below as a position type rather than a fourth frame.

The renderer adds one rule to them: **nothing reaches the GPU in world coordinates.** Every position
is differenced against the camera's position in `f64`, in whichever frame the camera is in, and only
the difference is narrowed to `f32`. The difference need not be small: a planet across the system is
10¹¹ m away. What makes narrowing safe is that an `f32`'s relative error, 6 × 10⁻⁸, is far below the
5 × 10⁻⁴ rad of a pixel at any distance, so the direction to everything is exact to the pixel and the
absolute error is smallest where the eye is, which is where it is needed. At 1 km from the camera
consecutive `f32` values are 0.06 mm apart; at 1,000 km, 6 cm apart.

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
  entirely. Composed with a translation of 1 au in `f32`, every transformed vertex lands on the
  16 km spacing of an `f32` at that magnitude.

Terrain patch origins, rocks and the coarse field are fixed to the body, so they are stored as a
body-fixed position, `coords::BodyFixedPosition` (with `BodyFixedVector`): `f64` metres from the
body's centre along its rotating axes. It is a position type, not a new frame. The camera and every
craft stay in the non-rotating body frame, which is inertial, so the flight model needs no fictitious
forces and plan 14's orbits stay in one kind of frame. Each frame the renderer rotates the origins
into the body frame in `f64` through plan 14's `body_fixed_at(body, t)` (P14.T14.c), one 3 × 3 matrix
per body, and only then differences them against the camera. The rotation itself is exact to about
10⁻⁹ m at an Earth's radius; the angle is the limit. Earth's is 2.2 × 10⁵ rad after a century, where
the `f64` step is 2.9 × 10⁻¹¹ rad, 0.2 mm at the surface, so `body_fixed_at` reduces the angle
exactly from the integer span since the epoch, as `observe::Drift` already does, and states a bound
of 10⁻¹² rad over the play window, 6 µm at an Earth's surface. Neither type is in `coords` yet.

The simulation already defines positions as frame plus offset, and already has a rule that selects a
system frame with hysteresis (`galaxy::frame::select_frame`, with `FRAME_HYSTERESIS` 0.1), so the
renderer inherits a floating origin rather than inventing one. The rule is pure functions, though:
there is no ship yet and nothing in the server calls it, the wire carries only the galactic
`GalacticPosition` with no system or body position type, and nothing selects a body frame, so the
frame-change event below needs body-frame selection, which is still to be built. What the renderer
must add is a rule for what happens at a frame change: the camera's frame changes under it, every
cached patch origin is now expressed against a different centre, and a naive implementation will
visibly jump. **Lean:** frame changes are a renderer event, the scene's `f64` origins are rebased in
one operation, and a test asserts, across a change from system to body frame and back with
hysteresis, that the `f64` vector from the camera to a fixed landmark on a body is continuous to
1 mm and its projection to within a pixel, and that a grounded craft's `BodyFixedPosition` is
constant while its `BodyPosition` moves at ω × r — in the same way the single-player brainstorm's
[flight-model test](single-player-experience.md#testing) will assert position and velocity are
continuous.

Drawn positions are apparent positions: each body at x_B(t − τ) − x_cam(t − τ), the body's position
at the retarded time less the camera's position then, with the light time τ solved by fixed-point
iteration. That is light-time and first-order aberration together (the Explanatory Supplement's
"planetary aberration"), never light-time alone, which would displace a planet 400 km below a ship at
7.7 km/s by v × 1.3 ms, about 10 m, from the surface that collision uses. For a comoving camera it
equals the present relative position, and the error of drawing present positions is only
|Δv⊥|/c in angle at any distance, below a tenth of a pixel while the relative speed is under
16 km/s. **Lean:**

- The camera's **local body**, the one whose terrain is streamed or inside whose Hill sphere the
  camera sits, is drawn with its rings and atmosphere geometrically at the present time in its own
  frame, the state collision uses, so drawn terrain and collision agree.
- Every other body is drawn at its apparent position, and its orientation for drawing is taken at
  the emitted time, as SPICE does for body-fixed frames.
- Every time-varying state is drawn at its retarded time, the local body's included: flares, lights,
  a craft's burn, eclipse contacts. That is the sensor model of the galaxy brainstorm's
  "What a sensor sees is the past", under [Orbits and time](galaxy-generation.md#orbits-and-time).
- The difference is visible in telescopic views, where a pixel is about 10⁻⁶ rad and 30 km/s shows,
  in event timing, and above about 0.01c, where first-order aberration gives way to the relativistic
  transform of directions.

The retarded evaluation has to be done in the system frame, about 1 mm, not in `GalacticPosition`,
whose 1–2 m spacing is 5 × 10⁻⁶ rad at 400 km and would make close views jitter. Plan 12's
`hyperion_sim::observe` (P12.T0–T2, T4) has retarded time on galactic positions, and its only
trajectory is `Drift`, for systems. The in-system sibling is a proposal for plan 12 or 14 to adopt: a
`SystemTrajectory` returning `SystemPosition` and `SystemVelocity`, implemented by plan 14's body
orbits with moons composed on their planets, and a `retarded_in_system` that iterates until the light
time changes by under 1 ns, at most three times, as SPICE's converged "CN+S" correction does. In-system
light times reach hours, so a single iteration can leave metres.

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

Reversed-Z `depth32float` spaces its values at about 1.2 × 10⁻⁷ of the distance at every distance
(Reed 2015), and with the `f32` vertex rounding and the rasteriser's interpolation added it orders two
surfaces reliably while their separation along the view exceeds about 10⁻⁶ of the distance: 1 m
apart is safe to 1,000 km, 1 km apart to 10⁹ m. **Lean:** no global depth bias. Transparent layers —
ocean, clouds, rings, atmosphere — are drawn after opaque geometry in their analytic altitude order,
test depth and do not write it, so cloud shells at least 1 km apart and rings need nothing more, and
the shadows between ring and planet are computed by ray–sphere and ray–plane tests, not from depth.
The one real hazard is an ocean's water thickness: taken as scene depth less ocean depth, its error is
0.12 m at 1,000 km, so beyond 10⁵ m the thickness comes from the bathymetry, and a lower layer is
culled once its separation falls below 10⁻⁶ of the camera distance. The shoreline, where the
separation goes to zero, shimmers at any precision and is handled by shading (foam, depth fade). At
range the terrain's level-of-detail error, metres to kilometres, dwarfs the depth error, and that is
cured by skirts or stitching, not by bias. `frag_depth` writes stay unneeded.

### Luminance in physical units, and an exposure model

A space scene has no ambient light and a dynamic range no display can show: the Sun's disc is about
2 × 10⁹ cd/m², and a magnitude-6.5 star whose light falls in one pixel is about 2 × 10⁻² cd/m²
(magnitude V = 0 is 2.54 µlx, Allen's value after Cox 2000 and Crumey 2014's zero point, which the
Bessell, Castelli and Plez 1998 calibration reproduces to 1% for an A0 spectrum; a pixel is
3 × 10⁻⁷ sr at 1080p across a 60° horizontal field, and a quarter of that at 4K, so point sources
brighten with resolution while discs and surfaces do not). That ratio, 10¹¹ between a faint star's
pixel and the Sun's disc, is the span within one frame near the Sun. Across scenes the range runs
from about 10⁻⁵ cd/m², the galactic band's faintest background seen from the Sun's radius, to about
3 × 10¹¹ cd/m², the centre of an O star's disc. A renderer that works in arbitrary units will either
blow out the sunlit side of every planet or lose the star field entirely, and the usual game trick of
authoring light values by eye is not available, because the simulation knows each star's luminosity
and every body's albedo and will happily state them on a console.

**Lean:** the renderer works in **absolute photometric units** end to end. The simulation's
luminosity is bolometric, so illuminance at a body follows from each star's absolute V magnitude
(`stellar::photometry::absolute_magnitude_v`, through its bolometric correction) by the inverse
square law and the extinction. V tracks photopic illuminance to within 0.08 mag from O5 to M6
(Pickles 1998 spectra against the CIE 1924 curve), below the bolometric-correction tables' own error.
Surfaces are shaded in cd/m² by a per-channel BRDF whose visual geometric albedo and phase integral
are the body's; the Bond albedo plan 14 derives is bolometric and belongs to the energy balance, not
the image. The camera then has a real exposure model — an EV100, metered from the scene under `AUTO`
or set from an aperture, shutter and sensitivity triple under `MAN` — and what reaches the display is
the tone-mapped result. This is more work than it sounds only once: after it, sunrise on a planet, a
dim red dwarf's light and a magnitude-6.5 star all come out at the right relative brightness without
tuning, which no amount of per-scene tweaking would achieve.

The units hold in the shading arithmetic, where `f32` spans the range with room to spare, not in the
stored buffer. HDR colour targets are `rgba16float`, whose largest value is 65,504 and smallest normal
6.1 × 10⁻⁵, some 30 stops at full precision; the Sun's disc alone is 3 × 10⁴ times the maximum.
**Lean:** values are pre-exposed by the previous frame's exposure before they are stored (Filament
§5.2.6, "Pre-exposed lights", as in Unreal and Frostbite), which slides those 30 stops over the
current scene. A star's disc beyond the window is clamped to the format's maximum before storage and
saturates as it would on a sensor, or is drawn analytically with its bloom energy handed to the glare
pass. `rgba32float` is available on the UHD 620, with `float32-blendable` and `float32-filterable`
both exposed, but it would double the bandwidth of a bandwidth-bound GPU for no visible gain, so it is
not used for colour targets. Bloom and other intermediates without alpha may use `rg11b10ufloat`
where `rg11b10ufloat-renderable` is present.

It also gives the cockpit something real to show. Exposure is a _camera_ property, so it belongs on
the display as a control with a value, which is the instrument
[What the guide must gain](#what-the-guide-must-gain) proposes in its sixth item, and it explains to
the player why the night side of a planet is black when the camera is exposed for the day side.

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
10⁸ m, or a body's position jumps when the ship crosses a frame boundary (a system's tidal radius or
a body's Hill sphere), the wireframe shows it against a grid with nothing else to blame. Every one of
those failures is much harder to diagnose once there is terrain, an atmosphere and a cloud layer in
front of it.

So the order is: foundations and wireframe first, proven at real scale, and only then anything lit.
This is also the order the single-player brainstorm's plan of attack implies, and it is the reason its
step 2 is "the rendering engine, and `VIEW` as a wireframe at real scale" rather than a lit scene.
The order is not a succession, though: the wireframe stays as a mode when the lit scene arrives, as
[Render styles and multiple views](#render-styles-and-multiple-views) sets out.

One addition to what it draws, which follows from having a photometric pipeline from the start: the
wireframe's stars should already be **exposed**, not drawn at arbitrary brightness. Stars plotted at
their true apparent magnitudes through a real exposure model, even on a wireframe, immediately tell
you whether the photometry is right — and a star field with the local star's disc in frame is the
cheapest possible test of the 10¹¹ between a faint star's pixel and the Sun's disc. Near the Sun the
field alone spans some eight magnitudes, about three orders of magnitude from Sirius to magnitude 6.5,
and it is the disc that takes it to eleven. In the nuclear disc, at 10–16 systems per cubic
light-year, the nearest system is typically 0.2–0.25 ly away (one within 0.1 ly in about one sky in
twenty) and is most likely an M dwarf at about magnitude 0. The brightest star is usually a giant a
few light-years off, at a median V of about −7.3 and brighter than −8 in about one sky in four, with
some 10⁴ stars brighter than magnitude 0 (the Milky Way fixture at generator version 15). Against a
naked-eye limit of about 5.4 that is five orders of magnitude in the field, and six in the rarer
skies with a giant within a light-year.

## Render styles and multiple views

The owner has ruled, on 2026-09-28, what this effort ends in: a **free-flying camera** that switches
between a **wireframe** style, in the graphic language of an Artemis: Spaceship Bridge Simulator
console, drawing vector hulls, graticule bodies, orbits and symbology, and a **photorealistic** one,
fully rendered like No Man's Sky. The bodies, orbits and perspective are HYPERION's own, drawn in
that language; only the style is Artemis's. Consoles need both. So the view is one renderer with two
styles, serving two deployments of equal standing in the design, of which the single-player one is
built first. The section sits here because everything after it — terrain, sky, budget — is chosen
per camera and per style, and the reader should know that before the planets.

### Two styles of one renderer

**Lean:** wireframe and photorealistic are two _render styles_ of one renderer, over one scene
description, one camera model and the same [real-scale foundations](#real-scale-foundations):
camera-relative differencing, reversed-Z and frame rebasing. A style chooses which passes run and
how marks are stroked; it owns no scene, camera or projection. A view therefore switches style
without rebuilding its scene, and a body projects to the same pixel in both. The style is chosen
per client and per display. The wireframe is kept as a permanent mode, no longer only the first
milestone, though it remains the precision rig of
[the view before the planets](#the-view-before-the-planets).

The wireframe style draws bodies as graticule spheres true to scale; terrain, where surveyed, as
contour or grid lines from the same height function and coarse field the lit style displaces its
patches with, so the two cannot disagree about the ground; orbits; craft as hulls from the hull
definitions; stars as points by magnitude; and the guide's symbology. Its terrain is drawn once,
depth only, so that hidden lines stay hidden, with the contours evaluated in the same pass from the
interpolated height. It selects patches at a screen-space tolerance of about 4 px rather than the
lit style's 1 px, and needs no normals, so it wants about a sixteenth of the patches and none of
their normal margins; but it still takes them from the height function, because the coarse field
alone resolves the ground only from beyond about 2 × 10⁷ m, where a 35 km cell is 4 px or less, and
a helm needs the ground below that. The patches under every grounded body in view are drawn at the
finest level in this style too, so that the helm's picture of the ground at touchdown agrees with
what the hull meets.

Its stars are exposed, by true magnitude through the exposure model, because that is the cheap
photometry test; exposure and the tone curve are applied per star sprite, not as a full-screen
pass. A wireframe view has no image to meter, so it takes the exposure of the photorealistic view
it accompanies where there is one, as the cockpit's instruments do, and otherwise a manual value,
shown as `MAN` under item 6 of [What the guide must gain](#what-the-guide-must-gain). Its lines are
not exposed: there is no photometric image beneath them, so their colours, widths and contrast obey
the UX guide, as that section sets out. The two can be blended, the wireframe's symbology, orbits
and hull outlines over the realistic image, under the rules that already govern symbology over the
image: graticules and contours there are cased like any mark.

### Two deployments, one scene

The deployments differ in composition — which clients, which views, who commands the camera — and
not in the renderer.

- **Bridge crew.** One dedicated machine is the ship's window: the **main screen**, a client role
  of its own, full-screen, driven to a television or projector, photorealistic, and usually on the
  best GPU in the room. It carries no console chrome, but not nothing: a status line, the mode
  banner, an alert annunciator and the view's label block, which item 9 of
  [What the guide must gain](#what-the-guide-must-gain) defines. The stations run on their own
  machines, and the helm, for one, draws the same surroundings as a wireframe, from a camera of its
  own or slaved to the main screen's. What the main screen shows is commanded from a station.
- **Single-player**, as in Elite Dangerous or No Man's Sky, on one machine that may be the UHD 620
  or a discrete GPU, with the server local. The photorealistic view is the primary, full-window
  cockpit view, and the wireframe style appears in the same client as cockpit instruments, such as
  a helm or navigation display beside or over the view. The player commands the camera locally.

**Lean:** every client draws from the same server scene, so the main screen and the helm's
wireframe agree on the bodies, their positions, the frame and the survey coverage, since Knowledge
belongs to the ship and not to a client. Their times stay aligned because every push of the scene
states a simulation time and a time rate, and each client renders at that time extrapolated by its
local elapsed time multiplied by the stated rate, so two clients differ only by the delivery of the
latest push. One scene in two styles is what guarantees the agreement.

On a bridge the discrete-GPU target applies to the main-screen machine, at a television's or
projector's resolution that may be 4K. The budget is stated at 1080p, and 4K costs about four times
that in fill-bound passes, so the main screen may render below native and upscale. Point sources
brighten per pixel with resolution, as
[the luminance section](#luminance-in-physical-units-and-an-exposure-model) notes, so the star
sprites take the pixel's true solid angle. Station machines may be UHD 620-class laptops, where the
wireframe is the natural style. Its target there, 60 fps at 1080p, is a lean of this document beyond
the owner's "playable" floor, and it gets a row of its own in [the budget](#performance-budget) and
in the performance runs. By estimate it fits: the depth-only terrain pass at a 4 px tolerance, the
lines and symbology, and line antialiasing come to about 4 to 9 ms of the 16.7 ms frame, a
reasoned figure that the performance runs replace. A station runs no server, so about three of its
four cores serve height workers, some 75 patches a second at about 40 ms a patch, against the 6 to
20 a second the wireframe's tolerance asks on a low fast pass. Each station that draws terrain
receives the surveyed coarse field, 2 to 15 MB a planet, about 0.1 s on gigabit Ethernet and a few
seconds on Wi-Fi once per arrival, and runs its own height workers.

### The free camera

**Lean:** the camera is not bound to the pilot's seat. It has its own `f64` position in the
simulation's frames, as frame plus offset, and its own frame selection with hysteresis, independent
of the ship's. That includes body-frame selection, which
[is not built](#the-floating-origin-is-already-in-the-simulation). Level-of-detail selection, patch
streaming and the sky bake follow the camera, not the ship. The patches under every grounded body in
view are still drawn at the finest level, wherever the camera is and in either style.

Its reach is anywhere in the current system and the scene the server sends, and it cannot see what
the ship does not know. Where the ship is in no system, in the Galactic frame, its reach is the
scene the server sends about the ship; when the ship leaves the system or jumps, a free camera
returns to the chase preset about the ship. The renderer stays a
[display sink](#runtime-and-code-shape), so moving the camera is not a sensor: unsurveyed ground
stays undrawn, and labelled, wherever it flies; a craft that is not a contact is not in the scene;
and a body is shown at the retarded time the ship sees it at, not the camera's. Whenever the camera
is off the hull the view says so in its label block: positions are as seen from the ship at its
light-time, not from the camera. Knowledge is not bypassed by moving the camera.

It has three presets: **seat**, the pilot's eye point, fixed to the hull and looking forward, aft or
at a target, and the default; **chase**, or external, held at an offset in the ship's frame; and
**free**, detached. A move between presets is a cut, as a view switch is in the guide and in flight
simulators, and so is a slew to look at a target. In single-player an eased move of 0.4 s is
offered as a setting, off by default and never applied under `prefers-reduced-motion`. The main
screen always cuts, because its watchers did not trigger the move. The free camera's own flight is
the operator's motion, not an animation; only its smoothing is non-physical.

The camera's state has one shape in both deployments: preset, target, style and, for the free
camera, a pose as frame plus offset. Who sets it, and what setting it means, differs. On a bridge
the main-screen machine may have no input device, so a station commands it. **Lean:** main-screen
control is granted per station, as EmptyEpsilon grants it. It goes by default to the Captain and to
Helm, and to Helm alone on a bridge without a Captain, as Artemis gives it to Helm and Weapons. Any
granted station may select the preset, the target and the style, each a ship command, drawn and
closed-loop as the guide requires. The last selection the server accepts stands, and the main
screen and every station show which station set it. Flying the free camera is exclusive: one
station holds it at a time, takes and releases it through the ship's control arbitration, and is
shown as holding it. The Captain can always take it.

**Lean:** the main screen's camera is ship state. The server holds it and integrates its free
flight from the commanding station's continuous input stream, as it does the stick's, and the main
screen interpolates between poses, as it does for craft. Every station can read the pose from the
scene topic, which is how a helm slaves its wireframe to it. From station input to the main
screen's pixel the loop is budgeted at 100 ms on a wired LAN, excluding the display device, whose
own lag, for a television outside game mode or a projector, may be larger (a recalled figure, to
be measured); discrete camera commands complete within 250 ms. The main screen's camera is saved
with the session. Its input is logged on a presentation track that a replay can show, but the
replay's bit-for-bit check does not cover it, because the camera changes nothing in the simulation;
its integration is presentation code in the server, outside `hyperion-sim` and its determinism
rules. A local view's camera, whether the single-player cockpit or a station's own wireframe, is a
display control: its client integrates it at display rate, because a server tick of 15.6 ms would
make mouse-look judder, and reports its position to the client's scene subscription, as plan 12's
`alerts_observer` reports the observer, so that the server can bound the scene. That pose never
becomes ship state and is not saved with the session.

None of this exists yet. It needs the sessions, ship state and closed-loop commands of the
single-player brainstorm's first step, and a client role, which `Hello` does not carry today, so
that the server knows a main screen from a station; the protocol's `ClientMessage` has only
`Hello`, `Ping`, `Request` and `Cancel`, and nothing identifies a station.

### Several views in one client

Single-player runs several views in one client as a matter of course: the cockpit view and its
wireframe instruments. Each view is a camera, a style and a target. **Lean:** they share one
`GPUDevice` and one resource cache. WebGPU lets one device drive several canvases, each through its
own `GPUCanvasContext` configured against it, so pipelines and the atmosphere's tables are built
once, and so is the sky cubemap wherever it is baked once per arrival; where it is re-baked as the
camera moves, in the nuclear disc and cluster, views whose cameras are far apart hold their own.
Streamed patches and height textures are cached by patch rather than by view, so views whose cameras
are near each other share them and views far apart select disjoint sets. Patch demand is the sum
over every view's camera, and on the UHD 620 a second streaming camera shares the same two worker
cores, so secondary views stream at lower priority.

Babylon's own multi-canvas feature, `registerView`, does not do this. It renders every view through
one working canvas, resizing it per view when sizes differ, and copies the result into each view's
canvas with a 2D `drawImage`. On the UHD 620 that copy costs about 4 to 5 ms per 1080p view
(measured on Chrome 152), and on Linux it cannot read a frame already presented. **Lean:** the
adapter configures one `GPUCanvasContext` per view against the engine's device (`engine._device`).
Each frame it wraps the view's `getCurrentTexture()` with `wrapWebGPUTexture` and
`updateWrappedWebGPUTexture` as the camera's `outputRenderTarget`, or as a Frame Graph import, as
Babylon's WebXR-on-WebGPU path does. Two internal names, `_device` and `_disableEngineYFlip`, are
pinned by an adapter test so that a Babylon upgrade that changes them fails loudly. Canvas contexts
are configured at each view's own size, so no view resizes another's attachments. The first step
proves it: one engine, a full-window cockpit canvas and two small instrument canvases, each with its
own camera, post-process chain and depth texture, rendered the right way up with no GPU time spent
in copies.

A `GPUDevice` belongs to one renderer process and cannot be posted to another, so separate windows
each hold a device and their own copies of pipelines, caches and the coarse field, which on the
UHD 620 all come out of shared system memory. The single-player deployment therefore draws the
cockpit and its instruments as canvases of one document on one device. If instruments move to
separate OS windows, which the single-player brainstorm's several windows allow, they are opened as
same-origin children, which Electron keeps in the opener's process, so the same device can serve
them; that is to be proved by a prototype before it is relied on.

Each view has its own budget. A secondary view renders at lower resolution or a lower rate, and on
the UHD 620's low setting one view is photorealistic and the rest wireframe. The wireframe views'
cost comes out of the photorealistic view's margin, which [the budget](#performance-budget) leaves
small, so the photorealistic view lowers its internal resolution while instruments are open, and the
local server shares the CPU as the budget says. Whether the cockpit and its instruments fit the
33 ms frame together is for the performance runs to show, measuring them together. Wireframe views
need no atmosphere, clouds, shadows or bloom, but where they draw terrain they pay for patch
streaming like any view.

### Still images

**Lean:** a console can request a single realistic image — a picture of a body, which surveys
nothing — rendered through the same pipeline, into an offscreen target, at a quality the live view
cannot afford: more samples, a higher resolution and the passes the low setting cuts. The client
that requests it renders it, so a station on a UHD 620 laptop pays for it in time rather than
missing it. It is rendered progressively, beside the live view: one tile or sample batch per
animation frame, submitted only after the previous one's `onSubmittedWorkDone()`, so at most one
frame of work is queued. The batch is sized from `timestamp-query` to take no more than about 6 ms
of GPU time on the low setting of the development machine and a quarter of the frame on the target,
and no single draw or compute workgroup may exceed about 50 ms on any machine. The limits are the
drivers': i915 resets the GPU when a batch cannot be preempted within 640 ms, Gen9 preempts only
between draws and between workgroups, and Windows' default `TdrDelay` is 2 s (not re-checked). A reset loses the device for
every view and the consoles, and three such losses disable WebGPU for the session.

It is data, so it is labelled with its time, its camera — frame, position, field of view and
exposure — its setting and whether decoration is on, and it shows only what the ship knew at that
time. It is a product of the client that made it, kept in that client's store with its label; it
is not ship state and is not saved with the session.

## Planets

### What the generator already owes us

Plan 14 is partly built, the rest specified, and the surface generator should be written against
what it promises rather than inventing its own global statistics. From its `BodyRecord` and hooks:
radius and mass, hence surface gravity; bulk composition as iron, rock, water and envelope fractions;
atmospheric composition as ordered gas fractions with a surface pressure; equilibrium and surface
temperatures with day–night and equator–pole contrasts; Bond albedo, iterated against the surface and
cloud state; rotation period, obliquity and rotation phase at the epoch, with a `BodyFixedFrame` of
pole, prime-meridian angle and rate; ocean fraction, ice fraction (from the latitude at which the
zonal temperature crosses freezing, which [open question 9](#open-questions) finds wrong for several
classes of world) and cloud fraction; a greatest relief of 20 km × (g⊕ ÷ g) × a lithosphere factor;
heat flow, and from it a tectonic regime and a volcanism level; surface age, and from it a crater
density by the Neukum–Ivanov–Hartmann chronology, scaled by the system's belt masses and zeroed for
small craters under a thick atmosphere; rings and belts as bodies in their own right. Of these, the
radius, mass, gravity, bulk fractions, equilibrium temperature, the Bond albedo's iteration (P14.T12
and T13) and the rings and belts are built. The atmosphere — its surface state, surface temperature
and pressure, partial pressures and cloud fraction — and the Bond albedo are derived (T13) but
carried by no field yet: the albedo has none in the bulk section, and the record's surface section
is still an empty type. Rotation and the `BodyFixedFrame` (T14), the surface seed (T23) and the
surface conditions and global figures (T24) are still to come. Two more of its outputs the coarse
pass can read directly, and a renderer once the surface section carries them: the atmosphere
derivation's `SurfaceState` — gas envelope, magma ocean, airless, runaway greenhouse, temperate or
snowball — with, for an airless world, its `SurfaceMaterial`, rock or ice from the bulk water
fraction; and T31's body events, also still to come, such as eruptions and global dust storms, as
visible phenomena.

And, crucially, a **`surface_seed`**, specified as a block output of the universe seed and the body's
ID alone, on its own `body.surface` stream, depending on nothing else, so that no later change to
any other derivation can alter a world's seed. That protects the random draws behind a map, not the
map itself: the map also realises plan 14's global figures, so a later change to how relief or ocean
fraction is derived changes the map too, and is a generator-version change like any other.

The surface generator is therefore a _consumer_, and its contract is narrow: given the surface seed
and those global figures, produce a height and a material class at any point on the sphere, plus the
coarse fields the renderer and the climate need. Both are authoritative: the material is decided
here, with the physical properties landing and the consoles read, and is baked beside the height,
never re-decided by a shader ([Materials](#materials-and-what-a-surface-looks-like)). It does not
decide how much relief a world has or whether it has an ocean — plan 14 already did, from physics —
nor whether it has plates, how cold its poles are, how much of it is ice, or how cratered it is.
Wherever the coarse pass computes one of those figures for itself, as the climate model does, the
result is constrained to plan 14's value rather than allowed to drift from it. This division is what
keeps the terrain defensible: the numbers a console states come from the astrophysics, and the
terrain merely realises them.

### The geometry: a quadtree on a cube sphere

The choice is narrowed sharply by one fact about the API: **WebGPU has no tessellation stage**, and no
mesh shaders. Hardware subdivision is simply unavailable, so any scheme that depends on it is out
before its merits are weighed.

| Scheme                                            | Verdict                                                                                                                                                                                                           |
| ------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Quadtree on a cube sphere, fixed-grid patches** | **The answer.** Six root quadtrees, one per cube face; each leaf a fixed grid of quads displaced along the radial direction. One grid serves every patch, and the topology is predictable.                        |
| CDLOD (Strugar 2009)                              | Its contribution is kept: continuous morphing between levels in the vertex shader. Its crack-free guarantee assumes one heightmap shared by every level, which band-limited levels are not, so height morphs too. |
| Geometry clipmaps (Losasso and Hoppe 2004)        | Fixed memory and few draws, but camera-centred rather than body-centred, which is awkward when several bodies are in view. Rejected.                                                                              |
| Hardware tessellation                             | Not available in WebGPU or WebGL2. Rejected by the platform.                                                                                                                                                      |
| Mesh shaders, Nanite-style virtual geometry       | WebGPU has no mesh shaders; a Nanite-style compute rasteriser would run on it, but is the wrong tool: terrain's structure is known, and its height is cheap.                                                      |
| ROAM                                              | Historical. CPU-bound triangle bintrees lost to GPU throughput two decades ago.                                                                                                                                   |

Cube-sphere distortion is the known cost, and it is accepted because the compensation is what we
want everywhere else: square parameter domains, trivial quadtree refinement, and square textures. Its
distortion is well characterised, in the planetary-rendering literature and in Google's S2 geometry
library, which indexes the Earth on a cube sphere and gives the ratio of largest to smallest cell
area for each warp of the face coordinates. On the plain gnomonic cube it is about 5.2, with the
corner cells the small ones; the tangent (equiangular) warp brings it to 1.41, and S2's default
quadratic warp to 2.08. **Lean:** the quadratic warp. It is a per-axis map, so cell edges stay great
circles and a cell's solid angle stays closed-form. Forward and inverse use only `sqrt`, which IEEE
754 rounds exactly, and S2 finds it about three times cheaper than the tangent warp, which the
pinned `libm` would also make deterministic. COBE's near-equal-area cube is out: its polynomial
inverse misses by up to 24″ (Calabretta and Greisen 2002), some 0.7 km at an Earth's radius, and the
point query must round-trip. Cell sizes quoted below are means.

**Lean:** cube-sphere quadtree on the quadratic warp; leaf patches of 64 × 64 quads, 65 × 65 vertices
(the figure to start from and measure; an even number of intervals, so that every other vertex lies
on a vertex of its parent); displacement along the radial direction in the vertex shader from a
per-patch height texture; vertex morphing between levels for continuity; per-patch `f64` origin,
fixed in the body's rotating frame, as in [Real-scale foundations](#real-scale-foundations). Height
textures are generated once per patch and cached, not evaluated per frame — the same trick
SpaceEngine uses.

Morphing positions alone does not close cracks here. CDLOD morphs grid positions over one shared
heightmap, but each level here is band-limited differently, so a child's edge vertex moved onto its
parent's position still carries the child's height. Across the morph zone each vertex therefore
blends to the parent level's band-limited height, which the patch carries as a second channel of its
height texture; neighbouring patches differ by at most one level; and skirts stay as a cheap guard
against `f32` hairline cracks along cube-face edges. Precision sets two more rules. The vertex shader
never forms (R + h)·dir in `f32`, which at an Earth's radius has a step of 0.5 m, coarser than the
finest vertex spacing: each point is formed relative to its patch origin, either from small `f32`
differences of face coordinates or from offsets the worker computes in `f64` and bakes with the
patch, whichever measures cheaper. And the height texture holds height above the reference radius,
not radius, so its `f32` step at ±20 km is about 2 mm.

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
more, which is neither a cheap transfer nor a cheap simulation. A fixed level cannot serve every
body, because cell size moves with radius: level 8 is about 36 km on an Earth but 10 km on the Moon
and 2.7 km on Ceres. **Lean:** one cube-sphere quadtree level per body, the shallowest whose mean cell
is no wider than about 40 km, floored at level 5 so that a small body still has a grid and capped at
level 8 for the transfer. That is level 8 for an Earth (393,216 cells of about 36 km) and for a
2 R⊕ world (cells of about 72 km, where the cap binds), level 7 for a Mars (98,304 cells of about
38 km), level 6 for the Moon (24,576 of about 39 km) and level 5 for Ceres (6,144 of about 21 km),
and **roughly 2 to 15 MB** from a Mars to an Earth. The simulation that produces it is then of order
10⁷ to 10⁸ cell updates across its iterations, which is of order seconds to tens of seconds of Rust
rather than minutes, with erosion and the climate model dominating: erosion alone is a few seconds at
level 8 (step 4), and the climate model's published runs take 2 to 20 s each. Both figures need
measuring rather than trusting, but they are the right order for "computed when a ship arrives", if
the pass starts while the ship is still on approach. Everything finer than a coarse cell is the local
synthesis's job.

What the pass does, in order:

1. **Plates, where plan 14's tectonic regime has them.** A mobile-lid world is tessellated into some
   tens of plates, each given a motion. Spherical Voronoi is the standard construction: Procedural
   Tectonic Planets built it with Renka's STRIPACK, a general-purpose spherical Delaunay package, and
   the convex hull of the points on the sphere, which is their Delaunay triangulation (Brown 1979), is
   the modern method. With tens of plates on a fixed grid neither is needed. Each coarse cell takes
   the nearest plate seed, by the largest dot product, with ties broken by seed index and the
   distance warped by low-frequency noise so that boundaries are not great-circle arcs: the diagram
   rasterised onto the coarse field's own cells, deterministically, in milliseconds. Plate boundaries
   are then cell adjacencies, and a distance transform over the cells gives each cell its signed
   distance to the nearest boundary and that boundary's type. Each boundary is classed by the plates'
   relative motion across it, and the landforms that read as geology are built from that — mountain
   belts at convergent boundaries, ridges at divergent ones, island arcs at subduction — following
   the procedural-tectonics line of work rather than a full geodynamic simulation. A stagnant-lid
   world, as Mars is and as Venus is usually modelled, gets no plate boundaries: its large-scale
   structure is one lid, with volcanic provinces sized by plan 14's volcanism level and impact basins
   where its crater density puts them.
2. **Coarse elevation**, with continental and oceanic crust distinguished, scaled so that the
   realised surface's **hypsometric standard deviation** σ_h, the area-weighted RMS elevation about
   the mean, equals plan 14's. The coarse pass targets σ_h² less the local synthesis's expected
   variance, and because variances add across scales the match holds at every resolution. Little
   rides on the local share: below 35 km Earth, Mars and Venus carry only about 0.1–0.2% of their
   elevation variance, some 35–125 m RMS, and the Moon about 2%, most of it craters (computed from the
   Earth2014, MarsTopo2600, VenusTopo719 and LOLA spherical-harmonic models, Venus's by extrapolation
   from 53 km). Plan 14 publishes a greatest relief today, an extreme-value statistic that grows with
   resolution and with whichever single feature is tallest, so no finite realisation can be made to
   match it; it is reported from the realised field instead, and plan 14 is asked for σ_h. Its gravity
   scaling needs a ruling as well: 20 km × (g⊕ ÷ g) gives about 53 km for Mars and 121 km for the Moon,
   against some 29 and 20 km observed, unless the lithosphere factor absorbs the difference. The ocean
   surface is placed to match plan 14's ocean fraction rather than chosen by eye. Craters wider than
   the boundary diameter of step 6 are placed here, from plan 14's crater density, because they are
   coarse-scale features that climate and drainage must see.
3. **Climate**, by the model the world's regime calls for ([open question 9](#open-questions)). For
   the common case a seasonal two-dimensional energy-balance model gives the monthly surface
   temperature — monthly because the climate classes below are defined on monthly climatology — and
   there is one published specifically for the climates of rapidly and slowly rotating _terrestrial
   planets_ (Ramirez 2024), which is exactly the generality needed. It steps through the orbit at
   six-hour steps on a 5° × 30° grid, or finer in longitude for a locked world, in seconds. It
   prescribes humidity and computes no precipitation, and the standard reference text on these models
   warns outright that precipitation cannot be solved by simple models. So precipitation is a
   documented monthly heuristic, labelled as a heuristic wherever a console shows it: an equatorial
   rain band centred each month on the model's energy-flux equator, dry belts and a storm-track band
   beyond it, amplitudes scaled so that global precipitation balances evaporation from the energy
   budget (after Siler, Roe and Armour 2018), and an orographic correction with rain shadows from the
   coarse elevation under the month's prevailing wind (Roe 2005). No published precedent for it in
   procedural world-building was found, so it is checked offline against a fast general circulation
   model, ExoPlaSim, which needs hours to reach equilibrium and so is never run on arrival. Both
   monthly fields are interpolated from the model's grid to the coarse cells, and the lapse-rate term
   is re-applied after step 4 changes the elevation. The model's temperatures are normalised to plan
   14's mean surface temperature and its day–night and equator–pole contrasts, so that the map and the
   console state one climate, and ice is placed where the model is coldest until its area matches plan
   14's ice fraction, as the ocean surface is placed to match the ocean fraction.
4. **Drainage and erosion**, where the world has, or had, liquid at its surface. The stream-power law,
   solved by the analytical method of Tzathas et al. It is closed-form in time but not in space: it
   needs a receiver for every cell, a sort from ridges down to the outlets to accumulate drainage
   area, a pass back upstream, and iteration to a fixed point sped by multigrid, which is exactly why
   it runs here, on the coarse grid on the server, and never per point. On a world wet now it runs to
   steady state, with the sea as base level and uplift from the plates. On a world dry now it runs for
   the effective length of plan 14's wet epoch, and only where the surface is older than the epoch's
   end. Mars's valley networks formed within a few hundred million years around 3.7 Ga, from perhaps
   10⁵ to 10⁷ years of active flow, and their volume is at least about a metre of rock spread over the
   planet (Hoke and Hynek 2009; Luo, Cang and Howard 2017): a thin, immature overprint, with the
   craters of the rest of the surface age placed on top of it. The base level then comes from a
   depression-filling pass. A past inventory above the basin capacity places a sea at the paleo-ocean
   fraction, by the same logistic of inventory over capacity that plan 14 uses for the present ocean;
   a smaller one fills and spills closed basins, as Mars's crater lakes did, and those that stay
   closed are terminal outlets. On a stagnant lid the uplift is volcanic construction and the
   flexural trough and bulge around each load, which over a short wet epoch are the initial surface
   rather than a rate. A world with no wet epoch, such as the Moon, Mercury or Venus now, has no
   fluvial step. The result is rescaled to plan 14's σ_h as in step 2. It gives the final coarse
   elevation, flow directions, drainage area and a channel-steepness index. Its published timings,
   1.79 s at 512² cells and 8.18 s at 1024², are for Python with its loops compiled by numba on one
   Xeon E5-2650 v4; they bracket level 8's 393,216 cells at about 3 s, and compiled Rust may gain one
   to three times over numba, so erosion stays of order seconds. This is the step that buys the
   realism, because it is computed globally where global is affordable, and everything local is then
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
   atmosphere. The pass adds a saturation level, and craters are split at one diameter, D_b, twice
   the coarse level's largest cell edge (about 100 km on an Earth at level 8). Craters of D_b or wider
   are the coarse pass's: smoothed into the coarse elevation at step 2, so that climate and drainage
   see them, and sent with the field as an explicit list of centre, diameter, morphology and
   degradation, which is short — a heavily cratered Moon has a few hundred above 100 km. The local
   pass sharpens their rims from that list and generates every smaller crater itself. Mars, under a
   thin atmosphere, is as cratered as its surface age says; Venus loses only its small craters.

**The grid.** HEALPix is equal-area with isolatitude rings and subdivides hierarchically, which is the
better physics grid; the cube sphere is the better rendering grid and is already the geometry. **Lean:**
the coarse field lives on the cube-sphere quadtree at the body's fixed shallow level chosen above, so
that no second spherical indexing scheme exists and a patch's lookup into the coarse field is an
ancestor lookup. Equal-area sampling is a real loss for the climate model and the honest mitigation
is to weight cells by their true solid angle, which the quadratic warp's great-circle edges keep
computable in closed form.

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
2. **It shrinks the determinism surface a great deal.** An iterative plate-and-flow simulation is
   exactly the kind of floating-point code most at risk of diverging between native x86-64 and
   WebAssembly: accumulation order, transcendental functions, fused multiply-add. If only the server
   ever runs it, that risk leaves the client-server agreement problem, though not the server's own: a
   save recomputes the field rather than storing it ([open question 4](#open-questions)), and
   surveyed ground cannot move, so the coarse pass stays under the sim's full determinism discipline
   on every machine a server may run on. What must match between client and server is only the
   **local** synthesis — a pure function of the coarse field, a position and a detail seed, with no
   iteration — which is a far smaller and more testable surface.
3. **It is the Knowledge overlay's natural answer.** The coarse field is precisely "what the ship has
   established about this world from orbit". It arrives because the ship surveyed it, region by
   region, through the Knowledge overlay. The alternative — handing the client a seed from which it
   can generate the whole planet — is the awkwardness the single-player brainstorm flagged and left
   to this document.

One constraint follows from reasons 2 and 3 together, and it must not be lost. The local synthesis
agrees between client and server only if its inputs do, so **Knowledge gates how much of the coarse
field the client holds, never how accurate it is.** A surveyed region arrives as the server's exact
cells, whole, with a margin of neighbouring cells as wide as anything the per-query evaluation reads
— the interpolation, the river network's neighbourhood, and the reach of the largest local crater,
about D_b or two coarse cells — and with every coarse crater whose reach touches it, so that the
synthesis at a region's edge has the same inputs on both sides; an unsurveyed one does not arrive at
all. A degraded copy — smoothed, quantised harder, or resampled at a sensor-limited resolution —
would feed the client's synthesis different inputs, and the ground it drew would stop being the
ground the server collides with. If the field is quantised for the wire, the server's own synthesis
reads the same quantised values. What the client holds is one question and what a readout may quote
is another: Knowledge gates coverage for the image, and coverage and resolution for every number
([Knowledge, and the surface seed](#knowledge-and-the-surface-seed)).

### The per-query evaluation

Per height query, on both sides, with no iteration and no global state:

1. Find the coarse cell and read the fields around it: continuous fields interpolated across the
   sphere, taking the neighbouring face's cells across a face edge, and categorical ones — plate,
   crustal type, climate class, flow direction — read from the nearest cell or blended by weight.
2. **Base elevation** from the interpolated coarse elevation.
3. **Structural detail** conditioned on crustal type and on the signed distance to the nearest plate
   boundary and its type, which the coarse field carries because nothing below a coarse cell could
   recover them from cell identities: ridged multifractal for a mountain belt, low-amplitude for an
   abyssal plain, domain-warped where a boundary is oblique.
4. **Channels below the coarse cell.** A point given only its cell's drainage area cannot know its
   own path downstream, so the channels come from a Dendry-style network (Gaillard, Benes, Guérin,
   Galin, Rohmer and Cani 2019): jittered key points in hashed integer cells. At the first level
   each joins its lowest Moore neighbour under the control function, here the server's coarse
   elevation and flow directions; at every later level each joins the nearest segment of all coarser
   levels, found by distance in at least a 5 × 5 neighbourhood, with elevations held to a minimum
   slope by level. The published network has four levels, so the span from a coarse cell to the band
   limit takes several instances stacked, each conditioned on the last, their number set by the
   body's coarse cell size. The reference code's standard-library generator is replaced by the
   project's `Stream`, opened from the detail seed on a registered tag such as `surface.channel` and
   keyed by a new `ObjectKey` constructor of body, face, level and cell. The profiles follow the
   steady-state stream-power slope, S = k_s·A^−θ, with the drainage area A from Hack's law and the
   steepness k_s from the server's solution; Hack's constants are Earth's, and overstate how
   integrated a network is on a world whose wet epoch was short. The network's geometry is procedural
   and labelled so; the profiles are physics. Incision only ever cuts down, so each channel level's
   incision has its mean over its parent cell subtracted, which, since the cells nest, zeroes it over
   the coarse cell too; otherwise the rule of
   [Level-of-detail consistency](#level-of-detail-consistency-and-why-collision-agrees) breaks.
5. **Craters** below D_b, wherever plan 14's crater density is not zero, by sparse convolution. A
   single hashed cell holding every diameter cannot work: at a cumulative size–frequency slope of −2
   or steeper, a fixed cell holds some 10⁹ times more 1 m craters than 35 km ones, and a crater that
   crosses a cell edge is found only if the search reaches its rim and ejecta, out to about twice its
   radius. So each diameter octave has its own quadtree level, with cells at least as large as that
   octave's reach, and a query searches the 3 × 3 neighbourhood at each octave's level, across face
   edges, where a cube corner has seven neighbours. Each cell draws its count from the density at its
   own canonical point, weighted by its true area and capped at saturation, and each crater a diameter
   within the octave, from the project's `Stream` on `surface.crater`, then a jittered position and a
   morphology by diameter — simple bowl, complex with a central peak, or multi-ring basin. Both passes
   draw diameters by inverting one shared function in the sim, plan 14's cumulative crater density,
   which T24.b does not yet provide, since it serves only N(>1 km). It must carry the production
   function's shape (Neukum, Ivanov and Hartmann 2001, 10 m to 300 km) scaled from N(>1 km); an
   atmospheric cutoff with a taper rather than a hard zero, from screening — a projectile stops once
   it has met its own mass of air, about 5 m on Earth, 0.5 km on Venus and 8 cm on Mars — raised by
   break-up, as Venus's smallest craters of 1.5–2 km show; the equilibrium saturation; and the
   simple-to-complex transition, which scales as 1/g. The number of octaves moves with the body's
   D_b. The cost is about nine cells per octave, which is still why the crater field is baked into
   the patch's height texture rather than evaluated per frame.
6. **Band-limit.** Sum only those octaves the current level of detail can resolve, a set fixed per
   level and body.

That last step is not a performance trick. It is what makes collision agree with the picture, and it
has its own section below.

The query defines the height at every vertex; it is not how a patch is made. Evaluated point by
point, the channel network alone is far too slow for a descent
([open question 5](#open-questions)), so a patch builds each channel level and crater octave once
for the cells that reach it, caches them by integer cell for its children and neighbours, and
evaluates each vertex against only its own cell's lists. The cache belongs to its caller, a height
worker or the server, and is tested order-independent, so that no build order can change a height.
Contributions combine by minimum or maximum, or are summed in a fixed order of level and then
canonical cell, never in rasterisation order, so the result matches the single-point query bit for
bit. The band limit makes coarse patches cheap but not the finest, which still need every level.

### The line between truth and decoration

GPU decoration is how a planet gets pebbles without the server knowing about pebbles, and the line has
to be drawn explicitly because safety in the fiction depends on it. Two words are kept apart from here
on: _amplification_ is the authoritative local synthesis above, run on both sides, and _decoration_
is what only the GPU draws.

**Lean:** the authoritative height function — sim code, run identically on both sides — owns everything
the ship can collide with. The terrain is band-limited at **2 m** and sampled at its finest level at
a vertex spacing of at most 0.375 m (level 19 on an Earth, 0.18–0.32 m), so that its
piecewise-linear interpolant keeps a 2 m wavelength to within about a third of its amplitude in any
direction on the mesh's triangles; 0.5 m would do so only along the grid's axes, and a wavelength of
twice the spacing sits at the Nyquist limit, where linear interpolation can erase it altogether.
Obstacles smaller than the band limit are not terrain but rocks, below. The limit is one generator
constant, not tied to the size of the body that touches the ground, since two bodies on the same
spot must meet the same surface, and it changes only with the generator version. The same function
owns the **material class** at every point, from slope, altitude, the coarse climate field and plan
14's ice fraction, with the physical properties that landing and the consoles read: albedo
range, friction, bearing strength. Everything finer is **GPU-only decoration**: high-frequency normal
detail, colour variation, sand ripples and small crater scars. It never displaces geometry the
collision query does not know about, so the surface a hull touches is always the surface both sides
computed. Because decoration is not the simulation's, the view says when it is on, as
[the guide's edits](#what-the-guide-must-gain) require.

**Scatter** — rocks and vegetation as instances — splits by height, not by the band limit. Every rigid
instance **0.2 m tall or more**, whatever its footprint, is authoritative from the first phase that
draws them: placed by hash in cells per size octave, with its abundance from a rock size–frequency
law (Golombek and Rapp 1997), whose rock abundance comes from the surface type and age, and with an
analytic shape, an ellipsoid or a low-order superquadric, that the server answers for at contact
points. A finest-level patch, 17.7 m across on an Earth, at the rock abundance of the Viking and
Pathfinder sites holds some tens of them, a cheap per-patch list. The threshold sits below the 0.3 m
hazard that landing-hazard detection is specified against (Epp and Smith 2007, for NASA's ALHAT),
because a rock is a hazard well below a gear's relief tolerance: Apollo's lunar module pad was about
0.9 m across and its gear tolerated 0.6 m of relief within the footprint, but its engine skirt
cleared only about 0.34 m, and InSight tolerated rocks up to 0.45 m under a footpad. A rock drawn
but not collidable would be exactly the geometry the rule above forbids, and the converse holds as
well: an authoritative rock is drawn wherever it could touch a grounded or descending body, whatever
the scatter setting. Smaller scatter is decoration, flagged as such, and culled where it intersects
a grounded body ([open question 11](#open-questions)).

Two consequences worth stating. Normals should come from **analytic derivatives** of the height
function rather than finite differences: there is no arbitrary epsilon to tune, and they stay stable
across levels of detail, which is what stops shading from popping as patches subdivide. They are baked
with each patch into a normal texture at twice the mesh's resolution, so that shading keeps detail the
mesh does not (at the mesh's resolution on the low setting, as the memory table has it), and an
analytic gradient costs of order two to three bare height evaluations, which the
[budget](#performance-budget)'s cost per point must include. And scatter placement is drawn from the
project's `Stream` on `surface.scatter`, keyed by integer cell, which is what lets the server answer
"is there a boulder here" without storing one.

### Level-of-detail consistency, and why collision agrees

If the rendered surface at one level and the collision height at another disagree, a ship either
floats or sinks into the ground, and the bug is intermittent and miserable. The fix is a property of
the height function rather than a reconciliation step: **a coarse evaluation must be a smooth
low-pass approximation of a fine one.**

Summed-octave noise approximates this when it is built for it — each octave is roughly a frequency
band, so truncating the sum is roughly a low-pass filter, which is the observation the original
eroded-fractal terrain work rests on. Perlin and simplex octaves are only approximately band-limited
(Lagae et al. 2010), and step 3's noise types need care: ridged multifractal octaves have a non-zero
mean and are weighted by the octave before, and domain warping lets fine octaves move coarse
evaluations. Built for it means each octave's mean removed, ridged terms included; warp offsets taken
only from octaves no finer than the level being warped; the set of octaves per level fixed per body
from the level's largest cell, not switched on by the local vertex spacing, which varies across a face;
and each new octave faded in across the morph zone rather than switched. The rule that follows: every
contribution to height must be band-limited and attributable to a level, and nothing may be added at
a fine level that changes the mean at a coarse one. Craters are the awkward case, because a crater is
not a noise band; a crater must therefore appear at the level whose resolution can represent its
diameter, with its rim smoothed to that level's resolution so that it sharpens as it refines, and its
profile must integrate to the same displacement at every finer level. Channels are the other, because
incision only removes material, which is why each channel level's mean incision over its parent cell
is subtracted.

The test is then simple to state and should be written early: for a sample of positions on a sample of
worlds, the height at level _n_ and the height at level _n + k_ differ by less than a stated bound that
depends only on _n_. Collision reads the piecewise-linear surface through the finest level's
vertices, on the same triangle diagonal as the mesh, and that level is drawn, with its morph held at
zero, within a stated radius of every grounded or descending body in view. The authoritative ground
is therefore that interpolant: the query defines its vertices, and their spacing, at most 0.375 m,
keeps the 2 m band limit within about a third of its amplitude. Where anything touches the ground,
collision and the picture agree to the `f32` step of the height texture, about 2 mm. Coarser levels
serve sensors and distant views, with the level-_n_ bound as their stated error, and the same bound
selects them: a level is drawn where its bound subtends no more than a stated screen-space error, as
[the budget](#performance-budget) sets out.

### Determinism hazards specific to terrain

The sim's rules already cover streams, word consumption, iteration and summation order and integer
widths, in the `sim-determinism` skill (`.claude/skills/sim-determinism/SKILL.md`), whose scope today
is the sim, testkit and fitting crates. Terrain adds hazards of its own, and they are the ones that
bite across architectures. They belong in that skill as a new section, not in
`.claude/rules/rust-dev.md`, which gains only a pointer: the determinism auditor reads the skill, and a
hazard written only in the rule file would reach the Rust reviewer and never the auditor. The skill's
`paths:`, and the review routing to the auditor, extend to the surface crate and to the base crate
beneath it when they are created. What can be mechanical is: the `clippy.toml` bans below and the
surface crate's `compile_error!`.

- **Transcendental functions.** `sin`, `cos`, `exp` and friends are not bit-identical across libm
  implementations and targets. The repository already knows this: `libm` is pinned to `=0.2.16` with a
  comment that a bump is a generator-version change, and Clippy's `disallowed-methods` routes
  every transcendental function through `hyperion_sim::math`, in the sim's own `clippy.toml`, in
  the fitting crate's (`crates/hyperion-fit/clippy.toml`), which repeats the sim's bans, and, since
  22 September 2026, in a workspace-root one that binds the server, protocol and testkit crates.
  Clippy takes the nearest file and does not merge them, so each crate's file is self-contained.
  Terrain must live inside that discipline, and the surface crate needs a self-contained
  `clippy.toml` of its own modelled on the sim's rather than falling back to the root file, because
  the root file deliberately omits the sim's ban on float-bit conversions. That ban exists to stop
  floats being hashed, which is exactly the mistake a noise function is tempted to make.
- **Fused multiply-add, where the code asks for it.** rustc never fuses a multiply and an add on its
  own, so there is no contraction to turn off; the hazards are the explicit forms. A correctly
  rounded `fma` gives the same bits in hardware or software, so the risks are a platform `fma` that
  is not correctly rounded, and fused and unfused forms mixed in one computation. `f64::mul_add`
  is already banned. The sanctioned `hyperion_sim::math::mul_add` is `libm::fma`, software on every
  target and so slow in a noise loop: hot noise code writes `a * b + c`. The `algebraic_*` float
  methods, stable since Rust 1.98, license the compiler to fuse and reassociate, and their
  documentation says the same inputs may give different results even within one program run. No
  `clippy.toml` bans them yet, and every one should: the root's, the sim's and the fitting crate's
  now, and the surface crate's and the base crate's when they exist, five in all. Core WebAssembly
  has no FMA at all; relaxed SIMD's `relaxed_madd` is the exception.
- **Summation order** in octave accumulation is fixed and never reassociated, which the skill's
  existing rule already requires; it is listed here because noise is where the temptation is.
- **Relaxed SIMD is banned outright, and mechanically.** Its whole premise is that an instruction
  may return different results on different hardware. A `compile_error!` under
  `target_feature = "relaxed-simd"` in the surface crate makes the ban a build failure. Fixed-width
  128-bit SIMD is IEEE-exact and welcome.
- **`min` and `max` are not exact at zero.** For equal inputs such as +0 and −0, Rust documents that
  either may be returned, and a flipped zero sign propagates through `1/x`, `atan2` and `copysign`.
  On x86-64 with rustc 1.98.1 the sign already varies within one build: the instruction returns the
  second operand, but the optimiser's constant folding orders −0 below +0, so the same expression
  gives a different sign when its inputs are known at compile time. The IEEE `minimum` and `maximum`
  are still unstable (`f64::minimum`, issue 91079), so the height path uses a `min` and `max` of its
  own that fix the sign.
- **A NaN's sign is nondeterministic on every target.** The testkit's golden writer already refuses
  NaN (`crates/hyperion-testkit/src/golden.rs`), and the sim's and the fitting crate's `clippy.toml`
  ban reading float bits, but the sign still leaks through `total_cmp`, which
  `.claude/rules/rust-dev.md` recommends for ordering, through `is_sign_*`, and through
  `copysign`. Heights are asserted finite before they are sorted, compared or emitted.
- **Flush-to-zero from outside.** Neither target flushes subnormals to zero under Rust, but a C
  library can turn flushing on for its thread, and a thread inherits the flag from the one that
  created it. GCC 13 and Clang 19 no longer link the fast-math startup code (`crtfastmath.o`) into
  `-shared` objects; older toolchains still do, so a library built with `-ffast-math` on them turns
  flushing on when it loads, and any library can write the control register itself. Current
  llama.cpp and ggml builds use no fast-math on GCC or Clang and set no flush mode, but a
  distribution's or an older pin's build might. So the server checks rather than trusts. Reading the
  control register needs `unsafe`, which the workspace forbids, so it probes safely instead: on every
  compute thread, when the pool starts, after any native library is loaded and at the end of each
  generation job, it checks that `black_box(f64::MIN_POSITIVE) / 2.0` is not zero (output flushing)
  and that the smallest subnormal times 2⁵² is not zero (input flushing, x86's DAZ, which the first
  check misses), the same in `f32`, and refuses to generate if either fails. The LLM runs out of
  process where it can, which makes the leak impossible.
- **Seed derivation is integer**, from integer cell coordinates rather than floats, and noise is a
  counter-based block function of those coordinates — the sim's Threefry2x64-20, on its `Stream` —
  rather than tables or state. That is the technique that makes a noise function depend on nothing
  but its inputs. Cache and cell keys are `u64`, never `usize`, which has 32 bits on
  `wasm32-unknown-unknown`.
- **`f32` anywhere in the authoritative path is a mistake.** Heights are `f64` in the generator and
  narrow only when they reach the GPU.

### Materials, and what a surface looks like

Kept brief, because it is the least uncertain part and the most easily changed:

- **Material is authoritative; its appearance is not.** The surface crate returns a material class
  at any point, from slope, altitude, the coarse climate field and plan 14's ice fraction, from the
  same inputs as the height. Cliffs above a slope threshold are rock, and ice lies where the coarse
  pass placed it, from the temperature field and the ice fraction rather than by latitude alone, so
  a warm pole has no ice cap and a tidally locked world's ice sits where the climate model puts it.
  The class carries the physical properties that landing and consoles read: albedo range, friction,
  bearing strength. The worker bakes the class into each patch beside its heights, and the shader
  blends the class's textures across a transition band but never re-decides the class, since an
  `f32` shader recomputing it would flip classes at every threshold, so the ground drawn and the
  ground quoted cannot disagree. Colour variation and micro-detail below the band limit are
  decoration.
- **Triplanar projection** for rock and detail textures, blended by the surface normal, which avoids
  the UV distortion any spherical parameterisation produces. Biplanar is the cheaper variant if three
  samples per fragment costs too much on the Intel part.
- **Repetition hidden** by blending two scales with a per-patch rotation and offset, which is two
  samples rather than the four or more that stochastic approaches need.
- **Instanced scatter** from hashed cells, filtered by biome and slope, with the instance count falling
  off with distance. Only decoration thins or is cut: authoritative instances, those 0.2 m tall or
  more, are always drawn within the finest-level patches under grounded and descending bodies, on
  every setting, so that nothing a hull can touch goes undrawn.

### Atmosphere

The requirement is unusual and it decides the choice: HYPERION needs atmospheres for **arbitrary
compositions**, seen from the ground, from orbit and from outside, through the terminator, with
correct fog on terrain at every distance.

| Model                                            | Verdict                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| ------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Hillaire 2020, the production LUT approach**   | **Lean.** Four small tables — transmittance, multiple scattering, sky-view and aerial perspective — cheap enough to rebuild often: transmittance and multiple scattering depend on the atmosphere alone and are rebuilt when it changes, and the sky-view and aerial-perspective tables depend on the view and the sun and are rebuilt every frame. Hillaire measured 0.17 ms for all four on a GTX 1080, 0.31 ms including the final sky and aerial-perspective pass at 720p, 0.5 ms with the per-pixel ray march it uses for views from space, and under a millisecond for the two per-planet tables on an iPhone 6s, which is roughly the UHD 620's class. It takes Bruneton's material model — his density profiles, ozone layer and Cornette–Shanks aerosol — and Bevy 0.19's version generalises it to any number of terms, each with its own density and phase function. RGB rather than spectral. |
| Bruneton's precomputed scattering, 2017 revision | Multiple scattering precomputed into four-dimensional tables, inside and outside the atmosphere, with aerial perspective, and spectral at no runtime cost. But an update takes 250 ms on the same GTX 1080, about 150 ms on the discrete target and seconds on the UHD 620, and its density profiles are limited to two layers, with one aerosol and one absorbing layer. Its WebGL demo loads tables precomputed offline. The reference for spectral error in thin atmospheres, if Hillaire's RGB approximation proves visibly wrong there; not a fallback for thick ones, where its iterations diverge.                                                                                                                                                                                                                                                                                                 |
| Nishita 1993, O'Neil (GPU Gems 2)                | Single scattering only, with the known darkening artefacts and a phase function disabled to hide them. Too approximate for a display that claims physical units.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Hosek–Wilkie and other analytic sky models       | Fitted for ground-level daylight on Earth. No use from orbit.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |

**Parameterising from physics** is the part that must be built rather than borrowed, and it is
straightforward in outline. The medium is a list of terms, each a density profile, a scattering and
absorption coefficient and a phase function, as Bevy's is, so a new gas is a new term rather than a
new model. Rayleigh scattering follows from each gas's dispersion formula and King factor, falling
as the inverse fourth power of wavelength, mixed by number fraction: dry air from Peck and Reeder
1972 and Bates 1984, N₂ from Peck and Khanna 1966, CO₂ and CH₄ from Sneep and Ubachs 2005, H₂ and He
from Dalgarno's cross-sections. Each refractive index is evaluated at the density its formula states.
The scale height follows from temperature, mean molecular mass and gravity, which plan 14 provides
with the pressure and the gas fractions (P14.T24.a). Earth's reference values anchor the
implementation: a Rayleigh scale height near 8 km (8.43 km at the US Standard Atmosphere's sea
level, which with the sea-level density carries Earth's whole column; 8 km leaves it 5% short), an
aerosol scale height near 1.2 km, and an aerosol asymmetry (the phase function's mean cosine) near
0.65, AERONET's continental value at 550 nm; in Cornette–Shanks's form that is g ≈ 0.58, not
Bruneton 2008's 0.76 (mean cosine 0.81) or Hillaire's 0.8, which are kept only to compare with their
images. One caution for whoever writes the code: **the widely copied Rayleigh coefficients are not
the physical ones.** Derived from the formula at 288.15 K and 1013.25 hPa, Earth's are 4.85, 11.5
and 28.7 × 10⁻⁶ m⁻¹ at 680, 550 and 440 nm, and the 550 nm cross-section matches Bucholtz 1995's
4.51 × 10⁻²⁷ cm². The set tutorials copy from Bruneton, 5.8, 13.5 and 33.1 × 10⁻⁶ m⁻¹, is 15–20%
higher and is not used. The constants are derived from the formula at stated wavelengths, with a
cited source, exactly as the project's rules already require of physical constants, and a test
recomputes Bucholtz's figure to 1%.

Absorption and aerosols are where character comes from: ozone on an Earth-like world, suspended dust
on a Mars-like one, hydrocarbon haze on a Titan-like one, and skipping them cannot be patched over by
tuning the Rayleigh terms. Mars' butterscotch sky and blue sunsets are both dust effects: without the
dust, its thin carbon dioxide would give a dark, faintly blue sky. Tinting the Rayleigh coefficients
can make the daytime colour right for the wrong reason, but Rayleigh scattering is nearly symmetric
while dust scatters strongly forward, so the glow around the sun and the blue of a Martian sunset —
both forward-scattering effects — would still come out wrong. Bevy's Mars preset takes its dust's
phase function from Mie theory, after Schneegans et al. 2024, which is the precedent to follow.

Plan 14 has no aerosol or absorber inventory yet, and the renderer must not invent haze. **Lean:**
plan 14 owns each atmosphere's aerosol modes, beside its composition in P14.T24.a: a material keyed
to a refractive-index table, a size distribution (a modified log-normal's effective radius and
variance, or a fractal aggregate's monomers for Titan-like haze), a column optical depth at 550 nm,
and a vertical profile. Condensate clouds form where a species crosses its saturation curve,
photochemical haze where methane meets ultraviolet, dust on arid, windy, thin-aired worlds; ozone is
an absorber only with O₂ and a star that emits ultraviolet, and methane absorbs on cold, reduced
worlds. The client turns the inventory into phase functions and coefficients offline, with Mie
theory for spheres and an aggregate model for haze.

Thick atmospheres are where both models are unproven. Bruneton's multiple-scattering iterations fail
to converge and then diverge at 40 orders, Hillaire's colour can drift at very high scattering
coefficients, and Bruneton's table layout mishandles small bodies with thick atmospheres, which is
Titan's case. Venus, at a Rayleigh optical depth near 15 and a cloud optical depth near 30, is a
diffusion regime that neither was built for. Hillaire's tables are fully spherical, so transmittance
and single scattering stay right; what fails is his analytic multiple-scattering term. **Lean:**
Hillaire's tables, the two per-planet ones regenerated when the atmosphere changes and the two
per-view ones every frame, with a ray march for views from orbit, where the sky-view table spends
its resolution on empty space; terrain beyond the aerial-perspective volume's 32 km reach (Hillaire
2020's, §5.4, and Bevy's; sebh's reference code reaches 128 km) also needs the march. Where the
multiple-scattering term drifts from a converged reference, at Venus-class depths and for
Titan-class haze, only that table is replaced, by one baked offline for that atmosphere with a
converged solver (discrete ordinates or a spherical Monte Carlo, in `f64`) and cached per world. A
cloud deck of optical depth above about 10 splits the atmosphere in two: above it, Hillaire's tables
run over the deck as a baked reflecting boundary; below it, a baked plane-parallel table of
downwelling radiance by altitude, view angle and sun angle gives both the sky and the aerial
perspective. Bruneton's iterated orders are not the fallback, since they diverge in exactly this
regime. Every baked table is validated against a path-traced reference, to 5% in radiance, before
Venus- and Titan-class atmospheres ship ([open question 3](#open-questions)).

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
requires, and it should be built with both settings from the start rather than retrofitted. Either
way the clouds are a transparent layer under the [depth rules](#depth-reversed-z-and-no-logarithmic-depth),
and a deck at least a kilometre above the terrain needs no depth bias out to 10⁹ m.

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

Two gaps, both smaller than they first looked. A **spherical** ocean at planetary scale is published.
Bruneton, Neyret and Holzschuch 2010 draw the ocean from space as a sphere whose reflectance comes
from the slope statistics of the waves too small to resolve — a Cox–Munk distribution, which is
exactly the glint-first path above — and switch below 20 km to a projected grid on the sphere.
Proland's ocean module implements it, working in a frame tangent to the sphere under the camera, and
Scatterer carries it into Kerbal Space Program. Outerra already draws its ocean on its terrain
patches, displacing their meshes with Gerstner waves. What remains our own is keeping an ocean on
the terrain quadtree consistent across levels of detail under the collision rules, and a wave
spectrum driven by the climate field. And the **shoreline**, where a wave field meets procedural
terrain, is the classic hard case: it needs the coarse ocean level, a depth-dependent wave amplitude,
and foam driven by the terrain's slope. The only account for procedural planetary terrain that we
found is Outerra's, a distance map with waves chosen by depth; Jeschke et al.'s water surface
wavelets (2018) handle waves meeting shores and obstacles, but not at planetary scale.

The water's thickness, which sets its colour over the shallows, comes from the bathymetry — sea level
less the terrain height — rather than from the depth buffer beyond about 100 km, and the shoreline is
hidden by foam and a depth fade, not a depth bias, as the
[depth rules](#depth-reversed-z-and-no-logarithmic-depth) set out.

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
recognisable cues, and both are computed analytically, a ray against a sphere and a ray against a
plane, rather than from depth.

Shadowing between particles cannot be left out: it accounts for most of the B ring's brightening
with elevation, about 20% in brightness against at most 10% from multiple scattering as the
elevation rises to 26° (Salo and French 2010). No analytic form has a published error against
those simulations — Irvine 1966 and Lumme and Bowell 1981 assume a semi-infinite layer many
particles thick, and Hapke 2008 was built for regoliths — and Salo and French fit a grid of Monte
Carlo simulations rather than a formula. **Lean:** the
classical single-scattering layer times a baked shadowing factor of phase angle, elevation and
optical depth, from an offline Monte Carlo of a particle slab at a filling factor near 0.05 with
plan 14's size distribution (Salo and Karjalainen 2003's method), a table under 50 kB. It must
reproduce the B ring's shadowing-driven brightening from 4° to 26° elevation to within five
percentage points. The low setting uses Hapke's shadow-hiding term with its width set by the filling
factor, and accepts an error that is unknown until the baked table exists to measure it against.

A ring's radial structure is generated, not one optical depth painted across the annulus, and plan
14's single optical depth and resonance gaps are only the start of it. **Lean:** plan 14, or a ring
sub-generator it owns, emits a radial profile of optical depth, albedo and spectral slope, built
from processes. The major moons' strong resonances give edges and gaps; embedded moonlets give gaps
a few Hill radii wide; weaker resonances give decaying density-wave trains, whose wavelength needs
the ring's surface density, not its optical depth alone; ballistic transport of meteoroid ejecta
gives ramps at every step in optical depth; a seeded random field in log optical depth gives dense
rings their irregular structure; and a pollution fraction tied to surface density darkens and
neutralises the sparse regions, as Saturn's C ring and Cassini division are darker and less red.
The density-wave formula is to be checked against Shu 1984 before it becomes code. The client
samples the profile as a one-dimensional texture at the mip for the pixel's footprint. Björn Jónsson's published radial profiles — backscattered, forward-scattered,
unlit side and transparency — are Saturn's, derived from Voyager and Cassini data, and the page
states no licence, so they are a private visual reference only. For anything tested, Saturn's
primary occultation profiles from the PDS Ring-Moon Systems Node are the data, and even they serve
only as a statistical target, never a template a generated ring reuses.

Where the annulus gives way follows from the pixel, so the criterion is an angle, not a distance. At
1080p and a 60° horizontal field of view, the convention throughout this document, a pixel is about
0.55 mrad on average and 0.60 mrad at the centre, so a body of size _D_ fills one at about 1,830 ×
_D_: 9 km for plan 14's 5 m top particle size (Zebker et al. 1985) and 18 km for a 10 m particle,
and a 10 m-thick layer seen edge-on reaches a pixel at the same 18 km. The range scales with the
resolution, to about 1,220 × _D_ at 720p and 3,670 × _D_ at 4K. **Lean:** the ring is an
optical-depth slab while the largest particle subtends less than a pixel. Nearer than that, only
bodies larger than a pixel are instanced, from the top size decade, and their share of the optical
depth is removed from the slab, so that total extinction is conserved. Within a few layer
thicknesses the view becomes a local particle field inside volumetric extinction, where sideways
visibility is only some ten metres. SpaceEngine does the same in outline, without publishing its
criterion.

### Knowledge, and the surface seed

The single-player brainstorm raised a problem and deferred it here: the client needs the surface to
draw it, but the rule is that the client holds only what the ship has seen, and a seed that reveals
the whole surface at once breaks that rule.

The [server-side coarse field](#who-computes-the-coarse-field-and-why-it-is-the-server) resolves most
of it. The client is given the regions of the coarse field the ship has surveyed — exact where it has
them, absent where it does not — and never the surface seed. The fine synthesis it runs locally takes
a **detail seed** instead: a block output of the universe seed on a domain tag of its own,
`body.surface.detail`, keyed by the body's ID, which shares nothing with the surface seed but the
universe seed. It needs only a new tag in the registry, which moves nothing already generated. The
client therefore has no path by which it could regenerate the coarse field, and can only elaborate
what the ship established. This is a discipline for an honest client, not a security boundary: the
client knows the universe seed and the generator ships with it, so a modified client can always
regenerate a world, as in any seed-based game. A mode that hid a universe from its crew would first
make the universe seed server-only, and the sibling derivation is already the right shape for it.

Plan 14 as specified contradicts this, and the contradiction is already wired in. Its records put
the hooks, the seed among them, in the `Full` detail level, and its wire task sends `BodyDetailDto`
with every section, hooks included. `BodyHooksDto` already carries `surface_seed`
(`crates/hyperion-protocol/src/planetary/record.rs`), and the client already parses and keeps it
(`surfaceSeed` in `apps/hyperion/src/renderer/src/lib/system/bodiesWire.ts` and `model.ts`), so the
seed reaches every client the moment P14.T23 computes it. **Lean:** the surface seed is
server-only. Plan 14 is amended before P14.T23 lands: `BodyHooksDto` replaces `surface_seed` with
`detail_seed`, the client's wire parser and `BodyHooks` follow, and the client receives only the
detail seed, with the surveyed coarse cells.

What remains is honest labelling, and it is a rendering requirement rather than a fiction one. The
amplified terrain is not an approximation: the server collides with the same function, so it is the
ground itself, and the view band-limits it to what can be resolved from where the camera is, which
is what an eye there would see. What the display must label is what is _not_ the ground: GPU
decoration, a setting that draws the surface coarser than the camera's position warrants, which is
the limited-detail annunciation proposed under
[What the guide must gain](#what-the-guide-must-gain), and the edge of what has been surveyed.

A readout is a different matter, because it states a measurement, and the principle is what the
ship knows, not what the client holds. **Knowledge gates coverage for the image, and coverage and
resolution for every number.** The view draws surveyed ground at whatever detail its camera
resolves, and its label block states the survey's resolution beneath its centre. A readout, whether
a console's or a cursor's on the view — an elevation or a slope at a distant landing site, a range to
the ground under the reticle — quotes the ground only as well as the ship has measured it. It gives
the coarse survey's value under the guide's estimated state, in the same way an unscanned contact's
mass carries its `~`, with the root-mean-square of the unsurveyed bands as its uncertainty, a
closed-form figure per cell since the fine synthesis is band-limited noise of known amplitude
(`ELEV ~2140 m ±180 m`), until a close-range survey has resolved the point. That is also the
gameplay loop real landing-site selection follows: survey from orbit, survey close, then descend on
the vehicle's own sensors.

A region not yet surveyed at all is drawn as what the ship knows of it, the disc and atmosphere from
plan 14's figures, with no terrain claimed. That cannot extend to ground the ship can reach. A ship
descending over an unsurveyed region surveys it with its own sensors on the way down, so the server
sends the cells under and ahead of it before it could touch them, and no hull ever meets terrain the
client was not given. **Lean:** the view carries the survey coverage and resolution as a readout,
the same way the star chart carries its census line.

Nothing builds that gating yet. Plan 12's Knowledge store holds contacts only, and plan 14 leaves the
body-level overlay to a later plan: until it exists the server grants whatever detail level a
request asks for. **Lean:** the coverage record is built by the plan that implements the surface
crate, the coarse pass and the wire, on P12.T7's store, as survey passes (one versioned JSON line
per orbital, close-range or landed pass, beside plan 12's contacts) from which the per-cell best
resolution is derived on load ([open question 4](#open-questions)). The body-level overlay, which
detail level the ship holds for each body, belongs to the sensors plan that follows sessions. Until
then the server keeps granting the level asked, except that the surface seed never reaches a client.

## The sky

### The star field is the galaxy, not a photograph

Most space games paint a sky box. HYPERION should not, and for once the realistic answer is also the
one this project is built for: the galaxy model already knows where every star is, and plan 06's
stellar stage knows what each one is. Plan 06's stellar brief is built: P06.T33 put
`include_stellar` and a `StellarBriefDto` — kind, class, `log_luminosity_lsun`, `teff_k` and
`star_count`, the luminosity and temperature null for a black hole or nothing — on each row of the
range query, and P06.T34 fills it on the server in chunks of 1,024 rows. That is what a star field
needs. The harder problem is selection. The range query is **volume-limited** — a centre, a time, a
radius, a mass-layer floor and a `limit` — while a sky is **flux-limited**: a K supergiant 3,000 ly
away is a naked-eye star, while Proxima Centauri at 4.2 ly is not. Range queries cannot serve it,
and the server bounds them twice over, by the systems returned and by `MAX_QUERY_CELLS`, 262,144
cells visited a query. The five stellar layers own bands of primary initial mass — A 0.08–0.5 M☉,
B 0.5–0.75, C 0.75–2.5, D 2.5–8 and E 8–150 — and the two substellar layers, brown dwarfs and rogue
planets, reach the sky only from a few hundredths of a light-year: an old brown dwarf of M_V ≈ 20
shows at V 5.9 from 0.05 ly, which is an ordinary distance in the nuclear cluster, so there the
parallax sprites' nearby query includes the substellar layers. To naked-eye magnitude 6.5 the
brightest stars of the heavier layers — supergiants and O stars from E, and yellow post-AGB stars
and giants from C and D — are visible thousands of light-years away. Near the Sun the spheres that
reach them, at the caps of [open question 13](#open-questions), hold some 2.6 × 10⁷ systems — 5.8 ×
10⁶ in C, 7.4 × 10⁶ in D and 1.3 × 10⁷ in E — and at the brightness rule's own radii upwards of
2.5 × 10⁸. At the Galactic Centre the same caps take in most of the bulge and the nuclear disc,
about 2 × 10⁹ systems in layers C to E. Either is far past the server's census limit of 20,000
systems a query, and placing every system, at a few microseconds each, would take minutes of one
core per arrival near the Sun and hours at the centre. That and the section's other counts are
orders of magnitude, the Milky Way fixture's at generator version 14, before plan 06's track fates
(P06.T30) and the Chabrier refit of version 15, and before layer C's cap is re-derived for its
post-AGB stars; they are to be re-derived. The sky therefore needs a request of its own.

With it, the sky is **generated from the same model the charts read**, at a different time. Every
star is placed and described at its retarded time t − d/c and its apparent position
(`crates/hyperion-sim/src/observe/retarded.rs`), as the galaxy brainstorm rules for every sensor
([Orbits and time](galaxy-generation.md#orbits-and-time)), while the `GALAXY` chart shows present
positions. The view out of the window and the chart therefore differ by the light-time and by
nothing else: from 26,000 ly the sky still shows hundreds of supergiants the chart knows are dead,
and a star the player jumps to is the star they were looking at, because its observed position and
velocity, extrapolated to the present (`extrapolate_to_present`), land on it. Since the owner's
ruling of 2026-10-08 that holds for the real stars: every star brighter than the hybrid sky's
ceiling and every star within its real boundary
([The hybrid sky](#the-hybrid-sky-the-census-near-synthetic-stars-far)). The brief describes
the primary as it is now, so the sky asks for it at the emitted time, which costs what the present
one does; but `BriefModel` (`crates/hyperion-sim/src/stellar/brief.rs`) tests its routes over the
clock window of ±1,000 years only, and until they are tested over the retarded interval a star that
died within the light-time would be served dead.

The figures familiar from Earth do not hold even near the Sun. Space has no airglow, so the
background near the Sun is starlight alone, μ_V ≈ 24.3 mag/arcsec² at the galactic poles (|b|
over 80°) and about 22.05 in the band (|b| under 5°) (Gaia DR3 flux sums of the stars fainter than
V 6.5), and the diffuse galactic light the dust scatters, about 10–35% of the integrated starlight
by direction (Toller 1981; Leinert et al. 1998, Table 39), which the band holds (R06.T9.g), plus
zodiacal light inside a system with a zodiacal cloud, about 23.3 at the Sun's
ecliptic pole (Leinert et al. 1998). Deeper
in, the sky is fuller and brighter, and its brighter background lowers the naked-eye limit. The
limit is Crumey's (2014) point-source threshold, his eq. 53 with Blackwell's scotopic coefficients;
eq. 55, m = 0.426 μ − 2.365 − 2.5 log₁₀ F, is its linear form for 21 < μ < 25, and backgrounds
brighter than μ ≈ 18.9 are mesopic, where his eq. 34 applies and the model is at its limit. The
background is corrected for its colour: a starlit sky with no airglow is 0.4–0.5 mag brighter to
the rods than its V surface brightness (Crumey §1.3). The field factor F is 1.4, Crumey's value for
an experienced, dark-adapted observer, which reproduces the familiar 6.5–6.6 under Earth's darkest
sky; it is an observer setting, and 2, his typical observer, takes about 0.4 mag off every limit.
Away from the Sun the backgrounds come from a hand model of the fixture and are orders of magnitude,
until the band's own integral replaces them:

| Where                                                 | Stars to V 6.5 | Background μ_V, mag/arcsec² | Naked-eye limit, V |
| ----------------------------------------------------- | -------------- | --------------------------- | ------------------ |
| Earth's darkest ground sky, for comparison            | 8,874          | 21.8                        | 6.6                |
| Near the Sun: in the band / at the galactic poles     | 8,874          | 22.05 / 24.3                | 6.45 / 7.4         |
| Inner disc, 4 kpc out                                 | 2–3.5 × 10⁴    | About 21                    | About 6.1          |
| Bulge, 1.5 kpc from centre: away from / towards it    | About 4 × 10⁴  | About 22 / 19.7             | 6.5 / 5.6          |
| Bulge centre, outside the nuclear disc: off / towards | About 3 × 10⁵  | 19.5 / 16.5                 | 5.6 / 5.3          |
| Nuclear disc: out of / in the plane                   | 0.5–2 × 10⁶    | 18.8 / 17.5                 | About 5.4 / 5.3    |
| A globular core like 47 Tuc                           | About 4 × 10⁵  | About 21, 17.7 with glare   | About 5            |

The limit map takes as each texel's background the light fainter than the eye's cut, about V 8.15
near the Sun. That light is darker than these figures by about 0.1 mag in the band and 0.3 at the
poles (Gaia DR3: 22.18 and 24.60 for V over 8.1), so there, with the diffuse light, the limits are
about 6.4 and 7.5.

In a globular core the unresolved light is faint, and what sets the limit is the veiling glare of
its thousand or so stars brighter than V −5, the same glare the renderer draws as bloom. At the
star-count slope near the Sun, 0.49 dex a magnitude between Hipparcos's 1,608 stars to V 5 and 8,874
to V 6.5, some 10⁵ stars remain visible at the bulge centre and 1–6 × 10⁵ in the nuclear disc, which
no fixed magnitude and no single budget sized for the Sun's sky can carry.

The practical form:

- **Apparent magnitude sets the flux**, from the star's luminosity, its distance, and the extinction
  along the line of sight from plan 07's built sightline integral through the dust field
  (`crates/hyperion-sim/src/galaxy/gas/extinction.rs`), which gives A_V and each band from U to K
  and has a budgeted quality for many rays. Plan 07's `extinction` request kind (P07.T10.c, not yet
  built) carries it per target; the sky request is its first bulk use. Flux converts to the
  absolute luminance units the rest of the pipeline uses. Nothing is authored by eye. The stellar
  brief describes the primary alone, by its bolometric luminosity and effective temperature, so a V
  magnitude needs a bolometric correction from the temperature, and the companions' light must come
  from `system_summary`, whose stars carry `absolute_v_mag`, or from the sky request itself. A
  primary that is now a white dwarf beside a bright companion, as in Sirius and Procyon, reads as
  faint from the brief alone, and white dwarfs have no absolute V magnitude in the photometry table
  at all. **The sky has two limits together** ([open question 13](#open-questions)): a physical
  one, V_lim, the naked-eye limit set **per direction**, at the band map's resolution, from the
  local background, which is the band's unresolved surface brightness plus the glare of the
  brightest resolved stars; and a count budget, N_max, which sends the brightest N_max stars by
  flux. Stars between N_max and V_lim join the band's unresolved light at the census, so nothing is
  counted twice. Per direction, because Crumey's background is the one immediately around the
  target, and the band's brightness varies by 2 mag across the sky near the Sun and about 4 in the
  bulge, so one limit per location would be wrong by up to a magnitude either way. Near the Sun
  V_lim runs from about 6.5 in the band to 7.55 at the poles, which makes about 15,000 stars visible
  at once, against Earth's 8,874 Hipparcos stars to V 6.5 (8,404 in the Bright Star Catalogue). The
  census selects to the location's deepest limit, some 2.3 × 10⁴ stars near the Sun, and the
  renderer thresholds per direction. A 16:9 view 60° wide is 36° tall, 0.620 sr, 4.9% of the sky,
  so it holds about 740 on average, and more along the plane.
- **The eye's limit belongs to the eye.** It applies to the view that stands for the player's eyes,
  the cockpit window in single-player. A camera view, the main screen included, uses its own: the
  magnitude whose pixel signal at the current exposure falls below the sensor's noise floor, about
  V 10 for a video camera in a dark sky, some 3 × 10⁵ stars near the Sun, and less with a bright
  object in frame, which drives the exposure down and takes the faint stars off the screen. AgX's
  toe has no noise, so the camera's cut is an explicit noise-floor model rather than left to tone
  mapping, or the camera would see without limit. The sky request is made to the deepest limit of
  the views open, within N_max, and each view thresholds its copy.
- **Colour from a spectral library**, not a blackbody: a precomputed table over effective
  temperature and surface gravity, the gravity from the brief's luminosity class, integrated from
  model spectra (PHOENIX, ATLAS9 and TLUSTY, with Pickles 1998 as the empirical check and white
  dwarfs from their own grids) against the CIE colour matching functions and converted to the
  display primaries, desaturating towards the white point when a colour falls outside the gamut. A
  blackbody at the effective temperature is up to 0.02 off in CIE 1960 uv for M dwarfs and 0.01
  for A stars, several just-noticeable steps, and real M dwarfs are less red than their blackbodies.
  Reddening by the same dust that dims the star is a further axis, E(B−V), or per-channel
  extinction at the primaries' effective wavelengths. The same table carries each spectrum's lux
  per V magnitude and its scotopic-to-photopic ratio, which moves the naked-eye threshold for an M
  star about 0.3–0.4 mag brighter than for a sunlike one and for an O star about 0.4 fainter
  (Crumey eq. 18). It costs a texture lookup.
- **Faint stars baked into a cubemap per location; bright ones drawn as sprites.** Tens of thousands
  of point sources are expensive to draw and, worse, they alias: a sub-pixel star flickers as the
  camera turns, which is the classic failure and it looks like a bug. Baking integrates each star's
  flux over the texel it falls in, which is correct antialiasing only while a texel is no larger than
  a pixel — and at 60° across 1080p, where the view's centre pixel is 124 arcseconds and a cube
  face's centre texel its largest, that needs faces of about 3,300 texels, some 270 to 530 MB for the
  six at 4 to 8 bytes a texel before mipmaps, and more if the view zooms. So the bake takes the faint,
  unresolved majority at a resolution the memory affords, where a faint star spread over a texel
  reads as the sky's grain; the bake is linear in star count, and even the nuclear disc's 10⁵ to 10⁶
  splats are cheap. The brightest stars, up to a sprite budget — near the Sun the roughly 1,600
  brighter than magnitude 5 — and the parallax sprites below are drawn every frame as sprites whose
  point-spread function is integrated over the pixel, which keeps their antialiasing at any field of
  view. Light fainter than V_lim or beyond the count budget belongs to the galactic band and to a
  statistical layer of unresolved stars, labelled as such. A deep exposure that wants fainter
  individual stars asks for a narrow cone rather than the whole sky: a 1° field is 2 × 10⁻⁵ of it.
- **Baked once per arrival, with parallax left to the sprites.** A star at distance _d_ shifts by
  206,265 × (baseline ÷ _d_) arcseconds, against roughly 110 arcseconds per pixel at 1080p across a
  60° field of view. In the solar neighbourhood, with the nearest star over a parsec away, a journey
  of a few astronomical units moves the sky by a few arcseconds, a few hundredths of a pixel. But
  in the grid alone the galaxy model places some fifteen million pairs of star systems within 0.1 ly
  of each other, over half of them in the nuclear disc
  ([Coordinates](galaxy-generation.md#coordinates)), and a star 0.1 ly away
  shifts by about 33 arcseconds per astronomical unit: crossing 30 au moves it nine pixels.
  **Lean:** a star that would shift by more than a tenth of a pixel across the system is drawn as a
  sprite at its true position every frame rather than baked, and everything else is baked once per
  arrival. The threshold is a number, and saying so with it is better than hoping nobody asks. How
  many sprites it makes depends on where the ship is. For a 30 au journey it takes every star within
  about 9 ly, which near the Sun, at about 0.002 systems per cubic light-year, is a handful. In the
  nuclear disc, at some 16, it is about 50,000 systems, of which about a quarter are bright enough to
  be in the sky at all, and in the nuclear cluster nearly every star. There the bake is instead
  redone as the camera moves, whenever the nearest baked star's accumulated shift reaches the
  threshold, and the budget's star-field figure is the solar neighbourhood's.
- **The galactic band from the galaxy's own model.** The Milky Way seen from inside is the light of
  the stars too faint or too many to draw individually, integrated along each line of sight outward
  from the ship and dimmed by the dust in front of it. That is related to what the `GALAXY` map
  shows but is not the same integral: the map's column density counts systems per square light-year
  along parallel lines through the whole galaxy, where the band needs luminosity along rays from a
  point, with extinction. It comes from the same density, stellar and dust fields, so it cannot
  disagree with the charts, but it needs a quadrature the simulation does not have yet: each
  population's and each layer's V light per system in stars fainter than a given M_V, its
  cumulative luminosity function, integrated over the population's ages at the light's emitted
  time — the counterpart of the mean present-day mass per system that `mean_present_mass` in
  `crates/hyperion-sim/src/galaxy/fates.rs` integrates. A mean light per system is only that
  function's total, right beyond every layer's cap and wrong inside it, which is where the ship is.
  Along each ray the band takes, at each distance, the light fainter than V_lim there, all of a
  layer's light beyond its cap, and the census's overflow past N_max as points, so the stars near
  the ship are not counted twice; the subtraction is exact in expectation, since the census skips
  the realised faint stars by mass rather than summing them. Its surface brightness is also the
  unresolved half of the background that sets V_lim. Drawing the band from the model means a ship
  in the outer disc sees a thin bright line in one direction and a sparse sky in the other, _because
  the model says so_, and dust lanes appear when the dust field does. No other game can do this,
  because no other game generates the galaxy it is standing in.

### The hybrid sky: the census near, synthetic stars far

_Signed off by the owner on 2026-10-08 (plan R13; `.git/rm23-orchestration/feasibility-hybrid-sky.md`
§11): the hybrid's basis, the ceiling and the amended promise. The label's wording is still a draft
for the UX decision agent._

An exact census to the eye's cut costs too much at generator version 21. Near the Sun the census
must generate most old massive systems to prove their remnants dark, about 0.4–1.0 × 10⁶
CPU-seconds, eight to eighteen hours on the development machine, and a camera's cut five to ten
times that (`decision-p11-t17c-bright.md`). Its cost is far away and its stars are near: within
1,000 ly lie about 0.35% of the records and over half the listed stars. So the owner ruled on
2026-10-08 for a hybrid. The closest stars are shown as they are today, and a second pass draws
points of light from the galaxy's own density and star formation for the rest:

- **Where the census ends.** For each heavy layer (C, D and E) and direction, the census is exact
  out to the radius beyond which fewer than one star brighter than a ceiling V_P is expected: the
  caps' own rule ([open question 13](#open-questions)), applied at the ceiling instead of at the
  cut. So **every star brighter than V_P, and every star nearer than that boundary, is the
  galaxy's own**. Layers A and B, and the brown dwarfs, are real throughout. At a ceiling of
  V 4.5, near the Sun, the boundary lies some 800 ly out in C, 1,200 in D and 2,400 in E
  (estimates, leaning low), and the census costs some thousands of CPU-seconds.
- **Beyond it, synthetic stars.** Between V_P and the cut, the stars are drawn statistically from
  the generated galaxy's own density, star formation history, luminosity functions and dust: the
  tables the band already integrates, never a catalogue of the Milky Way. They are drawn in fixed
  cells of the galaxy, by exact thinning in order of brightness, so each has a fixed place, shows
  true parallax, and is the same star for every observer, client and machine, never reshuffled as
  the camera moves. The band keeps only the light fainter than the cut (and the less than one
  star a layer brighter than V_P beyond the boundary), so nothing is counted twice. Population
  synthesis makes the same kind of catalogue of the Milky Way from its models (the Besançon model,
  Robin et al. 2003; TRILEGAL, Girardi et al. 2005; Galaxia, Sharma et al. 2011); here the model
  is the generated galaxy itself, and the test of the synthetic stars is the real census of that
  galaxy, against which they must agree in counts, light, colour and extinction.
- **What a synthetic star is.** A drawing, labelled, like the integrated starlight: drawn singly
  but not at any system's position, so it cannot be selected, targeted or counted, and the view's
  label and list say so. When the ship comes nearer, the census takes its region over and the
  synthetic stars there give way to the real ones, which differ. So the promise above, that a star
  the player jumps to is the star they were looking at, holds for every star brighter than V_P and
  every star within the boundary, and the faint distant field is the galaxy's statistics rather
  than its systems. At V 4.5 about nine in ten naked-eye stars stay real near the Sun, and most of
  a camera's faint stars are synthetic.

The owner set the ceiling at V 4.5 on 2026-10-08. RM3 ships first with the census at a ceiling of
V 5.0 and the band for the rest, the synthetic stars following in plan R13. Synthetic stars appear
in the naked-eye view as in every camera, labelled. The promise is amended to the real stars, and a
variant in which every far point is a real system with an estimated brightness is revisited once
the census's deferred levers land. This is the
[statistical layer of unresolved stars](#the-star-field-is-the-galaxy-not-a-photograph) the faint
majority was always going to need, made of points rather than of light alone.

### The local star as a disc

Close to, the star is a disc with structure, and two details do most of the work. **Limb darkening** —
the disc is brighter at the centre than at the edge, because a sightline near the limb passes through
cooler, less emissive layers. The standard form is a polynomial in the cosine of the emission angle;
for the Sun at 550 nm the coefficients give a limb at about 30% of the central intensity and a
disc-averaged intensity of about 80% of centre. Other stars take the power-2 law, I(μ)/I(1) =
1 − c(1 − μ^α), which fits model intensity profiles better than the quadratic at the same number of
parameters (Maxted 2018), with coefficients by effective temperature and surface gravity in B, V and
R standing in for the display primaries (Claret and Southworth 2022). It is a few lines and one
`pow` in a fragment shader, and it is the difference between a star and a white circle. **Angular
diameter** is twice the arcsine of radius over distance — about half a degree for the Sun at 1 au —
since a sphere, unlike a flat disc, fills the whole sky at its surface, and it must come from the
real radius, since plan 06 computes it.

Eclipses and the terminator follow from the disc having a size: a shadow is soft because the star is
not a point, so the penumbra comes from integrating visibility across the disc, sampled at a handful of
points. A ship crossing a moon's shadow should see a partial eclipse because the geometry produces one.

### Exposure and tone mapping

The pipeline works in absolute luminance, so something must map about 10⁻⁵ cd/m², the galactic
band's faintest background seen from the Sun's radius, to about 3 × 10¹¹ cd/m², the centre of an O
star's disc (2 × 10⁹ for the Sun), onto a display. No single exposure spans those sixteen decades: a
frame holds AgX's 25 stops or so around its metered exposure, and a disc in frame clips into its
veiling glare. The photographic model is the one to use, because it is the one that makes the
camera's behaviour explicable to the player:

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
- **A pre-exposed half-float target cannot hold a disc.** Under a dark-sky exposure an O star's disc
  lands some 10⁹ times above half-float's largest value, 65,504, so the disc is clamped to that
  value before storage, or drawn analytically with its glare energy handed to the bloom pass
  separately.
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

| Pass                          | Discrete target, 1080p, 16.7 ms budget                                       | UHD 620 low setting, 720p, 33 ms budget                                                                                                                       |
| ----------------------------- | ---------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Terrain geometry and patches  | 3–5 ms                                                                       | 8–14 ms, a shallower quadtree away from the camera; full depth kept under grounded bodies                                                                     |
| Atmosphere, per frame         | 0.5–1 ms: sky-view and aerial-perspective tables, and a ray march from orbit | 2–4 ms, smaller tables, aerial perspective on terrain only                                                                                                    |
| Atmosphere, per-planet tables | Under 0.1 ms, when the atmosphere changes                                    | About 1 ms, on the same occasions                                                                                                                             |
| Volumetric clouds             | 1.5–3 ms at quarter resolution                                               | **Cut.** Replaced by a two-dimensional layer at about 1 ms                                                                                                    |
| Ocean                         | 1–2 ms, Gerstner                                                             | 2–3 ms, about 8 wave components, no refraction, glint retained                                                                                                |
| Shadows                       | 1.5–3 ms, cascaded, with cloud shadows                                       | 0.3–0.5 ms, a horizon map baked with each patch; terrain self-shadowing only                                                                                  |
| Star field and galactic band  | Under 0.2 ms near the Sun, a cubemap and sprites to the sprite budget        | Under 0.5 ms                                                                                                                                                  |
| Exposure histogram            | 0.3–0.5 ms                                                                   | About 1 ms, over a quarter-resolution input                                                                                                                   |
| Bloom and tone mapping        | Under 1 ms                                                                   | 2–3 ms, fewer bloom levels, at quarter resolution                                                                                                             |
| Scatter instances             | 1–2 ms                                                                       | Off, or a token density under 1 ms; the first thing after clouds to go; authoritative rocks kept under grounded bodies                                        |
| Rings, when in view           | 0.5–1 ms                                                                     | Under 0.5 ms, a textured annulus                                                                                                                              |
| Station wireframe view        | Under 3 ms at 1080p                                                          | 4–9 ms at 1080p, against its own 16.7 ms frame: depth-only terrain at a 4 px tolerance with contours in the same pass, lines, symbology and line antialiasing |

The discrete column's lower ends sum to about 9 ms and its upper ends to about 18 ms, which is over
budget. That is recorded rather than tuned away: the frame fits only if the passes do not all land at
their worst, and the first measurements decide which one gives. The UHD 620 column sums to about 16
to 28 ms. Its upper end leaves some 5 ms for the wireframe instruments and the consoles' compositing
that share the frame in single-player, so the low setting must land nearer its lower end, and the
descent spike measures all three together. The sums leave out the rings, which are in view only near
a ringed body, the per-planet atmosphere tables, which are not rebuilt every frame, and the station
wireframe, which runs on a machine of its own.

The descent spike's one judged run (R05, 2026-10-07: the RTX 3080 at 1,509 × 821 px, 60% of
1080p's pixels, one sun and three terms) measured the atmosphere per frame at 3.19, 4.07 and
4.28 ms at the 50th, 95th and 99th percentiles, over the discrete column's 0.5–1 ms. Of that, the
sky-view table at the high setting's 75 steps, the aerial-perspective volume and the
full-resolution ray march of the sky above the atmosphere and of terrain beyond 32 km took 3.63 ms
at the 95th percentile, and the composite 0.58 ms. Terrain came in at 1.16, 2.15 and 2.38 ms,
under its 3–5 ms, so terrain and the atmosphere together met the 6 ms that their rows' upper ends
leave them, at 5.59 ms at the 95th percentile. These are one seed's figures, taken at the clocks
the driver chose for the spike's light load: performance states P3 to P8, the graphics clock at a
median 975 of 2,115 MHz and the memory clock at a median 810 MHz. A pass within its row stays
within it at higher clocks; how far the atmosphere's time would fall at the clocks a full frame's
load brings was not measured. The UHD 620 column is unmeasured, its runs waived by the owner on
2026-10-08. R12.T10 drafts both columns' replacement from measured runs.

The UHD 620 rows sit near the floor that the bandwidth ratio sets, three times the discrete figure,
and they are credible only because each makes a real cut, which the table names: bloom at fewer
levels and quarter resolution, which moves some 20 to 25 MB a frame, about 1 ms at the machine's
20 to 25 GB/s; an ocean of about eight Gerstner components; and shadows from a horizon map rather
than a cascade. A cascade re-rasterises the terrain into a depth map, which costs a large fraction
of the terrain row itself. A horizon map is computed once per patch on the height workers, as part
of its bake, and costs a lookup and a compare a pixel; it depends on the terrain alone, not on the
sun (Max 1988), so it never needs rebaking as the sun moves. **Lean:** the low setting's shadows are
the horizon map alone, which frees the 2 to 3 ms that the compositing needs.

The table is GPU time for the view alone, and two costs sit outside it. The consoles beside the view
share the same GPU for their own canvases and for compositing, which on the UHD 620 is not free:
copying a rendered 1080p view into another canvas with `drawImage`, the way Babylon's own
multi-canvas feature does, measured about 4 to 5 ms a view on this machine, which is why every view
renders into its own canvas context on the one device instead
([Several views in one client](#several-views-in-one-client)). And the descent's heaviest cost is on
the CPU: every patch's heights come from the height function in WebAssembly workers, a descent
streams patches faster than any other situation, and in single-player the same processor also runs
the server. That cost has no row because it is not a frame cost, but it has a budget of its own.

Patch demand is about (200 · _v_ + 290 · |_ḣ_|) ÷ _h_ patches a second while the quadtree is still
refining. The first term is horizontal motion: some 4 patches a second in low orbit, 10 to 20 in a
powered descent and 100 to 300 on a low fast pass. The second is the re-bake of the near-nadir
levels each time the altitude halves, about 200 patches a halving, so a vertical descent at 20 m/s
through 200 m adds about 30 a second. Both hold for a selection that uses a patch at about five
times its own size in distance, which at 1080p across 60° is about 5 px per vertex spacing and, for
terrain sloped at 0.1 to 0.2, about 1 px of geometric error; at that selection the finest level's
patches, 17.7 m on an Earth at level 19, cap the demand below about 89 m. Re-derived with k that
ratio of distance to size (R05 Design note 19), the horizontal constant is 8k², 200 at k = 5, or
12k², 300, if the parents exposed at a level's inner edge are no longer cached; the vertical one is
3πk² ÷ ln 2, about 340 rather than 290, since each halving re-bakes a nadir disc of 3πk², about 236
patches: both within this paragraph's "about". The selection rule itself is stated in geometric
error, as virtual-globe renderers state it (CesiumJS's default is 2 px): a level is drawn where the
stated bound for its level, from [Level-of-detail
consistency](#level-of-detail-consistency-and-why-collision-agrees), subtends at most _τ_ pixels.
**Lean:** _τ_ = 1 px on the high setting and 2 px on the low. The constants scale as
1 ÷ (_τ_ θ_px)² and the cap as 1 ÷ (_τ_ θ_px), θ_px being a pixel's angle, which at 720p is 1.5
times 1080p's, so the low setting's demand is about a ninth of these figures, some 10 to 35 patches
a second on a low fast pass, and its cap a third. That ninth holds only while the low setting's k
exceeds about 3; at the five above it is about 1.7, and once k falls below about 2 the quadtree's
granularity floors each level ring at about 36 patches, so the low setting draws nearer a quarter
of the high setting's patches (R10 Design note 15). The descent spike's fixed-step records measured
both settings (R05, 2026-10-05, seed 7, ridges off, 1080p and 720p across 60°). Under the hard
bound, whose k is larger, the low setting's patch counts were 0.10–0.12 of the high setting's in
orbit and on the descent arc and 0.18–0.23 from the approach to the low fast pass; under the
calibrated bound, min(hard, 4σ), they were 0.25–0.28 from orbit to the low fast pass. On the low
fast pass its demand was 0.23 and 0.22 of the high setting's, 85 and 23 patches a second. Below
300 m the region forced to the finest level under the descending camera, the same on both
settings, brings the ratio towards 1. The present patch-to-distance ratio is provisional until the
level-of-detail test measures that bound, which then sets it per body. Demand is summed over every
view whose camera streams terrain, and on the UHD 620 a second streaming camera shares the same
workers, so secondary views stream at lower priority.

The UHD 620 machine is an i7-8665U, four cores and eight threads. In single-player the local
server's pool must be capped, since it defaults to every logical thread but one
(`crates/hyperion-server/src/config.rs`), and its arrival work, the sky's census and the coarse
pass, lands during a descent. Budgeting two cores for height workers, a 65 × 65 patch of heights
and their analytic gradients must bake in about 40 ms: about 10 µs a point, where a gradient costs
two to three times a bare height. That sustains some 50 patches a second, which carries the orbit,
a powered descent and, at the low setting's tolerance, a low fast pass; where demand outruns it,
refinement lags and the view annunciates `TERRAIN: STREAMING` until it catches up. These are
estimates like the table's, and [the descent test](#testing) states the measured figure as patches
a second sustained against that demand. On the high setting, normals at twice the mesh's resolution
take 129² gradients a patch, about four times the 65² counted here; with them, three workers on the
development machine's Ryzen 7 3700X sustained 40 to 45 patches a second in the descent spike's
judged run (R05, 2026-10-07).

The frame-time table is also for one photorealistic view at 1080p. On a bridge that is the main
screen's machine, and at 4K its fill-bound passes cost about four times as much, so it may render
below native and upscale. A station's wireframe, at 60 fps at 1080p on the UHD 620, has the table's
last row, and in single-player the wireframe instruments come out of the photorealistic view's
margin; both are argued in [Two deployments, one scene](#two-deployments-one-scene) and
[Several views in one client](#several-views-in-one-client), and both are measured in the
performance runs.

The UHD 620 has no memory of its own. Mesa's ANV driver offers three-quarters of system RAM as its
heap, so the limit is not the heap but the 16 GB machine, shared with the server, its coarse fields
and census, the height workers and the local language model. **Lean:** a GPU-resident ceiling of
1 GB on the low setting and 2 to 3 GB on the discrete target, budgeted beside the frame time.

| Item                   | Discrete target                                        | UHD 620 low setting                                                                                    |
| ---------------------- | ------------------------------------------------------ | ------------------------------------------------------------------------------------------------------ |
| Star cubemap           | 3,072² faces in `rgb9e5ufloat`, about 300 MB with mips | 1,024² faces in `rgb9e5ufloat`, 34 MB with mips; faint stars spread over about 2 px as the sky's grain |
| Height-texture cache   | 128–256 MB, normals at twice the mesh resolution       | About 64 MB, normals at mesh resolution; evicted least recently used, never under a grounded body      |
| Atmosphere tables      | Under 2 MB a planet                                    | Under 2 MB a planet                                                                                    |
| Coarse field, GPU copy | About 6 MB                                             | About 6 MB                                                                                             |
| Render targets         | About 70 MB at 1080p                                   | About 30 MB at 720p, `rgba16float` for colour                                                          |
| Shadows                | Four 2,048² cascades, 64 MB                            | None beyond the patch cache, which holds the horizon maps                                              |
| Clouds                 | 3D noise volumes, 10–20 MB                             | A two-dimensional layer, under 16 MB                                                                   |
| **Total, GPU**         | About 0.6–1 GB                                         | About 150–300 MB                                                                                       |

On the CPU side each height worker holds its own copy of the surveyed coarse field, posted once on
arrival and updated per chunk: at 2 to 15 MB and two or three workers, tens of megabytes. These
figures are computed from formats and sizes, not measured, and a second window with a device of its
own duplicates its caches in the same shared memory.

The ladder, stated as policy rather than as a list of numbers:

| Feature        | High                                                                      | Low                                                                                        |
| -------------- | ------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| Clouds         | Raymarched volume with temporal reprojection                              | Two-dimensional layer, correct albedo and optical depth                                    |
| Terrain detail | Full quadtree depth, GPU decoration                                       | Shallower quadtree away from the camera, decoration off; full depth under grounded bodies  |
| Ocean          | Gerstner waves, shoreline foam, sun glint; spectral waves later           | About eight Gerstner components, no refraction, sun glint retained                         |
| Shadows        | Cascaded, with cloud shadows on terrain                                   | A horizon map baked with each patch: terrain self-shadowing only                           |
| Atmosphere     | Full tables, aerial perspective on everything                             | Smaller tables, aerial perspective on terrain only                                         |
| Scatter        | Instanced, filtered by biome and slope                                    | Decoration off, or a token density; authoritative rocks always drawn under grounded bodies |
| Rings          | Transmittance with a two-part phase, both shadows, and particles close to | A textured annulus, the planet's analytic shadow, Hapke's shadow-hiding term               |
| Resolution     | 1080p native                                                              | 720p, presented upscaled                                                                   |

Three rules keep this honest. The renderer **states its setting on the display at all times**, and
annunciates `TERRAIN: DETAIL LIMITED` whenever the setting draws the surface below the detail the
camera's position warrants, so a player is never misled about whether they are seeing the real
surface. Whatever the setting and whatever the style, **the patches under every grounded body in
view are drawn at the finest level**, which is the one collision reads: the low setting may coarsen
what nothing is touching, but never what something is, or the guarantee of [Level-of-detail
consistency](#level-of-detail-consistency-and-why-collision-agrees) would hold only on the high
setting. It costs a handful of patches, and it binds the wireframe's contours too, because the
helm's picture of the ground at touchdown must agree with what the hull meets. And the low setting
is **built alongside the high one**, not retrofitted: a two-dimensional cloud layer written after
the volumetric one exists will never be tested and will rot.

## Fit with the consoles

`docs/frontend/ux-guidelines.md` was written for instruments: thin vector marks on `--surface-0`,
reserved colours, no gradients, redraw on demand. A view out of the window breaks nearly every one
of those sentences, and it is right to. The guide's own principle is that "colour, motion and sound
are spent only on things that need the operator's attention" — and a photometrically correct image
of a planet is not decoration, it is the most information-dense thing on the ship. What matters is
that the guide gains the exception _explicitly_, with its boundary written down, rather than being
quietly ignored by one display. The boundary is this: **the image is data and obeys physics;
everything drawn over the image is symbology and obeys the guide.**

### The contrast problem, which is the real one

The guide requires 6:1 contrast for any mark that carries meaning. That is a promise about marks on
the three console surfaces, the darkest of them `--surface-0` at `#05080d`. Over a sunlit cloud deck
the background is at the top of the display's range, and `--text` at `#c8d6e5` against it fails
badly — a flight-path marker that vanishes over a bright planet is a safety defect in the fiction
and a usability defect outside it.

The fix is one this project already uses. The galaxy map's cursor is 1.5 px of `--accent` cased in
3.5 px of `--surface-0`, because `--accent` falls to 1.3:1 over the top of the density ramp. (An
optical head-up display cannot do this: its combiner only adds light, so it manages contrast with
brightness instead, and it is the wrong precedent for a view drawn on a screen.) The guide already
states the rule for a mark over a raster: a `1px` `--surface-0` casing on each side, which is a
solid outline, "never a blur or a glow", and the cursor is that rule applied. **Lean:** the view
extends the casing rule to every symbology mark over the image, each stroked twice — its casing, at
the guide's widths, beneath the thin coloured stroke — so that each mark carries its own dark
surface with it and the guide's pairing holds against any background. Text over the image needs
the same protection and cannot get it from a canvas stroke: a DOM readout positioned over the
canvas sits on a `--surface-0` plate of its own, which is chrome and obeys the guide. The
alternative, dimming the image under the symbology, is rejected: it falsifies the image, and the
honest-data principle forbids it.

A second consequence is subtler. A real image contains real colours, so a rusty planet fills the
window with something close to `--status-warning` and a chlorophyll-green one with
`--status-nominal`. The guide's reservation of the status colours cannot apply to photons. It applies
to symbology and chrome, which is why symbology must be told from the image by _shape and outline_
first — a rule the guide already states in another form, and which now has teeth.

### What the guide must gain

Collected here so a plan can make the edits in one pass:

1. **A class of display: the view.** Perspective, redrawn every frame, always labelled as a view,
   and **not a spatial display**. The conventions that [three-dimensional spatial
   displays](../../frontend/ux-guidelines.md#graphs-schematics-and-spatial-displays) carry do not
   apply to it as a set, because a perspective image breaks nearly all of them: it is not
   orthographic and has no single scale, its camera rolls with the ship or flies free, it has no
   reference plane or stalks, and physics dims and shrinks what is far away, where the guide says
   nothing is dimmed by depth. What it keeps is stated instead: the contact symbol set with shape
   for type, the bracket reticle for the selection and `--target` for a commanded destination,
   predicted paths dashed, the frame name, the time, the style and the camera mode always shown,
   whenever the camera is off the hull a statement that positions are as seen from the ship at its
   light-time and not from the camera, and the canvas paired with a DOM list. This was already
   leaned in the single-player brainstorm's open question 11; this document supplies the wording it
   needs. The class covers both [render styles](#two-styles-of-one-renderer). The wireframe style
   has no photometric image, so item 2's exception does not reach it, and its marks, graticules and
   contours included, follow the guide's ordinary rules for colour, stroke and contrast. It is still
   a view and not a spatial display, because its perspective breaks the same conventions: no single
   scale, no orthographic axes, and size falling with distance.
2. **The rendered image is data.** The ban on gradients, glows, blurs and transparency governs
   console chrome, and a photometric image is not chrome. This is a new exception, not an instance of
   the raster-field one: that exception is single-hue and forbids "smoothing or interpolation that
   invents values", and the view is full colour and carries GPU decoration, which is invented detail
   by construction. The exception therefore comes with its own honesty rule: what the simulation did
   not compute is labelled, so the view states when decoration is on, beside its survey coverage and
   its quality setting. Symbology over it is unaffected. The guide's flash threshold, no content
   flashing more than three times a second and no large flashing area, still binds the image: it is
   a limit on what reaches the eye, not a rule for chrome, and strobe beacons, lightning, a tumbling
   hull's glint and aliasing stars are all held to it.
3. **Outlines for symbology, and only for symbology.** The raster rule's casing extended to every
   mark over the image, stated as the way contrast is met there, with its existing note that a
   casing is never a blur or a glow; and the `--surface-0` plate for DOM text over the image.
4. **Glare as a physical effect.** Bloom and veiling glare around a real light source are camera
   optics and are allowed on the image. They are never applied to symbology or chrome, which keeps
   them clear of the guide's ban on neon glow.
5. **Scale, when there is no scale bar.** A perspective image cannot carry the 1-2-5 scale bar the
   guide demands, because it has no single scale. The view instead states its **field of view in
   degrees** and its reference frame, and every target carries a range readout. That substitution
   should be written down rather than left as an omission.
6. **Exposure is an instrument.** The camera's exposure is shown as a value with its unit and its
   automation level (`AUTO`, `MAN` or `INHIBITED`), under the guide's existing rule that automation
   always shows who is in control.
7. **Limited detail is an annunciation on the view, not a data state.** If the renderer is not
   drawing the surface at the detail the camera's position warrants, the display says so. This is
   the honest-data principle applied to pixels: an approximated surface must not present itself as a
   surveyed one. It is also the hook by which the low settings of
   [the performance budget](#performance-budget) stay honest. It is not a data state, because the
   guide's data states belong to values, and a view's readouts keep theirs whatever the renderer
   draws: they come from the server and the height function, not from pixels. It is not an alert
   either, because the console raises it about its own drawing, not the server about the ship. So
   the view's label block carries it as an annunciation in `--text` on its plate, steady, while the
   condition holds, and tells its two causes apart because the operator answers them differently:
   - `TERRAIN: STREAMING` while patches have not arrived, which clears by itself;
   - `TERRAIN: DETAIL LIMITED` while the quality setting holds the surface below that detail, which
     does not.

   Neither uses a status colour or the word "degraded", which belongs to the Caution class, "Degraded
   or out of limits". Both join the nomenclature list.

8. **Motion, for a display that never stops.** The view redraws continuously. The guide already lets
   gauges and plots move at frame rate, but it has no display whose whole picture moves by itself.
   Its nearest precedents are the three-dimensional spatial displays, which redraw at frame rate while
   the operator runs their time and stop when the time is held; the view moves whenever the ship
   does, with no operator control to stop it. A move between camera presets is a cut, as a display
   switch is in the guide and a view switch is in flight simulators: the guide's 80 to 150 ms
   transition would make it a whip-pan, worse than a cut for orientation and too short to show the
   path. **Lean:** single-player offers an eased move of 0.4 s as a setting, off by default, and the
   main screen always cuts, because the people watching it did not trigger the move. Under
   `prefers-reduced-motion` the world cannot be frozen, but every _non-physical_ motion must stop:
   the optional eased moves, idle drift, and any animation not caused by the simulation. Camera
   flight is the operator's own motion and is never suppressed, but its smoothing, such as
   acceleration ramps and damping, and any automatic camera motion are non-physical and are removed
   under the setting. Readouts stay at the guide's 4 Hz.
9. **The main screen: a display without console chrome.** The main screen has no header strip, work
   area or navigation bar, and no input, where the guide builds every console on the same fixed
   frame with a header strip that looks identical at every station. It is also the display where a
   missing replay banner or a missing emergency would mislead the most people, so in their place it
   always shows:
   - a **status line** along one edge: the ship's name, the labelled ship time and the link state;
   - the **mode banner** for simulation, training, replay or pause, persistent, as the guide requires
     on every console;
   - an **alert annunciator**: the counts of active and unacknowledged emergency, warning and
     caution alerts, and the newest unacknowledged emergency or warning in full text, under the
     guide's flash and reverse-video rules and never as a flashing border or a screen-wide overlay.
     Alerts are acknowledged at the stations, and the main screen mirrors their state;
   - the **view's label block** (item 1), including who commands the camera, as automation always
     shows who is in control.

   Its text is sized for the room rather than by the console rem scale: at least 20′ of visual angle
   at the stated furthest viewing distance, the figure human-factors standards such as MIL-STD-1472
   give, recalled and to be checked against the standard's current revision before it is cited. On
   loss of the link the view holds its last frame, the time label goes stale with its `S`, and the
   status line reads `NO CARRIER`.

### Accessibility, which a canvas threatens

The guide pairs every spatial display's canvas with a DOM list of its marks, and this document
extends that answer to views, which are not spatial displays; it is worth restating because it is
easy to lose in a 3D display: every view's canvas, in either style and however many are open, is focusable, carries
an accessible name, and is **paired with a DOM list** of what is in that view — contacts, bodies and
the selected target — from which selection works by keyboard.
The star chart already works this way: its canvas is focusable and named and paired with its
systems list, and the view inherits that pattern rather than inventing one. What the view adds is
where its readouts sit. The chart's readings are B612 Mono text in the DOM around its canvas; the
view's go in `output` elements in B612 Mono, on their `--surface-0` plates, positioned over the
canvas rather than drawn into it. Every camera control, flying the free camera included, is
reachable from the keyboard, and none depends on hover or a right click. A main screen with no
input device keeps its DOM list, for screen readers and for the guide; its selection is made at
the station that commands it.

## Runtime and code shape

What exists today, checked against the tree rather than assumed:

- **The frames are built.** `hyperion_sim::coords` provides `GalacticPosition` (an `LyCell` of `i32`
  plus an `f64` metre offset), `SystemPosition`, `BodyPosition` and a `Frame` enum of
  `Galactic | System(SystemId) | Body(BodyId)`. `GalacticPosition::displacement_to` subtracts integer
  cells before offsets and is precisely the floating-origin primitive the renderer needs; the client
  already has the same cells-first differencing, in light-years, as `galacticDeltaLy` in
  `packages/protocol/src/position.ts`. System-frame selection with hysteresis is implemented in
  `hyperion_sim::galaxy::frame`; nothing selects `Frame::Body` yet.
- **The spatial view is orthographic and pure.** `apps/hyperion/src/renderer/src/spatial/` holds a
  declarative scene (`marks.ts`), a pure orthographic projection (`camera.ts`), a pure draw list with
  its own painter's-algorithm ordering (`drawList.ts`), pure picking (`pick.ts`) and one
  canvas-bound painter (`paint.ts`). Its `Camera` carries a single `pxPerUnit`, because the guide
  requires one scale for the whole picture. There is no perspective projection, no field of view and
  no view matrix anywhere in the client.
- **Bodies exist; surfaces do not.** Plan 14 is in progress: `hyperion-sim` has its `planetary`
  and `orbit` modules, body records with identity, mass, orbit and bulk properties, and the server
  answers `system_bodies` and `body_detail`, with `body_events` refused until P14.T31. The
  record's surface section — atmosphere, surface conditions, rotation and global figures — has no
  contents yet, and the hooks section is `not_modelled` everywhere: `surface_seed` has its wire
  form in `crates/hyperion-protocol/src/planetary/record.rs` but is not computed. Nothing of
  terrain exists at all: no surface crate, no coarse field and no height function.
- **Retarded time is built in the sim; nothing past it is.** Plan 12's `hyperion_sim::observe`
  (P12.T0–T2 and T4) gives the retarded reading, the observer, bearings and `observe_hit` for
  systems on their drift. Nothing yet evaluates a body's orbit at the retarded time, nothing on the
  wire uses the observed mode (P12.T3 and T6), and the Knowledge store (P12.T7) and the
  subscriptions (P12.T9) are unbuilt.
- **The client loads no WebAssembly at all**, and the sim's second-architecture check runs on
  `wasm32-wasip1` under wasmtime, by hand through `just test-wasm`, for the sim and the testkit
  only, outside `just ci`. The browser target, `wasm32-unknown-unknown`, is in no check.

That last point corrects a sentence the single-player brainstorm once carried, that CI checked
`hyperion-sim` bit for bit under WebAssembly; it now says what is checked, and keeps the shared
terrain conditional on the browser target joining those checks. The check was overstated because
the WASI run is a manual gate outside `just ci`, not CI. So there are two gaps rather than one — the
browser target is in no check, and the check that exists is not automatic — and both are
load-bearing for the whole shared-terrain argument. They should be closed before any terrain code is
written, the descent spike's included, rather than after. A third is smaller: the planetary golden
tests' headers (`crates/hyperion-sim/tests/planetary_*golden.rs`) and the sim-determinism skill
(`.claude/skills/sim-determinism/SKILL.md`) say that CI checks 64-bit Arm and wasm32, but there is no
hosted CI and no recipe runs Arm at all.

**Lean, for the shape:**

- **A crate of its own for the surface.** The client should not have to ship galaxy generation to ask
  for a height. Terrain belongs in a small crate — `hyperion-surface` — that compiles to
  `wasm32-unknown-unknown` for the client and links natively into the server. The wasm bundle then
  holds the height function alone. It cannot depend on `hyperion-sim`: the flight model is sim code,
  and collision needs heights, so the sim depends on the surface crate and a dependency back would be
  a cycle. The sim's `math`, `rng` and `units` modules therefore move into a crate beneath both, which
  each depends on, and their discipline — the `libm` pin, the Clippy bans — moves with them. That
  move is larger than it sounds: `rng` imports from `crate::id`, so part of `id` goes with it, and
  `rng/tags.rs` registers every stage's domain tags, the planetary ones included, so the registry
  splits; `hyperion-fit` also uses `hyperion_sim::math`, every ban's path changes in all three
  `clippy.toml` files, and the two new crates each gain one of their own, five in all. The
  coarse pass, which only the server runs, lives in the sim beside plan 14's planetary stage and hands
  its field to the surface crate as data.
- **Both wasm targets join the checks, automatically.** `wasm32-unknown-unknown` is exercised with
  `wasm-bindgen-test` in its Node mode, with Electron's own binary standing in for Node through
  `ELECTRON_RUN_AS_NODE=1`, so that the check runs on the V8 the client ships — 15.2 in Electron 44,
  where the system's Node carries V8 12.4 and its Chromium 153 carries V8 15.3. The runner has no
  variable for the binary and runs whatever `node` is first on `PATH`, so the recipe prepends a
  directory holding a `node` shim that runs Electron with `ELECTRON_RUN_AS_NODE=1`; Electron was
  checked to accept the runner's `--expose-gc` and `NODE_PATH` that way. The golden height
  files are asserted equal across native, wasip1 and the browser target. The fast goldens under
  both wasm targets join `just ci`, which fails with a pointer to the recipe that installs the tools
  when one is missing, and never skips; the slow wasip1 suite joins `just ci-slow`. There is no
  hosted CI, so a job beside `just ci` would be a gate nobody runs
  ([open question 12](#open-questions)). The existing pin of `libm` to `=0.2.16`, already
  documented as a generator-version change if bumped, is what makes that plausible; the same
  discipline extends to the surface crate.
- **Fixed-width SIMD is allowed; relaxed SIMD is banned.** WebAssembly's relaxed SIMD proposal
  permits two implementations to return different results for the same instruction, which is
  precisely what the determinism rules forbid. Fixed-width 128-bit SIMD is IEEE-exact and may be
  used, provided the code does not let the compiler reassociate a sum. The rule is new text in two
  places. The terrain hazards, relaxed SIMD with the rest of
  [Determinism hazards](#determinism-hazards-specific-to-terrain), become a new section of
  `.claude/skills/sim-determinism/SKILL.md`, which is where the determinism auditor reads them; that
  skill's `paths:`, and the review routing to the auditor, extend to `crates/hyperion-surface/**`
  and to the base crate beneath it when they are created, and its claim that CI checks Arm and
  wasm32 becomes what is true. `.claude/rules/rust-dev.md`, whose numeric-safety section covers only
  overflow, `as` and float comparison, gains a pointer there, and its crate-boundary section gains
  entries for both new crates. The bans themselves are mechanical: `disallowed-methods` in every
  `clippy.toml`, and the surface crate's `compile_error!`.
- **The renderer sits in the app, beside `spatial/`, not inside it.** A new `view/` directory holds
  the engine wrapper, the scene builder, the cameras, and both render styles with their shaders.
  What the two share — `vec3`, `frame` and the direction conventions — moves to a common place
  rather than being duplicated or bent. The
  orthographic `Camera` is not generalised into a perspective one: they are different display classes
  and merging them would produce a type that means neither.
- **The engine is loaded lazily.** Consoles that do not draw a scene must not pay for the engine's
  bundle, which a dynamic import and a manual chunk achieve.
- **The scene arrives as a subscription.** The view needs the bodies and craft near the camera and
  the ship, bounded by what the ship knows, so the client reports each view's camera position to its
  subscription, one camera per subscription or a list for several views, as plan 12's
  `alerts_observer` reports the observer; the pose never becomes ship state. The current protocol
  cannot express any of it: it has ten
  request kinds and no push, since `ServerMessage` has no notification. Plan 04 reserved the
  mechanism — a request whose response carries a `subscription: u32`, pushes as `notification`, and
  binary frames for bulk payloads — and assigned `subscribe` and `unsubscribe` to plan 12, which
  designs them as `SubscribeRequest { universe, topic }` with a `SubscriptionTopic` and
  `ServerMessage::Notification`. P12.T9 builds them, with `SubscriptionTopic::Alerts`, likely before
  the view, and it is the precedent the scene follows. The scene is therefore a new topic under plan
  12's `subscribe`, and the view either waits on plan 12's envelope work or builds it. Bodies are on
  rails, so they travel as orbital elements, reusing the protocol's `BodyOrbitDto` of parent, orbit
  and `valid_until` and its `DetailLevelDto` (until Knowledge exists the server grants whatever
  level is asked and says which). A body's elements are sent on arrival and again when Knowledge
  changes or its `valid_until` passes, and only craft are pushed at the 64 Hz tick rate. Every push
  states its simulation time and the time rate, and each client renders at that time extrapolated
  by its own elapsed time multiplied by the rate. The main screen's commanded camera selection
  travels in it too, as a field, beside the command that sets it. Stars are not in it: the sky
  arrives once per arrival, by a request of its own. **Lean:** JSON for the scene, as the
  single-player brainstorm concluded: for a scene of some 100 bodies and 10 craft, at some 250 to
  300 bytes a body and 400 a craft, that is about 0.25 MB/s, against some 2 MB/s if everything were
  pushed every tick.
- **Bulk payloads travel as binary frames.** The coarse field, 2 to 15 MB a planet, goes in
  surveyed-region chunks of at most 1 MB, the first use of plan 04's reservation. The server refuses
  inbound binary frames today (`crates/hyperion-server/src/ws.rs`) and has no outbound binary
  framing, so the coarse field is the first to need it. Sent as base64 in JSON, the way the density
  map travels today, a 15 MB field would become a 20 MB frame. The server's 16 MiB outbound budget
  does not refuse such a frame but sends it once the queue is empty
  (`crates/hyperion-server/src/limits.rs`), so it would wait for the queue to drain and then hold
  every scene push behind it, and it would need some 16 Mbit/s to beat the 10 s write timeout that
  closes the connection. The sky's list costs about 220 B a star as JSON: some 2 MB near the Sun,
  but 22 MB for 10⁵ stars, so past one frame it travels as binary frames of about 24 bytes a star,
  chunked like the coarse field, rather than as JSON parts in plan 04's reserved `response_part`.
  Even binary, 10⁵ stars is 2.4 MB and the nuclear disc's counts would be tens of megabytes, so the
  sky's N_max is what keeps the whole under the outbound budget. Plan 04 requires a new kind to
  enter its table of reserved kinds first, so the sky request, and any coarse-field request that
  asks for chunks, go there before either is built, each declaring its size class as
  `crates/hyperion-server/src/requests/mod.rs` requires of every kind — the sky's as large. The
  scene is a topic, not a kind, and the chunks are binary frames, not a kind. Adding a kind or an
  optional field leaves `PROTOCOL_VERSION` at 2 (`crates/hyperion-protocol/src/lib.rs`); whether the
  first `notification` message and the first binary frames bump it is to be decided.
- **Workers hand heights to the render thread.** A `GPUDevice` cannot be shared between threads, so
  height workers transfer their results to the thread that owns the device, or the whole renderer
  runs in a worker on an `OffscreenCanvas`; every view's canvas context is configured on that one
  thread against the one device, and each view renders into its own context's texture rather than
  being copied from a shared canvas. Each worker runs its own WebAssembly instance and holds its own
  copy of the coarse field. That avoids `SharedArrayBuffer` and the cross-origin isolation it needs
  — isolation that pages loaded with `loadFile`, as the client's are in production, cannot declare;
  a custom scheme served through `protocol.handle` could send the headers, and Chromium's
  `SharedArrayBuffer` feature switch would lift the requirement. **Lean:** neither is adopted.
  `SharedArrayBuffer` would not remove the copies, because a WebAssembly instance without threads
  reads only its own linear memory, and Rust's WebAssembly threads need a nightly rebuild of the
  standard library; the copies cost tens of megabytes. ~~The renderer's Content Security Policy
  (`apps/hyperion/src/renderer/index.html`) must change before the first height worker: its
  `script-src 'self'` blocks WebAssembly compilation without `'wasm-unsafe-eval'`. That change needs
  the owner's sign-off, since `.claude/rules/typescript-dev.md` keeps the policy strict and requires
  asking before it is loosened.~~ Superseded 2026-09-30 (R04.T10.a): the policy blocks compilation
  on the page's own thread only; a same-origin module worker has no policy of its own under
  `file://` or the dev server and compiles under today's, so the policy does not change (see
  "Awaiting the owner"). Bundling the workers as same-origin module files, as Vite does by
  default, keeps `blob:` and a `worker-src` of their own out of it, and the WGSL-only adapter keeps
  Babylon's CDN out of it too ([The engine is kept at arm's length](#the-engine-is-kept-at-arms-length)).

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
- **Style switching.** Switching a view between the wireframe and the photorealistic style leaves
  its camera and projection identical: a body's projected position is the same in both. And the
  frame-change continuity test holds for a free camera, whose frame changes apart from the ship's.
- **Level-of-detail selection.** Which patches a given camera selects, and that the set is a
  function of the camera's pose, field of view and viewport, the quality setting and the grounded
  bodies' positions alone — not of history, not of arrival order — and meets the setting's
  screen-space error _τ_. A wrong answer here is a visible pop, and it is entirely testable on the
  CPU.
- **The grounded-body rule.** On the low setting as on the high, and in either style, the selected
  set includes the finest-level patches under every grounded body in view.
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
  [determinism discipline](../../../.claude/skills/sim-determinism/SKILL.md) the sim already
  follows, in `just ci` as [open question 12](#open-questions) settles. The testkit's golden
  loader reads and blesses files with `std::fs`, at paths built from `CARGO_MANIFEST_DIR`, and
  `wasm32-unknown-unknown` has no file system, so it gains an arm that embeds them with
  `include_str!`. That arm needs literal paths and cannot bless, so goldens are blessed natively
  and only compared there, and the golden tests carry the `wasm_bindgen_test` attribute on that
  target.
- **Collision agrees with what is drawn.** The collision query and the finest level's mesh must
  return the same height, and each coarser level must stay inside the stated bound for its level.
  This is the test that makes "the same terrain on both sides" a checkable claim instead of an
  intention.
- **The scene is bounded by Knowledge.** Wherever a view's camera is placed, no scene the server
  sends contains a craft that is not a contact or a surface cell that is not surveyed, as plan 12
  checks for its own observed queries.
- **Two clients agree.** Two clients subscribed to one scene place every body and craft at the same
  position to within one push, which is what lets the main screen and a station show one world.
- **Physical tables and constants.** The Rayleigh coefficients recompute Bucholtz's
  4.51 × 10⁻²⁷ cm² at 550 nm to 1%; every baked atmosphere table matches a path-traced reference to
  5% in radiance; the rings' baked shadowing factor reproduces Salo and French's B-ring brightening
  from 4° to 26° elevation to within five percentage points, and the ring's extinction is conserved
  across the split between slab and instances; and every Gerstner parameter set keeps its summed
  steepness, Σ Qᵢ wᵢ Aᵢ, at or below 1 (Finch).
- **The flush-to-zero probes** fail, in `f64` and `f32`, on a thread whose flush or
  denormals-are-zero mode has been turned on, so that the server's refusal to generate is tested.

**With a GPU, by hand, and recorded:**

- **The precision scene.** A hull plate at 1 m, a moon at 10⁸ m and a planet at 1 au in one frame,
  with no depth fighting and no jitter as the camera translates and rotates. The single-player
  brainstorm already names this test; it belongs in a scene that is kept, not a throwaway.
- **The frame-change scene.** Crossing a system and a body boundary while watching a body's projected
  position, asserting continuity to within a pixel.
- **The descent.** Orbit to a metre above the ground in one continuous motion, watching for pops,
  cracks between levels of detail, and the moment streaming cannot keep up, and recording tile
  demand against worker throughput, in patches a second sustained, against the budget's demand.
- **The performance runs** of [the budget](#performance-budget), on both GPUs, recorded with their
  settings so that a regression is visible as a number: the cockpit measured together with its
  wireframe instruments, a station wireframe during a descent measured alone, and the resident
  memory against the memory ceiling, the coarse field's copy into each worker included.
- **Several views and stills.** One engine drawing a full-window canvas and two small ones, each the
  right way up, with no GPU time in copies and a resize of one leaving the others' attachments alone;
  and a still rendered at the batch cap on the UHD 620 with no lost device, no lost GPU process and
  the live view holding its frame time.

**Golden images are rejected for CI.** They differ across drivers, across Mesa versions and between
software and hardware rasterisation, and a test that fails for reasons unrelated to the change is
worse than no test. Headless software rendering exists and is fine for a smoke test that answers
"did every shader compile and did a frame complete", which is worth having and is all it is worth
having. It runs Electron's WebGPU on SwiftShader, the CPU Vulkan implementation Electron ships, as
Dawn's fallback adapter (`--enable-unsafe-webgpu --use-webgpu-adapter=swiftshader`, in the test
harness only: the unsafe flag is needed there because Chromium blocklists CPU adapters, and is never
set by the client). It reads the frame back with
`copyTextureToBuffer` rather than a canvas copy or a page capture, and asserts only properties of
it, such as no texel that is not finite, never a stored image. SwiftShader exposes subgroups but not
`shader-f16`, unlike the UHD 620, so the smoke test runs the no-f16 path there, and once more with
subgroups withheld for the no-subgroup path. Mesa's
lavapipe is not used: Dawn does not treat it as a fallback adapter. SwiftShader checks correctness,
never performance. Image comparison stays a local, deliberate act with a human looking at it.

## Decisions

Settled with the project owner on 2026-09-22:

- **The performance floor.** A modern discrete GPU is the design target at 1080p60, and every rendering
  feature must have a documented low setting that stays playable — not necessarily at 60 fps — on the
  development machine's Intel UHD 620. The real renderer must always be runnable locally.
- **WebGPU is required.** Electron's main process sets the switches that force it, and no WebGL2
  fallback is maintained. Compute shaders may therefore be assumed. Which switches is this document's
  finding rather than the ruling's: on Linux, where Chromium under X11 gives WebGPU no adapter at all,
  they are the Vulkan-compositing set of
  [The graphics API, and the Intel problem](#the-graphics-api-and-the-intel-problem), set on every
  Linux machine, and `--enable-unsafe-webgpu` is not among them.
- **This document's scope.** It covers the engine, the real-scale view, and planet rendering including
  the deterministic surface-generation architecture that rendering depends on. Planet content — biomes as
  habitats, life, resources, what a landing party finds — remains for a later planets document. This
  partly answers the single-player brainstorm's open question 7.

Settled with the project owner on 2026-09-28:

- **A free camera in two styles.** The outcome of this rendering effort is a free-flying camera that
  switches between a wireframe style, in the graphic language of an Artemis console, drawing vector
  hulls, graticule bodies, orbits and symbology, and a photorealistic one, fully rendered like No
  Man's Sky. Consoles need both. The style is Artemis's; the bodies, orbits and perspective view are
  HYPERION's own, as a check of what Artemis draws found on 2026-09-29.
  [Render styles and multiple views](#render-styles-and-multiple-views) works it through.
- **Two deployments of equal standing** in the design, given as owner context, of which the
  single-player one is built first, as the single-player brainstorm's decisions have it. A bridge
  crew: one dedicated machine is the ship's window, the main screen, on a television or projector,
  photorealistic and usually on the room's best GPU, with its camera commanded from a station, while
  station machines draw the same surroundings as a wireframe, from a camera of their own or slaved to
  the main screen's. And single-player, as in Elite Dangerous or No Man's Sky: one machine, the
  photorealistic cockpit view full-window, wireframe instruments beside or over it, and the camera
  set by the player, with seat, chase and free presets, the seat the default.

Inherited from the earlier brainstorms, and unchanged except where noted:

- **The realism ruling applies**: where a choice is open, the most realistic answer wins unless it is
  technically infeasible.
- **The view starts as a wireframe**, and full 3D planets follow, as in No Man's Sky but at real scale.
  The 2026-09-28 ruling keeps that order and keeps the wireframe as a style beside them.
- **Travel within a system is open**, the whole of a system's space including its star.

Recommended here, as technical choices rather than rulings, each argued in the section it links to:

- **The engine.** Babylon.js in the renderer, **behind an adapter** that keeps every HYPERION-specific
  mechanism engine-agnostic, which is what makes the choice reversible, with its large-world feature
  off. The adapter's shaders are WGSL only: a GLSL compile is an error naming the shader, so Babylon
  never fetches its compilers from its CDN, and a test renders every material and post-process with
  the network disabled ([The engine](#the-engine)).
- **The Linux platform.** The switches
  `--ozone-platform=x11 --use-angle=vulkan --enable-features=Vulkan,VulkanFromANGLE,DefaultANGLEVulkan`
  and `--enable-dawn-features=enable_subgroups_intel_gen9`, set before `ready` on every Linux
  machine, so that a Wayland session runs the client through XWayland. The client refuses the photorealistic
  style on a fallback adapter, reports adapter, device and GPU-process loss as a ship-system fault,
  and after a crash loop relaunches once without Vulkan into a declared mode without the
  photorealistic style. Subgroup kernels have no-subgroup twins as their reference, chosen by feature
  detection ([The graphics API, and the Intel problem](#the-graphics-api-and-the-intel-problem)).
- **Depth.** Reversed-Z with a floating-point depth buffer and an infinite far plane; no logarithmic
  depth and no global depth bias; transparent layers drawn after opaque geometry in analytic order,
  testing depth without writing it; ring and planet shadows computed analytically
  ([Depth](#depth-reversed-z-and-no-logarithmic-depth)).
- **Positions.** Camera-relative rendering with per-patch `f64` origins, from the frames the
  simulation already defines, with patch origins as a body-fixed position type rotated into the body
  frame through plan 14's `body_fixed_at`. Drawn positions are apparent, light-time and aberration
  together, except the camera's local body, drawn geometrically at the present time
  ([The floating origin](#the-floating-origin-is-already-in-the-simulation)).
- **Light.** Absolute photometric units throughout: illuminance from each star's absolute V
  magnitude, with V = 0 at 2.54 µlx; surfaces shaded from the visual geometric albedo and phase
  integral, not the Bond albedo; a photographic exposure model under `AUTO` or `MAN`, and AgX tone
  mapping. Colour targets are `rgba16float` with pre-exposure, a star's disc clamped to the format's
  maximum or drawn analytically, and `rgba32float` is not used
  ([Luminance](#luminance-in-physical-units-and-an-exposure-model)).
- **Two styles.** Wireframe and photorealistic as two styles of one renderer, chosen per client and
  display, over one server scene at its stated time. The wireframe draws terrain once, depth only, at
  a 4 px tolerance from the same height function, and exposes its stars per sprite
  ([Two styles of one renderer](#two-styles-of-one-renderer)).
- **The main screen.** A client role whose camera selection is ship state, set by a station's
  command. Control is granted per station, by default to the Captain and Helm; the last selection the
  server accepts stands and is shown; one station at a time flies the free camera, and the Captain
  can always take it. The server integrates that flight from the station's input within 100 ms on a
  wired LAN and saves it with the session, on a presentation track outside the replay's bit-for-bit
  check. A local view's camera is integrated by its client and reported to the scene subscription,
  never ship state ([The free camera](#the-free-camera)).
- **The free camera.** Its own frame selection, reaching the current system and bounded by what the
  ship knows. Moves between presets are cuts; single-player offers a 0.4 s eased move, off by
  default, and the main screen always cuts.
- **Several views and stills.** Every view on one `GPUDevice`, each through its own
  `GPUCanvasContext` whose texture the adapter wraps as the camera's output, since Babylon's
  `registerView` copies each view with a `drawImage` that costs 4 to 5 ms at 1080p on the UHD 620.
  Each view has its own budget, one photorealistic at a time on the low setting, and a station's
  wireframe holds 60 fps at 1080p on a UHD 620-class machine, a lean beyond the owner's floor
  ([Several views in one client](#several-views-in-one-client)). Still images are rendered
  progressively by the client that asks for them and kept in its store
  ([Still images](#still-images)).
- **The scene.** A topic under plan 12's `subscribe`, in JSON, with bodies as orbital elements, only
  craft pushed at the tick rate, and every push stating its simulation time and time rate
  ([Runtime and code shape](#runtime-and-code-shape)).
- **Terrain geometry.** A cube-sphere quadtree on S2's quadratic warp, with patches of 64 × 64 quads
  that morph height as well as position, and levels chosen by screen-space geometric error, 1 px on
  the high setting and 2 px on the low
  ([The geometry](#the-geometry-a-quadtree-on-a-cube-sphere) and
  [Performance budget](#performance-budget)).
- **The coarse field.** Computed **on the server** at one cube-sphere level per body, the shallowest
  with cells of about 40 km or less, between levels 5 and 8, and sent to the client as binary frames,
  with all fine detail synthesised locally; Knowledge gates its coverage but never its accuracy. Its
  pass rasterises plates as a Voronoi diagram on its own cells, matches plan 14's hypsometric
  standard deviation σ_h, runs a seasonal energy-balance climate with a labelled precipitation
  heuristic, erodes where the world has or had surface liquid, and hands craters wider than D_b,
  twice the level's largest cell edge, to the client as a list
  ([The coarse global pass](#the-coarse-global-pass-once-per-planet)).
- **Seeds.** The surface seed is server-only. The client's local synthesis takes a detail seed, a
  block output of the universe seed on its own tag, `body.surface.detail`, which is a discipline for
  an honest client rather than a security boundary; plan 14's `BodyHooksDto` carries `detail_seed` in
  place of `surface_seed` before P14.T23 lands
  ([Knowledge, and the surface seed](#knowledge-and-the-surface-seed)).
- **Truth and decoration.** A terrain band limit of 2 m at a vertex spacing of at most 0.375 m
  (level 19 on an Earth); every rigid instance 0.2 m tall or more authoritative; and the material
  class authoritative, baked beside the height and never re-decided by a shader. Local draws come
  from the sim's `Stream` on registered `surface.*` tags
  ([The line between truth and decoration](#the-line-between-truth-and-decoration)).
- **Knowledge.** It gates coverage for the image, and coverage and resolution for every number, so a
  readout beyond the surveyed resolution carries its uncertainty. The coverage record is a log of
  survey passes on P12.T7's store ([open question 4](#open-questions)).
- **Atmospheres.** Hillaire 2020 from a list of physically parameterised terms, with Rayleigh
  scattering from each gas's dispersion formula rather than the tutorials' coefficients, and aerosols
  from an inventory plan 14 owns. Where the multiple-scattering term drifts, a table baked offline
  with a converged solver replaces it, and a thick cloud deck splits the atmosphere; every baked table
  matches a path tracer to 5%, and Bruneton's model is not the fallback ([Atmosphere](#atmosphere)).
- **Rings.** An optical-depth slab with a baked shadowing factor between particles and a radial
  profile generated from processes, giving way to instanced particles once the largest subtends a
  pixel ([Rings](#rings)).
- **The sky.** A request of its own, fed by plan 06's stellar brief at each star's emitted time, and
  limited by a naked-eye magnitude set per direction from Crumey's threshold at a field factor of 1.4
  — about 6.6 in the band to 7.4 at the poles near the Sun — and by a count budget. Camera views, the
  main screen among them, use a noise-floor limit of their own, about V 10. Colours come from a
  spectral library, and the band from a per-population cumulative luminosity function
  ([The sky](#the-sky)).
- **The budget.** A low setting targeting 30 fps at 720p on the UHD 620, against a discrete reference
  of the RTX 4060 class, with shadows from a horizon map alone; and GPU-resident memory within 1 GB on
  the low setting and 2 to 3 GB on the discrete target
  ([Performance budget](#performance-budget)).
- **Limited detail is an annunciation**, `TERRAIN: STREAMING` or `TERRAIN: DETAIL LIMITED` in the
  view's label block, not a data state (item 7 of
  [What the guide must gain](#what-the-guide-must-gain)).
- **The code.** A new `hyperion-surface` crate, with the sim's `math`, `rng` and `units` moved into a
  crate beneath it and the sim; five `clippy.toml` files, each banning the `algebraic_*` methods;
  relaxed SIMD a build failure in the surface crate; and the terrain hazards a new section of the
  `sim-determinism` skill, with a pointer in `rust-dev.md`. Both wasm targets' fast goldens join
  `just ci` — the browser target under Electron's own V8, through a `node` shim on `PATH`, and wasip1
  under wasmtime — and the slow wasip1 suite joins `just ci-slow`, before any terrain code. Height
  workers hold their own copies, with no `SharedArrayBuffer` and no cross-origin isolation. A
  headless smoke test on SwiftShader reads frames back to a buffer and asserts properties, never
  images ([Runtime and code shape](#runtime-and-code-shape) and [Testing](#testing)).

Awaiting the owner:

- **The Content Security Policy.** ~~The renderer's policy must gain `'wasm-unsafe-eval'` before
  the first height worker.~~ **Ruled 2026-09-30 (R04.T10.a, by the owner's authority given to the
  implementing lane): the policy does not change.** Tested on the repo's Electron 44.4.3, a
  same-origin module worker loaded from `file://` or the dev server has no policy of its own and
  compiles WebAssembly under today's `script-src 'self'`, while the page's own thread is refused
  and a `blob:` worker is refused at creation (R04 Design note 16). So the workers stay same-origin
  module files, the render thread never compiles WebAssembly (the loader names a policy refusal as
  its own fault, and a source test keeps the generated module's imports in workers), and the
  worker's `file://` reach over the disk is accepted while the renderer runs only its own code.
  Serving through a custom scheme with a header policy remains open should that change.
  `.claude/rules/typescript-dev.md`'s rule stands.
- **The guide.** The edits this document asks of `docs/frontend/ux-guidelines.md`, collected under
  [What the guide must gain](#what-the-guide-must-gain), are proposals, since guide additions are the
  owner's call.

## Open questions

Each question is marked **Closed**, where research settled it; **Lean**, where research recommends an
answer and a measurement or check remains; or **Open**. Closed questions keep their place and their
answers, so that the numbers cited elsewhere stay put. The review of 2026-09-29 raised further
questions that research settled outright — the cube-sphere warp, the relief statistic, the crater
boundary, the depth policy, the colour-target format, the magnitude zero point, main-screen authority
and the rest — and their answers are leans in the body, listed under [Decisions](#decisions).

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
   redesigns the low setting rather than triggering a native renderer. The spike's one judged run,
   on the RTX 3080 (R05, 2026-10-07), fit with headroom: our main-thread time was 3.6 ms and the
   GPU's passes 5.7 ms at the 95th percentile, against 13.3 ms. Chromium's presentation times
   missed the frame rows, but on that path (X11, NVIDIA, Vulkan) they are taken on the CPU when the
   swap completes, not at the vertical blank. The frame count shows a new frame at 99.7% of the
   display's refreshes, so the delivered frames met the 95th percentile and the rule did not fire.
   What failed is ours: 1.0–2.2% of frames skipped in four segments near the ground, against 1%.
   The native replay, the toggles' comparison and the UHD 620 half were not run (waived by the
   owner, 2026-10-08), so the replay's clause is untested. R12's consolidated runs measure both
   machines again, and a discrete result that meets the condition comes back to this rule.
3. **Whether Hillaire's model holds for thick atmospheres.** **Lean.** The per-planet precompute this
   question once asked about is gone: Hillaire's tables rebuild in under a millisecond. Neither model
   is validated at Venus's or Titan's optical depths, and no real-time method validated on either was
   found. Hillaire's tables are fully spherical, so his transmittance, single scattering, sky-view and
   aerial perspective stand; only his analytic multiple-scattering term fails. Where it drifts from a
   converged reference, a table baked offline per atmosphere with a converged solver — discrete
   ordinates, as in DISORT (Stamnes et al. 1988), or a spherical Monte Carlo, in `f64` — replaces it,
   and a cloud deck of optical depth above about 10 splits the atmosphere into Hillaire's tables over
   a baked reflecting boundary and a baked plane-parallel table beneath. Bruneton's iterated orders
   are not the fallback: they diverge in exactly this regime. What remains is the path-traced
   reference itself, which every baked table must match to 5% in radiance before Venus- and
   Titan-class atmospheres ship ([Atmosphere](#atmosphere)).
4. **Whether the coarse field is persisted.** **Lean:** not persisted. The server recomputes it and
   caches it keyed by seed, generator version and body, so the coarse pass stays bit-identical on
   every machine a server may run on. Knowledge records coverage only, as survey passes: one versioned
   JSON line per orbital, close-range or landed pass, in `knowledge/surveys.v1.jsonl` beside plan 12's
   `contacts.v1.jsonl`, holding the body, the time span, the source, the resolution, and the cells as
   a run-length-encoded cube-sphere quadtree cover. On load the server folds the passes into a byte
   per cell holding the best resolution reached — 384 KiB for a body at level 8, a sixteenth of that
   at level 6 — which gates the wire and the readouts and is never stored. The plan that implements
   step 7 builds the record on P12.T7's store; the body-level overlay belongs to the sensors plan that
   follows sessions ([Knowledge, and the surface seed](#knowledge-and-the-surface-seed)). A save made
   under another generator version already refuses to open, so surveyed ground cannot move under a
   save; the real risk is changing the coarse pass, its wire quantisation or the local synthesis
   without bumping `GENERATOR_VERSION`, so all three belong to it. What remains is the encoding, which
   is the plan's.
5. **Erosion: where the physics runs.** **Lean:** the stream-power law runs where it can. In the
   coarse pass, Tzathas et al.'s analytical solution produces the coarse elevation, flow directions,
   drainage area and steepness index wherever the world has or had surface liquid; a world dry now
   erodes only for plan 14's wet epoch, above a base level from a depression-filling pass (step 4 of
   [The coarse global pass](#the-coarse-global-pass-once-per-planet)). Below a coarse cell, a
   Dendry-style network anchored to those flow directions supplies the channels, with stream-power
   profiles. The network's geometry is a heuristic and labelled so. Dendry's published network has
   four levels, so spanning a coarse cell down to the band limit takes several instances stacked,
   their number set by the body's cell size. The reference code's 150 µs a point (10 s for a 512² grid
   on four cores) comes from rebuilding about 150 cells for every point, and a naive port to
   WebAssembly was estimated at 300 to 500 µs for one network of some fifteen levels. The budget is
   about 10 µs a point for the whole height function, its gradient included, so the network is built
   per patch and cached by integer cell, as [The per-query evaluation](#the-per-query-evaluation) sets
   out, and the spike measures it against an expected 1 to 3 µs a point. What remains is that
   estimate, which must be redone for stacked instances.
6. **The collision wavelength.** **Closed:** a fixed band limit of 2 m, at the finest level's vertex
   spacing of at most 0.375 m, which keeps it within about a third of its amplitude on the mesh's
   triangles (R05 Design note 3); it changes only with the generator version and is stated in the
   surface crate's documentation, because both sides depend on it. Obstacles below it are rocks,
   and every rigid instance 0.2 m tall or more is authoritative, under the 0.3 m hazard that
   landing-hazard detection is specified against. Collision reads the finest level's
   piecewise-linear surface, which is drawn with its morph held at zero around every grounded or
   descending body in view
   ([The line between truth and decoration](#the-line-between-truth-and-decoration)). A finer tier,
   nested inside this one, is added only if crews on foot ever touch terrain. Craters between 1 and
   2 m are left as open question 18.
7. **A planet-fixed ocean.** **Lean.** The spherical ocean is published, as a projected grid that
   follows the camera and a reflectance model from space, and so is an ocean on terrain patches:
   Outerra displaces its patches with Gerstner waves. What is our own is keeping it consistent across
   levels of detail under the collision rules, and driving its wave spectrum from the climate field.
   For the shoreline on procedural planetary terrain the only account found is Outerra's, a distance
   map with waves chosen by depth; Jeschke et al.'s water surface wavelets meet shores, but not at
   planetary scale. Start from the published sphere and glint, and treat the quadtree ocean's
   consistency and the shoreline as the risk ([Oceans and ice](#oceans-and-ice)).
8. **Rings close to.** **Lean.** The criterion is stated under [Rings](#rings), as an angle. Adopt it,
   with the baked shadowing factor between particles, and Hapke's shadow-hiding term on the low
   setting, and measure the instancing. SpaceEngine's volumetric rings run at 150 fps and more at
   1080p on an RTX 2080, at 85% resolution with temporal rendering, and at 45 fps at 4K, which
   suggests that the discrete target can afford particles, a 4K main screen only by rendering below
   native, and the UHD 620 not at all, so its low setting keeps the annulus.
9. **Climate for worlds that are not Earth-like.** **Lean.** Energy-balance models and Köppen
   classification are Earth-centred; tidally locked "eyeball" worlds, high-obliquity worlds where the
   equator is the coldest place, airless bodies and Titan-like hydrocarbon cycles each break them
   differently. The regime classifier belongs to plan 14's surface conditions, not to this document,
   because the coarse pass is constrained to plan 14's figures and those figures are wrong without
   one. Plan 14's ice fraction, taken from the latitude at which the zonal temperature crosses
   freezing, assumes the poles are coldest and the ice is water, which fails for a locked world's
   nightside ice, for obliquities between 54° and 126°, where stable equatorial ice belts form (Kilic
   et al. 2018), and for Titan and Pluto, where water ice is bedrock; and its equator–pole contrast
   needs a sign and, for a locked world, the substellar axis. The classifier has three fields rather
   than one list: a thermal regime from Koll's (2022) redistribution index on optical depth, surface
   pressure and equilibrium temperature, with the pressure below which carbon dioxide collapses on the
   night side (Wordsworth 2015) as its thin end; a forcing from the locking state, the length of the
   solar day and the obliquity, since slow rotators stay temperate at nearly twice the flux (Yang et
   al. 2014); and a condensable from the retained species against their phase diagrams. Each regime
   names its coarse model — radiative equilibrium with thermal inertia for airless and thin-atmosphere
   worlds, a seasonal or body-fixed energy-balance model (Ramirez 2024) for the rest, an isothermal
   surface for a Venus — and this document then says only what each one runs and classifies. The
   energy-balance model gives monthly temperature but no precipitation, so precipitation is the
   labelled heuristic of step 3 of
   [The coarse global pass](#the-coarse-global-pass-once-per-planet), checked offline against
   ExoPlaSim, and the model's ice edges can be checked against the four climate states of the FILLET
   intercomparison (Barnes et al. 2025). The index's thresholds and the onset of slow-rotator climates
   still need checking against the papers.
10. **Whether the scene subscription needs binary frames.** **Closed** for the scene: JSON, with
    bodies as orbital elements, only craft pushed at the tick rate, and every push stating its
    simulation time and time rate. Bulk payloads, the coarse field first, travel as binary frames, as
    [Runtime and code shape](#runtime-and-code-shape) sets out.
11. **Whether scatter is ever collidable.** **Closed:** yes, split by height rather than by the band
    limit. Every rigid instance 0.2 m tall or more is authoritative from the first phase that draws
    scatter: placed by hash in cells per size octave, with its abundance from a rock size–frequency
    law and an analytic shape the server answers for, and drawn wherever it could touch a grounded or
    descending body, whatever the scatter setting. Smaller scatter is decoration, culled where it
    intersects a grounded body.
12. **How the wasm targets join the determinism checks.** **Lean:** `wasm-bindgen-test` in Node mode
    with Electron's own binary as Node, so that the check exercises the V8 the client ships; the fast
    goldens under both wasm targets in `just ci`, which fails and never skips when a tool is missing;
    the slow wasip1 suite in `just ci-slow`; all of it before any terrain code, the descent spike's
    included. The runner has no variable for its Node binary and runs the first `node` on `PATH`, so
    the recipe puts there a shim that runs Electron with `ELECTRON_RUN_AS_NODE=1`; with Electron
    44.4.3 the shim ran V8 15.2 and accepted the runner's `--expose-gc` and `NODE_PATH`. What remains
    is a run of the full test suite that way, and what the extra runs add to `just ci`'s time.
13. **How the sky is selected.** **Lean:** plan 06's stellar brief, built in P06.T33 and T34,
    supplies luminosity and effective temperature, and the sky is a request kind of its own rather
    than a set of range queries, which `MAX_QUERY_CELLS` would cut short in any case. Its census
    counts stars, not systems, and keeps the brightest N_max. The limit is per direction, so the
    census selects to the location's deepest one, about 7.4 near the Sun, and to the deepest limit of
    the camera views open, about V 10, within N_max. The galaxy brainstorm's layering
    ([Placing star systems](galaxy-generation.md#placing-star-systems)) is necessary but not
    sufficient: one radius for every layer is still volume-limited. Each mass layer's radius is
    bounded by the brightest absolute magnitude plan 06's tracks reach in any phase of its life,
    dimmed by the least extinction in any direction. For layer C that phase is the post-AGB crossing,
    M_V ≈ −5.3 for a 2.5 M☉ progenitor and −3.8 to −4 for the long crossings near 1 M☉, which gives
    about 7,400 ly; for D, post-AGB to −7.1, about 17,000 ly; and for E the whole galaxy, where a
    150 M☉ star reaches M_V ≈ −10.95 and 101,000 ly. That is brighter than any observed star: the
    tracks hold their most massive stars cooler than 10,000 K above log L 5.8 for 1.8–2.4 × 10⁵
    years, beyond the Humphreys–Davidson limit, which is a finding for plan 06
    ([open question 17](#open-questions)); the rule follows the tracks while they stand. The dwarfs'
    bolometric corrections (`crates/hyperion-sim/src/stellar/photometry.rs`) also make late M giants
    up to 1.7 mag too bright, inflating the AGB peaks. The rule is the ceiling; the caps are derived,
    not chosen. Each is the smallest radius beyond which the layer's expected number of stars brighter
    than the limit falls below one, computed at the census from the layer's cumulative luminosity
    function (the quadrature the band needs), the density field and the extinction along some 48 rays.
    Near the Sun that gives about 3,000 ly for C, set by some tens of yellow post-AGB stars within
    reach; about 4,300 for D, whose bright giants and Cepheids at about −4.5 reach some 2,600 ly in the
    plane at its mean extinction of about 0.55 mag per 1,000 ly, and whose post-AGB stars have yet to
    be counted; and about 10,000 for E, whose supergiants at −8 to −9.5 reach 6,000–14,000 ly through
    low-extinction windows, so that 10,000 misses only a handful. The C and D caps are estimates for
    the census benchmark to confirm. In the nuclear disc dust holds even E to about 1,000 ly, so the
    caps shrink to a few hundred to about 1,000 ly and the census's cost with them; above the disc they
    grow towards the rule's bounds, since the disc's supergiants 10,000–30,000 ly away are then
    naked-eye stars. Layers A and B never leave the main sequence within the galaxy's age. Their
    brightest visible phase is the contraction onto it, a T Tauri star of M_V ≈ 4.8 at 0.5 Myr for
    0.75 M☉ (Baraffe et al. 2015, which plan 06's tracks match to 0.1 mag), which reaches V 6.5 at
    about 70 ly before its birth cloud's extinction. The earlier protostar, M_V ≈ 2.2 with its
    accretion light, is buried in its envelope, so the sky treats plan 06's Class 0/I phase
    (`protostar_class`) as dark in V; without that, the rule would stay at about 250 ly.

    The sky also needs what the grid does not place — feature members such as open clusters like the
    Pleiades and Hyades, globular clusters and the nuclear cluster, the catalogue classes and the
    streams — which reach the range query through its `SystemSource` hook. The server still passes it
    empty (`crates/hyperion-server/src/requests/galaxy.rs`): plan 09's `FeatureMemberSource`
    (P09.T23) is built in the sim but held back from the server until P09.T40's cached cluster models,
    because a cold 50 ly query in a globular core takes 12.7 s against a 20 ms target; the centre's
    members and plan 10's streams follow. Candidates are skipped by mass, which is one uniform word on
    `system.primary_mass` through a monotone quantile, so a threshold on the raw word costs little,
    and by age, tested under every population component the cell can hold, since the age word is
    independent of the component picked, before the density is evaluated. Both skips test each
    cell's light-time interval, at most about 220 years wide for a 128 ly cell, rather than each
    candidate's. Only accepted stars take `observe::retarded`, about 1.1 µs each, 0.1 s per 10⁵, and
    their brief is `BriefModel::brief_at` at the emitted time, once plan 06 has tested its routes over
    the retarded interval (open question 17). Consider, as a generator-version change, generating a
    cell's mass words in sorted order, so that the census skips light candidates without opening
    their streams. Each cell's bright subset is cached, so that a jump of up to 1,000 ly reuses most of
    the cells; systems move, and a cell is chosen by its systems' epoch positions and tested at the
    time asked, so the cache keeps the range query's padding rule. Under the near-Sun caps the census
    visits about 4 million cells of 32, 64 and 128 ly, some 3 s at 0.7 µs each, but the candidate
    streams opened for the mass skip scale with the systems enclosed: some 6 × 10⁷ near the Sun, 5–10
    CPU-seconds on first arrival, and some 5 × 10⁹ in the inner bulge, 400–800 CPU-seconds under
    fixed caps. The pool (`crates/hyperion-server/src/compute/pool.rs`) has two priorities,
    interactive jobs that fail fast with `queue_full` and bulk jobs that wait, so the census runs at
    bulk priority, never as interactive chunks, and never holds the charts' queries behind it. The
    cost needs a benchmark, and the counts re-deriving ([open question 19](#open-questions)).

14. **Whether Vulkan compositing is stable on the UHD 620.** **Open.** The probe's runs lasted
    seconds. A 30-minute soak on the development machine with the chosen switch set — the
    photorealistic view and the consoles together, a `<video>`, window resizes, a second window, and
    the display blanked and restored — records GPU-process restarts, `vkAcquireNextImageKHR` messages
    and DOM corruption, then repeats without `DefaultANGLEVulkan` to settle whether it is needed
    ([The graphics API, and the Intel problem](#the-graphics-api-and-the-intel-problem)).
15. **Whether per-view canvas contexts need Babylon's internals.** **Open.** The per-view route uses
    `engine._device` and `_disableEngineYFlip`, pinned by an adapter test. Step 1's proof shows
    whether it works; a public hook would retire the pin. Separately, whether same-origin child
    windows can share the opener's device is for a prototype
    ([Several views in one client](#several-views-in-one-client)).
16. **The sky's budgets.** **Open.** N_max and the sprite budget per location, which the nuclear
    disc's 10⁵ to 10⁶ visible stars and a camera view's 3 × 10⁵ near the Sun both outrun any fixed
    figure sized for the Sun's sky; the parameters of the camera's noise-floor model; and the field
    factor as an observer setting, whose default of 1.4 is Crumey's experienced observer
    ([The sky](#the-sky)).
17. **Plan 06's stellar model, as the sky uses it.** **Open,** for plan 06. `BriefModel::new`
    (`crates/hyperion-sim/src/stellar/brief.rs`) tests its routes over the clock window of ±1,000
    years only, so a brief asked at an emitted time more than about 1,000 years back is untested; it
    needs a validity interval over the retarded interval, a constructor such as
    `new_for(galaxy, record, earliest_emitted)`. And the tracks' cool supergiants above the
    Humphreys–Davidson limit need a physics ruling on whether plan 06 enforces the cool-side limit,
    log L ≤ 5.8, under which E's brightest would be M_V ≈ −9.7. The sky's photometry also needs plan
    06, or the sky plan, to give Class 0/I protostars no V magnitude or a large extinction.
18. **Craters between 1 and 2 m.** **Open.** Below the 2 m band limit they are decoration, scars in the
    normal detail only; but a crater 1–2 m across is some 0.2–0.4 m deep, near the rock threshold. If
    that matters for a footpad, bowls at least 0.2 m deep could become authoritative analytic dips,
    like the rocks ([The line between truth and decoration](#the-line-between-truth-and-decoration)).
19. **The sky's counts at generator version 15.** **Open.** The counts of [The sky](#the-sky) and of
    question 13 are the Milky Way fixture's at generator version 14, before plan 06's track fates
    (P06.T30) and the Chabrier refit, and C's cap is estimated. The census benchmark re-derives them at
    version 15, explains why the candidates opened (6 × 10⁷ near the Sun, 5 × 10⁹ in the inner bulge)
    exceed the systems that layers C to E hold (2.6 × 10⁷ and 2 × 10⁹), and checks the per-layer counts
    against the range query's benchmark `range_500ly_floor_d`, which returns 37,675 systems at version 15.
20. **Plan 14's relief and its gravity scaling.** **Open,** a physics ruling. Plan 14's greatest
    relief, 20 km × (g⊕ ÷ g) × a lithosphere factor, gives about 53 km for Mars and 121 km for the Moon
    against some 29 and 20 km observed. Plan 14 is asked to publish σ_h instead, and its scaling with
    gravity, and the lithosphere factor's part in it, go to a research agent before they are ruled
    (step 2 of [The coarse global pass](#the-coarse-global-pass-once-per-planet)).
21. **Whether the first push and the first binary frames bump the protocol.** **Open.** A new request
    kind or an optional field leaves `PROTOCOL_VERSION` at 2; the first `notification` message and the
    first binary frames may not ([Runtime and code shape](#runtime-and-code-shape)).

## Suggested order of attack

Not a plan, only the dependency order a plan would follow. Steps 1 to 3 are the ones that retire risk;
everything after them is additive. Steps 5 to 9 also wait on plan 14, which is in progress: its
radii, masses, bulk compositions, equilibrium temperatures and orbits are served already, and the
atmospheres and Bond albedos are derived but not yet carried, while rotation, the global figures and
the hooks that carry the surface seed are not built at all. Steps 1 and 3 do not, because they run on
scenes built by hand — the precision scenes of [Testing](#testing) and a hand-parameterised test
planet — and the wireframe draws generated bodies once plan 14 supplies them. Plan 12 is partly
built: the sim's `hyperion_sim::observe` (P12.T0–T2 and T4, commit 3cdfd02) gives retarded readings
for systems on their drift, but nothing evaluates a body's orbit at the retarded time, the observed
mode is not on the wire (P12.T3 and T6), and the Knowledge store (P12.T7) and the subscriptions
(P12.T9) are not built. Until the retarded evaluation reaches bodies and the wire, the scene the
server sends is the present state, which within a system differs from the retarded one by seconds to
hours of light-time. Each feature's low setting is built in that feature's own step rather than at
the end, as the budget's third rule requires, and the first one brings the limited-detail
annunciation with it.

Three things are done now rather than in a step, because they cost little today and a great deal
later. Plan 14 is amended before P14.T23 lands, so that `BodyHooksDto` carries `detail_seed` in place
of `surface_seed`, the client's parser follows, and the surface seed never reaches a client. The
root, sim and fitting crates' `clippy.toml` files ban the `algebraic_*` methods, and base's and the
surface crate's from their creation, five in all. And the claim in the
sim-determinism skill and the planetary golden tests' headers, that CI checks 64-bit Arm and wasm32,
is made to say what runs.

1. **Foundations and the wireframe.** First what the first WebGPU frame on this machine needs: the
   Electron main process's Linux switches and Gen9 subgroup toggle, the refusal of fallback adapters,
   the adapter- and device-loss fault and the crash-loop relaunch, followed by the soak of
   [open question 14](#open-questions). Then the engine adapter, with its WGSL-only guard and offline
   render test, and one canvas context per view on one device, proved with a cockpit canvas and two
   instrument canvases (open question 15). Camera-relative differencing, reversed-Z, frame-change
   rebasing, the photometric pipeline with exposure — in the wireframe the tone curve is applied per
   star sprite, and the full-screen pass waits for step 5 — and `VIEW`'s wireframe style, the
   permanent mode, at real scale with stars at their true magnitudes, seen from the free camera and
   its seat, chase and free presets, with the camera's own frame selection, body frames included.
   Those magnitudes come from plan 06's stellar brief on the range rows, which is built, so this step
   waits on nothing of plan 06; until step 4 the stars are the range query's volume-limited set with
   their briefs, and the view says so. The scene subscription, as a topic under plan 12's `subscribe`
   (P12.T9), built here if plan 12 has not reached it, with each view's camera reported to it and
   every push stating its time and time rate. The guide edits go to the owner here, since the
   wireframe already needs the view as a display class, the exposure instrument and the motion rule.
   This is the precision test rig, and the point at which the depth and precision tests exist.
2. **The wasm targets in the determinism checks.** `wasm32-unknown-unknown` under `wasm-bindgen-test`,
   run by Electron through a `node` shim, with both wasm targets' fast goldens in `just ci` and the
   slow wasip1 suite in `just ci-slow`; the terrain hazards added to the sim-determinism skill, with a
   pointer in `rust-dev.md`; and the client loading its first WebAssembly, in a worker, under its
   Content Security Policy unchanged (ruled 2026-09-30, R04.T10.a). Nothing terrain-shaped is
   built yet, but this is the check the shared terrain rests on, so it comes before the first line of
   terrain code, spike code included.
3. **The descent spike, which is the gate.** An Earth-sized test planet with Earth's atmosphere as
   measured, its continental aerosol at an optical depth of 0.1 at 550 nm, Ångström exponent 1.3 and
   single-scattering albedo 0.92, and Hillaire's reference aerosol, at an optical depth of
   5.3 × 10⁻³, kept as a comparison mode, from orbit to a metre above the ground, terrain from sim
   code in WebAssembly workers and the atmosphere drawn every frame, since after terrain it is one
   of the heaviest passes on the Intel part. The descent is scripted and seeded, identical every
   run, and records frame intervals at the 50th, 95th and 99th percentiles, main-thread time split
   between our code, the engine and idle, GPU time per pass — which the forced switches make
   available on Linux, quantised to 65.5 µs unless Dawn's `timestamp_quantization` toggle is
   disabled, as the measurement runs do — patches a second sustained against the demand of
   (200 · _v_ + 290 · |_ḣ_|) ÷ _h_, the level-of-detail bound per level that the selection's _τ_
   rests on, upload bytes, pipeline-creation stalls, garbage-collection pauses, and resident memory
   against the ceiling, a 15 MB coarse field posted to three workers included. It passes at 1080p60
   on the discrete target and at 30 fps at 720p on the UHD 620's low setting. The project has no
   discrete GPU today, so that half of the measurement needs one borrowed or rented; the UHD 620
   half runs on the machine that exists, and a failure there is already an answer. The
   single-player brainstorm's statement of the spike carries the same criterion. It decides whether
   the browser carries the planets, and it should happen before anything depends on the answer.
4. **The sky.** The sky request, entered first in plan 04's table of reserved kinds with the large size
   class, with its census to the per-direction naked-eye limit and the camera views' limit, a count
   budget, its caps derived there and each star at its retarded time; the per-population cumulative
   luminosity function, a new quadrature in the sim, for the caps and the band; plan 06's `BriefModel`
   tested over the retarded interval ([open question 17](#open-questions)); faint stars baked and
   bright or near ones drawn as sprites, the galactic band from the model, the local star as a
   limb-darkened disc, and colour from a spectral-library table. Plan 09's feature members join once
   P09.T40's cached cluster models let the server pass them. Cheap, highly visible, and it exercises
   the photometry.
5. **Lit bodies at real scale**, from plan 14's radii and rotation and its albedo, exported as a visual
   geometric albedo and phase integral beside the Bond albedo, with correct phase and the terminator.
   Still no surfaces. This is the first photorealistic style, so the style switch, a second view and
   the per-view budget arrive here. Once the single-player brainstorm's sessions, ship state and
   closed-loop commands exist, the main screen follows: a client role in `Hello`, the main-screen role,
   its camera commands and the scene's camera field, and the stations' control grants.
6. **Atmospheres**, parameterised from plan 14's composition, pressure, temperature and gravity and from
   the aerosol and absorber inventory it is asked to add beside P14.T24.a, with per-gas refractivity
   tables, aerial perspective on terrain, and the thick ones' baked tables checked against a path
   tracer ([open question 3](#open-questions)).
7. **The surface generator.** The shared `math`, `rng` and `units` moved into the crate beneath the
   sim, the `hyperion-surface` crate with its own `clippy.toml` and its relaxed-SIMD `compile_error!`,
   and plan 14's σ_h, volatile history and crater contract, asked for before the pass is written. The
   coarse global pass in the sim on the server, at each body's own level; its binary wire chunks, with
   Knowledge's coverage gating and its record of survey passes, which needs P12.T7's store; the
   `body.surface.detail` tag behind the detail seed plan 14 by then sends; and golden height files
   added to step 2's parity checks across native and both wasm targets. Nothing is drawn from it yet.
8. **Terrain.** The quadtree on the quadratic warp, patch streaming, height textures with their
   parent-height channel, morphing, the authoritative materials, the survey coverage readout and the
   readouts' uncertainty — and the tests that collision agrees with the picture and that the
   level-of-detail bound holds, with the morph held at zero near grounded bodies. The low setting's
   horizon-map shadows bake with the patches.
9. **The rest of the surface**, in rough order of value: GPU decoration with its label on the view,
   scatter with its authoritative rocks, clouds, ocean and glint, and rings with their baked shadowing
   and a radial profile from plan 14. Still images, rendered progressively through the offscreen
   pipeline, and soaked at the batch cap on the UHD 620.
10. **The measurements.** The budget's estimates replaced by figures measured on both GPUs and kept
    under version control — the cockpit with its instruments, a station's wireframe during a descent,
    and resident memory among them — and the ladder's settings adjusted to what they show.

The end state is the owner's goal: a free camera that switches any view between the wireframe and
the photorealistic style, anywhere the ship's knowledge reaches, with still images on request.

## Sources

Figures above are rounded, and several are explicitly estimates rather than measurements. Everything
here should be re-checked against these when it becomes code, in the same way the galaxy brainstorm's
figures are. Where a source could not be reached, it is marked, because the alternative is a document
that reads as more certain than the research was. **From memory** marks a citation recalled rather
than read: its details, and the figure it supports, must be checked before either becomes code.

**Games and references** (none re-checked against a primary source unless marked)

- No Man's Sky: I. McKendrick, _Continuous World Generation in No Man's Sky_, GDC 2017,
  <https://gdcvault.com/play/1024265> (the abstract was verified; the per-region, per-level
  evaluation with no stored world is from recall of the talk). Planet radii of tens of kilometres are
  a community measurement, not a published figure, for example
  <https://steamcommunity.com/app/275850/discussions/0/2952595757891767014> (about 130 km for a large
  planet); the larger worlds of 2025 are in Hello Games' _Worlds Part II_ notes of January 2025,
  <https://www.nomanssky.com/worlds-part-ii-update>.
- Artemis: Spaceship Bridge Simulator: its wiki, <http://artemiswiki.pbworks.com>, for the flat
  sector and for which stations set the main screen (Helm and Weapons), and the community protocol
  documentation, <https://artemis-nerds.github.io/protocol-docs>, which has no planet or orbit object
  and a `SetMainScreenPacket`.
- EmptyEpsilon's per-station main-screen control, in daid/EmptyEpsilon at commit 310bebd
  (`src/menus/shipSelectionScreen.cpp:823`, and `CMD_SET_MAIN_SCREEN_SETTING` in `playerInfo.cpp`).
- Elite Dangerous's landable planets, airless or with thin atmospheres since Odyssey (2021); Star
  Citizen's move to 64-bit world coordinates; SpaceEngine's per-patch height textures; and Kerbal
  Space Program's PQS terrain and discrete floating origin: from public talks and community
  documentation, **not re-checked**.
- Microsoft Flight Simulator's camera definitions, whose `Transition` flag defaults to off, so that a
  view switch is a cut. <https://docs.flightsimulator.com> (Cameras CFG).

**Engines and the platform** (all verified 2026-09-22 unless marked)

- Babylon.js releases, including the 9.0.0 notes of 26 March 2026 naming Large World Rendering, a
  geospatial camera and 3D Tiles support, each linked to its documentation page (Large World
  Rendering at <https://aka.ms/babylon9LWDoc>); 9.28.0 of 24 September 2026, with minor releases
  weekly. <https://github.com/BabylonJS/Babylon.js/releases>. An earlier draft of this document gave
  9.0.0 the date of 8.0.0, 27 March 2025.
- Babylon.js reversed-Z: `useReverseDepthBuffer` setting `GEQUAL` and `clearDepth(0.0)`, in
  `packages/dev/core/src/Engines/thinEngine.pure.ts`.
- Babylon.js Large World Rendering (verified 2026-09-23): the documentation source
  `content/features/featuresDeepDive/scene/large_world.md` in the BabylonJS/Documentation repository,
  to which <https://aka.ms/babylon9LWDoc> redirects; the override in
  `packages/dev/core/src/Materials/floatingOriginMatrixOverrides.ts` and the eye position in
  `scene.pure.ts`, at 9.27.1; its origin as an experiment in 8.28.3 (pull request 17183); and the
  maintainer's announcement, calling it experimental.
  <https://forum.babylonjs.com/t/new-large-world-rendering/61114>. An earlier draft of this document,
  unable to read the documentation site, called the feature unverified.
- Babylon.js compatibility (verified 2026-09-23): golden rule 1 of `contributing.md` in the engine
  repository, and `content/breaking-changes.md` in the documentation repository, which records visual
  changes such as PBR rough metals in 7.45.0, each with a flag to restore the old look. An earlier
  draft of this document found no policy.
- Babylon.js internals at `@babylonjs/core` 9.28.0 (verified 2026-09-29): `registerView` rendering
  through one working canvas and copying with `drawImage`
  (`Engines/AbstractEngine/abstractEngine.views.pure.js:86-141`); `wrapWebGPUTexture` and
  `updateWrappedWebGPUTexture` (`Engines/webgpuEngine.pure.d.ts:805-826`) and `_device` (`:216`);
  `outputRenderTarget` (`Cameras/camera.pure.d.ts:241`); WebXR's use of both, with
  `_disableEngineYFlip` (`XR/webXRWebGPURenderTargetTextureProvider.js:49-92`); and the lazy fetch of
  glslang and twgsl from `cdn.babylonjs.com` (`Engines/webgpuEngine.pure.js:1633-1637, 3318-3322`,
  `Engines/WebGPU/webgpuTintWASM.js:19-41`), which only `forceGLSL`
  (`Materials/material.pure.d.ts:657`) or a GLSL shader triggers.
- three.js r186 of 8 September 2026. <https://threejs.org/> and
  <https://github.com/mrdoob/three.js/releases>
- three.js migration guide, recording breaking changes in essentially every release and advising
  upgrades in increments of ten, and recording r181's brighter physically-based lighting and r185's
  darker ambient occlusion with no flag to restore the old look.
  <https://github.com/mrdoob/three.js/wiki/Migration-Guide>
- three.js `reversedDepthBuffer` and `logarithmicDepthBuffer` as options of the shared renderer, in
  `src/renderers/common/Renderer.js` (lines 96–97 at r186) and `WebGLRenderer.js` (line 3701,
  requiring `EXT_clip_control`), and `logdepthbuf_fragment.glsl.js` (writes `gl_FragDepth`, so early
  depth rejection is lost). Whether the two options may be combined is **unverified**.
- PlayCanvas 2.22.3 of 21 September 2026. <https://github.com/playcanvas/engine/releases>
- WebGPU implementation status (edited 2026-08-13): Linux default enablement for Intel Gen12+ from
  Chrome 144 and NVIDIA from Chrome 147, under Wayland, everything else behind a flag, and no default
  on Windows for Arm64; its suggested Linux switches are
  `--enable-unsafe-webgpu --ozone-platform=x11 --use-angle=vulkan --enable-features=Vulkan,VulkanFromANGLE`.
  <https://github.com/gpuweb/gpuweb/wiki/Implementation-Status>
- Chromium at tag 152.0.7977.130 (verified 2026-09-29): `software_rendering_list.json` entry 186,
  identical on main, blocking WebGPU's Vulkan-through-GL-interop display path except for Intel Gen12+
  with Mesa 22.0 or newer and NVIDIA 535.183.01 or newer; that path declared only by the Wayland
  backend (`ui/ozone/platform/wayland/ozone_platform_wayland.cc:372`) and interop disabled under X11
  (`gpu/ipc/service/gpu_init.cc:722-731`), leaving Dawn its null backend
  (`gpu/command_buffer/service/webgpu_decoder_impl.cc:1745-1760`); the Wayland backend's refusal of
  Vulkan (`wayland_surface_factory.cc:248-250`); the `Vulkan` feature off outside Android
  (`gpu/config/gpu_finch_features.cc:255-261`); `--enable-unsafe-webgpu` enabling experimental
  features (`content/child/runtime_features.cc:423`), lifting the CPU-adapter blocklist
  (`gpu/command_buffer/service/webgpu_blocklist_impl.cc:157-158`, `webgpu_decoder_impl.cc:1619-1625`)
  and lifting timestamp coarsening; `--use-webgpu-adapter` in `gpu/config/gpu_switches.cc`; WebGPU
  exposed to workers (`WorkerNavigator includes NavigatorGPU`); the canvas context's snapshot path and
  the unimplemented Linux front-buffer read (`gpu_canvas_context.cc`, crbug.com/40902474);
  `GPUDevice` neither serialisable nor transferable (`gpu_device.idl:7-10`); the Linux GPU watchdog
  of 15 s (`gpu/ipc/common/gpu_watchdog_timeout.h:18-39`); and three GPU-process crashes disabling GPU
  mode (`content/browser/gpu/gpu_process_host.cc:184-193`).
- Dawn's Vulkan backend: subgroups refused on Gen9 (`PhysicalDeviceVk.cpp`, about lines 1437–1450,
  crbug.com/391680973) unless the `enable_subgroups_intel_gen9` toggle is set, documented as
  "Enables subgroups on Intel Gen9 by polyfilling subgroupBroadcast(f16)" (`Toggles.cpp:618-621`) and
  honoured in release builds (Chromium's `gpu/command_buffer/service/service_utils.cc:296-300`).
  Its `timestamp_quantization` toggle (`Toggles.cpp:267-271`) masks the low word of each converted
  timestamp with `kTimestampQuantizationMask`, 0xFFFF0000 (`src/dawn/common/Constants.h:110`), so
  timestamps step by 65,536 ns unless it is disabled (read at `main`, 2026-10-08).
  <https://dawn.googlesource.com/dawn>
- gpuweb issue 5022: a report, naming no GPU, that adding `Vulkan,VulkanFromANGLE,DefaultANGLEVulkan`
  together stopped a `vkAcquireNextImageKHR` hang. <https://github.com/gpuweb/gpuweb/issues/5022>
- A local probe of 2026-09-29, not in the repository: Electron 44.4.3 on the UHD 620 (`8086:3ea0`),
  Mesa 26.2.3 ANV, Xorg with i3. No switches gave no adapter, `--enable-unsafe-webgpu` alone gave
  SwiftShader, and every Vulkan set gave the hardware adapter with correct output; `subgroups`
  appeared only with `enable_subgroups_intel_gen9`; `EXT_clip_control` was present on ANGLE over
  Vulkan and over GL; `float32-filterable` and `float32-blendable` were exposed; a 1080p `drawImage`
  copy took 4.6–4.9 ms against 0.6–0.8 ms to render; and a `node` shim running Electron as Node
  reported V8 15.2.124.28-electron.0 and accepted `--expose-gc` and `NODE_PATH`.
- Electron 44 bundling Chromium 152 <https://github.com/electron/electron/releases>; native Wayland
  from Electron 38 <https://www.electronjs.org/blog/electron-38-0>; same-origin `window.open`
  children in the opener's process (`docs/api/window-open.md:9-13`). The system Node's V8 12.4 and
  Chromium 153's V8 15.3 are **not re-checked**.
- W3C WebGPU specification: depth range, `depth32float`, `depthCompare`, and the absence of a
  tessellation stage. <https://www.w3.org/TR/webgpu/>. Chrome's _What's new in WebGPU_ for 119
  (`float32-filterable`) and 132 (`float32-blendable`), <https://developer.chrome.com>.
- Khronos WebGL extension registry, `EXT_clip_control`: extension 51, written against WebGL 1.0 and
  promoted to Community Approved on 2 November 2023.
  <https://registry.khronos.org/webgl/extensions/EXT_clip_control/>. Exposed by Chrome 152 on this
  machine (the probe; ANGLE's `vk_caps_utils.cpp:1053-1054`). An earlier draft of this document,
  relying on MDN's WebGL page, said the extension did not exist.
- The Wayland xdg-foreign protocol, which parents a top-level window to another's without embedding
  it. <https://wayland.app/protocols/xdg-foreign-unstable-v2>. That Wayland has no cross-process
  surface embedding otherwise is **from memory**.
- WebAssembly relaxed SIMD, whose instructions may return different results on different hardware.
  <https://github.com/WebAssembly/relaxed-simd>
- Electron multithreading, for workers and `nodeIntegrationInWorker`.
  <https://www.electronjs.org/docs/latest/tutorial/multithreading>
- Chromium SwiftShader, for headless software rendering.
  <https://chromium.googlesource.com/chromium/src/+/main/docs/gpu/swiftshader.md> (returned 503 on
  2026-09-29; to be re-checked).
- `wasm-bindgen-test-runner` at wasm-bindgen 0.2.129, which runs `Command::new("node")` with no
  variable for the binary (`crates/cli/src/wasm_bindgen_test_runner/node.rs:171-187`).
  <https://github.com/rustwasm/wasm-bindgen>
- Bevy 0.19.1 of 13 August 2026, and 0.20.0-rc.2 of 28 September 2026, after release intervals from
  0.15 to 0.19 of 4.8, 5.2, 3.5 and 5.2 months; its atmosphere settings, whose four lookup tables are
  Hillaire's, and its atmosphere module, whose documentation states that it implements Hillaire 2020
  (an earlier draft of this document called it Bruneton-style); its medium in
  `crates/bevy_light/src/atmosphere.rs`, any number of terms each with its own density and phase
  function, and a Mars preset whose dust follows Schneegans et al. 2024 (doi:10.1111/cgf.15010,
  **not read**: it returned 403); and the `big_space` crate for large worlds, whose 0.12 depends on
  Bevy ^0.18. <https://github.com/bevyengine/bevy/releases>,
  <https://docs.rs/bevy/0.19.1/bevy/pbr/struct.AtmosphereSettings.html>,
  <https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_pbr/src/atmosphere/mod.rs>,
  <https://github.com/aevyrie/big_space>
- Godot's large world coordinates, converting vectors and physics and emulating double precision in
  the standard vertex transform but not in custom world-space shaders, with custom editor and
  export-template builds.
  <https://docs.godotengine.org/en/stable/tutorials/physics/large_world_coordinates.html>
- wgpu 30.0.1. <https://github.com/gfx-rs/wgpu/releases>
- Mesa: ANV's heap of three-quarters of system RAM above 4 GB (`anv_restrict_sys_heap_size` in
  `src/intel/vulkan/anv_physical_device.c`), and Gen9's preemption levels
  (`src/intel/vulkan/genX_init_state.c:388-392, 518-527`); Linux's i915 preemption timeout of 640 ms
  and heartbeat of 2,500 ms (`drivers/gpu/drm/i915/Kconfig.profile`). Windows' default `TdrDelay` of
  2 s is from Microsoft's documentation, **not fetched**.
- Hardware figures for the budget: the UHD 620's 24 execution units at about 1.1 GHz, the i7-8665U's
  four cores and eight threads, the RTX 4060 class's roughly 15 TFLOP/s and 272 GB/s, and the
  PlayStation 4's 1.84 TFLOP/s. **Cited from memory and not re-checked**; they set the scale of
  estimates, not any measured figure.
- CesiumJS's `Globe.maximumScreenSpaceError`, 2 px by default.
  <https://cesium.com/learn/cesiumjs/ref-doc/Globe.html>

**Precision, depth and time**

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
- Nathan Reed, _Depth Precision Visualized_, 3 July 2015, the reversed-Z error figures and float
  reversed-Z's near-constant relative spacing (verified 2026-09-29).
  <https://www.reedbeta.com/blog/depth-precision-visualized/>
- Urban and Seidelmann (eds.) 2013, _Explanatory Supplement to the Astronomical Almanac_, 3rd
  edition, University Science Books, for planetary aberration as light-time and aberration together.
  **From memory.**
- NAIF SPICE, `spkezr_c`: the "LT", "CN" and "+S" corrections, and body-fixed orientation evaluated
  at the emitted time (verified 2026-09-29).
  <https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/C/cspice/spkezr_c.html>

**Terrain geometry and level of detail**

- Musgrave, Kolb and Mace 1989, _The synthesis and rendering of eroded fractal terrains_, SIGGRAPH — the
  summed-octave construction that makes a coarse evaluation a low-pass of a fine one.
- Lagae et al. 2010, _A Survey of Procedural Noise Functions_, Computer Graphics Forum 29(8), for
  Perlin and simplex noise being only approximately band-limited. **Pages from memory.**
- Strugar 2009, _Continuous Distance-Dependent Level of Detail for Rendering Heightmaps_ (CDLOD).
  **Unverified**: the original site is defunct; the technique is widely cited.
- Losasso and Hoppe 2004, _Geometry clipmaps: terrain rendering using nested regular grids_.
  **Unverified** fetch; widely documented.
- Dimitrijević, Lambers et al. 2016, _Comparison of spherical cube map projections used in planet-sized
  terrain rendering_.
- Google's S2 geometry library, `src/s2/s2coords.h`: the ratio of largest to smallest cell area of
  about 5.2 for the gnomonic cube, 1.414 for the tangent warp and 2.082 for the quadratic, which it
  finds about three times faster than the tangent. <https://github.com/google/s2geometry>
- Calabretta and Greisen 2002, A&A 395, 1077: the COBE quadrilateralised spherical cube's polynomial
  inverse, in error by up to 24″.
- Westerteiger et al. 2012, _Spherical Terrain Rendering using the hierarchical HEALPix grid_; Górski
  et al. 2005, ApJ 622, 759, for HEALPix itself (**from memory**).
- Max 1988, _Horizon mapping: shadows for bump-mapped surfaces_, The Visual Computer 4; Sloan and Cohen
  2000, _Interactive Horizon Mapping_, Eurographics Rendering Workshop — the low setting's shadows.
  **From memory.**
- Jimenez 2014, _Next Generation Post Processing in Call of Duty: Advanced Warfare_, SIGGRAPH Advances
  in Real-Time Rendering, for bloom at reduced resolution. **From memory.**

**Surface generation**

- Cortial, Peytavie, Galin and Guérin 2019, _Procedural Tectonic Planets_, Computer Graphics Forum
  38(2), doi:10.1111/cgf.13614, which builds its plates on STRIPACK.
- Renka 1997, _Algorithm 772: STRIPACK, Delaunay triangulation and Voronoi diagram on the surface of a
  sphere_, ACM TOMS 23(3), 416–434, doi:10.1145/275323.275329 — a general-purpose package.
- Brown 1979, _Voronoi diagrams from convex hulls_, Information Processing Letters 9, 223,
  doi:10.1016/0020-0190(79)90074-7: the convex hull of points on a sphere is their Delaunay
  triangulation.
- Tzathas, Gailleton, Steer et al. 2024, _Physically-based analytical erosion for fast terrain
  generation_, Computer Graphics Forum — closed-form in time but evaluated numerically over space,
  with receivers, a topological sort, an upstream pass and multigrid fixed-point iteration; Table 2
  gives 1.79 s at 512² and 8.18 s at 1024², in Python with numba on one Xeon E5-2650 v4. Read from the
  authors' PDF (verified 2026-09-23).
  <https://www-sop.inria.fr/reves/Basilic/2024/TGSC24/Analytical_Terrains_EG.pdf>. An earlier
  draft of this document called it a closed-form substitute that could be evaluated per point.
- Gaillard, Benes, Guérin, Galin, Rohmer and Cani 2019, _Dendry: A Procedural Model for Dendritic
  Patterns_, I3D (doi:10.1145/3306131.3317020), read from
  <https://perso.liris.cnrs.fr/eric.galin/Articles/2019-branching.pdf>: four levels, the first joined
  to the lowest Moore neighbour under the control function and the rest to the nearest segment of all
  coarser levels. Its reference code seeds each cell from integer coordinates but draws with
  `mt19937_64` and standard-library distributions, and rebuilds the network's neighbourhood for every
  point (`NoiseLib/include/noise.h`); its README gives about 10 s for a 512² grid on four cores.
  <https://github.com/mgaillard/Noise>
- Schott, Paris, Fournier, Guérin et al. 2023, _Large-scale terrain authoring through interactive erosion
  simulation_, ACM TOG; and 2024, _Terrain Amplification using Multi Scale Erosion_, which names the
  boundary problems tiled erosion runs into.
- Guérin et al. 2016, _Sparse representation of terrains for procedural modeling_; Guérin et al. 2017,
  _Interactive example-based terrain authoring with conditional generative adversarial networks_, ACM TOG;
  Grenier et al. 2024, _Real-time Terrain Enhancement with Controlled Procedural Patterns_, reporting
  amplification up to thirty-two times — the amplification line of work.
- Tucker and Whipple 2002, JGR; Harel, Mudd and Attal 2016, _Geomorphology_ — the stream-power law and its
  exponents. Flint's law and Hack's law, for channel slope and drainage area, are **cited from
  memory**, and Hack's constants (C = 1.5, h = 0.6) through Tzathas et al.
- Hoke and Hynek 2009, JGR 114, doi:10.1029/2008JE003247, and Luo, Cang and Howard 2017, Nature
  Communications 8, 15766, doi:10.1038/ncomms15766, for the age and volume of Mars's valley networks;
  Hoke, Hynek and Tucker 2011, EPSL, doi:10.1016/j.epsl.2011.09.053, for 10⁵ to 10⁷ years of active
  flow (**from memory**).
- Hypsometric statistics computed with pyshtools by a research agent, not in the repository, from
  Earth2014 (Hirt and Rexer 2015, International Journal of Applied Earth Observation and
  Geoinformation 39, 103), MarsTopo2600 and VenusTopo719 (Wieczorek 2015) and LOLA's lunar model.
- Lagae et al. 2009, on sparse convolution noise, and Worley 1996, on cellular noise — the
  per-cell, neighbourhood-searched construction the crater step uses. **Cited from memory.**
- Salmon, Moraes, Dror and Shaw 2011, _Parallel random numbers: as easy as 1, 2, 3_, SC11 —
  Threefry, the counter-based block function behind the sim's `Stream`; Zafar, Olano and Curtis 2010,
  _GPU random numbers via the tiny encryption algorithm_, HPG, is the GPU precedent for noise that
  depends on nothing but its inputs.
- Arteaga, Fuhrer and Hoefler 2014, _Designing bit-reproducible portable high-performance applications_,
  IPDPS; Sawaya, Bentley, Briggs et al. 2017, _FLiT_ — cross-platform floating-point reproducibility,
  including fused multiply-add and library functions as the principal hazards.
- Rust's documentation of the `algebraic_*` float methods ("may produce different results even
  within a single program run", `library/core/src/primitive_docs.rs`), and the unstable IEEE
  `minimum` and `maximum` (rust-lang/rust issue 91079).
- Flush-to-zero from shared objects: GCC 13's release notes, <https://gcc.gnu.org/gcc-13/changes.html>,
  and llvm-project pull request 80475 (Clang 19, issue 57589), no longer linking `crtfastmath.o` with
  `-shared`; llama.cpp and ggml at commit 48de2a1 setting no flush mode.

**Landing, rocks and craters**

- NASA TN D-6850, _Apollo Experience Report: Lunar Module Landing Gear Subsystem_, 1972: a footpad
  about 0.9 m across, 0.6 m of relief tolerated within the footprint, and the engine skirt's clearance
  of about 0.34 m. <https://ntrs.nasa.gov/citations/19720018253>
- Epp and Smith 2007, the ALHAT hazard-detection requirement of 0.3 m and 5°, NTRS 20060050771.
- Golombek and Rapp 1997, JGR 102(E2), 4117, the rock size–frequency law (**formula from memory**);
  Golombek et al. 2017, Space Science Reviews, doi:10.1007/s11214-016-0321-9, InSight's 0.45 m rock
  tolerance.
- Neukum, Ivanov and Hartmann 2001, _Cratering records in the inner solar system in relation to the lunar
  reference system_, Space Science Reviews; Michael and Neukum 2010, EPSL, for fitting the cumulative
  production function; Croft 1985, JGR, and Krüger, Hergarten and Kenkmann 2018, JGR Planets,
  _Deriving morphometric parameters and the simple-to-complex transition diameter_, for the
  simple-to-complex transition. Specific transition diameters were **not extracted** and must come
  from these papers directly, and the production function's coefficients from Neukum et al.'s Table 1.
- Atmospheric screening of small impactors, after Melosh 1989, _Impact Cratering_, and Venus's
  smallest craters of 1.5–2 km (Schaber et al. 1992; Herrick and Phillips 1994). **From memory.**

**Climate and biomes**

- Ramirez 2024, _A new 2D energy balance model for simulating the climates of rapidly and slowly rotating
  terrestrial planets_, Planetary Science Journal 5, 2, doi:10.3847/PSJ/ad0729, arXiv:2310.15992: six-hour
  steps, a 36 × 12 grid, some 2 to 20 s a run, prescribed humidity and no precipitation.
- Paradise et al. 2022, ExoPlaSim, MNRAS, doi:10.1093/mnras/stac172, the offline check.
- North, Cahalan and Coakley 1981, _Energy balance climate models_, Reviews of Geophysics; North and
  Kim 2017, _Energy Balance Climate Models_ — the latter cautioning that precipitation cannot be solved by
  simple models.
- Siler, Roe and Armour 2018, Journal of Climate 31, 7481, doi:10.1175/JCLI-D-18-0081.1; O'Gorman,
  Allan, Byrne and Previdi 2012, Surveys in Geophysics — precipitation from energy budgets.
- Roe 2005, _Orographic precipitation_, Annual Review of Earth and Planetary Sciences; Roe and Baker 2006 —
  rain shadows.
- Köppen–Geiger, as the climate classification.
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

**Atmosphere, clouds, ocean and rings**

- Bruneton and Neyret 2008, and Bruneton's 2017 revision with its reference implementation, whose
  defaults are a 256 × 64 transmittance table, a 32 × 128 × 32 × 8 scattering table, a 64 × 16
  irradiance table and four scattering orders, with density profiles of at most two layers; its WebGL
  demo loads tables precomputed offline. <https://ebruneton.github.io/precomputed_atmospheric_scattering/>
- Hillaire 2020, _A Scalable and Production Ready Sky and Atmosphere Rendering Technique_, EGSR: Table 2's
  0.17 ms for the four tables on a GTX 1080, and section 7's 0.31 ms in all at 1280 × 720, the 0.14 ms
  final pass included, with 0.5 ms for per-pixel ray marching from space; the per-planet tables under
  a millisecond on an iPhone 6s, and 250 ms for Bruneton's update on the same GTX 1080; section 6 and
  figures 11 and 12 for both models' behaviour at high optical depth (verified 2026-09-23).
  <https://sebh.github.io/publications/egsr2020.pdf>. An earlier draft of this document could read
  only the abstract.
- Stamnes, Tsay, Wiscombe and Jayaweera 1988, DISORT, Applied Optics 27, 2502 (**from memory**), and
  libRadtran's MYSTIC Monte Carlo, which is GPL and so a validation reference only, never linked.
- Nishita et al. 1993; O'Neil, _Accurate Atmospheric Scattering_, GPU Gems 2 chapter 16.
  <https://developer.nvidia.com/gpugems/gpugems2/part-ii-shading-lighting-and-shadows/chapter-16-accurate-atmospheric-scattering>
- Hosek and Wilkie 2012, _An analytic model for full spectral sky-dome radiance_, ACM TOG 31(4).
  **From memory.**
- Rayleigh scattering: Bucholtz 1995, Applied Optics 34, 2765, for the 550 nm cross-section of
  4.51 × 10⁻²⁷ cm² (verified through Crossref); and, **from memory**, Peck and Reeder 1972, JOSA 62,
  958, and Bates 1984, Planetary and Space Science 32, 785, for dry air, Peck and Khanna 1966 for N₂,
  Sneep and Ubachs 2005, JQSRT 92, 293, for CO₂ and CH₄, and Dalgarno and Williams 1962, ApJ 136, 690,
  for H₂. Tabulated refractive indices at <https://refractiveindex.info> (CC0).
- Scale heights, the Cornette–Shanks phase function and the tutorial Rayleigh coefficients, from
  Scratchapixel's _Simulating the Colors of the Sky_ and Zucconi's _Atmospheric Scattering_; the
  tutorials' coefficients, taken from Bruneton, are 15–20% above the formula's and are not used.
- That Mars's butterscotch sky and blue sunsets are dust effects, and Venus's Rayleigh and cloud
  optical depths of about 15 and 30: **from memory**, the latter checked only by the research agent's
  own estimate.
- Schneider and Vos 2015, _The Real-time Volumetric Cloudscapes of Horizon Zero Dawn_ (**PDF unreadable**);
  Guerrilla, _Nubis: Authoring Real-Time Volumetric Cloudscapes with the Decima Engine_, 2017, the source of
  the under-2 ms figure on PlayStation 4.
  <https://www.guerrilla-games.com/read/nubis-authoring-real-time-volumetric-cloudscapes-with-the-decima-engine>
- Bruneton, Neyret and Holzschuch 2010, _Real-time Realistic Ocean Lighting using Seamless Transitions
  from Geometry to BRDF_, Computer Graphics Forum, section 6 on planet-scale rendering, with its
  switch below 20 km. <https://inria.hal.science/inria-00443630>. Proland's ocean module, drawing flat
  or spherical oceans. <https://github.com/csbrandt/proland-4.0>. Scatterer's port for Kerbal Space
  Program. <https://github.com/LGhassen/Scatterer>. Outerra, _Ocean rendering_, 18 February 2011,
  drawing the ocean on its terrain patches. <https://outerra.blogspot.com/2011/02/ocean-rendering.html>.
  An earlier draft of this document said the published work was all flat-patch.
- Cox and Munk 1954, the distribution of sea-surface slopes from the sun's glitter, JOSA 44. **From
  memory.**
- Jeschke, Skřivan, Müller-Fischer, Chentanez, Macklin and Wojtan 2018, _Water Surface Wavelets_, ACM
  TOG 37(4). **Author list from memory.**
- Finch, _Effective Water Simulation from Physical Models_, GPU Gems chapter 1, for Gerstner waves and the
  steepness limit beyond which normals invert.
  <https://developer.nvidia.com/gpugems/gpugems/part-i-natural-effects/chapter-1-effective-water-simulation-physical-models>
- Tessendorf 2004, _Simulating Ocean Water_, SIGGRAPH course notes, for spectral ocean surfaces.
  **PDF unreadable**; cited bibliographically.
- Rings: Björn Jónsson's ring model and radial profiles, <https://bjj.mmedia.is/data/s_rings/>, which
  state no licence and are a private reference only; Saturn's occultation profiles at the PDS
  Ring-Moon Systems Node, <https://pds-rings.seti.org/>; Salo and French 2010, Icarus,
  doi:10.1016/j.icarus.2010.07.002 (arXiv:1007.0349), on shadowing between particles and the
  opposition surge; Salo and Karjalainen 2003, Icarus 164, 428, for the slab simulations; Lumme and
  Bowell 1981, AJ 86, 1694 and 1705; Déau et al. 2009 (arXiv:0902.0289), on the surge from 0.001° to
  25° phase; SpaceEngine's volumetric rings, 150 fps and more at 1080p on an RTX 2080 at 85%
  resolution with temporal rendering and 45 fps at 4K, <https://spaceengine.org/news/blog210611>;
  ring thicknesses and optical depths from Wikipedia's _Rings of Saturn_, a secondary source. Dones
  et al. 1993, on main-ring particles backscattering, Zebker et al. 1985, on the size distribution,
  Irvine 1966, JGR 71, 2931, Hapke 2008, Icarus 195, 918, Shu 1984, _Planetary Rings_ (IAU
  Colloquium 75), for density waves, and Durisen et al. 1992, Icarus 100, 364, and Estrada et al.
  2015, Icarus 252, 415, on ballistic transport, are **cited from memory**. An earlier draft of this
  document called the main rings forward-scattering.

**Exposure, tone mapping and colour**

- The exposure model — exposure value from average luminance, the reflected-light calibration constant of
  12.5 and the lens factor of 0.65, giving a white point near 9.6 times average luminance — and the GPU
  histogram. <https://bruop.github.io/exposure/>. It derives from Lagarde and de Rousiers 2014,
  _Moving Frostbite to Physically Based Rendering_, **cited from memory and not re-checked**.
- Filament, _Physically Based Rendering in Filament_, §5.2.6 "Pre-exposed lights".
  <https://google.github.io/filament/main/filament.html>. Unreal Engine's `View.PreExposure` over an
  fp16 scene colour: **from memory**, documentation link to be found.
- Tone mapping operators, and Blender's colour-management configuration as evidence of AgX's adoption and
  its roughly 25-stop range. <https://64.github.io/tonemapping/> and
  <https://github.com/blender/blender/blob/main/release/datafiles/colormanagement/config.ocio>. ACES,
  <https://www.oscars.org/science-technology/sci-tech-projects/aces>, and Khronos's PBR Neutral,
  <https://github.com/KhronosGroup/ToneMapping>, are **not re-checked**.
- Limb darkening, with the polynomial law and solar coefficients at 550 nm of 0.3, 0.93 and −0.23, giving a
  limb at 30% of centre and a disc average of about 80%. Via
  <https://en.wikipedia.org/wiki/Limb_darkening>, which cites Cox 2000, _Allen's Astrophysical Quantities_ —
  **the primary source was not read**. Other stars: Maxted 2018, A&A 616, A39, for the power-2 law, and
  Claret and Southworth 2022, A&A 664, A128, for its coefficients (T_eff and log g coverage to be
  confirmed on use).
- Walker, _Colour Rendering of Spectra_, for spectra to RGB through the CIE matching functions and
  gamut clamping. <https://www.fourmilab.ch/documents/specrend/>
- The illuminance of a magnitude-0 star, 2.54 × 10⁻⁶ lx, from Cox 2000, _Allen's Astrophysical
  Quantities_, §15, **not seen quoted verbatim**, and Crumey 2014's zero point; cross-checked by the
  Sun's V = −26.76 (Willmer 2018, ApJS 236, 47), which it turns into the measured 1.28 × 10⁵ lx above
  the atmosphere, and by Bessell, Castelli and Plez 1998, A&A 333, 231, to 1% for an A0 spectrum.
- Pickles 1998, PASP 110, 863 (VizieR J/PASP/110/863), the stellar spectral library, against the CIE
  1924 photopic luminosity function V(λ): V tracks photopic illuminance within 0.08 mag from O5 to M6.

**The star field**

- The Hipparcos Catalogue, ESA 1997, VizieR I/239: 8,874 stars to V 6.5, 1,608 brighter than V 5,
  15,544 to V 7.0 and 21,115 to V 7.3. The Yale Bright Star Catalogue, 5th revised edition, VizieR
  V/50: 8,404 to V 6.5.
- Crumey 2014, _Human contrast threshold and astronomical visibility_, MNRAS 442, 2600,
  arXiv:1405.4209: the point-source threshold of eq. 53, its linear form eq. 55 for 21 < μ < 25, the
  mesopic eq. 34, the colour term of eq. 18, and the background's colour correction in §1.3, with
  Blackwell 1946, JOSA 36, 624, for the coefficients. The widely quoted NELM ≈ 7.93 −
  5 log₁₀(10^(4.316 − μ/5) + 1) is Unihedron's SQM-to-NELM conversion for a B-band input, after
  Schaefer 1990 and Clark 1994, <http://unihedron.com/projects/darksky/NELM2BCalc.html>, and is not
  used.
- The background near the Sun: integrated starlight from Gaia DR3 (Gaia Collaboration, Vallenari et
  al. 2023, A&A 674, A1), with Riello et al. 2021, A&A 649, A3, for G to V; and zodiacal light from
  Leinert et al. 1998, _The 1997 reference of diffuse night sky brightness_, A&AS 127, 1 (**not
  opened; from memory**). The backgrounds away from the Sun are a research agent's hand model.
- Spectral libraries for star colour, **from memory**, coverage to be checked on use: PHOENIX (Husser
  et al. 2013, A&A 553, A6), ATLAS9 (Castelli and Kurucz 2003), TLUSTY's OSTAR2002 and BSTAR2006
  (Lanz and Hubeny 2003, ApJS 146, 417, and 2007, ApJS 169, 83), and the Montreal white-dwarf grids
  (Bédard et al. 2020, ApJ 901, 93).
- Baraffe et al. 2015, A&A 577, A42, pre-main-sequence tracks, for layers A and B. Humphreys and
  Davidson 1979, ApJ 232, 409 (**from memory**), and Davies, Crowther and Beasor 2018, MNRAS 478,
  3138, for the luminosity limit of cool supergiants. Old yellow post-AGB stars as standard candles
  near M_V −3.4, after Bond 1997 (**full citation to be found**).
- The Harris catalogue of Milky Way globular clusters, for 47 Tuc's structure. **From memory.**
- Chabrier 2003, PASP 115, 763, for the initial mass function behind the layers' shares.
- Nuclear-disc sky statistics from a research probe of the Milky Way fixture at generator version 15,
  seed `0x0311_1000_0000_0000`, 150 ly from Sgr A*, not in the repository.
