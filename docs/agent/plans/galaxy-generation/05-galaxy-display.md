# Plan 05: The `GALAXY` display

- **Milestone:** M1.
- **Depends on:** [04](04-server-and-protocol.md) (and, through it, 01–03).
- **Brainstorm sections covered:** [Visualiser](../../brainstorming/galaxy-generation.md#visualiser)
  and [The local chart in 3D](../../brainstorming/galaxy-generation.md#the-local-chart-in-3d) in
  full; the [Decisions](../../brainstorming/galaxy-generation.md#decisions) entries "The style guide
  gains what the chart needs", "The local chart is a rotatable 3D view", "Below the reference plane
  a symbol is open, and above it filled" and "A drive can jump to any system within range"; the
  named directions of [Coordinates](../../brainstorming/galaxy-generation.md#coordinates); the
  census rule of [The range query](../../brainstorming/galaxy-generation.md#the-range-query) as the
  chart shows it; the "By eye" entry of [Testing](../../brainstorming/galaxy-generation.md#testing).

## Goal

When this plan is done an operator can start the bridge client, move between the `LINK` and `GALAXY`
displays from the keyboard or the pointer, create a universe from a typed or random seed or open an
existing one, and inspect it. The `GALAXY` display shows the drawn parameters with units, the galaxy
map face-on and edge-on for all systems or the young population on a logarithmic single-hue ramp
with a legend, and a rotatable orthographic 3D chart of the systems around any point and time picked
from the two map views. The chart is the first user of a general spatial view, which the tactical
plot and the orbit map will reuse, and it is paired with a keyboard-operable system list and a
readout. `docs/frontend/ux-guidelines.md` has gained what the display needs. This completes the
first milestone: galaxy creation and exploration work end to end.

## Scope and non-goals

In scope:

- Interactive navigation between displays in `ConsoleFrame`, with single-key bindings.
- A way for any display to make requests over the client's one socket, built on plan 04's request
  layer, with closed-loop states (`PENDING`, accepted, rejected, timed out, no carrier).
- Universe list, open and create, against plan 04's messages.
- The `GALAXY` display: `UNIVERSE`, `PARAMETERS`, `GALAXY MAP` and `LOCAL CHART` panels.
- The general spatial view under `apps/hyperion/src/renderer/src/spatial/`: pure projection, camera,
  depth order, picking, scale, label placement and transition functions, a Canvas 2D painter, and
  the `SpatialView` component with its controls and orientation furniture.
- Edits to `docs/frontend/ux-guidelines.md`, and the drawn `☉` glyph.
- Tests for all of the above, under `.claude/rules/typescript-dev.md`.

Not in scope:

- Any Rust. Missing server behaviour is a finding against plan 04, not something to patch here.
- Stellar class, living or dead, luminosity (plan 06). M1 has one symbol shape, "system".
- Dust lanes on the map (plan 07). Motion of systems (plan 08): the chart already passes a time.
- Features, clusters and streams as chart marks (plans 09, 10). The mark model leaves room for them.
- A ship, a drive and commanded destinations. The spatial view supports a `--target` destination
  reticle and a drive range, and M1 feeds the drive range from an operator setting (see Design note
  D9). No command is sent to the server.
- The tactical plot and the orbit map themselves. Only their shared view is built.
- WebGL. Canvas 2D only, as the brainstorm leans.
- Any change to the Content Security Policy, the preload script or the main process. Nothing here
  needs one: the map's base64 is decoded in JavaScript by plan 04's decoder (no `data:` or `blob:`
  URL, no `fetch`), pictures reach the screen through `putImageData` and `drawImage` on canvases the
  renderer creates, glyphs and legends are inline SVG, and positions set through React's `style`
  prop go through the CSSOM. No task adds a dependency, a font or an origin.

## Provides

All paths are under `apps/hyperion/src/renderer/src/` unless they start with `docs/`.

### Navigation and frame

- `lib/displays.ts`: `DISPLAYS` (an `as const` array of `DisplayDefinition { id, title, key }`),
  `type DisplayId = "link" | "galaxy"`. Later plans add a display by adding an entry here and a case
  in `App`'s exhaustive `switch`.
- `components/ConsoleFrame.tsx`: props become `displays: ReadonlyArray<DisplayDefinition>`,
  `activeDisplay: DisplayId`, `onSelectDisplay: (id: DisplayId) => void`. The navigation bar renders
  one `button` per display with `aria-current="page"` on the active one.
- `lib/useDisplayKeys.ts`: `useDisplayKeys(displays, onSelect): void`, the function-key bindings.

### Requests and universes

- `lib/serverLink.ts`: `ServerLinkContext`, `useServerLink(): ServerLink`, where `ServerLink` is
  `{ readonly status: ConnectionStatus; readonly requests: RequestClient }`, and
  `linkDownReason(status): string | null` (`NO CARRIER`, `ESTABLISHING LINK`, `LINK INCOMPATIBLE`,
  or `null` when connected).
- `lib/useServerRequest.ts`: `useServerRequest` and `RequestState` (below).
- `lib/universe.ts`: `UniverseContext`, `useUniverse(): UniverseSession` (below).
- `lib/seed.ts`: `parseSeedHex`, `formatHex64`.
- `components/RequestStatus.tsx`: the shared presentation of a non-`ok` `RequestState`.

```ts
type RequestState<K extends RequestKind> =
  | { readonly kind: "idle" }
  | { readonly kind: "pending" }
  | { readonly kind: "ok"; readonly response: ResponseFor<K> }
  | { readonly kind: "rejected"; readonly code: string; readonly reason: string }
  | { readonly kind: "timed_out" }
  | { readonly kind: "link_down"; readonly reason: string };

function useServerRequest<K extends RequestKind>(
  body: RequestOf<K> | null,
  timeoutMs?: number,
): RequestState<K>;

interface UniverseSession {
  readonly open: UniverseInfo | null;
  readonly list: RequestState<"list_universes">;
  readonly command: RequestState<"create_universe"> | RequestState<"open_universe">;
  create(name: string, seed: SeedHex | null): void;
  openUniverse(universe: UniverseIdHex): void;
  refresh(): void;
}
```

### Galaxy data on the client

- `lib/galaxy/model.ts`: `ChartSystem`, `ChartCensus`, `ChartResult`, `CentreLy` (a readonly
  triple), `ROOT_CUBE_HALF_LY`, `CLOCK_WINDOW_YR`, `CHART_SYSTEM_LIMIT`.
- `lib/galaxy/wire.ts`: `toChartResult(response: SystemsInRange): ChartResult`,
  `toRangeRequest(universe, centreLy, radiusLy, timeYr, minLayer): RequestOf<"systems_in_range">`,
  `populationLabel(population)`, `layerIndex(layer)`.
- `lib/galaxy/mapGeometry.ts`: `MapGeometry`, `mapGeometry(map: DensityMap)`, `pixelToLy`,
  `lyToPixel`.
- `lib/galaxy/ramp.ts`: `parseHexColour`, `buildRamp`, `codeToLevel`, `rasterise`.
- `lib/format.ts`: `formatNumber`, `formatSigned`, `formatSci`, `formatLengthLy`,
  `formatScaleLength`, `formatBearingDeg`, `formatSignedDeg`, `formatAge`, `formatMassMsun`,
  `formatUniverseTimeYr`, `formatListPosition`, `TIME_SYSTEM_LABEL`.
- `lib/windowRange.ts`: `windowRange`, shared by every scrolling list.
- `components/SunGlyph.tsx`, `components/SolarMassUnit.tsx`, `components/UnitLabel.tsx` (an
  exhaustive rendering of plan 04's `Unit`).

### The general spatial view (`spatial/`)

- `vec3.ts`: `Vec3`, `add`, `sub`, `scale`, `dot`, `cross`, `norm`, `normalise`.
- `frame.ts`: `LocalFrame { coreward, spinward, north }`, `localFrameAt(positionLy)`,
  `toLocal(frame, vector)`, `cylindrical(positionLy)`.
- `camera.ts`: `Camera { azimuthDeg, elevationDeg, pxPerUnit }`, `ViewBasis { right, up, forward }`,
  `viewBasis`, `project`, `wrapAzimuthDeg`, `clampElevationDeg`, `rotateCamera`, `zoomCamera`,
  `fitPxPerUnit`, `PRESETS` (`top`, `side`, `front`, `oblique`), `matchesPreset`.
- `scale.ts`: `floor125`, `ceil125`, `scaleBar`, `gridSpacing`, `RADIUS_STEPS_LY`.
- `marks.ts`: `SpatialScene`, `PointMark`, `SphereMark`, `PlaneSpec`, `SymbolShape`, `SizeClass`,
  `MarkStatus`.
- `symbols.ts`: `symbolOutline(shape)`, `SIZE_CLASS_REM`.
- `plane.ts`: `gridLines`, `ringPolyline`.
- `drawList.ts`: `buildDrawList(scene, camera, viewport): DrawList`, `DrawOp` (discriminated union),
  `Anchor`.
- `pick.ts`: `pick(anchors, pointPx, tolerancePx): string | null`.
- `labels.ts`: `chooseLabels`, `placeLabels`.
- `transition.ts`: `easeOut`, `tweenCamera`, `TRANSITION_MS`.
- `redraw.ts`: `createRedrawScheduler(requestFrame, cancelFrame)`.
- `paint.ts`: `paint(context, drawList, tokens)`, `readTokens(element): ColourTokens`.
- `useThrottledValue.ts`: `useThrottledValue(value, intervalMs)`.
- `SpatialView.tsx`, `AxisTriad.tsx`, `CoreArrow.tsx`, `ScaleBar.tsx`, `PresetButtons.tsx`.

### Displays and panels (`displays/galaxy/`)

`GalaxyDisplay.tsx`, `UniversePanel.tsx`, `ParametersPanel.tsx`, `GalaxyMapPanel.tsx`,
`GalaxyMapView.tsx`, `DensityLegend.tsx`, `CentreEntry.tsx`, `LocalChartPanel.tsx`,
`ChartControls.tsx`, `CensusReadout.tsx`, `SystemList.tsx`, `SystemReadout.tsx`, `SymbolLegend.tsx`,
`chartModel.ts`, `useRangeQuery.ts`, `parameterLabels.ts`, `parameterReadings.ts`.

### Test helpers (`test/`)

- `FakeWebSocket.ts` (which plan 04 gives `serverResponds(id, body)` and `serverRejects(id, error)`)
  gains `requestsOfKind(kind)` and `serverAnswers(kind, build)`.
- `galaxyFixtures.ts`: builders typed against the generated bindings (`aUniverse`, `aUniverseList`,
  `someGalaxyParameters`, `aDensityMap`, `aSystemsInRange`, `aSystemRecord`, `aCensus`).
- `RecordingContext2D.ts`, `stubCanvas()`, `FakeResizeObserver.ts`, `stubMatchMedia(reduced)`.

### Documentation

`docs/frontend/ux-guidelines.md` gains what the brainstorm's Decisions entry lists: raster density
fields with a logarithmic ramp and a legend; the galactic direction names and the `GALACTIC` frame;
the units `M☉`, `Myr` and `Gyr`; the drawn `☉`, with `⊕` reserved. It also gains four things beyond
that entry, each of which **needs the owner's confirmation** (see P05.T2 and Risks): the unit `yr`,
E notation, the `UT` time system, and the 3D spatial display conventions with a short nomenclature
list.

## Consumes

From plan 04 (server, universes and protocol), whose "Provides" is authoritative:

- **Wire types** from `@hyperion/protocol`: `RequestBody` and `ResponseBody` with kinds
  `create_universe`, `list_universes`, `open_universe`, `galaxy_parameters`, `density_map` and
  `systems_in_range`; `CreateUniverseRequest { name, seed: SeedHex | null }`, `UniverseInfo`,
  `UniverseList`, `UniverseStatus`; `GalaxyParameters`, `ParameterGroup`, `Parameter`,
  `ParameterOrigin`, `ParameterValue`, `Unit`; `MapView`, `MapPopulation`, `DensityMapRequest`,
  `DensityMap`; `MassLayer`, `Population`, `SystemsInRangeRequest`, `SystemsInRange`, `Census`,
  `LayerCensus`, `LayerStatus`, `SystemRecord`; `GalacticPosition`, `UniverseTime`, the three hex
  newtypes; `RequestError` and `ErrorCode`.
- **Runtime** from `@hyperion/protocol`: `RequestClient`, `RequestChannel`, `PendingRequest`,
  `RequestOutcome`, `RequestFailure`, `RequestKind`, `RequestOf`, `ResponseFor`; `isHex64`;
  `galacticPositionFromLy`, `galacticDeltaLy`; `universeTimeFromYears`, `universeTimeToYears`;
  `decodeDensityMap` and `DecodedDensityMap` (`codes`, `maxCode`, `log10PerLy2`).
- **Client wiring** (P04.T5): `useServerConnection` returning `ServerConnection`, which carries the
  `RequestClient` as `requests`; `ConnectionStatus` with `"incompatible"`;
  `FakeWebSocket.serverResponds` and `serverRejects`.
- **Wire conventions** this plan relies on, from plan 04's design notes: requests are stateless and
  each names its universe (note 6), so nothing is re-opened after a reconnect; every `u64` is 16
  lower-case hex digits (8); positions are cell plus offset, and records carry the position and
  `age_myr` at the query's time, unsorted (10); parameters are a grouped list of key, value and unit
  with labels left to the client, and every `Unit` is one the guide allows or gains here, so the
  client formats and never converts (11); the map's row order, byte order, orientation, code 0, the
  floor at 5 dex (face-on) or 7 dex (edge-on) under the ceiling, the fixed extents and the
  resolutions 128–1,024 (12); the census lists all five layers with band, expected and returned
  counts and a status, and `complete_above_msun` is `null` when nothing fits (13); the limits of
  note 24 (census limit at most 20,000, radius at most 131,072 ly, time within ±H, the cell budget
  and `over_cell_budget`).

From plan 01, indirectly: the definitions of coreward, spinward and north, the root cube's bounds
(±65,536 ly) and the clock window H = 1,000 years, which the client restates as constants in
`lib/galaxy/model.ts` with a comment naming their owner.

## Design notes

**D1. Inactive displays stay mounted under React's `Activity`.** Each display is wrapped in
`<Activity mode={active ? "visible" : "hidden"}>`. It keeps a display's state (the chart centre, the
camera, the selection) across a visit to `LINK`, tears its effects down while hidden, and hides it
from the accessibility tree. The guide's "displays switch instantly" is met because nothing
animates. `react` is already at 19.3, which has `Activity`. Lifting every display's state into `App`
would break the rule that state stays local.

**D2. Displays switch on function keys.** `F1` is `LINK`, `F2` is `GALAXY`, shown on the tab as the
guide requires of single-key bindings. Letters and digits would fire while the operator types a
seed. Function keys do not, so they work from any focus. The listener is on `document`, ignores
events with modifiers, and calls `preventDefault`.

**D3. Display-local single keys.** `T`, `S`, `F`, `O` (presets), `Z` (fit), `+`, `=`, `−`, `-`
(zoom) and `C` (centre chart) act only while `GALAXY` is visible, and are ignored when the event
target is an `input`, `select` or `textarea`, or when a modifier is held. Arrow keys rotate only
while the chart canvas has focus, and move the map cursor only while a map has focus, as the
brainstorm requires.

**D4. Requests are closed-loop and never look like a web app.** A request in flight shows `PENDING`
as text. There is no spinner and no "Loading". A failure shows `REJECTED: <reason>`, `TIMED OUT`, or
the link's state (`NO CARRIER`, `LINK INCOMPATIBLE`). Each `useServerRequest` owns one
`RequestChannel`, so a newer request for the same panel supersedes the older one on the client and
cancels it on the server. `aborted` and `superseded` outcomes are dropped silently. Plan 04's client
has no timers, and the guide requires a timed-out state, so the hook cancels after 30 s (120 s for a
density map, which is "seconds, not milliseconds" and queues behind others) and shows `TIMED OUT`. A
read that failed because the link was down is sent again when the link returns. A command (create,
open) is not. While the link is down, controls that need the server are disabled and give the reason
on focus. Data already shown stays: it is a snapshot labelled with its universe and chart time, not
a live value, so it is not marked stale.

**D5. The map request and its decoding.** The display asks for `resolution: 512` and `bits: 8`. The
ramp has 256 levels, so 16 bits would buy nothing on screen, and 8 bits over 5 or 7 dex is 0.02–0.03
dex a step, finer than the eye or the cursor readout needs. A face-on map is then about 350 kB of
base64. Decoding is plan 04's `decodeDensityMap`; row order, orientation and the meaning of code 0
are its design note 12 and are restated on screen (view titles, axis labels, legend floor). A pixel
is 256 ly, far coarser than a chart, which is why the centre can also be typed (T8.g).

**D6. The ramp is linear in sRGB between the two tokens, in 256 steps.** The colours are read from
the computed `--surface-0` and `--text` custom properties, never written as literals. The picture is
drawn with `imageSmoothingEnabled = false`: a pixel is what the server computed.

**D7. Legend ticks and large parameters use E notation.** B612 has `¹ ² ³` but no other superscript
digits and no `⁻` (checked against the bundled `@fontsource` files, as were `θ`, `≥` and `≤`, all
absent; `²`, `³`, `±`, `−` and `×` are present, by the `cmap` tables of the bundled `latin` subset
files). So `10⁻⁴` cannot be set. Ticks read `1E-4`, and a stellar mass reads `5.20E10 M☉`. The guide
gains the rule. The map's unit is written `SYSTEMS/ly²`.

**D8. Wire shapes stop at an adapter, and hex is shown in upper case.** `lib/galaxy/wire.ts` turns a
`SystemsInRange` into client types with units in their names. Offsets from the chart centre come
from `galacticDeltaLy`, which subtracts cells before offsets, so a chart in hundredths of a
light-year keeps its precision 60,000 ly from the origin. Seeds and IDs are lower case on the wire
(plan 04, note 8) and upper case on screen, the guide's annunciator convention; `formatHex64` and
`parseSeedHex` are the only places that change case. A seed may be typed in either case with 1 to 16
digits and is left-padded.

**D8a. Parameter labels are a client glossary with a visible fallback.** Plan 04 sends dotted keys.
`parameterLabels.ts` maps each known key and group key to its label. An unknown key is shown as the
key itself in upper case, so a parameter added by a later plan appears at once and looks unfinished
until it is named. No unit is converted on the client: plan 04 sends only units the guide allows
(its note 11), the pattern speed already in `°/Myr`, and nothing per kiloparsec.

**D9. The drive range is an operator setting in M1.** No ship or drive exists yet, but the chart's
range circle, plane ring and reachability colouring must be built and tested now. The `LOCAL CHART`
panel has a `DRIVE RANGE` field, labelled `SET` to show it is entered and not measured, defaulting
to 50 ly. The query radius defaults to it, and then everything shown is reachable. When a ship
exists the field is replaced by the ship's value. The guide's rule on honest data forbids showing a
number the simulation does not have, so the value is never presented as a reading: it lives in an
editable field and nowhere in an `output`; every place that repeats it says `SET` (the range
circle's label is `RANGE 50 ly SET`, the legend's colour key `ACCENT WITHIN SET RANGE`); and the
list and readout say `IN RANGE`, which is a statement of geometry about the chart centre, never
`REACHABLE`, which would claim a ship and a drive.

**D10. Azimuth is the bearing of the direction of view.** `000°` looks coreward, `090°` spinward,
`180°` rimward, `270°` antispinward. Elevation is the camera's angle above the reference plane:
`+90°` looks south onto the plane. Presets: `TOP` (000°, +90°), `SIDE` (000°, 0°), `FRONT` (090°,
0°), `OBLIQUE` (030°, +30°), the default. With `c`, `s`, `n` the local coreward, spinward and north
unit vectors, `h = cos α · c + sin α · s`:

```text
forward = cos ε · h − sin ε · n        (from the viewer into the screen)
right   = cos α · s − sin α · c        (h × n; never degenerate, so there is no roll)
up      = cos ε · n + sin ε · h        (right × forward)
x_px = centre_x + k · (d · right)      y_px = centre_y − k · (d · up)      depth = d · forward
```

At `TOP` this puts coreward up and spinward right. At −90° it puts rimward up, which is the mirrored
view from the south that the brainstorm asks for. `(spinward, coreward, north)` is the right-handed
triple.

**D11. The local frame.** At a centre (x, y, z) with R = √(x² + y²): coreward = (−x, −y, 0) ÷ R,
spinward = (−y, x, 0) ÷ R, north = (0, 0, 1). Within 10⁻⁶ ly of the axis the named directions are
undefined, so the frame falls back to coreward = −x̂, spinward = +ŷ, and the display says
`DIRECTIONS UNDEFINED AT AXIS: GRID ALIGNED TO −X`.

**D12. Drawing order.** With the camera above the plane (ε ≥ 0): marks below the plane, far to near;
then the plane's grid and rings; then marks above the plane, far to near. With ε < 0 the two halves
swap. A mark's stalk is drawn immediately before its symbol, so it lies wholly in its own half. Ties
in depth are broken by ID so that the order is deterministic. The range circle, the query edge and
reticles are screen annotations and are drawn last.

**D13. Line styles.** Dashes are reserved for predicted paths and `--text-muted` for stale values,
so neither is used. The range circle is a solid `--text` line 1.5 px wide. The query edge is a solid
`--text` hairline with short outward ticks every 10°, the look of a chart boundary. The plane ring
at the drive range and the grid rings are `--line`. Each curve has a DOM label (`RANGE 50 ly SET`,
`QUERY EDGE 80 ly`, `PLANE 50 ly`); the labels are the scene's, so the view itself stays general.
When query radius and drive range are equal one circle is drawn, with ticks, and both labels.

**D14. Symbols.** M1 has one type, "system", drawn as a circle. `symbols.ts` also defines diamond,
square and triangle for later types, so the tactical plot does not have to touch the painter.
Diameters by mass layer, in `rem` so that they follow the interface scale: A 0.5, B 0.625, C 0.75, D
0.875, E 1.0, with a 1.5 px outline drawn inside the diameter. The smallest open circle then has a 5
px hole at 100% and 3.4 px at 80%, which still reads as open. The legend says
`SYMBOLS NOT TO SCALE`.

**D15. No text on the canvas.** Every label, number and legend is DOM or inline SVG, which keeps
B612, the `☉` glyph and interface scaling. The painter has no `fillText`.

**D16. The camera is React state of `SpatialView`, updated at most once a frame.** Input handlers
add deltas to a ref and ask the scheduler for a frame. The frame callback commits one `setCamera`. A
layout effect paints. The system list is a sibling, not a child, so a drag re-renders only the
canvas, a handful of labels and the triad. This is simpler than painting outside React and is well
inside budget for a few thousand marks.

**D17. The list is always windowed.** One code path instead of a switch at two thousand rows. Rows
are `2rem` tall, which is also the guide's target size. It is an ARIA `listbox` with
`aria-activedescendant`, and options carry `aria-posinset` and `aria-setsize` because most rows are
not in the DOM. A native control cannot be windowed.

**D18. The minimum-mass selector is a radio group, the radius selector a `select`.** An `option`
cannot hold the SVG `☉`, a radio's `label` can.

**D19. The map cursor and the chart centre are two things.** Clicking a map, or moving its cursor
with the arrow keys, moves a cursor. `CENTRE CHART` (`C`) makes the cursor the chart centre and
issues the range query. The edge-on view sets only z, as the brainstorm says, although a click there
also has an x. Otherwise each arrow-key press would start a query.

**D20. Chart limits.** The range query is sent with a limit of 4,000 systems, the top of the
brainstorm's "a few thousand". Query radius steps are 1-2-5 from 0.01 ly to 500 ly. The chart time
is entered in years from the epoch within ±1,000 (the clock window), shown as `UT +12.50 yr`. The
label comes from one constant, `TIME_SYSTEM_LABEL` in `lib/format.ts`, because the owner has yet to
confirm it (T2.b).

**D21. Distances keep one precision per chart.** Two decimals when the query radius is 10 ly or
more, three from 1 ly, four below. Ages read in `Myr` below 1,000 Myr and `Gyr` from there. The
values are static, so no hysteresis is needed. The scale bar reads in `ly` down to 0.01 ly and in
`AU` below that, following the guide's unit ladder.

## Tasks

Order and parallelism: the whole plan starts after plan 04, except T2, T3, T4 and T9, which need
nothing from it and can start at once. T2 (guide edits) touches only the guide and can run at any
time, but T3.b cites T2.d. T3's three subtasks are independent of each other. T1 needs
`spatial/vec3.ts` from T9.a for a type. Three tracks then run in parallel: **A** T1 → T5 → T6 → T7;
**B** T8.a–c (pure map code, needs only T3.a); **C** T9 (pure spatial code, needs only T3.a for
T9.c) → T10. T8.d–g need T5 and T6.d. T11 needs T1, T5, T6.d and T10. T12 is last. Within T9 every
subtask except T9.f (needs a–e) is independent. Every task ends with `pnpm typecheck`, `pnpm lint`,
`pnpm test` and `just ci` green. Every exported item gets TSDoc, and every component test queries by
role and name, uses `userEvent.setup()`, and takes no snapshot.

### P05.T1 Client model, wire adapter and fixtures

**P05.T1.a Model and adapter.** Write `lib/galaxy/model.ts` and `lib/galaxy/wire.ts` as listed under
Provides.

```ts
interface ChartSystem {
  readonly id: SystemIdHex; // as on the wire; upper-cased only by formatHex64
  readonly designation: string;
  readonly relLy: Vec3; // from the chart centre, galactic axes, at the chart time
  readonly distanceLy: number;
  readonly positionLy: Vec3; // galactic, for the readout's RADIUS, ANGLE, Z
  readonly layer: MassLayer;
  readonly population: Population;
  readonly initialMassMsun: number;
  readonly ageMyr: number;
}
type ChartCensus =
  { readonly kind: "complete"; readonly aboveMsun: number } | { readonly kind: "nothing_fits" };
interface ChartResult {
  readonly centreLy: CentreLy;
  readonly radiusLy: number;
  readonly timeYr: number;
  readonly census: ChartCensus;
  readonly layers: ReadonlyArray<LayerCensus>; // all five, for the legend and the census table
  readonly systems: ReadonlyArray<ChartSystem>; // sorted by distance, then ID
}
```

`toChartResult` takes centre, radius and time from the response's echo, not from the request, so
what is drawn is what was answered. `populationLabel` and `layerIndex` are exhaustive `switch`es
with no `default`. `Vec3` is imported as a type from `spatial/vec3.ts`; nothing else crosses that
boundary.

- Files: `lib/galaxy/model.ts`, `lib/galaxy/wire.ts`, `lib/galaxy/wire.test.ts`.
- Tests: distance and sort order, ties by ID; a centre at (60,000, 0, 0) ly with a system 0.01 ly
  away keeps `distanceLy` within 10⁻⁹ ly; `complete_above_msun: null` gives `nothing_fits`;
  `toRangeRequest` floors negative coordinates into cell −1 (through `galacticPositionFromLy`) and
  carries `CHART_SYSTEM_LIMIT`; every `Population` has a label.
- Acceptance: `pnpm --filter hyperion exec vitest run src/renderer/src/lib/galaxy/wire.test.ts`
  passes. `grep -rn "_msun\|radius_ly\|age_myr" apps/hyperion/src/renderer/src` finds matches only
  in `lib/galaxy/wire.ts` and `test/`.

**P05.T1.b Fakes and fixtures.** On top of plan 04's `serverResponds` and `serverRejects`, add to
`FakeWebSocket`: `requestsOfKind(kind)` (the sent `request` messages whose body has that kind, with
their IDs) and `serverAnswers(kind, build)` (finds the latest unanswered request of `kind` and
responds to its ID with `build(body)`; throws an error naming the kind when there is none). Add
`test/galaxyFixtures.ts` with builders typed against the generated bindings, so that a protocol
change breaks the fixtures at compile time. `aDensityMap` builds a small 8-bit map (8 × 4) from an
array of codes. `aSystemsInRange` builds records from `{ relLy, layer }` lists about a centre and
fills a consistent census.

- Files: `test/FakeWebSocket.ts`, `test/galaxyFixtures.ts`, `test/galaxyFixtures.test.ts`.
- Tests: `serverAnswers` uses the request's ID and skips answered requests; the named error; a
  fixture map decodes with plan 04's `decodeDensityMap`; existing tests still pass.
- Acceptance: `pnpm test` green.

### P05.T2 UX guide edits

Docs only. Each subtask is one section edit. Acceptance for each: `pnpm format:check` passes, and
the quoted strings are found by `grep` in `docs/frontend/ux-guidelines.md`. T2.a, T2.d, the
direction names of T2.b and `M☉`, `Myr` and `Gyr` of T2.c are what the brainstorm's Decisions entry
settled. The rest is marked **needs the owner's confirmation** where it appears: put those edits in
a commit of their own whose message lists them, so that the owner can accept or revert them without
touching the settled ones. The code does not wait for the answer, since each is confined to one
constant, formatter or component.

**P05.T2.a Raster density fields.** In "Graphs, schematics and spatial displays": a continuous field
(column density, later dust) may be drawn as a raster. Rules: a single hue from `--surface-0` to
`--text`; the ramp is logarithmic when the data span more than two orders of magnitude, and the
legend says which; the floor is stated and values at or below it are exactly `--surface-0`; a legend
with tick values and the unit is mandatory; no smoothing that invents values. In "Colour", reword
the gradient ban to say that it covers console chrome and that a data ramp is not a gradient in that
sense. Grep: `raster`, `logarithmic`.

**P05.T2.b Galactic directions, frame and time system.** In "Voice and nomenclature": the names
`COREWARD`/`RIMWARD`, `SPINWARD`/`ANTISPINWARD`, `NORTH`/`SOUTH`, their definitions (towards the
galactic axis; the direction of rotation; +z, from which rotation is counter-clockwise), that they
are local and undefined on the axis, the frame name `GALACTIC`, and its coordinates as `RADIUS`,
`ANGLE` (from +x, counter-clockwise from the north) and `HEIGHT`. **Needs the owner's
confirmation:** in "Numbers, units and time", the time system `UT`, universe time from the
generator's epoch, shown as signed years (`UT +12.50 yr`). M1 needs a label, because the guide
forbids a time without its time system and every chart carries a time. The owner should weigh that
`UT` also abbreviates Universal Time and that the header strip shows `UTC`; if another label is
preferred, only `TIME_SYSTEM_LABEL` and this sentence change. **Needs the owner's confirmation:**
the guide demands "a single ship-wide nomenclature list" and holds none, so start one as a short
table at the end of "Voice and nomenclature" with the display names `LINK` and `GALAXY` and the
abbreviations this display introduces: `AZM`, `ELV`, `DESIG`, `DIST`, `INIT MASS`, `GEN VER`, `EXP`,
`RET`, `UT`. Grep: `COREWARD`, `GALACTIC`, `AZM`.

**P05.T2.c Units and E notation.** In "Numbers, units and time": beside SI, `M☉` for stellar and
system masses, and `Myr` and `Gyr` for ages. **Needs the owner's confirmation:** `yr` for universe
time, which M1 needs because a chart time within ±1,000 years is an eleven-digit number of seconds
and a rounding error in `Myr`; and E notation with three significant figures for values outside a
unit's ladder (`5.20E10 M☉`, `1E-4` on a legend tick), which M1 needs because the stellar mass, the
system count and the legend's decades cannot be written otherwise: B612 has no superscripts beyond
`¹ ² ³`, and the guide forbids long digit strings. Rates compose allowed units (`°/Myr`, `/ly³`,
`SYSTEMS/ly²`). `dex` is not added: M1 displays no logarithmic ratio (plan 04 sends no metallicity),
and the plan that first shows one adds it. Grep: `Myr`, `E notation`.

**P05.T2.d Drawn glyphs.** In "Typography": `☉` (U+2609) is not in B612 or B612 Mono. It is drawn as
an inline SVG sized to the text, a circle with a centre dot, in `currentColor`, with the accessible
name "solar masses" on the unit as a whole. `⊕` (U+2295) is reserved to be drawn the same way when
planets need it. Neither character may be typed into a string that reaches the screen.

**P05.T2.e 3D spatial displays. Needs the owner's confirmation** as a whole: the Decisions settle
the chart's design (orthographic, plane and stalks, no roll, presets, filled above and open below),
and this subtask is what turns it into a ship-wide convention, which the brainstorm asks for when it
says fill "can mean nothing else on this kind of display" and that the tactical plot and orbit map
share the view. In "Graphs, schematics and spatial displays": orthographic projection only; orbit
camera with azimuth and elevation and no roll; presets `TOP`, `SIDE`, `FRONT` and an oblique
default; a reference plane with grid and rings, and a stalk from each mark to the plane; filled
above the plane, open below, and fill means nothing else on a spatial display; size may encode a
class but never depth, with a `NOT TO SCALE` legend; no dimming by depth; `--accent` marks what is
available (reachable), a bracket reticle marks the selection, and the reticle in `--target` marks a
commanded destination; a range sphere's outline is a circle at 6:1 and is labelled apart from the
ring on the plane; the edge of fetched data is drawn; the view shows a triad, azimuth and elevation,
a 1-2-5 scale bar, the frame name, the centre and the time; redraw on demand only; a canvas is
paired with a DOM list for keyboard selection. Grep: `orthographic`, `NOT TO SCALE`. The function
keys of D2 need no guide edit: the guide already asks for single-key bindings shown on the control,
and a ship-wide rule for display keys can wait for a second station.

- Files (all of T2): `docs/frontend/ux-guidelines.md`.

### P05.T3 Formatting and the `☉` glyph

**P05.T3.a `lib/format.ts`.** Pure functions, each returning a string without its unit unless named
for one. `formatNumber(value, decimals)` groups from five digits (`useGrouping: "min2"`, as
`ConnectionPanel` does; move its `WHOLE_NUMBER` use onto this). Negative values use `-`, and
`formatSigned` always shows the sign. `formatSci(value)` gives three significant figures as
`5.20E10` and exact powers of ten as `1E-4` when asked for a tick. `formatLengthLy(ly, decimals)`.
`formatScaleLength(ly)` gives `20 ly`, `0.05 ly`, `500 AU` (1 ly = 63,241.077 AU; cite IAU 2012 for
the au and the Julian year for the ly). `formatBearingDeg` gives `000°`–`359°`, rounding 359.6 to
`000°`. `formatSignedDeg` gives `+30°`, `-05°`. `formatAge(ageMyr)` gives `{ value, unit }` per D21.
`formatMassMsun` gives two decimals below 10 and one above. `formatUniverseTimeYr` gives the signed
years to two decimals (`+12.50`), and `TIME_SYSTEM_LABEL = "UT"` is the one place the time system's
name is written (D20). `formatListPosition(first, last, total)` gives `12-24 of 1,612`. Missing
values are the caller's em dash, not a formatter's.

- Files: `lib/format.ts`, `lib/format.test.ts`, `components/ConnectionPanel.tsx`.
- Tests: table-driven with `it.each` for every function, including 9,999 and 10,000, −0.004 (no
  `-0.00`), 359.6°, 999.9 and 1,000 Myr.
- Acceptance: the vitest file passes; `ConnectionPanel.test.tsx` unchanged and green.

**P05.T3.b `SunGlyph` and `SolarMassUnit`.** `SunGlyph` is an `aria-hidden` inline `svg`,
`viewBox="0 0 10 10"`, a circle r = 4 with `stroke="currentColor"`, `fill="none"`, stroke width 1,
and a centre dot r = 1 filled with `currentColor`; sized `0.7em`, lowered as a subscript in CSS.
`SolarMassUnit` renders the markup below. No colour literal and no `☉` character appear in either.

```tsx
<span className="unit" role="img" aria-label="solar masses">
  <span aria-hidden="true">M</span>
  <SunGlyph />
</span>
```

- Files: `components/SunGlyph.tsx`, `components/SolarMassUnit.tsx`,
  `components/SolarMassUnit.test.tsx`, `styles.css`.
- Tests: `getByRole("img", { name: "solar masses" })` is present; the document text does not contain
  U+2609.
- Acceptance: test passes;
  `grep -rn "☉" apps/hyperion/src --include=*.tsx --include=*.ts --include=*.css` finds comments
  only.

**P05.T3.c `lib/windowRange.ts`.** `windowRange`, pure, with 0-based inclusive indexes and `null`
members when `total` is 0:

```ts
function windowRange(
  scrollTopPx: number,
  rowHeightPx: number,
  viewportHeightPx: number,
  total: number,
  overscan: number,
): { first: number; last: number; firstVisible: number; lastVisible: number } | null;
```

`first` and `last` include the overscan and bound what is rendered; the visible pair feeds
`formatListPosition`. Used by the universe table, the parameters list and the system list.

- Files: `lib/windowRange.ts`, `lib/windowRange.test.ts`.
- Tests: tables for the top, the middle, the end, fewer rows than the window, zero rows, and a
  scroll offset past the end (clamped).
- Acceptance: the test file passes.

### P05.T4 Display navigation

**P05.T4.a Interactive navigation bar.** Add `lib/displays.ts`. `App` holds
`const [activeDisplay, setActiveDisplay] = useState<DisplayId>("link")` and renders each display
inside `Activity` (D1), choosing the component with an exhaustive `switch` on `id`. `GALAXY` renders
a placeholder panel titled `GALAXY` until T6.d. `ConsoleFrame` renders the navigation bar as
`<nav aria-label="Displays"><ul><li><button type="button" aria-current=…>` with the key hint and the
title inside the button. The header `h1` shows the active display's title. CSS: move the tab styles
from `li` to `button`; reset the button's chrome; `min-height: 2rem`; hover uses `--surface-2`;
focus is the guide's `2px` `--accent` outline through `:focus-visible`; the active tab keeps the
accent top rule, which is a shape as well as a colour, and is also marked by `aria-current`, so
colour is not the only signal. Tabs are display controls, not ship commands, and look like it: no
`--line-strong` outline.

- Files: `lib/displays.ts`, `App.tsx`, `components/ConsoleFrame.tsx`, `styles.css`, `App.test.tsx`,
  `components/ConsoleFrame.test.tsx`.
- Tests: the navigation has buttons `F1 Link` and `F2 Galaxy` in that order; clicking `Galaxy` makes
  the level-1 heading `Galaxy` and moves `aria-current="page"`; tabbing reaches the buttons and
  `Enter` activates; after `Galaxy` then `Link`, the link panel still shows the server version
  (state kept and link not re-opened: `FakeWebSocket.instances` has length 1); the hidden display's
  headings are not found by `queryByRole`.
- Acceptance: tests pass; update the existing "opens on the link display" test to query the button
  by role.

**P05.T4.b Function-key bindings.** `useDisplayKeys` (D2): one `keydown` listener on `document`,
removed on cleanup; maps `event.key` to a display through `DISPLAYS`; ignores repeats and modifiers.

- Files: `lib/useDisplayKeys.ts`, `lib/useDisplayKeys.test.tsx`, `App.tsx`.
- Tests: `user.keyboard("{F2}")` shows `Galaxy` even while a text field has focus; `{Control>}{F2}`
  does nothing; after unmount the listener is gone (spy on `removeEventListener`).
- Acceptance: tests pass.

### P05.T5 Requests from displays

**P05.T5.a The server link context.** P04.T5 has already put a `RequestClient` on
`useServerConnection`. This task makes it reachable: `lib/serverLink.ts` with `ServerLinkContext`,
`useServerLink()` (throws an error naming the missing provider) and `linkDownReason(status)`, an
exhaustive `switch` sharing its words with `LinkStatus`. `App` provides `{ status, requests }`,
memoised so that a latency update does not re-render every consumer.

- Files: `lib/serverLink.ts`, `lib/serverLink.test.tsx`, `App.tsx`, `components/LinkStatus.tsx`
  (export the label table).
- Tests: a consumer under `App` makes a request that reaches the `FakeWebSocket` and receives the
  response; a pong does not re-render a consumer (render counter); `linkDownReason` for each status.
- Acceptance: tests pass; `grep -rn "JSON.parse\|JSON.stringify" apps/hyperion/src/renderer/src`
  finds only `test/`.

**P05.T5.b `useServerRequest` and `RequestStatus`.** The hook owns one `RequestChannel` for its
lifetime. When `body` changes by value (callers pass a memoised body or `null`) and the link is
connected, it requests through the channel, sets `pending`, and maps the outcome per D4: a
`response` to `ok`; a `RequestError` to `rejected` with its code and message; `link_lost` to
`link_down`; `protocol_violation` to `rejected`; `aborted` and `superseded` to nothing. It checks an
`ignore` flag before setting state and cancels on clean-up. The timeout is a `setTimeout` that calls
`cancel()` and sets `timed_out`. With the link down it reports `link_down` without sending, and
sends when the link returns. `RequestStatus` renders `PENDING`, `REJECTED: <reason>`, `TIMED OUT` or
the link reason in an `output`, the failures in `--status-caution` with their text, and nothing for
`ok` and `idle`.

- Files: `lib/useServerRequest.ts`, `lib/useServerRequest.test.tsx`, `components/RequestStatus.tsx`,
  `components/RequestStatus.test.tsx`, `styles.css`.
- Tests: `pending` then `ok`; a changed body sends `cancel` for the first and its late response is
  ignored; `timed_out` with `vi.useFakeTimers()` and a `cancel` on the wire; unmounting mid-flight
  cancels and sets no state; `null` gives `idle`; a socket close gives `link_down`, and after
  reconnect and welcome the same body is sent again.
- Acceptance: tests pass.

### P05.T6 Universe creation and opening

**P05.T6.a `UniverseContext`.** `lib/universe.ts`: a provider in `App` holding the universe list
(`list_universes` through `useServerRequest`, refreshed after every successful create), the open
`UniverseInfo`, and the state of the last create or open command, sent through one `RequestChannel`
of its own. `create` opens the new universe on success, so creation is one action; the
`create_universe` response already is the `UniverseInfo`, so no second request is made.
`openUniverse` sends `open_universe`, which also warms the galaxy on the server. Requests are
stateless, so a reconnect changes nothing here but the list refresh.

- Files: `lib/universe.ts`, `lib/universe.test.tsx`, `App.tsx`.
- Tests: the list is requested once the link is `connected`; `create("SURVEY 1", seed)` sends the
  name and the lower-case seed, and `open` becomes the response; `create(name, null)` sends
  `seed: null`; a rejected create (`name_taken`) leaves `open` unchanged and exposes the reason; the
  list is requested again after a create and after a reconnect.
- Acceptance: tests pass.

**P05.T6.b `UniversePanel`: list and open.** A panel titled `UNIVERSE`. The open universe is shown
in a readout (`NAME`, `SEED`, `GEN VER`, `ID`), or em dashes with `NO UNIVERSE OPEN`. Below it a
table of universes (columns `NAME`, `SEED`, `GEN VER`), hex and numbers in B612 Mono, each row with
an `OPEN` button named `Open universe <name>`; the open one shows `OPEN` as text with
`aria-current="true"` in place of the button. A universe whose status is `generator_mismatch` has a
disabled button described by `GENERATOR VERSION <n>: SERVER IS <m>`, from
`server_generator_version`. The table scrolls inside the panel and shows `1-8 of 23`
(`windowRange`). An empty list reads `NO UNIVERSES: CREATE ONE BELOW`.

- Files: `displays/galaxy/UniversePanel.tsx`, `UniversePanel.test.tsx`, `styles.css`.
- Tests: rows from a fixture list; clicking `OPEN` sends `open_universe`, shows `PENDING`, then the
  readout shows the name and the seed in upper case; a rejection shows `REJECTED: <reason>`; a
  mismatched universe's button is disabled with its description; with the link down every button is
  disabled and described by `NO CARRIER`.
- Acceptance: tests pass.

**P05.T6.c Name, seed and creation.** In the same panel a `fieldset` `NEW UNIVERSE`: a text input
`NAME` with the hint `1-48 CHARACTERS`; a radio group `SEED` with `RANDOM` (the default; the server
draws it, plan 04 note 19) and `ENTERED`; a text input `SEED VALUE`, enabled for `ENTERED`, with the
hint `1-16 HEX DIGITS`, right-aligned, in B612 Mono; and `CREATE`. `lib/seed.ts`:
`parseSeedHex(text)` returns `{ ok: true; seed: SeedHex }` (trimmed, lower-cased, left-padded,
checked with `isHex64`) or `{ ok: false }`. Nothing is disabled silently: `CREATE` stays enabled,
and an invalid submit shows `NAME INVALID: ENTER 1 TO 48 CHARACTERS` or
`SEED INVALID: ENTER 1 TO 16 HEX DIGITS (0-9, A-F)` in an element tied to its input with
`aria-describedby` and `aria-invalid`, and sends nothing. `Enter` in either field submits. `CREATE`
changes server state, so it carries the `--line-strong` control outline that display-only controls
lack, shows `PENDING`, then the result: the new universe open in the readout with its seed (which is
how the operator learns a random one), or `REJECTED: <reason>`.

- Files: `displays/galaxy/UniversePanel.tsx`, `lib/seed.ts`, `lib/seed.test.ts`,
  `UniversePanel.test.tsx`.
- Tests: `parseSeedHex` table (empty, 17 digits, `g`, upper case, leading zeros, surrounding
  spaces); `formatHex64` upper-cases; with `RANDOM`, `CREATE` sends `seed: null` and the seed field
  is disabled; choosing `ENTERED`, typing `beef` and pressing `Enter` sends `000000000000beef`; each
  invalid message appears and nothing is sent; after the response the new universe is listed and
  open, and the readout shows `000000000000BEEF`.
- Acceptance: tests pass.

**P05.T6.d `GalaxyDisplay` shell and layout.** Replace the placeholder. A CSS grid with named areas,
sized in `rem`, that fills the work area without scrolling at 1280 × 720 and 1920 × 1080:

```text
| UNIVERSE   | GALAXY MAP (face-on) | LOCAL CHART (view)      | SYSTEMS (list) |
| PARAMETERS | GALAXY MAP (edge-on) | LOCAL CHART (controls,  | READOUT        |
|            |  legend, centre      |  census, legend)        |                |
```

Columns `minmax(16rem, 20rem) minmax(18rem, 26rem) minmax(20rem, 1fr) minmax(15rem, 18rem)`. The
work area's auto-fill grid stays for `LINK`; `GALAXY` sets its own through a modifier class. With no
universe open, the three data panels show their titles and `NO UNIVERSE OPEN`. Panels are `section`s
labelled by their `h2`, as `ConnectionPanel` is.

- Files: `displays/galaxy/GalaxyDisplay.tsx`, `GalaxyDisplay.test.tsx`, `App.tsx`, `styles.css`.
- Tests: four regions by name; the empty state; after a universe opens the empty-state text is gone.
- Acceptance: tests pass; by eye (`just client`) at both window sizes there is no page scrollbar.

### P05.T7 Parameters panel

**P05.T7.a Labels, units and readings.** `displays/galaxy/parameterLabels.ts`: `GROUP_LABELS` and
`PARAMETER_LABELS` as `Readonly<Record<string, string>>` keyed by plan 04's dotted keys, with the
fallback of D8a. Labels are upper case, three words or fewer where possible, one name per thing
(`STELLAR MASS`, `SYSTEMS`, `MEAN SYSTEM MASS`, `THIN DISC SCALE LENGTH`, `BAR HALF-LENGTH`,
`PATTERN SPEED`, `ARM PITCH`, `LAST MAJOR MERGER`, …). `components/UnitLabel.tsx` renders a `Unit`
with an exhaustive `switch` over plan 04's ten values: `none` and `count` as nothing, `msun` as
`SolarMassUnit`, `ly`, `Myr`, `Gyr`, `km/s`, `°/Myr`, `°` and `/ly³`, every character of which is in
B612. Pure `toReading(parameter): { label, text, unit, origin }` formats a `Number` by unit (masses
and counts of five digits or more in E notation above 10⁷ and grouped below; lengths grouped to one
decimal; angles and rates to two decimals; shares, which arrive with `unit: none`, to three
significant figures) and passes `Text` through in upper case. It converts nothing (D8a).

- Files: `displays/galaxy/parameterLabels.ts`, `displays/galaxy/parameterReadings.ts`,
  `parameterReadings.test.ts`, `components/UnitLabel.tsx`, `components/UnitLabel.test.tsx`.
- Tests: 5.2 × 10¹⁰ with `msun` reads `5.20E10`; 8,480.4 with `ly` reads `8,480.4`; 2.227 with
  `deg_per_myr` reads `2.23` with unit `°/Myr`; an unknown key falls back to the key; `UnitLabel`
  for `msun` has the `img` named "solar masses".
- Acceptance: tests pass; a `Unit` value missing from `UnitLabel` is a type error.

**P05.T7.b `ParametersPanel`.** Requests `galaxy_parameters` for the open universe through
`useServerRequest`. Renders the seed and generator version first, then each group as an `h3` plus a
table with columns label, value (B612 Mono, right-aligned), unit (`--text-muted`) and origin in
words (`DRAWN`, `DERIVED`, `FIXED`). The body scrolls vertically inside the panel and shows
`formatListPosition` for the rows in view, computed by `windowRange` (T3.c) from the scroll offset
and a fixed row height. All rows stay in the DOM: there are only a few dozen. States through
`RequestStatus`.

- Files: `displays/galaxy/ParametersPanel.tsx`, `ParametersPanel.test.tsx`, `styles.css`.
- Tests: sends the request naming the open universe; shows `PENDING`, then the row named
  `STELLAR MASS` reads `5.20E10` with an `img` named "solar masses" and `DRAWN`; a rejection shows
  its reason; a change of universe requests again.
- Acceptance: tests pass.

### P05.T8 Galaxy map

**P05.T8.a Map geometry.** `lib/galaxy/mapGeometry.ts`. Decoding the base64 and the codes is plan
04's `decodeDensityMap`; this module adds where a pixel is. `mapGeometry(map)` derives from
`centre_ly`, `ly_per_px`, `width_px` and `height_px` the ranges of the horizontal axis (x) and the
vertical axis (y face-on, z edge-on), with row 0 at the top. `pixelToLy(geometry, column, row)`
gives the pixel's centre; `lyToPixel` is its inverse, fractional, clamped to the picture.

- Files: `lib/galaxy/mapGeometry.ts`, `mapGeometry.test.ts`.
- Tests: a 512 × 512 map centred on the origin at 256 ly per pixel spans ±65,536 ly on both axes;
  the 512 × 256 edge-on map spans ±32,768 ly vertically; row 0 has the largest vertical value;
  `pixelToLy` then `lyToPixel` is the identity on every pixel of a 5 × 3 map; clamping.
- Acceptance: tests pass.

**P05.T8.b Ramp and rasteriser.** `lib/galaxy/ramp.ts`: `parseHexColour("#c8d6e5")` (trimmed first,
since a computed custom property keeps the stylesheet's leading space) to `{ r, g, b }` or a named
error; `buildRamp(surface0, text): Uint8ClampedArray` (256 RGBA levels, linear in sRGB, level 0
exactly `surface0`, level 255 exactly `text`); `codeToLevel(code, maxCode)`: 0 for code 0, otherwise
`1 + round((code − 1) ÷ (maxCode − 1) × 254)`, so that "above the floor" is never painted as the
floor and 16-bit maps work too; `rasterise(decoded, ramp): Uint8ClampedArray`
(`width × height × 4`). The ramp is logarithmic because the codes are linear in log density (plan
04, note 12); the module's doc comment says so. It is monotonic in every channel for the guide's two
tokens.

- Files: `lib/galaxy/ramp.ts`, `ramp.test.ts`.
- Tests: end points; monotonic; code 1 is level 1 and `maxCode` is level 255 at 8 and at 16 bits;
  `rasterise` output length and the first pixel's bytes for a 2 × 2 fixture.
- Acceptance: tests pass; no colour literal outside tests (`grep -n "#[0-9a-f]\{6\}"` on the file is
  empty).

**P05.T8.c `DensityLegend`.** A DOM and inline-SVG legend: a horizontal bar made of 32 `rect`s whose
fills come from the ramp (so it is the same ramp as the picture), a tick and a B612 Mono label at
each whole decade between floor and ceiling (`1E-3` … `1E2`, D7), the title `COLUMN DENSITY`, the
unit `SYSTEMS/ly²`, the words `LOG SCALE`, and `FLOOR 1.00E-4: AT OR BELOW SHOWN AS BACKGROUND`. The
SVG has `role="img"` and an accessible name that states the range and the unit in words.

- Files: `displays/galaxy/DensityLegend.tsx`, `DensityLegend.test.tsx`.
- Tests: for floor −4 and ceiling 2.3, ticks `1E-4` to `1E2` (seven); the unit, `LOG SCALE` and the
  floor sentence are present; the `img` name contains "systems per square light-year".
- Acceptance: tests pass.

**P05.T8.d `GalaxyMapView`.** One view of the map. Props: `universe`, `view`, `population`,
`cursorLy`, `onCursor`. Requests `density_map` (D5, 120 s timeout), decodes inside a `try` whose
failure reads `MAP DATA INVALID`, rasterises into an `ImageData` on an offscreen canvas of the map's
size, and draws it onto a visible canvas sized by its container (a `ResizeObserver`,
device-pixel-ratio backing store) with smoothing off (D6), keeping the map's aspect ratio so that
the picture is true to scale. Around it, in the DOM: the view title (`FACE-ON FROM NORTH` or
`EDGE-ON ALONG +Y`), axis labels (`+X` right; `+Y` or `+Z NORTH` up), the frame name `GALACTIC`, the
rotation sense (`ROTATION COUNTER-CLOCKWISE`) face-on, a `ScaleBar` (T10.g; until it exists, the
pure `scaleBar` of T9.c with a plain `div`), and the view's own `DensityLegend`, since the two views
have different floors. Painting happens in a layout effect when the map, the size or the tokens
change, never on a loop. `RequestStatus` covers the other states.

- Files: `displays/galaxy/GalaxyMapView.tsx`, `GalaxyMapView.test.tsx`,
  `test/RecordingContext2D.ts`, `test/FakeResizeObserver.ts`, `styles.css`.
- Tests (canvas stubbed with `stubCanvas()`): the request names universe, view, population,
  resolution 512 and 8 bits; `PENDING` then one `putImageData` and one `drawImage` with smoothing
  off; the title, frame, axes, legend and scale bar text; no second paint on an unrelated re-render;
  a changed population sends `cancel` and a new request; truncated data shows `MAP DATA INVALID`.
- Acceptance: tests pass.

**P05.T8.e `GalaxyMapPanel`.** Both views at once, face-on above edge-on and half its height, which
is their true proportion. A radio group `POPULATION`: `ALL SYSTEMS`, `YOUNG`. The panel owns
`population` and the cursor.

- Files: `displays/galaxy/GalaxyMapPanel.tsx`, `GalaxyMapPanel.test.tsx`.
- Tests: two canvases named `Galaxy map, face-on` and `Galaxy map, edge-on`; choosing `YOUNG` sends
  two new requests with the young population; each view has its own legend with its own floor.
- Acceptance: tests pass.

**P05.T8.f The map cursor.** Each map canvas is focusable (`tabIndex={0}`, `role="application"`,
named, focus ring, a visible key hint `ARROWS MOVE CURSOR`). A click or tap sets the cursor from
`pixelToLy`: x and y face-on; z only edge-on (D19). Arrow keys move it by one map pixel, ten with
`Shift`, clamped to the extent. The cursor is drawn as a DOM crosshair over both canvases (face-on
at x, y; edge-on at x, z), a thin `--accent` cross with a gap at the centre, positioned by
`lyToPixel` scaled to the canvas box. A `CURSOR` readout beside the map gives `X`, `Y`, `Z` in ly,
`RADIUS`, `ANGLE`, `HEIGHT` from `cylindrical`, and the column density under the cursor in each view
from `log10PerLy2` (`DENSITY 3.16E0 SYSTEMS/ly²`, or `BELOW FLOOR` for code 0), which is what tuning
by eye needs.

- Files: `displays/galaxy/GalaxyMapView.tsx`, `GalaxyMapPanel.tsx`, tests, `styles.css`.
- Tests: with a stubbed `getBoundingClientRect`, a click at the centre of the face-on canvas sets
  `X` and `Y` to the extent's centre and leaves `Z`; a click edge-on changes `Z` only;
  `{ArrowRight}` on the focused face-on map raises `X` by one pixel width; clamping at the edge; the
  readout's numbers carry `ly` and `°`.
- Acceptance: tests pass.

**P05.T8.g Centre entry and `CENTRE CHART`.** `CentreEntry`: three numeric fields `X`, `Y`, `Z` in
ly, bound to the cursor, right-aligned, with the format hint `±65,536 ly`. Out-of-cube or
non-numeric input shows `OUT OF RANGE: −65,536 TO 65,536 ly` and does not move the cursor. The
button `CENTRE CHART` with its key `C` (D3) publishes the cursor as the chart centre through
`onCentre`. The current chart centre is marked on both maps with a bracket reticle in `--text`,
distinct from the crosshair.

- Files: `displays/galaxy/CentreEntry.tsx`, `CentreEntry.test.tsx`, `GalaxyMapPanel.tsx`.
- Tests: typing `26000` into `X` moves the crosshair; an invalid value shows the message and keeps
  the old cursor; pressing `c` outside a field calls `onCentre` with the cursor; pressing `c` inside
  a field types a character and does not.
- Acceptance: tests pass.

### P05.T9 Spatial view: pure core

Every subtask is pure TypeScript with no DOM, tested with plain `vitest` files beside the code. The
module must not import from `displays/` or `lib/galaxy/`: it knows nothing about stars. Lengths are
in the scene's unit, passed with a formatter, so the orbit map can use kilometres.

**P05.T9.a Vectors and the local frame.** `vec3.ts` and `frame.ts` (D11). `cylindrical` returns
`{ radiusLy, angleDeg, heightLy }` with the angle in [0, 360) from +x towards +y.

- Tests: at (26,000, 0, 0) coreward is (−1, 0, 0) and spinward (0, 1, 0); at (0, −5, 0) coreward is
  (0, 1, 0) and spinward (1, 0, 0); `cross(spinward, coreward)` equals north at 100 random points
  (fixed seed); the axis fallback and its flag; `toLocal` round-trips.
- Acceptance: `vitest run src/renderer/src/spatial/frame.test.ts` passes.

**P05.T9.b Camera and projection.** `camera.ts` with the formulae of D10. `wrapAzimuthDeg` maps to
[0, 360); `clampElevationDeg` to [−90, 90]; `rotateCamera(camera, dAzDeg, dElDeg)`;
`zoomCamera(camera, factor, limits)`; `fitPxPerUnit(radius, viewport, marginPx)` fits a sphere of
the query radius inside the shorter side; `project(relative, basis, camera, viewport)` gives
`{ xPx, yPx, depth }`. `viewBasis` takes the `LocalFrame`, so the same code serves a body-centred
frame later.

- Tests: the basis is orthonormal for a grid of angles including ±90°; `TOP` puts a coreward point
  straight up and a spinward point to the right; `SIDE` puts north up and spinward right; `FRONT`
  puts north up and rimward right; at −90° the picture is the mirror of `TOP` about the horizontal
  axis; a point on a sphere of radius R about the centre projects at k·R or less from the centre at
  1,000 random orientations, and one on its limb at exactly k·R (the range circle property); no
  roll: north projects with zero x component at every angle short of ±90°; elevation clamps; azimuth
  wraps from 359 + 2 to 1.
- Acceptance: the test file passes.

**P05.T9.c Scale and steps.** `floor125(x)` and `ceil125(x)`: the nearest value of {1, 2, 5} × 10ⁿ
at or below, or at or above, a positive `x`, computed from integer exponents so that 0.01, 0.02 and
0.05 come out exactly. `scaleBar(pxPerUnit, maxBarPx): { length, lengthPx }` is the largest 1-2-5
length that fits. `gridSpacing(queryRadius)` is `floor125(queryRadius ÷ 2.5)` (2.5 to 5 rings).
`RADIUS_STEPS_LY`: 0.01, 0.02, 0.05, … 500 (15 steps).

- Tests: tables for each, including 0.01, 0.0100000001, 0.3, 50, 499; the bar never exceeds
  `maxBarPx` and is at least 40% of it; steps are strictly increasing and each equals its own
  `floor125`.
- Acceptance: the test file passes.

**P05.T9.d Marks and symbols.** `marks.ts`: the scene handed to the view.

```ts
interface PointMark {
  readonly id: string;
  readonly position: Vec3; // relative to the view centre, scene units
  readonly shape: SymbolShape; // "circle" | "diamond" | "square" | "triangle"
  readonly sizeClass: SizeClass; // 0..4
  readonly status: MarkStatus; // "plain" | "available"
  readonly label: string;
  readonly labelPriority: number;
}
interface SphereMark {
  readonly radius: number;
  readonly role: "range" | "data_edge";
  readonly label: string;
}
interface PlaneSpec {
  readonly spacing: number;
  readonly extent: number;
  readonly rings: ReadonlyArray<{ radius: number; label: string }>;
}
interface SpatialScene {
  readonly frame: LocalFrame;
  readonly points: ReadonlyArray<PointMark>;
  readonly spheres: ReadonlyArray<SphereMark>;
  readonly plane: PlaneSpec;
  readonly selectedId: string | null;
  readonly destinationId: string | null;
}
```

`symbols.ts`: `symbolOutline(shape)` as a closed unit polygon (or the marker `"circle"`), and
`SIZE_CLASS_REM` (D14). Above or below the plane is derived (`position · north ≥ 0` is above, so a
mark exactly on the plane is filled), never passed in.

- Tests: each polygon is closed, centred and fits the unit circle; sizes strictly increase and the
  smallest is 0.5 rem or more.
- Acceptance: the test file passes.

**P05.T9.e Plane geometry.** `plane.ts`: `gridLines(plane, frame)` gives chords of the disc of
radius `extent` in the reference plane, parallel to coreward and to spinward, at multiples of
`spacing`, each as two end points. `ringPolyline(radius, frame, segments = 96)` gives a closed
polyline in the plane.

- Tests: for extent 50 and spacing 20 there are five chords each way, the one through the centre 100
  long and those at ±40 of length 60; every end point has zero north component and lies on the
  disc's edge; the first chord family is parallel to coreward at a centre off the x axis (the grid
  aligns to the local directions, not to x and y); a ring's points are all at `radius`.
- Acceptance: the test file passes.

**P05.T9.f The draw list.** `drawList.ts`: `buildDrawList(scene, camera, viewport)` returns
`{ ops, anchors, curveLabels }`. `DrawOp` is a union of `line`, `polyline`, `circle`, `symbol`,
`reticle` and `ticks`, each with a `stroke` and optional `fill` given as a token name
(`"text" | "accent" | "target" | "line"`), never a colour. Rules: the order of D12; a stalk is a
`line` from the projected symbol to its projected foot, in the symbol's colour, 1 px; a symbol is
filled above the plane and open below with the same outline; `available` marks use `accent`, others
`text`; size comes from `sizeClass` and `viewport.remPx` only; the selection is a bracket `reticle`
in `accent`, 0.5 rem larger than the symbol, and the destination a `reticle` in `target`, and a mark
that is both gets both, the destination outside; spheres become a `circle` about the view centre of
radius k·R, `range` 1.5 px `text`, `data_edge` 1 px `text` plus `ticks` (D13), merged when the radii
are equal; grid and rings are `line` and `polyline` ops in `line`. `anchors` has one
`{ id, xPx, yPx, depth, radiusPx }` per point mark. `curveLabels` gives a position for each sphere
label (top of the circle) and each ring label (the ring's coreward point).

- Tests: with ε = +30° the ops are [below marks…, plane…, above marks…, annotations] and with −30°
  the halves swap; at ε = 0 the ε ≥ 0 order holds; within a half, depth decreases down the list; a
  stalk precedes its own symbol; fill flags follow height; colours follow status; a mark's size is
  the same at every camera angle and depth (no dimming, no depth scaling: there is no opacity field
  at all); the range circle's radius is k·R at three orientations; equal radii give one circle and
  two labels; equal depths order by ID; 4,000 marks build in one call without error.
- Acceptance: the test file passes.

**P05.T9.g Picking.** `pick(anchors, pointPx, tolerancePx)`: candidates are anchors whose centre is
within `max(tolerancePx, radiusPx)` of the point; the winner has the smallest screen distance;
candidates whose distances differ by under 0.5 px are tied and the smaller depth (nearer the viewer)
wins, then the lower ID. The caller passes a tolerance of 1 rem.

- Tests: nothing within tolerance gives `null`; the nearer of two wins; two coincident marks give
  the one nearer the viewer; a large symbol is pickable at its rim beyond the tolerance; the result
  does not depend on anchor order.
- Acceptance: the test file passes.

**P05.T9.h Labels.** `chooseLabels(points, selectedId, destinationId, count = 8)` returns the
selected and destination marks first, then the highest `labelPriority` (the chart passes initial
mass), ties by ID. `placeLabels(chosen, anchors, viewport)` puts each label to the right of its
symbol, flips it left at the right edge, estimates its box from its length (0.62 em a character at
0.875 rem, since jsdom cannot measure), and drops a lower-priority label whose box overlaps a placed
one. The selected label is never dropped.

- Tests: selection first; count respected; overlap drops the lesser; edge flip.
- Acceptance: the test file passes.

**P05.T9.i Transitions.** `TRANSITION_MS = 120` (inside the guide's 80–150). `easeOut(t)` is
`1 − (1 − t)³`. `tweenCamera(from, to, progress)` moves azimuth along the shorter arc, elevation and
the logarithm of zoom linearly, and returns `to` exactly at progress 1.

- Tests: 350° to 010° passes through 000°, not 180°; end points exact; `easeOut` is monotonic with
  `easeOut(0) = 0` and `easeOut(1) = 1`.
- Acceptance: the test file passes.

**P05.T9.j Redraw scheduler.** `createRedrawScheduler(requestFrame, cancelFrame)` returns
`{ request(callback), dispose() }`. Any number of `request` calls before the frame fires produce one
frame and run the latest callback once. With no request there is no frame: the scheduler never
re-arms itself. `dispose` cancels a pending frame.

- Tests (injected fake frame functions): three requests, one frame; no request, no frame, even after
  a frame has run; dispose cancels.
- Acceptance: the test file passes.

### P05.T10 Spatial view: component

**P05.T10.a Painter, tokens and canvas fakes.** `paint.ts`: `readTokens(element)` reads `--text`,
`--accent`, `--target`, `--line` and `--surface-0` from the computed style;
`paint(context, drawList, tokens)` clears to `surface-0` and executes each op with an exhaustive
`switch` and no `default`. Circles use `arc`, symbols a path scaled from the unit outline, brackets
four corner paths. No `fillText`, no `globalAlpha`, no shadow, no gradient.
`test/RecordingContext2D.ts` records method calls and property sets; `stubCanvas()` spies on
`HTMLCanvasElement.prototype.getContext` to return it (jsdom has no 2D context).

- Files: `spatial/paint.ts`, `paint.test.ts`, `test/RecordingContext2D.ts`.
- Tests: a filled symbol calls `fill` and `stroke`, an open one only `stroke`; stroke styles are the
  token values given; the call order follows the op order; the recorder never sees `fillText`,
  `globalAlpha`, `shadowBlur` or `createLinearGradient`.
- Acceptance: tests pass; `grep -n "fillText\|globalAlpha" apps/hyperion/src/renderer/src/spatial`
  is empty.

**P05.T10.b `SpatialView` skeleton.** Props: `scene`, `fitRadius`, `formatLength`, `frameName`,
`accessibleName`, `onSelect(id)`, and slots for furniture (`children` rendered in an overlay). It
owns the `Camera` (D16), initialised to `OBLIQUE` at `fitPxPerUnit`. Structure: a positioned
container; a `canvas` with `tabIndex={0}`, `role="application"`, `aria-label={accessibleName}`,
`aria-describedby` pointing at a visible key legend
(`ARROWS ROTATE  +/− ZOOM  T S F O VIEWS  Z FIT`); an overlay `div` for labels. A `ResizeObserver`
sets the viewport; the backing store is scaled by `devicePixelRatio`; `remPx` is read from the root
font size. A layout effect builds the draw list and paints when scene, camera, viewport or tokens
change. There is no animation loop.

- Files: `spatial/SpatialView.tsx`, `SpatialView.test.tsx`, `test/FakeResizeObserver.ts`,
  `styles.css`.
- Tests: `getByRole("application", { name })` exists and takes focus by `Tab`; one paint after mount
  and size; a re-render with equal props paints nothing more; a new scene paints once; unmount
  disconnects the observer and cancels a pending frame.
- Acceptance: tests pass.

**P05.T10.c Keyboard rotation and zoom.** On the focused canvas: `ArrowLeft`/`ArrowRight` change
azimuth by 5°, `ArrowUp`/`ArrowDown` elevation by 5°, 1° with `Shift`; `+`, `=` zoom in and `−`, `-`
zoom out by a factor of 1.25; `Z` fits. Zoom is clamped to [0.5, 100] × the fit scale, and is
independent of both radii. Arrow keys call `preventDefault` only on the canvas. Key repeats are
coalesced through the scheduler.

- Tests: with focus on the canvas, `{ArrowRight}` changes the azimuth readout from `030°` to `035°`
  (advance fake timers past the 4 Hz throttle); 13 presses of `{ArrowUp}` stop at `+90°`; arrows
  with focus on a sibling list do not rotate; `+` changes the scale bar's label.
- Acceptance: tests pass.

**P05.T10.d Pointer rotation, wheel and pinch.** Pointer events with `setPointerCapture`: a drag
rotates 8° per rem of travel (0.5° per CSS px at 100%), horizontal to azimuth, vertical to elevation
(dragging down raises the camera). Movement under 0.25 rem between down and up is a click (used by
T10.i), not a drag. The wheel zooms by `2^(−deltaY ÷ 400)` through a non-passive listener added in
an effect, so that the page never scrolls. Two active pointers zoom by the ratio of their
separations. Nothing depends on hover or the right button. `touch-action: none` on the canvas.

- Tests (`fireEvent.pointer*`, which `user-event` cannot fully produce for capture and pinch): a 32
  px horizontal drag changes azimuth by 16°; a 2 px movement is a click and leaves the camera; wheel
  changes the scale bar; two pointers moving apart zoom in; `pointercancel` ends a drag.
- Acceptance: tests pass.

**P05.T10.e Presets and transitions.** `PresetButtons`: four `button`s `TOP`, `SIDE`, `FRONT`,
`OBLIQUE`, each showing its key (`T`, `S`, `F`, `O`), with `aria-pressed` true while `matchesPreset`
holds. Keys per D3. Choosing a preset tweens the camera over `TRANSITION_MS` with `easeOut`, a frame
at a time through the scheduler, and lands exactly on the preset. Under
`prefers-reduced-motion: reduce` (read with `matchMedia`, subscribed to changes) the camera is set
at once. An operator input during a transition cancels it. Zoom is not changed by a preset.

- Files: `spatial/PresetButtons.tsx`, `SpatialView.tsx`, tests, `test/stubMatchMedia.ts`.
- Tests: clicking `TOP` ends at `000°` and `+90°` with `aria-pressed`; pressing `s` does the same
  for `SIDE`, and does nothing while a text field has focus; with reduced motion there is exactly
  one paint and no intermediate frame; without it, frames stop after 120 ms of fake time and no
  frame is requested afterwards (no idle drift).
- Acceptance: tests pass.

**P05.T10.f Triad, core arrow and angle readouts.** `AxisTriad`: an inline SVG, `role="img"`, named
`Axis triad`, drawing the projections of the coreward, spinward and north unit vectors from a common
origin, each with a DOM-text label (`COREWARD`, `SPINWARD`, `NORTH`); an axis pointing away from the
viewer ends in an open circle with a cross, one pointing at the viewer in a circle with a dot, so
that the mirrored south view is readable. `CoreArrow`: an arrow at the rim of the view pointing
along the projected coreward direction, labelled `CORE` with the distance to the axis
(`CORE 26,000 ly`); when coreward is within 5° of the line of sight it becomes the away or towards
symbol. Azimuth and elevation are shown in a `dl` as `AZM 030°` and `ELV +30°` through
`useThrottledValue(camera, 250)`. They are not in a live region: `dd`, not `output`. At the axis
fallback the triad is labelled `−X`, `+Y`, `NORTH` and the arrow is replaced by the D11 message.

- Files: `spatial/AxisTriad.tsx`, `spatial/CoreArrow.tsx`, `spatial/useThrottledValue.ts`, tests.
- Tests: `useThrottledValue` yields the first value at once, then at most one change per 250 ms, and
  the final value after the trailing interval (fake timers); ten rotations inside 100 ms change the
  readout text once; at `TOP` the triad's north is the towards symbol and at −90° the away symbol;
  the core arrow points up at `TOP`; no element in the view has `aria-live` or an implicit live role
  whose text changes during a drag.
- Acceptance: tests pass.

**P05.T10.g Scale bar, frame, centre and time.** `ScaleBar`: a DOM bar whose width is `lengthPx`
from `scaleBar` with end ticks and its label from `formatLength` (`20 ly`, `0.05 ly`, `500 AU`). A
fixed furniture row shows the frame (`FRAME GALACTIC`), the centre (`RADIUS 26,000.0 ly`,
`ANGLE 045.0°`, `HEIGHT +12.0 ly`), and the time (`UT +0.00 yr`), all passed in as props so the view
stays general. These keep the same place on every spatial display.

- Tests: zooming in by 1.25 repeatedly steps the label 20 → 10 → 5 … and down through `0.01 ly` to
  `500 AU`; the bar's width never exceeds its maximum; frame, centre and time text are present with
  units.
- Acceptance: tests pass.

**P05.T10.h DOM labels over the canvas.** Mark labels from `chooseLabels` and `placeLabels`, and
curve labels from the draw list, are absolutely positioned `span`s in the overlay, moved with
`transform: translate(…rem, …rem)`, B612 at 0.875 rem, `pointer-events: none`, and `aria-hidden`
(the list and the readout carry the same text accessibly). Reachable marks' labels are `--accent`,
like their symbols.

- Tests: the selected mark's label text is in the overlay; at most nine mark labels; `RANGE 50 ly`
  and `PLANE 50 ly` are both present and differ; with equal radii `QUERY EDGE` and `RANGE` both
  appear.
- Acceptance: tests pass.

**P05.T10.i Pointer picking, selection and destination.** A click (T10.d) calls `pick` with a
tolerance of 1 rem and then `onSelect(id)`; a click on empty space selects nothing and keeps the
current selection. `scene.selectedId` draws the `accent` reticle and `scene.destinationId` the
`target` reticle; both are inputs, so the list and the view cannot disagree.

- Tests: with one mark projected at a known point, a click 12 px away (under 1 rem) selects it and
  one 24 px away does not; of two coincident marks the nearer is selected; the draw list painted
  after selection contains a `reticle` op in `accent`, and one in `target` for a destination.
- Acceptance: tests pass.

### P05.T11 Local chart

**P05.T11.a Chart model.** `chartModel.ts`, pure:
`toScene(result, { frame, driveRangeLy, selectedId, destinationId }): SpatialScene`. The query
radius is the result's own. A system is `available` when `distanceLy ≤ driveRangeLy` (3D distance at
the chart time, not the projected one); `sizeClass` is `layerIndex(layer)`; `labelPriority` the
initial mass; the label the designation. Spheres: `range` at the drive range when it is within the
query radius or equal to it, labelled `RANGE <n> ly SET` (D9), and `data_edge` at the query radius,
labelled `QUERY EDGE <n> ly`. Plane: `gridSpacing(radiusLy)`, extent the query radius, grid rings at
each multiple of the spacing, and the labelled plane ring at the drive range. `censusLine(census)`
gives `{ text: "COMPLETE ABOVE", aboveMsun }` or `{ text: "NOTHING FITS: REDUCE RADIUS" }`.
`censusHint(layers)` gives `DENSE REGION: REDUCE RADIUS BEFORE RAISING MIN MASS` when a layer is
`over_limit`, `LARGE VOLUME: LAYERS DROPPED BY SERVER CELL BUDGET` when one is `over_cell_budget`,
and nothing otherwise, through an exhaustive `switch` on `LayerStatus`. `layerBands(layers)` gives
the legend's five bands from the census's `mass_min_msun` and `mass_max_msun`, so the client holds
no copy of the band table.

- Files: `displays/galaxy/chartModel.ts`, `chartModel.test.ts`.
- Tests: a system at 49.9 ly that projects inside the circle is available and one at 50.1 ly that
  also projects inside it (behind the centre) is not; a system high above the plane whose foot is
  inside the plane ring is not available; with drive range above the query radius there is no range
  sphere and every system is available; ring radii for 50 and for 0.05 ly; the range label ends in
  `SET`; both census lines; each hint.
- Acceptance: the test file passes.

**P05.T11.b `useRangeQuery`.** Builds the body with `toRangeRequest` from
`{ universe, centreLy, queryRadiusLy, minLayer, timeYr }`, memoised by value, and sends it through
`useServerRequest`. Returns `{ state, shown }`, where `shown: ChartResult | null` is the adapter's
output for the latest `ok` response of the same universe and is kept while a newer request is
pending or has failed. No centre gives `idle` and `null`.

- Files: `displays/galaxy/useRangeQuery.ts`, `useRangeQuery.test.tsx`.
- Tests: the sent body carries universe, centre (as cell and offset), radius, time, `min_layer` and
  the limit; changing the radius sends `cancel` and a new query, and the late first response is
  ignored; `shown` survives a pending and a rejected successor; a new universe clears it; no query
  without a centre.
- Acceptance: tests pass.

**P05.T11.c Chart controls.** `ChartControls`: `QUERY RADIUS`, a `select` over `RADIUS_STEPS_LY`
with `ly` in each option; `MIN MASS`, a radio group of the five band floors (`0.08`, `0.5`, `0.75`,
`2.5`, `8`, each with `SolarMassUnit`; the first also reads `ALL`), reported as a `MassLayer`, which
is also the declutter control; `DRIVE RANGE SET` (D9), a numeric field in ly with format hint and
validation, 0.01–500; `CHART TIME`, a numeric field in years within ±1,000 labelled `UT`, with
validation. Until the first result arrives the floors are labelled by layer letter only, since the
bands come from the census. While the operator has not touched the radius it follows the drive range
(rounded up with `ceil125` to a step).

- Files: `displays/galaxy/ChartControls.tsx`, `ChartControls.test.tsx`.
- Tests: the radius options run from `0.01 ly` to `500 ly`; choosing `0.75` reports layer C; an
  out-of-window time shows `OUT OF RANGE: −1,000 TO 1,000 yr` and reports nothing; the default
  radius equals the default drive range; changing the drive range to 80 moves an untouched radius to
  100 and leaves a touched one.
- Acceptance: tests pass.

**P05.T11.d Census, counts and states.** In `LocalChartPanel`: the census line always visible while
a result is shown, with `SolarMassUnit`, and the hint of T11.a under it; `SYSTEMS 1,612` and
`IN RANGE 1,204`; a census table with one row per layer: letter, band, `EXP` (expected, one
decimal), `RET` (returned) and the status in words (`INCLUDED`, `OVER LIMIT`, `BELOW MIN MASS`,
`OVER CELL BUDGET`), which is how placement is checked against the fields by eye; `RequestStatus`
for pending and failures, including `REJECTED: <message>` for plan 04's `bad_request`, `queue_full`
and `too_many_requests` (it has no "query too large" error: an over-large volume is answered, with
`over_cell_budget` layers). While a new query is pending the previous result stays on screen with
`PENDING` beside the census, and is replaced only by the response (no optimistic change of the
labelled radius: the view's circles use the radius of the result shown). `NOTHING FITS` clears the
marks, keeps the query edge, and shows the census line in `--status-caution` with its text, the
guide's data-overflow colour.

- Files: `displays/galaxy/CensusReadout.tsx`, `CensusReadout.test.tsx`.
- Tests: each state by its text; the five table rows with their words; the previous chart's list
  rows remain while pending; the caution line for nothing fits.
- Acceptance: tests pass.

**P05.T11.e `SystemList`.** Uses `windowRange` (T3.c) with an overscan of 8 rows. `SystemList`
(D17): a `div role="listbox"` named `Systems by distance`, `tabIndex={0}`, `aria-activedescendant`;
a spacer of `total × 2rem`; rendered rows absolutely positioned, each `role="option"` with a stable
`id` from the system ID, `aria-selected`, `aria-posinset`, `aria-setsize`. Columns: designation,
distance (`ly`, D21), initial mass, and reachability in words (`IN RANGE` or `OUT`), reachable rows
also in `--accent`. Keys while the list has focus: `ArrowUp` and `ArrowDown`, `PageUp` and
`PageDown` (one window), `Home`, `End`; selection follows the active row and is reported through
`onSelect`; the list scrolls the active row into view by setting `scrollTop` from the index (no
`scrollIntoView`, whose target may not be rendered). A click selects. A footer shows
`formatListPosition` for the rows in view. When the selection changes from the chart, the list
scrolls to it.

- Files: `displays/galaxy/SystemList.tsx`, `SystemList.test.tsx`, `styles.css`.
- Tests: with 3,000 fixture systems and a stubbed viewport height of 20 rows, fewer than 60 options
  are in the DOM; `{ArrowDown}` three times selects the fourth-nearest and reports it; `{End}`
  selects the last and the footer reads `2,981-3,000 of 3,000`; an option has `aria-setsize="3000"`;
  a row for a reachable system contains the text `IN RANGE`; selecting from outside scrolls the row
  into the window.
- Acceptance: tests pass.

**P05.T11.f `SystemReadout`.** An `output` named `Selected system` holding a `dl`: `DESIG`, `ID` (16
hex digits), `DIST`, `SET RANGE` (`IN RANGE`/`OUT OF RANGE`, D9), `HEIGHT` (signed, above or below
the reference plane, which is the only non-visual source of the fill cue), `COREWARD` and `SPINWARD`
offsets (signed ly), `RADIUS`, `ANGLE`, `Z` (galactic, of the system), `INIT MASS` with
`SolarMassUnit`, `AGE` with `AT UT +0.00 yr` beside it, and `POPULATION` (upper-case names:
`YOUNG THIN DISC` … `HALO`). With nothing selected every value is an em dash in `--text-muted`.

- Files: `displays/galaxy/SystemReadout.tsx`, `SystemReadout.test.tsx`.
- Tests: values and units for a fixture system below the plane (`HEIGHT -3.20 ly`); an age of 4,600
  Myr reads `4.60 Gyr`; the empty state; the element is an `output`.
- Acceptance: tests pass.

**P05.T11.g `LocalChartPanel`.** Wires it together: owns `selectedId`, query radius, floor, drive
range and chart time; computes the frame from the centre (`localFrameAt`), the scene through
`toScene`, and passes `SpatialView` its furniture props (`GALACTIC`, the centre's cylindrical
coordinates, `UT`). Adds `SymbolLegend`: the five sizes with their bands (`0.08-0.5`, … `8-150`,
`SolarMassUnit`), `SYMBOLS NOT TO SCALE`, and the fill key (`FILLED NORTH OF PLANE`,
`OPEN SOUTH OF PLANE`), the colour key in words (`ACCENT WITHIN SET RANGE`, D9), and the reticle
key. A new result keeps the selection if the system is still present and clears it otherwise. With
no centre chosen the panel reads `NO CENTRE: PICK ON MAP AND PRESS C`. The destination stays `null`
in M1.

- Files: `displays/galaxy/LocalChartPanel.tsx`, `SymbolLegend.tsx`, tests, `styles.css`.
- Tests: selecting in the list draws the reticle (recorded paint) and fills the readout; clicking a
  symbol selects the list option; the legend's texts; a new centre issues a new query and resets
  nothing but the selection; link loss disables the controls with `NO CARRIER` and keeps the chart.
- Acceptance: tests pass.

### P05.T12 Integration

**P05.T12.a End-to-end display test.** One `GalaxyDisplay.integration.test.tsx` that plays the
server through `FakeWebSocket`: welcome → `F2` → empty list → type a name, enter a seed and `CREATE`
→ `create_universe` answered → `galaxy_parameters` and two `density_map` requests answered → click
the face-on map, enter `12` in the centre's `Z` field, press `c` → `systems_in_range` answered with
40 systems → `Tab` to the list, `ArrowDown`, readout filled → press `t`, angles read `000°` and
`+90°` → choose `MIN MASS 0.5`, new query with `min_layer: "b"`, census line changes → enter `12.5`
in `CHART TIME`, the new query's `time` equals `universeTimeFromYears(12.5)`, and once answered the
view and the readout say `UT +12.50 yr` → `F1` and back, and the chart, its camera and the selection
are as they were. Each step is its own `it` sharing a setup helper, so each has one reason to fail.

- Acceptance: the file passes; total `pnpm test` time for the app stays under a minute locally;
  `git diff --stat` for the whole plan shows no change under `apps/hyperion/src/main`,
  `apps/hyperion/src/preload` or to `apps/hyperion/src/renderer/index.html` (the CSP), and no new
  entry in any `package.json`.

**P05.T12.b Cross-stack and by-eye pass.** Run `just server` and `just client`. First go through the
cross-stack checklist under Verification once, from an empty data directory. Then go through the
by-eye and accessibility lists at 1280 × 720 and 1920 × 1080, at root font sizes of 80%, 100% and
150%, with `prefers-reduced-motion` on and off, and once using only the keyboard. Findings are fixed
in the task that owns them or, for the server, reported against plan 04. Record the results in the
PR description, not in a new document.

## Verification

Automated: every task's tests, `just ci`, and the integration test of T12.a. The pure core of the
spatial view (T9) has no DOM in its tests, which is the brainstorm's condition for Canvas 2D.

Cross-stack checklist: galaxy creation end to end, by hand, against a real server. It is the one
manual check that spans plans 01–05, the counterpart of P04.T14.e (server side, automated) and
P05.T12.a (client side, automated against a fake socket). Start from an empty `HYPERION_DATA_DIR`.

1. `just server`, then `just client`. The header reads `LINK NOMINAL`, and the `LINK` display's
   `Protocol` row shows the same version on both sides.
2. `F2`. The `UNIVERSE` panel reads `NO UNIVERSES: CREATE ONE BELOW`, and the other panels
   `NO UNIVERSE OPEN`.
3. Create `SURVEY 1` with the entered seed `4D2`. `CREATE` shows `PENDING`, then the readout shows
   the name, the seed `00000000000004D2`, the generator version and an ID. The data directory holds
   exactly one `universe.json`.
4. `PARAMETERS` fills: the seed, a stellar mass of 3–10 `E10 M☉` marked `DRAWN`, a system count
   marked `DERIVED`, the pattern speed in `°/Myr`. No row lacks a unit that should have one, no row
   shows a raw key, and nothing reads in kpc.
5. Both maps show `PENDING`, then a picture with its legend, within seconds (plan 04's targets at
   512 pixels on eight workers are 1 s face-on and 3 s edge-on). While they compute, the header's
   link state stays nominal and the `LINK` display's latency keeps updating (the socket is not
   stalled).
6. Choose `YOUNG`: both maps are requested again and the arms sharpen. Choose `ALL SYSTEMS` again:
   the maps return at once (the server's cache).
7. Click the face-on map near 26,000 ly from the centre, set `Z` to `0` in the centre entry, press
   `C`. The chart shows `PENDING`, then one to five thousand systems, a census line, and the
   furniture `FRAME GALACTIC`, `RADIUS 26,000 ly` or near it, and `UT +0.00 yr`.
8. Select a system from the list by keyboard. The readout fills; note its `DESIG`, `ID` and `AGE`.
9. Set `CHART TIME` to `500`. A new query goes out; the same system is still listed at the same
   position (velocities are zero until plan 08), its `AGE` is 0.0005 Myr greater (visible only for a
   young system), and the view and readout say `UT +500.00 yr`. Enter `1001`: the field reads
   `OUT OF RANGE: −1,000 TO 1,000 yr` and no query is sent.
10. Stop the server. The header reads `NO CARRIER`, the controls are disabled and say why, and the
    chart stays. Start the server again: the link returns without a reload, the universe list still
    shows `SURVEY 1`, and changing the radius sends a query that is answered (requests are
    stateless, so nothing had to be re-opened).
11. Restart the client. `SURVEY 1` is listed; `OPEN` it; the same centre, radius and time give the
    system noted in step 8 with the same `ID`, position and mass.
12. Create a second universe with `RANDOM`. The readout shows a seed the operator did not type, and
    its maps differ from the first's.

By eye, against a running server (the brainstorm's "By eye" test entry, which this display exists to
serve):

- Face-on, all systems: a barred spiral, bar along x, arms faint. Young population: arms sharp,
  leaving the ends of the bar, trailing for counter-clockwise rotation. Edge-on: thin young disc,
  thicker old disc, boxy bulge, the young disc not aliased away.
- The legend's floor and ceiling bracket what is on screen; the background outside the galaxy is
  exactly the panel's `--surface-0`.
- A chart at (26,000, 0, 0) ly with a radius of 50 ly returns roughly 1,000–5,000 systems depending
  on the seed, or a census line naming a floor. A chart at the centre returns `NOTHING FITS` or a
  high floor until the radius is reduced, and works at 0.05 ly steps.
- In the chart: `TOP` has coreward up; the grid is parallel to the core arrow; `SIDE` and `FRONT`
  show stalks as vertical lines; rotating never rolls the horizon; symbols below the plane are open
  at every zoom and at 80% scale; a system behind the range sphere is `--text`, not `--accent`; the
  range circle stays a circle while the plane ring flattens; nothing moves unless the operator moves
  it (check the Performance panel for zero frames while idle).
- A drag with 4,000 systems stays at frame rate on the development machine. If not, record the frame
  time and the mark count as a finding; WebGL is not adopted in this plan.
- Accessibility: every control reachable by `Tab` with a visible focus ring; every action in this
  plan done once without a pointer; no state told by colour alone (reachability, selection, active
  tab, failures all have words or shapes); pointer targets 2 rem; the angle readouts silent in a
  screen reader during a drag while the `Selected system` output is announced on selection; contrast
  of every new pairing checked against the guide's table (`--accent` and `--target` on `--surface-0`
  are already listed).

## Generator version

This plan generates nothing and does not touch `GENERATOR_VERSION`. It displays the version a
universe was created with, in the `UNIVERSE` and `PARAMETERS` panels. It reserves nothing in the
generator. On the client it reserves: the `SymbolShape` values `diamond`, `square` and `triangle`,
drawn here and given their meaning by plan 06 (white dwarf, black hole, neutron star), with the
union open to the values later plans add, one meaning each: `ringed-circle` (plan 06, evolved
stars), `transient` (plan 12, an alert's contact), `triangle-down` (plan 13, a planet, bound or
free-floating), `pentagon` and `hexagon` (plan 14, a moon and an unresolved body);
`SpatialScene.destinationId` for the drive, `MapPopulation` as an open union for plan 07's dust, and
the `DisplayId` union for plan 14's `SYSTEM` display.

## Risks and open points

- **The split with plan 04.** The roadmap's first sketch put decoding of the quantised base64 map
  under this plan; plan 04 provides `decodeDensityMap`, so this plan only maps codes to the ramp and
  pixels to light-years (T8.a, T8.b). Plan 04 also owns the wiring of the request client into
  `connection.ts` (P04.T5), so T5.a is only the context. Universes have a name as well as a seed
  (plan 04, note 16), so the creation form has a `NAME` field, and a random seed is drawn by the
  server, not the client. No other deviation.
- **Guide additions that need the owner's confirmation.** The Decisions entry lists the raster
  field, the direction names, `M☉`, `Myr`, `Gyr` and the drawn `☉`. M1 needs four more, each marked
  in T2: `yr` and the `UT` time system (the chart time; `UT` sits beside `UTC` in the header and may
  be mistaken for Universal Time), E notation (a 10¹⁰ M☉ parameter and the legend's decades, with no
  superscripts in B612), and the 3D conventions with a nomenclature list (the fill rule "on this
  kind of display", and the guide's own demand for a list of abbreviations). Dropped as not needed
  for M1: `dex` (nothing logarithmic is displayed once plan 04 sends no metallicity) and a guide
  rule for function-key display tabs (D2 stands on the guide's existing single-key rule).
- **Drive range without a drive (D9).** The brainstorm requires the range circle and reachability in
  the chart, and also defers the drive. An operator-set value labelled `SET` is the reading that
  keeps both. It sits close to the guide's "never show a number the simulation does not have", which
  is why D9 keeps it in an editable field, repeats `SET` wherever the value is drawn, and speaks of
  range and never of reach. **Needs the owner's confirmation** all the same. For the same reason the
  `--target` destination is built and tested in the view but never set by the M1 display.
- **Edge-on picking.** The brainstorm says the edge-on view sets z. A click there also has an x. D19
  follows the text; setting x as well would be a one-line change if the owner prefers it.
- **Map resolution against chart scale.** A map pixel is 256 ly (128 ly at plan 04's finest), and a
  chart can be 0.01 ly across. The brainstorm's "the face-on view sets x and y" is therefore a
  coarse pick, refined by typed coordinates (T8.g). A zoomable map waits for plan 04's reserved
  `region` field.
- **"Windowed once it passes a couple of thousand rows"** is read as a performance requirement, met
  by always windowing (D17).
- **"A stated floor".** Plan 04 places the floor 5 or 7 dex under the ceiling, a decade more than
  the spans the brainstorm quotes, so the two views never share a legend. Each view states its own.
- **Units the guide does not allow** never reach the client: plan 04's note 11 keeps the kiloparsec
  off the wire. The brainstorm's own prose uses kpc freely, which is fine for a design document and
  is not a display.
- **Interface scale against the narrow window.** The four columns of T6.d have minimum widths that
  sum to 69 rem, which is 1,104 px at 100% and 1,656 px at 150%. So 1280 × 720 holds the display up
  to about 110% and 1920 × 1080 up to 150%. The guide asks for 80–150% "without breaking" and for
  usability at 1280 × 720, and does not say both at once; T12.b records what happens at 1280 × 720
  and 150%, and if the owner wants that combination the list and readout column must fold under the
  chart.
- **Client timeouts (D4)** are an addition to plan 04's timer-free client, required by the guide's
  closed-loop rule. A 1,024-pixel map is never requested, so 120 s is generous; if maps queue behind
  bulk work for longer in practice, raise it, do not remove it.
- **`Activity` (D1)** is recent React. If it misbehaves under the test environment the fallback is
  the `hidden` attribute on a wrapper with effects left running, which costs a few idle timers.
- **jsdom has no layout, canvas, `ResizeObserver` or `matchMedia`.** Every one is faked in `test/`,
  so geometry that depends on real layout (label collisions, the 1280 × 720 fit, real pointer
  tolerance in rem) is only checked by eye in T12.b.
- **Near the galactic axis** the named directions turn across the chart, as the brainstorm notes.
  The grid aligns to the directions at the centre only, and the triad says so by construction. The
  D11 fallback covers the exact axis.
- **`FakeWebSocket` is edited by plans 04 and 05.** T1.b builds on P04.T5's methods and must land
  after it.
- **As built (T3.a): on-screen numbers follow the guide, not this plan's examples.** Digits group
  from five, so four-digit values read `12-24 of 1612`, `2981-3000 of 3000`, `SYSTEMS 1612`; and a
  negative number takes the guide's `-` from `lib/format.ts`, so the range messages of T8.g and
  T11.c read `-65,536 TO 65,536 ly` and `-1000 TO 1000 yr`.
- **Deviations in T1, T5–T7, as built.** `useServerRequest(body, timeoutMs?, generation?)`: a change of `generation` sends an unchanged body again (the universe list after a create and on each reconnect, `RETRY`); bodies are compared by value without `JSON.stringify`. Any settled state of the current body and generation is its answer: a rejection or a timeout stays until `generation` changes, so hiding and showing a display sends nothing, and only a read lost with the link is sent again when it returns. An `ok` answer outlives a link loss and is not fetched again on reconnect (D4: data shown stays); a refresh in flight, or one cut off by the link (and sent again when it returns), keeps the last `ok` answer on show and shows no `PENDING`, since `RequestState` has no "ok, refreshing". States are shared objects, so a memoised consumer does not re-render. The outcome-to-state mapping and the timeout are an exported `followRequest`, used by the universe commands too; `REQUEST_TIMEOUT_MS`, `PENDING` and `SettledRequestState` are exported; the exhaustive `ErrorCode` switch is `settledState` in `lib/useServerRequest.ts` (P06.T35.a looks for it in `RequestStatus`). `useServerLinkValue(connection)` builds the value `App` provides, and `GalaxyDisplay` is `memo`-wrapped so that a pong does not re-render it; the pong test runs under `test/ServerLinkHarness.tsx`, which provides the link as `App` does, and the test under `App` sends the list request and finds its answer on `GALAXY`. `LINK_STATUS_LABEL` lives in `lib/serverLink.ts`, which `LinkStatus` imports, so that `lib/` imports no component. `lib/universe.ts` adds `useUniverseSession()`, provided by `components/UniverseProvider.tsx`; the session's actions are function-typed properties (oxlint `unbound-method`); a command lost with the link reads `NO CARRIER` until the link returns; a create that times out refreshes the list, since the server may have made the universe. The guide won over the plan's strings and colours: link states read in plain text, and `UNIVERSE` states the link's reason once, in a `p`, not through `RequestStatus`, since the header annunciates the link; the guide keeps yellow for alerts, limits and failed systems, so `--status-caution` marks only a timeout and a rejection by a failed or overloaded server, while a refusal (`name_taken`, `unknown_universe`, `generator_version_mismatch`, `bad_request`) and an invalid entry read in plain text, all in words and followed by `RETRY` where the caller can send again (`RequestStatus` gains `id` and `onRetry`, and display-only buttons a `.control` style); the action part of a message is mixed case (`NAME INVALID: enter 1 to 48 printable characters`, which covers the control characters the server refuses, `NO UNIVERSES: create one below`, `GENERATOR VERSION 1: server runs version 2`); and `HEX`, not on the nomenclature list, is spelled out (`1-16 HEXADECIMAL DIGITS`, and `DRAWN BY SERVER` while `RANDOM` is chosen). The disabled seed field keeps its `--line-strong` outline (3:1) and reads as inert by its surface and text. `OPEN` and `CREATE` are held back with `aria-disabled`, keeping focus and their outline, while the link is down (described by the visible `NO CARRIER`) or a command is pending (described by its `PENDING`), so a second command cannot supersede the first; the `OPEN` given reads `PENDING` itself, reaching left over its row's name while it does; a mismatched row's reason shows on a line below the list while its `OPEN` is pointed at or focused. The open universe's row carries an `--accent` rule down its edge and `OPEN` as plain text with `aria-current`, so the state cannot be taken for a `.control` such as `RETRY`. The form is `NewUniverseForm.tsx`, each hint on its field's label line, moving whole to a line of its own where both do not fit. **`NEW UNIVERSE` folds** (the orchestrator's ruling for the owner, replacing this bullet's open point): its legend is a display control (`.control`, `aria-expanded`, `aria-controls`, keyboard-operable, named `NEW UNIVERSE`) led by a drawn chevron, `components/DisclosureGlyph.tsx`, that points right while folded and down while shown, so the state is a shape and not a colour; the fields are shown while no universe is open and folded once one is, the operator's choice holds until another universe opens, folded fields keep what was typed, and a fold that takes the focus with it (a create that opens its universe) moves the focus to the control. The guide's Layout section prescribes nothing else for this case. To fit, the universe table's header is 2.75rem, its action column 5rem, the seed options' spacing 0.1em, the first column's rows `minmax(0, auto) minmax(4.5rem, 1fr)`, and the universe list gives way down to its header and half a row (4.5rem) rather than let the panel clip `CREATE`. Measured in headless Chromium with the bundled B612 at 100% (harness outside the tree): at a 1280 × 720 viewport the first column is 604 px; with a universe open `UNIVERSE` takes 280 px and `PARAMETERS` 308 px, its list 186 px (`1-3 of 78`); with none open `UNIVERSE` takes 516 px and `PARAMETERS` 72 px, its title and `NO UNIVERSE OPEN` whole. At 1280 × 688 (a 720 px window with a title bar, a 572 px column) `PARAMETERS` gets 276 px (list 154 px) with a universe open, and with none open the universe list shrinks to 72 px and `UNIVERSE` loses 4 px of its bottom padding; at 1920 × 1080 `PARAMETERS` gets 556 px (`1-9 of 78`); no page scrolls in any case. At 150% on 1920 × 1080 the short-window cap does not apply (media queries count `rem` at 16 px), so the universe list keeps three rows and `PARAMETERS` shows one row at a time. Universe and parameter rows are two fixed-height lines, since a 16 rem column cannot hold a name, a 16-digit seed, the version and `OPEN` on one line; the readout puts `NAME` and `GEN VER` on one line. `useScrollMetrics` measures the lists for `windowRange`, through a `ResizeObserver` as well as scroll and window resizes; `test/FakeResizeObserver.ts`, listed for T8.d and T10.a, is added now and stubbed for every test in `test/setup.ts`, and passes no entries to its callback, which a later user that reads `contentRect` must add. The parameters list takes focus so that it can be scrolled from the keyboard. **Parameter keys** are now fixed in P04.T14.b, which lists every key group by group (a table added there, with the exclusions); the client's glossary and a new fixture, `everyGalaxyParameter`, copy it, and `parameterLabels.test.ts` holds the glossary to that list both ways. The client's proposal matched plan 04's group names and its one example key (`disc.thin.scale_length`); P04.T14.b's "every structural getter appears once" adds the population masses and mean system masses (two groups, `population_masses` and `population_mean_masses`), `dark_halo.f_star`, `halo.lesser.share` (so that the halo's shares sum to 1) and the halo components' slopes, cores and flattenings, and drops the gas disc's fixed height, a generator constant. `rotation.radius` stays: the circular speed's radius is a parameter, not part of a label. The seed and generator version are shown above the groups and skipped within them. Readings choose a precision per unit (masses under 10⁴ as elsewhere, times and dimensionless values to three significant figures, speeds to one decimal, densities in E notation); `8480.4 ly` is ungrouped (T3.a note). Labels have three words or fewer, the guide's limit for upper case, with the group heading naming the component (`THIN SCALE LENGTH` under `DISCS`, not the plan's `THIN DISC SCALE LENGTH`; a population's name under `POPULATION MASSES`); `HALO` is the stellar halo as the population is named, the dark halo always `DARK HALO`, and the halo's dominant component the `MAJOR MERGER`, as `LAST MAJOR MERGER` names its time. At 1280 × 720 no label is cut; at the column's 16rem minimum three end in an ellipsis. `GALAXY MAP` and `LOCAL CHART` are title-only panels until T8.e and T11.g, and `LOCAL CHART` spans the last two columns; the work area takes a `console__work--<display>` modifier. `FakeWebSocket` also gains `serverWelcomes()` and `cancelledIds()`, `serverAnswers` returns the ID it answered, and the fixtures gain `aCreatedUniverse`, `anOpenedUniverse`, `aRangeRequest` and `everyGalaxyParameter`. One T9 test dropped `JSON.stringify` for T5.a's grep.
- **Deviations in T8.d–g, as built.** _T8.d:_ `GalaxyMapView` is a keyed shell over its body, so that `RETRY`, after a failed request or invalid data, mounts the body afresh and asks again from `PENDING` (a `generation` bump would keep an invalid `ok` answer on show until its successor). `MAP DATA INVALID` gives its cause in words, in `--status-caution` as a server fault, followed by `RETRY`: `not the map requested` (the answer's universe, view or population is not the request's), `size or scale unusable` (including a map that is not plan 04's M1 extent, square face-on, 2:1 edge-on and 131,072 ly across, since both pictures are drawn at one scale), `density range unusable` (a ceiling below the floor; plan 04's empty map, with the two equal, is drawn) and `pixel codes unreadable` (`decodeDensityMap` throws). The request status, `MAP DATA INVALID` and the create form's `CREATE UNCONFIRMED` were three hand-built copies of one line, and are one component, `components/StatusLine.tsx` (words in an `output`, caution for a fault, one `.control` after it), which `RequestStatus` uses too. **Resolution and reduction, the orchestrator's ruling:** D5's fixed 512 is replaced by the narrowest map the protocol offers (128, 256, 512 or 1,024 pixels, plan 04 design note 12 and P04.T14.c) that is at least as wide as the picture in device pixels, asked for again, from `PENDING`, when the picture grows or shrinks past one; where the map still has more pixels than the backing store it is reduced to it by area-weighted averaging of linear density, a mean below the floor shown as background, never by dropping pixels or averaging codes; otherwise its own pixels are drawn larger without smoothing (D6), which repeats pixels and drops none (`lib/galaxy/mapPicture.ts`: `MAP_RESOLUTIONS_PX`, `mapResolutionFor`, `turnClockwise`, `reducedLevels`, `paintLevels`, tested with a one-pixel disc row of a 512 map drawn 244 device pixels wide, which nearest-pixel drawing skips). The mean of linear density over a device pixel is that area's column density, so the guide's "no smoothing that invents values" holds. `GalaxyMapView` takes the pictures' CSS width and the device pixel ratio as props, `pictureWidthPx` and `devicePixelRatio`; the ramp is applied after the turn and after the reduction, so P07.T11.b's overlay, which lowers each pixel's log density by 0.4 A_V, applies per map pixel before `reducedLevels`, and its extinction raster takes `mapResolutionFor` and the same turn. Pictures wider than 512 device pixels (562 at 1920 × 1080) now ask for 1,024-pixel maps, which the "Client timeouts" entry above assumed never happen: four times the compute of plan 04's 512-pixel targets, some 4 s face-on and 12 s edge-on on eight workers, and 1.4 MB of base64 face-on; the 120 s timeout stands. **Orientation, checked against plan 01's axes and the spatial view:** the face-on raster is drawn a quarter-turn clockwise, +x down and +y to the right, as `TOP` shows a chart centred on the +x axis (`camera.test.ts`, "on the galactic axes"), and the edge-on raster as sent, x across and z up, as `FRONT` shows it there; the turn keeps the counter-clockwise rotation. T8.d's axis labels are therefore `+Y` to the right of the face-on picture and `+X` beside its bottom end, and `+X` to the right of the edge-on picture and `+Z NORTH` beside its top end; the two views no longer share their horizontal axis, but both are 131,072 ly across at one scale. The ramp's tokens are read in a layout effect when the view mounts and each time `Activity` shows it again. The frame (`FRAME GALACTIC`) and a local `MapScaleBar` over `scaleBar`, at most a quarter of the pictures' width, are shown once for both views, which share one scale, not once per view; T10.g replaces the bar with `ScaleBar` and takes the `.scale-bar` block over. `test/RecordingContext2D.ts` arrived early, with T8.d, built to T10.a's text, so T10.a adds only `spatial/paint.ts` and its test: a recorder that makes one proxy context per canvas (`contextFor(canvas)`, or `contextFor()` for none) and records every method call and property set in one ordered `records` list, each naming its canvas, with `calls(name)`, `sets(name)`, `names()` and `clear()`; `stubCanvas()` returns it and gives each canvas its own context. jsdom applies no stylesheet, so `test/colourTokens.ts` copies the colour tokens and `test/setup.ts` sets them before every test; a token changed in the guide and `styles.css` is changed there too. The density under the cursor is the value its code stands for, the centre of the code's step, since plan 04's quantiser rounds to the nearest code; the guide keeps `~` for estimated values, derived or sensor-limited, and is silent on a value rounded for the wire, so it carries no `~` (the orchestrator's ruling, recorded). The two legends stay: the views do not share a ramp, their floors lying 5 and 7 dex under ceilings that are each map's own maximum. _T8.e, and the orchestrator's ruling that the map fit 1280 × 720:_ measured in headless Chromium with the bundled B612 (the display's DOM from jsdom under the real stylesheet; harness outside the tree), T8.g as first built left the pictures 244 px at 1920 × 1080, none at 1280 × 720, and the panel clipped at 1280 × 688. All three of the ruling's measures were needed, since folding `UNIVERSE` and moving the cursor readout leave the map an 18–26rem column, 200 px of picture at most at 1280 px. (1) `UNIVERSE` folds to one line once a universe is open: its `h2` holds a display control (`.control`, `aria-expanded`, `aria-controls`, `DisclosureGlyph`), and folded it shows `NAME`, `SEED` and `GEN VER` beside it; it is whole while no universe is open and while a create is unconfirmed, the operator's choice holds until another universe opens, and a fold that takes the focus, or loses it as `OPEN` gives way to the word, moves it to the control. Shown whole, `UNIVERSE` takes the column and the pages are hidden until it folds, since the map's words need some 29rem of height that the whole panel would take at 1280 × 688; so with no universe open the pages are not shown, and only `LOCAL CHART` says `NO UNIVERSE OPEN`. (2) The cursor readout is the `CURSOR` panel at the head of the second column, beside the pictures and next after the map in reading order. (3) `PARAMETERS` and `GALAXY MAP` share one panel as two pages, `GalaxyPages.tsx`: a tab list (`role="tablist"`, one tab stop, the arrow keys, `Home` and `End` choosing, the focus following) that is the panel's title, the chosen page's name `--accent` over an `--accent` rule; the map shows by default and again once another universe opens; both pages stay mounted, the one not chosen hidden, so neither asks the server again, and the parameters are still fetched with the universe. `ParametersPanel` and `GalaxyMapPanel` are those pages, without panel chrome or titles of their own. On top of the measures: T6.d's four columns become two, `UNIVERSE` above the pages (at least 52rem) and `CURSOR` above `LOCAL CHART` (at least 20rem), sharing wider windows 4:3, so the second column is 25rem at 1280 px and T11 must fold its list and readout under its chart there; and each view's words (title, key hint, rotation sense, density under the cursor, legend) stand in a 24rem column beside its picture, with `POPULATION`, the frame and the scale above the face-on view's words, the face-on picture reaching the top of the page beside the tab list. `GalaxyMapPanel` measures the page once (`lib/useElementSize.ts`, which keeps its last size while the element is not laid out, so a hidden page keeps its maps) and gives both pictures one even width, `--map-picture`, face-on square and edge-on half as tall; the backing store is that times the device pixel ratio. Galaxy panels take 0.5rem of vertical padding, and the folded `UNIVERSE` 0.25rem. Measured the same way: at 1280 × 688 the pictures are 320 × 320 and 320 × 160, with 44 px and 32 px to spare beside them (18 px with the link down), and nothing clips or scrolls but the lists; at 1280 × 720 326 × 326 and 326 × 163, the width the column leaves; at 1920 × 1080 562 × 562 and 562 × 281; at 150% on 1920 × 1080 490 × 490 and 490 × 245 CSS px. The two columns need 73rem, 1,168 px at 100% and 1,752 px at 150%. The population's options are `ALL` and `YOUNG THIN DISC`, not `ALL SYSTEMS` and `YOUNG`: the young map counts the young thin disc alone, and the guide gives a thing one name, `populationLabel`'s; T12.a and Verification step 6 choose these names. While the link is down `POPULATION` is held back with `aria-disabled`, described by the link's reason shown above it, and the maps on show stay (D4). `DensityLegend` (T8.c) measures its bar and, where a label at every decade would run together, labels every second or third decade at longer ticks; unmeasured, as in jsdom, it labels every decade. _T8.f:_ the cursor's pure rules are `displays/galaxy/mapCursor.ts` (`cursorInView`, `withinExtent`, `pickCursor`, `stepCursor`, `markOnPicture`, `pixelIndexAt`, `ARROW_KEYS`, `LARGE_STEP_PX`, and for the turn `SCREEN_TURN`, `SCREEN_ASPECT`, `MAP_ACROSS_LY`, `screenSizePx`, `rasterToScreen`, `screenToRaster`, `rasterDirection`), which work on the raster as sent. The cursor is state of `GalaxyDisplay`, not of the panel, since the map and `CURSOR` are now siblings; it starts at the galactic centre, so `C` works before any pick, and is kept across a change of population or universe. A click takes the fractional point under the pointer, turned back to the raster; the arrow keys move one pixel of the map fetched, 128 to 1,024 ly with the picture's size (the edge clamp likewise), as the screen shows the axes, face-on the right arrow +y and the down arrow +x, so T8.f's "`{ArrowRight}` raises X" is `{ArrowDown}`; a pick or a step keeps the cursor between the centres of the map's edge pixels, since the face-on map's edges are the root cube's faces and its upper faces lie outside it. Edge-on the arrow keys, like a click, move only z (D19), so its hint reads `↑ ↓ MOVE CURSOR` and the face-on hint `ARROWS MOVE CURSOR`; each describes its canvas. An arrow with Alt, Ctrl or Meta is ignored. The canvas's `role="application"` needs an oxlint suppression. The cross is an inline SVG named `Cursor`, 1.5 px `--accent` cased in 3.5 px of `--surface-0`, since `--accent` falls to 1.3:1 over the top of the ramp; a point above or below the edge-on map is pegged to its edge with `↑` or `↓` and named `Cursor, off the map above`. The density under the cursor is read among each view's words: `CURSOR DENSITY 3.16E0 SYSTEMS/ly²` in a fixed 8ch slot, `BELOW FLOOR` at code 0, or an em dash off the map. `RADIUS`, `ANGLE` and `HEIGHT` read to one decimal in 9ch slots, `ANGLE` an em dash on the axis (D11). Each density line and a visually hidden summary of the whole position are polite, atomic live regions. Moving the cursor paints nothing. _T8.g:_ `CentreEntry` is the `CURSOR` panel, a `section.panel` titled `Cursor`: the hint `±65,536 ly`, `C CENTRE CHART`, the fields `X`, `Y` and `Z`, which replace T8.f's X, Y and Z readings, and `RADIUS`, `ANGLE` and `HEIGHT`. Props: `cursorLy`, `onCursor`, `onCentre`, `heldBack` (why the chart cannot be centred, such as `NO CARRIER`, shown in the panel). A typed value is entered when its field is left or on `Enter`, not at each keystroke, so the cursor never passes through the digits on the way; the plan's "typing `26000` into `X` moves the crosshair" is typing then `Tab`, tested in `GalaxyDisplay.test.tsx`. An entered value is rounded to four decimals before it is checked, and shown with the decimals it was typed with, up to four, in a field wide enough for `-65,535.9999`, with `ly` beside each field; grouping commas and either minus are accepted. The root cube is half-open, −65,536 ly up to 65,536 ly with that face excluded (plan 01's `in_root_cube`, which takes cells in `-65536..65536`), so 65,536 is refused. A refused value stays in its field with `aria-invalid`, keeps the cursor, and is named in plain text, the create form's pattern, not the plan's `OUT OF RANGE: −65,536 TO 65,536 ly` (nor the T3.a note's reading of it): `X INVALID: enter -65,536 to 65,535.9999 ly`, or `X, Z INVALID: …`; a move of the cursor along that axis drops it. `Enter` only enters; `CENTRE CHART` enters what is typed and then publishes the cursor through `onCentre`, and `C`, which is never pressed in a field, publishes the cursor, each field having entered its value as the focus left it; both are held back, `aria-disabled` and described by the reason, while an entry is refused or the link is down; `.control` gains a held-back style. `C` is a `document` listener, live while `Activity` shows `GALAXY` with a universe open, and tested to do nothing while `LINK` is shown (`App.test.tsx`); it ignores repeats, `Shift`, `Ctrl`, `Alt`, `Meta` and presses in a field that takes text. T12.a's and Verification step 7's "enter `12` in `Z`, press `c`" is `12`, `Tab`, then `c`. The chart centre is state of `GalaxyDisplay`, `null` until the first `C`, and kept across a change of universe, as the cursor is, since a point of the `GALACTIC` frame is a point in any universe's galaxy; T11.g's `LocalChartPanel` takes it as `centreLy` and compares it by value; `GalaxyMapPanel` and `GalaxyMapView` take `centreLy`. The reticle is four corner brackets named `Chart centre`, 1.5 px `--text` cased like the cursor.
- **Orchestrator's ruling for the owner, applied with T8.g (T6.a, T6.c).** A create lost with the link used to read nothing once the link returned. The server may have made the universe, so it now reads `CREATE UNCONFIRMED: link lost before reply; check the universe list` (the condition, the cause, then the action) in `--status-caution` in the create form's status area, followed by `DISMISS` (a `.control`, after which the focus goes to `CREATE`), until it is dismissed, the next create is given, or a universe is opened from the list; while it stands, `UNIVERSE` and its `NEW UNIVERSE` fields stay shown, so that it is seen, and both their controls are held back with `aria-disabled`, described by the report, which has an ID of its own. The list is refreshed on reconnect as before, and an open lost with the link still reads nothing. `UniverseSession` gains `createUnconfirmed` and `dismissUnconfirmed`, and tracks which command is in flight. This widens `--status-caution` beyond a timeout and a failed or overloaded server.
- **Deviations in T10, as built.** _T10.a:_ `paint(context, drawList, tokens, pixelRatio)` takes the device pixel ratio as a required fourth argument, so that a caller cannot leave a high-density canvas drawn in its top left-hand corner: it clears the whole backing store (`canvas.width` × `canvas.height`) to `--surface-0` with `fillRect` under an identity transform, then draws the ops, which are in CSS pixels, under `setTransform(pixelRatio, 0, 0, pixelRatio, 0, 0)`; `SpatialView` sets no transform of its own. `ColourTokens` is `Record<ColourToken, string>` (the four names an op may give) plus `surface0`; `readTokens` trims each value and throws an `Error` naming a token that is not set, since a missing stylesheet is a bug, not a state. A reticle is one path of four corner sub-paths, each arm a third of the square's side, stroked once. The members the test bans are wider than the task's four (text, alpha, compositing, `filter`, the shadow properties, every gradient and `createPattern`) and are listed as `BANNED_SPATIAL_MEMBERS` in `test/RecordingContext2D.ts`, so that no source under `spatial/` names them; the acceptance grep, which as written reads a directory and prints nothing whatever the sources hold, is run as `grep -rn "fillText\|globalAlpha" apps/hyperion/src/renderer/src/spatial` and is empty. A context the recorder makes for no canvas has a `null` canvas, which `paint` cannot size, so tests paint on a stubbed canvas's context. Polygon symbols are stroked with mitred joins, whose points reach up to 0.75 px past the diameter; plan 06, which first draws them, may round the joins.
  _T10.b:_ The viewport comes from `lib/useElementSize.ts` on the stage `div` (a `ResizeObserver` and window resizes, `getBoundingClientRect`, the device pixel ratio and the root font size; a hidden view keeps its last size), not from an observer of its own; `test/FakeResizeObserver.ts` is T6's, unchanged. The camera state is the angles and `pxPerUnit: number | null`, `null` while the view fits; the `Camera` is derived during render at `fitPxPerUnit(fitRadius, viewport, 2 rem)`, the margin leaving room for the data edge's ticks and a curve label, so the default view keeps fitting through resizes and new radii until the operator zooms, and `Z` (T10.c) returns it to `null`. A chosen zoom keeps its pixels per unit and is clamped to `zoomLimits` of the current fit whenever the camera is derived. This is a reading of the brainstorm's "zoom is separate from both radii and defaults to fitting the query sphere", which the plan's own clamp already ties to the query radius at the extremes; the alternative, holding the scale on a new radius even before the first zoom, would draw a 0.01 ly sphere at half the fitted size after a step down from 50 ly. Kept by the orchestrator's ruling (7) below. The draw list is built during render (`useMemo` on scene, camera and viewport, for its identity), not in the layout effect, so that T10.h's labels and T10.i's anchors read the list painted; the layout effect only paints, on a new draw list, token set or pixel ratio, one of which comes with every new backing size. The tokens are read in a layout effect on mount and each time `Activity` shows the view again; `paint.ts` gains `sameTokens`, so an unchanged set keeps its identity and paints nothing. The key legend is a `p` above the stage, one inline `span` per key in `--text-muted`, the spaces between them real text; since ruling (6) below it reads `ARROWS ROTATE +/− ZOOM Z FIT`. The overlay takes no pointer, so the canvas gets every drag and pick, and holds no controls: T10.e's preset buttons sit outside it. Nothing requests a frame before T10.c's keys, so the view has no scheduler until then, and the test "unmount … cancels a pending frame" is T10.c's.
  _T10.c:_ The camera moves through `spatial/useOrbitCamera.ts`: `useOrbitCamera(fittedPxPerUnit)` returns the state and `move(CameraMove)`, a move being `turn` (degrees of azimuth and elevation), `zoom` (a factor) or `fit`. Moves are gathered in a ref and folded in order on the next frame, through `createRedrawScheduler`, into one `setCamera`, so key repeats and T10.d's pointer moves cost one render a frame and the result does not depend on how the moves fall into frames (13 presses up and one down end at +85° either way); a camera the moves leave as it was, as against a limit, keeps its identity and paints nothing. Each move uses T9's `rotateCamera` and `zoomCamera` on the scale shown, the chosen one brought within the current fit's limits first, so a zoom after a change of radius starts from what is on screen. The scheduler is made in an effect and disposed on unmount and when `Activity` hides the view, which cancels a pending frame and drops the gathered moves; T10.b's "unmount … cancels a pending frame" is tested here. The arrow keys turn the camera only on the focused canvas (`preventDefault` there alone; ignored with Ctrl, Alt or Meta). The zoom keys and `Z` follow D3 rather than the task's "on the focused canvas": a `document` listener, live while `Activity` shows the view, acting from any focus but a field that takes text (`isTextEntry`, moved from `CentreEntry.tsx` to `lib/textEntry.ts`) and ignoring Ctrl, Alt and Meta, so that Ctrl with `=` or `-` stays the interface scale's; Shift is allowed with `+`, which some layouts type with it, and ignored with `Z`, as `C` ignores it. The readouts do not exist yet, so the tests check the painted canvas: the mark's projected position (35° after `{ArrowRight}`, 29° with Shift, 85° after the clamp), the query edge's radius (× 1.25, the 100× and 0.5× clamps, a chosen zoom kept when the fit radius changes, `Z`, a fit within one frame), a text field, and a button beside the view; T10.f adds the plan's `030°` → `035°` readout test and T10.g the scale bar's label under `+`. Frames are faked with `vi.useFakeTimers({ toFake: ["requestAnimationFrame", "cancelAnimationFrame"] })` and `vi.advanceTimersToNextFrame()`, since `user-event` waits on real timeouts that a fully faked clock never fires. The arrows now turn the view in one sense, the drag's (ruling (4) below): right raises the azimuth and down raises the camera. The legend does not show Shift's 1° step, since the plan fixes its words.
  _T10.d:_ Pointer input is `spatial/usePointerOrbit.ts`: `usePointerOrbit(canvasRef, remPx, move, onClick)` returns the canvas's React pointer handlers and feeds T10.c's `move`, so a drag, the wheel and a pinch cost one render a frame; `DRAG_DEG_PER_REM` and `CLICK_SLOP_REM` are exported. Only the primary button (a mouse's left, a finger, a pen's tip) starts a gesture, and its pointer is captured until `pointerup`, `pointercancel` or a lost capture, each of which ends that pointer's part of the gesture, the last two without a click, so that a missed up never leaves a pointer behind to turn the next drag into a pinch. A gesture is a click until a move takes it 0.25 rem from where it went down (a pointer that strays and comes back is a drag), reported at the down point; a drag counts from the down point, so a 32 px drag at 16 px/rem turns 16°. Right raises the azimuth, as `{ArrowRight}` does, and down raises the camera. A second pointer makes the gesture a pinch, never a click: its moves zoom by the ratio of the first two pointers' separations and do not turn. The wheel's `deltaY` is read in CSS pixels (a line 16 px, a page the canvas's height) and zooms by 2^(−px ÷ 400); its listener prevents the default of every wheel event on the canvas, Ctrl held included, so Ctrl with the wheel (and Chromium's trackpad pinch, sent as one) zooms the chart at that rate, not the interface. jsdom has no pointer capture, so `test/pointerCapture.ts` gives `Element.prototype` recording `setPointerCapture`, `releasePointerCapture` and `hasPointerCapture`, installed before every test by `test/setup.ts`; it retargets no event, so tests assert what was captured. Drags, clicks, the secondary button and pinches are driven with `user-event` (`MouseLeft`, `MouseRight`, `TouchA`, `TouchB`); `pointercancel`, a lost capture and the wheel, which it cannot produce, with `fireEvent`. With no scale bar before T10.g, the wheel tests check the painted query edge (× 2 for −400 px, × ½ for 25 lines or two 200 px pages) and T10.g adds the plan's scale-bar check. The click already calls `pick` at 1 rem and then `onSelect` (T10.i's wiring, landed here): the tests that a 2 px click on a mark selects it and that a drag, a pinch or a cancel ending there does not are here, the tolerance tests T10.i's. 0.25 rem is 4 px at 100%, under a finger's usual wobble on a tap, so a finger now has a slop of its own (ruling (5) below).
  _T10.e:_ `PresetButtons` is a `fieldset` named `Views` of `.control` buttons, each showing its key before its word as `C CENTRE CHART` does, so named `T TOP`, `S SIDE`, `F FRONT` and `O OBLIQUE`, with `aria-keyshortcuts`; the pressed one has a 2 px `--accent` rule under it as well as `aria-pressed`. `PRESET_CONTROLS` (`PresetControl`) gives the view's key listener its keys. The buttons share a row with the key legend above the stage, outside the overlay, and come before the canvas in the Tab order. The preset keys follow D3 as T10.c's do: the same `document` listener, ignored in a text field, with any modifier (Shift too) and on a key repeat. `useOrbitCamera` gains `turnTo(angles, instantly)`: it keeps the zoom (a fitting view keeps fitting), drops moves gathered and not yet applied, asks for nothing when the camera already looks that way, and otherwise turns the angles alone with `tweenCamera` and `easeOut`, a scheduler frame at a time from the angles on screen (kept in a ref, so a second preset carries on from where the first turn got to), lands exactly on the preset and then asks for no frame. The first frame starts the clock, so a late first frame skips none of the turn; at 16 ms frames the turn lands on the ninth frame, 144 ms after the choice, after eight paints, which the test asserts in place of "after 120 ms"; at 60 Hz that is within the guide's 150 ms, and starting the clock at the choice would save a frame. Any move (a turn, a zoom, `Z`, a drag, the wheel, a pinch) stops a turn where it stands, as the task says of "an operator input"; a zoom could instead let the turn carry on, the owner's call. Under reduced motion the camera is set in the handler with no frame: one render, one paint. `lib/usePrefersReducedMotion.ts` (`usePrefersReducedMotion`, `REDUCED_MOTION_QUERY`) reads the setting through `useSyncExternalStore` and follows its `change` events; it lives in `lib/`, having nothing spatial in it. `test/stubMatchMedia.ts`: `stubMatchMedia(reduced)` returns `set(reduced)`, which changes the setting and tells the listeners; `test/setup.ts` installs `stubMatchMedia(false)` before every test, so a test that chooses a preset (T11, T12.a's `t`) runs frames or stubs reduced motion first. The readouts do not exist yet, so the tests check the painted mark; T10.f adds `AZM 000°` and `ELV +90°` after `TOP`. A view hidden mid-turn is shown again where the turn left it. Any operator input stopping a turn is kept by ruling (7) below.
  _T10.f:_ `AxisTriad` (`frame`, `angles`) is a `div` with `role="img"` named `Axis triad`, sized `TRIAD_BOX_REM` (15 × 8 rem) in the stage's bottom left-hand corner, holding an `aria-hidden` SVG of the axes (1.75 rem seen side on) and the three labels as DOM `span`s (0.875 rem, 0.1 em spacing, `--text-muted`). An axis leaning away from the viewer ends in the open circle with a cross, one leaning towards the viewer in the circle with a dot, and one exactly in the screen (depth within 10⁻⁹, as spinward and north are at `SIDE`) in an arrowhead, a third ending the task does not name. Layout is pure, in the new `spatial/furniture.ts`: `triadLayout(frame, angles)` puts each label beyond its axis's end, an axis seen end on (projected under a quarter of its length) has its label beside its symbol away from the other two, and a label that would cover another takes the nearest clear place a line or two up or down or half a label or a label across, else further out along its axis, always inside the box (tested every 15°); `triadFootprintPx` is the part of the view it covers. `CoreArrow` (`layout`, `viewport`, `distance`) draws what `coreArrowLayout(frame, angles, viewport, labelText, obstacles)` decides: a 1.5 rem arrow whose head stands 0.5 rem inside the edge where the projected coreward direction leaves the view, or sooner, where it would run into the triad; within 5° of the line of sight the away or towards symbol, 1.25 rem across so that it is never taken for a mark (at most 1 rem, D14), at the top centre. Its label (`CORE 26,000.0 ly`, the distance in B612 Mono) goes left of an arrow that runs up or down and below one that runs across, else on the other side, beyond the tail, or moved up or down, clear of the triad and inside the view (tested every 15° in a 22.5 rem view, the chart's width at 1280 × 720). The view has no position to measure the distance from, so `SpatialView` takes a required `coreDistance: SpatialQuantity`, which T11.g passes as the centre's `RADIUS` reads it: the plan's `CORE 26,000 ly` beside `RADIUS 26,000.0 ly` would give one quantity two precisions (guide, "Numbers, units and time"). On the galactic axis `coreArrowLayout` gives `undefined`, nothing is drawn, and `AXIS_FALLBACK_MESSAGE` (the D11 words, exported from `CoreArrow.tsx`) stands in the furniture row below the view rather than over the canvas, where it covered the curve labels. The triad and the arrow follow `scene.frame`; plan 14, whose orbit map tilts that frame, will need a way to hand the triad and the arrow the galactic directions (its `axes`). The angles are a `dl` in the controls row, after the preset buttons, built with the new `spatial/Reading.tsx` (`Reading`, `SpatialReading { label, value: string | null, unit, widthCh }`, `SpatialQuantity`): each value right-aligned in a slot `widthCh` wide (4 for `000°` and `+90°`), a missing one an em dash in `--text-muted` with no unit, set with the shared `.field` classes; the canvas's `aria-describedby` names the angles and the legend, so that they are read on focus and never announced. `useThrottledValue(cameraState.angles, 250)` throttles the angles, not the camera, so a zoom starts nothing; it acts on both edges (the first change after a quiet interval shows at once, so the plan's "advance fake timers past the 4 Hz throttle" is not needed for one press), adjusts its state during render, and times each hold with one `setTimeout`, identified so that a hold that follows another at once is timed again. The triad and the arrow move at frame rate. `labels.ts` gains `textSizeRem(text, letterSpacingEm)` and `halfExtentRem`. `test/fakeFramesAndTimeouts.ts` fakes frames and `setTimeout` and stubs a global `jest` with Vitest's clock, since Testing Library's async wrapper waits on a `setTimeout` of 0 and advances only Jest's fake timers; T12.a, whose `t` step turns the camera, must let the 250 ms hold pass before reading `AZM 000°`.
  _T10.g:_ `ScaleBar` (`pxPerUnit`, `maxBarPx`, `formatLength`, `units?`) calls `scaleBar` itself and is a `div` with `role="img"` named `Scale bar, <label>`, drawn 1 px wider than `lengthPx` so that its end ticks' centres are the length apart, at the end of a slot as long as the longest bar, its label in a slot 9 ch wide, so that a new length moves nothing beside it; the `.scale-bar` block moved beside the view's styles. It replaces `GalaxyMapPanel`'s `MapScaleBar`, where it stays a quarter of the pictures' width. In the view it is at most 6 rem and stands in the furniture row below the stage, after `FRAME`, as the map sets its own, not over the canvas: over it, it met the triad and the core arrow on a 22.5 rem view. Since the T9.c checkpoint `scale.ts` has `ScaleUnit { perSceneUnit, minSceneLength }`, `SCENE_UNIT_ONLY` and `scaleBar(pxPerUnit, maxBarPx, units)`; `SpatialView` takes the ladder as the optional `scaleUnits`, and the chart must pass light-years down to `SCALE_AU_BELOW_LY` and then AU, or a bar under 0.01 ly reads, say, `316 AU`. The furniture row is `FRAME` with `frameName`, then `centre: ReadonlyArray<SpatialReading>` and `time: SpatialReading` (T11.g passes `RADIUS`, `ANGLE`, `HEIGHT` with widths 9, 6 and 9, and `UT` with 9), then the scale bar and, on the axis, the D11 message. Zoom stops at 100 times the fit, so the plan's `20 ly` … `500 AU` sequence cannot come from one radius: the test presses `+` 25 times fitted to 50 ly, steps the radius to 0.5 ly as the operator would, keeps the zoom, and presses 25 more (`20 ly` … `0.01 ly`, `500 AU`, `200 AU`); the bar is never wider than 6 rem and its 1 px.
  _T10.h:_ Mark labels are `span`s in an `aria-hidden` layer of the overlay (which takes no pointer), moved with `translate(…rem, …rem)`, B612 at 0.875 rem with no letter spacing, since `placeLabels` estimates their boxes without it; an available mark's label is `--accent`. `placeLabels` gains a last, optional `obstacles: ReadonlyArray<BoxPx>` (`BoxPx` now lives in `labels.ts`): the view places mark labels last and drops one that would cover the triad, the core arrow or a curve label, never the selection's. `CurveLabel` is now a union: a sphere's `CircleLabel` carries its circle (`centre`, `radiusPx`), a ring's `RingLabel` does not. `placeCurveLabels(labels, viewport, obstacles)` (in `furniture.ts`, giving each `PlacedCurveLabel` its box) sets a sphere's labels one above another: above the top of the circle, 0.375 rem clear of its ticks and 0.5 rem right of the top point, which is left to the core arrow, when there is room, else at the first of a round of points on the circle, outside it and then inside it, where they fit; a ring's label below and right of its coreward point, or on another side of it. Every curve label stays inside the view and clear of the triad, the core arrow and the others, and is dropped only when nothing fits, as when the view lies inside its circle; the first placement, at the top only, cut the second label off on a 22.5 rem view and lost both once a zoom took the top out of sight. Curve labels are `--text-muted` (a label, and the range an entered setting, D9), spaced 0.1 em, their numbers in B612 Mono. "Present and differ" is tested as present and placed apart.
  _T10.i:_ The click wiring landed with T10.d. The two coincident marks lie 10 ly apart along the line of sight; reticles are read from the recorded paint as paths of four sub-paths, in the colours the stylesheet gives `--accent` and `--target`. The touch slop of ruling (5) is tested with a 6 px wobble: a tap for a finger, a drag for a mouse.
- **Orchestrator's rulings for the owner, applied with T10.** (1) _Reticle symbols._ One symbol, one meaning: the guide's 3D conventions give the bracket reticle to the selection (and the `--target` reticle to a commanded destination) and assign nothing to a chart centre, so the selection keeps T10.a's four-corner brackets and the chart centre on the galaxy map (T8.g's `Chart centre`) is now a small diagonal cross, `M8 8L16 16M16 8L8 16` in the mark's 24-unit box, 1.5 px `--text` cased like the cursor. It is turned 45° from the cursor's upright crosshair so that it reads apart from it by shape, not colour alone, and its arms lie on the diagonals, clear of the cursor's even where the cursor stands on the centre, as it does after `C`. The guide's own rules do not call for an entry (its symbol rules cover contacts and the 3D reticles), so it is recorded here only. Tested by the shape of the drawn strokes (`GalaxyMapView.test.tsx`). (2) _Map resolution._ `mapResolutionFor` now asks for the narrowest offered map r with r × `MAX_UPSCALE` (1.25) ≥ the picture's device-pixel width, the widest when none is; enlarging repeats pixels and loses none, and reduction stays area-weighted. At 1920 × 1080 the 562 px pictures ask for 512, not 1,024, which restores the "Client timeouts" entry's assumption; 320 px pictures (1280 × 688) ask for 256. Tests updated in `mapPicture.test.ts`, `GalaxyMapView.test.tsx` and `GalaxyMapPanel.test.tsx`. (3) _Resolution changes._ While a map of another resolution for the same universe, view and population is on its way, or failed, `GalaxyMapView` keeps the last picture it drew, resampled to the new size by the same path (drawn larger, or reduced by area-weighted averaging), and marks it stale by the guide's data-state convention: the picture and its legend are painted on a ramp from `--surface-0` up to `--text-muted` instead of `--text`, a trailing `S` stands at the foot of the gutter to the picture's right, the density under the cursor reads in `--text-muted` with a trailing `S` (and "stale" for assistive technology), the canvas is named `Galaxy map, face-on, stale`, and the request's state (`PENDING`, or a failure with `RETRY`) reads beside it among the view's words. The new map replaces it; `RETRY` still mounts the view afresh and asks again from `PENDING`. A map of another universe or population is other data and is waited for from `PENDING`, as before. The age of the stale picture is not shown. Tested in `GalaxyMapView.test.tsx` (seven tests). (4) _Arrow keys._ They turn the view in one sense, the drag's grab: `{ArrowRight}` raises the azimuth as before, `{ArrowDown}` raises the camera and `{ArrowUp}` lowers it, so the plan's "13 presses of `{ArrowUp}` stop at `+90°`" is 13 presses of `{ArrowDown}`. The legend names no sense and is unchanged by it. (5) _Click slop._ The guide has consoles run on touch screens with 2 rem targets, so a finger (`pointerType === "touch"`, as it goes down) may stray `TOUCH_CLICK_SLOP_REM`, 0.5 rem (8 px at 100%, Android's touch slop), and a mouse or a pen 0.25 rem as before; a drag still turns from where the pointer went down, so nothing of it is lost. (6) _Key legend._ It reads `ARROWS ROTATE +/− ZOOM Z FIT`: the preset buttons show their own keys. (7) _Kept as built:_ zoom follows the fit until the operator zooms (T10.b), and any operator input, a zoom included, stops a preset turn in progress (T10.e). (8) _The axis fallback's words._ D11's message was set in capitals, which the guide keeps for annunciations of three words or fewer, and wrote the axis with the typographic minus, where the guide's signed values take `-`. `AXIS_FALLBACK_MESSAGE` now reads `DIRECTIONS UNDEFINED: grid aligned to -X`, the create form's pattern of an annunciated condition and then a sentence in mixed case, and the `.spatial-view__axis-note` loses its letter spacing, as `.form-field__error` has none. The triad's fallback labels are `-X` and `+Y` for the same reason, so that one minus is written everywhere on the console. `AT AXIS` is dropped from D11's words to keep the annunciation within three: the message stands only at the axis, where `ANGLE` already reads an em dash (T8.f) and the guide itself says the named directions are "undefined on the axis". (9) _Kept as built:_ the arrowhead that ends a triad axis lying in the plane of the screen, a third ending D11 and T10.f do not name, without which `SIDE` and `FRONT` would end two axes in nothing. Its strokes, and the core arrow's, are held at 1.5 CSS pixels with `vector-effect: non-scaling-stroke`, since D13 and D14 give sizes in `rem` and line weights in pixels, as the painter does: the triad's user units are a sixteenth of a `rem`, so its axes would otherwise reach 2.25 px at 150% beside a 1.5 px range circle.
