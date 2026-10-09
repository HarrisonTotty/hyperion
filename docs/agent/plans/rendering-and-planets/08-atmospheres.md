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

The names consumed from R01, R03, R05, R06 and R07 were those in their plans as written on
2026-09-29. They were reconciled with the code as built by the `revalidate-plan` skill on
2026-10-09, before the first task ran (Risks, "Re-validated at bce2aef5"): the names below are the
code's, and where R05's as built differs from this plan's first sketch, R05's shape is kept and
widened.

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
before any atmosphere is drawn from it, and the check runs in `just ci`. The reference is vector,
so polarisation's effect on the radiance is in what the tables are held to (Design note 10). Until
its gate passes, a thick world is drawn with the analytic term under the drafted label
`ATMOSPHERE: APPROXIMATE`, never presented as computed. The low setting is R05's
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
  irradiance there, which R07's, R10's and R11's lit passes read (R08.T9.b). Planetshine takes
  both, as starlight does, and lights the receiving body's sky (Design note 7).
- Thick atmospheres: the measured regime boundary, the view-dependent converged bake, the
  cloud-deck split, and the path-traced reference with the 5% gate.
- Polarisation's effect on the drawn radiance: a per-world correction from a low-stream vector
  solve, in both regimes (Design note 10, R08.T14.e–f).
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
- Refraction of rays, scintillation, airglow, aurorae, lightning, night-side thermal emission,
  and the polarisation state (Q, U, V) of the drawn image. The drawn radiance does include
  polarisation's effect on it (R08.T14.e–f). The reference tracer's Stokes mode is every
  gate's reference (Design note 10).
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
/** The render channels' wavelengths for smooth terms, nm (Design note 5). R05's constant, `Rgb`,
 * is (680, 550, 440) as built, the wavelengths R05's Earth constants are evaluated at. R08.T4.a
 * fits the triple from (620, 540, 445) into `channels.json`; R08.T6.d, which rebuilds Earth at it,
 * makes this constant read it. */
export const CHANNEL_WAVELENGTHS_NM: Rgb; // R05's, in `medium.ts`
/** The 15 bins every off-frame bake is solved in, nm: centres 392.67 + 25.33 k, k = 0..14, each
 * 25.33 nm wide over 380–760 (Design note 5). R06's `sky::colour::BAKE_WAVELENGTHS_NM` mirrors
 * it, and both are tested against `packages/protocol/fixtures/bake_wavelengths_nm.json`. */
export const BAKE_WAVELENGTHS_NM: ReadonlyArray<number>;
// `Rgb` is the one type R05, R07 and this plan share (`view/photometry/toneCurve.ts`).

export type DensityProfile =
  // R05's union, which gains `tabulated`
  | { readonly kind: "exponential"; readonly scaleHeightM: number } // R05's
  | {
      readonly kind: "tent";
      readonly bottomM: number;
      readonly peakM: number;
      readonly topM: number;
    } // R05's
  | {
      readonly kind: "tabulated";
      readonly altitudesM: Float64Array;
      readonly relative: Float64Array;
    };
export type PhaseFunction =
  // R05's union, whose `rayleigh` gains ρ and which gains `tabulated`
  | { readonly kind: "rayleigh"; readonly depolarisation: Rgb } // ρ per channel, Design note 4
  | { readonly kind: "cornette-shanks"; readonly asymmetry: number } // R05's Earth reference only
  | { readonly kind: "none" } // R05's: an absorption-only term (an absorber, Design note 5)
  | { readonly kind: "tabulated"; readonly table: PhaseTable }; // Mie, literature, aggregate
export interface PhaseTable {
  readonly u: Float64Array; // u = √(θ/π), 256 entries, dense near forward
  readonly values: readonly [Float64Array, Float64Array, Float64Array]; // a₁ per channel
  /** a₂, a₃, a₄, b₁, b₂ per channel on `u`, normalised as `values` is (block-diagonal; Hovenier,
   * van der Mee and Domke 2004). `undefined`: the source publishes no matrix, and the term is a
   * total depolariser in the reference and the polarisation correction (Design note 10). */
  readonly matrix: PhaseMatrixTable | undefined;
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
  // R05's `{ name, topHeightM, groundAlbedo, terms }`, widened. As R05 built it, the medium carries
  // no radius: the tables take the figure, and R_ref (Design note 17) is R05's
  // `tableRadiusM(figure)`, the mean radius (2a + c) ÷ 3 the per-planet tables are built on.
  readonly name: string;
  readonly referenceGravityMS2: number; // g_ref = √(g_e g_p), the column's gravity (Design note 17)
  readonly slicing: OblateSlicing; // κ slices and latitude bands; one of each for Earth
  readonly topHeightM: number; // R05's: geodetic height of the top
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
/** The level spheroid: R07's `BodyFigure` (a, c; `view/terrain/planet.ts`, from the wire's
 * `BodyFigureDto`), GM = G × the record's `mass_kg` (CODATA G, the sim's
 * `hyperion_base::units::GRAVITATIONAL_CONSTANT`), ω the spin rate at the scene time of the
 * record's rotation law (`BodySummaryDto.rotation`, P14.T46.f; the client's `lib/system/rotation.ts`),
 * or R05 Design note 14's test-planet period for R05's Earth. For a figure not flattened by its
 * spin alone, ω is Design note 17's ω_fig. */
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
export const ONE_SLICE_BELOW = 0.02; // ln(κ_max ÷ κ_min) under which one slice serves
export const ONE_BAND_BELOW = 0.02; // ln(s_max ÷ s_min) under which one band serves; never widened
```

### Optics (`rayleigh.ts`, `absorbers.ts`, `mie.ts`, `sizeDistribution.ts`, `aggregate.ts`, `materials/`)

```ts
// plan 14's `Gas` (`Hydrogen` … `Argon`, in `Gas::ALL`'s order), by formula
export type Gas = "H2" | "He" | "H2O" | "CH4" | "NH3" | "N2" | "O2" | "CO2" | "Ar";
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
reads (R08.T4.a). As built, renderer code, tests included, reads no files through `node:fs`
(R04.T10.c; `tsconfig.web.json` carries no Node types), and no TypeScript bless path exists: a
committed table is read as a JSON import, and `bless.ts` rewrites it through vitest's
`toMatchFileSnapshot`, whose update mode `HYPERION_BLESS=1` turns on in `vitest.config.mts`. The
raw inputs that are fetched and never committed (the CIE matching functions, the cross-sections;
R05.T12.d's ruling) are reduced by a Node tool in `apps/hyperion/src/tools/`, run by a script under
`apps/hyperion/scripts/`, as R05's `solarFactors.ts` and `solarFactors.mjs` are.

### Assembly, workers and cache (`assemble.ts`, `optics.worker.ts`, `AtmosphereCache.ts`)

The worker is `optics.worker.ts`, by the client's rule that worker files are `*.worker.ts` under
`tsconfig.worker.json`.

```ts
export function assembleMedium(input: BodyAtmosphereInput): AssembledMedium;
export interface AssembledMedium {
  readonly medium: AtmosphereMedium | undefined;
  readonly labels: ReadonlyArray<AtmosphereLabel>;
}
export type AtmosphereLabel =
  | "atmosphereNotResolved" // the section its atmosphere comes from is withheld
  | "atmosphereNotYetModelled" // the section its atmosphere comes from is `not_modelled`
  // (a giant's envelope while absent)
  | "aerosolsNotYetModelled" // no aerosol or absorber inventory
  | "atmospherePending" // its section is asked and the reply not yet drawn
  | "atmosphereComputing" // a gated thick bake is running
  | "atmosphereApproximate"; // a thick world whose gate has not passed, or an oblate body drawn
// with one slice before R08.T6.f (Design note 17); also after a failed bake, until the next lands
export class AtmosphereCache {
  /* per body and inventory hash; requests `body_detail` (`toBodyDetailRequest`,
     `lib/system/bodiesWire.ts`); posts work to the optics workers; one per device, holding each
     body's per-planet tables (R05's `AtmosphereTables`) for every view */
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
  /** The stars whose light the sky takes: the body's own, R07's `MAX_BODY_LIGHTS`, read and never
   * a second literal, so that the sky's stars are the surface's (Design note 7). */
  readonly skySunCap: number; // 2 on both settings while R07's count is 2; R08.T7
  /** The planetshine sources the sky takes: R07's `SETTINGS[s].photoreal.planetshineSources`. */
  readonly skyPlanetshineSources: number; // 2 high, 1 low
  /** The quality limits this plan lists for R12's audit, one string each. */
  readonly qualityLimits: ReadonlyArray<string>;
}
// R05's `ViewSettings` gains `atmosphereView: AtmosphereViewSettings`; `SETTINGS[s].atmosphereView`
export const ATMOSPHERE_VIEW_SETTINGS: Readonly<Record<QualitySetting, AtmosphereViewSettings>>;
export const SKY_SUN_CAP: Readonly<Record<QualitySetting, number>>; // SETTINGS[s].atmosphereView.skySunCap
/** The sources the sky's kernels hold: four stars (this plan's first high figure, for R07's
 * deferred raise) and two planetshine sources (Design note 7). */
export const MAX_SKY_SOURCES = 6;
/** Where aerial perspective applies (Design note 11): R05's `TableSizes.aerialPerspectiveScope`,
 * `SETTINGS[s].atmosphere.aerialPerspectiveScope`, projected here rather than held twice. "scene"
 * (high) applies it to everything opaque and publishes the volume; "terrain" (low) to terrain
 * alone, in R05's one deferred composite. */
export const AERIAL_PERSPECTIVE_SCOPE: Readonly<Record<QualitySetting, "scene" | "terrain">>;
export const ATMOSPHERE_QUALITY_LIMITS: Readonly<Record<QualitySetting, ReadonlyArray<string>>>;
```

R05's `TABLE_SIZES` (`hillaire.ts`), which is `SETTINGS[s].atmosphere`, stays in R05's module and
gains the thick table's size (R08.T14.c). As built it holds the transmittance (256 × 64) and
multiple-scattering (32²) sizes, the sky view (192 × 108 at 75 steps on high, 128 × 64 at 16 on
low), the aerial-perspective volume (32³ and 32 × 32 × 16, 2 steps a slice, to 32 km), the march
(full resolution at 32 steps, half at 16) and `aerialPerspectiveScope`. `ViewSettings` also holds
R06's `sky` and R07's `internalScaleBounds`, `budget` and `photoreal` today. `settings.ts` is the
module R12 reads for this plan's ladder entries.

### Tables and passes (`shaders/`, `hillaire.ts`), widening R05's `HillaireAtmosphere`

- R05's WGSL compute kernels, widened to read a storage buffer of N terms (in place of R05's
  `Medium` uniform of at most `MAX_TERMS` = 8 terms, packed by `tables.ts`'s `packMedium`), a 2D
  array texture of density tables (one layer a term) and one of phase tables. R05's kernels are
  assembled from the libraries `common.wgsl`, `medium.wgsl`, `view.wgsl` and `source.wgsl` (R05's
  Risks, "the medium read in place"), which these tasks widen as well. Two engine facts bind them
  (R01 as built): a compute pass binds no sampler (`ComputeBindings` has none), so the tables are
  read by `textureLoad` with R05's hand filtering (`common.wgsl`'s `bilinear`, which gains a
  `texture_2d_array` overload); and a compute binding is viewed at the dimension its kernel's WGSL
  declares, so a texture of one layer binds where a kernel declares `texture_2d_array` or
  `texture_storage_2d_array`, as a one-layer array (R08.T0, R01's seam; `decision-r08-design.md`
  item 1). A one-slice, one-band or one-term table is therefore an ordinary array of one layer. A
  medium with no term of a kind binds a one-texel placeholder of one layer, since a texture has at
  least one. The kernels:
  - `transmittance.wgsl`, which stores optical depth, curves of growth included (Design note 8),
    in a 2D array texture of one layer a κ slice (Design note 17);
  - `multiScattering.wgsl`, Hillaire's isotropic 32² table for `thin` worlds, one layer a latitude
    band;
  - `skyView.wgsl`;
  - `aerialPerspective.wgsl`.

  Beside them, `rayMarch.wgsl` is R05's compute kernel for views from outside and for terrain
  beyond the aerial-perspective reach, writing a ray-march target at the setting's scale, and
  `composite.wgsl` is R05's composite, a full-screen material draw that `drawFrame` returns as a
  `DrawItem` (it reads the scene's colour and depth), widened to several suns.
  New: `irradiance.wgsl`, a per-planet table of the sky's diffuse irradiance on a horizontal
  surface by altitude and sun zenith, one layer a latitude band, and `surfaceLighting.wgsl`, which
  exports `atmosphere_sun_transmittance(altitude_m, mu_sun, latitude_rad, sun_azimuth_rad, curve)`,
  `atmosphere_sky_irradiance(altitude_m, mu_sun, latitude_rad)` and
  `atmosphere_source_transmittance(altitude_m, mu_centre, latitude_rad, azimuth_rad, angular_radius_rad, curve)`,
  each `-> vec3f`. The last is Design note 7's three-node disc rule for a wide source
  (planetshine). `curve : u32` is the light's slot among the body's per-sun absorber curves
  (Design note 5), and a planetshine source passes slot 0. They serve the lit passes of R07, R10
  and R11 (R08.T9.b). The latitude and azimuth arguments are Design note 17's;
  `altitude_m` is the geodetic height, scaled inside. `oblate.wgsl` holds the shared WGSL: normal
  gravity, R_α, κ and the slice and band reads. Every kernel is registered in R01's
  `WGSL_CATALOGUE` (`view/engine/catalogue.ts`; R05's are `ATMOSPHERE_TABLE_ENTRIES` and
  `ATMOSPHERE_VIEW_ENTRIES`; a compute entry carries no `displayName`, a material such as the
  composite's `"ATMOSPHERE"` does) and created through `RenderEngine.createCompute(KernelPair)`,
  `presentation-only`, with no subgroup variant.

- `tablesCpu.ts` is a CPU twin of every kernel in `f64`, built on R05's `opticalDepth.ts` oracle
  and `marchSteps.ts` (the twin of R05.T12.e's step placement), which it extends rather than
  replaces. It is the test oracle and the smoke harness's comparison.
- `HillaireAtmosphere` in `hillaire.ts` is R05's class, widened. As built it is
  `new HillaireAtmosphere(engine, medium, tables: TableSizes, figure: SpheroidFigure)`, owns its
  per-planet `AtmosphereTables` (`tables.ts`; `setMedium(medium, bottomRadiusM): boolean`, built at
  `tableRadiusM(figure)`), and has `setMedium(medium): void`,
  `drawFrame(view: AtmosphereCamera, sun: SunState, scene: AtmosphereScene): DrawItem`,
  `aerialPerspectiveVolume()`, the smoke page's `frameTables` getter and `dispose()`. `SunState` is
  `{ directionBodyFixed, distanceAu, angularRadiusRad }`, its light R05's `solar.ts`. Widened:
  - `setMedium(medium)` builds the per-planet tables, shared by every view on the device, one
    transmittance layer per κ slice and one multiple-scattering and irradiance layer per band of
    `medium.slicing`. Sharing moves the per-planet `AtmosphereTables` out of the per-view class
    into `AtmosphereCache` (R08.T10.a), which hands them to each view's `HillaireAtmosphere`;
  - `drawFrame(view, suns: ReadonlyArray<SunState>, scene)` builds the per-view tables, summed over
    the drawn suns (Design note 7), and returns R05's composite `DrawItem`. `SunState` gains each
    sun's per-channel illuminance, R07's `starIlluminance` (R08.T6.d);
  - `aerialPerspective(view)`, widening R05's `aerialPerspectiveVolume()`, exposes the volume and
    the transmittance table to R11, the table as its 2D array texture of κ slices, read through
    `oblate.wgsl`'s slice lookup;
  - `skyView(view): SkyViewTable` exposes the view's sky-view table (R05's `frameTables.skyView`),
    the sky's radiance by local azimuth and elevation summed over the drawn suns, for R11's sky
    reflection in the ocean. If R08.T7's check forces the per-sun fallback, it returns the per-sun
    tables with their suns instead, in a field of the same type.
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
/** ΔJ_pol(h, μ₀, μ_v, m), m = 0..2: the vector-minus-scalar source function (I) of one 8-stream
 * discrete-ordinates solve of the medium, at the render channels, for one latitude band. Added
 * to the multiple-scattering term of thin and thick worlds alike (Design note 10, R08.T14.e). */
export function bakePolarisationCorrection(
  medium: AtmosphereMedium,
  band: number,
): PolarisationTable;
```

Files: `thick/regime.ts` (R08.T13), `thick/discreteOrdinates.ts` and `thick/bake.ts` (R08.T14),
`thick/polarisation.ts` (R08.T14.e), `thick/deck.ts` (R08.T15).

### For R07's analytic disc (`discReflectance.ts`)

`bakeDiscReflectance(medium, appearance: BodyAppearance) -> DiscReflectanceTable` is this plan's
name; `DiscReflectanceTable` is the name R07 consumes. It gives the reflectance by phase angle and
disc position per channel, baked spectrally, one layer a latitude band of the medium's slicing,
read at each disc pixel's geodetic latitude (Design note 17). R07's `view/shaders/bodyDisc.wgsl`
reads it beyond `GAS_GIANT_FULL_PASS_BOUNDARY_M` (`view/bodies/regime.ts`, 10⁹ m) for a body with
an atmosphere, in place of its albedo-only shading; R08.T16.b adds that read path to R07's shader,
which as built has no placeholder for it. `bodyDisc.wgsl` is a library since R07.T9: the disc's
entry points are in `bodyDiscDraw.wgsl`, and the materials `bodies:disc` and `bodies:discLimb`, the
smooth mesh and the `disc cells` kernel share its source and `view/bodies/draw.ts`'s
`DISC_TEXTURES`, with group 2's bindings 0–5 taken.

### Offline reference (`crates/hyperion-fit`)

```rust
// crates/hyperion-fit/src/atmosphere/
pub struct AtmosphereCase { /* a spectral medium, as the TypeScript writes it, its `Shells`,
    and its geometry set */ }
pub enum Polarisation { Scalar, Stokes }                 // Stokes: every gate's reference, Design note 10
pub enum Shells {                                        // Design note 17
    Sphere { radius_m: f64 },
    /// The level spheroid; density at the gravity-scaled height h·g(φ)/g_ref, by delta tracking.
    Spheroid { equatorial_radius_m: f64, polar_radius_m: f64, gm_m3_s2: f64, omega_rad_s: f64 },
}
pub struct ReferenceRadiances { /* per geometry and wavelength: radiance, standard error, samples,
    detector cone; the two flux aggregates; in the Stokes mode also Q, U, V and the scalar
    radiance of the same paths, each with its error */ }
pub fn trace_reference(case: &AtmosphereCase, polarisation: Polarisation, samples: NonZeroU64,
    threads: NonZeroUsize) -> ReferenceRadiances;        // backward Monte Carlo, f64, over `Shells`
```

Command: `hyperion-fit atmosphere-reference <case.json> --out <reference.json> [--samples N]
[--threads N] [--stokes]`, a clap variant of `cli.rs`'s `Command` dispatched in `run_in`, as
`orbits` is; it is not a `FitTask`, so `tables.lock` and `just fit-check` do not cover its output.
`--threads` is the crate's `Option<NonZeroUsize>`, and the output is the same for any number by the
crate's convention (`parallel::map_reduce_chunks`, with counter-based draws keyed per item as
`tasks/displaced_forms/births.rs`'s `Draws` are). Fixtures:
`view/atmosphere/reference/{earth,earth-ozone,mars,venus-92bar,venus-58bar,titan-class,giant-deck,saturn-oblate}.case.json`
and their `.reference.json`, formatted so that `prettier --check .` (in `just ci`) passes, or
listed in `.prettierignore` as written data, and each under the repository's 500 kB added-file
hook. The metric is `gate.ts` (R08.T12.b).

## Consumes

Names are those the owning plans give; the owning plan is authoritative.

- **Galaxy plan 14:**
  - P14.T13's `derive::atmosphere::{Atmosphere, Gas, PartialPressures, SurfaceState}` (built:
    `crates/hyperion-sim/src/planetary/derive/atmosphere.rs`). As built it fills only H₂O, CO₂,
    N₂ and Ar (H₂, He, CH₄, NH₃ and O₂ stay 0, read only for Jeans retention), and gives its Venus
    58 bar and its Titan 1.4 bar with no methane (Design note 16). `Atmosphere` holds the state,
    surface temperature, surface pressure (`None` for a gas envelope), partial pressures, grey
    optical depth, Bond albedo and cloud fraction, in the sim only. `Gas::molar_mass_g_per_mol` is
    IUPAC 2021's (H₂ 2.016 … Ar 39.95).
  - These reach the client through P14.T24.a's `SurfaceConditions` in the record's surface
    section. That section is today the sim's uninhabited `record::Surface {}`
    (`crates/hyperion-sim/src/planetary/record.rs`) and the wire's `BodySurfaceDto {}`
    (`crates/hyperion-protocol/src/planetary/record.rs`; TypeScript `never`), on `BodyRecordDto`
    only, so the server answers `not_modelled` for every planet and moon with a surface and
    `not_applicable` for a giant. P14.T24.a and T24.b are not built (`planetary/hooks/` holds only
    `mod.rs` and `seed.rs`), and **no plan-14 task gives the section its fields**: P14.T35.b only
    tags it `not_modelled`. R08.T1's amendment P14.T35.e asks for them with T24.c–f's (P14.T35.d is
    taken, "Body-state times beyond 2⁵³ s"). As written into plan 14 and reconciled with R09.T0.a's
    asks, P14.T48.e gives the record's section its contents and P14.T35.e puts all of it on the
    wire, with an `envelope` section for every gas-envelope body (Risks, "The asks R08.T1 wrote
    into plan 14").
  - The `body_detail` request (`BodyDetailRequest { universe, body, time, detail }`) and its
    `BodyDetailDto { universe, time, granted, record: BodyRecordDto }` (P14.T35.b–c, built:
    `crates/hyperion-protocol/src/{envelope.rs, planetary/requests.rs, planetary/record.rs}`),
    which is the only message carrying the surface section: the scene's `BodySummaryDto` carries
    `mass_kg`, `orbit`, `moons`, `rings`, `population`, `bulk` and, at `bulk`, the optional
    `rotation`, `figure` and `photometry` sections. The client's request is `toBodyDetailRequest(universe, body, time)`
    (`lib/system/bodiesWire.ts`), which only the `SYSTEM` display uses today (`useBodyDetail`).
    The section's state (`ok`, `not_resolved`, `not_modelled`, `not_applicable`) is how this plan
    learns the granted detail for a body. `body_events` (P14.T35.c, after T31) says when a body's
    figures change with time; it answers `unsupported` until P14.T31.
  - P14.T24.b's cloud fraction, for `classifyRegime`'s per-body rule (Design note 9). Not on the
    wire: the sim's is `SurfaceState::cloud_fraction`, a constant of the state (1 for a gas envelope
    or a runaway greenhouse, 0.67 temperate, 0 otherwise), and the photometry section carries
    neither it nor the surface pressure.
  - The bulk section's radius and surface gravity (built: `BulkPropertiesDto.radius_m` and
    `surface_gravity_m_s2`).
  - The body's mass, for `oblate.ts`'s GM = G·M (built). It is not in the bulk section: ruling 53
    of 2026-09-22 shows it at the `mass_and_orbit` level in a section of its own,
    `BodySummaryDto.mass_kg` and `BodyRecordDto.mass_kg` (`SectionDto<f64>`, kg), the sim's
    `BodyRecord` mass section. G is `hyperion_base::units::GRAVITATIONAL_CONSTANT`,
    6.674 30 × 10⁻¹¹ (CODATA 2018, unchanged in 2022); the client's one copy is module-private in
    `lib/scene/sceneWire.ts`.
  - The body's rotation rate, for `oblate.ts`'s ω (built: P14.T14.a–c, and on the wire by
    P14.T46.f). `BodySummaryDto.rotation` (`BodyRotationDto`, at `bulk`) carries the rotation law,
    whose rate moves from `initial_rate_rad_s` to `locked_rate_rad_s` over the locking age; the
    client reads it as `SystemBodyRotation` (`lib/system/bodiesWire.ts`) and turns it with
    `lib/system/rotation.ts`'s `rotationAngleAt` and `bodyFixedAxesAt`, whose rate at an age is
    private (`rateAtAge`). R05's test planet carries its own fixed pole and 86,164.0905 s sidereal
    period, this plan's ω for R05's Earth. A body whose rotation section is not `ok` takes
    R08.T3.d's one-slice fallback.
  - The figure (built, P14.T46): `BodySummaryDto.figure` (`BodyFigureDto`: equatorial and polar
    radii, flattening, pole, datum `solid_surface` or `one_bar`, `moment_of_inertia_factor` and
    `law` (`FigureLawDto`)), whose radii and pole R07's `BodyFigure` reads
    (`WireAppearance.figure`). The factor and the law, which `oblate.ts`'s ω_fig reads (Design
    note 17), are `SystemBodyFigure.momentOfInertiaFactor` and `.law` (`lib/system/model.ts`),
    passed in by R08.T10.a.
  - `DetailLevel::Surface`, and `Section::{NotResolved, NotModelled, NotApplicable}` (built;
    `DetailLevelDto` and `SectionDto` on the wire).
  - The asks of R08.T1: P14.T24.c–f and P14.T35.e.
- **R01:**
  - `RenderEngine.createCompute(pair: KernelPair): ComputeHandle` (and `createComputeAsync`), and
    `dispatch(kernel, bindings: ComputeBindings, workgroups, pass?)`, where `ComputeBindings` holds
    `uniforms`, `buffers`, `sampled` and `storage` and no samplers; a 2D array texture is
    `TextureSpec.dimension: "2d"` with `depthOrArrayLayers` ≥ 1, viewed in a compute binding at the
    dimension its kernel's WGSL declares, so that a one-layer texture is a one-layer `2d-array`
    where the kernel declares an array (R08.T0); `readTexture(texture, level?, rect?, access?)`
    reads every layer;
  - `WGSL_CATALOGUE` in `view/engine/catalogue.ts`;
  - the headless SwiftShader smoke harness (`renderer/src/smoke/`, `just test-render`), on its two
    variants, `default` and `no-subgroups` (`smoke/page.ts`'s `VARIANTS`); there is no `no-f16`
    variant, and no WGSL enables `f16`. It stays outside `just ci` (R01.T9.e), so every task that
    adds or changes a catalogued kernel runs `just test-render` as part of its own gate.
- **R02:**
  - camera-relative `f64` differencing and the rotation-only view matrix;
  - reversed-Z and the transparent-layer order: the atmosphere tests depth and writes none;
  - the photometric pipeline: V = 0 at 2.54 µlx (`V0_ILLUMINANCE_LX`,
    `view/photometry/magnitude.ts`), pre-exposure, `rgba16float` targets;
  - `ViewLabelBlock` (`displays/view/ViewLabelBlock.tsx`), and the nine guide items it drafted, of
    which items 2 and 7 bear on this plan. A photorealistic view's statements are built by
    `photorealStatements(run, lighting, drawn, labels)` (`displays/view/viewRun.ts`), the lit
    bodies' labels gathered by `litLabelsOf(scene)` (`displays/view/photorealFrame.ts`): this
    plan's labels join them there.
- **R03:** `sceneAt(model, observer, time, previous): SceneFrame | null` (`lib/scene/apparent.ts`).
  A `placed` body has `geometricM`, `apparentM` and `level` (its granted `DetailLevelDto`, per body
  since R03 Design note 13); a `contact` body has no `geometricM` and draws no atmosphere; the
  ship's local body is `SceneFrame.localBody`. A change of `level` triggers a rebuild (Design note
  14). The scene's `SceneBodyDto.record` is a `BodySummaryDto`, so the surface section itself still
  comes from `body_detail`, and its section state decides the label.
- **R05:** `view/atmosphere/` as its Design note 16 builds it:
  - `medium.ts`'s `MediumTerm`, `DensityProfile`, `PhaseFunction`, `AtmosphereMedium` and
    `CHANNEL_WAVELENGTHS_NM` (the shapes in Provides), with `densityAt`, `columnLengthM`,
    `extinction` and `termNamed`; `earth.ts`'s `EARTH_REFERENCE` (Rayleigh, exponential at the US
    Standard Atmosphere's 8,434.5 m, 4.848, 11.487 and 28.71 × 10⁻⁶ m⁻¹; a continental aerosol,
    exponential at 1.2 km, τ(550) 0.1, Ångström 1.3, ω 0.92, Cornette–Shanks g 0.584; ozone, an
    absorption-only tent at 10, 25 and 40 km with phase `none`, the module-private `OZONE_TERM`)
    and `HILLAIRE_REFERENCE` (sebh's Earth, for comparison); `HillaireAtmosphere` (Bevy 0.19's WGSL
    port, with sebh's reference) in `hillaire.ts`; `AtmosphereTables` and `packMedium` in
    `tables.ts`; the `f64` oracle `opticalDepth.ts`; `marchSteps.ts`, the twin of R05.T12.e's
    step placement with its gate in `marchSteps.test.ts`; `solar.ts`'s `sunIlluminanceRgb()` and
    `skyLuminanceScale()`; the smoke checks `checkAtmosphereTables`, `checkAtmosphereFrames`,
    `checkAtmosphereSteps` and `captureAtmosphere` (`renderer/src/smoke/atmosphere.ts`).
  - `TABLE_SIZES: Record<QualitySetting, TableSizes>`, which is `SETTINGS[s].atmosphere` of R05's
    one settings list (`view/quality/qualitySetting.ts`, R05 Design note 26). This includes its
    low sizes: sky-view 128 × 64, aerial perspective 32 × 32 × 16, the march at half resolution,
    and aerial perspective in one deferred pass on terrain alone (`aerialPerspectiveScope`
    `"terrain"`).
  - The transmittance table stores e^(−τ) in `rgba16float`, read as transmittance in four places
    (`source.wgsl`'s `tableTransmittance`, `multiScattering.wgsl`'s `transmittanceToSun`,
    `composite.wgsl`'s `transmittanceToSpace` and `rayMarch.wgsl`'s ground hit), and the smoke check
    "R05.T12.b every transmittance texel is finite and within [0, 1]" holds it there.
  - Design note 16's lookups over the rotational spheroid: r = √(MN) + h with h the geodetic
    height, μ against the spheroid normal, and the march clipped against the spheroid shells
    (researched 2026-09-29), which this plan's widened tables keep. The per-planet tables are
    built on `tableRadiusM(figure)`, (2a + c) ÷ 3; the per-view tables on the camera's √(MN).
  - `atmosphereInputs(camera: Pick<AtmosphereCamera, "positionM">, figure: SpheroidFigure)` in
    `hillaire.ts` (R05.T12.c), Design note 16's inputs at the camera (`AtmosphereInputs`, the
    radius, height and normal), which R08.T6.f widens in R05's file with the camera's latitude
    and s;
    `geodeticOf(p, figure)`, a fixed-point iteration that fails to converge above a flattening of
    about 0.04 (R05's Risks), which R08.T6.e replaces.
  - Where it is drawn: only by the spike (`view/spike/spikeRun.ts`) and the smoke page. R07's
    `PhotorealRenderer` draws no atmosphere yet (R07, below).
  - The `MemoryCategory` members `atmosphere-tables` and `atmosphere-view` that R05.T12.b and T12.c
    add under R12's names. This plan allocates every per-planet table under the first, the gated
    thick bakes included, and every per-view table, summed over suns, under the second (R12
    Design note 6).
  - The terrain pass (`view/terrain/terrainPass.ts`'s `TerrainPass`).
  - `HeightWorkerPool`'s module-worker and transfer pattern (`view/terrain/workers/pool.ts`).
  - The spike's `metrics.ts` (`view/spike/metrics.ts`, `SpikeMetrics`) and `just descent-spike`.
- **R06:**
  - `HostDiscDto` (R06's wire form of `HostDisc`; `crates/hyperion-protocol/src/sky.rs`,
    generated in `packages/protocol/src/generated/`), carrying each host star's `StarColour`
    fields (`chroma`, `lux_per_v0`) and `bake_spectrum: [f64; 15]` (`SKY_BAKE_BINS`); its luminance
    triples are in B, V, R order. The last is built by R06.T3.c and T10: the colour row's spectrum
    averaged over each bin of `BAKE_WAVELENGTHS_NM` and normalised to unit photopic illuminance by
    the 15-bin sum (Σ 683 ȳᵢ Sᵢ Δλ = 1 lx), so a bake's luminance scales by the star's lux. R06 has
    no TypeScript `StarColour`; this plan reads the fields off `HostDiscDto`. R06 pins the bins in
    `packages/protocol/fixtures/bake_wavelengths_nm.json`, which no TypeScript reads yet.
  - The sky layers, which this plan dims by transmittance: the sprites (`skySpriteStars` and
    `SKY_SPRITE_HDR_MATERIAL`, `view/sky/sprites.ts` and `spriteHdr.ts`), `BandLayer`,
    `SkyCubeLayer` (the baked cube, which holds most stars) and `HostDiscLayer`, built in
    `displays/view/viewFrameDrawer.ts`. As built, none takes a per-direction factor: each takes one
    scalar `exposureScale`. They are drawn into the scene target before the atmosphere (R07's pass
    order).
  - **Asked of R06 (open, outside R06's scope):** a 1 nm model spectrum per colour-table row for M
    stars, for the curves of growth (Design note 5, Risks).
- **R07:**
  - `starIlluminance(disc: HostDiscDto, distanceM: number): Rgb` (`view/lighting/illuminance.ts`),
    with `Rgb` from R02's `view/photometry/toneCurve.ts`, the one per-channel type R05, R07 and this
    plan share; `shiningStars` and `lightsAt(pointM, hosts, max)` (`view/lighting/hostLights.ts`),
    the latter brightest first, which R07 calls with `MAX_BODY_LIGHTS` = 2: a lit body takes at
    most two stars (`MAX_DISC_LIGHTS` = 2 in `bodyDisc.wgsl`), and
    `planetshineSources(body, lit, max)` with its `SecondarySource`s
    (`view/lighting/planetshine.ts`), at `SETTINGS[s].photoreal.planetshineSources` (2 high, 1
    low). The arrays `planLitBodies` builds for a body are this plan's list of that body's sources
    (Design note 7);
  - the body's appearance. `BodyAppearance` (with `BodyPhotometry`, `view/appearance/`) exists,
    but as built no production code builds it: the scene carries R07's `WireAppearance`
    (`heldAppearanceOf`, `view/scene/fromServer.ts`; its `figure: BodyFigure | null`, photometry
    and labels), and the view builds `LitBodyInput` (`litBodiesOf`,
    `displays/view/photorealFrame.ts`). This plan reads the `WireAppearance` per body, with the
    regime its caller decides;
  - `litRegimes(bodies: LitSphere[], camera, viewport, previous)` (`view/bodies/regime.ts`, called
    from `planLitBodies` in `view/bodies/draw.ts`) and `GAS_GIANT_FULL_PASS_BOUNDARY_M` (10⁹ m),
    to which nothing yet applies hysteresis: R07 leaves it to this plan's caller
    (`REGIME_HYSTERESIS` = 0.1 is beside it);
  - `view/shaders/bodyDisc.wgsl`, into which R08.T16.b adds the `DiscReflectanceTable` read path
    (Provides, "For R07's analytic disc");
  - `view/shaders/litBody.wgsl`, whose stubs `atmosphere_sun_transmittance` (returning 1) and
    `atmosphere_sky_irradiance` (returning 0) take R08.T9.b's signatures (Provides, "Tables and
    passes"). Their callers in `bodyDisc.wgsl` pass altitude 0,
    latitude 0 and azimuth 0, call the sky term once at the first light's μ₀ times the shares'
    mean A, and send planetshine through `atmosphere_sun_transmittance` too (R07's; kept and
    widened by this plan, Design note 7); `view/appearance/litBodyProbe.ts` and its smoke check
    pin the stubs' 1, 1 and 0;
  - `METER_CLASS` (R07 Design note 10; `view/post/meter.ts`): every translucent pass of this plan
    into the HDR target blends its alpha with source factor zero and destination factor one, so
    that R07's meter class survives, through R01's `blend` modes `"premultiplied"` and
    `"additive"`, both of which do so (R01 Design note 21, R01.T8.i);
  - `photorealisticPasses` (`view/photoreal/passes.ts`), with its slot `"atmosphere"` (owner R08,
    `built: false`, after the discs and before R11's rings), which is metadata only:
    `PhotorealRenderer.render` (`view/photoreal/renderer.ts`) hard-codes the passes it draws and
    draws no atmosphere, so filling the slot means adding the passes there (R08.T10.b);
    `viewBudgets(views, setting)` (`view/budget/viewBudget.ts`);
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
       are binned to 1 nm, and Karkoschka and Tomasko's methane coefficients are used at their own
       resolution, never binned coarser, since they are band-model values for e^(−ku) at their
       stated resolution (10 cm⁻¹ below 19,300 cm⁻¹). R08.T4.b takes them from NASA PSG's
       conversion (`decision-r08-licences.md` row 1).
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
   sunset and a 1/x-wide peak starves on a cos θ grid. Beside a₁ the table carries the scattering
   matrix's other elements where the source gives them: spheres from Mie's S₁ and S₂, and
   aggregates if T5.c's matrix agrees with optool's. A term with none is a total depolariser
   (Design note 10, Polarisation). How the forward peak is handled:
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
   - **One list of sources per body** (decided 2026-10-09, `decision-r08-design.md` item 3). A
     body's air is lit by exactly the sources that light its surface in the same frame, so that no
     image shows a lit sky over unlit ground or the reverse.
     - **The list.** The body's stars as R07 takes them (`lightsAt(centre, hosts, MAX_BODY_LIGHTS)`,
       past `STAR_CUT_RELATIVE`, brightest first), and its planetshine sources
       (`planetshineSources`, at the setting's count). The arrays R07's `planLitBodies` builds for
       the body are passed on, never ranked again.
     - **The counts.** `SKY_SUN_CAP` reads `MAX_BODY_LIGHTS`, 2 on both settings. The kernels hold
       `MAX_SKY_SOURCES` = 6, so a raise of R07's count changes no kernel. The per-frame bound is
       2 + 2 sources on high and 2 + 1 on low.
   - **Inside the air, the sky loop skips** two kinds of source whose contribution to the sky cannot
     be seen. The surfaces keep both kinds.
     - _Below the twilight limit:_ one whose upper limb lies below e = −[acos(R ÷ r_c) +
       2 acos(R ÷ R_top)] at the camera, with R the figure's smallest radius of curvature c² ÷ a,
       r_c = R + the camera's geodetic height and R_top = R + `topHeightM`. It lights no air the
       camera can see: 20.2° on Earth's ground with a 100 km top, beside astronomical twilight's
       18° (single scattering, refraction not drawn).
     - _Negligible:_ one under `STAR_CUT_RELATIVE` of the brightest source above the camera's
       horizon, such as the Moon by day, at 2.5 × 10⁻⁶ of the Sun.

     Outside the air (R08.T8) no source is skipped.

   - **A star outside the body's list lights neither its surface nor its air.** Only triple and
     higher systems have one. The galaxy's multiplicity follows Moe and Di Stefano 2017 (galaxy
     plan 11), and a third star passes the cut at a planet at a_p only from within about
     100 a_p √(L₃ ÷ L₁).
     - **Its share** is between 10⁻⁴ and roughly 10⁻¹ of the brightest star's. By day its omission
       is a few percent at most and unseen.
     - **While the two brightest are below the horizon** it is the only light, about 13 lx at the
       cut to 13,000 lx at a share of 10⁻¹ at Earth's insolation, and the view shows it as a disc
       over a night.
     - **The record.** That limit is in `ATMOSPHERE_QUALITY_LIMITS`. Raising R07's count, and
       letting a set star yield its slot, are deferred corrections to R07 (Risks).
   - **Surface lighting.** Every lit pass (R07's bodies, R10's terrain, R11's clouds, oceans and
     rings) takes each sun's illuminance times the transmittance from the lit point to that sun,
     read from the shared transmittance table, plus the sky's diffuse irradiance from a small
     per-planet irradiance table, as Bruneton 2017 does (his irradiance texture, 64 × 16 by altitude
     and sun zenith). Under a cloud-deck split the irradiance below the deck comes from the deck
     tables (R08.T15.b).
   - **Planetshine** (decided 2026-10-09, `decision-r08-design.md` item 2). A neighbour's reflected
     light crosses the receiving body's air as starlight does. Every source of R07's
     `planetshineSources`, at the setting's count (`SETTINGS[s].photoreal.planetshineSources`, 2 on
     high and 1 on low), is a source of this plan's wherever a star is:
     - on lit surfaces (R08.T9.b), its transmittance from the lit point along its direction, and the
       sky's diffuse irradiance it gives there, its illuminance times `atmosphere_sky_irradiance` at
       its own μ. The sky term is added below the local horizon as above it, as a star's is in
       twilight.
     - in the sky-view, the aerial-perspective volume and the march (R08.T7, R08.T8), a point
       source at its centre, with its per-channel illuminance (the neighbour's reflected colour) and
       the planet's shadow. It has no disc of its own, since R07 draws the body.

     Its direct transmittance is integrated over its disc, since a giant seen from a near moon is
     wide. The rule takes three nodes along the disc's vertical diameter, at the centre and at
     ±(√2/2)ρ in zenith angle, with weights ¼, ½ and ¼ times each node's μ. Nodes below the horizon
     are dropped. It is Gauss–Chebyshev of the second kind, exact for a uniform disc to degree 5.
     For ρ = 9.8° (Jupiter from Io) under τ = 0.25, the centre alone reads 19% low at 10° of
     elevation and 63% low at 5°, where the rule is within 0.2% and 4%. Under τ = 1 the centre
     alone is 17% low at 20° and the rule under 0.1% (computed for a uniform disc with Kasten and
     Young 1989's air mass). Stars keep the centre, their discs being under about 1°. A planetshine
     source takes the absorber curve of the receiving body's brightest star (light slot 0), which
     in a planet–moon pair is the star that lights the neighbour.

     The magnitudes are computed as p (R ÷ Δ)² at full phase, from R07's albedos: moonlight on
     Earth is 2.5 × 10⁻⁶ of sunlight, Saturnshine on Titan 1.1 × 10⁻³ and Jupiter-shine on Io
     1.5 × 10⁻². A full Moon's sky (18–19 V mag arcsec⁻², 3–7 × 10⁻³ cd m⁻²; Krisciunas and
     Schaefer 1991, PASP 103, 1033) is within about two stops of the moonlit ground (about
     0.01 cd m⁻² at albedo 0.15). At Titan-class haze depths most of the ground's light is diffuse
     (Tomasko et al. 2005, Nature 438, 765). So transmittance alone is not enough.

     Not modelled, and recorded (Risks): the band depletion of the neighbour's reflected spectrum;
     the source's disc in the sky's phase function (under 0.8% for Rayleigh at ρ = 9.8°, 3% at
     20°); a partly set source's sky, lit or shadowed by its centre alone; and the source's
     illuminance in the march taken at the body's centre (±0.9% across an Io-class moon).

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
       recorded beside it) triggers the fallback of the Risks. The ceiling stays the laptop's
       figure; the desktop fails only if over it (decided 2026-09-30, Risks).
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
        and a scalar tracer differs by up to 12% on them (R08.T12.a measured −11.8% to +10.7% at
        τ 0.5, μ₀ 0.2);
      - Kokhanovsky et al. 2010 (JQSRT 111, 1931) and IPRT Phase A (Emde et al. 2015, JQSRT 164,
        8), also through the Stokes mode, with each case's tabulated scattering matrix (a₁–a₄, b₁,
        b₂; R08.T12.c). Garcia and Siewert's are scalar problems and run in the scalar mode;
      - and for spherical shells Loughman et al. 2004 (JGR 109, D06303), at its 2–4% model spread
        rather than 3σ.

      The 1960 Coulson–Dave–Sekera tables are not used, being wrong in the fourth decimal.

    - **The metric** (decision-backlog-1, 2026-10-09). For every case, every geometry g of a fixed
      set and every channel c:

      |L_client(g,c) − L_ref(g,c)| ≤ T(g,c) + 4σ_ref(g,c), where
      T(g,c) = max(0.05·L_ref(g,c), F·L_max(I(g),c)) and F = 3 × 10⁻⁴.

      - L_ref is the radiance I of the tracer's Stokes mode, and σ_ref its standard error. L_client
        includes the polarisation correction (Polarisation, below), from R08.T14.f on.
      - **I(g) is g's image:** the case's geometries that share g's observer and its suns.
        - On the ground: one sun zenith, with every view direction and azimuth.
        - In orbit: the limb heights at one sun, and each disc phase alone.
        - Aerial perspective: its paths at one sun.

        L_max(I,c) is the image's brightest diffuse radiance in channel c. The floor thus follows
        the exposure the image is seen at, and a twilight image is held to 5% of its own light, not
        excused by the noon image's.

      - **The floor takes over below 20F** (6 × 10⁻³ of the image's brightest), where the tolerance
        is F·L_max.
        - Through R07's AgX (`toneCurve`) at the AVG meter's exposure, which puts the image's mean at
          0.104 (`exposure.ts`), 5% is at most 0.96 ΔL* anywhere on the curve: about one
          just-noticeable difference.
        - An error at the floor stays under the same 0.96 ΔL* in an image whose brightest diffuse
          radiance is up to 30 times its mean (4.9 stops, AgX's shoulder). At 10⁻³ it would reach
          3 ΔL* there.
        - The comparison is in radiance, not in the tone-mapped image, so that it holds at every
          exposure the operator may set.
      - **σ_ref(g,c) ≤ T(g,c) ÷ 8 at every geometry.** The reference's noise then widens the gate by
        half at most, and `gate.ts` refuses a reference that is noisier. A client exactly 5% off
        fails one comparison with probability 3 × 10⁻⁵, about 0.1 false failure over ten cases'
        360 comparisons each.
      - **Named constants in `gate.ts`:** the 0.05, F, the 4 and the 8, so that a later ruling
        changes one line.
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
      - **Recorded beside each case, never gated:**
        - the u′v′ difference;
        - per image, the largest ΔE*ab between the two pipelines' colours through `toneCurve` at the
          AVG meter's exposure, the image's mean diffuse radiance taken as the metered average;
        - IPRT's relative RMS difference over the image, (Σ(L_client − L_ref)²)^½ ÷ (Σ L_ref²)^½
          (Emde et al. 2015), for comparison with the intercomparisons' figures of about 1%;
        - each image's L_max ÷ mean, so that an aureole brighter than the floor's bound of 30 is seen.

    - **The gate** is an ordinary vitest that runs the CPU twin on each committed case. So the
      brainstorm's "every baked atmosphere table matches a path-traced reference to 5%" is
      automatic, as its Testing section files it.
    - **Polarisation** (ruled 2026-10-09, `decision-r08-vector.md`; Mishchenko, Lacis and Travis
      1994, JQSRT 51, 491; Lacis et al. 1998, GRL 25, 135; Kotchenova et al. 2006, Appl. Opt. 45,
      6762).
      - **The error.** The eye sees I, the first Stokes parameter of the vector equation. A scalar
        solve differs from it in orders two and up; single scattering of unpolarised light is exact.
        Measured for the ruling (plane-parallel, checked against R08.T12.a's Natraj run):
        - pure Rayleigh: −3.3% to +3.6% at τ 0.06, −4.5% to +5.2% at τ 0.1, −8.0% to +8.5% at
          τ 0.23, and −18.7% to +11.9% at τ 0.5–1, with ±9–12% even with the sun overhead;
        - Earth's channels with an aerosol at sun zenith 78°: ±1.7% red, ±2.7% green and −5.2% to
          +4.6% blue;
        - fluxes: at most 0.5% to τ 1 and 1.1% at τ 4.
      - **The correction.** ΔJ_pol(h, μ₀, μ_v, m) is J_vector − J_scalar (its I), both from one
        8-stream discrete-ordinates solve of the world's own medium, m = 0..2, at the render
        channels, per latitude band (R08.T14.e). Every kernel adds it to its multiple-scattering
        term, in both regimes (R08.T14.f).
        - Measured: an 8-stream difference added to a converged scalar solve leaves ≤ 0.12% of I
          at τ 0.23–2, and ≤ 0.03% with an aerosol of g 0.6.
        - Modes: m ≤ 1 leaves 2.3–9.5%, and m ≤ 2 under 0.004%.
        - Refused:
          - a factor in Hillaire's view-independent multiple-scattering table;
          - an offline family;
          - two orders of scattering (Natraj and Spurr 2007, JQSRT 107, 263), which leaves up to
            12.6% at τ 1;
          - a 16-stream vector bake, about 27× the scalar solve.
      - **Matrices.** Every term carries its scattering matrix or is a total depolariser (a₁ alone),
        marked in the medium and the case:
        - Rayleigh by Hansen and Travis 1974's eqs. (2.15)–(2.16);
        - spheres from T5's Mie amplitudes;
        - Cornette–Shanks, literature phase functions without a published matrix,
          Henyey–Greenstein fallbacks, and aggregates unless T5.c's matrix agrees with optool's
          are depolarisers, a stated approximation.
      - **The gates.**
        - From R08.T14.f on, every gate compares the client, with the correction, against the
          vector I.
        - The same run's scalar I is compared with the client's uncorrected twin as a recorded
          diagnostic.
        - R08.T13's drift gate and sweep, and `THICK_MS_BOUNDARY`, stay scalar against scalar,
          since they measure Hillaire's scalar term.
      - **What a viewer sees** (R07's AUTO meter and AgX): the uncorrected sky's worst geometry is
        ΔE₀₀ 0.76 at Earth and 1.2–1.5 on a three-bar world. The correction is for physical
        accuracy: the corrected sky is inside the 5% target, and the uncorrected one is not.
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
      tables, not the per-view ones. Earth and every body under `ONE_SLICE_BELOW` and
      `ONE_BAND_BELOW` keep the 0.40 MB above. Transmittance and its absorber alpha are per κ
      slice, 131 KB each. Multiple scattering, irradiance, the thick J_ms table and the deck tables
      are per band.
      - A thin Saturn-class world (5 slices, 4 bands) comes to about 0.72 MB: 655 KB of
        transmittance and 64 KB of multiple scattering and irradiance. A thin Jupiter-class world
        (4 slices, 3 bands) comes to about 0.57 MB.
      - An ice giant (2 slices, 2 bands) comes to about 0.29 MB thin and 0.82 MB with two J_ms
        bands.
      - At Saturn's flattening a thick world comes to 1.77 MB with four J_ms bands (1.05 MB),
        before a second absorber table (131 KB a slice). The generator's worst case is flatter.
        Plan 14 caps f at 0.2, and its spin law puts there 9–31% of unlocked giants (P14.T46.c;
        science-r08-oblate). At the cap, Design note 17's ω_fig gives 9–10 slices and 6–7 bands.
        - Thin, that is 1.3–1.4 MB.
        - With a J_ms band per band it is 2.8–3.3 MB.
        - After both widenings below (5–6 slices, 5 bands) it is still about 2.05–2.2 MB, if a
          deck band costs a thick band's 278 KB.

        R08.T11 records it. If it stays over 2 MB, R08.T11 reports to "main", and a decision agent
        rules between a further widening and fewer bands at the cap.

      - A world whose per-planet bytes would pass 2 MB widens `KAPPA_STEP` to 0.3 (grazing
        interpolation error about 0.3%) and `BAND_STEP` to 0.15, in that order, and R08.T11
        records the step taken.
    - **The polarisation correction** (Design note 10) adds one table a latitude band: 32
      altitudes × 32 μ₀ × 16 μ_v × 3 modes × RGB, 393 KB in `rgba16float`, or 197 KB at 16 μ₀ if
      the gate allows. It is counted with the per-planet tables, so a thin one-band world comes to
      about 0.79 MB. Where the sum would pass 2 MB, the correction's μ₀ axis halves first, then the
      widening above applies. Its per-frame cost is three reads per sample per source in the
      sky-view, aerial-perspective and march kernels, recorded by R08.T14.f.
    - The table sizes are R05's `TABLE_SIZES`, taken from Hillaire's code, which the research
      agent's reading of his Table 2 confirms in kind: 32² multiple scattering, 32³ aerial
      perspective over 32 km, and a sky-view of about 200 × 100. R05's low sizes and its deferred,
      terrain-only aerial perspective are this plan's low setting.
    - This plan adds Design note 7's source count: the body's stars (`SKY_SUN_CAP`) and its
      planetshine sources. These bound the per-frame cost of the source loop, at most 4 sources on
      high and 3 on low, against `MAX_SKY_SOURCES` = 6 in the kernels. It also adds the thick
      table's size, and keeps both settings built together from R08.T6 on.
    - The thick bakes cost CPU at arrival, not frame time, so they run on both settings.
12. **What the view says.** The atmosphere is computed physics, not decoration. Six states are
    labelled. The first three follow the guide's existing grammar for a withheld or unmodelled
    section; the last three are new annunciations about the view's own drawing, as item 7 of "What
    the guide must gain" frames them:
    - `ATMOSPHERE: NOT RESOLVED` when the surface section is withheld, or a gas envelope's
      `envelope` section (decision-r08-giant-label). No atmosphere is drawn then, since drawing
      Earth's instead would be invention.
    - `ATMOSPHERE: NOT YET MODELLED` while the surface section is `not_modelled`, which is every
      generated body until P14.T24.a's figures are on the wire (R08.T1's P14.T35.e). No
      atmosphere is drawn. A body whose atmosphere is a gas envelope takes it from its envelope
      instead: a giant or a sub-Neptune, whose surface section is `not_applicable`, while its
      `envelope` section (P14.T24.d, on the wire by P14.T35.e) is `not_modelled` or absent
      (decision-p14-t35e-wire; a sub-Neptune's surface from P14.T48.e). A kept scene's body with no
      atmosphere set shows it too (decision-r08-giant-label).
    - `AEROSOLS: NOT YET MODELLED` while plan 14 publishes no aerosol or absorber inventory. It
      covers the absorbers too: ozone and methane are drawn only from the inventory.
    - `ATMOSPHERE: PENDING` from a body's `body_detail` request until its reply is drawn. No
      atmosphere is drawn meanwhile.
    - `ATMOSPHERE: COMPUTING` while a gated thick bake runs.
    - `ATMOSPHERE: APPROXIMATE` while a thick world is drawn with the analytic term because its
      regime's gate has not passed (Design note 9). It is also shown while a body's medium holds an
      ammonium hydrosulphide (NH₄SH) mode with less than `CLOUD_DECK_SPLIT_OPTICAL_DEPTH` of
      optical depth above it at 550 nm: no visible optical constants for NH₄SH are published, and
      its index is a stated stand-in (R08.T5.b; `decision-r08-licences.md` row 5). That note clears
      only when measured constants replace the stand-in, not when a gate passes.

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
        normal gravity of the level spheroid from (GM, a, c, ω_fig), ω_fig being the spin under
        which plan 14's figure is level ("Figures not flattened by the spin alone", below), and
        g_ref = √(g_e g_p) is the gravity the column (R08.T3.a) is built at. Optical depth read
        from a table is divided by s.
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
      - _Latitude bands._ Multiple scattering depends non-linearly on the column. So the
        multiple-scattering and irradiance tables, Design note 9's thick bakes, R08.T15's deck
        tables and `DiscReflectanceTable` are built per band of s.
        - Each band is an ordinary per-planet build at that band's g(φ) and √(MN). Bands are
          spaced Δln s ≤ 0.1 and read by linear interpolation in ln s at the sample's or disc
          pixel's latitude.
        - The count is 1 + ⌈ln(s_max ÷ s_min) ÷ 0.1⌉ once that span exceeds 0.02
          (`ONE_BAND_BELOW`), and 1 below it, at s = 1. That gives 1 band for Earth, 2 for the ice
          giants, 3 for Jupiter and 4 for Saturn.
        - The threshold is set by the error budget (ruled 2026-10-09, science-r08-oblate). One
          band is wrong in the column by ±½ ln(s_max ÷ s_min) at the pole and the equator. The
          threshold holds that to ±1%, within the 1% of Design note 10's 5% kept for geometry.
        - One band would leave the ice giants' column ±2.4–2.7% off. Two leave under 0.15%.
        - The threshold does not widen with `BAND_STEP`.
        - The bake count multiplies by the band count against `BAKE_CEILING_S`. The ice giants'
          second bake is 0.1–1 s by R08.T14's operation count.
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
    - **Figures not flattened by the spin alone** (researched 2026-10-09, science-r08-oblate; a
      physics ruling). Somigliana's γ is exact on a level spheroid for the spin it is given, at any
      flattening and for any interior (Stokes's theorem). So it needs the spin under which the drawn
      figure is level. Plan 14's figure laws (P14.T46.c, the wire's `FigureLawDto`) give it.
      - **`rotational`, and every fixture:** the true ω.
      - **`rotational_and_tidal`:** ω_fig = √2.5 ω, and g(φ) = γ(φ; ω_fig) + ω²R, R the
        volumetric radius.
        - _Why √2.5._ On a synchronous body the primary's static tide, ω²r²P₂(cos ψ) with n = ω,
          averages over longitude to −½ω²r²P₂(cos θ). With the spin's −⅓ω²r²P₂(cos θ), that makes
          a zonal forcing 2.5 times the spin's, whatever the interior. This is the factor that
          flattens plan 14's spheroid (Dermott 1979; the hydrostatic J₂ ÷ C₂₂ = 10 ÷ 3).
        - _Why + ω²R._ The tide has no degree-0 term, which √2.5 ω adds as
          ⅔(ω_fig² − ω²)R = ω²R. Adding it back takes g_ref's error from about −m to under 0.01%
          at m = 0.005.
        - _What the true ω would cost._ It falls short of g's poleward rise by 15m ÷ 4
          (m = ω²R³ ÷ GM), ±15m ÷ 8 in s:
          - 0.3% on an Io or a TRAPPIST-1b;
          - 0.8% on an HD 209458b;
          - 1.2% on a WASP-39b;
          - 3.4% on an inflated hot Saturn;
          - 5.8% on a 0.28-day rocky planet.

          This is against first-order hydrostatic theory, g ÷ g₀ = 1 − 2Q₀ + (k_f − 4)Q₂.

        - _What ω_fig leaves._ About 0.1% at m = 0.02, and 0.3% at m = 0.03. An exact Roche
          figure confirms it (0.04% at m = 0.01; 0.018% with ω²R in s, R08.T3.d's test).
      - **`capped`:** ω_fig is the spin whose Darwin–Radau flattening at the record's C ÷ Ma² is the
        drawn f, ω_fig² = (GM ÷ a³) · f[1 + (25 ÷ 4)(1 − (3 ÷ 2) C ÷ Ma²)²] ÷ 2.5, with no added
        term.
        - The same inversion gives ω on a `rotational` figure and √2.5 ω on a tidal one.
        - The true ω has no level f = 0.2 spheroid below 1.27 break-up periods (γ_e ≤ 0). Plan
          14's spin law gives that to 0.8–7% of unlocked giants. Above that period, the true ω
          spreads s over a factor of 7, which asks for 19 slices and 21 bands.
        - With ω_fig the cap takes 9–10 slices and 6–7 bands.
        - Its equatorial gravity is overstated: the true spin's centrifugal term at the equator is
          (ω² − ω_fig²)a larger, ⅔(ω² − ω_fig²)R of it uniform. That is about 0.4 γ_e at 1.4
          break-up periods, and all of γ_e near the break-up floor, where q(a) reaches 1.25
          (computed, R08.T3.d's follow-up). The cap is plan 14's.
      - **`sphere`:** the true ω. A rigid sphere's g rises by m, not Somigliana's 5m ÷ 2. But no
        sphere keeps an atmosphere under plan 14's Jeans rule (under 300 km, λ < 25 at 5 T_eq).
      - **Not modelled, and stated: the sectoral tide.** Plan 14 draws the spheroid, not the
        4 : 1 : 3 triaxial figure (decision-p14-phase-j, 3).
        - The true level surface rises towards the primary by 0.6(a − c): about 0.2–2 scale heights
          on close-in planets, for example 0.8 H on an HD 209458b and 1.4 H on a TRAPPIST-1b under
          N₂. That rise is not drawn, and the atmosphere follows the drawn surface.
        - So is g's longitude variation not drawn: ±¾(4 − k_f)m about the zonal mean at the
          equator, where (4 − k_f) ÷ (1 + k_f) = (25 ÷ 4)(1 − (3 ÷ 2) C ÷ Ma²)². That is ±0.4% on
          an Io, ±1.2% on an HD 209458b and ±5% on an inflated hot Saturn.
        - A triaxial datum would be plan 14's to give, and every rendering plan's to read.
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
        spherical. No oblate-atmosphere renderer is known to have precedent here;
      - Dermott 1979 (Icarus 37, 575) and Murray and Dermott 1999, ch. 4, as plan 14 cites them (the
        4 : 1 : 3 synchronous figure);
      - Iess et al. 2010 (Science 327, 1367), the hydrostatic J₂ ÷ C₂₂ = 10 ÷ 3 tested on Titan;
      - Leconte, Lai and Chabrier 2011 (A&A 528, A41; doi:10.1051/0004-6361/201015811), close-in
        planets' tidal and rotational ellipsoids;
      - for the figure laws: plan 14's `figure.rs` (P14.T46.c) and `rotation.rs` (P14.T14).

      The figures are computed (Somigliana at each body's GM, a, c and period, Chapman to first
      order in H/R, and first-order hydrostatic theory with an exact Roche-model check for the
      tide), not measured. R08.T12.b's spheroid case measures them.

## Tasks

The order:

- T0 (R01's one-layer seam in compute) needs nothing and can start at once. T6.b, T6.f and T9.b
  need it.
- T1 and T2 are documents and can start at once. T1 must land in plan 14 before plan 14 builds
  P14.T24 (unbuilt). R09.T0.a also writes asks into plan 14, so the two are committed one after the
  other, not in parallel.
- T3.b (dispersion and King factors) and T3.d (normal gravity and the slicing) need nothing and can
  start at once. T3.a (the column) follows T3.d, at whose g_ref it is built and whose normal
  gravity its spheroid test reads. T3.c (the molecular term) follows T3.a and T3.b.
- T4.a follows T3, since its Earth cases need the molecular term, and builds `bless.ts`; T4.b
  follows T4.a.
- T5.a (Mie) needs nothing of T3 or T4 and can start at once; T5.b follows it, and T5.c follows
  T5.b.
- T6 generalises R05's tables over N terms. T6.a needs T3.a's `tabulated` density; T6.b follows
  T6.a and T0; T6.c follows T6.b and T4.b (the ozone curve of growth); T6.d follows T6.c, T3.c
  and T4.a (the fitted channels); T6.e follows T6.d and T3.d; T6.f follows T6.e.
- T7 (several suns, the sky's extinction), T8 (other bodies from outside) and T9.a (aerial
  perspective) follow T6.d; T9.b (surface lighting, by band) follows T6.f.
- T10.a assembles the medium from a body and needs T2–T6 (T5 for the aerosol terms); T10.b draws
  it in the photorealistic view and follows T10.a and T7. T11 records the thin benchmarks and needs
  T7–T10.
- T12.a (the tracer) needs nothing of this plan and can start at once. T12.c (its benchmarks) and
  T12.d (the spheroid mode) follow it. T12.b (the cases and references) needs T5, T10's fixtures,
  T12.c and T12.d.
- T13 needs T6 (T6.e for `saturn-oblate`) and T12. T14 follows T13, and T15 follows T14, whose
  solver it uses.
- T16 needs T8 and T15. It runs on the hand giant fixture, and re-runs on plan 14's envelope when
  P14.T24.d is on the wire. T17 closes.

Some tasks wait on the owner or on another plan, and say so where they do:

- **Licences** (ruled 2026-10-09, delegated; `decision-r08-licences.md`, after `decisions-r05.md`
  item 4). No raw table is committed except under an explicit permissive licence. The values R08
  uses are committed reduced, with their citations in `NOTICE`, and their sources are fetched by
  URL and SHA-256:
  - methane (R08.T4.b) from NASA PSG's conversion of Karkoschka and Tomasko 2010;
  - H₂SO₄, Mars dust and tholin (R08.T5.b) from ARIA's, NASA Ames's and HITRAN's copies of Palmer
    and Williams 1975, Wolff et al. 2009 and Khare et al. 1984.

  So the Venus-class, Mars and Titan-class fixtures of R08.T10, their cases and references in
  R08.T12.b, and their gates in R08.T13–T15 wait only on R08.T5.b's files. NH₄SH has no visible
  optical constants: R08.T5.b gives it a stated stand-in, a body that shows it is labelled
  `ATMOSPHERE: APPROXIMATE` (Design note 12), and no NH₄SH case is committed or gated. The
  benchmark tables of R08.T12.c and R08.T14.a are committed only as the values each test asserts.
  Serdyuchenko's ozone was ruled on 2026-10-02 (`decisions-r05.md` item 4): the reduced 1 nm
  table may be committed with its citation, and the raw table may not.

- **Star spectra.** Every spectral bake and fit reads the star's `bake_spectrum` from R06's
  `HostDiscDto` (R06.T3.c and T10). The channel fit's non-solar suns are R06's colour-table rows at
  those temperatures.
- **Galaxy plan 14.** No generated body has an atmosphere on the wire until P14.T24.a–b and R08.T1's
  P14.T24.c–f and P14.T35.e are built (none is). Every task here runs on the fixtures of Design
  note 16 meanwhile, and generated bodies show `ATMOSPHERE: NOT YET MODELLED`. In plan 14's order
  a thin atmosphere's gases, pressure and temperature arrive first (P14.T24.f, T24.a–b, T48.e and
  T35.e), drawn on the isothermal seam with `AEROSOLS: NOT YET MODELLED`; the vertical structure
  (T24.e), the inventory (T24.c) and the gas envelopes (T24.d) follow.
- **Drafts for the owner.** R08.T1's amendments and R08.T2's labels are committed marked drafted
  for the owner, and the client is built to them meanwhile; the acceptance and the sign-off are
  the owner's, recorded here when given.
- **The UHD 620.** Its runs (R08.T7, T11, T14.d's bake time, T17) are the owner's, on the owner's
  laptop; the development machine's figures are recorded beside them.

TypeScript paths are under `apps/hyperion/src/renderer/src/view/atmosphere/` unless a path says
otherwise; a path that starts `view/`, `lib/`, `displays/`, `smoke/` or `test/` is under
`apps/hyperion/src/renderer/src/`.

- Tests that need no GPU run under `pnpm test`, and so in `just ci`.
- Every task that adds or changes a catalogued kernel also passes `just test-render` (R01.T9.e).
- GPU checks are by hand and recorded here, as the brainstorm's Testing section says.
- Every physical constant carries its source, re-checked against that source when it is written
  (the galaxy README's Figures rule).
- Every timing in this plan's design notes was taken under shared load (load average about 14 on
  eight threads) and is provisional. Each task that records a timing re-measures it on a quiet
  machine, with the load average stated.

### R08.T0 One-layer arrays in compute (R01's engine seam)

A compute binding is viewed at the view dimension its kernel declares, under the rule a material's
binding already follows (`decision-r08-design.md` item 1).

As built, `resolveKernelResources` (`view/engine/webgpu/kernelResources.ts`) views every sampled
and storage texture at the texture's own dimension (`viewDimensionOf`, `resources.ts`): `2d` for
one layer, `2d-array` for more. A compute pipeline's layout is `auto`, built from the WGSL
declarations, so a kernel declaring `texture_2d_array` or `texture_storage_2d_array` cannot bind a
one-layer texture. The bind group fails WebGPU's validation (§8.2.1: the view's dimension must
equal the layout entry's), and the submission that holds it is refused. This task:

- `compute.ts`: `KernelBinding` gains `viewDimension: GPUTextureViewDimension | undefined`.
  `kernelBindings` reads it from the declaration's type, by WebGPU's table (§6.2.1):
  - `texture_2d`, `texture_depth_2d`, `texture_multisampled_2d` and `texture_storage_2d` → `2d`;
  - `texture_2d_array`, `texture_depth_2d_array` and `texture_storage_2d_array` → `2d-array`;
  - `texture_3d` and `texture_storage_3d` → `3d`;
  - `texture_cube` and `texture_depth_cube` → `cube`;
  - `texture_cube_array` and `texture_depth_cube_array` → `cube-array`;
  - `undefined` for buffers and samplers.
- `kernelResources.ts`: each sampled and storage texture is viewed at its binding's declared
  dimension once `viewDimensionBinds(declared, own)` holds. A storage binding also takes a cube as
  a six-layer `2d-array`, as today, since a storage view is never a cube (§8.1.1). Any other
  mismatch throws before any GPU call, naming the kernel, the binding, the declared dimension and
  the texture's.
- `viewDimensionBinds` moves from `drawing.ts` to `resources.ts`, beside `viewDimensionOf`, so that
  the material and compute paths share one rule.
- `ComputeBindings` and every caller are unchanged. Every existing kernel's declared dimension
  already equals its texture's, so nothing built changes. R07.T8.d's two-layer
  `bodies:no class map` may stay as it is.

Files: `view/engine/webgpu/{compute,kernelResources,resources,drawing}.ts` and their tests, a smoke
check beside R01's in `smoke/work.ts`, and a pointer in R01's Risks.

Tests (fakes):

- `kernelBindings` reports each texture binding's declared dimension, `texture_2d_array` and
  `texture_storage_2d_array` as `2d-array`;
- a one-layer 2D texture bound where `texture_2d_array` is declared is viewed
  `{ dimension: "2d-array" }`, sampled and as storage at a level;
- a three-layer texture where `texture_2d` is declared throws, naming the kernel and the binding;
- a cube's storage view stays a six-layer `2d-array`, and a 3D texture is viewed `3d`.

Smoke (`just test-render`, on `default` and `no-subgroups`): a kernel declaring a
`texture_2d_array<f32>` input and a `texture_storage_2d_array<rgba16float, write>` output copies
texels layer by layer, on a one-layer and a three-layer texture. The texels are values exact in
half precision, and they read back exactly.

Acceptance: `pnpm --filter hyperion exec vitest run view/engine/webgpu` and `just test-render`
pass.

### R08.T1 Asks of galaxy plan 14

Write five amendment tasks into [galaxy plan 14](../galaxy-generation/14-planetary-systems.md),
four beside P14.T24.a and one in P14.T35, as R04 amends it for the detail seed. The physics below
was researched on 2026-09-29 (Design notes 3, 6 and 16, and the sources cited there). The amendment
cites it, and any rule marked "from memory" is checked against its paper by the agent who builds
it. The letters follow the README's asks table. The build order, reconciled with R09.T0.a's asks
(plan 14's Phase K), is f first, then P14.T24.a–b with P14.T48.a–e, then T35.e, then e, c and d:
T24.c's rules read T24.e's profile and T24.f's gases, and T24.e's α reads P14.T48.d's condensable
(plan 14's Tasks, "Order and parallelism"). The wire amendment is P14.T35.e: P14.T35.d is taken
("Body-state times beyond 2⁵³ s", 2026-09-30).

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
- **P14.T35.e The atmosphere on the wire.** As built, the sim's `record::Surface` and the wire's
  `BodySurfaceDto` are uninhabited enums, and no plan-14 task gives them fields (P14.T35.b only
  tags the section `not_modelled`). So `BodySurfaceDto` gains, with the record's `Surface`, first
  P14.T24.a's and T24.b's figures that this plan reads: the surface state and material, the
  surface temperature and pressure, the gravity, the ordered gas fractions and the cloud fraction.
  Then the fields of T24.c, T24.e and T24.f: the inventory's modes and absorbers, the vertical
  structure's (T_s, p_s, β, T_skin), and the gas fractions with CH₄ and O₂. A giant gains an
  `envelope` section of its own on `BodyRecordDto`, holding T24.d's figures, since its `surface`
  stays `not_applicable`. Each field's name carries its SI unit; `just gen-protocol` follows. A
  change of the wire's form is coordinated through "main" (`PROTOCOL_VERSION` is 2). R09–R11 read
  the same section (the ocean, ice and cloud fractions, surface age and crater density; the
  roadmap's asks table), so the task is written to serve them too, with R09.T0.a's asks. Its test
  pins the wire form of an Earth, a Venus and a Jupiter, one test per section state.

Files: `docs/agent/plans/galaxy-generation/14-planetary-systems.md`, and the two P14.T35.d
mentions in the roadmap's asks tables (`README.md`), which this re-validation renamed to
P14.T35.e. Acceptance:

- `npx prettier --check docs/agent/plans/galaxy-generation/14-planetary-systems.md` passes;
- the four P14.T24 tasks appear under Phase E and P14.T35.e under Phase H, with their tests and
  sources;
- plan 14's Risks name this plan as their consumer, and record its Venus at 58 bar against the
  real 92.

This is a brainstorm-driven plan edit. It is committed marked drafted for the owner (the RM4/RM5
rule for galaxy-plan amendments), and plan 14's owner accepts it; the acceptance is recorded here.

_Done 2026-10-09, drafted for the owner and awaiting the sign-off: the five tasks are plan 14's
P14.T24.c–f under Phase E and P14.T35.e under Phase H, reconciled with R09.T0.a's Phase K. The
record, with what departs from the bullets above, is Risks' "The asks R08.T1 wrote into plan 14".
The prettier check ran as `pnpm exec prettier --check`, the same tool, since this machine has no
`npx`._

### R08.T2 The atmosphere labels, drafted for the owner

Draft the six nomenclature entries of Design note 12 in the form of R02's drafted items, each
with its meaning and when it clears:

- `ATMOSPHERE: NOT RESOLVED`;
- `ATMOSPHERE: NOT YET MODELLED`;
- `AEROSOLS: NOT YET MODELLED`, covering absorbers as well;
- `ATMOSPHERE: PENDING`, which clears by itself;
- `ATMOSPHERE: COMPUTING`;
- `ATMOSPHERE: APPROXIMATE`, which clears when its regime's gate passes, or, for an NH₄SH
  stand-in, when measured constants replace it (`decision-r08-licences.md` row 5).

None uses a status colour or the word "degraded" (item 7 of
[What the guide must gain](../../brainstorming/rendering-and-planets.md#what-the-guide-must-gain)).
The client is built to the draft. The task is committed with the draft marked for the owner, and
**the owner signs off** later; the sign-off is recorded here.

Files: the six entries as rows of the guide's nomenclature table in
`docs/frontend/ux-guidelines.md` (as `BODY PHOTOMETRY: NOT YET MODELLED`, a `Label`, and
`TERRAIN: STREAMING`, an `Annunciation`, are), each marked
`_Draft (plan R08, R08.T2): the owner signs off._`, and `labels.ts`, the strings and the
`AtmosphereLabel` keys. R08.T10.b feeds them to R02's `ViewLabelBlock` through
`photorealStatements` (`displays/view/viewRun.ts`), beside `litLabelsOf`. Tests: `labels.test.ts`
pins the strings. Acceptance:

- `pnpm --filter hyperion exec vitest run view/atmosphere/labels` passes;
- the console-ux lint passes (`python3 .claude/skills/console-ux/scripts/ux_lint.py` on the
  changed files);
- the draft markers are in the guide, and the sign-off is recorded here when given.

### R08.T3 The column and Rayleigh scattering per gas

- **R08.T3.a The column.** `column.ts`: `TemperatureProfile` with `isothermal` and
  `radiativeConvective`, `temperatureAt`, and `hydrostaticColumn` (Design note 3), with molar
  masses from plan 14's `Gas::molar_mass_g_per_mol` (IUPAC 2021), each naming its Rust source.
  The column is built at g_ref = √(g_e g_p) from R08.T3.d, not at the bulk section's single
  gravity, and its altitudes are gravity-scaled heights (Design note 17). R05's `DensityProfile`
  gains its `tabulated` variant here, in `medium.ts`, with `densityAt` and `columnLengthM`; the WGSL
  side (`common.wgsl`'s `densityOf`, which reads any kind above 0.5 as a tent) is R08.T6.b's. Tests:
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
  `KAPPA_STEP`, `BAND_STEP`, `ONE_SLICE_BELOW` and `ONE_BAND_BELOW` (Design note 17). Its inputs
  are R07's `BodyFigure` (`view/terrain/planet.ts`), GM from the record's `mass_kg` section times
  CODATA's G, and ω, the spin rate at the scene time of the record's rotation law
  (`BodySummaryDto.rotation`, read as `SystemBodyRotation`; R05 Design note 14's test-planet period
  for R05's Earth); with no rotation section, g(φ) is taken as the bulk section's gravity and one
  slice results. A figure not flattened by its spin alone takes Design note 17's ω_fig from the
  figure's law and C ÷ Ma², which `BodyGravityInput` carries (null for fixtures and R05's Earth,
  read as `rotational`); R08.T10.a fills them from the record's figure section
  (`SystemBodyFigure.law`, `.momentOfInertiaFactor`). As built, `lib/system/rotation.ts` keeps the
  law's rate private (`rateAtAge`), so this task exports the rate at a time beside
  `rotationAngleAt`, and G has one client copy, module-private in `lib/scene/sceneWire.ts`, which
  this task exports from one place (with the sim's citation) rather than writing a second literal.
  Files: `oblate.ts`, `oblate.test.ts`, `lib/system/rotation.ts` and the G constant's module.
  Tests:
  - WGS 84's figure, GM and ω give NIMA TR8350.2's γ_e = 9.7803253359 and γ_p = 9.8321849378
    m s⁻² to 10⁻⁹ relative;
  - Saturn's (a = 60,268 km, c = 54,364 km, GM = 3.7931 × 10¹⁶ m³ s⁻², 10.656 h) give 9.08 and
    12.04 m s⁻², and Jupiter's 23.12 and 26.98, to 0.5%;
  - R_α equals M at α = 0 and N at 90°, lies between them at every α, and is a²/c for every α at
    the pole;
  - the slice and band counts are 1 and 1 for Earth, 2 and 2 for Uranus, 4 and 3 for Jupiter and 5
    and 4 for Saturn; a sphere with ω = 0 gives one of each with s ≡ 1; a body whose
    ln(s_max ÷ s_min) ≤ `ONE_BAND_BELOW` takes one band, with |ln s| ≤ 0.01 at every latitude;
  - by figure law (Design note 17, "Figures not flattened by the spin alone"):
    - `rotational` and a body with no law take the true ω (WGS 84 unchanged to 10⁻⁹);
    - the inversion returns ω on a `rotational` figure and √2.5 ω on a `rotational_and_tidal` one,
      each built by plan 14's iteration, to 10⁻⁹;
    - on an HD 209458b-like synchronous figure (C ÷ Ma² = 0.25, m = 4.4 × 10⁻³), s matches
      first-order hydrostatic theory's zonal mean to 10⁻⁴ and g_ref to 10⁻⁴; the true ω misses s
      by at least 0.8%. The theory is
      g ÷ g₀ = 1 − ⅔m + (k_f − 4) m(¾ cos²φ − ½ sin²φ − ⅓), with
      k_f = (4 − η²) ÷ (1 + η²) and η = (5 ÷ 2)(1 − (3 ÷ 2) C ÷ Ma²);
    - a `capped` Saturn-density giant at 1.1 break-up periods, where the true ω gives γ_e ≤ 0,
      builds without a `RangeError`, with s rising poleward, at most 10 slices and at most 7 bands.

Acceptance: `pnpm --filter hyperion exec vitest run view/atmosphere/column view/atmosphere/rayleigh view/atmosphere/oblate`.

### R08.T4 Channels and absorbers

- **R08.T4.a The fitted triple.** First `bless.ts`, the client's bless helper: under
  `HYPERION_BLESS=1`, the variable the Rust testkit already reads
  (`crates/hyperion-testkit/src/golden.rs`, which also refuses a bless under `CI`), a vitest
  rewrites its committed table. Otherwise it compares, and on a mismatch fails naming the command,
  `HYPERION_BLESS=1 pnpm --filter hyperion exec vitest run <file>`. As built, renderer tests read
  no files through `node:fs` and have no Node types (Provides), so the helper reads the table as a
  JSON import and rewrites it through vitest's `toMatchFileSnapshot`, `vitest.config.mts` turning
  the update mode on under `HYPERION_BLESS=1` (and never under `CI`). `BAKE_WAVELENGTHS_NM` (15
  over 380–760 nm) is written here, tested against R06's
  `packages/protocol/fixtures/bake_wavelengths_nm.json`. A bless-style vitest then fits the
  channel triple by minimax Δu′v′ over a stated case family: Earth at several suns (3,200 K, solar
  and 9,000 K), and Mars with dust. The two non-solar spectra are R06's colour-table rows at those
  temperatures, read through the table's `bake_spectrum` (R06.T3.c). It writes the triple and the
  objective's value into `channels.json`, and is checked unchanged otherwise. R05's
  `CHANNEL_WAVELENGTHS_NM` stays (680, 550, 440), the wavelengths of R05's Earth constants, until
  R08.T6.d rebuilds Earth at the fitted triple. Mars's dust case reads R08.T5.b's dust file
  (licence ruled 2026-10-09, `decision-r08-licences.md` row 3). Until that file lands, the family
  is Earth's alone, recorded as such.
  It uses CIE 1931 2° colour-matching functions reduced to linear Rec. 709, the primaries R06 uses.
  The CIE table (CC BY-SA 4.0) is fetched with its checksum and not committed; a Node tool in
  `apps/hyperion/src/tools/`, run by a script under `apps/hyperion/scripts/` (R05's
  `solarFactors` precedent), reduces it to the committed values the fit reads, with the CIE's
  required citation in `NOTICE`'s Data section (R05.T12.d's ruling, `decisions-r05.md` item 4).
  Files: `bless.ts`, `medium.ts` (`BAKE_WAVELENGTHS_NM`), `channels.ts`, `channels.json`,
  `channels.test.ts`, `apps/hyperion/vitest.config.mts`, the tool and its script, `NOTICE`.
  Tests:
  - the refit starts from (620, 540, 445) and does not worsen the recorded objective;
  - at Earth the fitted triple beats 680/550/440 at a sun zenith of 85°.
- **R08.T4.b Curves of growth.** `absorbers.ts`: `absorberCurve(absorber, spectrum)` computes T_c(u)
  per absorber and channel for one sun, weighted by max(r̄_c, 0)·S (Design note 5), on σ's own grid
  of 1 nm or finer, with S interpolated from the sun's 15 `bake_spectrum` bin averages. It runs in
  the optics worker at arrival, once per sun. The reduced cross-sections, ozone binned to 1 nm and
  methane's coefficients as published, are written to `absorbers/crossSections.json` by the same
  reduction step (T4.a's Node tool, since a renderer test fetches and reads no raw file). Ozone's
  1 nm bins are centred, on [λ − 0.5, λ + 0.5) nm (`decisions-r05.md` item 2), unlike R05's
  three upward 10 nm bins, which R05 keeps for parity with Bruneton and sebh.
  - Sources: ozone from Serdyuchenko et al. 2014 (AMT 7, 625; the articles CC BY 3.0, the data
    page's terms unstated). Methane from Karkoschka and Tomasko 2010 (Icarus 205, 674), as NASA's
    Planetary Spectrum Generator converts them to cross-sections at 100, 198 and 296 K:
    <https://psg.gsfc.nasa.gov/data/linelists/xuv/data/ch4.txt>, SHA-256
    `cf7f7195a9ceadad1480b436d1657b722d1bb6264a603618faead3dd8aa4a3ef`, fetched 2026-10-09, 4,949
    points to 0.836 µm, with no terms stated. Elsevier's supplementary Table 4 is not an input: its
    table is images, and Elsevier reserves text and data mining on it.
  - Only the reduced values are committed, with attribution in `NOTICE`'s Data section. The raw
    files are fetched with their checksums and never committed.
    - Ozone: the reduced 1 nm table (ruled 2026-10-02, `decisions-r05.md` item 4).
    - Methane (ruled 2026-10-09, `decision-r08-licences.md` row 1): σ at the three temperatures
      over 380–800 nm, interpolated onto the tool's own uniform grid of 0.25 nm or finer, so that
      no band-model value is averaged.
    - T4.b's science check confirms from the paper:
      - that PSG's columns are Table 4's infinite-pressure coefficients;
      - the finite-pressure correction (Eqs. 2–4, the caption's constant 150 K^½) and Eq. 8's
        temperature law, or records their omission;
      - PSG's wavelength convention;
      - where its MPI-Mainz ultraviolet completion begins. Any value used from it also cites
        Keller-Rudek et al. 2013.
    - Karkoschka 1998 (PDS GBAT_0001, DOI 10.17189/2bp8-k793, CC0) is the open fallback and a
      cross-check. It has no temperature dependence, so using it is a recorded deviation.

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
  - Ruled 2026-10-09 (`decision-r08-licences.md` rows 2–5). Each file is reduced by T4.a's Node
    tool from a source fetched by URL and SHA-256 and never committed. Its header carries:
    - its paper, its source, the checksum and the date fetched;
    - "reduced values; the source states no terms".

    The citation goes in `NOTICE`'s Data section. The files:
    - **H₂SO₄**, 75 and 84.5 wt% at 300 K, from Palmer and Williams 1975 (Appl. Opt. 14, 208) as
      ARIA distributes them:
      - `https://eodg.atm.ox.ac.uk/ARIA/data_files/Acids/Sulphuric/70%25_to_79%25/Sulphuric_acid_75%25_300K_(Palmer_and_Williams_1975)/original/H2SO4_75%25_300K_R_Palmer_1975.ri`
        (SHA-256 `ca84ce9a373bf9d0356dbbeeb59d61f771bf9d14667cecb3206f8a3cdac954fe`);
      - the `80%25_to_100%25/…84.5%25…` file
        (`6ae3eceb2c3895ec367f9b86e8e082445f1473413d224c2b625d85baaf452dc3`).

      In the visible they give n only, at 359.7, 408.2, 449.4, 555.6 and 701.8 nm, with k `NaN`.
      The file states how k is set there, and checks n against Hansen and Hovenier 1974's
      1.44 ± 0.015 at 550 nm.

    - **Mars dust** from Wolff et al. 2009 (JGR 114, E00D04): NASA Ames's
      `Dust_Refractive_Indicies.txt`, as kept at
      `https://raw.githubusercontent.com/sukritranjan/ranjanwordsworthsasselov2017b/1e67a4819255ff907df2a97eb8cecd9eee32c928/Raw_Data/ComplexRefractionIndices/Dust_Refractive_Indicies.txt`
      (SHA-256 `bf6069ad22fbfbbef7bf957d386a8177647c87fd98ed6574b5c5f8d6c8016769`). The
      repository's MIT licence covers its code, not these data. The dust's literature
      single-scattering albedo and phase function follow the same rule: values with citation, no
      figure or file copied.
    - **Tholin** from Khare et al. 1984 (Icarus 60, 127), as HITRAN2024's aerosol compilation
      distributes it, inside <https://hitran.org/data/Aerosols/Aerosols-2024/hitran_ri.tar>
      (207,422,976 bytes). Its SHA-256 is recorded at the first fetch. Only Khare's ASCII file is
      read, and none of the archive's code is run. He et al. 2022 (PSJ 3, 25; CC BY 4.0) is the open
      alternative, and T5.b's science check picks between them.
    - **NH₄SH** has no measured visible optical constants (Howett et al. 2007 cover 1,300–12,000
      cm⁻¹).
      - Its stand-in is a non-absorbing particle of real index 1.80, constant over 380–780 nm
        (Sromovsky et al. 2017, Icarus 291, 232, §IV.3, after Sato et al. 2013's fitted 1.85). It
        is named `standIn` in its file, unless T5.b's science check prefers NH₃ ice's index.
      - A body whose medium shows an NH₄SH mode carries `ATMOSPHERE: APPROXIMATE` (Design note 12).
      - No NH₄SH case is committed or gated.
    - **CO₂ ice**, a derived file rather than a reduction of a fetched data file: no data file
      exists, and refractiveindex.info, NASA's OCdb and ARIA hold none over 0.3–1 µm
      (`science-r08-sulphur-co2ice.md`, 2026-10-09).
      - The real index is n(λ) = 1.3994 + 0.004312 µm² ÷ λ², a two-term Cauchy fit to Warren 1986's
        Table I over 0.30–1.10 µm (Appl. Opt. 25, 2650, pp. 2663–2667).
        - Warren's index comes from Kramers–Kronig, "accurate to ±0.05", and the fit departs from
          it by under 0.001.
        - The fit was made by the science agent from the author's copy:
          `https://atmos.uw.edu/~sgw/PAPERS/1986_CO2ice_mcx.pdf`, SHA-256
          `55873a3bcb4820867112c1243b4f583fa311f995d9a56f3db26f635664a7b485`, fetched 2026-10-09.
      - The imaginary index is k = αλ ÷ 4π with α = 10⁻² m⁻¹, Hansen 2005's estimated visible
        absorption (JGR 110, E11003), below that paper's detection limit over 0.25–1.0 µm.
        Warren's Egan–Spagnolo k, about 10³ larger, is the stated upper bound.
      - The header carries both papers, the fit and its residual. It reads: "derived values: Optica
        holds Warren's article under its pre-2017 agreement (reuse by permission only, text and
        data mining reserved; its copyright page, 2026-10-09), and AGU's terms for Hansen 2005
        could not be read; the data are the authors' (decision-r08-licences.md)". Its citation goes
        in `NOTICE`'s Data section.
      - No table of Warren's is committed.
      - Its shape class is crystal (T24.c), so Mie supplies cross-sections only.

  - Until the H₂SO₄ file lands, the Venus mode-2 test below takes the single index n = 1.44 at
    550 nm that Hansen and Hovenier 1974 publish.
  - The non-spherical materials' literature phase functions sit beside their indices.

  Tests:
  - the distribution reproduces r_eff and v_eff to 10⁻⁴;
  - every file covers 380–780 nm;
  - a narrow distribution equals the single sphere;
  - a mode's table carries its scattering matrix (a₁ = a₂ and a₃ = a₄ for spheres, b₁ and b₂,
    from S₁ and S₂ in T5.a's convention). A sphere of x = 10⁻³ reproduces Hansen and Travis
    1974's Rayleigh matrix (eq. 2.15, δ = 0) to 10⁻⁶. The single-scattering degree of
    polarisation is −b₁ ÷ a₁;
  - Venus's mode-2 droplets (r_eff 1.05 µm, v_eff 0.07, n ≈ 1.44 at 550 nm; Hansen and
    Hovenier 1974) give g within that paper's figure;
  - an NH₄SH mode with less than `CLOUD_DECK_SPLIT_OPTICAL_DEPTH` above it gives
    `atmosphereApproximate`, and one beneath a τ 30 deck does not;
  - CO₂ ice's real index is 1.413 ± 0.001 at 553 nm and 1.404 ± 0.001 at 1,000 nm (Warren 1986,
    Table I, asserted values);
  - its k lies between 0 and 2.2 × 10⁻⁶ over 380–780 nm;
  - a 2 µm sphere's single-scattering albedo at 550 nm exceeds 0.9999.

- **R08.T5.c Aggregates and phase tables.** `aggregate.ts` implements Tazaki and Tanaka 2018's MMF
  with D_f ≤ 2.5, and the phase-shift gate of Design note 6: a mode with Δφ ≥ 1 keeps its
  opacities and takes a Henyey–Greenstein phase from its asymmetry. Beside it go the √θ phase
  tables per channel, with the matrix elements where the model gives them, and the delta-M
  truncated series with its fraction, for the bakes only (Design note 6). An aggregate's matrix is
  kept only if it agrees with optool's elements to their stated tolerance; otherwise `matrix` is
  `undefined`, a total depolariser, flagged in the code (Design note 10). Tests:
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

- **R08.T6.a The CPU twin over N terms.** `tablesCpu.ts`, built on R05's `opticalDepth.ts` and
  `marchSteps.ts`, gives each of R05's kernels in `f64` at R05's sizes over a list of terms, with
  tabulated density and phase (`PhaseFunction`'s `tabulated` variant is added here, in
  `medium.ts`, if R08.T5.c has not yet added it). Tests (no GPU):
  - R05's constants as a medium give R05's oracle values to 10⁻⁶;
  - the sky view's and the march's twins place their steps as R05.T12.e does, and pass its
    quadrature gate (`marchSteps.test.ts`'s 1,046 rays, metric and tolerances, `LOW_TWIN_WORST`
    on low), with the multiple-scattering term included;
    the multiple-scattering kernel's even steps are measured against a placed reference and
    recorded;
  - the multiple-scattering kernel takes R05.T12.e's stable step factor from `common.wgsl`, and its
    twin records the change;
  - two identical half-density terms equal one;
  - optical depth adds across terms.

  Acceptance: `pnpm --filter hyperion exec vitest run view/atmosphere/tablesCpu`.

- **R08.T6.b The kernels over N terms.** Terms travel in a storage buffer in place of R05's
  `Medium` uniform (`MAX_TERMS` = 8, 416 B, packed by `packMedium`), and density and phase tables
  in 2D array textures, one layer a term. The `tabulated` density and phase get their own codes in
  WGSL, whose `densityOf` today reads any profile kind above 0.5 as a tent. The `thin`
  multiple-scattering table keeps Hillaire's isotropic 32². The kernels read the tables by
  `textureLoad` with hand filtering (a compute pass binds no sampler), and a one-layer array binds
  through R08.T0's seam. A medium with no tabulated phase term binds a one-layer, one-texel
  placeholder, which the kernels never read. Every changed kernel stays registered in
  `WGSL_CATALOGUE`. Files: R05's
  `shaders/{transmittance,multiScattering,skyView,aerialPerspective,rayMarch,composite}.wgsl` and
  the libraries `shaders/{common,medium,view,source}.wgsl` they are assembled from, `hillaire.ts`,
  `tables.ts`, `medium.ts`, `tables.test.ts` (which pins `packMedium`'s length and its "at most 8"
  refusal) and `hillaire.test.ts` (which checks that no assembled module takes a `Medium` by value),
  and the smoke page's checks (`smoke/atmosphere.ts`). Smoke (`just test-render`): every kernel
  compiles, and the readbacks agree with the twin to 10⁻³ relative above 10⁻⁶ of the table's
  maximum, on both variants (`default` and `no-subgroups`). Acceptance: `pnpm test` and
  `just test-render` pass.

- **R08.T6.c Optical depth and curves of growth.** Transmittance stores optical depth in RGB and
  the accumulated absorber column in alpha, with a second table for further absorbers. Each sun's
  −ln T_c(u) is added on read (Design notes 5 and 8), in the twin and the kernels: at R05's four
  read sites, `source.wgsl`'s `tableTransmittance`, `multiScattering.wgsl`'s `transmittanceToSun`,
  `composite.wgsl`'s `transmittanceToSpace` and `rayMarch.wgsl`'s ground hit. R05.T12.b's smoke
  assertion "every transmittance texel is finite and within [0, 1]" (`checkAtmosphereTables`) is
  rewritten as: every stored optical depth is finite and non-negative; its 20 oracle texels are
  compared in optical depth, not as e^(−τ) under the half-float floor. Tests: a Venus-class
  transmittance is finite with no underflow, and R05's Earth, with its ozone as an absorber curve
  reduced to the linear regime, reproduces R05's transmittance to 10⁻³. That regression feeds the
  curve R05's own three ozone coefficients (`OZONE_ABSORPTION_PER_M`, Bruneton's and sebh's upward
  10 nm bins), not T4.b's centred 1 nm bins, which differ by +10%, −6% and −19% at the three
  channels (`decisions-r05.md` item 2). Acceptance: `pnpm test` and `just test-render` pass.

- **R08.T6.d Earth rebuilt.** `EARTH_REFERENCE` (`earth.ts`) is rebuilt from R08.T3's column and
  molecular term (tabulated, about 8.4 km in scale height, in place of R05's exponential at the US
  Standard Atmosphere's 8,434.5 m; `HILLAIRE_REFERENCE` keeps sebh's 8 km), R05's aerosol, the
  ozone curve of growth (R08.T4.b) and the fitted channels (R08.T4.a), to which
  `CHANNEL_WAVELENGTHS_NM` moves here.
  - The Sun's per-channel illuminance comes from R07's `starIlluminance`, through R06's colour
    (Design note 5), carried on each `SunState`, in place of R05's spectral-to-luminance factors in
    `solar.ts` (`sunIlluminanceRgb()`, `skyLuminanceScale()`).
  - `solar.ts` is kept as a check: the two agree in luminance to 1%, and their chromaticity
    difference is recorded.
  - R05's constants stay as a second medium for comparison, beside `HILLAIRE_REFERENCE`.
  - Earth's tabulated profile reproduces Bodhaine et al. 1999's τ_R(550) = 0.097 to 1%, as R05's
    exponential does (`decisions-r05.md` item 3), so that the comparison with R05 differs in
    vertical distribution, not in column.
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
  density at h\*. R05's `geodeticOf` (`hillaire.ts`), a fixed-point iteration that does not
  converge above a flattening of about 0.04, is replaced here, keeping its signature, by a closed
  form (Vermeille 2002, as R08.T12.d's Rust takes, or Bowring 1976), since Saturn's f = 0.098 needs
  it; R05.T12.c's tests keep passing. Tests (no GPU):
  - a sphere with ω = 0 reproduces T6.d's tables to 10⁻¹²;
  - the geodetic conversion round-trips on a Saturn-class figure to 10⁻⁶ m from the datum to 10
    scale heights;
  - on a Saturn-class figure, an `f64` brute-force march of the sun's path on the true spheroid,
    density at h\*, gives the sliced lookup's optical depth within 0.5% at grazing and 0.1% at μ
    ≥ 0.2: at the equator looking north and east, at 45° and at the pole;
  - interpolating between slices in ln κ costs under 0.1% in grazing optical depth, and between
    bands under 0.5% in multiple-scattering radiance, against a table built at the exact value;
  - Earth's slicing is one slice and one band, and Earth's differences from T6.d (s within ±0.26%)
    are recorded here.

  Acceptance: `pnpm --filter hyperion exec vitest run view/atmosphere/tablesCpu view/atmosphere/hillaire`.

- **R08.T6.f Slices and bands, the kernels.** `transmittance.wgsl`, `multiScattering.wgsl`,
  `skyView.wgsl`, `aerialPerspective.wgsl` and `rayMarch.wgsl` gain T6.e's slicing through
  2D array textures, one layer a slice or band, and a shared `oblate.wgsl`, a library prepended to
  each kernel as R05's `common.wgsl` and `medium.wgsl` are (the kernels it joins are what the
  catalogue holds). This task widens R05's `atmosphereInputs(camera, figure)` in R05's
  `hillaire.ts` itself, as R08.T6 widens R05's other names: its result, R05's
  `AtmosphereInputs { radiusM, heightM, normal }`, gains the camera's geodetic latitude and
  s = g(φ) ÷ g_ref from R08.T3.d (`gravityRatio`'s, `BodyGravity.gravityOffsetMS2` included, which
  `oblate.wgsl` takes beside γ_e, γ_p and g_ref), and R05.T12.c's tests (`hillaire.test.ts`) keep
  passing with s ≡ 1 at a = c. `setMedium` allocates the layers under `atmosphere-tables`; Earth's
  one slice and one band are one-layer arrays, bound through R08.T0's seam. When this lands,
  `ATMOSPHERE: APPROXIMATE` clears for oblate thin bodies. Smoke (`just test-render`): the
  readbacks agree with the twin to 10⁻³ on a Saturn-class figure and on Earth. Acceptance:
  `pnpm test` and `just test-render` pass; by hand, the Saturn-class limb from orbit over the
  equator and the pole is recorded here.

### R08.T7 Several suns, and the sky through the air

Sky-view and aerial perspective per photorealistic view, summed over the drawn suns in one march
(Design note 7), in `drawFrame(view, suns, scene)`. As built, R05's kernels carry one sun:
`view.wgsl`'s `AtmosphereView` has one `sun`, `skyScale` and `sunDisc`, `source.wgsl`'s
`sampleMediumAt` folds the phase into one cos θ, and `sourceAt` takes one μ_sun; this task widens
them. Files: `settings.ts`, `sources.ts` and `sources.test.ts`, `view/quality/qualitySetting.ts`,
`hillaire.ts`, `shaders/{view,source,skyView,aerialPerspective,rayMarch,composite}.wgsl`,
`tablesCpu.ts` and the smoke checks.

- the sky's sources are the body's, Design note 7's one list: the stars R07 lights it with
  (`lightsAt(centre, hosts, MAX_BODY_LIGHTS)`, `view/lighting/hostLights.ts`) and its planetshine
  sources (`planetshineSources`, `view/lighting/planetshine.ts`, at
  `SETTINGS[s].photoreal.planetshineSources`). The arrays are passed in, never rebuilt.
  `skySources(stars, planetshine, camera, figure, medium)` in `sources.ts` turns them into
  `SunState`s and applies Design note 7's two skips inside the air. A planetshine source's
  `SunState` carries its per-channel illuminance (T6.d's field, carried to the camera by the
  inverse square), its angular radius, light slot 0's absorber curve and no disc.
- this task writes `settings.ts` and adds `atmosphereView` to R05's `ViewSettings`, with its
  values in `SETTINGS`: `skySunCap` reading `MAX_BODY_LIGHTS`, and `skyPlanetshineSources`
  reading `photoreal.planetshineSources`. It sizes the kernels' source array at
  `MAX_SKY_SOURCES`, and fills `ATMOSPHERE_QUALITY_LIMITS` with Design note 7's limits: stars
  beyond the body's list, and planetshine's approximations.
- the sky-view, aerial-perspective and composite passes blend alpha with source zero and
  destination one, keeping R07's `METER_CLASS` in the HDR target;
- a sun below the horizon still feeds the sky through the planet's shadow in the tables;
- R06's sprites, band, cube and host discs are multiplied by the transmittance from the camera
  along their direction. As built none of R06's layers takes a per-direction factor (each takes one
  scalar `exposureScale`), and they are drawn into the scene target before the atmosphere, whose
  composite reads that colour (`AtmosphereScene.colour`). So the composite applies the
  transmittance to space along each sky pixel's ray, and R06's shaders (`view/sky/shaders/`) are
  edited only if that cannot serve, recorded here.

Tests (no GPU, CPU twin):

- a second equal sun at the same direction doubles the sky radiance;
- the sky's stars and planetshine sources are the arrays R07 builds for the body, element for
  element, on a shared fixture;
- on an Earth fixture, with the Sun 1° beyond the twilight limit and a full Moon at 45°, the Sun
  is skipped. The sky equals the noon sky's twin with the Sun at the Moon's direction, scaled per
  channel by the Moon's illuminance over the Sun's, to 10⁻⁶. With the Sun kept, the sky changes
  by under 10⁻⁶ of the Moon's;
- by day the Moon is skipped, and the sky changes by under 10⁻⁴;
- the source loop is exercised at `MAX_SKY_SOURCES`;
- after the atmosphere's passes, the HDR target's alpha still holds each pixel's `METER_CLASS`;
- two views with the same camera have equal tables;
- the summed sky-view agrees with per-source tables in the twin to 2% within 10° of each source,
  for a Rayleigh sky and for Earth's aerosol; if it fails, the per-sun fallback of Design note 7 is
  built here and the finding recorded;
- the per-view tables' bytes do not grow with the number of suns;
- a star seen at the zenith from Earth's surface is dimmed by e^(−τ) of the column.

Acceptance: `pnpm test` and `just test-render` pass, and by hand a binary sky on an Earth fixture
shows both twilights, recorded here with the development machine's timings and, from the owner,
the UHD 620's, and a moonlit night on the Earth fixture (the Sun beyond the twilight limit, a full
Moon) shows a sky within about two stops of the moonlit ground, with the fainter stars washed out.
Until R08.T10.b draws the atmosphere in the photorealistic view, the binary sky is looked at
through the smoke harness's hidden captures (`just test-render --captures=DIR`, R05.T12.c's) with
two suns, and its timings are taken with T11's.

### R08.T8 Views from outside and other bodies

Widen R05's ray march to N terms and several sources: each body's list of Design note 7 (its stars
and its planetshine sources), with no source skipped outside the air. The march draws any body's
atmosphere other than the ship's local body (`SceneFrame.localBody`) at its `apparentM` from R03's
`sceneAt(model, observer, time, previous)` (Design note 13), with the limb, the terminator and the
planet's shadow in its own air; a `contact` body, which has no position, draws none. The switch to
the sky-view table is by altitude, with a blend band. Tests (no GPU):

- the CPU march just inside the top agrees with the sky-view twin to 1% at the band's edges;
- R05.T12.e's quadrature gate holds for the widened march, at one source and at
  `MAX_SKY_SOURCES`;
- the sky-view table's interpolation across the limb, from cameras at 60–100 km, against the march
  at the same pixels, recorded with the blend band's altitudes;
- the limb falls to zero outside the top;
- a body's atmosphere is drawn at `apparentM`, and only the ship's local body at `geometricM`,
  including when a free camera sits inside another body's Hill sphere.

Acceptance: `pnpm test` and `just test-render` pass, and by hand an ascent from R05's test planet
to 10⁸ m shows no step at the switch, recorded here.

### R08.T9 Aerial perspective and surface lighting

- **R08.T9.a Aerial perspective.**
  - On the low setting, R05's deferred aerial perspective applies to terrain alone. On the high
    setting it applies to everything opaque (`AERIAL_PERSPECTIVE_SCOPE` in `settings.ts`, the
    projection of R05's `TableSizes.aerialPerspectiveScope`, `"terrain"` and `"scene"`).
  - Beyond the volume's 32 km reach (Hillaire 2020 §5.4, confirmed) the march takes over.
  - `aerialPerspective(view)`, widening R05's `aerialPerspectiveVolume()` (which returns `null` on
    `"terrain"`), exposes the volume and transmittance, and `skyView(view)` the view's summed
    sky-view table, to R11.

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
  and R11's lit passes. Both are catalogued. As built, the two names are R07's stubs in
  `view/shaders/litBody.wgsl` (1 and 0), so this task:
  - removes the stubs there, the modules that include `litBody.wgsl` joining
    `surfaceLighting.wgsl` in their place (WGSL resolves every name in a module);
  - binds the body's transmittance and irradiance tables to the disc materials, the smooth mesh and
    the `disc cells` kernel through `view/bodies/draw.ts`'s `DISC_TEXTURES`, at group 2 bindings 6
    and above (0–5 are taken). The `disc cells` kernel, a compute kernel, binds a one-layer table
    through R08.T0's seam;
  - has the callers in `bodyDisc.wgsl` pass the geodetic height and latitude from the spheroid
    normal and the sun's azimuth from local north, in place of R07's zeros, and take the sky term
    per light rather than once at the first light's μ₀;
  - keeps planetshine through the air, as Design note 7 rules (R07 left the choice to this plan).
    Each source's direct term goes through `atmosphere_source_transmittance`, the three-node disc
    rule over `atmosphere_sun_transmittance` at its centre's μ and azimuth, with light slot 0's
    absorber curve. Its sky term is its illuminance times `atmosphere_sky_irradiance` at its own μ,
    on the Lambert share as a star's is. Both terms, a star's and a planetshine source's, are added
    outside R07's below-horizon `continue`, so that a source just set still lights the twilight
    ground;
  - gives `atmosphere_sun_transmittance` the light's absorber-curve slot as a fifth argument,
    `curve : u32`. Design note 5's curves are per sun, and the four-argument form cannot pick one.
    R10's `terrainLit.wgsl` and R11's lit passes pass it too, and take planetshine through the same
    functions;
  - rewrites `view/appearance/litBodyProbe.ts`'s and its smoke check's pins of the stubs' 1, 1
    and 0 against the tables' twin.

  A body without an atmosphere binds R07's one-texel defaults and reads 1 and 0, as the stubs did.

  Tests (no GPU, twin):
  - at the top of the atmosphere the sun's transmittance is 1 and the sky's irradiance is 0;
  - with no scattering terms the irradiance is 0 at every altitude;
  - Earth's surface irradiance, direct plus diffuse at a sun zenith of 0°, lies between the
    direct beam alone and the top's; R08.T13 checks it against the reference;
  - on a Saturn-class figure at equal sun zenith, the pole's diffuse irradiance at the datum is
    below the equator's, by the band tables' column ratio;
  - a planetshine source and a star of equal illuminance and direction, with ρ → 0, light a point
    identically, direct and sky;
  - the three-node rule equals the centre's lookup as ρ → 0 to 10⁻⁶, and at ρ = 9.8° and 10° of
    elevation on Earth's medium it agrees with a 200 × 200 quadrature over the disc in the twin to
    2%;
  - a source 2° below the local horizon gives no direct term and a non-zero sky term;
  - an airless body's planetshine is R07's, unchanged.

  Smoke: the readbacks agree with the twin to 10⁻³. Acceptance: `pnpm test` and
  `just test-render` pass.

### R08.T10 The medium of a body

In two subtasks (split on re-validation, since as built the atmosphere is not yet drawn in the
photorealistic view at all): T10.a assembles and caches the medium, and T10.b draws it in the view.

- **R08.T10.a The medium and its cache.** Described below, with its tests. Acceptance:
  `pnpm --filter hyperion exec vitest run view/atmosphere/assemble view/atmosphere/AtmosphereCache`
  and `pnpm test` pass.
- **R08.T10.b In the photorealistic view.** It needs T10.a and T7. The paragraph "This task also
  draws" below. Tests: `photorealisticPasses`' `atmosphere` entry is built and carries the passes'
  labels; `PhotorealRenderer` submits them after the discs and before R11's slots, and none for a
  view whose bodies have no medium (R05's counting fake, `test/countingRenderEngine.ts`); a
  withheld, unmodelled or approximate body's label reaches `photorealStatements`, after its other
  notes (`atmosphereStatements`, `labels.ts`); a kept scene's body with no atmosphere set, such as
  R07's `TEST GIANT`, carries `ATMOSPHERE: NOT YET MODELLED` (decision-r08-giant-label).
  Acceptance: `pnpm test` and `just test-render` pass, and by hand a flight past every fixture, on
  a kept test scene that carries them (`view/scenes/`), is recorded here.

`assemble.ts`, `optics.worker.ts`, `AtmosphereCache.ts` build, from the body's appearance and its
surface section, in a module worker, cached as Design note 14 says. The appearance is the scene's
R07 `WireAppearance` (`heldAppearanceOf`, `view/scene/fromServer.ts`: the figure, the photometry
and the labels), with the record's `mass_kg` and rotation; `BodyGravityInput` (R08.T3.d) takes the
figure's `law` and `momentOfInertiaFactor`, read from the record's `SystemBodyFigure` since
`WireAppearance.figure` (R07's `BodyFigure`) carries only the radii and the pole, so that
`bodyGravity` applies Design note 17's ω_fig; R07's `BodyAppearance` is not built in production.
The surface section comes from `body_detail`, requested through
`toBodyDetailRequest` (`lib/system/bodiesWire.ts`), which nothing in `view/` calls yet.
`AtmosphereCache` requests it when a photorealistic view first draws a body in the disc or mesh
regime (R07's `litRegimes`, decided in `planLitBodies`), again when the scene's arrival or the
granted detail changes, and again when `body_events` reports an event on the body (P14.T31, once
it exists; today it answers `unsupported`). The section's state decides the label. The build
produces:

- the column;
- the molecular, absorber and aerosol terms;
- the labels of Design note 12.

This task also draws the atmosphere in the photorealistic view (T10.b), where it is not drawn today:
`PhotorealRenderer.render` (`view/photoreal/renderer.ts`, which hard-codes its passes) gains the
atmosphere's passes after the discs, R07's `photorealisticPasses` slot `"atmosphere"` becomes
built with the labels those passes submit under (R12's `PASS_ROWS` keys on them), and the labels
of Design note 12 join `photorealStatements` (`displays/view/viewRun.ts`) beside `litLabelsOf`
(`displays/view/photorealFrame.ts`). `PhotorealRenderer` hands each body's stars and planetshine
sources, the arrays `planLitBodies` built, to that body's atmosphere (Design note 7's one list).
For the camera's local body, which `planLitBodies` may not light while terrain draws it, the same
two calls are made once a frame.

An airless body (`SurfaceState::Airless`) draws no atmosphere and no label. Until the surface
section carries P14.T24.a's figures (R08.T1's P14.T35.e), generated bodies draw no atmosphere and
show `ATMOSPHERE: NOT YET MODELLED`, and so does a giant, whose surface section is
`not_applicable`, until its `envelope` section exists (decision-r08-giant-label). The task runs on
the fixtures of Design note 16: Earth, Earth with ozone, Mars with hand dust, Venus at 92 and at
58 bar, a hand Titan, and a hand giant. Those that need R08.T5.b's files (Venus's clouds, Mars's
dust, Titan's haze) wait on those files. Their licences were ruled on 2026-10-09
(`decision-r08-licences.md`).

Tests (T10.a):

- a withheld section gives no medium and `atmosphereNotResolved`, and a `not_modelled` one
  `atmosphereNotYetModelled`;
- a request is sent once per body per trigger, and none for a point-regime body;
- an absent inventory gives no aerosol term and `aerosolsNotYetModelled`;
- a giant's record (surface `not_applicable`, no envelope) gives no medium and
  `atmosphereNotYetModelled`;
- a sub-Neptune's record (surface `not_applicable`, envelope `not_modelled`) gives no medium and
  `atmosphereNotYetModelled`, as a giant's does;
- an airless body gives no medium and no label;
- a body whose request is in flight gives `atmospherePending`, and none on a re-request while its
  previous state is drawn;
- a flyby that brings many bodies into the disc regime does not make `ATMOSPHERE: PENDING` flash
  more than three times a second (the guide's flash threshold; decision-r08-giant-label, §4). If
  it would, the scene's lit bodies are asked for on arrival, or the line is held as R05's terrain
  annunciation holds its own (`view/terrain/annunciation.ts`);
- the same inputs give the same key and a changed inventory another;
- a `rotational_and_tidal` record's medium is built at ω_fig = √2.5 ω (its g_ref includes ω²R),
  and a `capped` one builds where its true ω would put γ_e ≤ 0;
- results transfer, and the cache evicts least recently used within its stated size;
- each fixture's optics time is recorded, on a quiet machine.

Acceptance: T10.a's and T10.b's, above.

### R08.T11 The thin benchmarks

Record, by hand on the development machine's RTX 3080 (the discrete target) and, by the owner, on
the UHD 620 at 720p (low), against
the budget ([Performance budget](../../brainstorming/rendering-and-planets.md#performance-budget)):

- the per-frame atmosphere time, against 2–4 ms low and 0.5–1 ms discrete, at one source, at the
  setting's count (2 + 2 on high, 2 + 1 on low) and at `MAX_SKY_SOURCES`, the last for R07's
  deferred raise. These are estimates, recorded as findings: R05's gate judges terrain and
  atmosphere together (R05 Design note 21, decided 2026-10-06). R05 measured, at one sun and three
  terms on the RTX 3080, about 1.6 ms p50 at full clock, 2.62 / 3.37 ms p50 / p95 at the driver's
  light-load clocks after R05.T12.e, and 3.19, 4.07 and 4.28 ms at the 50th, 95th and 99th
  percentiles on the gate's judged run (1,509 × 821 px; R05's Risks, "The gate's verdict"), the
  figures the brainstorm's budget section has carried since 2026-10-08; R12.T10 replaces the
  estimate;
- the per-planet table time, against about 1 ms and under 0.1 ms;
- the table bytes a planet, against 2 MB, and the per-view bytes on their own line, against
  Design note 11's 0.43 MB, at one, two and four sources;
- the slice and band counts, bytes and per-planet table time of the Jupiter- and Saturn-class
  figures, against Design note 11's 0.57 and 0.72 MB, of an ice giant (2 and 2), and of a giant
  at plan 14's cap (f = 0.2, 9–10 slices and 6–7 bands, Design note 11's worst generated case),
  and any widening of `KAPPA_STEP` or `BAND_STEP`. If the capped giant stays over 2 MB after both
  widenings, it is reported to "main" for a decision agent's ruling (Design note 11).

R05's `just descent-spike` runs with the generalised atmosphere, and its percentiles are recorded
against R05's. Where the low setting misses, `TABLE_SIZES` changes here. Acceptance: the record is
here, and `pnpm test` passes.

### R08.T12 The reference path tracer

- **R08.T12.a The tracer.** `crates/hyperion-fit/src/atmosphere/` holds a spherical backward
  Monte Carlo in `f64` (`Shells::Sphere`; R08.T12.d adds the spheroid), with its unit tests in the
  module, so that the filter `atmosphere` selects them. It reads a spectral case (`serde_json`, a
  workspace dependency, added to the crate's `Cargo.toml` as `serde_json.workspace = true`, with
  the comment there that lists the crate's allowed dependencies extended) and has:
  - next-event estimation to each sun, and Russian roulette;
  - an explicit detector cone;
  - an optional Stokes mode for Rayleigh;
  - reproducible results for any thread count, by the crate's convention: an
    `Option<NonZeroUsize>` thread count, `parallel::map_reduce_chunks`, and counter-based draws
    keyed per sample, as `tasks/displaced_forms/births.rs`'s `Draws` are; arithmetic through
    `hyperion_sim::math`, as the crate's `clippy.toml` requires.

  The command `atmosphere-reference` is a variant of `cli.rs`'s `Command`, dispatched in `run_in`
  and listed in the module's documentation, with any new `RunFitError` variant in `lib.rs`'s
  `exit_code`.

  Tests:
  - an absorbing-only medium gives Beer–Lambert to 10⁻⁹;
  - one and four threads give identical results;
  - the Stokes mode's I equals the scalar mode's for an isotropic scatterer.

  The scalar-against-vector Rayleigh difference is measured on one thin Rayleigh case of Natraj et
  al. 2009 and recorded as a finding (Risks). It was ruled on 2026-10-09
  (`decision-r08-vector.md`): it is corrected by R08.T14.e–f, and the gates compare with the
  vector reference. Acceptance: `cargo test -p hyperion-fit atmosphere`.

- **R08.T12.c The tracer's benchmarks.** Slow tests run the benchmarks of Design note 10, and the
  tracer's references (R08.T12.b) are committed only once these pass. They are the workspace's
  slow tests, `#[ignore = "slow: …"]` (run by `just test-slow` under its `slow-test` profile), in a
  module `atmosphere::benchmarks`:
  - Garcia and Siewert's Haze L and Cloud C1, Natraj et al.'s Rayleigh tables (Stokes),
    Kokhanovsky et al. 2010 and IPRT Phase A, each to 3σ with σ ≤ 0.3%;
  - Loughman et al. 2004 within its 2–4% spread.

  Before the benchmarks run, the Stokes mode gains (ruled 2026-10-09, `decision-r08-vector.md`
  item 1):
  - a `tabulated` phase kind in the case format: a₁ on its own angle grid and, optionally, a₂,
    a₃, a₄, b₁ and b₂ per wavelength (block-diagonal; Hovenier, van der Mee and Domke 2004),
    which the scalar mode reads for a₁ alone;
  - a `depolarising` mark: in the Stokes mode a term without a matrix is refused unless the case
    marks it so, and a marked term scatters by a₁ alone;
  - the Stokes vector (I, Q, U, V), V coupled through b₂;
  - beside I, the scalar radiance of the same paths, with the standard errors of both and of
    their difference.

  A tabulated matrix is checked on load (|a₂|, |a₃|, |a₄|, |b₁|, |b₂| ≤ a₁), and its table
  against the source's normalisation and asymmetry to 10⁻⁴. The matrices' sources:
  - Kokhanovsky's aerosol and cloud, and IPRT's spheres, from R08's Mie over its size
    distributions (`decision-r08-licences.md` row 6c). These cases need R08.T5.b; the others do
    not wait for it.
  - A non-spherical IPRT case, from IPRT's own matrix in `crates/hyperion-fit/data/iprt_phase_a/`
    if it fits the 500 kB hook, or recorded as not run.

  The modes:
  - Garcia and Siewert's cases run in the scalar mode.
  - Natraj's, Kokhanovsky's and IPRT's run in the Stokes mode, asserting I, Q and U, and V where
    published.

  Variance reduction for the clouds' forward peak is allowed only if unbiased. A case that
  cannot reach σ ≤ 0.3% records the σ reached and asserts 3σ at it, and "main" is told.

  Unit tests:
  - a sphere's single-scattering degree of polarisation is −b₁ ÷ a₁;
  - I equals the scalar mode's for a `depolarising` term;
  - a small sphere's tabulated matrix reproduces the built-in Rayleigh matrix.

  The same commit corrects three citations in `atmosphere/optics.rs` and `atmosphere/case.rs`
  (item 3):
  - "Hansen and Travis 1974 … eq. 2.15" becomes "eqs. (2.15)–(2.16), p. 541 (their δ is ρ here)";
  - "Witt 1977, ApJS 35, 1, eq. 13" becomes "eq. 19";
  - "Cornette and Shanks 1992 (…), eq. 8" loses its number, gains "doi:10.1364/AO.31.003152; the
    same function is Draine 2003, ApJ 598, 1017, eq. 5 at α = 1", and its mean cosine is stated
    as derived and checked numerically.

  The benchmarks' published values are committed only as the values each test asserts, in the
  test's own layout, each with its paper and table or figure cited. No table is committed whole,
  and none is shipped (ruled 2026-10-09, `decision-r08-licences.md` row 6). The sources:
  - **Natraj's tables** from <https://web.gps.caltech.edu/~vijay/Rayleigh_Scattering_Tables/>
    by SHA-256 (`CDS/CDS.tar.gz`:
    `49b01e4dbd7ba7aba928b2489b9901c69de05ae8d636f366d8ca5cb11da37134`). The host omits its
    intermediate certificate, which is supplied, never bypassed.
  - **IPRT Phase A's results** from <https://www.libradtran.org/iprt/> (CC BY-SA 3.0). The
    selected rows go in `crates/hyperion-fit/data/iprt_phase_a/`, with a sibling licence note,
    never inline in Rust.
  - **Kokhanovsky et al. 2010 and Loughman et al. 2004** from their free copies
    (<https://elib.dlr.de/65941/1/kokhanovsky2010a.pdf>, SHA-256
    `93888e0be3d8e9f3e10be5fa994438b5a2ad0cf713c0186da993fed777d05905`; the publisher's PDF).
  - **Garcia and Siewert 1985** from a lawfully held copy.

  The papers are kept in an untracked local directory. Without a copy of Garcia and Siewert, its
  assertions here and in R08.T14.a are recorded as pending for the owner, for access.
  Acceptance: `cargo test -p hyperion-fit atmosphere` and `just test-slow atmosphere::benchmarks`.

- **R08.T12.d The spheroid mode** (Design note 17). It needs R08.T12.a. `Shells::Spheroid`:
  free paths by delta tracking against a majorant (the density at the lowest gravity-scaled
  height the path can reach), boundary crossings against the spheroid shells as quadrics, and
  density at h·g(φ)/g_ref, with h the geodetic height by Vermeille's closed form (J. Geodesy 76,
  451, 2002) and g(φ) by Somigliana, both in Rust (rotational figures; a tidal case would take
  Design note 17's ω_fig and ω²R). Next-event estimation to each sun marches the same geometry.
  Tests:
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
  command `hyperion-fit atmosphere-reference --stokes` traces them, so that each reference holds
  the vector I and the scalar I of the same paths (Design note 10, Polarisation). The references
  are committed with sample counts, times and load average. The case writer gives every term its
  matrix from T5's tables, or marks it `depolarising`. The cases are written through `bless.ts`'s
  `toMatchFileSnapshot` and read as JSON imports; both kinds of file pass `prettier --check` or are
  listed in `.prettierignore`, and each stays under the 500 kB added-file hook.
  - A vitest asserts that every case equals what the optics produce now, and names the commands to
    regenerate both files.
  - The metric of Design note 10 is written once as `gate.ts`, with its own tests on synthetic
    inputs:
    - the images' grouping and each one's floor: a twilight image's dim sky is held to 5% of its
      own brightest, not to the noon image's;
    - the 4σ allowance, and the refusal of a reference whose σ exceeds T ÷ 8;
    - the cone average;
    - the recorded ΔE*ab, relative RMS difference and max ÷ mean.

    The references are traced until σ_ref ≤ T ÷ 8 at every geometry (decision-backlog-1), and
    each case's sample counts are committed with it.

  - Sanity tests on the references:
    - the Venus-class case's surface downward flux is 2–4% of the top's at the sun of the Pioneer
      Venus large probe's solar flux radiometer (LSFR), with the surface sky red-shifted (Tomasko
      et al. 1980, 2.5%);
    - the Titan-class case's is 5–15% (Tomasko et al. 2008, marked to be re-read);
    - Earth's and Mars's aggregates are recorded.

  Acceptance: `pnpm test` passes and `just fit-check` is unaffected (the subcommand is not a
  `FitTask`, so its outputs are outside `tables.lock`).

### R08.T13 Where the analytic term drifts

A vitest applies `gate.ts` to the CPU twin's thin tables with the analytic term on every case,
against the references' scalar radiance. This task measures Hillaire's scalar term; R08.T14.f
re-runs the gate against the vector radiance with the polarisation correction (Design note 10). The
subcommand `hyperion-fit atmosphere-sweep --out <sweep.json> [--smoke]` traces single-layer media
over vertical extinction optical depth, single-scattering albedo and asymmetry. Beside each point
it writes the twin's analytic radiances, which a bless-style vitest supplies as a case file. The
full sweep is run by hand and its output committed as `reference/ms-sweep.json`, kept under the
500 kB added-file hook (the grid is sized for it, or the file is split by ω). `atmosphere-sweep` is
a `cli.rs` variant like `atmosphere-reference`. From it,
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

Per Design notes 9 and 10, in six subtasks. R08.T14.d follows R08.T14.f.

- **R08.T14.a The plane-parallel solver.** `thick/discreteOrdinates.ts`: discrete ordinates in
  `f64`, 16 streams, about 64 layers, with all μ₀ solved on one factorisation. It is ported from
  Stamnes et al. 1988, never from GPL cdisort. Tests: Garcia and Siewert's Haze L and Cloud C1 to
  their published digits at 10⁻³ (the values asserted only, as in R08.T12.c); energy is conserved
  to 10⁻⁶ for ω = 1. A PythonicDISORT plane-parallel cross-check is run once by hand and recorded.
  Acceptance:
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
  `ATMOSPHERE: COMPUTING` shows while a gated bake runs, in `ATMOSPHERE: APPROXIMATE`'s place and
  never beside it for one body, and `ATMOSPHERE: APPROXIMATE` takes its place if the bake fails,
  until the body's atmosphere is next computed (the guide's draft rows, R08.T2). Acceptance:
  `pnpm test` and `just test-render` pass.
- **R08.T14.e The polarisation correction** (Design note 10; ruled 2026-10-09,
  `decision-r08-vector.md` item 2). It needs R08.T14.a and R08.T14.b.
  - **The solver.** `thick/polarisation.ts` is a vector mode of `thick/discreteOrdinates.ts`
    for (I, Q, U, V), with V carried only when a term has b₂.
    - It is ported from the papers (Siewert 2000, JQSRT 64, 227; Schulz, Stamnes and Weng
      1999, JQSRT 61, 105), never from GPL code.
    - Each term's matrix is expanded in generalised spherical functions (de Rooij and van der
      Stap 1984, A&A 131, 237) and delta-M truncated as the scalar bake is.
  - **The bake.** `bakePolarisationCorrection` solves the vector and the scalar problem on the
    same layers and stores ΔJ_pol(h, μ₀, μ_v, m) = J_vector − J_scalar (the I component) on
    R08.T14.b's grid.
    - The solve: 8 streams, Fourier modes m = 0..2, R08.T14.b's pseudo-spherical beam, all μ₀
      on one factorisation, at the render channels, once per latitude band.
    - Where the beam is undefined, ΔJ_pol = 0, recorded.
    - It runs in the optics worker for every world with a term whose b₁ ≠ 0, thin or thick.
  - Tests:
    - at 16 streams the vector mode reproduces Natraj et al. 2009's τ 0.5, μ₀ 0.2 I, Q and U to
      10⁻³ (the values asserted only, as in R08.T12.c);
    - energy is conserved to 10⁻⁶ for ω = 1;
    - with every term `depolarising`, ΔJ_pol is 0 to 10⁻¹²;
    - the 8-stream ΔJ_pol added to the 16-stream scalar solve matches the 16-stream vector
      solve to 0.5% of I on the Earth, three-bar, Venus-class and Titan-class columns (the
      ruling measured ≤ 0.12% on a Rayleigh slab);
    - modes past m = 2 change I by under 0.1%, or more are kept, recorded;
    - on the thick cases, the channel solve agrees with a 15-bin solve converted to channels
      to 0.5% of I, or thick worlds solve it in the bins, recorded;
    - the time a band is recorded beside the scalar bake's (estimated by operation count at
      0.15–0.5 s).

  Acceptance: `pnpm --filter hyperion exec vitest run view/atmosphere/thick/polarisation`.

- **R08.T14.f The correction's read path and the vector gates.** It needs R08.T13, R08.T14.c
  and R08.T14.e.
  - **The read path.** The sky-view, aerial-perspective and march kernels and their CPU twin
    add σ_s·Σₘ ΔJ_pol,m cos(mΔφ) per source to their multiple-scattering term: Hillaire's
    σ_s·Ψ_ms·p_u, or R08.T14.c's thick table.
    - It is stored one layer a band, interpolated at the sample's latitude, and read on both
      settings.
    - R08.T16.b's disc bake reads it as the march does.
    - `TABLE_SIZES` gains its size, and it is cached per world.
    - While a world's correction bakes, the label block says `ATMOSPHERE: COMPUTING`, as for
      any gated bake (Design note 9).
  - Tests:
    - R08.T13's case gate, re-run against the references' vector I with the correction:
      Earth, Mars and `saturn-oblate` pass. The uncorrected twin against the scalar I is
      recorded beside;
    - R08.T13's surface-irradiance test against the vector flux aggregate, to 2%, uncorrected;
    - the kernels' correction equals the twin's under `just test-render`;
    - the per-frame cost on the RTX 3080 and the bytes a planet are recorded against Design
      note 11; the UHD 620's cost is the owner's to record. If the low setting misses its
      budget, it drops the correction from the aerial-perspective volume first and m = 1
      second, recorded.

  Acceptance: `pnpm test` and `just test-render` pass.

- **R08.T14.d The gates.**
  - It needs R08.T14.f. The Venus-class cases (92 and 58 bar) and the Titan-class case, through
    the CPU twin with the baked table and the polarisation correction, pass `gate.ts` against the
    vector I per geometry. Both m = 0 alone and m = 0..1 are run, and which is needed is
    recorded. The Rayleigh-only Venus columns gate first. The cloudy and hazy cases follow once
    R08.T5.b's H₂SO₄ and tholin files exist (licences ruled 2026-10-09,
    `decision-r08-licences.md`).
  - The bake time on the UHD 620's host (by the owner), summed over the bands with the
    polarisation correction's, is under `BAKE_CEILING_S` on a quiet machine, for a Saturn-class
    figure (4 bands) and for a giant at plan 14's cap (6–7 bands, or 5 after `BAND_STEP`'s
    widening), or the Risks' fallback is taken and recorded. Before that fallback, the correction
    drops to 6 streams and then halves its μ₀ axis, each recorded. If the capped giant fails it,
    it is reported to "main" for a decision agent's ruling (a further widening or fewer bands at
    the cap, Design note 11). The bands nearest the camera bake first, and
    `ATMOSPHERE: COMPUTING` clears when the last lands.
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
  - R08.T14.e's correction runs over the medium above the deck, with the deck as a depolarising
    lower boundary (a stated approximation), and the giant-deck case's gate is against the
    vector I.
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
  When P14.T24.d and P14.T35.e are on the wire, the medium comes from the giant's `envelope`
  section, and the tests re-run on it. Tests:
  - the medium has no ground, and its deck lies at the pressure its input states;
  - once the envelope section exists, a generated Jupiter-class giant's medium places its NH₃
    deck at 0.5–1 bar, as P14.T24.d's own test requires;
  - its limb shell of about ten scale heights is resolved at 2.2 × 10⁸ m, at R02's centre-pixel
    scale, which R07 adopted on 2026-10-03 (R07's Risks, T5 as built); the brainstorm's 2.5 × 10⁸ m,
    under
    [The scales the view spans](../../brainstorming/rendering-and-planets.md#the-scales-the-view-spans),
    is the same figure at width ÷ field.
- **R08.T16.b `DiscReflectanceTable`.** `discReflectance.ts` bakes it spectrally from the medium
  and tables, one layer a latitude band (Design note 17). This task adds its read path to R07's
  `view/shaders/bodyDisc.wgsl`, read at each disc pixel's geodetic latitude, beyond
  `GAS_GIANT_FULL_PASS_BOUNDARY_M` for a body with an atmosphere, keeping the albedo-only shading
  for airless bodies, and updates the catalogue entries that compose the library (`bodies:disc`,
  `bodies:discLimb`, the smooth mesh and the `disc cells` kernel). The table binds through
  `view/bodies/draw.ts`'s `DISC_TEXTURES` at a free group 2 binding (6 or above, after R08.T9.b's),
  and R07's CPU twin `view/bodies/discShading.ts` gains the same read. The boundary's hysteresis,
  which R07 leaves to this plan's caller, is applied here in `planLitBodies` (`view/bodies/draw.ts`),
  with R07's `REGIME_HYSTERESIS`. Tests:
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
  - every table's CPU twin against the reference on Design note 10's metric (T13–T16;
    decision-backlog-1), in `just ci`, with u′v′, ΔE*ab and the relative RMS difference recorded;
  - the polarisation correction against a 16-stream vector solve (T14.e), and every gate against
    the vector reference from T14.f on;
  - the surface irradiance against the reference's downwelling flux, and the summed sky-view against
    per-sun tables (T7, T13);
  - oblate bodies (Design note 17): normal gravity against WGS 84's γ_e and γ_p to 10⁻⁹ in both
    languages (T3.d, T12.d); the sliced twin against an `f64` brute-force spheroid march (T6.e);
    and `saturn-oblate` passing the gate with the slicing and failing it without (T13).
- **Reference, slow:** the tracer against Garcia and Siewert (scalar), Natraj, Kokhanovsky and
  IPRT A (Stokes, with tabulated matrices) and Loughman, under `just test-slow`; its spheroid mode
  against its sphere mode at a = c (T12.d).
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
from figures plan 14 generates. The asks of R08.T1 are plan 14's tasks, with plan 14's bumps and
P14.T35.e's change of the wire (`PROTOCOL_VERSION` is 2), each coordinated through "main".
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
  latitude bands. Earth-like bodies get one of each. Four things are still open:
  - the slice and band counts are computed, not measured, and wait on R08.T12.d's spheroid mode
    and T12.b's Saturn-class case;
  - the extra bakes may push a Saturn-class world, or a giant at plan 14's cap (6–7 bands), past
    `BAKE_CEILING_S`, and the capped giant past 2 MB (Design note 11);
  - zonal-wind gravity (about 1.4% on Saturn), the real 1-bar surface's departure from the
    spheroid, and T(p) varying with latitude are not modelled;
  - the sectoral tide of synchronous bodies (Design note 17): plan 14 draws the spheroid, so the
    0.6(a − c) bulge towards the primary (0.2–2 H on close-in planets) and g's ±¾(4 − k_f)m in
    longitude are not drawn; and a capped figure's equatorial gravity is overstated.

  Until R08.T6.f lands, a body with ln(κ_max ÷ κ_min) > 0.02 is drawn under
  `ATMOSPHERE: APPROXIMATE`. Sources: Chapman 1931; Heiskanen and Moritz 1967; Syndergaard 1998;
  Lindal, Sweetnam and Eshleman 1985 (full list in Design note 17).

- **Deviations in T3.d, as built** (2026-10-09).
  - `oblate.ts` adds, beside the Provides:
    - `BodyGravityInput { figure, massKg, angularVelocityRadS: number | null, bulkGravityMS2 }`;
    - `BodyGravity { spheroid: LevelSpheroid | null, referenceGravityMS2, slicing }`;
    - `bodyGravity(input, referenceRadiusM)` and `gravityRatio(gravity, φ)`.

    (The follow-up adds `figureLaw` and `gravityOffsetMS2`, below.) The Provides' four functions
    had no place for the task's inputs (GM = G × `mass_kg`, ω or none) or its fallback. With no
    rotation section the spheroid is `null`, g_ref the bulk gravity, the slicing one slice and one
    band, and `gravityRatio` 1. T10.a fills the input from the record. `AtmosphereMedium` as
    sketched carries g_ref and the slicing but not the spheroid, which T6.e and T6.f need for s at
    every latitude: T10.a puts the `BodyGravity` in the medium, or T6.e takes it beside the medium.

  - `directionalCurvatureRadiusM(figure: SpheroidFigure, …)` takes R05's radii-only figure
    (`hillaire.ts`), since R_α needs no gravity; a `LevelSpheroid` or R07's `BodyFigure` serves.
  - The single slice is at κ = 1 and the single band at s = 1, R05's per-planet tables as they
    are; Design note 17 does not place them. κ and s are ranged over 1,025 latitudes with R_α
    between M and N; where gravity rises poleward the extremes are s_e M_e and s_p a² ÷ c.
  - Bands, as first built: one at s = 1 while ln(s_max ÷ s_min) ≤ `BAND_STEP`, otherwise
    1 + ⌈ln(s_max ÷ s_min) ÷ `BAND_STEP`⌉ from s_min to s_max, evenly in ln s. Design note 17 gave
    the step and the counts but no threshold; its counts (the ice giants' span 0.050 → 1,
    Jupiter's 0.154 → 3, Saturn's 0.282 → 4) put it in [0.050, 0.154), widening with `BAND_STEP`.
    Ruled below and replaced by `ONE_BAND_BELOW` in the follow-up.
  - ω is `spinRateAt(law, time)`, exported from `lib/system/rotation.ts`: the sim's
    `RotationLaw::rate_at`, leaving out the capture's phase as it does, and held to the slope of
    `rotationAngleAt` over every law of `frame/body_rotations.golden` to 10⁻⁶ once the capture's
    2δΔ ÷ d² is added. For R05's Earth ω is `TEST_PLANET_RATE_RAD_PER_S` (`view/spike/rotation.ts`,
    the 86,164.0989 s stellar day since R05.T13.a; the Consumes' "86,164.0905 s sidereal" predates
    that correction); T3.d does not wire it.
  - G is `GRAVITATIONAL_CONSTANT_M3_PER_KG_S2` in a new `lib/system/constants.ts`, with the unit in
    its name and no imports, so that a worker can take it without the wire's adapters.
    `sceneWire.ts`'s private copy is gone, and `bodiesWire.ts`'s `EARTH_MASS_KG`, whose literal was a
    second copy, divides by it, the same quotient to the bit. The sim's path is
    `hyperion_base::units::consts::GRAVITATIONAL_CONSTANT`.
  - Below e′ = 0.25, e′q₀′ ÷ q₀ is a 16-term series, where the closed forms cancel: WGS 84 and
    Uranus take it, Jupiter and Saturn the closed form, and a test holds the join to 10⁻¹².
    Computed: WGS 84's γ_e 9.7803253359 and γ_p 9.8321849379 m s⁻² (TR8350.2's 9.8321849378, to
    10⁻¹¹); Saturn's 9.077 and 12.037, Jupiter's 23.124 and 26.977; ln κ spans 0.0154, 0.120,
    0.355 and 0.592, and ln s spans 0.0053, 0.050, 0.154 and 0.282, for Earth, Uranus, Jupiter and
    Saturn.
  - Test bodies the plan does not give: Jupiter (Archinal et al. 2018's radii, IAU 2015 B3's
    nominal GM, 870.536° d⁻¹) and Uranus (Archinal et al. 2018's radii, Jacobson 2014's GM to four
    figures, 501.1600928° d⁻¹).
  - The shared T3 acceptance selects only `oblate.test.ts`; T3.d's other changes are gated by
    `pnpm test`, or by vitest on `lib/system/rotation`, `lib/system/bodiesWire` and
    `lib/scene/sceneWire`.
  - Ruled 2026-10-09 (science-r08-oblate), applied by a T3.d follow-up:
    - _The one-band span._ One band only while ln(s_max ÷ s_min) ≤ `ONE_BAND_BELOW` = 0.02,
      fixed and not widened with `BAND_STEP`.
      - One band would leave the column ±½ ln(s_max ÷ s_min) off: ±2.52% on Uranus, ±2.39% on
        Neptune, and ±2.66% on plan 14's own Uranus. Nothing gates it.
      - The threshold holds it to ±1%. Uranus and Neptune take 2 slices and 2 bands; two bands
        leave 3 × 10⁻⁴ to 1.3 × 10⁻³.
      - The second bake is 0.1–1 s by T14's count, under Saturn's accepted four.
    - _Figures not flattened by the spin alone._ Design note 17's ω_fig, by figure law:
      - `rotational`: the true ω;
      - `rotational_and_tidal`: √2.5 ω, with + ω²R;
      - `capped`: the Darwin–Radau inversion at the record's C ÷ Ma², which also stops the
        `RangeError` and the unbounded counts at the true ω;
      - `sphere`: the true ω (no sphere keeps an atmosphere).

      The sectoral tide is recorded as plan 14's spheroid limit.
  - Finding for plan 14 (the deferred list): `RotationLaw`'s doc (`planetary/derive/rotation.rs`)
    puts the capture's rate "under 10⁻¹⁵ rad s⁻¹ for a lock more than a year off"; 2δΔ ÷ d² is
    bounded by 2π ÷ d, 2 × 10⁻⁷ rad s⁻¹ for a lock a year off.

- **Deviations in T3.a, as built** (2026-10-09). `column.ts` and `column.test.ts`, and the
  `tabulated` variant in `medium.ts`.
  - _Names beyond the sketch._ The Provides names `ColumnInput` and `AtmosphereColumn` without
    sketching them:
    - `ColumnInput`: `surfacePa`, `temperature`, `meanMolarMassGPerMol`, `referenceGravityMS2` and
      `referenceRadiusM`;
    - `AtmosphereColumn`: `altitudesM`, `pressuresPa`, `temperaturesK`, `density`,
      `surfaceNumberDensityPerM3`, `meanMolarMassGPerMol`, `referenceGravityMS2`,
      `referenceRadiusM` and `topHeightM`, its `density` the `tabulated` n ÷ n_s over the same
      `altitudesM`.

    Added: `COLUMN_TOP_PRESSURE_RATIO` (10⁻⁷) and `COLUMN_INTERVALS` (1,024); the mixture as data,
    `GasProperties` (`molarMassGPerMol`, `heatCapacityOverR`, `source`), `MixtureComponent`
    (`properties`, `moleFraction`), `meanMolarMassGPerMol(mixture)`, `dryAdiabatExponent(mixture)`
    (R ÷ c_p, mixed by c_p) and `MOLE_FRACTION_SUM_TOLERANCE` (10⁻⁹, for T3.c's refusal too);
    `GAS_HEAT_CAPACITY` (a `HeatCapacity`, `overR` and `source`, per `Gas`) and
    `gasProperties(gas)`. In `medium.ts`, the union's new member is named, `TabulatedDensity`, and
    checked by `tabulatedDensity(altitudesM, relative)`. `lib/system/constants.ts` gains
    `ATOMIC_MASS_CONSTANT_KG`, the sim's CODATA 2022 m_u.

  - _The gases are T3.b's._ `Gas`, `BOLTZMANN_J_PER_K` and the molar masses
    (`GAS_MOLAR_MASS_G_PER_MOL`) come from `rayleigh.ts` (`GASES` in the test only), one copy each
    (main's ruling, 2026-10-09); `column.ts` holds only c_p ÷ R. Under the owner's directive of
    2026-10-09 the column reads no list of gases: μ and R ÷ c_p come from any species given as
    `GasProperties`.
    `GAS_HEAT_CAPACITY` covers T3.b's thirteen: Ne, Kr and Xe at 5/2, exact for atoms, and N₂O at
    CO₂'s 13/3 as the same linear triatomic, medium confidence (NIST-JANAF's c_p at 298 K is 7%
    higher). _Closed set, for the composition audit:_ the table is keyed by `Gas`, so a gas joins
    it with its c_p ÷ R when T3.b's list grows.
  - _The integral in closed form._ Design note 3 integrates "from p_s upward … the local kT ÷ (μ m_u
    g) at each step". The column takes the same equation's exact solution instead, with no step
    error: the geopotential height Φ(p) = k ÷ (μ m_u g_ref) × ∫ T d ln p in closed form for both
    profiles, and z = R_ref Φ ÷ (R_ref − Φ) for g = g_ref (R_ref ÷ r)², the inverse of the U.S.
    Standard Atmosphere 1976's geopotential height. A test holds it to a fourth-order Runge–Kutta
    integration of dz ÷ d ln p to 10⁻⁹, on Venus-, Saturn- and Titan-class columns, on a datum
    below the profile's p_s, and on β = 0 and a skin warmer than the datum. A column whose Φ reaches
    R_ref below its top, which no bound hydrostatic atmosphere has, is refused.
  - _The levels:_ 1,025, evenly in ln p from the datum to p_s × 10⁻⁷, or 1,026 with the tropopause
    added where it falls between. A tropopause within 10⁻³ of an interval of a level adds none
    (`TROPOPAUSE_MERGE_FRACTION`, private), so that the heights stay strictly ascending. Linear
    interpolation holds the density to 3.1 × 10⁻⁵ and its column to 2 × 10⁻⁵. T6.b resamples it
    onto its square-root spacing.
  - _Refusals._ `temperatureAt` throws `RangeError` for a pressure that is not finite and positive,
    a profile temperature or p_s that is not, or a β that is negative or not finite;
    `hydrostaticColumn` also for a p_s, μ, g_ref or R_ref that is not finite and positive.
  - _The datum._ `ColumnInput.surfacePa` is the column's base. A `radiativeConvective` profile's
    own `surfacePa` need not equal it (a gas envelope's 1-bar datum), and the column starts at
    `temperatureAt(profile, surfacePa)`.
  - _The `tabulated` rule_ is R08.T12.a's tracer's (`hyperion-fit`'s
    `atmosphere::case::DensityProfile::Tabulated`, relayed by main), so that the client and the
    reference read the same air: linear between levels and constant beyond the first and the last,
    with the tracer's checks (at least two levels, finite and strictly ascending heights, finite
    and non-negative densities). So a column holds its top level's density, about 10⁻⁷, above its
    top, and a medium built on it takes a `topHeightM` no higher than the column's. `packMedium`
    refuses a `tabulated` term, since `densityOf` would read it as a tent, and `tables.test.ts`
    holds the refusal; R08.T6.b replaces both when it gives `tabulated` a code.
  - _The tests' figures._
    - Titan's tropopause from Design note 3's inputs is 0.2439 bar. The note's "0.24" is that figure
      to two places, 1.6% off, so the test holds 0.244 to 1%. Design note 3's 0.24 is corrected to
      0.244 here, for the next revision.
    - Earth's "about 8.4 km" is 8,433.4 m at the datum (8,433.6 m over the first interval, as the
      test reads it) at μ 28.97 and WGS 84's g_ref 9.8062, 1.3 × 10⁻⁴ from the U.S. Standard
      Atmosphere's R\*T₀ ÷ (M₀g₀) = 8,434.5 m, held to 10⁻³.
    - The Saturn-class column: 134 K at 1 bar and 96.3% H₂, 3.25% He and 4,500 ppm CH₄ (the NSSDCA
      fact sheet), α = 0.85 (Robinson and Catling 2012's Jupiter), and a skin of 2^(−1/4) × 81.0 K.
      Its mass at 0°, 45° and 90° is p_s ÷ g(φ) × 1.0014, its spherical excess (computed). The
      plan's test asserts p_s ÷ g(φ) to 0.5%, which cannot tell g_ref from the bulk gravity on
      Saturn (10.45 against 10.44 m s⁻²). An added test holds g(φ) times the column, at each of the
      three, to g_ref times the reference column, ∫ (1 + z ÷ R_ref)² dp with z from the stepped
      integration, to 10⁻⁴: a column built at the bulk gravity misses it by 1.2 × 10⁻³ (checked by
      hand). The column takes the gravity it is given; R08.T10.a passes
      `bodyGravity(…).referenceGravityMS2`.
    - The thick case is a Titan-class isothermal column (94 K, 1.4 bar of N₂), 1.6% over p_s ÷ g.
      It matches the exact isothermal integral to 10⁻⁴, and 1 + 2H ÷ R + 6(H ÷ R)².
    - Venus's skin, which does not bind at 1 bar, is 2^(−1/4) × the NSSDCA's 226.6 K, and the
      Venus-class column's radius and gravity the fact sheet's 6,051.8 km and 8.87 m s⁻².
    - The Titan-class radius and gravity are 2,574.76 km (Archinal et al. 2018, as JPL SSD quotes
      it) and GM ÷ R² with JPL's SAT441 GM, 1.3543 m s⁻².
    - An added test holds every gas's R ÷ c_p within 7.5% of its NIST-JANAF value at 298.15 K
      (Design note 3's "about 7%"; CH₄, NH₃ and N₂O are furthest, at 7.2–7.3%).
  - _Design note 3's NIST list, corrected_ (the science check): NH₃'s R ÷ c_p at 298 K is 0.233
    (NIST-JANAF's c_p of 35.652 J mol⁻¹ K⁻¹), not 0.237, which is 8.314 ÷ 35.06; N₂'s is 0.2855,
    0.285 to three places. Its 1/4 stays, 7.2% from the measured value.
  - _Not modelled_, as `column.ts` states: real-gas compressibility, which moves no column, only its
    lowest scale height, 0.55% too dense for CO₂ at Venus's surface (Z = 1.0055, the NIST
    Chemistry WebBook); and the level spheroid's free-air gradient by latitude (Heiskanen and Moritz
    1967, §2-10, eq. 2-121), which on Saturn moves the column above the datum by at most 6 × 10⁻⁴,
    at the equator.
  - _For P14.T24.e_ (a finding, not built here): Design note 3's T_skin = 2^(−1/4) T_eq counts
    sunlight alone. For a giant with internal heat the grey skin is 2^(−1/4) T_eff; Saturn's T_eq
    of 81.0 K (the NSSDCA fact sheet) gives 68 K, its T_eff of about 95 K about 80 K, nearer the
    84 K observed at 0.1 bar (the science check, from memory for T_eff). The test's Saturn-class
    fixture follows the note.
  - _Pending:_ the golden list of the sim's levels to 10⁻¹² waits on P14.T24.e, which builds
    `planetary::temperature_at` (no `it.todo`: oxlint's `vitest/warn-todo` refuses one).
  - The shared acceptance selects `column.test.ts`; `medium.test.ts` and `tables.test.ts`'s new
    tests run under `pnpm test`.
- **Deviations in the T3.d follow-up, as built** (2026-10-09; science-r08-oblate's rulings 1 and
  2, `oblate.ts` and `oblate.test.ts`).
  - _Names beside the Provides:_
    - `darwinRadauSpinRadS(figure, gmM3S2, momentOfInertiaFactor)`, the inversion, exported for
      its test;
    - `FigureLawInput { law: FigureLawDto, momentOfInertiaFactor }`, which a `SystemBodyFigure`
      satisfies as it is;
    - `BodyGravityInput.figureLaw: FigureLawInput | null`, required so that T10.a cannot leave it
      out, `null` read as `rotational`;
    - `BodyGravity.gravityOffsetMS2`, the added ω²R (0 unless `rotational_and_tidal`), which
      `referenceGravityMS2`, `gravityRatio` and the slicing include.

    `LevelSpheroid`, `normalGravity` and `referenceGravity` stay pure Somigliana, so a tidal body's
    `BodyGravity.referenceGravityMS2` is not `referenceGravity(spheroid)`. `oblateSlicing` keeps
    its signature with nothing added; `bodyGravity` slices with the offset.

  - _The spins._ `rotational_and_tidal` takes √2.5 times the true ω, as ruled, rather than the
    inversion; the two agree to 1.4 × 10⁻¹³ on plan 14's figures. R is ∛(a²c), the figure's own
    volumetric radius, which plan 14's `Spheroid::from_volumetric` makes the record's mean radius,
    so the bulk section's radius is not read. `capped` reads only the figure, GM and C ÷ Ma², so
    its ω_fig is the same at every spin past the cap: for a Saturn-density giant 0.70 ω at 1.4
    break-up periods (the ruling's "0.66–0.70 ω" was taken there) and 0.50–0.55 ω at 1.0–1.1.
    `darwinRadauSpinRadS` refuses a C ÷ Ma² outside (0, 0.4], the wire's own range
    (`bodiesWire.ts`).
  - _What the wire lacks: nothing for this._ `BodyFigureDto` carries `law` and
    `moment_of_inertia_factor`, read as `SystemBodyFigure.law` and `.momentOfInertiaFactor`
    (`lib/system/model.ts`), so no plan-14 field is needed. R07's `WireAppearance.figure` (its
    `BodyFigure`) carries only the radii and the pole, so T10.a reads the two from the record's
    `SystemBodyFigure` beside the appearance (T10.a's text says so). The sectoral tide's triaxial
    datum is not on the wire; it is plan 14's to give, and recorded, not asked
    (decision-p14-phase-j, 3).
  - _Tests beyond the ruling's:_
    - an Earth-density rocky world at 12.5 h (ln s spans 0.0195) takes 2 slices and 1 band, and at
      12 h (0.0212) 2 and 2: slices and bands are decided apart;
    - `sphere` takes the true ω beside `rotational` and no law;
    - the inflated hot Saturn's two bands end at its g(φ) ÷ g_ref with ω²R;
    - an exact Roche-model figure, independent of first-order theory: a point mass (k_f = 0)
      locked about a primary of 1,000 times its mass at m = 0.01, its level surface found by
      bisection in the restricted three-body problem's pseudo-potential (Murray and Dermott 1999,
      ch. 3), and drawn as the spheroid of its mean equatorial and polar radii. It is held to
      5 × 10⁻⁴ in s and in g_ref (0.018% and 0.018% measured, against the ruling's 0.04% and
      0.017%), and the true ω's miss in s above 1.5% (1.9% measured);
    - the inversion refuses a C ÷ Ma² outside (0, 0.4] and a GM of 0, NaN or ∞, and
      `bodyGravity` refuses a `capped` record at C ÷ Ma² = 0.5.
  - _Computed_ (the bounds above are what is asserted). With the tests' figures:
    - the HD 209458b-like giant (m 4.37 × 10⁻³, f 0.0080): s within 4.7 × 10⁻⁵ of first-order
      theory and g_ref within 4.0 × 10⁻⁵; the true ω is 0.82% off; its span, 0.0194, is one band;
    - the inflated hot Saturn: two bands; s within 7.3 × 10⁻⁴ (the ruling's 0.10% left ω²R out of
      s), g_ref −0.059% (not asserted);
    - the capped Saturn-density giant (ρ 690 kg m⁻³, C 0.21, radius 7 × 10⁷ m, 1.1 break-up
      periods): 10 slices and 7 bands.

    With the same helpers, not asserted: an Io-like body, 9 × 10⁻⁶; and capped giants at
    Jupiter's density and C 0.25 (9 slices and 6 bands) and at an ice giant's 1,600 kg m⁻³ and 0.23
    (9 and 7). At the cap the counts depend on f and C ÷ Ma² alone, since ω_fig²a³ ÷ GM =
    f(1 + η²) ÷ 2.5.

  - _Departures from the ruling's text._
    - Design note 11, T11 and T14.d send a capped giant that stays over 2 MB or `BAKE_CEILING_S`
      to "main" for a decision agent's ruling. The ruling's plan text had "the owner rules"; the
      RM4/RM5 common rules of 2026-10-09 send nothing to the owner.
    - Its item 8(b) edit to the old `rotational_and_tidal` sentence falls with the "Open"
      sub-bullet that 8(b) then replaces wholesale; the corrected figures (±15m ÷ 8, 0.32% at Io's
      m) are Design note 17's and `oblate.ts`'s.
    - T10.a adds that the law and C ÷ Ma² are read from `SystemBodyFigure`; the Consumes' figure
      entry names the two fields; T3.d's "Bands" bullet above is kept, as first built; T3.d's
      constants list names `ONE_BAND_BELOW`; T6.f's s and T12.d's Somigliana name the added ω²R
      and the rotational case.
    - _A capped figure's overstated equatorial gravity, corrected_ (the science review; raised with
      "main", for a science agent to confirm). The ruling put it at "up to ⅔(ω² − ω_fig²)R",
      which is only the uniform part. At the equator the true spin's centrifugal term exceeds
      ω_fig's by the whole (ω² − ω_fig²)a: for a Saturn-density giant 0.42 γ_e at 1.4 break-up
      periods (the ruling's form gives 0.26) and 0.62 at 1.27, and near the floor, where q(a)
      reaches 1.25, the drawn equator would be unbound at the true spin. Design note 17 and
      `oblate.ts` state it so. It is a stated limit only; nothing computed changes.
    - _The HD 209458b-like case_ is the ruling's, at 1.38 times Jupiter's volumetric radius (1.35
      of IAU 2015 B3's equatorial R_eJ, in which published radii are quoted), so its m is 4.37 ×
      10⁻³ against 4.5 × 10⁻³ for Torres et al. 2008's or Southworth 2010's set; the test says so,
      and 15m ÷ 8 is 0.82–0.85% either way.
  - _Checked:_ every formula against its source as cited. The ruling's `oblate_check.py`, re-run
    under the capped scope, reproduces its `run1.log` exactly. The first-order theory was
    re-derived from Q = m[(3 ÷ 2)x² − ½z²]: its zonal part, its g ÷ g₀ = 1 − 2Q₀ + (k_f − 4)Q₂,
    its k_f from Darwin–Radau, f\* = η²f and the 15m ÷ 4.

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
  modern polarisability anisotropy would settle it. _Checked by R08.T3.b (2026-10-09):_ with Raj,
  Hamaguchi and Witek 2018's ab initio anisotropy in the King factor and the real-gas N, the
  module's H₂ runs 7.0% above Dalgarno and Williams (6.9% with the ideal N). Raj et al.'s own ᾱ
  gives a cross-section within 0.4% of the module's at 532 nm (a science check). Ford and Browne
  1973, or Raj et al.'s mean polarisability set against Peck and Huang, would test the
  refractivity itself.
- **Scalar radiance: decided 2026-10-09** (`decision-r08-vector.md` item 2). The error is
  corrected, not accepted.
  - **Measured:** R08.T12.a found −11.8% to +10.7% on Natraj's τ 0.5 case. The ruling's sweep
    found up to −18.7% and +11.9% at τ 0.5–1, and −5.2% to +4.6% in Earth's blue with aerosol.
  - **The correction:** R08.T14.e's 8-stream vector-minus-scalar table, read by R08.T14.f.
    From R08.T14.f on, every gate compares with the vector reference (Design note 10,
    Polarisation). Until then, the drawn sky carries the scalar error.
  - **Stated approximations:**
    - terms without a published matrix are total depolarisers;
    - planetshine is unpolarised;
    - Lambertian ground and decks depolarise;
    - the correction is plane-parallel with the pseudo-spherical beam, so its residual at
      twilight and the limb is the gate's to find;
    - fluxes and the surface irradiance are not corrected (under about 1%).
  - **Its costs**, which R08.T14.e–f measure: a sub-second bake a band, 0.20–0.39 MB a band,
    and three reads per sample per source.
- **Data licences: decided 2026-10-09** (delegated, `decision-r08-licences.md`, after
  `decisions-r05.md` item 4). No raw table is committed; the values R08 uses are committed
  reduced, with their citations in `NOTICE`:
  - methane from NASA PSG's conversion of Karkoschka and Tomasko 2010 (Elsevier's own image table
    is not used);
  - H₂SO₄, Mars dust and tholin from ARIA's, NASA Ames's and HITRAN's copies.

  NH₄SH takes a stated stand-in under `ATMOSPHERE: APPROXIMATE`. Benchmarks are committed as
  asserted values only. Serdyuchenko's data files (terms unstated) were ruled on 2026-10-02
  (`decisions-r05.md` item 4): no raw table, and R08.T4.b's reduced 1 nm table with its citation.
  The same rule covers the CIE matching functions (CC BY-SA 4.0): fetched with a checksum, and
  derived values committed with the CIE's citation. `NOTICE` must ship in the app once packaging
  exists (`decisions-r05.md` item 4's packaging ask).

- **Stars beyond the body's two** (decided 2026-10-09, `decision-r08-design.md` item 3). The sky
  takes the body's own list (Design note 7), so it agrees with the surfaces by construction. A
  third or fourth star past `STAR_CUT_RELATIVE` lights neither: it is unseen by day, and it is a
  night under a visible star while the two brightest are set. Two corrections to R07 are deferred
  (main's deferred list, for R12's audit):
  - `MAX_BODY_LIGHTS` per setting, 4 on high and 2 on low, which this plan's kernels already hold
    (`MAX_SKY_SOURCES` = 6);
  - for the camera's local body inside its air, Design note 7's twilight limit applied before the
    cap, so that a set star yields its slot to one that is up.

  Whether a dropped star that is up needs an annunciation is for R12's audit.

- **Planetshine's approximations** (Design note 7; decided 2026-10-09, `decision-r08-design.md`
  item 2). A planetshine source lights surfaces and the sky as a star does. Five things are not
  modelled:
  - the neighbour's own band depletion: a giant's methane seen through a Titan-class air's
    methane, where light slot 0's curve reads the absorption somewhat too deep;
  - its disc in the sky's phase function (a point at its centre, under 0.8% for Rayleigh at
    ρ = 9.8°);
  - a partly set source's sky, lit or shadowed by its centre;
  - the inverse square across the body in the march (±0.9% at Io);
  - R07's own stated errors (the crescent's centroid, the far-field E).

  The low setting's sky may take three sources (2 + 1) in a multiple system's twilight with a moon
  up. R08.T11 records it, and if the budget misses there, the low sky drops planetshine while a
  star lies within the twilight limit, recorded.

- **Which star's absorber curve the shared tables are built with** (open; `decision-r08-design.md`,
  adjacent finding 3, noticed there and not checked). Design note 5 makes each absorber's curve of
  growth per sun, and Design note 8 applies each sun's curve when the transmittance table is read.
  But the per-planet multiple-scattering and irradiance tables (Design note 7; R08.T6, R08.T9.b)
  are built once and shared by every sun, and their builds take the light to the sun through the
  absorbers. The plan does not yet state which star's curve those builds use. R08.T6.c's and
  R08.T9.b's agents settle it, and state it here, before they build.
- **`atmosphere_sun_transmittance`'s fifth argument** (`decision-r08-design.md` item 2 and
  adjacent finding 1). R08.T9.b gives it `curve : u32`, the light's absorber-curve slot, and adds
  `atmosphere_source_transmittance` for planetshine (Provides, "Tables and passes"). R10's plan
  still names the four-argument form (its Consumes, for R10.T10.b's `terrainLit.wgsl`), as R11's
  does. R10's re-validation must pick up the fifth argument, `curve`.
- **Refraction and scintillation are not drawn.** On Venus, refraction near the surface raises the
  horizon; R06 leaves scintillation to this plan, and the brainstorm asks for neither. If the
  realism ruling wants them, they are later tasks. R10 asks this plan for the sun's refracted
  apparent elevation for its shadow test (R10 Design note 10). The ask stays open until refraction
  is drawn; until then R10 reads the geometric elevation.
- **The metric's floor** (decided, decision-backlog-1, 2026-10-09).
  - The gate is |ΔL| ≤ max(0.05·L_ref, 3 × 10⁻⁴·L_max(image)) + 4σ_ref, with σ_ref ≤ T ÷ 8
    (Design note 10).
  - It replaces max(0.05·L_ref, 3σ_ref, 10⁻³·L_max(case)), which had three faults:
    - its floor took over from the 5% below 2% of the case's brightest, not 10⁻³ as this bullet
      said;
    - over the whole case, a twilight image's sky fell under the noon image's floor;
    - its 3σ term gave no noise allowance where the 5% governed.
  - The 5% is in radiance, so that it holds at every exposure. Through AgX at the automatic
    exposure it is at most about one just-noticeable difference (0.96 ΔL*).
  - The 2% aggregates stand.
  - A regime whose table cannot reach the gate stays `ATMOSPHERE: APPROXIMATE`. The gate is not
    loosened for it.
  - _The guide row, still to apply._ decision-backlog-1 §2.6 (f) gives the guide's
    `ATMOSPHERE: APPROXIMATE` row (`docs/frontend/ux-guidelines.md`) the check's figure: after
    "against an independent path-traced reference for that kind of atmosphere", the clause
    "agreement within 5% in radiance wherever in the view a difference could be seen", and its
    draft trailer's parenthesis becomes "(plan R08, R08.T2; the check's figure per
    decision-backlog-1)". Another lane is editing the guide, so the row is applied after that lane
    lands. Until then the row names no figure.
- **Licences no longer block the cloudy gates** (decided 2026-10-09, `decision-r08-licences.md`).
  The Venus-class cloudy case, Mars and the Titan-class haze gate as R08.T5.b's files land, and
  are drawn `ATMOSPHERE: APPROXIMATE` only until their gates pass. NH₄SH alone stays ungated: a
  stand-in index, the label, and no committed case, until visible optical constants are
  published. If a source is withdrawn, the earlier lean stands, with its own physics check: a
  published parameterisation cited without copying the table, that is a constant index with a
  published Cauchy slope, or a Henyey–Greenstein pair fitted in the paper.
- **M-dwarf suns.** A curve of growth weighted by a 15-sample spectrum is adequate for FGK, A and B
  stars, but not for M dwarfs. Their TiO bands at 590–630 and 705–760 nm overlap Chappuis and
  methane's 727 nm band, so the weight correlates with σ (Design note 5). This is a recorded
  limitation. The lean is that R06 supply a model spectrum at 1 nm for M stars (for example
  PHOENIX, Husser et al. 2013, A&A 553, A6). The ask is made and open (the roadmap's between-plans
  table; R06's Risks), outside R06's scope and waiting on the stellar libraries' licence ruling;
  until then the 15-bin interpolation stands for M stars and the limitation is recorded with them.
- **Titan's skin temperature.** Design note 3's Titan check takes T_skin = 64 K. A T_eq of
  83.5 K, from a Bond albedo of 0.265, would give T_skin 70 K and a tropopause at 0.40 bar,
  outside P14.T24.e's test. Where plan 14's Titan T_eq comes from is to be re-checked when
  P14.T24.e is built. _Checked by R08.T1 (2026-10-09), computed:_ plan 14's Titan is a snowball,
  Bond albedo 0.50, placed at 9.583 au with e 0.0565 (its Solar System table), so its T_eq is
  about 75.6 K and T_skin 63.6 K, which puts the tropopause near 0.24 bar inside the test. A change
  of the snowball's albedo moves it, and P14.T24.e's test says so.
- **The summed sky-view** (Design note 7) is a research lean of medium confidence, not checked
  against Hillaire's parameterisation. R08.T7 tests it, and the per-sun fallback costs 0.43 MB a
  sun a view.
- **The reference in `hyperion-fit`** is a new kind of output for that crate. If plan 15's owners
  prefer, it moves to a sibling offline tool, and nothing else changes.
- **Knowledge.** Until a sensors plan builds the body-level overlay, the server grants the detail
  level asked, so `ATMOSPHERE: NOT RESOLVED` is exercised only by tests and by requests that ask for
  less.
- **CPU budgets on the desktop, decided 2026-09-30 by a delegated decision** (the hardware
  decisions, item 3). `BAKE_CEILING_S`, 5 s a world, stays the UHD 620 laptop's figure, the
  minimum specification. The development machine (Ryzen 7 3700X) records its own bake time beside
  it and fails only if it is over the laptop's ceiling; no separate desktop ceiling is set.
- **The asks R08.T1 wrote into plan 14** (2026-10-09, at `62c196c3`). P14.T24.c–f sit under
  plan 14's Phase E and P14.T35.e under Phase H, each marked drafted for the owner. **The owner's
  sign-off is pending**: the orchestrator has a decision agent rule on them, the acceptance is
  recorded here when given, and the client is built to the drafts meanwhile. They were reconciled
  with R09.T0.a's five asks (plan 14's Phase K, P14.T48.a–e), and the reconciliation is in plan
  14's Risks too ("The rendering plans' asks of the surface section"). What departs from T1's
  bullets:
  - _One record section, one wire task._ P14.T48.e (R09's) defines the record's surface section;
    T24.e and T24.c add their members to it. P14.T35.e is "The surface section and the gas envelope
    on the wire": the whole section mirrored member for member, which R10 and R11 also read, not
    only the atmosphere's fields. Its members for T24.c–e and T24.d are declared
    from the start and absent until their tasks land (plan 14's T34 rule), which this plan reads as
    not modelled: `AEROSOLS: NOT YET MODELLED` and the isothermal seam.
  - _Field by field, where the two plans' asks met:_
    - T_s and p_s are P14.T24.a's, read by T24.e's profile and by T48.c's crater screening. The
      wire's vertical structure carries β and T_skin alone, so R08.T3.a's `radiativeConvective`
      takes `surfaceK` and `surfacePa` from the conditions.
    - The gravity is the bulk section's, which T24.a and T48.c both read; nothing repeats it.
    - The gases are T24.a's mole fractions, a list largest first summing to 1 to 10⁻⁹, which
      R08.T3.c's refusal assumes; T24.f fills CH₄ and O₂ and defines no field. Methane as an
      absorber is its gas fraction, so R08.T10.a builds methane's absorber term from that fraction
      and the column, and ozone's from T24.c's absorbers. That still draws methane only from plan
      14's inventory (Design note 12).
    - The condensing species is P14.T48.d's condensable, which T24.e's α reads (water 0.6, methane
      0.77, CO₂ or none 0.8), so the picture's lapse rate and R09's climate model agree.
    - T24.c's decks lie on T13.c's one saturation curve per species, with the vapour set at the
      ground by a surface relative humidity (from memory, for the builder), and each deck gives
      its vapour's cold-trap fraction above it, which R08.T10.a reads for the vapour's column there.
    - T24.c's dust reads plan 14's own figures, never R09's coarse wind, which reads the section (a
      cycle otherwise); R10's `Dust` class still follows R09's winds locally.
  - _The order:_ T24.f; then T24.a–b with P14.T48.a–e; then T35.e; then T24.e, then T24.c; T24.d
    beside any of them.
  - _The gas envelope_ (decision-r08-giant-label, finding F1). T24.d serves every body in the
    gas-envelope state, giants and sub-Neptunes alike (every sub-Neptune is one by construction),
    and such a body's surface section is `not_applicable` (P14.T48.e as reconciled). Design note
    13's "Giants inside 10⁹ m" and R08.T16 are written for giants alone. Once P14.T24.d lands they
    read every gas-envelope body, a sub-Neptune's visible atmosphere being its envelope and its
    deck the surface; the labels hold either way (that ruling). The envelope's profile is the
    `Envelope` variant of plan 14's `VerticalStructure`, so R08.T16.a gives `TemperatureProfile` a
    matching variant. T24.d gains H₂O for its deck; H₂S is the envelope's own species.
  - _Findings for this plan._ Plan 14 tracked no sulphur. A science agent has since drafted
    P14.T24.g (`science-r08-sulphur-co2ice.md`, 2026-10-09; for the owner).
    - It gives a runaway-greenhouse world a sulphuric-acid deck of four modes. On plan 14's Venus
      the deck lies at 1.34–0.04 bar with τ(550) 33, so this plan classifies it `cloudDeck` and
      R11's boundary leaves it here.
    - It gives a temperate world a stratospheric sulphate layer of τ about 0.003.
    - It adds SO₂ as an absorber. This plan draws no SO₂ term until a cross-section is ruled: its
      weak band near the violet edge is from memory, check at build.
    - A deck's modes carry their deck, and Design note 9's per-body rule sums them.

    CO₂ ice now has a derived index file in R08.T5.b: Warren 1986's real index as a Cauchy fit,
    and Hansen 2005's k. The Titan check of Design note 3 holds on plan 14's albedo (the Titan
    bullet above).

  - _Marked "from memory" in plan 14_, for the builder: the surface relative humidity, Ackerman and
    Marley's closed form, Mars's background dust, Niemann et al.'s 5.65%, the protosolar He/H₂, a
    sub-Neptune's metallicity, Sudarsky's class temperatures and Venus's trace of O₂.
  - _For "main", each with its lean:_ whether P14.T35.e bumps `PROTOCOL_VERSION` (lean: no, since
    an older client passes an `ok` surface through unread); a sub-Neptune's surface section turned
    `not_applicable` (lean: accept). **Decided** (decision-p14-t35e-wire): no bump, and the
    `not_applicable` sub-Neptune accepted, with `has_surface` split and the `envelope` slot added
    in P14.T48.e. The third asks for a sulphur rule for a Venus deck (lean: a science agent drafts
    it before P14.T24.c).
- **Re-validated at bce2aef5** (2026-10-09, RM4's start: `rendering-and-planets` is `main` at
  3e3dbb80, R01–R07 merged by PR #3, plus R13's plan; `GENERATOR_VERSION` 21, `PROTOCOL_VERSION`
  2). Swept every Consumes item against the code, and folded in R05's, R06's and R07's as-built
  records and the decisions that name this plan (`decisions-r05.md` items 1–4,
  `decision-r05-high-atmosphere.md`, R07's centre-pixel scale of 2026-10-03). Nothing built
  changes. What changed here:
  - _R05's names, as built_ (R05's T12.a note asked for them): `cornette-shanks` with `asymmetry`,
    `none` kept for absorption-only terms, `topHeightM`, and a medium carrying no radius (R_ref is
    `tableRadiusM(figure)`); `CHANNEL_WAVELENGTHS_NM` stays (680, 550, 440) until T6.d;
    `HillaireAtmosphere`'s four-argument constructor and `drawFrame(view, suns, scene)` returning
    the composite `DrawItem`; `SunState` gaining its illuminance; the per-planet `AtmosphereTables`
    moving to `AtmosphereCache`; `aerialPerspective(view)` widening `aerialPerspectiveVolume()`;
    `AERIAL_PERSPECTIVE_SCOPE` projecting R05's `TableSizes.aerialPerspectiveScope` (`"scene"` and
    `"terrain"`, not a second field with `"opaque"`); the WGSL libraries, the `Medium` uniform and
    the four transmittance read sites (T6.b, T6.c); `rayMarch.wgsl` a compute kernel; `geodeticOf`
    replaced in T6.e.
  - _R01:_ the smoke harness has no `no-f16` variant (it runs `default` and `no-subgroups`); a
    compute pass binds no sampler; a one-layer array was viewed as `2d` in a compute binding
    (decided: R08.T0); compute catalogue entries carry no `displayName`.
  - _R02 and R03:_ the label path (`photorealStatements`, `litLabelsOf`); `sceneAt`'s fourth
    argument and `null`, `SceneFrame.localBody`, and `contact` bodies, which draw no atmosphere.
  - _R06:_ the sky layers' real names (`skySpriteStars`, `BandLayer`, `SkyCubeLayer`,
    `HostDiscLayer`), none with a per-direction factor, so T7 dims them in the composite.
  - _R07:_ the scene's `WireAppearance` in place of `BodyAppearance`, which production never
    builds; `lightsAt` and `MAX_BODY_LIGHTS` = 2; `PhotorealRenderer` draws no atmosphere, so T10 is
    split into T10.a (the medium and its cache) and T10.b (the view's passes, the pass slot and the
    labels); `bodyDisc.wgsl` a library with group 2's bindings 0–5 taken; the stubs' callers and
    probe (T9.b); the 10⁹ m boundary's hysteresis (T16.b); the limb's 2.2 × 10⁸ m (T16.a).
  - _Galaxy plan 14:_ rotation and figure are built and on the wire (P14.T46.f), so ω comes from
    `BodyRotationDto`; P14.T35.d is taken, so R08.T1's wire amendment is P14.T35.e (the roadmap's
    two mentions renamed with it); no plan-14 task gives the surface section fields, so P14.T35.e
    also asks for P14.T24.a's and T24.b's; the cloud fraction and surface pressure are not on the
    wire; G has one module-private client copy.
  - _Tooling:_ `bless.ts` under R04.T10.c's rule that renderer code reads no files through
    `node:fs` (`toMatchFileSnapshot`, `vitest.config.mts`); raw data reduced by a Node tool in
    `apps/hyperion/src/tools/` (R05's `solarFactors` precedent); `optics.worker.ts`; `serde_json`
    added to `hyperion-fit`; the subcommands as `cli.rs` variants outside `tables.lock`; slow tests
    `#[ignore]` under `just test-slow atmosphere::benchmarks` rather than the whole slow suite;
    committed JSON under `prettier --check` and the 500 kB hook.
  - _Decisions folded in:_ centred 1 nm ozone bins in T4.b, and R05's own coefficients in T6.c's
    regression (`decisions-r05.md` item 2); Bodhaine's τ_R(550) = 0.097 in T6.d (item 3); the
    reduced ozone table ruled committable (item 4); R05's measured per-frame figures in T11.
  - _Task order:_ T3.b, T3.d, T5.a and T12.a start at once beside T1 and T2; T3.a follows T3.d;
    T6's subtasks name their own predecessors; T1 and R09.T0.a are committed to plan 14 one after
    the other.
  - _The brainstorm_, since this plan was written (576bd1ee): its atmosphere passages changed on
    2026-10-02 and 2026-10-08 (R05.T12.a's and R05.T19's findings: the per-planet tables rebuilt
    only when the atmosphere changes, Earth's aerosol mean cosine 0.65 and Rayleigh scale height
    8.43 km, the 32 km reach's sources, the measured per-frame atmosphere). Each agrees with this
    plan's design notes; nothing here contradicts the brainstorm.
  - _Pending re-validation:_ none. Every task's inputs are in the code, in this plan or in a
    fixture; generated bodies' atmospheres wait on P14.T24.a–b and R08.T1's asks (Tasks).
  - _Open, raised with "main", each with its lean:_
    - _Decided 2026-10-09 (`decision-r08-design.md` item 1):_ the one-layer array in a compute
      binding. The adapter views each compute texture at the dimension its kernel declares (R08.T0),
      and the two-layer fallback is refused.
    - P14.T35.e's scope (the surface section's base fields beside T24.c–f's) and its wire change,
      written with R09.T0.a's asks: lean, one wire task for every rendering plan that reads the
      section;
    - whether the published benchmark tables (Garcia and Siewert 1985, Natraj et al. 2009 and 2012,
      Kokhanovsky et al. 2010, IPRT Phase A, Loughman et al. 2004) may be committed whole: lean,
      only the values each test asserts, with citations, as R05.T12.d's rule has it. Decided
      2026-10-09 (`decision-r08-licences.md` row 6): the lean is upheld;
    - _Decided 2026-10-09 (`decision-r08-design.md` item 2):_ planetshine through the receiving
      body's air is kept and taken as far as starlight: its transmittance (a three-node disc rule)
      and its sky term on surfaces (T9.b), and the drawn sky through the body's one list of sources
      (T7, T8).
    - _Decided 2026-10-09 (`decision-r08-design.md` item 3):_ R07's two-star cap against the sky's
      four. The sky takes the body's own list (`SKY_SUN_CAP` reads `MAX_BODY_LIGHTS`), with two
      skips of invisible contributions. R07's raise and the set star's yielded slot are deferred
      corrections to R07.
- **Deviations in T2, as built** (2026-10-09).
  - _A sixth label and the giant's note, decided._ T2 asked "main" whether a giant drawn without
    its air carries a note, since its surface section is `not_applicable` and Design note 12's
    five labels gave it none. The ruling, delegated and adopted, is
    `decision-r08-giant-label.md` in the RM4/RM5 orchestration directory. Each note follows the
    section the body's air comes from: the surface section, or a gas envelope's own (a giant's or a
    sub-Neptune's), never a giant's `not_applicable` surface. A kept scene's body with no
    atmosphere set carries `ATMOSPHERE: NOT YET MODELLED` (R07's `TEST GIANT`). Airless bodies,
    points, marks, contacts and stars carry none. A section asked and not yet answered gets a
    sixth label, `ATMOSPHERE: PENDING`, an annunciation in `LIGHTING: PENDING`'s form, which
    R08.T10.a sets. T2's commit applies the ruling's plan text: Design note 12 (six states, the
    envelope sentence, the `PENDING` bullet), T2's list, the Provides sketch, T10's giant
    sentence and three T10.a tests. From the plan-conformance review it adds what the ruling
    implies but did not list: the withheld `envelope` in Design note 12's `NOT RESOLVED` bullet and
    the Provides comment; a fourth T10.a test, the ruling's §4 check that `PENDING` does not flash
    more than three times a second on a flyby; T10.b's tests for the notes' place after the
    block's others and for `TEST GIANT`'s note; and T14.c's rule for `COMPUTING` and
    `APPROXIMATE` (below). Its finding F1, that P14.T35.e gave the `envelope` section to giants
    only, went to R08.T1, which landed first: P14.T35.e now gives one to every gas-envelope body,
    and a sub-Neptune's surface section turns `not_applicable`, a lean open with "main" (Risks,
    "The asks R08.T1 wrote into plan 14"). The labels hold either way.
  - _The module._ `view/atmosphere/labels.ts` holds `AtmosphereLabel`, there rather than in
    `assemble.ts` as the Provides sketch groups it (the task's Files; R08.T10.a imports it), and
    `ATMOSPHERE_STATEMENTS`, each key's string. Two names are added. `ATMOSPHERE_LABELS` gives the
    keys in the order the six stand among themselves: the two `NOT YET MODELLED` notes, then
    `ATMOSPHERE: NOT RESOLVED` after them, as the guide's "Data states" places a `NOT RESOLVED`
    note, then `PENDING`, `COMPUTING` and `APPROXIMATE`. `atmosphereStatements(labels)` gives
    each label's string once in that order, scene-level as `litLabelsOf` is. R08.T10.b appends its
    result after `photorealStatements`' other notes, so that `ATMOSPHERE: NOT RESOLVED` stands
    after every `NOT YET MODELLED` note on the block, `ROTATION`'s and `BODY PHOTOMETRY`'s
    included.
  - _Per body._ The TSDoc states the precedence that T10.a and T14.c build. A body carries at
    most one of `NOT RESOLVED` and `NOT YET MODELLED`, the withheld section first, and no
    `AEROSOLS` note under either. It carries at most one of `COMPUTING` and `APPROXIMATE`:
    `COMPUTING` while a gated bake runs, in `APPROXIMATE`'s place, and `APPROXIMATE` if the bake
    fails, until the body's atmosphere is next computed. T14.c's text now says so, for its tests
    to pin.
  - _The guide's rows._ Six rows, one for each label so that each can be accepted or reverted
    alone. They follow `PHOTOREALISTIC: PREPARING` in the nomenclature list, the three `Label`s
    first and then the three `Annunciation`s, each marked
    `_Draft (plan R08, R08.T2): the owner signs off._`. The `NOT YET MODELLED` and `NOT RESOLVED`
    rows carry the ruling's clauses word for word, and `PENDING` is its row. Beyond Design note
    12, from the reviews:
    - the labels concern lit bodies drawn larger than a point (T10.a requests no section for a
      point-regime body);
    - `NOT YET MODELLED` no longer claims the body has air;
    - both `NOT YET MODELLED` notes stand on their own lines, never composed with another
      `NOT YET MODELLED` note. This is T2's reading, for the owner: the ruling's §4 says only that
      the giant's clause changes no composition, and the guide composes named pairs alone;
    - `AEROSOLS` is never read as "none";
    - `COMPUTING` applies only to a kind whose check has passed, stands in `APPROXIMATE`'s place,
      and gives way to `APPROXIMATE` if the computation fails, until the body's atmosphere is next
      computed (a choice for the owner; Design note 9 does not cover a failed bake);
    - `APPROXIMATE` concerns thick and strongly flattened bodies only, so it does not reach a thin
      atmosphere drawn before R08.T13's check; it names the oblate case before R08.T6.f (Design
      note 17) beside the thick one, as `AtmosphereLabel`'s sketch does, and, since
      decision-backlog-1 ruled the floor, names the check's figure: agreement within 5% in
      radiance wherever in the view a difference could be seen (Design note 10). The clause is
      applied to the guide after the lane now editing it lands (Risks, "The metric's floor").
  - _Sign-off: pending._ The owner signs off the six rows (the roadmap's "Still awaiting", which
    now lists `ATMOSPHERE: PENDING`). The client is built to them meanwhile, and the sign-off is
    recorded here when given.
- **Deviations in T5.a, as built** (2026-10-09; `view/atmosphere/mie.ts`, `mie.test.ts`). Wiscombe
  1980's structure as the task gives it, in double precision. What differs:
  - _Names beyond the sketch:_ `ComplexIndex` is `{ n, k }`, m = n + ik with k ≥ 0 absorbing,
    defined here (T5.b's `refractiveIndex` returns it); `mieTermCount(x)`;
    `logarithmicDerivatives(zRe, zIm, nMax)` and `LogarithmicDerivatives`; `MIE_MIN_SIZE_PARAMETER`
    10⁻⁶ and `MIE_MAX_SIZE_PARAMETER` 20,000, the top of Wiscombe's fitted range (his (50)).
    `mieSphere` throws `RangeError` outside them, for n ≤ 0, k < 0, a non-finite index or |μ| > 1,
    so T5.b's size grids stay inside [10⁻⁶, 20,000]. `MieResult`'s fields are readonly.
  - _Amplitudes:_ `s1` and `s2` interleave (re, im) per cosine, in Wiscombe's and van de Hulst's
    convention (e^(+iωt)); Bohren and Huffman's are their conjugates, and Q, g and |S|² do not
    depend on it. No `qBack`: Q_back = 4 |S₁(180°)|² ÷ x².
  - _Dₙ:_ the task's first option, Lentz's continued fraction at N_stop, evaluated by the modified
    Lentz method (Thompson and Barnett 1986) to 10⁻¹⁵ in place of Wiscombe's two-term stride and
    ε₂ = 10⁻⁸, and recurred downward at every index. MIEV0's small-particle formulas and its upward
    Dₙ are not built. N_stop is (50)'s middle branch at every x, never fewer terms than the others.
  - _ψ₁_ comes from its Taylor series below x = 0.1, where sin x ÷ x − cos x loses digits
    (3 × 10⁻⁶ at x = 10⁻⁵); the Rayleigh test at x = 10⁻⁵ fails without it.
  - _Design note 6's figure:_ BHMIE's start (D = 0 at max(x + 4x^(1/3) + 2, |mx|) + 15 = 1,515) at
    real z = 1,500 is 38% wrong in D₁ and 27% in D₀, not 25% in D₁. The figure swings with z; over
    1,450–1,550 neither is ever better than 5%. The note's point stands; its number is corrected
    here, for the next revision.
  - _Wiscombe's cases:_ MIEV0's cases 5–19 against his own stored answers (`MVTstNew.f`,
    `BLOCK DATA CHEKMI`, the 1996 MIEV distribution, 7 significant figures) rather than miepython's
    6-decimal copies; the science check found no mismatch in the 510 numbers. Q to 10⁻⁶ (absolute
    at values of order one, relative below), g to 10⁻⁶ absolute, and S₁, S₂ from 0° to 180° in 30°
    steps for every case to 10⁻⁶ of each amplitude. Case 15 (m = 1.5 − i, x = 100), which the task
    said to omit, is kept: it agrees to 2.5 × 10⁻⁷, so miepython's relaxation is miepython's. Six
    amplitudes are held to 5 × 10⁻⁶ where the reference falls short: S₂(90°) of cases 5 and 12
    (MIEV0's small-particle formulas); S₂(60°, 120°) of case 6 (MIEV0 sums 2 terms where N_stop
    sums 3); S₁ and S₂(180°) of case 11 (MIEV0 sums 10,088 terms, and the 10,089th, a resonance
    with |aₙ|² + |bₙ|² = 2.1 × 10⁻¹³, moves S(180°) by 4.2 × 10⁻⁶; this code is 4 × 10⁻⁷ from the
    converged series, by the science check's 40-digit evaluation).
  - _Tests beyond the task:_ ⟨cos θ⟩ on the same Gauss–Legendre grid to 10⁻⁹; small spheres against
    the power series of jₙ and yₙ to 10⁻⁹; case 11's backward sum against a second evaluation with
    ψₙ from Dₙ(x) to 10⁻⁹; D₀ = cot z to 10⁻¹¹; BHMIE's start measured over 10% off; `mieTermCount`
    against (50)'s three branches; the input ranges. The Rayleigh bounds are x² (Bohren and Huffman
    1983, §5.1). B&H's g = 0.63314 is miepython's (`test_03_bh_dielectric`); BHMIE prints none.
  - _Timing, provisional:_ x = 10³, m = 1.33, 256 angles, 1.0 ms median at load average 5.5 and
    1.8 ms at 25 (Node 26.10, Ryzen 7 3700X, `schedutil`), against the research's 4.8–6.3 ms. The
    quiet-machine run (load under 1) is pending; Design note 1's 40–800 ms a mode waits on T5.b.
  - _Pending a ruling:_ the MIEV distribution states no licence. The test commits only the values
    it asserts (MIEV0's answers for cases 5–19, B&H's three printed values), with citations, under
    the open benchmark-tables lean above; if that is refused, it falls back to miepython's MIT
    values (Q and g to 6 decimals, case 14's amplitudes).
- **Deviations in T12.a, as built** (2026-10-09; `crates/hyperion-fit/src/atmosphere/`, the
  `atmosphere-reference` command in `cli.rs`, `RunFitError` in `lib.rs`).
  - _Signatures._ `trace_reference` returns `Result<ReferenceRadiances, TraceReferenceError>`: the
    thread pool can fail (`ThreadPool`), the Stokes mode refuses a term it has no matrix for
    (`StokesNeedsRayleigh`, a Cornette–Shanks term), and `samples` is capped at `MAX_SAMPLES` = 2⁵³
    so that every count is an exact `f64` (`TooManySamples`). `RunFitError` gains
    `AtmosphereCase` (exit 1), `AtmosphereReference` (exit 2 for `StokesNeedsRayleigh` and
    `TooManySamples`, bad command lines; 1 otherwise) and `WriteReference { path, source }` (exit
    1). The command's arguments are a clap `Args` struct, `AtmosphereReferenceArgs`, in a tuple
    variant; `--samples` defaults to 100,000 per geometry, aggregate and wavelength, and
    `--threads` resolves as `orbits`'s does. The fit crate's `clippy.toml` gains `LeVeque` in
    `doc-valid-idents` (Chan, Golub and LeVeque 1979, the moments' merge).
  - _The case format (`CASE_FORMAT` 1) is T12.a's reader's_ (`case.rs`'s module documentation), so
    T12.b's "builds the case format" now reads as the client's writer of it. Names are the client's
    camel case and unknown fields are refused, inside the tagged kinds too. Terms are the client's
    `MediumTerm` with every per-channel array one entry per traced wavelength, without `absorber` or
    `spectral`; there may be any number of them, each from data, so the medium names no species.
    Densities: `exponential`, `tent` and `tabulated`, the last linear in relative density between
    nodes and constant beyond the end nodes, a negative height reading as the ground; **T3.a's
    `densityAt` for `tabulated` must follow the same rule**, or the gate counts the difference as
    the client's error. Phases: `rayleigh` (ρ per wavelength), `cornette-shanks` (`asymmetry`),
    `isotropic` (added, for the Stokes test and the benchmarks) and `none`. Suns are spectral
    irradiances normal to the beam at the top. A geometry is an observer (height, latitude), a view
    (zenith, azimuth from north towards east), one direction per sun, an optional `maxDistanceM`
    (a black target, for aerial perspective) and an optional cone; aggregates are a latitude and
    sun directions; an optional `seed` defaults to 0. Cones of 0°–10° are accepted, 0° for the
    analytic tests and point benchmarks; Design note 10's 0.5°–1° is the cases' to choose. At most
    65,536 wavelengths, and fewer than 2³² geometries and aggregates each, which the draws' keys
    pack.
  - _The tabulated phase function is not read yet._ R08.T12.c adds the one its benchmarks need
    (Garcia and Siewert's Legendre series; Kokhanovsky's and IPRT's tables) before it runs, since it
    precedes T12.b; T12.b then reads T5's `PhaseTable` (u = √(θ/π)) through it.
  - _Output (`REFERENCE_FORMAT` 1)._ One `samples`, `seed` and `polarisation` for the whole
    reference, not samples per geometry. Each geometry adds `sunOpticalDepth` per sun and
    wavelength (`null` where the ground hides the sun), the direct beam for Design note 10's
    Beer–Lambert check, and in the Stokes mode Q and U with their errors. Each aggregate gives
    `incidentTop`, `directGround`, `diffuseGround` and `upwellingTop` (the last two with errors):
    the downwelling flux is direct plus diffuse, the plane albedo upwelling over incident. Q and U
    are in the frame e₁ in the view's vertical plane, e₂ = k × e₁ for the light's direction k;
    Natraj et al. 2009 tabulate −Q and −U of these at the same relative azimuth (their Q = Iᵣ − Iₗ).
    `to_json` is `serde_json`'s pretty form, which `prettier --check` would reflow: T12.b formats
    the committed references for it or lists them in `.prettierignore`.
  - _The estimator._ Absorption is a weight (quadrature, so an absorbing-only medium gives
    Beer–Lambert exactly); free paths by delta tracking against per-shell majorants, so T12.d
    changes only the shells and the majorant; the first collision on the view ray is forced;
    next-event estimation to every sun integrates the transmittance by 8-point Gauss–Legendre per
    shell; the ground is Lambertian; Russian roulette below 0.1 of each branch's starting weight
    (an absolute threshold left a 10 H limb's multiple scattering at 8% noise, against 0.3%
    now). Shells split at every profile kink and every scale height, to 36 H.
  - _Reproducibility._ `Draws` is reused from `tasks/displaced_forms/births.rs`, not copied, with
    the case's seed XORed with `ATMOSPHERE_STREAM` so that its streams are its own; a sample's key
    is (wavelength, geometry or aggregate by its own index, sample), so adding a geometry leaves
    the others' draws alone (tested). Blocks of 1,024 samples run through `map_reduce_chunks` with
    chunk 1. `atmosphere_reference_bits_are_pinned` pins two values to the bit: an edit to `Draws`
    (P15's to make), `Gl16Panel`, `bisect` or the Gauss–Legendre tables shows there before it moves
    a committed reference. Moving `Draws` to a shared module is plan 15's change, later.
  - _Tests_ beyond the three named: single scattering in a thin slab (to 10⁻⁴), Rayleigh's degree
    and plane of polarisation, energy conservation over black and white ground, a grazing optical
    depth against a fine quadrature (10⁻¹⁰), two suns adding, case validation, phase normalisation
    and sampling, shell walks, moments, frames and cones, and the command line. The
    Stokes-equals-scalar test holds I to 10⁻¹² relative, not bit for bit: the Stokes mode mixes the
    terms' matrices, which rounds differently from the scalar mixture.
  - _Cost, provisional_ (shared load 8–9 on 16 threads, the dev profile at `opt-level` 2, 4 threads
    under a 400% CPU quota): about 9 µs a path on the τ = 0.5 Rayleigh slab and 107 µs on an
    Earth-like case of three terms (Rayleigh, a Cornette–Shanks aerosol and an ozone tent; about 50
    shells), where the quadrature of each sun's transmittance dominates. A twilight ground view
    (sun at 95°) is noisy, about 4–7% at 2 × 10⁴ samples, since few forced collisions see the sun;
    such radiances lie under Design note 10's 10⁻² L_max, where σ_ref ≤ 1% does not bind, but T12.b
    budgets their samples.
  - _Citations, decided 2026-10-09_ (`decision-r08-vector.md` item 3).
    - Hansen and Travis 1974's matrix is eq. (2.15), with Δ and Δ′ in eq. (2.16), p. 541; their
      δ is the code's ρ.
    - Witt 1977's Henyey–Greenstein inverse CDF is eq. (19). The "eq. 13" cited is the forced
      first collision's.
    - Cornette and Shanks 1992's number is unreadable (every copy is paywalled), so it is cited
      by DOI without one, beside Draine 2003's eq. (5) at α = 1.
    - R08.T12.c's commit corrects `optics.rs` and `case.rs`.
- **The scalar against the vector Rayleigh radiance, measured in R08.T12.a** (2026-10-09; a
  finding, for a decision agent's sign-off through "main"). The case: Natraj, Li and Yung 2009
  (ApJ 691, 1909), τ = 0.5, μ₀ = 0.2, A = 0, ρ = 0, πF₀ = π, diffuse radiance only, as a
  plane-parallel slab (1 km of uniform Rayleigh scatterer on a sphere of radius 10¹² m, a pencil
  detector), at the top (upwelling) and bottom (downwelling) for μ = 0.1, 0.2, 0.52, 0.84 and 1 and
  relative azimuth 0°, 90° and 180°: 26 geometries, 10⁶ samples each, seed 0, 4 threads, 58 s per
  mode at load 8–9. The case's generator is in the lane's handoff for T12.c.
  - The Stokes mode reproduces Natraj's Tables 1–2 (read from the paper by the science check) within
    3σ at all 26 geometries (largest 2.98σ, χ² = 28.1 for 26; independent runs at seed 1 and before
    the review's re-keying gave largest 2.12σ and 1.81σ), σ ≤ 0.17%, and the degree of
    polarisation to about 0.001.
  - Scalar minus vector, on the same paths: −11.8% at the bottom, μ = 0.1, towards the sun's
    azimuth, −10.8% at μ = 0.1 away from it, and −10.4% and −8.7% at μ = 0.2; +10.0% at the top's
    nadir and +10.7% at the bottom's zenith; at 90° in azimuth within 0.4% for μ ≤ 0.2, +2.0% to
    +2.5% at μ = 0.52 and +6.6% to +7.2% at μ = 0.84. An independent scalar adding–doubling by the
    science check gives −11.79% to +10.75%, and the literature has errors "as large as 10%" for pure
    Rayleigh (Lacis et al. 1998, GRL 25, 135; Kotchenova et al. 2006, Appl. Opt. 45, 6762).
  - _Decided 2026-10-09 (`decision-r08-vector.md`)._
    - The error is corrected, not accepted: R08.T14.e–f, with the gates against the vector
      reference from R08.T14.f on (item 2).
    - The Stokes mode gains tabulated block-diagonal matrices (a₁–a₄, b₁, b₂), the Stokes vector
      (I, Q, U, V), a `depolarising` mark and the scalar I of the same paths, in R08.T12.c (item
      1).
    - The equation numbers are settled under "Deviations in T12.a, as built" (item 3).
- **Closed set, for the composition audit (R08.T12.a).** The tracer's medium is open: any number of
  terms, each a density, spectral coefficients and a phase function from data, so it references
  whatever composition the client's optics write. Closed, each small and stated: the phase
  function kinds (Rayleigh, Cornette–Shanks, isotropic, none: no tabulated phase until T12.c, so a
  Mie or aggregate aerosol cannot be referenced yet); the Stokes mode's kinds (Rayleigh, isotropic,
  none; R08.T12.c adds tabulated matrices and the `depolarising` mark, `decision-r08-vector.md`);
  the ground (Lambertian only: no BRDF, ocean or glint, which are R11's); and the shells
  (the sphere until T12.d).
- **Deviations in T0, as built** (2026-10-09; in `view/engine/webgpu/`, `compute.ts`,
  `kernelResources.ts`, `resources.ts`, `drawing.ts` and their tests; `smoke/work.ts` and
  `smoke/page.ts`). R01's seam as the task gives it. What differs:
  - _The type table is all of WebGPU's §6.2.1 list._ Beyond the task's, `texture_1d` and
    `texture_storage_1d` give `1d`, and `texture_depth_multisampled_2d` gives `2d`.
    `texture_external` binds an external texture, not a view, and gives `undefined`, as buffers and
    samplers do. `TextureSpec` has no 1D texture, so a `1d` declaration refuses every texture.
  - _A name with no declared dimension._ A texture given under a name the kernel does not declare,
    or declares as a buffer, a sampler or a type the table does not read, is viewed at its own
    dimension (a cube's storage as the six-layer `2d-array`), unchecked, as before T0. It is refused
    where it was before: `kernelBindGroupEntries` throws for an undeclared name inside the
    dispatch's encoding, so `engine.test.ts`'s "leave out a dispatch whose encoding threw" still
    exercises the timer's rollback, and WebGPU refuses a kind mismatch.
  - _"Before any GPU call."_ `resolveKernelResources` looks up and checks every sampled and storage
    texture before it makes or writes a uniform buffer or makes a view, and `dispatch` calls it
    before it encodes. A mismatch now throws from `dispatch` itself, before any writer is recorded,
    where before the call returned and WebGPU refused the submission. No caller mismatches today
    (`tables.ts`, `hillaire.ts`, the sky's `bake.ts` and the post kernels checked by review). The
    message also names the texture, as "kernel K declares B as D, but T is O", where O is `cube`
    for a cube bound as storage.
  - _`viewDimensionBinds`_ has its citation corrected from "§6.1.4" to §6.2.1 "Texture View
    Creation" (`"2d-array"` asks only that the texture be `2d`, whatever its layer count). Its
    unit test moves with it from `drawing.test.ts` to `resources.test.ts`.
  - _Tests beyond the task:_ `kernelBindings` over the whole table; a refusal leaves no uniform
    buffer, queue write or view; a cube refused where `texture_storage_2d` is declared; a texture
    under a name declared as a uniform keeps its own dimension.
  - _The smoke check._ `checkOneLayerArrays` runs as the group "R08.T0 one-layer arrays in
    compute", which `smoke/page.ts` adds after T9.i's (the task's Files did not name `page.ts`, but
    a check runs only from the page). Each channel is its index ÷ 4, at most 47.75, compared bit for
    bit; the textures are released in a `finally`.
  - _GPU runs._ `just test-render` (SwiftShader, at d627f1bd with T0's diff): both variants exit 0,
    both checks pass, 578 checks in all, none failing, no uncaptured GPU error. A hidden RTX 3080
    run (offscreen, under the GPU lock) passes both checks on `default` and `no-subgroups`, with no
    uncaptured GPU error. That run's one failure, R07.T16.f's spatial stroke contrast, draws on a
    2D canvas with no WebGPU and passes on SwiftShader, so it is not T0's; reported to "main".
- **Deviations in T3.b, as built** (2026-10-09). `rayleigh.ts` and `rayleigh.test.ts`, as
  Provides names them, with additions. `Dispersion` gains `compressibility` (Z at the reference
  state), `dataFile` (the refractiveindex.info file, or `undefined`) and `measuredNm` (the fitted
  span). `Gas` is derived from `GASES`, and each species is one entry in each of `GAS_DISPERSION`,
  `GAS_KING_FACTOR` (`KingFactor { factor, source }`; `kingFactor` reads it) and
  `GAS_MOLAR_MASS_G_PER_MOL` (below, "Species"). The module also exports
  `RAYLEIGH_WAVELENGTH_RANGE_NM` (300–1,000 nm vacuum; outside it a `RangeError`, a unit error),
  `BOLTZMANN_J_PER_K`, `referenceNumberDensityPerM3`,
  `crossSectionFromDispersionM2(dispersion, kingFactorAt, wavelengthNm)` (F_K passed as a function
  of the wavelength, so that it is taken where n is), and the Earth check's `DRY_AIR_DISPERSION`
  and `dryAirKingFactor`. Two science-checker agents checked Design note 4's table against the
  primary sources, and their key figures were recomputed; a third reviewed the result. Main
  adopted every lean below on 2026-10-09. Design note 4 stands as written; these corrections
  supersede it:
  - **CH₄** takes He, Fang, Shoshanim, Brown and Rudich, Atmos. Chem. Phys. 21 (2021) 14927,
    eq. 10: 10⁸(n − 1) = 3,603.09 + 4.403 62 × 10¹⁴ ÷ (1.1741 × 10¹⁰ − ν²), at 288.15 K, fitted
    over 264–671 nm, with F_K = 1. Sneep and Ubachs 2005's eq. 18 is a fit to Hohm 1993's
    polarisabilities. It runs 13% high in n − 1 against Loria 1909, Rollefson and Havens 1940 and
    the static polarisability, and Wilmouth and Sayres 2019 measured 22% below it. He et al.'s
    formula matches Loria to 0.5% over 529–658 nm.
  - **O₂** takes refractiveindex.info's 1.181 494 × 10⁻⁴ + 9.708 931 × 10⁻³ ÷ (75.4 − λ⁻²), at
    20 °C, cited to Křen, Appl. Opt. 50 (2011) 6484, which refits Zhang et al.'s data. Zhang, Lu
    and Wang 2008's own eq. 20, 15,532.45 + 456,402.97 ÷ (50 − λ⁻²), holds only over 740–860 nm.
    The tests hold the formula to Zhang's eq. 20 within 0.1% over 740–860 nm, to Bates's O₂
    (Sneep and Ubachs, eq. 23) within 0.2% over 300–546 nm, and to Peck and Reeder's dry air
    within 0.1% through the mixture. _Open:_ Křen's comment was not read (Optica, closed). Reading
    it would settle the coefficients and the 20 °C state.
  - **N₂** takes Peck and Khanna's own 15 °C form, 6,497.378 + 3,073,864.9 ÷ (144 − λ⁻²) (their
    abstract), from 468 nm. Sneep and Ubachs's eq. 10, 6,498.2 + …, is their 0 °C form scaled as an
    ideal gas and runs 1.5 × 10⁻⁴ higher. Below 468 nm the module takes Bates 1984's ultraviolet
    branch (Sneep and Ubachs, eq. 11), which joins it to 4 × 10⁻⁷.
  - **CO₂** takes Bideau-Mehu et al. 1973 at 0 °C, as Sneep and Ubachs's eq. 13 corrects it.
    Their printed last numerator, 0.121 814 5 × 10⁻⁴, is 10⁴ too small: only 0.121 814 5
    reproduces their Table 2's 13.29, and it is what refractiveindex.info's 0 °C copy carries.
    Their printed prefactor, 1.1427 × 10⁶, is 10³ too large.
  - **King factors.** "Hohm 1993" (Mol. Phys. 78, 929) is the hydrocarbon mean-polarisability
    paper. The anisotropies are in Hohm 1994 (Chem. Phys. 179, 533), which was not read.
    - H₂ takes 1.0312 + 3.09 × 10⁻⁴ λ⁻² (1.0322 at 550 nm). This is 1 + (2/9)(γ ÷ ᾱ)² for the
      v = 0, J = 0 averages of Raj, Hamaguchi and Witek 2018's ab initio α∥ and α⊥ (J. Chem.
      Phys. 148, 104308; data public), fitted over 380–800 nm. With it, the module's H₂ runs
      7.0% above Dalgarno and Williams 1962 (6.9% with the ideal N); Design note 4's "6.5% low"
      is the same gap seen from the other side.
    - H₂O's 1.001 is now cited: Murphy 1977, ρ = (3.0 ± 1.4) × 10⁻⁴ for linear polarisation,
      which gives F_K = 1.0010 ± 0.0005.
    - _Open:_ NH₃ takes F_K = 1, provisional and probably about 1% low. Reading Hohm 1994 or
      Bridge and Buckingham 1966 would settle it.
  - **The real gas.** N_ref = p ÷ (Z k_B T), with Z = p ÷ (ρRT) from the NIST Chemistry
    WebBook's densities at each state. The literature's ideal N leaves σ high by 1 ÷ Z²: 3.1% for
    NH₃, 1.36% for CO₂ and Xe, 0.55% for Kr and at most 0.15% for the rest. Air takes CIPM-2007's
    0.999 592. He et al.'s CH₄ and N₂O take Z = 1, because their n is defined through the ideal N.
    _Open_, each settled by reading the paper:
    - whether Cuthbertson 1914 reduced NH₃ as a real gas or an ideal one (±3% in σ);
    - whether the literature indices behind Börzsönyi et al.'s Sellmeier forms were reduced as a
      real gas (±1.4% in Xe's σ, ±0.6% in Kr's);
    - whether He et al.'s N was the ideal one at their measuring state, about 295 K and 1020 hPa
      (if so, +0.57% in N₂O's σ and +0.18% in CH₄'s).
  - **He's cross-check** is the form Kurucz 1970 (SAO Spec. Rep. 309, §5.8) prints as "from
    Dalgarno (1962)". Mansfield and Peck agree with it to 0.5%. _Open:_ Chan and Dalgarno 1965 was
    not read; reading it would confirm the attribution.
  - **Smaller corrections.**
    - Sneep and Ubachs's 532.2 nm cross-sections are in their Table 2, not Table 3.
    - Peck and Reeder's standard air carries 330 ppm of CO₂ (Bodhaine §1). Bodhaine and R05
      apply the formula as 300 ppm, a 3 × 10⁻⁵ difference in σ. refractiveindex.info's "450 ppm"
      is Ciddor's.
    - Cuthbertson's NH₃ formula is probably in air wavelengths, a 2 × 10⁻⁵ difference in n − 1.
    - The tutorials' set is Riley et al. 2004's, through Bruneton and Neyret 2008, §2.
    - _Machine-readable copies._ H₂O, CH₄ and N₂O have no refractiveindex.info file, so their
      `dataFile` is `undefined`. H₂O's Ciddor coefficients are as NIST's Engineering Metrology
      Toolbox reproduces them; CH₄'s and N₂O's are He et al. 2021's eqs. 10 and 9 as printed.
  - **The brainstorm** ("Atmosphere", and its Sources, which mark these "from memory") names CH₄
    from Sneep and Ubachs 2005, and H₂ and He from Dalgarno's cross-sections. Design note 4 took
    H₂ and He from Peck and Huang 1977 and Mansfield and Peck 1969, keeping Dalgarno's figures as
    cross-checks; the roadmap's "Brainstorm corrections" table records that. T3.b takes CH₄ from
    He et al. 2021. The brainstorm's rule, each gas's measured dispersion and King factor at its
    formula's own state, holds; only the source names change. _Drafted for the owner:_ the
    roadmap's table gains the CH₄ row and the King factors' correction (R08.T3.b), for the owner
    to apply to the brainstorm. This corrects the re-validation's "nothing here contradicts the
    brainstorm".
  - **Design note 4's dense-gas factor** (a finding for the owner; nothing computed uses it). The
    note gives "about 1.06 at Venus's surface". A science check of NIST's CO₂ (Span–Wagner) at
    737 K and 9.2 MPa gives Z = 1.0057 and ρk_BTκ_T = 0.985, about 1.025 with the Lorentz–Lorenz
    local field (medium confidence). It remains a recorded omission, and the figure is the
    owner's to correct.
  - **`BOLTZMANN_J_PER_K`** lives in `rayleigh.ts`, because R08.T3.a imports it from there (main,
    2026-10-09). `lib/system/constants.ts`, which R08.T3.d made for G, is its natural home once
    both tasks land.
  - **As measured** (`rayleigh.test.ts`):
    - Earth gives 4.846, 11.482 and 28.698 × 10⁻⁶ m⁻¹ at 680, 550 and 440 nm. That is 0.04%
      below R05's constants, since β scales with Z.
    - Dry air at 550 nm is 4.506 × 10⁻²⁷ cm², against Bucholtz's 4.51 and Bodhaine's 4.5105.
    - The per-gas mixture matches Peck and Reeder's dry air to 3 × 10⁻⁴ over 380–760 nm.
    - A 92-bar CO₂ column has τ_R = 16.26 at 550 nm and 40.9 at 440 nm.
    - At 532.2 nm, against Sneep and Ubachs's measurements:

      | Gas | Formula | Measured                                                                  |
      | --- | ------- | ------------------------------------------------------------------------- |
      | Ar  | 4.555   | 4.45 ± 0.3                                                                |
      | N₂  | 5.293   | 5.10 ± 0.24                                                               |
      | CO₂ | 13.11   | 12.4 ± 0.8 (formula +6%, 0.9σ)                                            |
      | O₂  | 4.64    | 4.50 ± 0.15                                                               |
      | CH₄ | 11.35   | 12.47 ± 0.23 (an extinction, with CH₄'s absorption; He et al. 2021, §3.4) |

      All in 10⁻²⁷ cm². Ar's, N₂'s and CO₂'s formulas are pinned to their own values, Sneep and
      Ubachs's n-based figures times Z². Each measurement is a separate check within its 1σ: Ar
      0.35σ, N₂ 0.8σ, CO₂ 0.9σ and O₂ 0.97σ. O₂'s 4.50 comes from their three-component fit with
      collision-induced absorption, not from a pure scattering slope.
  - **Extrapolations** beyond the fitted spans, inside 300–1,000 nm:
    - He below 480 nm, and Ar below 468 nm;
    - Ne, Kr and Xe below 400 nm, the database's validity range;
    - NH₃ outside 480–671 nm;
    - CH₄ above 671 nm, and N₂O outside 307–725 nm;
    - O₂ below 400 nm;
    - CO₂'s and N₂O's King factors outside 457.9–647.1 nm, Alms, Burnham and Flygare 1975's
      lines (J. Chem. Phys. 63, 3321);
    - H₂'s King factor outside 380–800 nm (1.3 × 10⁻⁴ off at 300 nm, by a science check's
      refit).
  - **Species** (the owner's directive of 2026-10-09, relayed by main: the generation handles any
    composition one might expect to exist). `Gas` is plan 14's nine, in `Gas::ALL`'s order, then:
    - Ne, Kr and Xe from Börzsönyi, Heiner, Kalashnikov, Kovács and Osvay, Appl. Opt. 47 (2008)
      4856: Sellmeier forms at 0 °C and 1,000 mbar (their Table 2), with F_K = 1 and Z from the
      WebBook. They measured the phase at 800 nm and joined it to the literature's ultraviolet and
      visible indices; 400–1,000 nm is refractiveindex.info's validity range. The coefficients
      are the database's; the paper was not read. Xe's C₁ takes the database's correction of a
      typo, 12.75 × 10⁻⁶ to 12.75 × 10⁻³ µm². The tests hold them to C. and M. Cuthbertson's
      (1910, 1932): Ne within 0.1% and Xe within 0.3%. Kr is held within 0.5%, since
      Cuthbertson's Kr sits a flat 0.47% low and Koch 1949's (Leonard 1974) agrees with Börzsönyi
      to 0.1%. The same paper's Ar, He and N₂ forms match the module's within 0.3%, which confirms
      its state.
    - N₂O from He et al. 2021, eq. 9 (307–725 nm, ideal N, Z = 1), with Sneep and Ubachs's
      eq. 19 King factor (Alms et al. 1975, 457.9–647.1 nm; 1.225 at 532.2 nm). It matches their
      eq. 20 within 1%.
      Their measured 15.90 ± 0.08 × 10⁻²⁷ cm² at 532.2 nm is 12% below the formula. He et al.'s
      own measurements match the n-based value to −0.6 ± 1.1%, so it is recorded, not tested.

    `GAS_MOLAR_MASS_G_PER_MOL` carries the sim's `Gas::molar_mass_g_per_mol` for the nine (IUPAC
    2021, abridged; a test pins them) and IUPAC 2021's abridged weights for the rest. It is the
    client's one copy, so R08.T3.a's column reads it rather than writing a second.

    _Closed set, for the composition audit:_ these gases have no visible dispersion checked here,
    and wait on the audit:
    - CO: Sneep and Ubachs's eq. 17 fits only 168–288 nm data, and its printed 0.456 × 10¹² is
      10¹⁴ by their Table 2.
    - SO₂, H₂S, HCN and O₃: refractiveindex.info has no gas-phase entry for them.
    - C₂H₆, C₂H₄ and C₂H₂: Loria 1909 dispersions exist (ethane's refitted by
      refractiveindex.info, whose comment says the paper's formula and data disagree), but no
      King factor is sourced.

    Plan 14's wire carries only its nine gases, and R08.T10.a maps them onto `Gas`.

  - **For T3.c.** The mixture's F_mix is the plan's σ-weighted rule. Bodhaine's eq. 23, in
    `dryAirKingFactor`, weights by volume instead; the two differ by about 0.2% at 550 nm. The
    tests' local Σxᵢσᵢ helper is the check to replace with `molecularTerm`. R05's `earth.test.ts`
    keeps its own copy of Peck and Reeder and Bates; R08.T6.d, which rebuilds Earth, may take
    `rayleigh.ts`'s instead.
