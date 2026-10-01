# Plan R08: Atmospheres

- **Milestone:** Rendering milestone RM4.
- **Depends on:** [R05 Terrain geometry and the descent spike](05-terrain-geometry-and-descent-spike.md)
  (its Earth cut of Hillaire's tables, which this plan generalises, and the terrain that aerial
  perspective falls on), [R07 Lit bodies, the photorealistic style and the main
  screen](07-lit-bodies-styles-and-main-screen.md) (the photorealistic style, lit bodies, the
  tone-mapping pass and per-view budgets), and [galaxy plan 14](../galaxy-generation/14-planetary-systems.md)
  (composition, pressure, temperature and gravity, P14.T13 and P14.T24.a, and the asks of R08.T1).
  Through them it reads R01–R04 and R06.
- **Brainstorm sections covered** (by heading, in
  [the brainstorm](../../brainstorming/rendering-and-planets.md)): "Atmosphere"; open question 3;
  the atmosphere rows of the budget and memory tables and the atmosphere line of the ladder under
  "Performance budget"; the "Physical tables and constants" bullet of "Testing" as far as it
  concerns atmospheres; the "Atmospheres" entry of "Decisions"; the gas-giant sentences of "The
  scales the view spans"; step 6 of "Suggested order of attack". From
  [the single-player brainstorm](../../brainstorming/single-player-experience.md), the density
  sentences of "Gravity" (the same air the flight model will drag through).

The names consumed from R01, R03, R05, R06 and R07 are those in their plans as written on
2026-09-29. They are reconciled with the code by the `revalidate-plan` skill before the first task
runs, as the galaxy plans are when their turn comes.

## Goal

When this plan is done, every body that plan 14 gives an atmosphere is drawn with one. In the
photorealistic style it is seen from the ground, from orbit and from across the system, through the
terminator, with fog on terrain at every distance. The atmosphere is computed, not chosen: the
client turns plan 14's composition, surface pressure, temperature, gravity and vertical structure,
and the aerosol and absorber inventory this plan asks plan 14 to publish, into a medium of
physically parameterised terms:

- Rayleigh scattering from each gas's measured dispersion formula and King factor;
- absorption by curves of growth;
- aerosol modes, through Mie theory, literature phase functions for non-spherical grains, or a
  mean-field aggregate model.

It draws them with Hillaire's four tables, generalised from R05's Earth cut to any number of terms
and any number of suns. Aerial perspective lies on the terrain, and the stars and the sky's band
are dimmed by the air they are seen through. Where Hillaire's analytic multiple-scattering term
drifts, at Venus- and Titan-class depths, a table baked per world in a worker replaces it. The
table depends on view direction, is solved spectrally with discrete ordinates in `f64`, and is
converted to the render channels. A cloud deck of optical depth above 10 splits the atmosphere in
two. Every baked table matches an independent, path-traced reference to 5% on a stated metric
before any atmosphere is drawn from it, and the check runs in `just ci`. Until its gate passes, a
thick world is drawn with the analytic term under the drafted label `ATMOSPHERE: APPROXIMATE`, never
presented as computed. The low setting is R05's
smaller tables, with aerial perspective on terrain only. It is built beside the high one and holds
the budget's figures on the UHD 620.

## Scope and non-goals

In scope:

- Asks of galaxy plan 14, written into it as amendment tasks:
  - the aerosol and absorber inventory, with a shape class per material, beside P14.T24.a;
  - the visible atmosphere of a gas envelope;
  - the vertical temperature structure;
  - carbon speciation and an O₂ source, without which no world has methane or oxygen.
- The medium: its terms, density profiles, channel coefficients and phase functions, as pure
  TypeScript with a CPU twin of every table for tests.
- The hydrostatic column in `f64` from plan 14's figures and vertical structure.
- Rayleigh scattering from per-gas dispersion and King factors, mixed by number fraction.
- Absorption, by per-channel curves of growth.
- Aerosols: Mie theory over a size distribution, literature phase functions for non-spheres,
  mean-field aggregates for haze, and the materials' refractive-index tables.
- Hillaire 2020's four tables over N terms and several suns. This includes the ray march for
  views from outside and for terrain beyond the aerial-perspective volume, aerial perspective on
  terrain, and the extinction of the sky's stars, band and discs through the air (R06 leaves
  this to R08).
- Surface lighting through the air: each sun's transmittance to a lit point and the sky's diffuse
  irradiance there, which R07's, R10's and R11's lit passes read (R08.T9.b).
- Thick atmospheres: the measured regime boundary, the view-dependent converged bake, the
  cloud-deck split, and the path-traced reference with the 5% gate.
- Gas giants inside the 10⁹ m boundary, and the `DiscReflectanceTable` that R07's analytic disc
  reads beyond it for a body with an atmosphere, with the read path in R07's `bodyDisc.wgsl`
  (R08.T16.b edits R07's shader, as R08.T6 edits R05's).
- The low setting, and this plan's own benchmarks, recorded.
- The view's atmosphere labels, drafted for the owner.

Non-goals:

- Clouds as a drawn layer or volume (R11), which R11 draws inside this plan's medium. A cloud deck
  of this plan's split is an optical boundary, not a drawn layer.
- Oceans, glint and rings (R11); the sky's stars, band and the local star's disc (R06), which this
  plan only attenuates; lit surface shading and the tone-mapping pass (R07).
- Refraction of rays, scintillation, airglow, aurorae, lightning, night-side thermal emission and
  polarised radiance in the drawn image. The reference tracer has a Stokes mode for validation only
  (R08.T12.a).
- Drag and the thermosphere. The flight model has no plan. The vertical structure asked of plan 14
  serves it too (Design note 3), and a thermosphere above the render column is the flight model's.
- Giant cloud bands. No plan generates them (R07's Risks); they are not asked here.
- Any change to generated output. Plan 14's asks are plan 14's tasks and bumps.

## Provides

TypeScript paths are under `apps/hyperion/src/renderer/src/view/atmosphere/`, the directory R05
creates, unless a path says otherwise. Signatures are sketches. R05's names are kept: this plan
widens them rather than adding parallel ones.

### The medium (`medium.ts`, `column.ts`), widening R05's

```ts
/** The render channels' wavelengths for smooth terms, nm: fitted, provisional (Design note 5). */
export const CHANNEL_WAVELENGTHS_NM: readonly [number, number, number]; // 620, 540, 445 until R08.T4 refits
/** The 15 bins every off-frame bake is solved in, nm: centres 392.67 + 25.33 k, k = 0..14, each
 * 25.33 nm wide over 380–760 (Design note 5); R06's `sky::colour::BAKE_WAVELENGTHS_NM` mirrors it. */
export const BAKE_WAVELENGTHS_NM: ReadonlyArray<number>;
// `Rgb` is the one type R05, R07 and this plan share.

export type DensityProfile =
  // R05's union, which gains `tabulated`
  | R05ExponentialOrTent
  | {
      readonly kind: "tabulated";
      readonly altitudesM: Float64Array;
      readonly relative: Float64Array;
    };
export type PhaseFunction =
  | { readonly kind: "rayleigh"; readonly depolarisation: Rgb } // ρ per channel, Design note 4
  | { readonly kind: "cornetteShanks"; readonly asymmetry: number } // R05's Earth reference only
  | { readonly kind: "tabulated"; readonly table: PhaseTable }; // Mie, literature, aggregate
export interface PhaseTable {
  readonly u: Float64Array; // u = √(θ/π), 256 entries, dense near forward
  readonly values: readonly [Float64Array, Float64Array, Float64Array];
}
export interface MediumTerm {
  // R05's fields, same meaning
  readonly name: string;
  readonly density: DensityProfile;
  readonly scattering: Rgb; // m⁻¹ at relative density 1
  readonly absorption: Rgb;
  readonly phase: PhaseFunction;
  readonly absorber: AbsorberSpec | undefined; // an absorber, whose curves are per sun, Design note 5
  readonly spectral: SpectralTerm | undefined; // the same term at BAKE_WAVELENGTHS_NM, for bakes
}
export interface AtmosphereMedium {
  // R05's, widened
  readonly referenceRadiusM: number;
  readonly referenceGravityMS2: number; // g_ref = √(g_e g_p), the column's gravity (Design note 17)
  readonly slicing: OblateSlicing; // κ slices and latitude bands; one of each for Earth
  readonly topAltitudeM: number;
  readonly groundAlbedo: Rgb; // or the deck's, under a split
  readonly terms: ReadonlyArray<MediumTerm>;
  readonly regime: AtmosphereRegime;
}
export type AtmosphereRegime =
  | { readonly kind: "thin" }
  | { readonly kind: "thickScattering"; readonly bakeKey: string }
  | { readonly kind: "cloudDeck"; readonly deckAltitudeM: number; readonly bakeKey: string };
// `DensityProfile`'s `tabulated` variant: R08.T3.a. `BAKE_WAVELENGTHS_NM`: R08.T4.a.

export type TemperatureProfile =
  | { readonly kind: "isothermal"; readonly temperatureK: number } // provisional seam, removed with P14.T24.e
  | {
      readonly kind: "radiativeConvective";
      readonly surfaceK: number;
      readonly surfacePa: number;
      readonly beta: number;
      readonly skinK: number;
    };
export function temperatureAt(profile: TemperatureProfile, pressurePa: number): number; // the sim's formula, copied
export function hydrostaticColumn(input: ColumnInput): AtmosphereColumn; // f64, DN 3, at g_ref
export const ATMOSPHERE_OPTICS_VERSION: number; // client caches key on it, not the generator
```

### Oblate bodies (`oblate.ts`)

```ts
/** The level spheroid: R07's `BodyFigure` (a, c), GM = G × the record's `mass_kg` (CODATA G), ω
 * from P14.T14.c's `BodyFixedFrame` rate (R05 Design note 14's test-planet period until then). */
export interface LevelSpheroid {
  readonly equatorialRadiusM: number;
  readonly polarRadiusM: number;
  readonly gmM3S2: number;
  readonly angularVelocityRadS: number;
}
/** Somigliana's closed-form normal gravity at geodetic latitude φ, m s⁻² (Design note 17). */
export function normalGravity(body: LevelSpheroid, geodeticLatitudeRad: number): number;
export function referenceGravity(body: LevelSpheroid): number; // √(g_e g_p)
/** 1/R_α = cos²α ÷ M + sin²α ÷ N, α the azimuth from north, m. */
export function directionalCurvatureRadiusM(
  body: LevelSpheroid,
  geodeticLatitudeRad: number,
  azimuthRad: number,
): number;
export interface OblateSlicing {
  readonly kappa: Float64Array; // κ_k of the transmittance slices, ascending, Δln κ ≤ KAPPA_STEP
  readonly bandGravityRatio: Float64Array; // s_b of the latitude bands, Δln s ≤ BAND_STEP
}
export function oblateSlicing(body: LevelSpheroid, referenceRadiusM: number): OblateSlicing;
export const KAPPA_STEP = 0.15; // widened to 0.3 over the 2 MB row (Design note 11)
export const BAND_STEP = 0.1; // widened to 0.15 over the 2 MB row
export const ONE_SLICE_BELOW = 0.02; // ln(κ_max ÷ κ_min) under which one slice and one band serve
```

### Optics (`rayleigh.ts`, `absorbers.ts`, `mie.ts`, `sizeDistribution.ts`, `aggregate.ts`, `materials/`)

```ts
export type Gas = "H2" | "He" | "H2O" | "CH4" | "NH3" | "N2" | "O2" | "CO2" | "Ar"; // plan 14's `Gas`
export interface Dispersion {
  readonly nMinusOne: (wavelengthNm: number) => number;
  readonly referenceK: number;
  readonly referencePa: number;
  readonly source: string;
}
export const GAS_DISPERSION: Readonly<Record<Gas, Dispersion>>;
export function kingFactor(gas: Gas, wavelengthNm: number): number;
export function rayleighCrossSectionM2(gas: Gas, wavelengthNm: number): number; // R08.T3.b
export function molecularTerm(column: AtmosphereColumn, fractions: GasFractions): MediumTerm; // T3.c
export interface AbsorberCurve {
  // one absorber under one sun, computed at arrival (Design note 5)
  readonly logColumns: Float64Array;
  readonly transmittance: readonly [Float64Array, Float64Array, Float64Array];
}
export function absorberCurve(absorber: AbsorberSpec, spectrum: BakeSpectrum): AbsorberCurve; // R08.T4.b
export function absorberTerm(absorber: AbsorberSpec, column: AtmosphereColumn): MediumTerm; // R08.T4.b
export interface MieResult {
  qExt: number;
  qSca: number;
  asymmetry: number;
  s1: Float64Array;
  s2: Float64Array;
}
export function mieSphere(sizeParameter: number, index: ComplexIndex, mu: Float64Array): MieResult;
export function modeOptics(mode: AerosolMode, wavelengthNm: number): ModeOptics; // over its size distribution
export function aggregateOptics(mode: AggregateMode, wavelengthNm: number): ModeOptics; // Tazaki–Tanaka MMF
export function refractiveIndex(material: AerosolMaterial, wavelengthNm: number): ComplexIndex;
export function aerosolTerm(mode: AerosolModeSpec, column: AtmosphereColumn): MediumTerm; // R08.T5.c
```

Beside them: `channels.json` (the fitted triple and its objective, R08.T4.a),
`absorbers/crossSections.json` (R08.T4.b), and `bless.ts`, the client's bless helper, which
rewrites a committed table only under `HYPERION_BLESS=1`, the variable the Rust testkit already
reads (R08.T4.a).

### Assembly, workers and cache (`assemble.ts`, `opticsWorker.ts`, `AtmosphereCache.ts`)

```ts
export function assembleMedium(input: BodyAtmosphereInput): AssembledMedium;
export interface AssembledMedium {
  readonly medium: AtmosphereMedium | undefined;
  readonly labels: ReadonlyArray<AtmosphereLabel>;
}
export type AtmosphereLabel =
  | "atmosphereNotResolved" // the surface section is withheld
  | "atmosphereNotYetModelled" // the surface section is `not_modelled`
  | "aerosolsNotYetModelled" // no aerosol or absorber inventory
  | "atmosphereComputing" // a gated thick bake is running
  | "atmosphereApproximate"; // a thick world whose gate has not passed, or an oblate body drawn
// with one slice before R08.T6.f (Design note 17)
export class AtmosphereCache {
  /* per body and inventory hash; requests `body_detail`; posts work to the optics workers */
}
```

`labels.ts` holds the drafted strings (R08.T2).

### Settings (`settings.ts`)

R05's `SETTINGS` stays the one list (R05 Design note 26): this plan adds one field to R05's
`ViewSettings` and its values to `SETTINGS`, and no second list. `settings.ts` holds the field's
type, the values it contributes, and named projections of them, as R05's `TERRAIN_SETTINGS` is of
`SETTINGS[s].terrain`.

```ts
export interface AtmosphereViewSettings {
  /** How many suns' skies a photorealistic view draws, brightest first (Design note 7). */
  readonly skySunCap: number; // high 4, low 2; R08.T7
  /** Where aerial perspective applies (Design note 11). */
  readonly aerialPerspectiveScope: "opaque" | "terrain"; // high "opaque", low "terrain"; R08.T9.a
  /** The quality limits this plan lists for R12's audit, one string each. */
  readonly qualityLimits: ReadonlyArray<string>;
}
// R05's `ViewSettings` gains `atmosphereView: AtmosphereViewSettings`; `SETTINGS[s].atmosphereView`
export const ATMOSPHERE_VIEW_SETTINGS: Readonly<Record<QualitySetting, AtmosphereViewSettings>>;
export const SKY_SUN_CAP: Readonly<Record<QualitySetting, number>>; // SETTINGS[s].atmosphereView.skySunCap
export const AERIAL_PERSPECTIVE_SCOPE: Readonly<Record<QualitySetting, "opaque" | "terrain">>;
export const ATMOSPHERE_QUALITY_LIMITS: Readonly<Record<QualitySetting, ReadonlyArray<string>>>;
```

R05's `TABLE_SIZES`, which is `SETTINGS[s].atmosphere`, stays in R05's module and gains the thick
table's size (R08.T14.c). `settings.ts` is the module R12 reads for this plan's ladder entries.

### Tables and passes (`shaders/`, `hillaire.ts`), widening R05's `HillaireAtmosphere`

- R05's WGSL compute kernels, widened to read a storage buffer of N terms, a 2D array texture of
  density tables (one layer a term) and one of phase tables:
  - `transmittance.wgsl`, which stores optical depth, curves of growth included (Design note 8),
    in a 2D array texture of one layer a κ slice (Design note 17);
  - `multiScattering.wgsl`, Hillaire's isotropic 32² table for `thin` worlds, one layer a latitude
    band;
  - `skyView.wgsl`;
  - `aerialPerspective.wgsl`.

  Beside them, `rayMarch.wgsl` is the fragment pass for views from outside and for terrain beyond
  the aerial-perspective reach, and `composite.wgsl` is R05's composite, widened to several suns.
  New: `irradiance.wgsl`, a per-planet table of the sky's diffuse irradiance on a horizontal
  surface by altitude and sun zenith, one layer a latitude band, and `surfaceLighting.wgsl`, which
  exports `atmosphere_sun_transmittance(altitude_m, mu_sun, latitude_rad, sun_azimuth_rad)` and
  `atmosphere_sky_irradiance(altitude_m, mu_sun, latitude_rad)`, each `-> vec3f`, for the lit
  passes of R07, R10 and R11 (R08.T9.b). The latitude and azimuth arguments are Design note 17's;
  `altitude_m` is the geodetic height, scaled inside. `oblate.wgsl` holds the shared WGSL: normal
  gravity, R_α, κ and the slice and band reads. Every kernel is registered in R01's
  `WGSL_CATALOGUE` and created through `RenderEngine.createCompute(KernelPair)`, with no subgroup
  variant.

- `tablesCpu.ts` is a CPU twin of every kernel in `f64`, built on R05's `opticalDepth.ts` oracle,
  which it extends rather than replaces. It is the test oracle and the smoke harness's comparison.
- `HillaireAtmosphere` in `hillaire.ts` is R05's class, widened:
  - `setMedium(medium)` builds the per-planet tables, shared by every view on the device, one
    transmittance layer per κ slice and one multiple-scattering and irradiance layer per band of
    `medium.slicing`;
  - `drawFrame(view, suns: ReadonlyArray<SunState>)` builds the per-view tables, summed over the
    drawn suns (Design note 7);
  - `aerialPerspective(view)` exposes the volume and the transmittance table to R11, the table as
    its 2D array texture of κ slices, read through `oblate.wgsl`'s slice lookup;
  - `skyView(view): SkyViewTable` exposes the view's sky-view table, the sky's radiance by local
    azimuth and elevation summed over the drawn suns, for R11's sky reflection in the ocean. If
    R08.T7's check forces the per-sun fallback, it returns the per-sun tables with their suns
    instead, in a field of the same type.
- `TABLE_SIZES: Record<QualitySetting, TableSizes>` is R05's, kept. The thick table's size is
  added.

### Thick atmospheres (`thick/`)

```ts
export interface ThickBoundary {
  readonly omega: Float64Array; // single-scattering albedo grid
  readonly g: Float64Array; // asymmetry grid
  readonly tauStar: Float64Array; // vertical extinction τ* at each (ω, g), row-major
}
export const THICK_MS_BOUNDARY: ThickBoundary; // measured in R08.T13, Design note 9
export const CLOUD_DECK_SPLIT_OPTICAL_DEPTH = 10; // the brainstorm's, open question 3
export const BAKE_CEILING_S = 5; // per world on the UHD 620 host, Design note 9
export interface BodyCover {
  readonly cloudFraction: number; // P14.T24.b's, 0–1
}
export function classifyRegime(medium: AtmosphereMedium, cover: BodyCover): AtmosphereRegime; // per body, Design note 9
/** J_ms(h, μ₀, μ_v, m), m = 0..1: the source function of orders two and higher, spectral in,
 * channels out, for one latitude band of `medium.slicing` (Design note 17). f64, in a worker. */
export function bakeMultipleScattering(medium: AtmosphereMedium, band: number): MsSourceTable;
/** Above: the deck as a reflecting boundary; below: downwelling radiance. One latitude band. */
export function bakeDeck(medium: AtmosphereMedium, deckAltitudeM: number, band: number): DeckTables;
```

Files: `thick/regime.ts` (R08.T13), `thick/discreteOrdinates.ts` and `thick/bake.ts` (R08.T14),
`thick/deck.ts` (R08.T15).

### For R07's analytic disc (`discReflectance.ts`)

`bakeDiscReflectance(medium, appearance: BodyAppearance) -> DiscReflectanceTable` is this plan's
name; `DiscReflectanceTable` is the name R07 consumes. It gives the reflectance by phase angle and
disc position per channel, baked spectrally, one layer a latitude band of the medium's slicing,
read at each disc pixel's geodetic latitude (Design note 17). R07's `shaders/bodyDisc.wgsl` reads
it beyond
`GAS_GIANT_FULL_PASS_BOUNDARY_M` for a body with an atmosphere, in place of its albedo-only
shading; R08.T16.b adds that read path to R07's shader.

### Offline reference (`crates/hyperion-fit`)

```rust
// crates/hyperion-fit/src/atmosphere/
pub struct AtmosphereCase { /* a spectral medium, as the TypeScript writes it, its `Shells`,
    and its geometry set */ }
pub enum Polarisation { Scalar, Stokes }                 // Stokes for Rayleigh benchmarks only
pub enum Shells {                                        // Design note 17
    Sphere { radius_m: f64 },
    /// The level spheroid; density at the gravity-scaled height h·g(φ)/g_ref, by delta tracking.
    Spheroid { equatorial_radius_m: f64, polar_radius_m: f64, gm_m3_s2: f64, omega_rad_s: f64 },
}
pub struct ReferenceRadiances { /* per geometry and wavelength: radiance, standard error, samples,
    detector cone; the two flux aggregates */ }
pub fn trace_reference(case: &AtmosphereCase, polarisation: Polarisation, samples: NonZeroU64,
    threads: NonZeroUsize) -> ReferenceRadiances;        // backward Monte Carlo, f64, over `Shells`
```

Command: `hyperion-fit atmosphere-reference <case.json> --out <reference.json> [--samples N]
[--threads N] [--stokes]`. Fixtures:
`view/atmosphere/reference/{earth,earth-ozone,mars,venus-92bar,venus-58bar,titan-class,giant-deck,saturn-oblate}.case.json`
and their `.reference.json`. The metric is `gate.ts` (R08.T12.b).

## Consumes

Names are those the owning plans give; the owning plan is authoritative.

- **Galaxy plan 14:**
  - P14.T13's `derive::atmosphere::{Atmosphere, Gas, PartialPressures, SurfaceState}` (built:
    `crates/hyperion-sim/src/planetary/derive/atmosphere.rs`). As built it fills only H₂O, CO₂,
    N₂ and Ar, and gives its Venus 58 bar (Design note 16).
  - These reach the client through P14.T24.a's `SurfaceConditions` in the record's surface
    section. That section is today the sim's empty `record::Surface`
    (`crates/hyperion-sim/src/planetary/record.rs`) and the wire's empty `BodySurfaceDto`
    (`crates/hyperion-protocol/src/planetary/record.rs`), filled by P14.T35 when T13, T14 and T24
    land, and by the amendment P14.T35.d of R08.T1 for T24.c–f.
  - The `body_detail` request and its `BodyDetailDto` (P14.T35.b–c, built:
    `crates/hyperion-protocol/src/envelope.rs`), which is the only message carrying the surface
    section: the scene's `BodySummaryDto` carries the `orbit`, `bulk`, `moons` and `rings`
    sections alone. The section's state (`ok`, `not_resolved`, `not_modelled`, `not_applicable`)
    is how this plan learns the granted detail for a body. `body_events` (P14.T35.c, after T31)
    says when a body's figures change with time.
  - P14.T24.b's cloud fraction, for `classifyRegime`'s per-body rule (Design note 9).
  - The bulk section's radius and surface gravity (built: `BulkPropertiesDto.radius_m` and
    `surface_gravity_m_s2`).
  - The body's mass, for `oblate.ts`'s GM = G·M (built). It is not in the bulk section: ruling 53
    of 2026-09-22 shows it at the `mass_and_orbit` level in a section of its own,
    `BodySummaryDto.mass_kg` and `BodyRecordDto.mass_kg` (`SectionDto<f64>`, kg), the sim's
    `BodyRecord` mass section.
  - The body's rotation rate, for `oblate.ts`'s ω (not built): P14.T14.a–b's spin and
    P14.T14.c's `BodyFixedFrame { pole, w0, rate }` (`planetary/frames.rs`), the rotation of the
    record's surface section, which reaches the client with it through `body_detail` (P14.T35).
    R05 Design note 14 reads no rotation from plan 14: until P14.T14.c exists R05's test planet
    carries its own fixed pole and 86,164.0905 s sidereal period, which is this plan's ω for R05's
    Earth. A body with no rotation yet takes R08.T3.d's one-slice fallback.
  - `DetailLevel::Surface`, and `Section::{NotResolved, NotModelled, NotApplicable}` (built).
  - The asks of R08.T1: P14.T24.c–f and P14.T35.d.
- **R01:**
  - `RenderEngine.createCompute(KernelPair)`;
  - `WGSL_CATALOGUE` in `view/engine/catalogue.ts`;
  - the headless SwiftShader smoke harness (`src/smoke/`, `just test-render`), with its no-f16
    run. It stays outside `just ci` (R01.T9.e), so every task that adds or changes a catalogued
    kernel runs `just test-render` as part of its own gate.
- **R02:**
  - camera-relative `f64` differencing and the rotation-only view matrix;
  - reversed-Z and the transparent-layer order: the atmosphere tests depth and writes none;
  - the photometric pipeline: V = 0 at 2.54 µlx, pre-exposure, `rgba16float` targets;
  - `ViewLabelBlock`, and the nine guide items it drafted, of which items 2 and 7 bear on this
    plan.
- **R03:** `sceneAt(model, observer, time) -> SceneFrame`, with each body's `geometricM`,
  `apparentM` and `level` (its granted `DetailLevelDto`, per body since R03 Design note 13), and
  the ship's local body named. A change of `level` triggers a rebuild (Design note 14). The scene's
  `SceneBodyDto.record` is a `BodySummaryDto`, so the surface section itself still comes from
  `body_detail`, and its section state decides the label.
- **R05:** `view/atmosphere/` as its Design note 16 builds it:
  - `MediumTerm`, `DensityProfile`, `EARTH_REFERENCE`, `HillaireAtmosphere` (Bevy 0.19's WGSL
    port, with sebh's reference) and `TABLE_SIZES: Record<QualitySetting, TableSizes>`, which is
    `SETTINGS[s].atmosphere` of R05's one settings list (R05 Design note 26). This
    includes its low sizes: sky-view 128 × 64, aerial perspective 32 × 32 × 16, the march at
    half resolution, and aerial perspective in one deferred pass on terrain alone.
  - Design note 16's lookups over the rotational spheroid: r = √(MN) + h with h the geodetic
    height, μ against the spheroid normal, and the march clipped against the spheroid shells
    (researched 2026-09-29), which this plan's widened tables keep.
  - `atmosphereInputs(camera, figure)` in `hillaire.ts` (R05.T12.c), Design note 16's inputs at
    the camera, which R08.T6.f widens in R05's file with the camera's latitude and s.
  - The `MemoryCategory` members `atmosphere-tables` and `atmosphere-view` that R05.T12.b and T12.c
    add under R12's names. This plan allocates every per-planet table under the first, the gated
    thick bakes included, and every per-view table, summed over suns, under the second (R12
    Design note 6).
  - The terrain pass (`view/terrain/terrainPass.ts`).
  - `HeightWorkerPool`'s module-worker and transfer pattern.
  - The spike's `metrics.ts` and `just descent-spike`.
- **R06:**
  - `HostDiscDto` (R06's wire form of `HostDisc`), carrying each host star's `StarColour` fields
    (`chroma`, `lux_per_v0`) and `bake_spectrum: [f64; 15]`. The last is built by R06.T3.c and
    T10: the colour row's spectrum averaged over each bin of `BAKE_WAVELENGTHS_NM` and normalised
    to unit photopic illuminance by the 15-bin sum (Σ 683 ȳᵢ Sᵢ Δλ = 1 lx), so a bake's luminance
    scales by the star's lux. R06 has no TypeScript `StarColour`; this plan reads the fields off
    `HostDiscDto`.
  - The sky layers (`SkySprites`, `BandLayer`, `HostDiscLayer`), which this plan dims by
    transmittance.
  - **Asked of R06 (open, outside R06's scope):** a 1 nm model spectrum per colour-table row for M
    stars, for the curves of growth (Design note 5, Risks).
- **R07:**
  - `starIlluminance(disc: HostDiscDto, distanceM: number): Rgb`, with `Rgb` from R02's
    `view/photometry`, the one per-channel type R05, R07 and this plan share;
  - `BodyAppearance` (with `BodyPhotometry`), which R08 reads per body;
  - `litRegimes` with `GAS_GIANT_FULL_PASS_BOUNDARY_M`;
  - `shaders/bodyDisc.wgsl`, which consumes `DiscReflectanceTable`, and into which R08.T16.b adds
    the read path;
  - `shaders/litBody.wgsl`, which calls R08.T9.b's `surfaceLighting.wgsl` through stubs returning
    1 and 0 until this plan lands, for each sun's transmittance and the sky's irradiance;
  - `METER_CLASS` (R07 Design note 10): every translucent pass of this plan into the HDR target
    blends its alpha with source factor zero and destination factor one, so that R07's meter class
    survives, through R01's `blend` modes `"premultiplied"` and `"additive"`, both of which do so
    (R01 Design note 21, R01.T8.i);
  - `photorealisticPasses`, with its empty slot for R08, and `viewBudgets`;
  - `STAR_CUT_RELATIVE` (1e-4, R07 Design note 4), the cut of a star under 10⁻⁴ of the brightest,
    applied here as the same constant.
- **R10 and R11 (consumers):** R10's `terrainLit.wgsl` (R10.T10.b) takes the direct sun through
  `atmosphere_sun_transmittance` and the sky through `atmosphere_sky_irradiance`, scaled by its
  `terrain_sky_factor`. Clouds, oceans and rings are drawn inside this plan's medium,
  through `HillaireAtmosphere.aerialPerspective(view)` and `skyView(view)`; lit terrain, clouds,
  oceans and rings take their light through `surfaceLighting.wgsl`. R11's boundary of a cloud deck
  (a body whose cloud fraction is 1 and whose deck exceeds τ 10) is `classifyRegime`'s rule.
- **R12:** the consolidated performance runs and the ladder, which audit this plan's low setting
  through `settings.ts`.

## Design notes

1. **Where each part runs.** The brainstorm says the client turns the inventory into phase
   functions and coefficients, and that a thick atmosphere's table is "baked offline for that
   atmosphere … and cached per world".
   - Worlds are generated, so for a world "offline" means off the frame loop: at arrival, in
     workers, cached per world. The only work done at development time is the reference of Design
     note 10.
   - The optics, the medium, the tables and the bakes are client code, and the server gains
     nothing. The renderer stays a display sink.
   - The client code is TypeScript in module workers, not Rust in WebAssembly, for three reasons:
     - a JavaScript number is an `f64`, which the solver needs;
     - the atmosphere is presentation, outside the determinism discipline, so nothing needs to stay
       equal between client and server;
     - a new Rust crate would contradict the brainstorm's five `clippy.toml` files, and putting the
       optics in `hyperion-surface` would put presentation in a crate whose wasm "holds the height
       function alone".
   - It also keeps this plan off the `'wasm-unsafe-eval'` sign-off.
   - Research measured Mie in TypeScript at about 40–800 ms a mode (R08.T5.a; provisional, taken
     under shared load), which confirms the language is adequate.
2. **The medium is R05's list of terms, widened.** A term is R05's density profile, scattering and
   absorption coefficients per channel and phase function. A new gas or aerosol is a new term,
   never a new model.
   - R05's `DensityProfile` gains a `tabulated` variant, relative density against altitude. The
     structure of Design note 3 is a polytrope, not an exponential, and condensate and dust layers
     have a base and a top. The kernels read the tables from a 2D array texture, one row a term,
     on a square-root altitude spacing.
   - The molecular gases share one term, with the number-fraction mixture of their cross-sections.
   - Every aerosol carries its phase function tabulated (Design note 6).
   - Cornette–Shanks is kept only for R05's `EARTH_REFERENCE`.
   - A term also carries its `spectral` form at `BAKE_WAVELENGTHS_NM` for the off-frame bakes, and
     an absorber carries its species and column, from which each sun's curve of growth is
     computed.
3. **The vertical structure** (researched 2026-09-29, a physics ruling; Robinson and Catling 2012,
   ApJ 757, 104, and 2014, Nature Geoscience 7, 12). The profile is a scaled adiabat from the
   surface to an isothermal skin: T(p) = max(T_s (p/p_s)^β, T_skin).
   - β = α·R/c_p. R/c_p comes from kinetic theory with fixed degrees of freedom, as R&C 2012 take
     it (γ = 1 + 2/N, their Eq. 9), mixed as c_p = Σxᵢc_p,ᵢ, never by averaging γ (researched
     2026-09-29). The values: 2/7 = 0.286 for H₂, N₂ and O₂ (H₂'s frozen rotation below about
     150 K is a recorded caveat); 0.400 for He and Ar; 3/13 = 0.231 for CO₂, R&C's γ = 1.3; 0.25
     for H₂O, CH₄ and NH₃ (N = 6, medium confidence for the last two). NIST's values at 298 K
     (0.288, 0.286, 0.283, 0.400, 0.400, 0.224, 0.248, 0.233, 0.237) agree within about 7%. A
     temperature-dependent c_p is not used: it breaks the constant-β form, and R&C calibrated α
     against their constant γ.
   - α scales the dry adiabat for latent heat, by the condensing species: 0.6 with a water ocean,
     0.77 for methane, 0.8 dry or CO₂, and 0.83–0.94 for giants (R&C 2014 Table 1; R&C 2012 §4.1
     for Venus).
   - T_skin = 2^(−1/4)·T_eq is the τ → 0 limit of the same grey Eddington atmosphere as P14.T13.c,
     so the two plans agree at both ends.
   - The checks, with their inputs:
     - Earth (T_s 288 K, p_s 1 bar, α 0.6, β 0.171, T_skin 214.4 K): tropopause 0.179 bar at
       214 K, against about 0.16–0.23 bar and 217 K observed (R&C 2014 Table 1; US Standard
       Atmosphere 1976, 0.226 bar at 216.65 K).
     - Venus (T_s 730 K, p_s 92 bar, α 0.8, R/c_p 3/13, β 0.1846): 316.8 K at 1 bar, against about
       350 K (VIRA, Seiff et al. 1985). At plan 14's 58 bar the same β gives about 345 K.
     - Titan (T_s 94 K, p_s 1.4 bar, α 0.77, R/c_p 2/7, β 0.22, T_skin 64 K): tropopause 0.24 bar
       at 64 K (0.26 bar at the real 1.5 bar), against 0.1–0.2 bar and 70 K (R&C 2014 Table 1;
       Huygens HASI).

   Plan 14 publishes the parameters (T_s, p_s, β, T_skin), and for giants its envelope form (R08.T1,
   P14.T24.e), with `planetary::temperature_at(structure, p)` in the sim for the flight model's
   drag. The client copies the formula in `temperatureAt`, tested against golden levels from the
   sim, so the picture and the drag read the same air. The column integrates the hydrostatic
   equation in `f64` from p_s upward, with g = g₀(R/r)², the local kT/(μ m_u g) at each step, and a
   top at p_s × 10⁻⁷.

   Until P14.T24.e lands the `isothermal` seam at T_s stands. It is named provisional, and it is
   wrong by about 3× in scale height at Venus's cloud tops (15.7 km at 735 K against 5.2 km at
   240 K). The adopted profile is itself provisional in the stratosphere: its skin is isothermal,
   with no ozone or haze heating, so Titan lacks its 170 K stratosphere. R&C 2012's shortwave
   stratospheric term is the upgrade once P14.T24.c exists.

4. **Rayleigh from each gas's measured dispersion** (researched 2026-09-29, a physics ruling). Every
   gas takes the same route:
   - a measured n − 1 at the formula's own stated temperature and pressure;
   - Lorentz–Lorenz to that state's number density;
   - σ = 24π³/(λ⁴N²)·((n² − 1)/(n² + 2))²·F_K.

   | Gas     | n − 1                                                                                                   | King factor                                                                       | Reference state      |
   | ------- | ------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------- | -------------------- |
   | Dry air | Peck and Reeder 1972, JOSA 62, 958 (the Earth check only)                                               | Bates 1984 per gas, mixed after Bodhaine et al. 1999                              | 288.15 K, 101,325 Pa |
   | N₂      | Peck and Khanna 1966, JOSA 56, 1059; below 468 nm, the Bates 1984 branch that Sneep and Ubachs 2005 use | Bates 1984                                                                        | 288.15 K             |
   | O₂      | Zhang, Lu and Wang 2008, Appl. Opt. 47, 3143                                                            | Bates 1984                                                                        | 293.15 K             |
   | Ar      | Peck and Fisher 1964, JOSA 54, 1362                                                                     | 1                                                                                 | 288.15 K             |
   | CO₂     | Bideau-Mehu et al. 1973                                                                                 | Sneep and Ubachs 2005                                                             | 273.15 K             |
   | CH₄     | Sneep and Ubachs 2005, JQSRT 92, 293                                                                    | 1                                                                                 | 288.15 K             |
   | H₂O     | Ciddor 1996, Appl. Opt. 35, 1566, Eq. 3                                                                 | 1.001                                                                             | 293.15 K, 1,333 Pa   |
   | NH₃     | Cuthbertson and Cuthbertson 1914                                                                        | Hohm 1993, Mol. Phys. 78, 929                                                     | 273.15 K             |
   | H₂      | Peck and Huang 1977, JOSA 67, 1550                                                                      | Hohm 1993; Dalgarno and Williams 1962, which runs 6.5% low, is only a cross-check | 273.15 K             |
   | He      | Mansfield and Peck 1969, JOSA 59, 199; Chan and Dalgarno 1965 agrees to 0.4%                            | 1                                                                                 | 273.15 K             |

   The formulas' machine-readable copies are refractiveindex.info's (CC0), cited beside each paper.
   The reference state is read from each primary paper and pinned by a test. Reading Bideau-Mehu's
   CO₂ at 288.15 K instead of 273.15 K raises σ by 11%, a trap three public codes split on.

   The mixture has σ_mix = Σxᵢσᵢ and F_mix = Σxᵢσᵢ / Σxᵢ(σᵢ/F_i), with ρ = 6(F − 1)/(3 + 7F) in the
   phase function (Chandrasekhar; Bucholtz 1995).

   The brainstorm's figures are confirmed by recomputation, and are tests:
   - Earth's 4.848, 11.487 and 28.710 × 10⁻⁶ m⁻¹ at 680, 550 and 440 nm;
   - Bucholtz's 4.510 × 10⁻²⁷ cm²;
   - a 92-bar CO₂ Venus column has τ_R(550) ≈ 16 (and about 41 in the blue).

   The tutorials' 5.8, 13.5 and 33.1 × 10⁻⁶ m⁻¹ appear nowhere. The dense-gas fluctuation factor,
   about 1.06 at Venus's surface, is a recorded omission.

5. **Channels, spectral bakes and curves of growth** (researched 2026-09-29; Bruneton 2017, arXiv
   1612.04336 §14.3; Elek and Kmoch 2010). This is a design choice, not a ruling.
   - **Channel wavelengths.** The per-frame tables stay three-channel (Hillaire, the budget).
     Smooth terms are evaluated at fitted wavelengths, not at 680/550/440, and multiplied by the
     star's linear Rec. 709 colour from R06. The 680/550/440 set is Bruneton's code constant: at
     Earth it puts low suns up to 60% too bright and 0.01–0.04 off in u′v′. A minimax fit over an
     Earth case family gives (620, 540, 445) nm, within 0.0005–0.016. R08.T4 refits it with a Mars
     dust case and records the objective. The 680/550/440 figures stay as tests of `rayleigh.ts`.
   - **Spectral bakes.** Everything baked off the frame loop is solved in the 15 bins of
     `BAKE_WAVELENGTHS_NM` (centres 392.67 + 25.33 k nm) over 380–760 nm and converted to linear
     Rec. 709 on storage, through the CIE matching functions times the star's spectrum. That is
     Bruneton's "precomputed illuminance" mode, within Δu′v′ ≤ 0.001 of the research agent's
     spectral reference. The bakes are the thick multiple-scattering table, the deck tables,
     `DiscReflectanceTable` and the reference. Transmittance multiplies, so it cannot be
     pre-converted and stays three-channel. At Venus depths every three-sample triple is 0.04–0.08
     off in u′v′, so the thick bakes cannot skip this. It reads each star's spectrum in the bake
     bins from R06's `HostDiscDto.bake_spectrum`.
   - **Curves of growth** (researched 2026-09-29, medium confidence, from a synthetic-band model).
     Absorbers take a per-channel curve of growth, not a band-averaged cross-section, which
     Jensen's inequality biases dark and which fails for saturated narrow bands such as methane's on
     a Neptune. Per absorber, channel and sun, T_c(u) = ∫ w_c S e^(−σ(λ)u) dλ / ∫ w_c S dλ is
     tabulated on a log grid of column u.
     - **Per sun, at arrival.** The curve depends strongly on the star: 300 DU of ozone gives a
       red-channel depth of 0.033 under a 2,500 K star against 0.046 under a 30,000 K one, and
       methane's red transmittance through 300 km-am runs 0.36 to 0.20 over the same range. So
       one solar table is not used. The optics worker computes each sun's curves at arrival from
       that sun's spectrum, about 10⁶ exponentials, a few milliseconds.
     - **Storage.** The shared per-planet table cannot hold a curve per sun. So it stores the
       molecular and aerosol optical depth in RGB and the accumulated absorber column u in alpha;
       a second `rgba16float` table holds up to four more absorbers' columns. Each sun's 1-D curve
       is applied on read (Design note 8).
     - **Weights.** The weights are w_c = max(r̄_c, 0), the channel's colour-matching weight
       clipped at zero. Rec. 709's blue weight is negative over 500–620 nm, which would give
       T_B > 1 (1.003 for the Sun, 1.19–1.50 for a 2,500 K star). The clipped weights' small colour
       error in the direct beam is recorded by the spectral check.
     - **Resolution.** The integral runs on σ's own grid, 1 nm or finer. Ozone's cross-sections
       are binned to 1 nm, and Karkoschka and Tomasko's methane coefficients are used as
       published, since they are band-model values for e^(−ku) at their stated resolution.
       Interpolating S from the 15 bin averages is adequate for FGK, A and B stars (under
       3 × 10⁻³ in T_c at 2,500 K, under 10⁻⁴ for the Sun). It is not adequate for M dwarfs, whose
       TiO bands overlap Chappuis and methane's 727 nm band; that is a recorded limitation (Risks).
     - The non-additivity over segments that the curves introduce is recorded by the spectral
       check. Ozone's Chappuis band is in the linear regime, where the curve reduces to the band
       average.
6. **Aerosols come from plan 14's inventory, or not at all** (researched 2026-09-29, the shape
   class a physics ruling). The renderer must not invent haze. Each mode has:
   - a material keyed to a refractive-index table, with a shape class (sphere, non-spherical
     mineral, crystal);
   - a size distribution: Hansen and Travis 1974's gamma or a modified log-normal, by effective
     radius and variance, or a fractal aggregate's monomer radius, count and dimension;
   - a column optical depth at 550 nm and a vertical profile.

   How each shape is treated:
   - **Liquid spheres** (H₂SO₄, water, hydrocarbons) go through Mie theory: Wiscombe 1980's
     structure, with a correct downward-recurrence start. BHMIE's |mx| + 15 start is 25% wrong in
     D₁ at |mx| = 1,500.
   - **Non-spheres** take a literature phase function and single-scattering albedo per material,
     since sphere Mie puts a spurious rainbow and glory into a dusty sky. For Mars dust that is
     Wolff et al. 2009, from T-matrix cylinders; for ice, a severely roughened crystal habit. Mie
     supplies only their cross-sections, flagged in the code as an approximation.
   - **Aggregates** use Tazaki and Tanaka 2018's modified mean-field model (ApJ 860, 79), with
     optool (MIT; Dominik, Min and Tazaki 2021, ascl:2104.010) as its reference (researched
     2026-09-29). The paper validates it against T-matrix results at D_f 1.9 and 3.0 only, with
     opacities within about 20–25%. The inventory's enum is capped at D_f ≤ 2.5. As optool does,
     the phase table is kept only while the phase shift Δφ < 1 (the paper's §4.2); beyond it the
     mode keeps its opacities and takes a Henyey–Greenstein phase from its MMF asymmetry, flagged
     in the code. Titan's haze passes (Δφ about 0.4 for the aggregate).

   Each mode's density is scaled so its column τ(550) is the inventory's. The phase function is
   tabulated per channel on u = √(θ/π) with 256 entries, since dust's forward peak makes the blue
   sunset and a 1/x-wide peak starves on a cos θ grid. How the forward peak is handled:
   - Single scattering (sky-view, aerial perspective, the march) uses the full table and the full
     extinction.
   - Only the multiple-scattering bakes use delta-M truncation (Wiscombe 1977), with the scaled
     τ′ = (1 − fω)τ and Nakajima and Tanaka 1988's single-scatter correction.
   - The shared transmittance table is never truncated, which would double-count forward light.
     Only diffraction narrower than the star's disc goes into the direct beam.

   Where the surface section is granted but the inventory is `not_modelled`, the medium has no
   aerosol terms and the label block says `AEROSOLS: NOT YET MODELLED` (Design note 12).

7. **Per-planet tables are shared; per-view tables are summed over suns** (researched
   2026-09-29, medium confidence).
   - Transmittance and multiple scattering depend on the atmosphere alone, as R05's Design note 16
     already corrects the brainstorm. They are built once per planet and shared by every view and
     every sun, through this plan's `AtmosphereCache`, one per device.
   - Sky-view and aerial perspective are per view, each one table whatever the number of suns. The
     reason: in-scattered radiance is linear in each sun's illuminance, and the aerial-perspective
     volume is indexed by the camera's frustum, not by the sun. The kernels loop over the drawn
     suns inside one march along each ray, and an extra sun costs its phase function, its
     transmittance lookup and its shadow test.
   - The sky-view table is indexed by local azimuth instead of Hillaire's sun-relative longitude,
     since it is rebuilt every frame and the sun's disc is drawn separately. R08.T7 checks that
     each sun's forward peak is still resolved: the summed table must agree with per-sun tables
     in the twin to 2% within 10° of each sun. If it does not, the sky-view falls back to one
     table per sun, capped as below; aerial perspective stays summed.
   - A sun is a star whose illuminance at the body is at least 10⁻⁴ of the brightest's. This is
     R07's `STAR_CUT_RELATIVE` (its Design note 4), one constant for both plans, applied above the
     horizon or not,
     since a night side lit by a companion still has a sky.
   - The high setting draws up to four suns' skies and the low setting two, brightest first
     (`SKY_SUN_CAP` in `settings.ts`).
   - A dropped sun still lights surfaces through `surfaceLighting.wgsl` (R08.T9.b), dimmed by its
     transmittance to the lit point; only its scattered sky is lost. That limit is listed in
     `ATMOSPHERE_QUALITY_LIMITS` for R12's audit.
   - **Surface lighting.** Every lit pass (R07's bodies, R10's terrain, R11's clouds, oceans and
     rings) takes each sun's illuminance times the transmittance from the lit point to that sun,
     read from the shared transmittance table, plus the sky's diffuse irradiance from a small
     per-planet irradiance table, as Bruneton 2017 does (his irradiance texture, 64 × 16 by altitude
     and sun zenith). Under a cloud-deck split the irradiance below the deck comes from the deck
     tables (R08.T15.b).
   - Wireframe views draw no atmosphere.
   - Within an atmosphere, R06's sprites, band and host discs are multiplied by the transmittance
     along their direction from the camera.
8. **Transmittance is stored as optical depth.** In `rgba16float` a stored transmittance underflows
   below about e⁻¹⁶·⁶, which is zero along any slant path through a Venus-class atmosphere.
   Stored optical depth has a half-float relative precision of 10⁻³, a transmittance error of
   10⁻³τ: under 1% while T > e⁻¹⁰, and negligible beneath that. The kernels exponentiate on read.
   The table's alpha, and a second table for further absorbers, hold the accumulated absorber
   columns. On read, each sun's −ln T_c(u) is added to the optical depth (Design note 5).
9. **Thick atmospheres, by measurement** (researched 2026-09-29; Hillaire 2020 §5.5, §6 and Fig. 12;
   Stamnes et al. 1988; Dahlback and Stamnes 1991; Loughman et al. 2004).
   - **Why Hillaire's term fails.** His isotropic multiple-scattering term is inadequate at Venus
     depths. By his own figures its error grows sevenfold from g = 0 to g = 0.8, he tested only to
     fifty times Earth's Rayleigh density, and near a boundary under τ ≈ 25 of g ≈ 0.7 cloud the
     source function's anisotropy is a 30–70% effect.
   - **Where it fails is measured** (researched 2026-09-29, medium confidence). R08.T13 measures a
     boundary τ*(ω, g) on its sweep grid, the vertical extinction optical depth at which the
     analytic term first leaves the 5% metric, and stores it as `THICK_MS_BOUNDARY`, a small 2-D
     table. A column is thick when, on its worst channel, its vertical τ exceeds τ*(ω̄, ḡ), where
     ω̄ = τ_s/τ and ḡ is weighted by scattering optical depth. Similarity theory suggests that
     the boundary may collapse onto constant τ(1 − ωg) (van de Hulst 1980; Joseph, Wiscombe and
     Weinman 1976; King and Harshvardhan 1986). But that holds for fluxes far from boundaries, not
     for the near-boundary radiance where Hillaire's term fails. So a single scalar is adopted
     only if the sweep shows the collapse within one grid step, and R08.T13 tests for it.
   - **The replacement.** Above the boundary the replacement is a view-dependent source-function
     table, J_ms(h, μ₀, μ_v, m) with m = 0 and 1 azimuthal modes. It is about 32 altitudes × 32
     μ₀ × 16 μ_v × 2 × RGB, some 0.26 MB. The sky-view, aerial-perspective and march kernels read
     it in place of σ_s·Ψ_ms·p_u. Whether m = 1 is needed is decided by the gate, not assumed.
   - **The bake** is DISORT-style discrete ordinates in `f64` in a worker. It uses 16 streams,
     about 64 layers, Dahlback and Stamnes's pseudo-spherical beam, delta-M and the Nakajima–Tanaka
     corrections, and solves all μ₀ on one factorisation. It is spectral (Design note 5).
     - It is ported from the papers, never from GPL cdisort, and cross-checked by hand against
       PythonicDISORT (MIT; plane-parallel only).
     - Estimated by operation count at 10⁸ flops, 0.1–1 s. `BAKE_CEILING_S` = 5 s a world on the
       UHD 620's host (the owner's laptop, timed by the owner; the development machine's time is
       recorded beside it) triggers the fallback of the Risks.
     - Discrete ordinates is chosen because it converges without sampling noise. The Monte Carlo is
       kept for the reference, so that the two stay independent.
     - Bruneton's iterated orders are not the fallback: they diverge in this regime.
   - **The cloud-deck split.** The rule is per body, never per column, so that it agrees with
     R11's boundary (R11 Design note 9): a body is `cloudDeck` when one of its condensate decks
     (P14.T24.c) has τ(550) above `CLOUD_DECK_SPLIT_OPTICAL_DEPTH` and its cloud fraction
     (P14.T24.b) is 1. A broken field whose cells exceed τ 10 stays R11's clouds. Under the split:
     - above the deck, the tables run with the deck as a baked reflecting boundary, and any
       thinner aerosol or cloud layer above the deck stays a term of the upper medium;
     - below it, a baked plane-parallel table of downwelling radiance by altitude, view angle and
       sun angle gives both sky and aerial perspective.
   - **While a gated bake runs**, the analytic term is drawn and the label block says
     `ATMOSPHERE: COMPUTING`, which clears when the bake lands.
   - **Routing.** Thick regimes route to their bakes only once R08.T14's and R08.T15's gates pass.
     Before then a thick world is drawn with the analytic term under its own label,
     `ATMOSPHERE: APPROXIMATE`, which clears only when the gate for its regime passes. The
     brainstorm's "before Venus- and Titan-class atmospheres ship" is read as: no thick world is
     presented as computed until its gate passes.
   - **Titan.** A pseudo-spherical diffuse field reads up to 8% high at 60 km tangent height in
     limb views (Loughman et al. 2004). The gate therefore reports per geometry, so that a
     spherical fallback can be applied to the geometries that fail.
10. **The reference, and the 5% metric** (researched 2026-09-29).
    - **The reference.** It is a spherical backward Monte Carlo path tracer in `f64` with no
      tables, in Rust in `hyperion-fit`: a different language and algorithm from the TypeScript
      bake. `hyperion-fit` is the workspace's offline numerical crate, and its `orbits` subcommand
      is the precedent for a long run outside the table manifest. Its JSON fixtures for the client
      are a new kind of output for the crate, stated in its docs. Its runs are by hand and their
      outputs committed.
    - **Validation**, before it is trusted, all to |ΔI| ≤ 3σ with σ ≤ 0.3%:
      - Garcia and Siewert 1985's Haze L and Cloud C1 (τ = 64, the deck regime; Transport Theory
        and Statistical Physics 14, 437);
      - Natraj, Li and Yung 2009 (ApJ 691, 1909) and Natraj and Hovenier 2012 (ApJ 748, 28) for
        thin and thick Rayleigh, through the tracer's Stokes mode, since those tables are vector
        and a scalar tracer differs by up to about 10%;
      - Kokhanovsky et al. 2010 (JQSRT 111, 1931) and IPRT Phase A (Emde et al. 2015, JQSRT 164,
        8);
      - and for spherical shells Loughman et al. 2004 (JGR 109, D06303), at its 2–4% model spread
        rather than 3σ.

      The 1960 Coulson–Dave–Sekera tables are not used, being wrong in the fourth decimal.

    - **The metric** (a design choice; the owner may tune the floor). For every case, every
      geometry g of a fixed set and every channel c:

      |L_client(g,c) − L_ref(g,c)| ≤ max(0.05·L_ref, 3σ_ref, 10⁻³·L_max(case,c)).

      - L_max is the case's brightest diffuse radiance.
      - σ_ref ≤ 1% wherever L_ref ≥ 10⁻² L_max.
      - Only diffuse radiance is compared. The direct beam is checked against Beer–Lambert to
        10⁻⁶ in optical depth, no geometry lies within 3° of a sun, and both pipelines average over
        the tracer's stated detector cone of 0.5°–1°.
      - The geometry set:
        - ground: view zenith {0, 30, 60, 75, 85}° × relative azimuth {0, 45, 90, 135, 180}° ×
          sun zenith {0, 30, 60, 80, 90, 95}°, less its duplicates and its in-cone points
          (researched 2026-09-29, computed). Azimuth is meaningless at view zenith 0° or sun
          zenith 0°, so five azimuths collapse to one there. The seven entries within 3° of the
          sun are dropped: (0°, any, 0°), (30°, 0°, 30°) and (60°, 0°, 60°). That leaves 107
          unique ground geometries, the nearest 5° from the sun. If the aureole is wanted, points
          at 5–10° from the sun are added explicitly, not by shifting the grid;
        - orbit: the disc at five phase angles, and the limb at 0.1, 0.3, 1, 3 and 10 scale
          heights;
        - aerial perspective over 1, 10 and 32 km.
      - Two aggregates, downwelling flux at the ground and plane albedo at the top, each within
        2%.
      - The u′v′ difference is recorded, never gated.

    - **The gate** is an ordinary vitest that runs the CPU twin on each committed case. So the
      brainstorm's "every baked atmosphere table matches a path-traced reference to 5%" is
      automatic, as its Testing section files it.
11. **The low setting.** The budget's per-frame figures are 0.5–1 ms discrete (sky-view, aerial
    perspective, the march) and 2–4 ms on the UHD 620 at 720p. Per-planet tables cost under 0.1 ms
    and about 1 ms, rebuilt only when the atmosphere changes, and the tables stay under 2 MB a
    planet on both.
    - **Memory** (researched 2026-09-29, the arithmetic at 8 B a texel in `rgba16float`). The
      per-planet tables come to about 0.40 MB: transmittance 256 × 64 (131 KB), multiple scattering
      32² (8 KB) and the thick J_ms table (262 KB), plus the absorber-column table and the
      irradiance table (64 × 16, 8 KB). The per-view tables come to about 0.43 MB on high,
      sky-view 192 × 108 (166 KB) and aerial perspective 32³ (262 KB), whatever the number of
      suns, since they are summed (Design note 7). One table per sun would have been 1.71 MB a
      view at four suns, 2.11 MB with the planet's, over the row. This plan reads the brainstorm's
      "under 2 MB a planet" as the per-planet tables, and counts the per-view tables on a line of
      their own, 0.43 MB a view. R08.T11 records both. For context only, both are small beside
      the development machine's 10 GiB of VRAM and the UHD 620's share of its laptop's system
      memory; the 2 MB row stands.
    - **Oblate bodies** (Design note 17, the same arithmetic). The slices multiply the per-planet
      tables, not the per-view ones. Earth and every body under `ONE_SLICE_BELOW` keep the 0.40 MB
      above. Transmittance and its absorber alpha are per κ slice, 131 KB each. Multiple
      scattering, irradiance, the thick J_ms table and the deck tables are per band.
      - A thin Saturn-class world (5 slices, 4 bands) comes to about 0.72 MB: 655 KB of
        transmittance and 64 KB of multiple scattering and irradiance. A thin Jupiter-class world
        (4 slices, 3 bands) comes to about 0.57 MB.
      - The worst case is a thick world at Saturn's flattening: 1.77 MB with four J_ms bands (1.05
        MB), before a second absorber table (131 KB a slice).
      - A world whose per-planet bytes would pass 2 MB widens `KAPPA_STEP` to 0.3 (grazing
        interpolation error about 0.3%) and `BAND_STEP` to 0.15, in that order, and R08.T11
        records the step taken.
    - The table sizes are R05's `TABLE_SIZES`, taken from Hillaire's code, which the research
      agent's reading of his Table 2 confirms in kind: 32² multiple scattering, 32³ aerial
      perspective over 32 km, and a sky-view of about 200 × 100. R05's low sizes and its deferred,
      terrain-only aerial perspective are this plan's low setting.
    - This plan adds the per-sun cap of Design note 7 (`SKY_SUN_CAP`, which bounds the per-frame
      cost of the sun loop) and the thick table's size, and keeps both settings built together
      from R08.T6 on.
    - The thick bakes cost CPU at arrival, not frame time, so they run on both settings.
12. **What the view says.** The atmosphere is computed physics, not decoration. Five states are
    labelled. The first three follow the guide's existing grammar for a withheld or unmodelled
    section; the last two are new annunciations about the view's own drawing, as item 7 of "What
    the guide must gain" frames them:
    - `ATMOSPHERE: NOT RESOLVED` when the surface section is withheld. No atmosphere is drawn then,
      since drawing Earth's instead would be invention.
    - `ATMOSPHERE: NOT YET MODELLED` while the surface section is `not_modelled`, which is every
      generated body until P14.T24.a is on the wire. No atmosphere is drawn.
    - `AEROSOLS: NOT YET MODELLED` while plan 14 publishes no aerosol or absorber inventory. It
      covers the absorbers too: ozone and methane are drawn only from the inventory.
    - `ATMOSPHERE: COMPUTING` while a gated thick bake runs.
    - `ATMOSPHERE: APPROXIMATE` while a thick world is drawn with the analytic term because its
      regime's gate has not passed (Design note 9).

    The provisional profile is recorded in the plan and the code, not on the display. The phrases
    are drafted for the owner (R08.T2).

13. **Local and other bodies, and giants.**
    - **The ship's local body.** Only the ship's local body is drawn geometrically at the present
      time, the state collision uses (R03's Design note 7). A free camera's own local body, like
      every other body, is drawn at its `apparentM`, its orientation taken at the emitted time, by
      the march. That is how a moon's sky shows its planet.
    - **Giants inside 10⁹ m** take the full passes. Their surface is the cloud deck of Design note
      9, their visible atmosphere Rayleigh and absorption above it. They wait on P14.T24.d.
    - **Beyond 10⁹ m** R07's analytic disc reads `DiscReflectanceTable`, baked from the same
      medium, so that a giant's colour does not jump at the boundary.
14. **When tables are rebuilt.** Per-planet tables and bakes are rebuilt:
    - on arrival;
    - when the granted detail level changes;
    - when plan 14's atmosphere or inventory changes with time, as a dust storm of P14.T31 would.

    The cache key is the body, a hash of its figures and inventory, and `ATMOSPHERE_OPTICS_VERSION`.
    That version is a client constant, not the generator version, since the atmosphere is not
    generated output. Per-view tables are rebuilt every frame.

15. **Determinism.** Nothing here is authoritative or read back into anything that is. So none of it
    is under the sim's determinism rules, and none of it runs in `hyperion-sim` or
    `hyperion-surface`; twins are compared to tolerances, not bits. The reference in `hyperion-fit`
    uses `hyperion_sim::math`, which the crate's Clippy bans require, and is reproducible for any
    thread count by the crate's own convention.
16. **Fixtures, while plan 14 lacks the gases** (researched 2026-09-29).
    - **What plan 14 fills.** As built, plan 14 fills only H₂O, CO₂, N₂ and Ar. Its Titan is
      1.4 bar of N₂ with no methane, no world has O₂, and a gas envelope has no composition. So
      the haze and ozone rules and the giants cannot fire until P14.T24.c, d and f land.
    - **The fixtures.** Until then the Titan-class, ozone-bearing and giant fixtures are hand
      parameterised, not taken from plan 14's Solar System table. The Mars fixture's dust is hand
      parameterised too.
    - **Venus.** The Venus-class fixture uses the real 92 bar (τ_R ≈ 15.5), and a second case uses
      plan 14's 58 bar (τ_R ≈ 9.8), so that both the physics and the generated world are gated.
17. **Oblate bodies: gravity-scaled height and curvature slices** (researched 2026-09-29, a physics
    ruling, medium-high confidence). This widens R05 Design note 16's r = √(MN) + h to every
    flattening. On Earth it reduces to that rule.
    - **Two errors, not one.** R05's lookup removes the height offset and the latitude-mean
      curvature. Two errors remain, and the second is the larger:
      - _Curvature by direction._ By Euler's theorem the radius of curvature along a ray of
        azimuth α is 1/R_α = cos²α ÷ M + sin²α ÷ N. At the equator it spans c²/a to a about
        √(MN) = c. Chapman's grazing optical depth goes as √R (Ch(X, 90°) ≈ √(πX/2), X = R/H). So
        the grazing error is ±f/2: 0.17% on Earth, 3.5% on Jupiter, 5.4% on Saturn.
      - _Gravity by latitude._ Every level of a hydrostatic atmosphere with one T(p) sits at a
        height proportional to 1/g(φ). The column above a pressure is p/g. So a medium built with
        one g is wrong in vertical optical depth by the ratio of the gravities. For a level
        spheroid, Somigliana's closed form gives g_p/g_e − 1 = 0.53% (Earth), 5.2% (Uranus), 4.9%
        (Neptune), 16.7% (Jupiter) and 32.6% (Saturn, 9.08 against 12.04 m s⁻²). The Earth value
        matches WGS 84's 9.7803 and 9.8322. Around a mid g this is ±0.26%, ±2.5%, ±2.4%, ±7.7% and
        ±14.1% in vertical optical depth.
      - The earlier "about 17% at Saturn" was the ratio of polar to equatorial meridional
        curvature, which √(MN) already removes. With one table, the grazing spread is ¼ of
        ln[(g_p a²/c) ÷ (g_e c²/a)]: ±0.38% on Earth, ±2.5–3.0% on the ice giants, ±8.9% on
        Jupiter and ±14.8% on Saturn.
    - **How much of that reaches radiance.** Radiance near the horizon from the ground moves by at
      most the relative optical-depth error, and by less where the horizon is thick. Transmittance
      moves by τ times it, so it is about four times worse at a τ ≈ 4 sunset. The limb from orbit
      is marched per pixel on the true spheroid (R05 Design note 16), so curvature costs it
      nothing there. But a density that ignores g(φ) is wrong by exp(n·Δg/g) at n scale heights,
      out of the 0.1–10 H of Design note 10's limb set. At 10 H this is 2.7% on Earth, about 28%
      on the ice giants, and a factor of 2–4 on Jupiter and Saturn.
    - **Where the lookup holds.** Against Design note 10's 5% (keeping ≤ 1% of it for geometry),
      R05's lookup holds to f ≈ 0.005, the Earth class. Above that, the limb at several scale
      heights fails first. Horizon radiance holds to 1% up to f ≈ 0.007 and to 5% up to f ≈ 0.03.
    - **The rule, for every body.** There is no flattening switch; Earth-like bodies get one slice.
      - _Gravity-scaled height._ The medium's density, and every table lookup, use h\* = s·h with
        s = g(φ)/g_ref. Here h is the geodetic height above the datum, g(φ) is Somigliana's
        normal gravity of the level spheroid from (GM, a, c, ω), and g_ref = √(g_e g_p) is the
        gravity the column (R08.T3.a) is built at. Optical depth read from a table is divided by s.
        This is geopotential height, the U.S. Standard Atmosphere 1976's vertical coordinate. The
        scaling is exact for optical depth: shrinking lengths by s maps the local sphere of
        radius R_α onto one of radius s·R_α carrying the reference medium, with τ multiplied by s.
      - _Curvature slices._ The transmittance table becomes a small family over κ = s·R_α ÷
        R_ref. Slice k is the reference medium on a sphere of radius κ_k·R_ref, spaced Δln κ ≤
        0.15 (interpolation error under 0.1% in grazing τ) and read at (κ, r = κ_k·R_ref + h\*,
        μ). Here α is the azimuth of the ray being looked up (the sun's, for a sample's
        transmittance). The count is 1 + ⌈ln(κ_max ÷ κ_min) ÷ 0.15⌉ once the range exceeds 0.02,
        and 1 below that: 1 for Earth, 2 for the ice giants, 4 for Jupiter and 5 for Saturn. That
        is 128 KB a slice at the high size, rebuilt only when the medium changes.
      - _Latitude bands._ Multiple scattering depends non-linearly on the column, so the
        multiple-scattering and irradiance tables, Design note 9's thick bakes, R08.T15's deck
        tables and
        `DiscReflectanceTable` are built per band of s. Each band is an ordinary per-planet build
        at that band's g(φ) and √(MN). Bands are spaced Δln s ≤ 0.1 and read by linear
        interpolation at the sample's or disc pixel's latitude: 1 band for Earth and the ice
        giants, 3 for Jupiter and 4 for Saturn. The bake count multiplies by the band count
        against `BAKE_CEILING_S`.
      - _Per-view marches._ Sky-view and aerial perspective march each ray in its own osculating
        sphere R_α (azimuth is a sky-view axis, so this costs a few ALU a texel column), with
        density at h\*. The orbit march keeps R05's true spheroid shells with density at h\* per
        sample. Both read the slices and bands above.
      - Rejected:
        - a camera-local osculating sphere per view is exact on the ground but wrong from orbit,
          where one image spans every latitude;
        - a full spheroidal march of the sun's path per sample is a nested march over the UHD 620's
          budget (Design note 11).
    - **Gas giants.** Nothing is new in kind, only in size. Their datum is the 1-bar level
      spheroid, so gravity-scaled height above it is exactly their hydrostatic structure (under
      one T(p)). That makes the gravity term mandatory there, being 30–60 times Earth's. Three
      effects are not modelled, and each is stated:
      - zonal winds change the effective gravity by 2ωu + u²/a, about 1.4% of g under Saturn's
        400 m s⁻¹ equatorial jet;
      - the real 1-bar surface departs from the best-fit spheroid through differential rotation
        (Lindal, Sweetnam and Eshleman 1985), a datum question for plan 14;
      - T(p) varies with latitude, which is weather, and plan 14's to give.
    - **The gate must see it.** Design note 10's tracer is spherical, so as written a spherical
      client passes against it whatever the flattening. R08.T12.d therefore adds a spheroid mode:
      delta tracking against a majorant, the spheroid shells, and density at h\*. R08.T12.b adds
      a thin Saturn-class case (f = 0.098, H₂–He Rayleigh over a Lambertian 1-bar boundary, so that
      R08.T13 gates it with the thin cases) with ground views at the equator (ray azimuths north and
      east), at 60° and at the pole, and the limb at 0.3, 1 and 3 H over the equator and the pole.
      These run under the same 5% metric.
    - Until R08.T6.f lands, a body with ln(κ_max ÷ κ_min) > 0.02 is drawn under Design note 9's
      `ATMOSPHERE: APPROXIMATE`. The tasks are R08.T3.a and T3.d (the column at g_ref, normal
      gravity and the slicing), T6.e and T6.f (the twin and the kernels), T9.b (irradiance by
      band), T12.d and T12.b (the tracer's spheroid mode and the Saturn case), T14 and T15 (bakes by
      band) and T16.b (the disc by band).
    - Sources:
      - Chapman 1931 (Proc. Phys. Soc. 43, 483);
      - Heiskanen and Moritz 1967, _Physical Geodesy_ §2-7 to 2-9 (the level ellipsoid,
        Somigliana's closed form for any flattening), and NIMA TR8350.2 eq. 4-1;
      - Syndergaard 1998 (J. Atmos. Sol.-Terr. Phys. 60, 171; doi:10.1016/S1364-6826(97)00056-4),
        whose oblateness correction for limb sounding is the local centre of curvature in the
        ray's plane;
      - Lindal et al. 1981 (JGR 86, 8721) and Lindal, Sweetnam and Eshleman 1985 (AJ 90, 1136;
        doi:10.1086/113820), whose Voyager occultations of Jupiter and Saturn were reduced with
        the local radius of curvature and gravity by latitude;
      - Hillaire 2020, Bruneton and Neyret 2008, Bruneton 2017, sebh's code and Bevy 0.19, all
        spherical. No oblate-atmosphere renderer is known to have precedent here.

      The figures are computed (Somigliana at each body's GM, a, c and period, and Chapman to
      first order in H/R), not measured. R08.T12.b's spheroid case measures them.

## Tasks

The order:

- T1 and T2 are documents and can start at once. T1 must land in plan 14 before plan 14 builds
  P14.T24.
- T3–T5 build the optics, in order. T6 generalises R05's tables over N terms and needs T3 and T4
  (the ozone curve of growth and the fitted channels).
- T7 (several suns, the sky's extinction), T8 (other bodies from outside) and T9 (aerial
  perspective and surface lighting) follow T6.
- T10 assembles the medium from a body and needs T2–T6. T11 records the thin benchmarks and needs
  T7–T10.
- T12.a (the tracer) needs nothing of this plan and can start at once, and T12.c (its benchmarks)
  follows it. T12.b (the cases and references) needs T5, T10's fixtures and T12.c.
- T13 needs T6 and T12. T14 follows T13, and T15 follows T14, whose solver it uses.
- T16 needs T8 and T15. It runs on the hand giant fixture, and re-runs on plan 14's envelope when
  P14.T24.d is on the wire. T17 closes.

Some tasks wait on the owner or on another plan, and say so where they do:

- **Licences.** The files held in R08.T5.b (H₂SO₄, Mars dust, tholin) wait on the owner's
  licence ruling. So do the cases that use them: the Venus-class, Mars and Titan-class
  fixtures of R08.T10, their cases and references in R08.T12.b, and their gates in R08.T13–T15.
  Until the ruling, those cases are not committed, and the gates run on the licence-free cases:
  Earth, Earth with ozone, the Rayleigh-only Venus columns and the giant deck. The regimes they
  cannot yet gate stay `ATMOSPHERE: APPROXIMATE` (Design note 9).
- **Star spectra.** Every spectral bake and fit reads the star's `bake_spectrum` from R06's
  `HostDiscDto` (R06.T3.c and T10). The channel fit's non-solar suns are R06's colour-table rows at
  those temperatures.

TypeScript paths are under `apps/hyperion/src/renderer/src/view/atmosphere/` unless a path says
otherwise.

- Tests that need no GPU run under `pnpm test`, and so in `just ci`.
- Every task that adds or changes a catalogued kernel also passes `just test-render` (R01.T9.e).
- GPU checks are by hand and recorded here, as the brainstorm's Testing section says.
- Every physical constant carries its source, re-checked against that source when it is written
  (the galaxy README's Figures rule).
- Every timing in this plan's design notes was taken under shared load (load average about 14 on
  eight threads) and is provisional. Each task that records a timing re-measures it on a quiet
  machine, with the load average stated.

### R08.T1 Asks of galaxy plan 14

Write five amendment tasks into [galaxy plan 14](../galaxy-generation/14-planetary-systems.md),
four beside P14.T24.a and one in P14.T35, as R04 amends it for the detail seed. The physics below
was researched on 2026-09-29 (Design notes 3, 6 and 16, and the sources cited there). The amendment
cites it, and any rule marked "from memory" is checked against its paper by the agent who builds
it. The letters follow the README's asks table; the build order they state is e, f, c, d, then
T35.d, since T24.c's rules read T24.e's profile and T24.f's gases.

- **P14.T24.c The aerosol and absorber inventory.**
  - Per mode: a material from a closed enum with a shape class (sphere, non-spherical mineral,
    crystal); a size distribution by effective radius and variance, or an aggregate's monomer
    radius, count and D_f ≤ 2.5; a column τ(550); and a vertical profile.
  - Per absorber: a species and its column.
  - The formation rules:
    - condensate decks where a species' partial pressure crosses its saturation curve along
      P14.T24.e's profile;
    - an ozone column N_⊕ × F(p_O₂/PAL) × U(star) (Segura et al. 2003, 2005), as a layer near
      10–30 mbar;
    - methane haze at CH₄ ≳ 10⁻³ on N₂–CH₄ worlds, or CH₄/CO₂ ≳ 0.1–0.2 on CO₂ worlds (Trainer et
      al. 2006; Arney et al. 2016), as a fractal-aggregate mode with Titan's monomers;
    - dust on arid, windy, thin-aired worlds.
  - The inventory is a function of age + t, in the surface section.
- **P14.T24.d The envelope's visible atmosphere.** For `SurfaceState::GasEnvelope`, in a section a
  giant has (today its surface is `Section::NotApplicable`):
  - T_int, T_irr and a Guillot 2010 Eq. 29 profile joined to an adiabat;
  - He/H₂ at about 0.16;
  - CH₄, NH₃ and H₂S at solar abundance × E(M) × 10^[Fe/H];
  - decks by saturation crossing, with Ackerman and Marley 2001's f_sed;
  - a haze τ.

  Its tests: Jupiter's T(1 bar) 166 K ± 10%, its NH₃ deck at 0.5–1 bar, CH₄/H₂ ≈ 2 × 10⁻³, and
  Sudarsky et al. 2000's classes as the check.

- **P14.T24.e The vertical structure.** The parameters (T_s, p_s, β, T_skin) of Design note 3, with
  α by condensing species, R/c_p per gas from kinetic theory with R&C 2012's fixed degrees of
  freedom (Design note 3's values), mixed by c_p, and
  `planetary::temperature_at(structure: &VerticalStructure, p: Pascals) -> Kelvin` in the sim. Its
  tests: Earth's tropopause at 0.12–0.25 bar and 205–220 K; Venus at 300–370 K at 1 bar; Titan's
  tropopause below 0.3 bar at 60–75 K. It notes R&C 2012's stratospheric upgrade for when T24.c
  lands, and that the thermosphere is the flight model's.
- **P14.T24.f Carbon speciation and O₂.** This is the precondition of T24.c's haze and ozone rules.
  - Carbon on cold bodies (T_s ≲ 150 K) beyond the snow line is CH₄, not CO₂, held at saturation,
    calibrated on Titan's 5.65%.
  - Abiotic O₂ comes from water loss in the runaway state (Luger and Barnes 2015).
  - Biotic O₂ is a gap for the owner.
- **P14.T35.d The atmosphere on the wire.** `BodySurfaceDto` gains the fields of T24.c, T24.e and
  T24.f: the inventory's modes and absorbers, the vertical structure's (T_s, p_s, β, T_skin), and
  the gas fractions with CH₄ and O₂. A giant gains an `envelope` section of its own, holding
  T24.d's figures, since its `surface` stays `not_applicable`. Each field's name carries its SI
  unit; `just gen-protocol` follows. Its test pins the wire form of an Earth, a Venus and a
  Jupiter, one test per section state.

Files: `docs/agent/plans/galaxy-generation/14-planetary-systems.md`. Acceptance:

- `npx prettier --check docs/agent/plans/galaxy-generation/14-planetary-systems.md` passes;
- the four P14.T24 tasks appear under Phase E and P14.T35.d under Phase H, with their tests and
  sources;
- plan 14's Risks name this plan as their consumer, and record its Venus at 58 bar against the
  real 92.

This is a brainstorm-driven plan edit, so it is shown to the owner before it is committed.

### R08.T2 The atmosphere labels, drafted for the owner

Draft the five nomenclature entries of Design note 12 in the form of R02's drafted items, each
with its meaning and when it clears:

- `ATMOSPHERE: NOT RESOLVED`;
- `ATMOSPHERE: NOT YET MODELLED`;
- `AEROSOLS: NOT YET MODELLED`, covering absorbers as well;
- `ATMOSPHERE: COMPUTING`;
- `ATMOSPHERE: APPROXIMATE`, which clears when its regime's gate passes.

None uses a status colour or the word "degraded" (item 7 of
[What the guide must gain](../../brainstorming/rendering-and-planets.md#what-the-guide-must-gain)).
The client is built to the draft, and the task ends when **the owner signs off**.

Files: the draft beside R02's in `docs/frontend/ux-guidelines.md` as a proposed edit, and
`labels.ts`, which feeds R02's `ViewLabelBlock`. Tests: `labels.test.ts` pins the strings.
Acceptance:

- `pnpm --filter hyperion exec vitest run view/atmosphere/labels` passes;
- the console-ux lint passes;
- the sign-off is recorded here.

### R08.T3 The column and Rayleigh scattering per gas

- **R08.T3.a The column.** `column.ts`: `TemperatureProfile` with `isothermal` and
  `radiativeConvective`, `temperatureAt`, and `hydrostaticColumn` (Design note 3), with molar
  masses from plan 14's `Gas::molar_mass_g_per_mol` (IUPAC 2021), each naming its Rust source.
  The column is built at g_ref = √(g_e g_p) from R08.T3.d, not at the bulk section's single
  gravity, and its altitudes are gravity-scaled heights (Design note 17). Tests:
  - an isothermal scale height equals kT/(μ m_u g_ref) to 10⁻⁶;
  - on a level spheroid, the column mass above the datum at latitude φ, read through the gravity
    scaling, is p_s/g(φ) to 0.5%, at the equator, 45° and the pole of a Saturn-class figure;
  - Earth at 288.15 K, 1013.25 hPa, μ = 28.97 gives about 8.4 km, the brainstorm's "near 8 km";
  - the column mass per area is p_s/g to 0.5% for a thin atmosphere, with the spherical excess for
    a thick one;
  - `temperatureAt` reproduces Design note 3's Earth, Venus and Titan checks from the inputs
    stated there: 0.179 bar, 316.8 K at 1 bar, and 0.24 bar, each to 1%;
  - the mixed R/c_p of Design note 3's per-gas values equals 3/13 for pure CO₂ and 2/7 for pure
    N₂, and mixes by c_p, not by γ;
  - once P14.T24.e lands, a golden list of levels from the sim is matched to 10⁻¹².
- **R08.T3.b Dispersion and King factors.** `rayleigh.ts`: `GAS_DISPERSION` and `kingFactor` per
  Design note 4's table, each with its paper, its refractiveindex.info file and its reference
  state. Tests:
  - each formula's (T, p, N_ref) is pinned against its paper;
  - CO₂ evaluated at 288.15 K instead fails by at least 10% (the trap);
  - dry air recomputes Bucholtz's 4.51 × 10⁻²⁷ cm² to 1% (Applied Optics 34, 2765);
  - Earth gives 4.85, 11.5 and 28.7 × 10⁻⁶ m⁻¹ at 680, 550 and 440 nm to 1%;
  - H₂ agrees with Dalgarno and Williams within 8%, and He with Chan and Dalgarno within 1%;
  - a 92-bar CO₂ column gives τ_R(550) of 16 ± 1;
  - no constant equals the tutorial set.

  Before a test is pinned to Sneep and Ubachs's measured 532 nm cross-sections, their Table 3 is
  read. If the formula route and the measurement disagree beyond its stated error (CO₂ may, by
  7%), the formula is pinned to its own value and the measurement is recorded as a separate check
  with its error.

- **R08.T3.c The molecular term.** `molecularTerm(column, fractions)`: the number-fraction
  mixture, F_mix and ρ_mix of Design note 4, and the Rayleigh phase function. Tests:
  - a pure N₂ column equals N₂ alone;
  - the mixture is linear in the fractions;
  - fractions that do not sum to 1 within 10⁻⁹ are refused;
  - ρ_mix for dry air is within 2% of Bates's.

- **R08.T3.d Normal gravity and the slicing.** `oblate.ts` (Provides): `normalGravity`
  (Somigliana's closed form for the level ellipsoid, Heiskanen and Moritz 1967 §2-7 to 2-9),
  `referenceGravity`, `directionalCurvatureRadiusM` and `oblateSlicing`, with the constants
  `KAPPA_STEP`, `BAND_STEP` and `ONE_SLICE_BELOW` (Design note 17). Its inputs are R07's
  `BodyFigure`, GM from the record's `mass_kg` section times CODATA's G, and ω from P14.T14.c's
  `BodyFixedFrame` rate (R05 Design note 14's test-planet period for R05's Earth until then); with
  no rotation, g(φ) is taken as the bulk section's gravity and one slice results. Tests:
  - WGS 84's figure, GM and ω give NIMA TR8350.2's γ_e = 9.7803253359 and γ_p = 9.8321849378
    m s⁻² to 10⁻⁹ relative;
  - Saturn's (a = 60,268 km, c = 54,364 km, GM = 3.7931 × 10¹⁶ m³ s⁻², 10.656 h) give 9.08 and
    12.04 m s⁻², and Jupiter's 23.12 and 26.98, to 0.5%;
  - R_α equals M at α = 0 and N at 90°, lies between them at every α, and is a²/c for every α at
    the pole;
  - the slice and band counts are 1 and 1 for Earth, 2 and 1 for Uranus, 4 and 3 for Jupiter and 5
    and 4 for Saturn; a sphere with ω = 0 gives one of each with s ≡ 1.

Acceptance: `pnpm --filter hyperion exec vitest run view/atmosphere/column view/atmosphere/rayleigh view/atmosphere/oblate`.

### R08.T4 Channels and absorbers

- **R08.T4.a The fitted triple.** First `bless.ts`, the client's bless helper: under
  `HYPERION_BLESS=1`, the variable the Rust testkit already reads
  (`crates/hyperion-testkit/src/golden.rs`), a vitest rewrites its committed table. Otherwise it
  compares, and on a mismatch fails naming the command,
  `HYPERION_BLESS=1 pnpm --filter hyperion exec vitest run <file>`. `BAKE_WAVELENGTHS_NM` (15 over
  380–760 nm) is written here. A bless-style vitest then refits `CHANNEL_WAVELENGTHS_NM` by minimax
  Δu′v′ over a stated case family: Earth at several suns (3,200 K, solar and 9,000 K), and Mars with
  dust. The two non-solar spectra are R06's colour-table rows at those temperatures, read through
  the table's `bake_spectrum` (R06.T3.c). It writes the triple and the objective's value into
  `channels.json`, and is checked unchanged otherwise. Mars's dust case waits on the licence ruling
  for Wolff et al.'s data (R08.T5.b), and until then the family is Earth's alone, recorded as such.
  It uses CIE 1931 2° colour-matching functions reduced to linear Rec. 709, the primaries R06 uses.
  Tests:
  - the refit starts from (620, 540, 445) and does not worsen the recorded objective;
  - at Earth the fitted triple beats 680/550/440 at a sun zenith of 85°.
- **R08.T4.b Curves of growth.** `absorbers.ts`: `absorberCurve(absorber, spectrum)` computes T_c(u)
  per absorber and channel for one sun, weighted by max(r̄_c, 0)·S (Design note 5), on σ's own grid
  of 1 nm or finer, with S interpolated from the sun's 15 `bake_spectrum` bin averages. It runs in
  the optics worker at arrival, once per sun. The reduced cross-sections, ozone binned to 1 nm and
  methane's coefficients as published, are written to `absorbers/crossSections.json` by the same
  bless step.
  - Sources: ozone from Serdyuchenko et al. 2014 (AMT 7, 625; the articles CC BY 3.0, the data
    page's terms unstated) and methane from Karkoschka and Tomasko 2010 (Icarus 205, 674;
    Elsevier's terms).
  - Only the reduced values are committed, with attribution. The bless step fetches the raw
    tables, and whether a raw table may be committed is asked of the owner.

  Tests:
  - a flat spectrum's curve is e^(−σu);
  - every T_c lies in (0, 1] with no clamp, for a 2,500 K and a 30,000 K Planck sun (the clipped
    weights);
  - two suns of different temperature give different curves, and the same sun the same curve;
  - the sun's colour through 0.1–10 × a reference column matches the spectral result to
    Δu′v′ ≤ 0.002, for the Sun and for a 6,500 K Planck spectrum;
  - 300 DU of ozone gives a green-channel Chappuis optical depth of 0.02–0.04;
  - the bless check fails on a changed table and names the command.

Acceptance: `pnpm --filter hyperion exec vitest run view/atmosphere/absorbers view/atmosphere/channels`.

### R08.T5 Aerosols

- **R08.T5.a Mie for a sphere.** `mie.ts` follows Wiscombe 1980's structure (Appl. Opt. 19, 1505):
  - N_stop = x + 4.05x^(1/3) + 2;
  - ψ_n and χ_n by upward recurrence;
  - D_n(mx) by downward recurrence started from Lentz 1976's continued fraction, or at
    n ≥ |mx| + 4.05|mx|^(1/3) + 16. The code says why BHMIE's start is not enough.

  Tests:
  - Wiscombe's published cases as miepython tabulates them (MIT), to 10⁻⁶, omitting
    m = 1.5 − i, x = 100, which miepython itself relaxes;
  - Bohren and Huffman's m = 1.55 case (Q_ext 3.10543, Q_back 2.92534, g 0.63314);
  - the Rayleigh limit;
  - Q_abs = 0 for a non-absorbing sphere to 10⁻¹²;
  - the phase normalisation to 10⁻⁹, on a Gauss–Legendre grid of more than N_stop nodes.

  The research timing (4.8–6.3 ms for x = 10³ at 256 angles) was taken under load and is re-measured
  here on a quiet machine. Acceptance: `pnpm --filter hyperion exec vitest run view/atmosphere/mie`.

- **R08.T5.b Size distributions and materials.**
  - `sizeDistribution.ts`: Gauss–Legendre in ln r over ±4σ, doubled until the phase integral and
    ⟨cos θ⟩ change by under 10⁻⁴, typically 200–500 nodes.
  - `materials/`: one refractive-index file per material, each with its paper and its CC0 or MIT
    copy in a header: water (Hale and Querry 1973), ice (Warren and Brandt 2008), NH₃ ice
    (Martonchik et al. 1984), CH₄ (Martonchik and Orton 1994), silicates (Dorschner et al. 1995),
    soot, iron.
  - Held until their licences are checked with the owner: H₂SO₄ (Palmer and Williams 1975), Mars
    dust (Wolff et al. 2009) and tholin (Khare et al. 1984). The owner's ruling is a precondition
    of each held file and of the fixtures, cases and gates that use it (the task order's list).
    Until then, the Venus mode-2 test below takes the single index n = 1.44 at 550 nm that Hansen
    and Hovenier 1974 publish, and no spectral H₂SO₄ table is committed.
  - NH₄SH has no visible data and waits on an owner ruling.
  - The non-spherical materials' literature phase functions sit beside their indices.

  Tests:
  - the distribution reproduces r_eff and v_eff to 10⁻⁴;
  - every file covers 380–780 nm;
  - a narrow distribution equals the single sphere;
  - Venus's mode-2 droplets (r_eff 1.05 µm, v_eff 0.07, n ≈ 1.44 at 550 nm; Hansen and
    Hovenier 1974) give g within that paper's figure.

- **R08.T5.c Aggregates and phase tables.** `aggregate.ts` implements Tazaki and Tanaka 2018's MMF
  with D_f ≤ 2.5, and the phase-shift gate of Design note 6: a mode with Δφ ≥ 1 keeps its
  opacities and takes a Henyey–Greenstein phase from its asymmetry. Beside it go the √θ phase
  tables per channel, and the delta-M truncated series
  with its fraction, for the bakes only (Design note 6). Tests:
  - optool fixtures for a Titan-like aggregate (monomer 0.05 µm, about 3,000 monomers, D_f = 2;
    Tomasko et al. 2008), generated offline and committed, to their stated tolerance;
  - the Titan-like aggregate's Δφ is below 1, and a compact large aggregate's above it falls back;
  - tables integrate to 1 to 10⁻⁶;
  - the truncated series plus its fraction reproduces the full integral and asymmetry.

Acceptance for T5.b and T5.c:
`pnpm --filter hyperion exec vitest run view/atmosphere/sizeDistribution view/atmosphere/aggregate view/atmosphere/materials`.

### R08.T6 Hillaire's tables over N terms

Widen R05's `HillaireAtmosphere` (`hillaire.ts`) and its kernels (`shaders/`) to the medium of
Design note 2, in six subtasks. The first three change no picture: R05's constants, run through
the widened code, reproduce R05. The fourth changes Earth's picture by amounts it records. The
fifth and sixth widen R05 Design note 16's spheroid lookup to every flattening (Design note 17).

- **R08.T6.a The CPU twin over N terms.** `tablesCpu.ts`, built on R05's `opticalDepth.ts`, gives
  each of R05's kernels in `f64` at R05's sizes over a list of terms, with tabulated density and
  phase. Tests (no GPU):
  - R05's constants as a medium give R05's oracle values to 10⁻⁶;
  - two identical half-density terms equal one;
  - optical depth adds across terms.

  Acceptance: `pnpm --filter hyperion exec vitest run view/atmosphere/tablesCpu`.

- **R08.T6.b The kernels over N terms.** Terms travel in a storage buffer, and density and phase
  tables in 2D array textures, one layer a term. The `thin` multiple-scattering table keeps
  Hillaire's isotropic 32². Every changed kernel stays registered in `WGSL_CATALOGUE`. Files: R05's
  `shaders/{transmittance,multiScattering,skyView,aerialPerspective,rayMarch,composite}.wgsl`,
  `hillaire.ts`, `tables.ts`, `medium.ts`. Smoke (`just test-render`): every kernel compiles, and
  the readbacks agree with the twin to 10⁻³ relative above 10⁻⁶ of the table's maximum, on the
  no-f16 run. Acceptance: `pnpm test` and `just test-render` pass.

- **R08.T6.c Optical depth and curves of growth.** Transmittance stores optical depth in RGB and
  the accumulated absorber column in alpha, with a second table for further absorbers. Each sun's
  −ln T_c(u) is added on read (Design notes 5 and 8), in the twin and the kernels. R05.T12.b's smoke
  assertion that every transmittance texel lies within [0, 1] is rewritten as: every stored
  optical depth is finite and non-negative. Tests: a Venus-class transmittance is finite with no
  underflow, and R05's Earth, with its ozone as an absorber curve reduced to the linear regime,
  reproduces R05's transmittance to 10⁻³. Acceptance: `pnpm test` and `just test-render` pass.

- **R08.T6.d Earth rebuilt.** `EARTH_REFERENCE` is rebuilt from R08.T3's column and molecular
  term (tabulated, about 8.4 km in scale height, in place of R05's 8 km exponential), R05's
  aerosol, the ozone curve of growth (R08.T4.b) and the fitted channels (R08.T4.a).
  - The Sun's per-channel illuminance comes from R07's `starIlluminance`, through R06's colour
    (Design note 5), in place of R05's spectral-to-luminance factors in `solar.ts`.
  - `solar.ts` is kept as a check: the two agree in luminance to 1%, and their chromaticity
    difference is recorded.
  - R05's constants stay as a second medium for comparison.
  - The differences from R05 are recorded here, in the sky's zenith radiance at noon and at a sun
    zenith of 85°, and in u′v′.

  Acceptance: `pnpm test` and `just test-render` pass. By hand, R05's descent at Earth is compared
  with R05's recorded run, and the differences are noted here.

- **R08.T6.e Slices and bands, the twin.** It needs R08.T3.d. In `tablesCpu.ts`: density and
  every lookup at the gravity-scaled height h\* = s·h; transmittance over the κ slices of
  `medium.slicing`, read at (κ, r = κ_k·R_ref + h\*, μ) with κ from the looked-up ray's own
  azimuth and optical depth divided by s; multiple scattering and irradiance per band, each an
  ordinary build at the band's g(φ) and √(MN). The sky-view and aerial-perspective marches march
  each ray in its own osculating sphere R_α, and the orbit march keeps R05's spheroid shells with
  density at h\*. Tests (no GPU):
  - a sphere with ω = 0 reproduces T6.d's tables to 10⁻¹²;
  - on a Saturn-class figure, an `f64` brute-force march of the sun's path on the true spheroid,
    density at h\*, gives the sliced lookup's optical depth within 0.5% at grazing and 0.1% at μ
    ≥ 0.2: at the equator looking north and east, at 45° and at the pole;
  - interpolating between slices in ln κ costs under 0.1% in grazing optical depth, and between
    bands under 0.5% in multiple-scattering radiance, against a table built at the exact value;
  - Earth's slicing is one slice and one band, and Earth's differences from T6.d (s within ±0.26%)
    are recorded here.

  Acceptance: `pnpm --filter hyperion exec vitest run view/atmosphere/tablesCpu`.

- **R08.T6.f Slices and bands, the kernels.** `transmittance.wgsl`, `multiScattering.wgsl`,
  `skyView.wgsl`, `aerialPerspective.wgsl` and `rayMarch.wgsl` gain T6.e's slicing through
  2D array textures, one layer a slice or band, and a shared `oblate.wgsl` (catalogued). This task
  widens R05's `atmosphereInputs(camera, figure)` in R05's `hillaire.ts` itself, as R08.T6 widens
  R05's other names: its result gains the camera's geodetic latitude and s = g(φ) ÷ g_ref from
  R08.T3.d, and R05.T12.c's tests keep passing with s ≡ 1 at a = c. `setMedium` allocates the
  layers under `atmosphere-tables`. When this lands, `ATMOSPHERE: APPROXIMATE` clears for oblate
  thin bodies. Smoke (`just test-render`): the readbacks agree with the twin to 10⁻³ on a
  Saturn-class figure and on Earth. Acceptance: `pnpm test` and `just test-render` pass; by hand,
  the Saturn-class limb from orbit over the equator and the pole is recorded here.

### R08.T7 Several suns, and the sky through the air

Sky-view and aerial perspective per photorealistic view, summed over the drawn suns in one march
(Design note 7), in `drawFrame(view, suns)`:

- suns are ranked by R07's `starIlluminance`, with R07's `STAR_CUT_RELATIVE` and the setting's
  cap, `SKY_SUN_CAP`. This task writes `settings.ts`, adds `atmosphereView` to R05's
  `ViewSettings` with its values in `SETTINGS`, and fills the cap's entry in
  `ATMOSPHERE_QUALITY_LIMITS`;
- the sky-view, aerial-perspective and composite passes blend alpha with source zero and
  destination one, keeping R07's `METER_CLASS` in the HDR target;
- a sun below the horizon still feeds the sky through the planet's shadow in the tables;
- R06's sprites, band and host discs are multiplied by the transmittance from the camera along
  their direction.

Tests (no GPU, CPU twin):

- a second equal sun at the same direction doubles the sky radiance;
- `STAR_CUT_RELATIVE` and the cap keep the right suns, agreeing with R07 on a shared fixture;
- after the atmosphere's passes, the HDR target's alpha still holds each pixel's `METER_CLASS`;
- two views with the same camera have equal tables;
- the summed sky-view agrees with per-sun tables in the twin to 2% within 10° of each sun, for a
  Rayleigh sky and for Earth's aerosol; if it fails, the per-sun fallback of Design note 7 is
  built here and the finding recorded;
- the per-view tables' bytes do not grow with the number of suns;
- a star seen at the zenith from Earth's surface is dimmed by e^(−τ) of the column.

Acceptance: `pnpm test` and `just test-render` pass, and by hand a binary sky on an Earth fixture
shows both twilights, recorded here with the development machine's timings and, from the owner,
the UHD 620's.

### R08.T8 Views from outside and other bodies

Widen R05's ray march to N terms and several suns. The march draws any body's atmosphere other
than the ship's local body at its `apparentM` from R03's `sceneAt` (Design note 13), with the limb,
the terminator and the planet's shadow in its own air. The switch to the sky-view table is by
altitude, with a blend band. Tests (no GPU):

- the CPU march just inside the top agrees with the sky-view twin to 1% at the band's edges;
- the limb falls to zero outside the top;
- a body's atmosphere is drawn at `apparentM`, and only the ship's local body at `geometricM`,
  including when a free camera sits inside another body's Hill sphere.

Acceptance: `pnpm test` and `just test-render` pass, and by hand an ascent from R05's test planet
to 10⁸ m shows no step at the switch, recorded here.

### R08.T9 Aerial perspective and surface lighting

- **R08.T9.a Aerial perspective.**
  - On the low setting, R05's deferred aerial perspective applies to terrain alone. On the high
    setting it applies to everything opaque (`AERIAL_PERSPECTIVE_SCOPE` in `settings.ts`).
  - Beyond the volume's 32 km reach (Hillaire 2020 §5.4, confirmed) the march takes over.
  - `aerialPerspective(view)` exposes the volume and transmittance, and `skyView(view)` the
    view's summed sky-view table, to R11.

  Tests (no GPU):
  - a point at the volume's far edge takes the same in-scattering from volume and march to 2%;
  - the low setting touches terrain alone;
  - `skyView(view)` returns the table `drawFrame` built this frame.

  Acceptance: `pnpm test` passes, and by hand R05's descent shows fog continuous through 32 km,
  recorded here with the pass's time.

- **R08.T9.b Surface lighting.** `irradiance.wgsl` builds the per-planet sky-irradiance table
  (64 × 16 by altitude and sun zenith, Bruneton 2017's layout) from the transmittance and
  multiple-scattering tables, one layer a latitude band (Design note 17). It is rebuilt with them
  and has a CPU twin in `tablesCpu.ts`. `surfaceLighting.wgsl` exports
  `atmosphere_sun_transmittance` and `atmosphere_sky_irradiance`, with the latitude and sun
  azimuth arguments of the Provides, for R07's `litBody.wgsl` callers, R10's `terrainLit.wgsl`
  and R11's lit passes. Both are catalogued.

  Tests (no GPU, twin):
  - at the top of the atmosphere the sun's transmittance is 1 and the sky's irradiance is 0;
  - with no scattering terms the irradiance is 0 at every altitude;
  - Earth's surface irradiance, direct plus diffuse at a sun zenith of 0°, lies between the
    direct beam alone and the top's; R08.T13 checks it against the reference;
  - on a Saturn-class figure at equal sun zenith, the pole's diffuse irradiance at the datum is
    below the equator's, by the band tables' column ratio.

  Smoke: the readbacks agree with the twin to 10⁻³. Acceptance: `pnpm test` and
  `just test-render` pass.

### R08.T10 The medium of a body

`assemble.ts`, `opticsWorker.ts`, `AtmosphereCache.ts` build, from `BodyAppearance` and the body's
surface section, in a module worker, cached as Design note 14 says. The surface section comes from
`body_detail`. `AtmosphereCache` requests it when a photorealistic view first draws a body in the
disc or mesh regime (R07's `litRegimes`), again when the scene's arrival or the granted detail
changes, and again when `body_events` reports an event on the body (P14.T31, once it exists). The
section's state decides the label. The build produces:

- the column;
- the molecular, absorber and aerosol terms;
- the labels of Design note 12.

An airless body (`SurfaceState::Airless`) draws no atmosphere and no label. Until P14.T24.a is on
the wire, generated bodies draw no atmosphere and show `ATMOSPHERE: NOT YET MODELLED`. The task
runs on the fixtures of Design note 16: Earth, Earth with ozone, Mars with hand dust, Venus at 92
and at 58 bar, a hand Titan, and a hand giant. Those that need held data wait on the licence
ruling (the task order's list).

Tests:

- a withheld section gives no medium and `atmosphereNotResolved`, and a `not_modelled` one
  `atmosphereNotYetModelled`;
- a request is sent once per body per trigger, and none for a point-regime body;
- an absent inventory gives no aerosol term and `aerosolsNotYetModelled`;
- the same inputs give the same key and a changed inventory another;
- results transfer, and the cache evicts least recently used within its stated size;
- each fixture's optics time is recorded, on a quiet machine.

Acceptance: `pnpm test` passes, and by hand a flight past every fixture is recorded here.

### R08.T11 The thin benchmarks

Record, by hand on the development machine's RTX 3080 (the discrete target) and, by the owner, on
the UHD 620 at 720p (low), against
the budget ([Performance budget](../../brainstorming/rendering-and-planets.md#performance-budget)):

- the per-frame atmosphere time, against 2–4 ms low and 0.5–1 ms discrete, at one sun and at the
  setting's `SKY_SUN_CAP`;
- the per-planet table time, against about 1 ms and under 0.1 ms;
- the table bytes a planet, against 2 MB, and the per-view bytes on their own line, against
  Design note 11's 0.43 MB, at one, two and four suns;
- the slice and band counts, bytes and per-planet table time of the Jupiter- and Saturn-class
  figures, against Design note 11's 0.57 and 0.72 MB, and any widening of `KAPPA_STEP` or
  `BAND_STEP`.

R05's `just descent-spike` runs with the generalised atmosphere, and its percentiles are recorded
against R05's. Where the low setting misses, `TABLE_SIZES` changes here. Acceptance: the record is
here, and `pnpm test` passes.

### R08.T12 The reference path tracer

- **R08.T12.a The tracer.** `crates/hyperion-fit/src/atmosphere/` holds a spherical backward
  Monte Carlo in `f64` (`Shells::Sphere`; R08.T12.d adds the spheroid). It reads a spectral case
  (`serde_json`, a workspace dependency, added to the crate) and has:
  - next-event estimation to each sun, and Russian roulette;
  - an explicit detector cone;
  - an optional Stokes mode for Rayleigh;
  - reproducible results for any thread count.

  Tests:
  - an absorbing-only medium gives Beer–Lambert to 10⁻⁹;
  - one and four threads give identical results;
  - the Stokes mode's I equals the scalar mode's for an isotropic scatterer.

  The scalar-against-vector Rayleigh difference is measured on one thin Rayleigh case of Natraj et
  al. 2009 and recorded as a finding for the owner. Acceptance:
  `cargo test -p hyperion-fit atmosphere`.

- **R08.T12.c The tracer's benchmarks.** Slow tests (`just test-slow`) run the benchmarks of Design
  note 10, and the tracer's references (R08.T12.b) are committed only once these pass:
  - Garcia and Siewert's Haze L and Cloud C1, Natraj et al.'s Rayleigh tables (Stokes),
    Kokhanovsky et al. 2010 and IPRT Phase A, each to 3σ with σ ≤ 0.3%;
  - Loughman et al. 2004 within its 2–4% spread.

  Acceptance: `cargo test -p hyperion-fit atmosphere` and `just test-slow`.

- **R08.T12.d The spheroid mode** (Design note 17). It needs R08.T12.a. `Shells::Spheroid`:
  free paths by delta tracking against a majorant (the density at the lowest gravity-scaled
  height the path can reach), boundary crossings against the spheroid shells as quadrics, and
  density at h·g(φ)/g_ref, with h the geodetic height by Vermeille's closed form (J. Geodesy 76,
  451, 2002) and g(φ) by Somigliana, both in Rust. Next-event estimation to each sun marches the
  same geometry. Tests:
  - a = c with ω = 0 agrees with `Shells::Sphere` to 3σ on the Earth case;
  - an absorbing-only Saturn-class medium gives Beer–Lambert against an `f64` quadrature of the
    density along the same chord to 10⁻⁶, grazing at the equator north and east and at the pole;
  - the normal gravity reproduces WGS 84's γ_e and γ_p to 10⁻⁹, as R08.T3.d's TypeScript does.

  Acceptance: `cargo test -p hyperion-fit atmosphere`.

- **R08.T12.b Cases and references.** It needs R08.T5, R08.T10's fixtures, R08.T12.c and
  R08.T12.d. It
  builds the case format, and a bless-style vitest (R08.T4.a's `bless.ts`) that writes the cases
  of Design note 16 from the client's optics, and `saturn-oblate`, Design note 17's thin
  Saturn-class case under `Shells::Spheroid` (H₂–He Rayleigh over a Lambertian 1-bar boundary;
  ground views at the equator looking north and east, at 60° and at the pole; the limb at 0.3, 1
  and 3 H over the equator and the pole). R08.T13's gate covers it with the thin cases. The
  command `hyperion-fit atmosphere-reference`
  traces them, and the references are committed with sample counts, times and load average.
  - A vitest asserts that every case equals what the optics produce now, and names the commands to
    regenerate both files.
  - The metric of Design note 10 is written once as `gate.ts`, with its own tests on synthetic
    inputs (the floor, the 3σ term, the cone average).
  - Sanity tests on the references:
    - the Venus-class case's surface downward flux is 2–4% of the top's at the sun of the Pioneer
      Venus large probe's solar flux radiometer (LSFR), with the surface sky red-shifted (Tomasko
      et al. 1980, 2.5%);
    - the Titan-class case's is 5–15% (Tomasko et al. 2008, marked to be re-read);
    - Earth's and Mars's aggregates are recorded.

  Acceptance: `pnpm test` passes and `just fit-check` is unaffected.

### R08.T13 Where the analytic term drifts

A vitest applies `gate.ts` to the CPU twin's thin tables with the analytic term on every case. The
subcommand `hyperion-fit atmosphere-sweep --out <sweep.json> [--smoke]` traces single-layer media
over vertical extinction optical depth, single-scattering albedo and asymmetry. Beside each point
it writes the twin's analytic radiances, which a bless-style vitest supplies as a case file. The
full sweep is run by hand and its output committed as `reference/ms-sweep.json`. From it,
`thick/regime.ts` sets `THICK_MS_BOUNDARY`, the τ*(ω, g) table of Design note 9, and
`classifyRegime(medium, cover)` routes by that table and by the per-body deck rule (τ above 10,
cloud fraction 1). Tests:

- Earth and Mars classify `thin` and pass the gate, or the finding is recorded and raised for a
  ruling;
- `saturn-oblate` classifies `thin` and passes the gate with R08.T6.e's slicing, and fails it with
  one slice and one band at the limb above 1 H (the check that the gate sees oblateness); both are
  recorded;
- two-layer media (Rayleigh over a g ≈ 0.7 aerosol) that straddle the boundary classify on each
  side of it as the sweep predicts;
- whether τ* collapses onto constant τ(1 − ωg) within one grid step is tested and recorded. Only if
  it does is the table replaced by one scalar;
- a body with a τ 30 deck and cloud fraction 0.6 is not `cloudDeck`, and with fraction 1 it is;
- R08.T9.b's surface irradiance at Earth, direct plus diffuse at a sun zenith of 0°, agrees with
  the reference's downwelling flux aggregate to 2%;
- the boundary table and the sweep's provenance are in its doc comment.

Acceptance: `cargo test -p hyperion-fit atmosphere` (the sweep's `--smoke` mode) and `pnpm test`
pass, and the full sweep is recorded here.

### R08.T14 The view-dependent multiple-scattering bake

Per Design note 9, in four subtasks.

- **R08.T14.a The plane-parallel solver.** `thick/discreteOrdinates.ts`: discrete ordinates in
  `f64`, 16 streams, about 64 layers, with all μ₀ solved on one factorisation. It is ported from
  Stamnes et al. 1988, never from GPL cdisort. Tests: Garcia and Siewert's Haze L and Cloud C1 to
  their published digits at 10⁻³; energy is conserved to 10⁻⁶ for ω = 1. A PythonicDISORT
  plane-parallel cross-check is run once by hand and recorded. Acceptance:
  `pnpm --filter hyperion exec vitest run view/atmosphere/thick/discreteOrdinates`.
- **R08.T14.b The bake.** `thick/bake.ts`: Dahlback and Stamnes's pseudo-spherical beam, delta-M
  with Nakajima and Tanaka's corrections, and the source function J_ms(h, μ₀, μ_v, m) for
  m = 0, 1. The solve is spectral at `BAKE_WAVELENGTHS_NM`, converted to channels on storage, and
  runs in the optics worker, once per latitude band of `medium.slicing` (Design note 17), each
  band's column at its own g(φ). Tests: with delta-M off and on, the truncated solve's flux matches
  the full solve's to 1%; the conversion of a flat spectrum is the identity; a one-band body bakes
  once, and a two-band body's bands differ in vertical optical depth by their gravity ratio.
  Acceptance: `pnpm test`.
- **R08.T14.c The read path.** The sky-view, aerial-perspective and march kernels read the thick
  table in place of σ_s·Ψ_ms·p_u (catalogued), one layer a band, interpolated at the sample's
  latitude. `TABLE_SIZES` gains the thick size, and the table is cached per world.
  `ATMOSPHERE: COMPUTING` shows while a gated bake runs. Acceptance: `pnpm test` and
  `just test-render` pass.
- **R08.T14.d The gates.**
  - The Venus-class cases (92 and 58 bar) and the Titan-class case, through the CPU twin with the
    baked table, pass `gate.ts` per geometry. Both m = 0 alone and m = 0..1 are run, and which is
    needed is recorded. The Rayleigh-only Venus columns gate before the licence ruling, the
    cloudy and hazy cases after it.
  - The bake time on the UHD 620's host (by the owner), summed over the bands, is under
    `BAKE_CEILING_S` on a quiet machine, or the Risks' fallback is taken and recorded. The bands
    nearest the camera bake first, and `ATMOSPHERE: COMPUTING` clears when the last lands.
  - When a regime's gates pass, `classifyRegime` routes its `thickScattering` worlds to the bake,
    and `ATMOSPHERE: APPROXIMATE` clears for them.

  Acceptance: `pnpm test`.

### R08.T15 The cloud-deck split

- **R08.T15.a Above the deck.** The deck as a reflecting boundary, with its bidirectional
  reflectance per channel from the solver over the deck's layers, baked per latitude band (Design
  note 17). Tests:
  - a thick non-absorbing deck reflects its incident flux to 1%;
  - on a Saturn-class figure, the deck lies at the same pressure in every band, and the column
    above it differs between bands by their gravity ratio;
  - the giant-deck case passes the gate.
- **R08.T15.b Below the deck.** The plane-parallel downwelling table by altitude, view angle and
  sun angle, one layer a band, gives sky and aerial perspective beneath. Tests:
  - the Venus-class surface passes the gate and the flux sanity test of R08.T12.b;
  - the table's surface flux equals the solver's;
  - below the deck `atmosphere_sky_irradiance` reads the table's downwelling flux, and the
    direct sun's transmittance is the deck's;
  - `classifyRegime` routes `cloudDeck` worlds once both gates pass, by the per-body rule of
    Design note 9: a deck above τ 10 with cloud fraction 1 splits, and a fraction below 1 does
    not.

  By hand, the Venus-class surface and a descent through its deck are recorded here.

Acceptance: `pnpm test`.

### R08.T16 Gas giants and the disc reflectance

- **R08.T16.a Giants.** The giant's medium is drawn inside 10⁹ m through R08.T15's split, with
  the deck as its surface and the limb by the march. It is built on the hand giant fixture, which
  carries Jupiter's figure and rotation (f = 0.065), so that Design note 17's slicing runs on it.
  When P14.T24.d and P14.T35.d are on the wire, the medium comes from the giant's `envelope`
  section, and the tests re-run on it. Tests:
  - the medium has no ground, and its deck lies at the pressure its input states;
  - once the envelope section exists, a generated Jupiter-class giant's medium places its NH₃
    deck at 0.5–1 bar, as P14.T24.d's own test requires;
  - its limb shell of about ten scale heights is resolved at 2.5 × 10⁸ m (the brainstorm's
    figure, under
    [The scales the view spans](../../brainstorming/rendering-and-planets.md#the-scales-the-view-spans)).
- **R08.T16.b `DiscReflectanceTable`.** `discReflectance.ts` bakes it spectrally from the medium
  and tables, one layer a latitude band (Design note 17). This task adds its read path to R07's
  `shaders/bodyDisc.wgsl`, read at each disc pixel's geodetic latitude, beyond
  `GAS_GIANT_FULL_PASS_BOUNDARY_M` for a body with an atmosphere, keeping the albedo-only shading
  for airless bodies, and updates the shader's catalogue entry. Tests:
  - at 10⁹ m the disc's integrated radiance agrees with the full passes' to 5% for the Jupiter and
    Earth fixtures, and for a Saturn-class figure at a phase angle of 0° and 90°;
  - a one-band body's table has one layer and reads as before;
  - its geometric albedo is reported beside R07's `BodyPhotometry`.

  By hand, a Jupiter fixture crossing 10⁹ m is recorded here.

Acceptance: `pnpm test` and `just test-render`.

### R08.T17 Verification pass

Run the by-hand scenes on the development machine's RTX 3080 (the discrete target) and, by the
owner, on the UHD 620, on a quiet machine. Record every figure here and in its doc comment, and run
the spectral check. Acceptance: `just ci`, `just test-slow` and `just test-render` pass, and the
records are here.

## Verification

- **Physics, automatic:**
  - Bucholtz's 4.51 × 10⁻²⁷ cm² to 1%;
  - Earth's 4.85, 11.5 and 28.7 × 10⁻⁶ m⁻¹;
  - each gas's reference state;
  - the column's hydrostatic and profile checks;
  - Mie against Wiscombe's cases;
  - the aggregates against optool;
  - every table's CPU twin against the reference on the stated metric (T13–T16), in `just ci`;
  - the surface irradiance against the reference's downwelling flux, and the summed sky-view against
    per-sun tables (T7, T13);
  - oblate bodies (Design note 17): normal gravity against WGS 84's γ_e and γ_p to 10⁻⁹ in both
    languages (T3.d, T12.d); the sliced twin against an `f64` brute-force spheroid march (T6.e);
    and `saturn-oblate` passing the gate with the slicing and failing it without (T13).
- **Reference, slow:** the tracer against Garcia and Siewert, Natraj, Kokhanovsky, IPRT A and
  Loughman, under `just test-slow`; its spheroid mode against its sphere mode at a = c (T12.d).
- **GPU, software:** every kernel compiles and agrees with its twin on SwiftShader under
  `just test-render`, asserting properties, never images.
- **By eye, recorded:**
  - Earth's sky from the ground at noon and sunset, and from orbit through the terminator;
  - the limb from 400 km;
  - Mars's butterscotch day and blue sunset;
  - the Venus-class surface and deck;
  - a Titan-class haze;
  - a Jupiter-class limb across 10⁹ m;
  - a Saturn-class limb from orbit over the equator and the pole (T6.f);
  - a binary star's two twilights;
  - fog through a descent;
  - stars dimmed near the horizon.
- **Spectral error:** the reference at 15 wavelengths against its three-channel run at Earth and
  Mars. Expect about 0.003 in u′v′ by day and up to 0.02 at low sun with the fitted triple (0.036
  with 680/550/440). Bruneton 2017's implementation (BSD-3) is the cross-check at Earth. A visible
  difference goes to the owner.
- **Benchmarks, recorded on a quiet machine:** per-frame and per-planet times on both settings,
  table bytes (with the oblate slice and band counts), optics-worker times, bake times summed over
  bands, and the references' trace times.

## Generator version

No change to generated output and no bump. The atmosphere is presentation computed in the client
from figures plan 14 generates. The asks of R08.T1 are plan 14's tasks, with plan 14's bumps.
Client caches key on `ATMOSPHERE_OPTICS_VERSION` instead. The plan reserves nothing in the
generator, and the reference's sampling needs no domain tag.

## Risks and open points

- **The atmosphere on strongly oblate bodies** (from R05's Risks; researched 2026-09-29, resolved
  by Design note 17, medium-high confidence). R05 Design note 16's r = √(MN) + h holds within
  Design note 10's 5% up to f ≈ 0.005, the Earth class. Past that, the limb at several scale
  heights fails first, because a medium built at one g is wrong in scale height by g_p/g_e: 17% on
  Jupiter and 33% on Saturn. The directional-curvature error is only ±f/2 in grazing optical
  depth. The earlier "17% at Saturn" compared curvatures that √(MN) already removes.

  Design note 17 therefore has every body use gravity-scaled height h·g(φ)/g_ref, with transmittance
  in curvature slices over κ = s·R_α ÷ R_ref and the multiple-scattering and baked tables in
  latitude bands. Earth-like bodies get one of each. Three things are still open:
  - the slice and band counts are computed, not measured, and wait on R08.T12.d's spheroid mode
    and T12.b's Saturn-class case;
  - the extra bakes may push a Saturn-class world past `BAKE_CEILING_S`;
  - zonal-wind gravity (about 1.4% on Saturn), the real 1-bar surface's departure from the
    spheroid, and T(p) varying with latitude are not modelled.

  Until R08.T6.f lands, a body with ln(κ_max ÷ κ_min) > 0.02 is drawn under
  `ATMOSPHERE: APPROXIMATE`. Sources: Chapman 1931; Heiskanen and Moritz 1967; Syndergaard 1998;
  Lindal, Sweetnam and Eshleman 1985 (full list in Design note 17).

- **Plan 14 produces no CH₄, O₂ or giant composition** (Design note 16). Titan-class haze, ozone and
  giants are fixture-only until P14.T24.c, d and f land. Biotic O₂ has no owner.
- **The stratosphere is provisional** (Design note 3). The skin is isothermal, with no ozone or
  haze heating, until R&C 2012's term follows P14.T24.c. Until P14.T24.e lands the isothermal seam
  stands, 3× wrong in scale height at Venus's cloud tops; the gate cannot catch it, because the
  reference traces the same column.
- **The bake's cost is an operation count, not a measurement.** If R08.T14 exceeds `BAKE_CEILING_S`,
  the fallback is a family of tables computed at development time. The research lean is that a
  family over (τ, ω, g, H/R) cannot represent a layered Venus, so it would need its own gate and
  probably a coarser claim.
- **Titan limb and low sun** may fail the pseudo-spherical gate by up to 8% (Loughman et al. 2004).
  The fallback for the failing geometries is a spherical Monte Carlo in the worker, with its noise
  and minutes of time.
- **H₂'s cross-section** (medium confidence): Peck and Huang's refractivity runs 6–7% above
  Dalgarno and Williams, a 6–7% difference in a giant's Rayleigh depth. Ford and Browne 1973 or a
  modern polarisability anisotropy would settle it.
- **Scalar radiance.** The drawn image is scalar, while the Rayleigh benchmark tables are vector,
  and the scalar error for Rayleigh can reach about 10%. R08.T12.a measures it and reports it as a
  finding.
- **Data licences.** Before these data are committed, the owner must decide on the Karkoschka and
  Tomasko coefficients (Elsevier), Serdyuchenko's data files (terms unstated), Wolff's Mars dust
  and Khare's tholin, and NH₄SH, which has no visible index at all.
- **The sun cap on the low setting** (Design note 7) drops the scattered sky of the third and later
  suns. Whether it needs an annunciation is for R12's audit.
- **Refraction and scintillation are not drawn.** On Venus, refraction near the surface raises the
  horizon; R06 leaves scintillation to this plan, and the brainstorm asks for neither. If the
  realism ruling wants them, they are later tasks. R10 asks this plan for the sun's refracted
  apparent elevation for its shadow test (R10 Design note 10). The ask stays open until refraction
  is drawn; until then R10 reads the geometric elevation.
- **The metric's floor is for the owner.** Design note 10's max(0.05·L_ref, 3σ_ref, 10⁻³·L_max)
  and the 2% flux aggregates are this plan's reading of the brainstorm's "to 5% in radiance". The
  floor relaxes the 5% only at radiances under 10⁻³ of the case's brightest, and where the
  reference's own noise is larger. The owner may tighten or loosen it, and the gate's tests take
  the floor as one named constant, so that a ruling changes one line.
- **Licences block the cloudy gates.** Until the owner rules on H₂SO₄, Mars dust and tholin, the
  Venus-class cloudy case, Mars and the Titan-class haze cannot be gated. Those regimes are drawn
  `ATMOSPHERE: APPROXIMATE` meanwhile. If a licence is refused, the lean is a published
  parameterisation cited without copying the table: a constant index with a published Cauchy
  slope, or a Henyey–Greenstein pair fitted in the paper. That lean needs its own physics check.
- **M-dwarf suns.** A curve of growth weighted by a 15-sample spectrum is adequate for FGK, A and B
  stars, but not for M dwarfs. Their TiO bands at 590–630 and 705–760 nm overlap Chappuis and
  methane's 727 nm band, so the weight correlates with σ (Design note 5). This is a recorded
  limitation. The lean is that R06 supply a model spectrum at 1 nm for M stars (for example
  PHOENIX, Husser et al. 2013, A&A 553, A6), an ask not yet made.
- **Titan's skin temperature.** Design note 3's Titan check takes T_skin = 64 K. A T_eq of
  83.5 K, from a Bond albedo of 0.265, would give T_skin 70 K and a tropopause at 0.40 bar,
  outside P14.T24.e's test. Where plan 14's Titan T_eq comes from is to be re-checked when
  P14.T24.e is built.
- **The summed sky-view** (Design note 7) is a research lean of medium confidence, not checked
  against Hillaire's parameterisation. R08.T7 tests it, and the per-sun fallback costs 0.43 MB a
  sun a view.
- **The reference in `hyperion-fit`** is a new kind of output for that crate. If plan 15's owners
  prefer, it moves to a sibling offline tool, and nothing else changes.
- **Knowledge.** Until a sensors plan builds the body-level overlay, the server grants the detail
  level asked, so `ATMOSPHERE: NOT RESOLVED` is exercised only by tests and by requests that ask for
  less.
