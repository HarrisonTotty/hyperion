# Plan 04: Server, universes and protocol

- **Milestone:** M1.
- **Depends on:** [01](01-determinism-foundation.md), [02](02-galaxy-model.md),
  [03](03-placement-and-range-query.md). Tasks P04.T1–T5 and T8–T10 need none of them and can start
  at once; see [Order and parallelism](#order-and-parallelism).
- **Brainstorm sections covered:** Generator version; Time and Identifiers (their wire forms only);
  Overlays and persistence; Runtime and code shape (caches, thread pool, protocol growth); Testing
  (the parts that concern the server and the wire); The range query (limits and the census as the
  client sees them); Visualiser (the three protocol messages, requests over the single socket, the
  density map's cost, cache and encoding); Suggested order of attack, step 3.

## Goal

When this plan is done the server can create a universe from a seed, given or drawn by the server,
persist it as `(seed, generator_version)` and nothing generated, list and reopen it, and refuse one
whose generator version it cannot run. A client can ask, over the one WebSocket it already has, for
a universe's galaxy parameters, a density map and the systems within range of a point at a time.
Each request carries an ID, is answered exactly once with a response or an error of its own, and can
be cancelled. Generation runs on a CPU pool with bounded queues, so hello and ping are answered
while a map computes. Cells, density maps and galaxies (parameters plus potential tables) sit in
bounded caches that the server owns. `@hyperion/protocol` has the generated types, decoders for the
wire forms that need one, and a typed request client, and `connection.ts` exposes it. Plan 05 builds
the display on that and writes no transport code.

## Scope and non-goals

In scope: `crates/hyperion-protocol`, `crates/hyperion-server`, `packages/protocol`,
`apps/hyperion/src/renderer/src/lib/connection.ts` with its test fake and the two components that
switch on the link status, one small change to `Simulation` in `hyperion-sim`, the `README.md`
section on running the server, and `.gitignore`.

Not in scope:

- Any generation code. The server calls Plans 01–03 and converts their types to wire types.
- Any display, navigation between displays, or the universe creation screen (Plan 05).
- Overlays (pinned content, deltas, enrichment, knowledge). The save directory is defined as their
  home and only the identity file is written.
- A running session: nothing steps a `Simulation`, no ship exists, and the clock is not persisted.
  Every query carries its own time.
- Deleting or renaming a universe. An operator removes the directory by hand.
- Authentication, TLS, more than one server process sharing a data directory.
- System summary, body detail, alerts and subscriptions. Their place in the convention is reserved
  under [Extending the convention](#extending-the-convention).
- Zoomed or panned density maps. The response describes its own extent so that a `region` field can
  be added to the request later without changing the response.

## Provides

### Wire types (`hyperion_protocol`, generated into `packages/protocol/src/generated`)

`PROTOCOL_VERSION` becomes `2`. `ClientMessage` and `ServerMessage` keep `hello`, `ping`, `welcome`,
`pong` and `error` unchanged except that `Welcome` gains `generator_version: u32`. Both enums lose
`Eq`, because requests and responses carry `f64`.

```rust
// primitives.rs
pub struct SeedHex(String);        // 16 lowercase hex digits; serde try_from = "String"
pub struct UniverseIdHex(String);  // same form
pub struct SystemIdHex(String);    // same form
//   each: from_u64(u64) -> Self, to_u64(&self) -> u64, as_str(&self) -> &str,
//         TryFrom<String, Error = ParseHex64Error>
pub enum ParseHex64Error { WrongLength, InvalidDigit }
//   Display for both: "expected 16 lowercase hexadecimal digits"
pub struct UniverseTime { pub seconds: i64, pub nanos: u32 }
pub struct GalacticPosition { pub cell_ly: [i32; 3], pub offset_m: [f64; 3] }

// envelope.rs
pub struct RequestId(pub u32);     // serde transparent
pub enum ClientMessage { Hello {..}, Ping {..},
    Request { id: RequestId, body: RequestBody }, Cancel { id: RequestId } }
pub enum ServerMessage { Welcome {..}, Pong {..}, Error {..},
    Response { id: RequestId, body: ResponseBody },
    RequestError { id: RequestId, error: RequestError } }
pub struct RequestError { pub code: ErrorCode, pub message: String, pub field: Option<String> }
pub enum ErrorCode { BadRequest, Unsupported, HelloRequired, UnknownUniverse,
    GeneratorVersionMismatch, UnsupportedSaveFormat, NameTaken, UniverseLimitReached,
    TooManyRequests, QueueFull, Cancelled, StorageFailed, Internal }
pub enum RequestBody {   // #[serde(tag = "kind", rename_all = "snake_case")]
    CreateUniverse(CreateUniverseRequest), ListUniverses, OpenUniverse(OpenUniverseRequest),
    GalaxyParameters(GalaxyParametersRequest), DensityMap(DensityMapRequest),
    SystemsInRange(SystemsInRangeRequest) }
pub enum ResponseBody {  // same tag, same kind strings as the request answered
    CreateUniverse(UniverseInfo), ListUniverses(UniverseList), OpenUniverse(UniverseInfo),
    GalaxyParameters(GalaxyParameters), DensityMap(DensityMap), SystemsInRange(SystemsInRange) }
pub const REQUEST_KINDS: &[&str];  // every RequestBody kind string, pinned by a test

// universe.rs
pub struct CreateUniverseRequest { pub name: String, pub seed: Option<SeedHex> }
pub struct OpenUniverseRequest { pub universe: UniverseIdHex }
pub struct UniverseInfo { pub id: UniverseIdHex, pub name: String, pub seed: SeedHex,
    pub generator_version: u32, pub status: UniverseStatus }
pub enum UniverseStatus { Compatible, GeneratorMismatch }
pub struct UniverseList { pub universes: Vec<UniverseInfo>, pub server_generator_version: u32 }

// galaxy.rs
pub struct GalaxyParametersRequest { pub universe: UniverseIdHex }
pub struct GalaxyParameters { pub universe: UniverseIdHex, pub seed: SeedHex,
    pub generator_version: u32, pub groups: Vec<ParameterGroup> }
pub struct ParameterGroup { pub key: String, pub parameters: Vec<Parameter> }
pub struct Parameter { pub key: String, pub origin: ParameterOrigin, pub value: ParameterValue }
pub enum ParameterOrigin { Drawn, Derived, Fixed }
pub enum ParameterValue {          // #[serde(tag = "type")]
    Number { value: f64, unit: Unit }, Text { value: String } }
pub enum Unit { None, Count, Msun, Ly, Myr, Gyr, KmPerS, DegPerMyr, Deg, PerLy3 }
pub enum MapView { FaceOn, EdgeOn }
pub enum MapPopulation { All, Young }
pub struct DensityMapRequest { pub universe: UniverseIdHex, pub view: MapView,
    pub population: MapPopulation, pub resolution: u16, pub bits: u8 }
pub struct DensityMap { pub universe: UniverseIdHex, pub view: MapView,
    pub population: MapPopulation, pub width_px: u16, pub height_px: u16,
    pub centre_ly: [f64; 2], pub ly_per_px: f64, pub bits: u8,
    pub floor_log10_per_ly2: f64, pub ceiling_log10_per_ly2: f64, pub data_base64: String }
pub enum MassLayer { A, B, C, D, E }
pub enum Population { YoungThinDisc, OldThinDisc, ThickDisc, Bulge, LongBar, NuclearDisc, Halo }
pub struct SystemsInRangeRequest { pub universe: UniverseIdHex, pub centre: GalacticPosition,
    pub radius_ly: f64, pub time: UniverseTime, pub min_layer: MassLayer, pub limit: u32 }
pub struct SystemsInRange { pub universe: UniverseIdHex, pub centre: GalacticPosition,
    pub radius_ly: f64, pub time: UniverseTime, pub census: Census,
    pub systems: Vec<SystemRecord> }
pub struct Census { pub limit: u32, pub complete_above_msun: Option<f64>,
    pub layers: Vec<LayerCensus> }
pub struct LayerCensus { pub layer: MassLayer, pub mass_min_msun: f64, pub mass_max_msun: f64,
    pub expected: f64, pub returned: u32, pub status: LayerStatus }
pub enum LayerStatus { Included, OverLimit, BelowMassFloor, OverCellBudget }
pub struct SystemRecord { pub id: SystemIdHex, pub designation: String,
    pub position: GalacticPosition, pub layer: MassLayer, pub initial_mass_msun: f64,
    pub age_myr: f64, pub population: Population }
```

Every enum is `snake_case` on the wire. The wire forms are fixed under
[Design notes](#design-notes).

### Server (`hyperion_server`)

```rust
pub struct ServerConfig;            // builder; from_env() -> Result<_, ParseConfigError>
pub struct Server;                  // Server::start(ServerConfig) -> Result<_, StartServerError>
impl Server { pub fn router(&self) -> axum::Router; pub fn stats(&self) -> ServerStats;
              pub async fn shutdown(self) -> Result<(), ShutDownServerError>; }
pub mod limits;                     // every limit below, as documented constants
pub mod universe { UniverseId, Universe, UniverseRegistry, UniverseStore, Entropy, OsEntropy,
                   CreateUniverseError, OpenUniverseError }
pub mod compute  { CpuPool, Priority, SubmitJobError, CancelToken, SingleFlight, GalaxyCache,
                   SharedCellCache, CellCacheHandle, DensityMapService, MapKey,
                   RawDensityMap, quantise_map }
pub mod cache    { ByteLru, HeapBytes }
```

`hyperion_server::app()` is replaced by `Server::router()`. `DEFAULT_ADDR` stays.

Environment read by `ServerConfig::from_env`: `HYPERION_ADDR` (existing), `HYPERION_DATA_DIR`
(default `./hyperion-data`), `HYPERION_WORKERS` (default: available parallelism less one, at least
one), `HYPERION_CELL_CACHE_MB` (default 256), `HYPERION_MAP_CACHE_MB` (default 64).

### TypeScript (`@hyperion/protocol`)

```ts
export const PROTOCOL_VERSION: number; // generated/ProtocolVersion.ts
export type RequestKind = RequestBody["kind"];
export type RequestOf<K extends RequestKind> = Extract<RequestBody, { kind: K }>;
export type ResponseFor<K extends RequestKind> = Extract<ResponseBody, { kind: K }>;
export type RequestFailure =
  | RequestError
  | { code: "link_lost" | "aborted" | "superseded" | "protocol_violation"; message: string };
export type RequestOutcome<K extends RequestKind> =
  { ok: true; response: ResponseFor<K> } | { ok: false; error: RequestFailure };
export interface PendingRequest<K extends RequestKind> {
  readonly outcome: Promise<RequestOutcome<K>>;
  cancel(): void;
}
export class RequestClient {
  constructor(send: (message: ClientMessage) => boolean);
  request<K extends RequestKind>(body: RequestOf<K>): PendingRequest<K>;
  handleServerMessage(message: ServerMessage): boolean; // true when the message was consumed
  linkLost(): void;
}
// A request on a channel cancels the channel's previous one ("latest wins").
export class RequestChannel {
  constructor(client: RequestClient);
  request<K extends RequestKind>(body: RequestOf<K>): PendingRequest<K>;
}
export function u64ToHex(value: bigint): string;
export function hexToU64(hex: string): bigint;
export function isHex64(text: string): boolean;
export const METRES_PER_LIGHT_YEAR: number;
export const SECONDS_PER_JULIAN_YEAR: number;
export function galacticPositionFromLy(xyzLy: readonly [number, number, number]): GalacticPosition;
export function galacticDeltaLy(
  from: GalacticPosition,
  to: GalacticPosition,
): [number, number, number];
export function universeTimeFromYears(years: number): UniverseTime;
export function universeTimeToYears(time: UniverseTime): number;
export interface DecodedDensityMap {
  readonly widthPx: number;
  readonly heightPx: number;
  readonly codes: Uint8Array | Uint16Array;
  readonly maxCode: number;
  log10PerLy2(code: number): number | null;
}
export function decodeDensityMap(map: DensityMap): DecodedDensityMap;
```

In the client, `useServerConnection` returns `ServerConnection`, which extends the existing
`ConnectionState` with `readonly requests: RequestClient`; `ConnectionState` gains
`serverProtocolVersion` and `serverGeneratorVersion` (`number | null`), and `ConnectionStatus` gains
`"incompatible"`. This plan owns the socket, the request client and the decoders. How components
reach `requests` (a context, props), every hook built on it, the ramp and everything drawn are Plan
05's.

### Test helpers

- `crates/hyperion-server/tests/common/mod.rs`: `TestServer::start()` and `start_with(ServerConfig)`
  (temporary data directory, fixed entropy, port 0), `TestClient` with `hello()`,
  `request(body) -> Result<ResponseBody, RequestError>`, `send_raw(&str)`, `next_message()`, every
  wait under `NETWORK_TIMEOUT`.
- `packages/protocol/fixtures/density_map_4x2.json`: one encoded map pinned from both languages.
- `FakeWebSocket.serverResponds(id, body)` and `serverRejects(id, error)`.

## Consumes

Names are those of the neighbouring plans' Provides as drafted alongside this one. Where they
change, the owning plan wins, and only `crates/hyperion-server/src/convert.rs` and `src/compute/`
follow.

- **Plan 01.** `hyperion_sim::GENERATOR_VERSION`, a `version::GeneratorVersion` (`get() -> u32`,
  `is_supported()`). `rng::Seed` (`Seed::new(u64)`, `get()`): the server stores and sends a seed as
  a `u64` and wraps it only when it calls the sim. `time::UniverseTime::new(seconds, nanos)`, which
  rejects `nanos ≥ 10⁹`, `UniverseTime::EPOCH`, `checked_add`, `time::Span`, `time::ClockWindow`
  (`contains`) and `time::CLOCK_WINDOW_H`. `coords::LyCell` and
  `coords::GalacticPosition::new(cell, offset)`, which rejects a non-canonical offset, with
  `in_root_cube()` as a separate check that the server makes itself. The getters the wire conversion
  is built on: `UniverseTime::seconds()` and `subsec_nanos()`, `Span::new(seconds, nanos)` from
  whole seconds and nanoseconds (for `Simulation::now`) with the same two getters, and
  `GalacticPosition::cell()` and `offset_metres()`; each pair rebuilds its value exactly through
  `new` (P01.T4.a, P01.T5.a). `id::SystemId` with `from_raw`, `raw` and `designation()`, and
  `id::Designation` (`Display`).
  `units::{LightYears, SolarMasses, Years, Megayears, KilometresPerSecond, PerYear}` and
  `units::consts`. Slow tests as `#[ignore = "slow: …"]` under `just test-slow`; `just bench`;
  Criterion in `[workspace.dependencies]`.
- **Plan 02.** `galaxy::Galaxy::new(seed: Seed)`, which is `Send + Sync`, has no interior mutability
  and builds the in-plane potential tables (target 100 ms); `Galaxy::with_full_potential` (about a
  second) is for Plans 08–10 and is not called here. `Galaxy::params()`, `potential()` (`v_circ`,
  `escape_speed_in_plane`, `bar_pattern_speed`, in radians per Julian year), `system_count()` and
  `mean_system_mass()`, and the `GalaxyParams` getters for every drawn and derived quantity. From
  `galaxy::map`: `MapView`, `MapSelection`, `MapSpec` and
  `render_rows(&Fields, &MapSpec, Range<u32>, &mut Vec<f64>)`, which renders a range of rows of one
  `MapSpec` in systems per square light-year and is what a band job calls. `imf::MASS_BAND_EDGES`
  and `imf::MassBand` for the band edges, and `galaxy::Population`.
- **Plan 03.** From `placement`: `CellKey`, `SystemRecord`, `generate_cell`, `cell_heap_bytes` and
  `resolve`. From `query`: `RangeQuery`, `RangeQueryBuilder` with `time`, `limit`, `mass_floor` and
  `cell_budget` (`limit` and `cell_budget` take a `NonZeroU32`) and `build()`, which itself refuses
  a time outside ±H (Plan 03, design note 12), `MassFloor`, `SystemHit`, `RangeResult`, `Census`,
  `CensusStop`, `LayerCounts` and `range_query`. **`placement::CellCache`**, the trait through which
  the caller supplies cells:
  `with_cell(&mut self, &Galaxy, CellKey, impl FnOnce(&[SystemRecord]) -> R) -> R`. The server's
  implementation is `CellCacheHandle` (P04.T12). `range_query` takes no cancellation token, so a
  running query always finishes; its cell budget bounds how long that is.

## Design notes

Each is a decision the brainstorm leaves open.

### The request convention

1. **One envelope, a nested body.** Requests are `{"type":"request","id":7,"body":{"kind":…}}` and
   not a top-level variant per request. The ID, cancellation, the error path and the in-flight table
   are then written once, and the TypeScript client derives `ResponseFor<K>` from the `kind` literal
   with no table kept by hand.
2. **IDs are chosen by the client**, a `u32` counter per connection starting at 1. An ID may be
   reused once its terminal message has arrived. A `request` whose ID is already in flight is
   dropped and answered with the connection-level `error`, because a `request_error` would end the
   wrong request.
3. **Exactly one terminal message** per accepted request: `response` or `request_error`. That holds
   for a cancelled request too, which ends with `cancelled` unless its response was already queued.
   The client can therefore drain its table without timers.
4. **Errors per request.** A frame that fails to parse as `ClientMessage` is parsed again with a
   lenient probe (`type`, `id`, `body.kind`). If it was a request with a readable ID the reply is a
   `request_error` for that ID, `unsupported` when the kind is not in `REQUEST_KINDS` and
   `bad_request` otherwise. Only a frame with no usable ID gets the old connection-level `error`.
5. **Cancellation, and supersession built on it.** `cancel` names an ID. A queued job is skipped, a
   running range query finishes and its result is dropped (the sim's query takes no token, and its
   cell budget bounds the waste), and closing the socket cancels everything the connection had in
   flight. The `cancelled` reply frees the request's in-flight slot at once while the job runs on;
   what a client can waste that way is bounded by the workers and the interactive queue, after which
   it is refused with `queue_full`. No thread is ever killed, which is also why generation runs on
   the pool and not under `spawn_blocking`. There is no separate supersede field: the client's
   `RequestChannel` cancels its previous request when a new one is made, which gives displays
   "latest wins" with one server mechanism. A density map in progress is never stopped by one
   requester leaving, since its result is cached and others may be waiting; it stops only when every
   waiter has gone.
6. **Requests are stateless.** Each names its universe. The client reconnects every two seconds
   after a drop, and a "current universe" held per connection would be lost each time.
   `open_universe` therefore means "check it, load it, warm its galaxy and tell me about it", and a
   request for a universe that is on disk but not loaded loads it.
7. **Hello first.** A `request` before `hello` is refused with `hello_required`. `ping` stays
   allowed at any time.

### Wire forms

8. **Every `u64` is 16 lowercase hexadecimal digits**: system IDs as the brainstorm requires, and
   seeds and universe IDs by the same rule for the same reason. Upper case is rejected so that one
   value has one string. How an operator types a seed is Plan 05's concern.
9. **`UniverseTime` is `{"seconds": i64, "nanos": u32}`** with `seconds` as a JSON number. The
   source horizon is 8.3 × 10¹² s either side, far inside the 2⁵³ a JavaScript number holds exactly.
   `#[ts(type = "number")]` stops ts-rs emitting `bigint`. `nanos` is below 10⁹ and counts forward
   from `seconds`, as Plan 01 defines.
10. **Positions cross exactly**: the integer light-year cell and the metre offset, never one `f64`
    of light-years, which at 50,000 ly resolves only 65 km. The client gets chart coordinates from
    `galacticDeltaLy`, a subtraction of cells and offsets that is exact to well under a metre.
    Records carry the position at the query's time. Wire order is the order of `RangeResult`;
    clients sort for themselves.
11. **Galaxy parameters are a grouped list of `key`, value and unit**, not a struct mirroring
    `GalaxyParams`. The set grows in Plans 07, 09 and 10, the display is a table, and labels belong
    to the client's glossary under the style guide's "one name per thing". Keys are dotted
    `snake_case` (`disc.thin.scale_length`), never renamed once shipped. `origin` separates drawn
    values from derived ones (system count, pattern speed) and fixed ones (the mass function, which
    belongs to the generator version). **The wire carries only units that the UX guide allows or
    gains in Plan 05**, so that the client formats and never converts: lengths in light-years,
    masses in M☉, times in Myr or Gyr, speeds in km/s, angles in degrees, densities per cubic
    light-year, and the bar's pattern speed in degrees per Myr (`deg_per_myr`; 38 km/s per kpc is
    2.23 °/Myr). The kiloparsec appears in the brainstorm's prose and in the sources, never on the
    wire: `convert.rs` converts from the sim's working units with `units::consts`. Every `Unit`
    value is one that Plan 05's `UnitLabel` can set in B612. M1 sends the structural parameters the
    brainstorm's Visualiser asks for; the metallicity gradient and the scatter terms are not
    structural and are not sent (T14.b lists the exclusions), so no `dex` unit exists yet. It joins
    `Unit` with the plan that first displays a metallicity.
12. **Density map.** The brainstorm says 8 or 16 bits; both are implemented and the request picks,
    since quantising is a millisecond and the raw grid is what is cached. Rows run top to bottom,
    pixels left to right, 16-bit codes little-endian. Face-on is seen from galactic north with +x to
    the right and +y up, so rotation is counter-clockwise on screen. Edge-on looks along +y with +x
    to the right and +z up. Code 0 means "at or below the floor, or empty"; codes 1 to 2ᵇⁱᵗˢ − 1
    span floor to ceiling linearly in log₁₀ of systems per square light-year. The ceiling is the
    grid's maximum and the floor is the ceiling less 5 dex face-on and 7 dex edge-on, one decade
    more than the spans the brainstorm quotes. M1 extents are fixed: face-on a square of 131,072 ly
    centred on the origin, edge-on 131,072 by 65,536 ly (`height_px = resolution ÷ 2`). Resolutions
    are 128, 256, 512 or 1,024 pixels wide.
13. **The census on the wire** lists all five layers with band, expected count, returned count and a
    status, plus `complete_above_msun`, which is `null` when nothing fits. "Nothing fits" is a valid
    response with no systems, not an error.
14. **Wire structs have public fields.** They are passive data and the server validates them in
    `convert.rs`. The three hex newtypes are the exception, since their invariant is purely
    syntactic and serde can enforce it at parse time. Encoding and decoding a hex string is the only
    behaviour the protocol crate gains.
15. **`PROTOCOL_VERSION` goes to 2** although the change is additive, because a version 1 server
    cannot serve this client at all. From here on, adding a request kind or an optional field does
    not bump it, since an older server answers `unsupported`; removing or changing one does. The
    client compares the welcome's version with its own and shows `incompatible`.

### Universes

16. **A save has its own identity.** Two campaigns may share a seed and version and will differ in
    their overlays, so a universe has a `UniverseId`, a random `u64` from the server's entropy
    source, and an operator-given name. Names are 1–48 characters after trimming, without control
    characters, unique without regard to case. The mass function is not a creation option: it
    belongs to the generator version.
17. **On disk**: `<data_dir>/universes/<id as 16 hex>/universe.json`, holding
    `{"format":1,"id":…,"name":…,"seed":…,"generator_version":…}` with `u64`s in the wire's hex
    form. It is written to a temporary file, synced and renamed. Unknown fields are ignored so that
    later formats can add them; a `format` above 1 is refused. The directory is the home of later
    overlays: the names `pinned/`, `deltas/`, `enrichment/`, `knowledge/` and `session.json` (the
    clock and ship state) are reserved and not created.
18. **Version mismatch.** A save whose generator version differs from the server's is listed with
    status `generator_mismatch` and cannot be opened or queried: `generator_version_mismatch`, with
    both versions in the message. The file is never rewritten and nothing is regenerated under a new
    version. This is the brainstorm's "saves are declared incompatible"; keeping old generators
    runnable is a decision for the first release.
19. **Randomness and the clock stay out of the sim.** A missing seed and every universe ID come from
    an `Entropy` trait, `OsEntropy` over the `getrandom` crate in production and a fixed sequence in
    tests. Saves carry no timestamp, so nothing reads the wall clock.
20. **`Simulation` and universe.** A `Universe` is immutable identity plus a lazily built
    `Arc<Galaxy>`, shared by everyone. A `Simulation` is the mutable, stepped state of one session
    inside a universe: ship, clock, later the deltas. M1 has no session, so the server constructs
    none. The struct stays, its seed documented as the universe's seed, and it gains
    `now() -> UniverseTime` (the epoch plus elapsed time) so that the session clock is already
    expressed on the universe clock when Plan 12 needs "now".

### Runtime

21. **A hand-written pool, not rayon**, whose queue is unbounded. Worker threads are plain
    `std::thread`s. Two queues, `Interactive` and `Bulk`, each bounded by a
    `tokio::sync::Semaphore`; workers always take interactive work first. Range queries, galaxy
    builds and parameter reads are interactive and fail fast with `queue_full`. Density map bands
    are bulk and wait for a slot. A map therefore cannot delay a chart by more than one band. Jobs
    run under `catch_unwind`, so a panic costs one request (`internal`) and not a worker.
22. **The connection task owns its state.** One task per socket reads frames, owns the in-flight
    table and a `JoinSet` of request tasks, and feeds a writer through a bounded channel. Pool jobs
    return the finished text frame, so serialising a 4 MB response also happens off the runtime.
23. **One byte budget per cache kind for the process**, with `(seed, generator_version)` in every
    key, so two saves of one seed share entries. `ByteLru` is a `HashMap` plus a `BTreeMap` of
    use-ticks behind one `std::sync::Mutex`; an entry larger than the whole budget is returned and
    not stored. Sharding waits for a benchmark that shows contention. Galaxies are the exception: a
    `Galaxy` is fixed-size, so `GalaxyCache` is bounded to four entries, which is bounding bytes.
    The systems-level cache of the brainstorm holds full systems with stars and bodies and is
    instantiated from the same `ByteLru` by Plan 06; in M1 a cell's accepted records are its
    systems.
24. **Limits** (all in `limits.rs`): inbound frame 16 KiB; 8 requests in flight per connection; 16
    malformed frames in a row closes the socket; 256 universes; census limit at most 20,000; query
    time within ±H (Plan 03's `build()` refuses the same, by its design note 12; the server checks
    first so that the error names the field); radius finite, above zero and at most 131,072 ly;
    centre inside the root cube; **at most 262,144 cells visited per query**, passed to Plan 03's
    `RangeQueryBuilder::cell_budget`, whose own default of 2²⁰ is sized for a caller with no clients
    to protect. The budget is Plan 03's addition to the brainstorm: the census limit bounds systems
    returned, not work, and 2,000 ly in the outer halo expects a few thousand layer-A systems while
    visiting 10⁸ cells. A layer dropped for the budget is reported as `over_cell_budget`, and the
    census stays all-or-nothing per layer.

### Extending the convention

Later plans add a request by adding a variant to `RequestBody` and `ResponseBody` with the same
`kind`, its string to `REQUEST_KINDS`, a wire-form test for each, a handler, and
`just gen-protocol`. The TypeScript client needs no change. Reserved now:

- Kind strings, one owner each. A later plan takes its names from this list and adds none that is
  not here without adding it here first; no string is ever reused for another meaning. M1's own are
  `create_universe`, `list_universes`, `open_universe`, `galaxy_parameters`, `density_map` and
  `systems_in_range`.

  | Plan | Reserved kinds                                                                        |
  | ---- | ------------------------------------------------------------------------------------- |
  | 06   | `system_summary`                                                                      |
  | 07   | `extinction_map`, `extinction`                                                        |
  | 09   | `features_in_range`, `galaxy_features`, `feature_detail`, `black_hole_state`          |
  | 10   | `global_features`, `stream_track`                                                     |
  | 12   | `resolve_system`, `subscribe`, `unsubscribe`, `alerts_observer`, `alerts_acknowledge` |
  | 14   | `system_bodies`, `body_detail`, `body_events`                                         |

  Plans 08, 11 and 13 add fields to existing kinds and no kind of their own. `galaxy_features` (plan
  09: the features drawn on the galaxy map) and `global_features` (plan 10: the entries of the
  global list) are different requests despite the likeness.

- Server message type `notification`, for pushes. A subscription is opened by an ordinary request
  whose response carries a `subscription: u32`; pushes name it; it ends with `unsubscribe` or the
  socket. Alerts reach a console that way, through the knowledge overlay.
- Server message type `response_part`, should a payload outgrow one frame: parts precede the
  terminal `response`. Binary frames stay unused and reserved for bulk payloads.
- Any `SystemIdHex` or body ID arriving from a client goes through the sim's `resolve` before use,
  as the brainstorm requires of IDs read from the protocol. M1 has no such inbound field.
- `MassLayer`, `Population`, `LayerStatus`, `Unit` and `ErrorCode` will grow. TypeScript switches
  over them are exhaustive with no `default`, so a new value is a compile error where it matters.
- Optional request fields `region` (density map) and `include_substellar` (range query).

## Tasks

### Order and parallelism

Three tracks run side by side. **A, protocol:** T1 → T2 → T3 → T4 → T5. **B, registry:** T6 → T7.
**C, compute:** T8, T9, T10 in any order → T11, T12. They join at T13 (needs T1, T6.a and T8), then
T14 (needs T2, T3, T7, T11, T12, T13), T15 and T16. T1–T6 and T8–T10 need nothing from Plans 01–03.
T7 needs Plan 01's `GENERATOR_VERSION` and `UniverseTime`. T11 needs Plan 02, T12 Plans 02 and 03,
T14 onwards all three. Within T3 and T14 the lettered subtasks are independent.

### Track A: protocol

#### P04.T1 Envelope, primitives and version

**P04.T1.a Wire primitives.** Split `crates/hyperion-protocol/src/lib.rs` into modules `primitives`,
`envelope`, `universe`, `galaxy`, re-exported flat from `lib.rs`. Move `assert_wire_form` to a
`#[cfg(test)] pub(crate) mod testing`. Add the three hex newtypes through one private macro over
shared `encode`/`decode` functions, `ParseHex64Error`, `UniverseTime` and `GalacticPosition`, all
`#[ts(export)]`, the newtypes with `#[ts(type = "string")]`. If ts-rs warns that it cannot parse
`#[serde(try_from)]`, turn on its `no-serde-warnings` feature in the root `Cargo.toml`; the
`#[ts(type)]` override already says what the binding is.

Files: `crates/hyperion-protocol/src/{lib,primitives,testing}.rs`.

Tests: `seed_hex_wire_form` (`"00000000000004d2"` ↔ 1234),
`hex64_rejects_uppercase_short_long_and_prefix`, `hex64_round_trips_zero_and_max`,
`universe_time_wire_form` with a negative second count, `galactic_position_wire_form`, and doctests
on `from_u64`/`to_u64`.

Accept: `cargo test -p hyperion-protocol` passes; `just gen-protocol` emits `SeedHex.ts`,
`UniverseIdHex.ts`, `SystemIdHex.ts` as `string` and `UniverseTime.ts` with `seconds: number`.

**P04.T1.b Envelope and errors.** Add `RequestId`, `ClientMessage::{Request, Cancel}`,
`ServerMessage::{Response, RequestError}`, `RequestError`, `ErrorCode`, and `RequestBody` /
`ResponseBody` with the single kind `list_universes`, which needs `UniverseList`, `UniverseInfo` and
`UniverseStatus` in `universe.rs` now. Add `REQUEST_KINDS`. Drop `Eq` from the two message enums. In
`crates/hyperion-server/src/ws.rs` answer every `Request` with `request_error`/`unsupported` and
ignore `Cancel`, so the workspace stays green until T13. Re-export the new types from
`packages/protocol/src/index.ts`. The client's `switch` over `message.type` in
`apps/hyperion/src/renderer/src/lib/connection.ts` must stay exhaustive, because oxlint's
`switch-exhaustiveness-check` is an error: add `response` and `request_error` cases that do nothing,
with a comment that T5 routes them.

Tests: `request_wire_form`, `cancel_wire_form`, `response_wire_form`, `request_error_wire_form`
(with and without `field`), `universe_info_wire_form`, `universe_list_wire_form`,
`universe_status_strings`, `error_code_strings` covering every code,
`request_kinds_lists_every_variant` (serialise one value of each variant and look its `kind` up),
`unknown_request_kind_is_rejected`.

Accept: `just ci` green; the generated `ServerMessage.ts` contains `"type": "request_error"`.

**P04.T1.c Version 2 and its TypeScript constant.** Set `PROTOCOL_VERSION = 2`, rewrite its doc
comment to the rule in design note 15, and add `generator_version` to `Welcome` (the server fills it
from `hyperion_sim::GENERATOR_VERSION.get()`, or `0` with a comment until Plan 01 has landed). Add a
test `export_bindings_protocol_version` that writes `ProtocolVersion.ts`
(`export const PROTOCOL_VERSION = 2;`, formatted from the Rust constant) into the directory named by
the `TS_RS_EXPORT_DIR` environment variable, read at run time: `.cargo/config.toml` sets it for
every `cargo test`, `gen-protocol-check` overrides it, and the test falls back to ts-rs's default
`./bindings` when it is unset. The name makes the existing `just gen-protocol` and
`gen-protocol-check` filters pick it up with no change to the `justfile`. Re-export the constant
from `packages/protocol/src/index.ts`.

Tests: update `welcome_wire_form`; the server's `hello_then_ping_over_websocket` still passes. The
generated `Welcome` now requires `generator_version`, so add it to the welcome fixture in
`apps/hyperion/src/renderer/src/App.test.tsx`, or `pnpm typecheck` fails.

Accept: `just gen-protocol-check` passes and fails if the constant is edited by hand.

#### P04.T2 Universe lifecycle messages

Add `CreateUniverseRequest`, `OpenUniverseRequest` and the `create_universe` and `open_universe`
kinds, both answered with `UniverseInfo`.

Files: `crates/hyperion-protocol/src/universe.rs`, `envelope.rs`, `packages/protocol/src/index.ts`,
regenerated bindings.

Tests: `create_universe_wire_form_with_seed`, `create_universe_wire_form_without_seed` (pins
`"seed": null`), `open_universe_wire_form`, and the two response wire forms.

Accept: `just ci` green.

#### P04.T3 Galaxy messages

Each subtask adds its types to `galaxy.rs`, its kind to both bodies and `REQUEST_KINDS`, re-exports
and regenerated bindings, and leaves `just ci` green.

**P04.T3.a Galaxy parameters.** `GalaxyParametersRequest`, `GalaxyParameters`, `ParameterGroup`,
`Parameter`, `ParameterOrigin`, `ParameterValue`, `Unit`.

Tests: request and response wire forms with one numeric and one text parameter, `unit_strings`
covering every `Unit` (`none`, `count`, `msun`, `ly`, `myr`, `gyr`, `km_per_s`, `deg_per_myr`,
`deg`, `per_ly3`; spell each with `#[serde(rename)]` where `snake_case` would give another string)
and `parameter_origin_strings`.

**P04.T3.b Density map.** `MapView`, `MapPopulation`, `DensityMapRequest`, `DensityMap`. The doc
comment on `DensityMap` states design note 12 in full: row order, byte order, orientation of both
views, the meaning of code 0, the decode formula and the unit.

Tests: both wire forms, enum strings.

**P04.T3.c Systems within range.** `MassLayer`, `Population`, `SystemsInRangeRequest`,
`SystemsInRange`, `Census`, `LayerCensus`, `LayerStatus`, `SystemRecord`. Doc comments give units,
the frame (`GALACTIC`, axes as in the brainstorm's Coordinates), and that `age_myr` and `position`
are at `time`.

Tests: request wire form; response wire form with two records and a census whose
`complete_above_msun` is `0.5`; a second with `null`; enum strings for the three enums.

#### P04.T4 `@hyperion/protocol` runtime

**P04.T4.a Test set-up and helpers.** `pnpm add --filter @hyperion/protocol -D vitest`, a `test`
script and `vitest.config.mts` (node environment, `src/**/*.test.ts`). The package keeps
`lib: ["es2023"]` and no DOM types, so nothing here uses `atob` or `AbortSignal`. Add `hex.ts`,
`position.ts` and `time.ts` with the functions under Provides. `METRES_PER_LIGHT_YEAR` is
9,460,730,472,580,800 and `SECONDS_PER_JULIAN_YEAR` 31,557,600, both exact by definition; check them
against Plan 01's `units`. `galacticPositionFromLy` floors to the cell and keeps the offset in
`[0, 1 ly)`; `universeTimeFromYears` throws a `RangeError` on a non-finite value.

Tests: hex round trips including `0xffffffffffffffff`, rejection of bad strings; negative
coordinates land in cell −1 with a positive offset; `galacticDeltaLy` of two positions 50,000 ly out
and 3 ly apart is exact to 10⁻⁹ ly; time round trips at ±1,000 years.

Accept: `pnpm --filter @hyperion/protocol test`, `pnpm typecheck` and `pnpm lint` pass.

**P04.T4.b Density map decoder.** `densityMap.ts` with a table-driven base64 decoder (about thirty
lines, rejecting bad length and characters with an `Error`), `decodeDensityMap` checking that the
byte count equals `width_px × height_px × bits ÷ 8`, and `log10PerLy2` returning `null` for code 0.
Commit `packages/protocol/fixtures/density_map_4x2.json`, a 4 × 2 map at 16 bits with known codes.

Tests: decodes the fixture at 16 bits and an inline 8-bit case; floor and ceiling map to codes 1 and
`maxCode`; truncated data throws.

Accept: as T4.a. The same fixture is pinned from Rust in T11.b.

**P04.T4.c `RequestClient` and `RequestChannel`.** `requests.ts`. IDs count from 1 and wrap at 2³² −
1, skipping any still pending. `request` sends at once; when `send` returns `false` the outcome is
`link_lost`. `handleServerMessage` settles on `response` and `request_error`, reports a `response`
whose `kind` differs from the request's as `protocol_violation`, returns `false` for other messages
and ignores unknown IDs. `cancel()` sends `cancel`, settles as `aborted` at once and lets the
server's late terminal message fall on the floor. `linkLost()` settles everything. Outcomes never
reject. `RequestChannel.request` cancels its previous pending request, which settles as
`superseded`. The one cast from `ResponseBody` to `ResponseFor<K>` follows the `kind` check and
carries the documented trust-boundary comment.

Tests (`requests.test.ts`, a recording `send`): resolves with the typed response; server error
passes through with its code; mismatched kind; unknown ID ignored; `cancel` sends one `cancel` and
settles `aborted`; `linkLost` settles all; a refused send; the channel sends `cancel` for the older
request and only the newer resolves; IDs are not reused while pending.

Accept: as T4.a, and `RequestOf<"density_map">` fails to typecheck with a range-query body (asserted
with `@ts-expect-error` in the test file).

#### P04.T5 `connection.ts` on the request client

Keep the hook's shape: one effect owns the socket, ping and reconnect. Create one `RequestClient`
per hook instance (`useState` initialiser) whose `send` writes to the current socket only while the
status is `connected`. Route every decoded message through `handleServerMessage` before the existing
`switch`, and add the two new message types to that switch as no-ops so that it stays exhaustive.
Call `linkLost()` on close and on clean-up. On `welcome`, compare `protocol_version` with
`PROTOCOL_VERSION`: on a mismatch set the status `incompatible`, start no ping timer, and keep the
socket open without retrying, since a retry cannot help. Add `serverProtocolVersion` and
`serverGeneratorVersion` to `ConnectionState`. Return `ServerConnection`. This task and T4 are the
whole of the client's transport: Plan 05 adds a context and hooks on top and touches neither the
socket nor the decoders.

Files: `lib/connection.ts`, new `lib/connection.test.ts`, `test/FakeWebSocket.ts`, `App.test.tsx`
(its welcome fixture sends `PROTOCOL_VERSION`, since version 1 now reads `LINK INCOMPATIBLE`),
`components/LinkStatus.tsx` (`incompatible: "LINK INCOMPATIBLE"`), `components/ConnectionPanel.tsx`
(a `Protocol` row showing server and client versions) and their tests, `styles.css` for the new
annunciator modifier in `--status-warning`.

Tests: a request made while connected reaches the fake socket and resolves when it responds; a
request while disconnected settles `link_lost`; closing the socket settles pending requests; a
welcome with version 1 shows `LINK INCOMPATIBLE` and sends no ping after the ping interval.

Accept: `pnpm test`, `pnpm typecheck`, `pnpm lint` pass; `App.tsx` compiles unchanged.

### Track B: universe registry

#### P04.T6 Configuration, server state and storage

**P04.T6.a `ServerConfig` and `Server`.** `config.rs`: `ServerConfig` with a builder and `from_env`,
`ParseConfigError` naming the variable and the value. `lib.rs`: `Server::start` builds the shared
state (`Arc<AppState>`, empty for now), `router()` attaches it with axum's `State`, and `shutdown()`
is the explicit teardown. `main.rs` parses the configuration, starts the server, serves with
graceful shutdown and then awaits `shutdown()`. Set the WebSocket `max_message_size` and
`max_frame_size` to `limits::MAX_INBOUND_FRAME_BYTES`. Move the existing test onto
`tests/common/mod.rs` (`TestServer`, `TestClient`), where it keeps asserting the welcome's version
and the pong's nonce. Add `tempfile`, `base64` and `getrandom` to `[workspace.dependencies]` in the
root `Cargo.toml` (`base64` 0.22 and `getrandom` 0.4, both already in `Cargo.lock`), and `sync` and
`time` to tokio's features there. The server crate takes `serde`, `base64`, `getrandom` and
`futures-util` as dependencies and `tempfile` as a dev-dependency, each with `workspace = true`. Add
`hyperion-data/` to `.gitignore` and the variables to `README.md`.

Tests: `from_env` defaults, a bad `HYPERION_WORKERS`, zero workers refused;
`oversized_frame_closes_the_connection` over a real socket.

Accept: `just ci` green; `just server` starts and creates no files until a universe is created.

**P04.T6.b `UniverseStore`.** `universe/store.rs`: `SaveFileV1` (private serde struct, design note
17), `UniverseStore::new(&Path)`, `scan() -> Result<Vec<SavedUniverse>, ScanStoreError>` (skips and
logs with `tracing::warn!` any directory whose name is not 16 hex digits, whose file is unreadable,
or whose `id` disagrees with its directory), `write(&SavedUniverse) -> Result<(), WriteSaveError>`
(temporary file, `sync_all`, rename, then sync the directory). All of it is blocking code called
through `spawn_blocking` by the registry.

Tests (temporary directories): write then scan round-trips; a file with an extra unknown field
loads; `format: 2` is reported as unsupported and not loaded; a truncated file is skipped and the
rest load; no `.tmp` file is left behind; the JSON on disk equals a pinned string.

Accept: `cargo test -p hyperion-server store` passes.

**P04.T6.c `Entropy`.** `universe/entropy.rs`: the trait, `OsEntropy`, `DrawEntropyError`, and
`SequenceEntropy` for tests (public, documented as a test double, because integration tests use only
the public API). `ServerConfigBuilder::entropy(..)` injects one; the default is `OsEntropy`.

Accept: unit test that `OsEntropy` returns two different values.

#### P04.T7 `UniverseRegistry` and `Simulation`

**P04.T7.a `UniverseRegistry`.** `universe/mod.rs`. `UniverseId(u64)`;
`Universe { id, name, seed, generator_version }` with getters and `key() -> GalaxyKey`
(`(seed, generator_version)`); `UniverseRegistry::load(store, entropy)` scans once into a
`Mutex<BTreeMap<UniverseId, Arc<Universe>>>`. `create(name, Option<u64>)` validates the name, checks
the count, draws seed and ID (redrawing an ID that exists), reserves the name under the lock, writes
through the store and removes the reservation if the write fails. `list()` is sorted by name, then
ID. `open(id)` returns `UnknownUniverse`, or `GeneratorVersionMismatch { saved, server }`, or the
universe. No guard is held across an `.await`.

Tests: name rules (empty, 49 characters, control character, duplicate in another case); a given seed
is kept and a missing one comes from the entropy double; the 257th universe is refused; a registry
reloaded from the same directory lists the same universes; a save written with another generator
version lists as `GeneratorMismatch` and refuses to open, and its file is byte-identical afterwards;
two concurrent creates of one name yield one success.

Accept: `cargo test -p hyperion-server universe` passes.

**P04.T7.b `Simulation` on the universe clock.** In `crates/hyperion-sim/src/lib.rs` rewrite
`Simulation`'s documentation per design note 20 and add `Simulation::now() -> UniverseTime`, the
epoch plus elapsed time. Coordinate with Plan 01, which adds modules to the same file.

Tests: three 50 ms steps give 150 ms past the epoch; the existing determinism test still passes.

Accept: `cargo test -p hyperion-sim` passes.

### Track C: compute and caching

#### P04.T8 `CpuPool`

**P04.T8.a Queue, workers, shutdown.** `compute/pool.rs`.
`CpuPool::new(workers, interactive_cap, bulk_cap)` spawns threads named `hyperion-cpu-N` and keeps
their `JoinHandle`s. Shared state is a `Mutex` over two `VecDeque`s and a shutdown flag, with a
`Condvar`. `try_submit(Priority::Interactive, job)` takes a semaphore permit without waiting or
returns `SubmitJobError::QueueFull`; `submit(Priority::Bulk, job).await` waits for one. Both return
a `oneshot::Receiver<Result<T, JobError>>`; the permit travels with the job and is released when a
worker takes it. Default capacities: 64 interactive, 256 bulk. `shutdown()` sets the flag, wakes
everyone, drops queued jobs (their receivers see `JobError::ShutDown`) and joins the threads inside
`spawn_blocking`. `Drop` only sets the flag and notifies.

Tests: a job's value comes back; interactive work queued behind bulk work runs first (one worker,
held by a job that waits on a channel the test controls, so no sleeps); the 65th interactive submit
is `QueueFull` while the worker is held; `shutdown` returns with jobs still queued.

**P04.T8.b Cancellation and containment.** `compute/cancel.rs`: `CancelToken` (`Arc<AtomicBool>`),
`cancel()`, `is_cancelled()`, and `CancelOnDrop`. Jobs are submitted with a token; a worker skips a
cancelled job (`JobError::Cancelled`) and hands the token to the closure. Wrap each job in
`catch_unwind(AssertUnwindSafe(..))` giving `JobError::Panicked`, logged with `tracing::error!`.
Each job runs in a span with `priority`, `queue_wait_ms` and `run_ms`.

Tests: a job cancelled while queued never runs; a panicking job yields `Panicked` and the same
worker runs the next job.

Accept for T8: `cargo test -p hyperion-server pool` passes in debug and under `--release`.

#### P04.T9 `ByteLru`

`cache/byte_lru.rs`: `trait HeapBytes { fn heap_bytes(&self) -> usize; }`,
`ByteLru<K: Eq + Hash + Clone, V: HeapBytes>` holding `Arc<V>`, with `new(budget_bytes)`,
`get(&K) -> Option<Arc<V>>` (refreshes the tick), `insert(K, Arc<V>)` (evicts oldest-first until the
total fits, refuses an oversized entry), `len()`, `bytes()`, and hit, miss and eviction counters.
Each entry is charged `heap_bytes()` plus `size_of::<V>()` plus a fixed 96-byte overhead, so that a
million empty rim cells still count. `SharedByteLru` wraps it in a `Mutex`.

Tests: eviction order follows use, not insertion; the byte total never exceeds the budget across a
scripted sequence; an oversized insert leaves the cache unchanged; replacing a key adjusts the
total; empty values are still bounded by the overhead.

Accept: `cargo test -p hyperion-server byte_lru`.

#### P04.T10 `SingleFlight`

`compute/single_flight.rs`: `SingleFlight<K, V, E: Clone>` with
`run(key, make_future) -> Result<Arc<V>, E>`, holding `futures_util::future::Shared` futures in a
`Mutex<HashMap>` and removing the entry when it completes. If every waiter is dropped the shared
future is dropped, and with it the `CancelOnDrop` it owns.

Tests (`#[tokio::test]`, channels for ordering): two callers of one key run the computation once and
both get the value; different keys run separately; an error reaches both callers and the next call
retries; dropping all callers cancels the token.

#### P04.T11 Galaxies and density maps

**P04.T11.a `GalaxyCache`.** `compute/galaxies.rs`:
`get(GalaxyKey) -> Result<Arc<Galaxy>, ComputeError>` through `SingleFlight` and an interactive pool
job calling `Galaxy::new`. This is where the potential tables are "computed once" per universe; the
four-entry bound is explained in design note 23. A key whose version fails
`GeneratorVersion::is_supported` is a bug and panics, since the registry refuses such universes
first. Log build time at `info`.

Tests: two concurrent `get`s build once (counter in a test-only builder hook injected through the
constructor); a fifth key evicts the least recently used; the rebuilt galaxy's parameters equal the
first build's.

**P04.T11.b Quantiser.** `compute/density_map.rs`:
`RawDensityMap { width_px, height_px, ly_per_px, centre_ly, log10: Vec<f32> }` with
`f32::NEG_INFINITY` for empty pixels and `HeapBytes`;
`quantise_map(&RawDensityMap, view, bits) -> QuantisedMap { floor, ceiling, bytes }` by the formula
of design note 12 (`code = 1 + round((v − floor) ÷ (ceiling − floor) × (max − 1))`, clamped, 0 at or
below the floor); base64 with the standard alphabet and padding. An all-empty map has floor and
ceiling 0 and every code 0.

Tests: hand-computed 8-bit and 16-bit cases; the ceiling pixel gets `max`; a pixel exactly at the
floor gets 0 and one just above gets 1; little-endian byte order; the 4 × 2 case reproduces
`packages/protocol/fixtures/density_map_4x2.json` byte for byte.

**P04.T11.c `DensityMapService`.** `MapKey { galaxy: GalaxyKey, view, population, resolution }`.
`get(key) -> Result<Arc<RawDensityMap>, ComputeError>`: map cache, else `SingleFlight`, else build.
A build makes one `MapSpec` for the whole raster and splits its rows into bands of 16. Each band is
a bulk job that checks the token, calls Plan 02's `render_rows` for its rows and converts to `log10`
as `f32`; the bands are assembled in order. Because every band renders from the same `MapSpec`, the
map is bit-identical for any worker count and band size.

Tests: a 128-pixel map built with one worker equals one built with four; the second `get` is a cache
hit (counter) and runs no job; two concurrent `get`s build once; the face-on map of a fixed seed is
unchanged by a rotation of 180° to a relative 10⁻⁹ (bar, discs and evenly spaced arms all have that
symmetry) and peaks at the centre; the edge-on map is symmetric in z. A golden test pins a 128 × 128
face-on map by the sum of its 8-bit codes and eight sampled codes, regenerated whenever a plan bumps
`GENERATOR_VERSION`.

Accept for T11: `cargo test -p hyperion-server density_map galaxies` passes.

#### P04.T12 `SharedCellCache`

`compute/cells.rs`: `CachedCell(Vec<SystemRecord>)` with `HeapBytes` from Plan 03's
`cell_heap_bytes`; `SharedCellCache` over `SharedByteLru<(GalaxyKey, CellKey), CachedCell>`; and
`SharedCellCache::handle(&self, GalaxyKey) -> CellCacheHandle`, the per-query value that implements
Plan 03's `CellCache`. Its `with_cell` looks the key up, on a miss calls `generate_cell` into a
scratch `Vec` it then moves into an `Arc` and inserts, and runs the closure on the slice with no
lock held. Two threads may generate the same cell once in a while and the second insert replaces an
equal value, which is cheaper than coordinating. Entries are accepted systems as state at the epoch,
never positions at a time, as the brainstorm requires.

Tests: a query through the handle equals the same query through Plan 03's `NoCache`, cold, warm, and
with a 1 KiB budget that evicts constantly (eviction is always safe); queries at two times share
cell entries (hit counter); an order-independence test runs queries A then B and B then A against
fresh caches and compares results.

Accept: `cargo test -p hyperion-server cells`.

### Track D: connection runtime and handlers

#### P04.T13 Connection task

**P04.T13.a Reader, writer, outbound queue.** Rewrite `ws.rs::serve`: split the socket; a writer
task drains a bounded `mpsc` (capacity 32) of text frames and is owned and joined by the connection
task; the reader loop `select!`s over inbound frames and `JoinSet::join_next`. `hello` and `ping`
are answered by pushing to the queue. Count consecutive malformed frames and close with a policy
violation at the limit. Give each connection a span with a counter ID and the peer address. Keep
`handle_text`'s two unit tests, adapted.

**P04.T13.b Dispatch, in-flight table, cancel.** `requests/mod.rs`. Parsing uses the lenient probe
of design note 4. The in-flight table maps `RequestId` to `CancelToken`; a duplicate live ID, a
request before hello and a ninth concurrent request are refused as design notes 2, 7 and 24 say.
Each accepted request becomes a task in the `JoinSet` that runs
`handle(state, body, token) -> Result<ResponseBody, RequestError>`, has the terminal frame
serialised on the pool when the body is a map or a range result, and pushes it to the writer;
`join_next` removes the ID. `cancel` sets the token; the task then answers `cancelled` unless it
already finished. On socket close every token is cancelled and the `JoinSet` is shut down. Map
`SubmitJobError::QueueFull` to `queue_full`, `JobError::Panicked` to `internal`, `ShutDown` to
`internal`. Every request runs in a span with `id` and `kind` and logs its outcome and duration at
`debug`, refusals at `warn`. `Server::start` builds the `CpuPool` from the configured worker count
and holds it in `AppState`; `Server::shutdown` shuts it down. Until T14 the handler returns
`unsupported` for every kind.

Tests (unit, with a fake handler injected through `AppState`): one terminal message per request
under interleaved cancels; a malformed body with a good ID gives `bad_request` for that ID; an
unknown kind gives `unsupported`; a frame with no ID gives `error`; a duplicate ID gives `error` and
the first request still completes; the ninth request gives `too_many_requests`.

**P04.T13.c `ServerStats`.** `Server::stats() -> ServerStats`: queue depths, jobs run and in-flight
requests now, and the hit, miss, eviction, byte and build counters of each cache once it is in
`AppState` (T14). Each cache exposes its own `counters()` from the task that builds it (T9, T11,
T12), so that those tasks do not wait for this one. The integration tests read these instead of
guessing at timing.

Tests: counters move as a fake handler's requests are accepted, run and finish.

Accept for T13: `just ci` green.

#### P04.T14 Handlers

All conversion and validation lives in `convert.rs` as `TryFrom` impls between wire and sim types,
with one error type, `ConvertRequestError { field, reason }`, that becomes `bad_request` with
`field` set. `Server::start` now also loads the `UniverseRegistry` (the scan runs under
`spawn_blocking`) and builds the `GalaxyCache`, the `DensityMapService` and the `SharedCellCache`
from the configuration, all held in `AppState`; the subtask that first needs one adds it. Each
subtask adds integration tests in `crates/hyperion-server/tests/` using `TestServer` and
`TestClient`.

**P04.T14.a Universe lifecycle** (`requests/universe.rs`). `create_universe`, `list_universes`,
`open_universe` over the registry; `open` also awaits `GalaxyCache::get` so the galaxy is warm when
it returns. Log creation and opening at `info` with ID, seed (hex) and version.

Tests (`tests/universes.rs`): create with a seed, list shows it, open returns the same info; create
without a seed returns the entropy double's value; a bad name gives `bad_request` with
`field: "name"`; a duplicate gives `name_taken`; an unknown ID gives `unknown_universe`;
**restart**: stop the server, start another on the same directory, list shows the universe and the
data directory holds exactly one file per universe; a save edited to another generator version lists
as `generator_mismatch` and `open`, `galaxy_parameters`, `density_map` and `systems_in_range` all
give `generator_version_mismatch`.

**P04.T14.b Galaxy parameters** (`requests/galaxy.rs`, `convert.rs`). Build the groups from
`GalaxyParams`: `identity` (seed, generator version, mass function), `mass` (stellar mass, system
count, mean mass per system, dark halo mass and concentration, gas mass, black hole mass),
`populations` (each share), `population_masses` and `population_mean_masses` (each population's
mass and mean system mass), `discs`, `bulge_and_bar` (including pattern speed), `nuclear_disc`,
`halo`, `arms` (number, pitch), `history` (formation timescale, last major merger) and `rotation`
(circular speed at 26,000 ly, escape speed there). The groups, their keys and their order are
exactly the table below, which Plan 05's glossary (`parameterLabels.ts`) and its fixture
(`everyGalaxyParameter` in `test/galaxyFixtures.ts`) copy and a client test holds them to. Units
are those of design note 11: the pattern speed is converted from Plan 02's radians per Julian year
to degrees per Myr (× 180 ÷ π × 10⁶), times from years to Gyr, and nothing is sent per kiloparsec.
Every structural getter Plan 02 provides appears once. Not sent in M1, and listed with the reason
in a constant `EXCLUDED_PARAMETERS` beside the builder: the metallicity gradient and each halo
component's mean [Fe/H] (no `dex` unit until a plan displays a metallicity); the scatter draws
(they are already folded into the sizes and masses shown; `BlackHoleParams::scatter` is the one
with a getter); the per-progenitor accretion list (Plan 10 displays it), which includes the lesser
progenitors' own halo components, whose total share is sent; the halo components' age
distributions (the `history` group stands for them); `HaloParams::discrete_share` (held at zero
until Plan 10, which sends it); the dark halo's scale radius (r₂₀₀ ÷ c₂₀₀, both sent); the
generator's constants, the same for every seed (the gas disc's scale height, the nuclear cluster's
slopes and break radius, the halo components' cut radii, the globular-born debris's flattening);
and the potential's radial profiles, of which only the `rotation` group's radius is sent
(`PotentialTables::bar_corotation` is `bar.corotation_radius`). A unit test counts sent plus
excluded parameters against a constant that a new getter's author must raise, and asserts keys are
unique and match `^[a-z0-9_]+(\.[a-z0-9_]+)*$`.

Keys by group, each with its `Unit` (or `text`) and origin (drawn, derived or fixed). `<p>` stands
for each `Population` in wire order (`young_thin_disc`, `old_thin_disc`, `thick_disc`, `bulge`,
`long_bar`, `nuclear_disc`, `halo`):

| Group                    | Keys, unit, origin                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| ------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `identity`               | `seed` (text, the wire's hex; fixed), `generator_version` (`count`, fixed), `mass_function` (text, `kroupa` or `chabrier`; fixed)                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `mass`                   | `stellar_mass` (`msun`, drawn), `system_count` (`count`, derived), `mean_system_mass` (M★ ÷ N; `msun`, derived), `mean_formed_mass`, `gas.mass`, `black_hole.mass`, `nuclear_cluster.mass` (`msun`, derived), `dark_halo.mass` (M₂₀₀; `msun`, derived), `dark_halo.concentration` (`none`, derived), `dark_halo.virial_radius` (r₂₀₀; `ly`, derived), `dark_halo.f_star` (`none`, drawn)                                                                                                                                                           |
| `populations`            | `population.<p>.share` (`none`; drawn for `thick_disc`, `nuclear_disc` and `halo`, derived for the rest)                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `population_masses`      | `population.<p>.mass` (`msun`, derived)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| `population_mean_masses` | `population.<p>.mean_system_mass` (`msun`, derived)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| `discs`                  | `disc.thin.scale_length` (`ly`, derived), `disc.thin.scale_height` (the mean; `ly`, drawn), `disc.young.scale_length` (`ly`, derived), `disc.young.scale_height` (`ly`, drawn), `disc.thick.scale_length`, `disc.thick.scale_height`, `disc.gas.scale_length` (`ly`, derived)                                                                                                                                                                                                                                                                      |
| `bulge_and_bar`          | `bulge.scale_x`, `bulge.scale_y`, `bulge.scale_z` (`ly`, derived), `bulge.boxiness` (`none`, drawn), `bar.share_of_bulge` (`none`, drawn), `bar.half_length`, `bar.width` (`ly`, derived), `bar.height` (`ly`, drawn), `bar.corotation_ratio` (`none`, drawn), `bar.corotation_radius` (`ly`, derived), `bar.pattern_speed` (`deg_per_myr`, derived)                                                                                                                                                                                               |
| `nuclear_disc`           | `nuclear_disc.scale_length`, `nuclear_disc.scale_height` (`ly`, derived)                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `halo`                   | `halo.in_situ.share`, `halo.dominant_merger.share`, `halo.lesser.share` (all lesser progenitors together), `halo.globular_debris.share` (`none`, derived); `halo.in_situ.slope`, `halo.dominant_merger.slope`, `halo.globular_debris.slope` (`none`, drawn); `halo.in_situ.core`, `halo.dominant_merger.core`, `halo.globular_debris.core` (`ly`, drawn); `halo.in_situ.flattening`, `halo.dominant_merger.flattening` (`none`, drawn); `halo.dominant_merger.break_radius` (`ly`, drawn), `halo.dominant_merger.break_steepening` (`none`, drawn) |
| `arms`                   | `arms.count` (`count`, drawn), `arms.pitch` (`deg`, drawn), `arms.young_width` (`ly`, drawn), `arms.young_fraction`, `arms.old_amplitude` (`none`, drawn)                                                                                                                                                                                                                                                                                                                                                                                          |
| `history`                | `history.formation_timescale` (`gyr`, drawn), `history.last_major_merger` (`gyr`, drawn), `history.globular_clusters` (`count`, derived)                                                                                                                                                                                                                                                                                                                                                                                                           |
| `rotation`               | `rotation.radius` (26,000 ly; `ly`, fixed), `rotation.circular_speed`, `rotation.escape_speed` (`km_per_s`, derived)                                                                                                                                                                                                                                                                                                                                                                                                                               |

Tests (`tests/galaxy_parameters.rs`): the response for a fixed seed equals a golden JSON file under
`crates/hyperion-server/tests/golden/` (regenerated on a generator bump); two universes with one
seed give equal groups.

**P04.T14.c Density map.** Validate `resolution` ∈ {128, 256, 512, 1024} and `bits` ∈ {8, 16}. Get
the raw map from the service, quantise and encode on the pool with the response frame.

Tests (`tests/density_map.rs`, 128 pixels): the decoded byte count matches the header for both
depths and both views; `ly_per_px × width_px` is 131,072; the ceiling code sits at the centre pixel
face-on; `young` and `all` differ; a repeated request is served from the cache (compare a counter
exposed by `Server::stats()`); bad resolution and bits give `bad_request` naming the field; **the
socket does not stall**: with one worker, request a 1,024-pixel edge-on map, then send `ping`, and
the `pong` arrives before the map's response; then `cancel` the map and receive `cancelled`.

**P04.T14.d Systems within range.** Validate per design note 24, in this order: universe, time
inside ±`CLOCK_WINDOW_H` and `nanos` below 10⁹, centre canonical and inside the root cube, radius,
limit between 1 and 20,000. Build the `RangeQuery` with the time, limit, `MassFloor` from
`min_layer` and `limits::MAX_QUERY_CELLS`, and run `range_query` on the pool as an interactive job
with a `CellCacheHandle` and no extra sources. Convert on the pool too: each `SystemHit` becomes a
record (position at the query's time, `age_at(time)` in Myr, population from the record's
`population()`, designation from the ID), and the census lists all five layers with Plan 02's band
edges and Plan 03's expected counts. A layer at or above `complete_down_to` is `included`; one below
the requested floor is `below_mass_floor`; the rest take `over_limit` or `over_cell_budget` from
`stopped_by`. `returned` is counted from the hits and `complete_above_msun` is
`Census::complete_above`.

Tests (`tests/systems_in_range.rs`, fixed seed, a centre in the plane at 26,000 ly): every record
lies within `radius_ly` of the centre by `GalacticPosition` arithmetic; IDs are 16 hex digits,
unique, and each decodes to a `SystemId` that the sim resolves to the same record; the response
echoes time, centre and radius; the same request twice gives identical JSON; a query at −500 years
returns the epoch query's records at the same positions while velocities are zero (Plan 08 changes
this test), less exactly those whose age at the epoch is under 500 years, and every `age_myr` is
0.0005 lower; `min_layer: "c"` returns only layers C to E and the census says so; a 50 ly query at
the galactic centre with limit 5,000 returns a census with `over_limit` layers and no partial layer;
a 5,000 ly query in the halo reports `over_cell_budget` layers and still answers; a radius of
200,000 gives `bad_request`; NaN cannot be sent as JSON, so `1e999` is tried and gives
`bad_request`; a centre outside the cube, `nanos = 10⁹`, a time of +2,000 years and a limit of 0
each give `bad_request` with the right `field`.

**P04.T14.e Galaxy creation end to end.** One integration test, `tests/galaxy_creation.rs`, walks
the path the roadmap calls galaxy creation over one real socket, as Plan 05's display will: `hello`;
`create_universe` with a name and the seed `00000000000004d2`; `galaxy_parameters` for the returned
ID (the seed echoes and the `mass` group is present); `density_map` face-on and edge-on at 128
pixels and 8 bits; a chart centre taken from the maps as the display takes it, x and y from the
centre of a face-on pixel at about 26,000 ly and z from the edge-on row nearest the plane, through
the same pixel-to-light-year rule that the `DensityMap` doc comment states; `systems_in_range` there
with a radius of 50 ly at the epoch and again at +100 years, both non-empty and both echoing their
time. Then stop the server, start another on the same data directory, `list_universes`,
`open_universe`, and repeat the epoch query: the response is byte-identical to the first. This is
the test that fails if any seam between Plans 01–03 and the wire moves.

Accept for T14: `cargo test -p hyperion-server` passes; `just ci` green.

#### P04.T15 Abuse and concurrency suite

`tests/abuse.rs`, over a real socket: 200 range queries fired without waiting end in exactly 200
terminal messages, some `too_many_requests`, and the server still answers `ping`; closing the socket
with a 1,024-pixel map in flight leaves the pool idle afterwards (`Server::stats()` reports no
queued jobs once the map's waiters are gone); two clients asking for one map get byte-identical
responses and one build; a 17 KiB frame closes the connection; sixteen malformed frames close it;
results for interleaved queries from two connections equal the same queries run alone (order
independence through the shared caches); `Server::shutdown()` returns within `NETWORK_TIMEOUT` with
work queued.

Accept: `cargo test -p hyperion-server --test abuse` passes ten times in a row
(`for i in $(seq 10)`), to catch ordering races.

#### P04.T16 Benchmarks and the timing test

`crates/hyperion-server/benches/density_map.rs` and `range_query.rs` (Criterion, under
`just bench`): raw map build at 512 pixels for each view and population with all workers and with
one; quantise plus base64 at 512 and 1,024 pixels for both depths; serialising a 5,000-record
response; a 50 ly query at 26,000 ly through the cell cache, cold and warm, and the same through
Plan 03's uncached path, to show what the cache and its lock cost. One slow test, marked as Plan 01
prescribes, `density_map_512_builds_within_budget`: both views in a release build finish within 20 s
on CI's two cores. Record measured figures in the bench files' module docs. Targets, as findings and
not CI failures: 512-pixel map cold on eight workers, face-on under 1 s and edge-on under 3 s; a
cached map request under 5 ms end to end; the 50 ly query's compute under the brainstorm's 5 ms cold
with the cache adding under 10%, and under 15 ms end to end over loopback; `pong` within 50 ms while
a map builds.

Accept: `just bench` runs both files; `just test-slow` passes.

## Verification

- `just ci` green after every task, and `just test-slow` and `just bench` after T16.
- Every message has a wire-form test: `hello`, `ping`, `welcome`, `pong`, `error`, `request`,
  `cancel`, `response`, `request_error`, each of the six request bodies and six response bodies, and
  every enum's strings. `request_kinds_lists_every_variant` keeps the list honest.
- The density map fixture is decoded in TypeScript and produced in Rust from the same file.
- `just gen-protocol-check` passes after every task of T1–T3, which is part of `just ci`.
- Galaxy creation end to end is covered by `tests/galaxy_creation.rs` (T14.e) on this side of the
  wire, by Plan 05's P05.T12.a on the other, and across both by the cross-stack checklist in Plan
  05's Verification.
- End to end by hand: `just server`, then with `websocat ws://127.0.0.1:7878/ws` send `hello`,
  `create_universe` with seed `00000000000004d2`, `galaxy_parameters`, `density_map` at 512, and a
  50 ly `systems_in_range`; restart the server and `list_universes` shows the universe; the data
  directory holds one small JSON file. With `RUST_LOG=hyperion_server=debug` the log shows one
  galaxy build, one map build per key, and cache hits on repeats.
- By eye, once Plan 05 lands: the face-on map reads as a barred spiral with counter-clockwise
  rotation and trailing arms, which checks the orientation rules of design note 12.

## Generator version

This plan generates nothing and never bumps `GENERATOR_VERSION`. It pins the version's behaviour at
the edges: the version is stored with every save, sent in `welcome`, in `UniverseInfo` and in
`GalaxyParameters`, and is part of every cache key. Golden files it owns
(`crates/hyperion-server/tests/golden/`) are regenerated in the commit of whichever plan bumps the
version. What it reserves so that later plans change no wire form: the request kinds, message types
and optional fields under [Extending the convention](#extending-the-convention); the save
directory's reserved names; hex forms for every 64-bit value; a time on every positional query.

## Risks and open points

- **Seams with Plans 01–03** were checked against their drafts, not their code. The ones that would
  hurt if they moved: `CellCache::with_cell` taking `&mut self` (the handle is per query, so this
  suits a shared cache), `Galaxy: Send + Sync`, and `render_rows` rendering any row range from one
  `MapSpec`.
- **A running range query cannot be cancelled**, because `range_query` takes no token. At 262,144
  cells that is a second or two of one worker at worst. If it matters, the fix is a token argument
  in Plan 03, not a thread kill here.
- **The split with Plan 05** is settled: this plan owns `connection.ts`, `RequestClient`,
  `RequestChannel` and `decodeDensityMap` (T4, T5); Plan 05 owns the context that hands `requests`
  to displays, `useServerRequest` with its timeouts, the ramp, the map geometry and everything
  drawn. `FakeWebSocket` is edited by both, this plan first.
- **Ambiguity: "a galaxy map at any zoom" (Fields) against a cache "per seed, view and population"
  (Visualiser).** Read as: M1 serves the whole galaxy at four resolutions, keyed as the Visualiser
  says plus resolution, and zoom is a later optional `region`. At 1,024 pixels a pixel is 128 ly, so
  Plan 05 cannot pick a chart centre from the map more finely than that and needs typed coordinates
  or a nudge control as well.
- **Ambiguity: "8 or 16 bits"** could be a choice for the design or for the request. Both are
  offered, per request.
- **The cell budget is an addition** to the brainstorm's census rule, made by Plan 03 and tightened
  here. It keeps the rule's shape (whole layers, reported) and is visible in the census.
- **Query times are held to ±H.** The brainstorm says nothing breaks beyond the window, only that
  errors grow; the server refuses rather than serve a degrading answer, and the pad of 1,000 km/s ×
  |t| would otherwise be unbounded. Plan 12's retarded-time evaluation happens inside the server and
  is not bound by this check.
- **A 20,000-record response is about 4 MB of JSON.** Fine on a LAN. If it proves slow, the reserved
  binary frames or `response_part` are the way out, not a lower limit.
- **Bulk work can starve** under a constant stream of interactive jobs. The in-flight cap per
  connection bounds it for a bridge's worth of clients; revisit if maps stall in practice.
- **`getrandom`'s API differs between 0.3 and 0.4**, both of which are in `Cargo.lock`. Pick the
  newer, behind `OsEntropy`, so that nothing else sees it.
- **`Simulation::now()` touches a file Plan 01 rewrites.** It is a five-line change; whoever lands
  second rebases.
- **The ordering test for "the socket does not stall"** relies on a 1,024-pixel edge-on map taking
  longer than one ping round trip on one worker. That is a safe margin of several orders of
  magnitude, but it is a margin and not a proof; the unit test of queue priority in T8.a is the
  proof.
- **Deviations in T1–T5, as built.** `nextFreeRequestId`, `MAX_REQUEST_ID` and the test-only
  `setLastRequestId` are exported from `requests.ts` but not from the package index, so the ID
  wrap-around can be tested without 2³² requests. A cancelled ID stays reserved until the server's
  late reply arrives or the link drops. oxlint rejects reading a ref during render, so the socket
  that requests use lives in a small `RequestLink` class held in `useState`; only the socket that
  requests are sent on settles them as `link_lost` when it closes. `serverGeneratorVersion` stays
  `null` when the server speaks another protocol version. The Protocol row reads
  `SERVER n / CLIENT m`, because `SRV` and `CLT` are not on the nomenclature list. ts-rs's
  `no-serde-warnings` feature is on in the root `Cargo.toml`; `packages/protocol` gained vitest.
  Until T13, `ws.rs` answers every request with `request_error` code `unsupported` and ignores
  `cancel`.
- **Deviations in T6–T10, as built.** `UniverseStore::scan()` returns `StoreScan` (loaded saves
  plus later-format ones), so `open` can answer `UnsupportedSaveFormat` and a create never reuses a
  later-format save's ID. `write` only creates: it fails with `AlreadyExists`, the registry then
  draws another ID, and a failed write removes what it wrote. Creates in progress count against
  `MAX_UNIVERSES` from the moment their name is reserved. An empty `HYPERION_DATA_DIR` is refused.
  `CpuPool::new` takes three `NonZeroUsize` and returns `Result`; jobs are
  `FnOnce(&CancelToken) -> T`; `shutdown(&self)` is safe to cancel and to call twice; `counters()`
  is added; a panic in a job's drop code is caught too. `SingleFlight::run` registers the caller
  at once and returns a `Flight` future whose `make_future` runs lazily on first poll, outside the
  lock (hence its `Send + 'static` bound); if the computation panics, every waiter panics with the
  original message and the key is freed, so T13 must answer `internal` when a request task
  panics. `ByteLru::insert` returns `Insertion` (stored or refused) and uses checked arithmetic;
  `SharedByteLru` drops evicted values after releasing its lock. `Server::start` is async and
  fails if the data directory is not a directory. For T13: `universe::Compatibility` stands in for
  `UniverseStatus`, the server's own `ParseHex64Error` in `universe/id.rs` should give way to the
  protocol's hex type, `TestClient::request` is still to add, and axum's WebSocket tasks are not
  joined by graceful shutdown, so `Server::shutdown` needs a task tracker.
- **Deviations in T13, as built.** The connection task alone sends terminal messages: a request's
  task returns its finished frame through the `JoinSet`, and `cancel` aborts the task and answers
  `cancelled` at once, dropping any later frame, so "already queued" (design note 3) means already
  pushed to the writer. The seam is `requests::Handler`
  (`handle(&self, Arc<AppState>, RequestBody, CancelToken) -> HandlerFuture`, a `BoxFuture`), held
  in `AppState` as `Arc<dyn Handler>`; `requests::Handlers` matches every kind and answers
  `unsupported` until T14 replaces each arm, and the crate-private `Server::start_with_handler`
  injects test doubles. Maps and range results are serialised by `pool.submit(Interactive, …)`,
  which waits for a place rather than refusing a finished response; since a handler returns a
  `ResponseBody`, T14.c's quantising and encoding is a job of its own before that one, unless T14
  widens the seam. A missing or non-string `kind` is `bad_request`. A request that does not parse
  still gets the duplicate-ID `error` and `hello_required` before its own error.
  `From<SubmitJobError>` and `From<JobError>` for `RequestError` give T13.b's codes, plus
  `JobError::Cancelled` → `cancelled`. A kind not in `REQUEST_KINDS` neither counts towards nor
  resets the malformed-frame limit, since a newer client sends one in good faith; a binary frame
  counts. T13.a's queue of 32 is `limits::OUTBOUND_QUEUE_FRAMES`, and `limits::CLOSE_TIMEOUT`
  (1 s, this task's choice, not the plan's) bounds each close and so `Server::shutdown`. A
  server-initiated close (1008 for malformed frames, 1001 for shutdown) reads on until the client's
  close, so that the socket is not reset. The task tracker is `connections::Connections`: a guard
  is taken before the upgrade (a server shutting down answers 503), and `Server::shutdown` closes
  every connection, waits for the guards, then stops the pool. `ServerStats` has `connections()`,
  `requests()` (`RequestCounters`: in flight, accepted, refused, responded, failed, cancelled,
  abandoned) and `pool()`. `ShutDownServerError` is an enum (`CpuPool`); `StartServerError` gains
  `StartPool`. `Universe::compatibility()` is now `status() -> UniverseStatus`,
  `universe::ParseHex64Error` is gone, and `UniverseId` converts to and from `UniverseIdHex`. The
  fake-handler tests are unit tests over real sockets (`src/testing.rs`), because integration tests
  cannot inject a handler. `main.rs` serves with connect info so that peers are logged. `TestClient`
  gains `request`, `send_request`, `cancel` and `close`, and `closed()` returns the close code.
- **Slow readers, for T15 (ruled after T13).** T13's outbound queue is bounded by frame count (32
  queued plus up to 8 finished), with no write timeout, so a client that stops reading can pin about
  160 MB of 4 MB range results until shutdown. T15 adds a per-frame write timeout
  (`limits::WRITE_TIMEOUT`, 10 s: a frame that cannot be written in that time closes the connection
  with 1008) and a per-connection byte budget on the outbound queue (`limits::OUTBOUND_BYTES`, 16
  MiB: the reader stops dispatching finished requests while the queue holds more), each with a test
  in the abuse suite. An oversized inbound frame still closes without 1009, since sending one needs
  a direct tungstenite dependency pinned to axum's; the plan only requires the close.
- **Deviations in T15's slow-reader part, as built.** `src/outbound.rs` holds the queue
  (`Outbound`), the `Writer` and `Held`. Each frame is charged its payload bytes (no WebSocket
  header) from being queued until it is written or dropped. The budget rule is stricter than
  "while the queue holds more": a finished request's frame is queued only if nothing is held before
  it and the queue is empty or stays within `OUTBOUND_BYTES`, else it waits in `Held`, oldest first.
  A held request stays in flight, so it keeps its slot, `cancel` ends it and drops its frame, and a
  close abandons it; the connection's own answers (`pong`, `welcome`, refusals, `cancelled`) are
  never held, and the 32-frame bound is unchanged. After a write timeout the writer drops every
  queued frame but the close, so the 1008 close follows the rest of the stuck frame if the client
  reads again, and otherwise the socket is dropped after `CLOSE_TIMEOUT`. `Requests::finish` is
  split into `settle` and `end`. `ServerStats::outbound()` returns `OutboundCounters`
  (`queued_bytes`, `largest_queue_bytes`, `held_requests`, `write_timeouts`), and
  `Harness::stop` checks that the first and third are zero. The crate-private
  `ws::ConnectionLimits` (outbound bytes, write and close timeouts; defaults from `limits`) sits in
  `AppState`, and `Server::start_with_handler` takes it; `testing.rs` gains
  `Harness::start_with_limits`, `outbound_until`, `stick_writer`, `response_frame_len` and
  `CLOGGING_BYTES`. The tests are unit tests in `outbound.rs`, not in `tests/abuse.rs`, because
  they need a scripted handler and test-only limits; T15's ten-run loop should also run
  `cargo test -p hyperion-server --lib outbound::`. The writer's own tests use a paused clock, so
  the server's tokio dev-dependency gains `test-util`.
- **Slow-reader limits, accepted residue (validated).** Two bounded imprecisions stay, by ruling:
  after a write timeout a held reply released in the instant between the writer's stop signal and
  its byte release is counted `responded` rather than `abandoned` (stats only; the client sees the
  same close), and the outbound queue may exceed `OUTBOUND_BYTES` by the connection's own small
  answers, at most about 34 of them under the 16 KiB inbound limit (about 0.55 MiB).
- **Deviations in T11.b, as built.** `quantise_map(&RawDensityMap, MapView, CodeDepth)` takes the
  protocol's `MapView`, since plan 02's `galaxy::map` was not yet in the tree; only the floor's span
  depends on it, so T11.c's `MapKey` may hold either view type. The depth is `CodeDepth`
  (`Eight`, `Sixteen`; `bits()`, `max_code()`), read from the wire by `TryFrom<u8>` with
  `ParseCodeDepthError`, which is T14.c's `bits` check; plan 07's `quantise_map_with_floor` should
  take a `CodeDepth` too, and can reuse the private `code_of(value, floor, span, max)`.
  `RawDensityMap`'s fields are private with getters of the same names, and
  `RawDensityMap::new(width_px, height_px, centre_ly, ly_per_px, log10)` returns
  `BuildRawMapError` for a zero size, a value count other than width × height, a pixel size not
  finite and positive or a centre not finite, and a NaN or +∞ value; `HeapBytes` charges the
  grid's capacity. `QuantisedMap` carries its depth and has `depth()`, `floor_log10_per_ly2()`,
  `ceiling_log10_per_ly2()`, `bytes()` and `to_base64()`. The ceiling is the largest finite `f32`
  widened to `f64`, the span is `ceiling − floor` as the decoder has it, and ties round away from
  zero. T11's acceptance command runs as `cargo test -p hyperion-server -- density_map galaxies`
  (cargo takes one filter before `--`).
- **Deviations in T14.a, as built.** Plan 02's `Galaxy::new` (P02.T9) was not yet in the tree, so
  by the orchestrator's ruling `open_universe` checks and answers but does not warm the galaxy:
  T11.a makes `requests::universe::open` async (taking `Arc<AppState>` and the request by value)
  and awaits `GalaxyCache::get(universe.key())` between the lookup and the answer. The lookup is
  `requests::universe::openable_universe(&AppState, &UniverseIdHex)`, giving `unknown_universe`
  (naming `universe`), `generator_version_mismatch` (both versions in the message) or
  `unsupported_save_format`; T14.b–d call it before converting any other field, add their kind to
  `tests/universes.rs`'s mismatch test, which covers `open` alone until then, and remove it from
  `the_handlers_refuse_every_kind_until_its_handler_exists`. `convert.rs` has
  `ConvertRequestError::new(field, reason)` (message `invalid <field>: <reason>`), `NewUniverse`
  (`TryFrom<CreateUniverseRequest>`, `into_parts()`), `From<&Universe> for UniverseInfo`,
  `universe_list`, and `From<CreateUniverseError>` and `From<OpenUniverseError>` for `RequestError`:
  `name_taken` names `name`, a failed write is `storage_failed`, and a failed draw, no free ID or
  an interrupted create is `internal`. T7's `UniverseRegistry::create` now takes a `UniverseName`,
  so the name is checked once, in `convert.rs`, and `CreateUniverseError::InvalidName` is gone. A
  create outlives a cancelled request, so the registry's blocking task logs it: `info` with ID,
  name, seed and version, a server fault at `error` with its causes. `StartServerError` gains
  `LoadRegistry`. `TestServer::restart()` starts another server on the data directory with the
  default test configuration and a fresh `test_entropy`. `tests/universes.rs` also covers a
  later-format save; `tests/websocket.rs` now uses `list_universes` as a served request.
