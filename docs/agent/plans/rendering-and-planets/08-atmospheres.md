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
before those atmospheres are drawn, and the check runs in `just ci`. The low setting is R05's
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
- Thick atmospheres: the measured regime boundary, the view-dependent converged bake, the
  cloud-deck split, and the path-traced reference with the 5% gate.
- Gas giants inside the 10⁹ m boundary, and the `DiscReflectanceTable` that R07's analytic disc
  reads beyond it for a body with an atmosphere.
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
/** The wavelengths every off-frame bake is solved at, nm: 15 over 380–760 (Design note 5). */
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
  readonly curve: AbsorberCurve | undefined; // an absorber's curve of growth, Design note 5
  readonly spectral: SpectralTerm | undefined; // the same term at BAKE_WAVELENGTHS_NM, for bakes
}
export interface AtmosphereMedium {
  // R05's, widened
  readonly referenceRadiusM: number;
  readonly topAltitudeM: number;
  readonly groundAlbedo: Rgb; // or the deck's, under a split
  readonly terms: ReadonlyArray<MediumTerm>;
  readonly regime: AtmosphereRegime;
}
export type AtmosphereRegime =
  | { readonly kind: "thin" }
  | { readonly kind: "thickScattering"; readonly bakeKey: string }
  | { readonly kind: "cloudDeck"; readonly deckAltitudeM: number; readonly bakeKey: string };

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
export function hydrostaticColumn(input: ColumnInput): AtmosphereColumn; // f64, Design note 3
export const ATMOSPHERE_OPTICS_VERSION: number; // client caches key on it, not the generator
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
export function rayleighCrossSectionM2(gas: Gas, wavelengthNm: number): number;
export function molecularTerm(column: AtmosphereColumn, fractions: GasFractions): MediumTerm;
export interface AbsorberCurve {
  readonly logColumns: Float64Array;
  readonly transmittance: readonly [Float64Array, Float64Array, Float64Array];
}
export function absorberTerm(absorber: AbsorberSpec, column: AtmosphereColumn): MediumTerm;
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
export function aerosolTerm(mode: AerosolModeSpec, column: AtmosphereColumn): MediumTerm;
```

### Assembly, workers and cache (`assemble.ts`, `opticsWorker.ts`, `AtmosphereCache.ts`)

```ts
export function assembleMedium(input: BodyAtmosphereInput): AssembledMedium;
export interface AssembledMedium {
  readonly medium: AtmosphereMedium | undefined;
  readonly labels: ReadonlyArray<AtmosphereLabel>;
}
export type AtmosphereLabel =
  "atmosphereNotResolved" | "aerosolsNotYetModelled" | "atmosphereComputing";
export class AtmosphereCache {
  /* per body and inventory hash; posts work to the optics workers */
}
```

### Tables and passes (`luts/`), widening R05's `HillaireAtmosphere`

- The WGSL compute kernels read a storage buffer of N terms, a 2D array texture of density tables
  and one of phase tables:
  - `transmittance.wgsl`, which stores optical depth, curves of growth included (Design note 8);
  - `multipleScattering.wgsl`, Hillaire's isotropic 32² table for `thin` worlds;
  - `skyView.wgsl`;
  - `aerialPerspective.wgsl`.

  Beside them, `rayMarch.wgsl` is the fragment pass for views from outside and for terrain beyond
  the aerial-perspective reach. Every kernel is registered in R01's `WGSL_CATALOGUE` and created
  through `RenderEngine.createCompute(KernelPair)`, with no subgroup variant.

- `lutsCpu.ts` is a CPU twin of every kernel in `f64`. It is the test oracle and the smoke
  harness's comparison.
- `HillaireAtmosphere` is R05's class, widened:
  - `setMedium(medium)` builds the per-planet tables, shared by every view on the device;
  - `drawFrame(view, suns: ReadonlyArray<SunState>)` builds the per-view, per-sun tables;
  - `aerialPerspective(view)` exposes the volume and the transmittance table to R11.
- `TABLE_SIZES: Record<QualitySetting, TableSizes>` is R05's, kept. The thick table's size is
  added.

### Thick atmospheres (`thick/`)

```ts
export const THICK_MS_THRESHOLD: number; // measured in R08.T13, Design note 9
export const CLOUD_DECK_SPLIT_OPTICAL_DEPTH = 10; // the brainstorm's, open question 3
export const BAKE_CEILING_S = 5; // per world on the UHD 620 host, Design note 9
export function classifyRegime(medium: AtmosphereMedium): AtmosphereRegime;
/** J_ms(h, μ₀, μ_v, m), m = 0..1: the source function of orders two and higher, spectral in, channels out. */
export function bakeMultipleScattering(medium: AtmosphereMedium): MsSourceTable; // f64, worker
export function bakeDeck(medium: AtmosphereMedium, deckAltitudeM: number): DeckTables; // above: reflecting boundary; below: downwelling radiance
```

### For R07's analytic disc (`discReflectance.ts`)

`bakeDiscReflectance(medium, appearance: BodyAppearance) -> DiscReflectanceTable` is R07's
consumed name. It gives the reflectance by phase angle and disc position per channel, baked
spectrally. R07's `shaders/bodyDisc.wgsl` reads it beyond `GAS_GIANT_FULL_PASS_BOUNDARY_M` for a
body with an atmosphere, in place of its albedo-only shading.

### Offline reference (`crates/hyperion-fit`)

```rust
// crates/hyperion-fit/src/atmosphere/
pub struct AtmosphereCase { /* a spectral medium, as the TypeScript writes it, and its geometry set */ }
pub enum Polarisation { Scalar, Stokes }                 // Stokes for Rayleigh benchmarks only
pub struct ReferenceRadiances { /* per geometry and wavelength: radiance, standard error, samples,
    detector cone; the two flux aggregates */ }
pub fn trace_reference(case: &AtmosphereCase, polarisation: Polarisation, samples: NonZeroU64,
    threads: NonZeroUsize) -> ReferenceRadiances;        // spherical backward Monte Carlo, f64
```

Command: `hyperion-fit atmosphere-reference <case.json> --out <reference.json> [--samples N]
[--threads N] [--stokes]`. Fixtures:
`view/atmosphere/reference/{earth,mars,venus-class,titan-class,giant-deck}.case.json` and their
`.reference.json`.

## Consumes

Names are those the owning plans give; the owning plan is authoritative.

- **Galaxy plan 14:**
  - P14.T13's `derive::atmosphere::{Atmosphere, Gas, PartialPressures, SurfaceState}` (built:
    `crates/hyperion-sim/src/planetary/derive/atmosphere.rs`). As built it fills only H₂O, CO₂,
    N₂ and Ar, and gives its Venus 58 bar (Design note 16).
  - These reach the client through P14.T24.a's `SurfaceConditions` in the record's surface
    section. That section is today the empty `record::Surface` and `BodySurfaceDto`
    (`crates/hyperion-protocol/src/planetary/record.rs`), filled by P14.T35 when T13, T14 and T24
    land.
  - The bulk section's radius and surface gravity (built).
  - `DetailLevel::Surface`, and `Section::{NotResolved, NotModelled, NotApplicable}` (built).
  - The four asks of R08.T1: P14.T24.c–f.
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
- **R03:** `sceneAt(model, observer, time) -> SceneFrame`, with each body's `geometricM` and
  `apparentM`, the ship's local body named, and each body's `DetailLevelDto`.
- **R05:** `view/atmosphere/` as its Design note 16 builds it:
  - `MediumTerm`, `DensityProfile`, `EARTH_REFERENCE`, `HillaireAtmosphere` (Bevy 0.19's WGSL
    port, with sebh's reference) and `TABLE_SIZES: Record<QualitySetting, TableSizes>`. This
    includes its low sizes: sky-view 128 × 64, aerial perspective 32 × 32 × 16, the march at
    half resolution, and aerial perspective in one deferred pass on terrain alone.
  - The terrain pass (`view/terrain/terrainPass.ts`).
  - `HeightWorkerPool`'s module-worker and transfer pattern.
  - The spike's `metrics.ts` and `just descent-spike`.
- **R06:**
  - `sky::colour::StarColour`: the per-channel chroma and `lux_per_v0`;
  - `HostDisc`, and the sky layers (`SkySprites`, `BandLayer`, `HostDiscLayer`), which this plan
    dims by transmittance.
  - **Asked of R06:** each colour-table row's spectrum sampled at `BAKE_WAVELENGTHS_NM`, which the
    spectral bakes need (Design note 5).
- **R07:**
  - `Rgb` and `starIlluminance(star, distanceM): Rgb`;
  - `BodyAppearance` (with `BodyPhotometry`), which R08 reads per body;
  - `litRegimes` with `GAS_GIANT_FULL_PASS_BOUNDARY_M`;
  - `shaders/bodyDisc.wgsl`, which consumes `DiscReflectanceTable`;
  - `photorealisticPasses`, with its empty slot for R08, and `viewBudgets`;
  - its Design note 4's cut of a star under 10⁻⁴ of the brightest, which is the same cut here.
- **R11 (consumer):** clouds, oceans and rings are drawn inside this plan's medium, through
  `HillaireAtmosphere.aerialPerspective(view)`.
- **R12:** the consolidated performance runs and the ladder, which audit this plan's low setting.

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
     an absorber carries its curve of growth.
3. **The vertical structure** (researched 2026-09-29, a physics ruling; Robinson and Catling 2012,
   ApJ 757, 104, and 2014, Nature Geoscience 7, 12). The profile is a scaled adiabat from the
   surface to an isothermal skin: T(p) = max(T_s (p/p_s)^β, T_skin).
   - β = α·R/c_p. R/c_p comes from kinetic theory, mixed by mole fraction (R&C 2012 Eqs. 7–9).
   - α scales the dry adiabat for latent heat, by the condensing species: 0.6 with a water ocean,
     0.77 for methane, 0.8 dry or CO₂, and 0.83–0.94 for giants (R&C 2014 Table 1; R&C 2012 §4.1
     for Venus).
   - T_skin = 2^(−1/4)·T_eq is the τ → 0 limit of the same grey Eddington atmosphere as P14.T13.c,
     so the two plans agree at both ends.
   - The research agent's checks on plan-14 figures:
     - Earth: tropopause 0.18 bar at 214 K, against about 0.16–0.23 bar and 217 K observed.
     - Venus: 316 K at 1 bar, against about 350 K.
     - Titan: tropopause 0.24 bar at 64 K, against 0.1–0.2 bar and 70 K.

   Plan 14 publishes the parameters (T_s, p_s, β, T_skin), and for giants its envelope form
   (R08.T1, P14.T24.e), with `planetary::temperature_at(p)` in the sim for the flight model's
   drag. The client copies the formula in `temperatureAt`, tested against golden levels from the
   sim, so the picture and the drag read the same air. The column integrates the hydrostatic
   equation in `f64` from p_s upward, with g = g₀(R/r)², the local kT/(μ m_u g) at each step, and
   a top at p_s × 10⁻⁷.

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
   - **Spectral bakes.** Everything baked off the frame loop is solved at 15 wavelengths over
     380–760 nm and converted to linear Rec. 709 on storage, through the CIE matching functions
     times the star's spectrum. That is Bruneton's "precomputed illuminance" mode, within
     Δu′v′ ≤ 0.001 of the research agent's spectral reference. The bakes are the thick
     multiple-scattering table, the deck tables, `DiscReflectanceTable` and the reference.
     Transmittance multiplies, so it cannot be pre-converted and stays three-channel. At Venus
     depths every three-sample triple is 0.04–0.08 off in u′v′, so the thick bakes cannot skip
     this. It needs each star's spectrum at the bake wavelengths, asked of R06.
   - **Curves of growth.** Absorbers take a per-channel curve of growth, not a band-averaged
     cross-section, which Jensen's inequality biases dark and which fails for saturated narrow
     bands such as methane's on a Neptune. Per absorber and channel,
     T_c(u) = ∫ r̄_c S e^(−σ(λ)u) dλ / ∫ r̄_c S dλ is tabulated on a log grid of column u. The
     transmittance kernel adds −ln T_c(u) of the accumulated column to the stored optical depth.
     The non-additivity over segments that this introduces is recorded by the spectral check.
     Ozone's Chappuis band is in the linear regime, where the curve reduces to the band average.
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
     optool (MIT) as its reference, for fractal dimensions up to 2.5.

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

7. **Per-planet tables are shared; per-view tables are per sun.**
   - Transmittance and multiple scattering depend on the atmosphere alone, as R05's Design note 16
     already corrects the brainstorm. They are built once per planet and shared by every view and
     every sun, through this plan's `AtmosphereCache`, one per device.
   - Sky-view and aerial perspective are per view and per sun.
   - A sun is a star whose illuminance at the body is at least 10⁻⁴ of the brightest's. This is
     R07's Design note 4 cut, one rule for both plans, and it applies above the horizon or not,
     since a night side lit by a companion still has a sky.
   - The high setting draws up to four suns' skies and the low setting two, brightest first.
   - A dropped sun still lights surfaces through R07 and is dimmed by transmittance; only its
     scattered sky is lost. That limit is listed with the setting for R12's audit.
   - Wireframe views draw no atmosphere.
   - Within an atmosphere, R06's sprites, band and host discs are multiplied by the transmittance
     along their direction from the camera.
8. **Transmittance is stored as optical depth.** In `rgba16float` a stored transmittance underflows
   below about e⁻¹⁶·⁶, which is zero along any slant path through a Venus-class atmosphere.
   Stored optical depth has a half-float relative precision of 10⁻³, a transmittance error of
   10⁻³τ: under 1% while T > e⁻¹⁰, and negligible beneath that. The kernels exponentiate on read.
   The absorbers' −ln T_c terms add to it.
9. **Thick atmospheres, by measurement** (researched 2026-09-29; Hillaire 2020 §5.5, §6 and Fig. 12;
   Stamnes et al. 1988; Dahlback and Stamnes 1991; Loughman et al. 2004).
   - **Why Hillaire's term fails.** His isotropic multiple-scattering term is inadequate at Venus
     depths. By his own figures its error grows sevenfold from g = 0 to g = 0.8, he tested only to
     fifty times Earth's Rayleigh density, and near a boundary under τ ≈ 25 of g ≈ 0.7 cloud the
     source function's anisotropy is a 30–70% effect.
   - **Where it fails is measured.** R08.T13 sets `THICK_MS_THRESHOLD` where the analytic term
     first leaves the 5% metric.
   - **The replacement.** Above the threshold the replacement is a view-dependent source-function
     table, J_ms(h, μ₀, μ_v, m) with m = 0 and 1 azimuthal modes. It is about 32 altitudes × 32
     μ₀ × 16 μ_v × 2 × RGB, some 0.26 MB. The sky-view, aerial-perspective and march kernels read
     it in place of σ_s·Ψ_ms·p_u. Whether m = 1 is needed is decided by the gate, not assumed.
   - **The bake** is DISORT-style discrete ordinates in `f64` in a worker. It uses 16 streams,
     about 64 layers, Dahlback and Stamnes's pseudo-spherical beam, delta-M and the Nakajima–Tanaka
     corrections, and solves all μ₀ on one factorisation. It is spectral (Design note 5).
     - It is ported from the papers, never from GPL cdisort, and cross-checked by hand against
       PythonicDISORT (MIT; plane-parallel only).
     - Estimated by operation count at 10⁸ flops, 0.1–1 s. `BAKE_CEILING_S` = 5 s a world on the
       UHD 620's host triggers the fallback of the Risks.
     - Discrete ordinates is chosen because it converges without sampling noise. The Monte Carlo is
       kept for the reference, so that the two stay independent.
     - Bruneton's iterated orders are not the fallback: they diverge in this regime.
   - **The cloud-deck split.** Where a cloud deck's optical depth exceeds 10, the atmosphere
     splits:
     - above the deck, the tables run with the deck as a baked reflecting boundary;
     - below it, a baked plane-parallel table of downwelling radiance by altitude, view angle and
       sun angle gives both sky and aerial perspective.
   - **While a bake runs**, the analytic term is drawn and the label block says
     `ATMOSPHERE: COMPUTING`.
   - **Routing.** Thick regimes route to their bakes only once R08.T14's and R08.T15's gates pass.
     Before then a thick world is drawn with the analytic term under the same label.
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
          sun zenith {0, 30, 60, 80, 90, 95}°;
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
    - The table sizes are R05's `TABLE_SIZES`, taken from Hillaire's code, which the research
      agent's reading of his Table 2 confirms in kind: 32² multiple scattering, 32³ aerial
      perspective over 32 km, and a sky-view of about 200 × 100. R05's low sizes and its deferred,
      terrain-only aerial perspective are this plan's low setting.
    - This plan adds the per-sun cap of Design note 7 and the thick table's size, and keeps both
      settings built together from R08.T6 on.
    - The thick bakes cost CPU at arrival, not frame time, so they run on both settings.
12. **What the view says.** The atmosphere is computed physics, not decoration. Three states are
    labelled, in the guide's existing grammar for a withheld or unmodelled section:
    - `ATMOSPHERE: NOT RESOLVED` when the surface section is withheld. No atmosphere is drawn then,
      since drawing Earth's instead would be invention.
    - `AEROSOLS: NOT YET MODELLED` while plan 14 publishes no inventory.
    - `ATMOSPHERE: COMPUTING` while a thick bake runs.

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

## Tasks

T1 and T2 are documents and can start at once. T1 must land in plan 14 before plan 14 builds
P14.T24. T3–T5 build the optics, in order. T6 generalises R05's tables over N terms and needs T3.
T7 (several suns, the sky's extinction), T8 (other bodies from outside) and T9 (aerial
perspective) follow T6. T10 assembles the medium from a body and needs T3–T6 and T2. T11 records
the thin benchmarks. T12 (the reference) can start with T3 and runs beside everything. T13 needs
T6 and T12, and T14 and T15 follow T13. T16 needs T15 and P14.T24.d. T17 closes.

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

Write four amendment tasks into [galaxy plan 14](../galaxy-generation/14-planetary-systems.md)
beside P14.T24.a, as R04 amends it for the detail seed. The physics below was researched on
2026-09-29 (Design notes 3, 6 and 16, and the research files' sources). The amendment cites it,
and any rule marked "from memory" there is checked against its paper by the agent who builds it.

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

- **P14.T24.e The vertical structure.** The parameters (T_s, p_s, β, T_skin) of Design note 3,
  with α by condensing species, R/c_p from kinetic theory, and `planetary::temperature_at(p)` in
  the sim. Its tests: Earth's tropopause at 0.12–0.25 bar and 205–220 K; Venus at 300–370 K at
  1 bar; Titan's tropopause below 0.3 bar at 60–75 K. It notes R&C 2012's stratospheric upgrade
  for when T24.c lands, and that the thermosphere is the flight model's.
- **P14.T24.f Carbon speciation and O₂.** This is the precondition of T24.c's haze and ozone rules.
  - Carbon on cold bodies (T_s ≲ 150 K) beyond the snow line is CH₄, not CO₂, held at saturation,
    calibrated on Titan's 5.65%.
  - Abiotic O₂ comes from water loss in the runaway state (Luger and Barnes 2015).
  - Biotic O₂ is a gap for the owner.

Files: `docs/agent/plans/galaxy-generation/14-planetary-systems.md`. Acceptance:

- `npx prettier --check docs/agent/plans/galaxy-generation/14-planetary-systems.md` passes;
- the four tasks appear under Phase E with their tests and sources;
- plan 14's Risks name this plan as their consumer, and record its Venus at 58 bar against the
  real 92.

This is a brainstorm-driven plan edit, so it is shown to the owner before it is committed.

### R08.T2 The atmosphere labels, drafted for the owner

Draft the three nomenclature entries of Design note 12 in the form of R02's drafted items, each
with its meaning and when it clears:

- `ATMOSPHERE: NOT RESOLVED`;
- `AEROSOLS: NOT YET MODELLED`;
- `ATMOSPHERE: COMPUTING`.

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
  Tests:
  - an isothermal scale height equals kT/(μ m_u g) to 10⁻⁶;
  - Earth at 288.15 K, 1013.25 hPa, μ = 28.97 gives about 8.4 km, the brainstorm's "near 8 km";
  - the column mass per area is p_s/g to 0.5% for a thin atmosphere, with the spherical excess for
    a thick one;
  - `temperatureAt` reproduces Design note 3's Earth, Venus and Titan checks;
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

Acceptance: `pnpm --filter hyperion exec vitest run view/atmosphere/column view/atmosphere/rayleigh`.

### R08.T4 Channels and absorbers

- **R08.T4.a The fitted triple.** A bless-style vitest (the testkit's `HYPERION_BLESS=1`) refits
  `CHANNEL_WAVELENGTHS_NM` by minimax Δu′v′ over a stated case family: Earth at several suns
  (3,200 K, solar and 9,000 K), and Mars with dust. It writes the triple and the objective's value
  into `channels.json`, and is checked unchanged otherwise. It uses CIE 1931 2° colour-matching
  functions reduced to linear Rec. 709, the primaries R06 uses. Tests:
  - the refit starts from (620, 540, 445) and does not worsen the recorded objective;
  - at Earth the fitted triple beats 680/550/440 at a sun zenith of 85°.
- **R08.T4.b Curves of growth.** `absorbers.ts`: T_c(u) per absorber and channel, weighted by
  r̄_c·S (the star's spectrum from R06), tabulated in `absorbers/curves.json` by the same bless
  step.
  - Sources: ozone from Serdyuchenko et al. 2014 (AMT 7, 625; the articles CC BY 3.0, the data
    page's terms unstated) and methane from Karkoschka and Tomasko 2010 (Icarus 205, 674;
    Elsevier's terms).
  - Only the reduced values are committed, with attribution. The bless step fetches the raw
    tables, and whether a raw table may be committed is asked of the owner.

  Tests:
  - a flat spectrum's curve is e^(−σu);
  - the sun's colour through 0.1–10 × a reference column matches the spectral result to
    Δu′v′ ≤ 0.002;
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

  The research timing (4.8–6.3 ms for x = 10³ at 256 angles) was taken under load and is
  re-measured here on a quiet machine. Acceptance: `pnpm --filter hyperion exec vitest run
view/atmosphere/mie`.

- **R08.T5.b Size distributions and materials.**
  - `sizeDistribution.ts`: Gauss–Legendre in ln r over ±4σ, doubled until the phase integral and
    ⟨cos θ⟩ change by under 10⁻⁴, typically 200–500 nodes.
  - `materials/`: one refractive-index file per material, each with its paper and its CC0 or MIT
    copy in a header: water (Hale and Querry 1973), ice (Warren and Brandt 2008), NH₃ ice
    (Martonchik et al. 1984), CH₄ (Martonchik and Orton 1994), silicates (Dorschner et al. 1995),
    soot, iron.
  - Held until their licences are checked with the owner: H₂SO₄ (Palmer and Williams 1975), Mars
    dust (Wolff et al. 2009) and tholin (Khare et al. 1984).
  - NH₄SH has no visible data and waits on an owner ruling.
  - The non-spherical materials' literature phase functions sit beside their indices.

  Tests:
  - the distribution reproduces r_eff and v_eff to 10⁻⁴;
  - every file covers 380–780 nm;
  - a narrow distribution equals the single sphere;
  - Venus's mode-2 droplets (r_eff 1.05 µm, v_eff 0.07, n ≈ 1.44 at 550 nm; Hansen and Hovenier 1974) give g within that paper's figure.

- **R08.T5.c Aggregates and phase tables.** `aggregate.ts` implements Tazaki and Tanaka 2018's MMF
  with D_f ≤ 2.5. Beside it go the √θ phase tables per channel, and the delta-M truncated series
  with its fraction, for the bakes only (Design note 6). Tests:
  - optool fixtures for a Titan-like aggregate (monomer 0.05 µm, about 3,000 monomers, D_f = 2;
    Tomasko et al. 2008), generated offline and committed, to their stated tolerance;
  - tables integrate to 1 to 10⁻⁶;
  - the truncated series plus its fraction reproduces the full integral and asymmetry.

Acceptance for T5.b and T5.c: `pnpm --filter hyperion exec vitest run view/atmosphere/sizeDistribution
view/atmosphere/aggregate view/atmosphere/materials`.

### R08.T6 Hillaire's tables over N terms

Widen R05's `HillaireAtmosphere` and kernels to the medium of Design note 2 without changing what
they draw for Earth:

- terms travel in a storage buffer, and density and phase tables in 2D array textures;
- transmittance stores optical depth, curves of growth added (Design note 8);
- the `thin` multiple-scattering table keeps Hillaire's isotropic 32².

`lutsCpu.ts` implements each kernel in `f64` at the same sizes. Every kernel is registered in
`WGSL_CATALOGUE`.

Files: `luts/*.wgsl`, `luts/HillaireAtmosphere.ts` (R05's, widened), `lutsCpu.ts`, `medium.ts`.
R05's `EARTH_REFERENCE` is rebuilt from R08.T3's molecular term, R05's aerosol and the ozone curve
of growth; the difference from R05's constants is recorded.

Tests (no GPU):

- R05's constants as a medium give the CPU twin R05's values to 10⁻⁶;
- two identical half-density terms equal one;
- optical depth adds across terms;
- a Venus-class transmittance is finite with no underflow.

Smoke (`just test-render`): every kernel compiles, and the readbacks agree with the twin to 10⁻³
relative above 10⁻⁶ of the table's maximum, on the no-f16 run.

Acceptance: `pnpm test` and `just test-render` pass, and by hand R05's descent scene looks
unchanged at Earth, noted here.

### R08.T7 Several suns, and the sky through the air

Sky-view and aerial perspective per photorealistic view and per sun (Design note 7), in
`drawFrame(view, suns)`:

- suns are ranked by R07's `starIlluminance`, with the 10⁻⁴ cut and the setting's cap;
- a sun below the horizon still feeds the sky through the planet's shadow in the tables;
- R06's sprites, band and host discs are multiplied by the transmittance from the camera along
  their direction.

Tests (no GPU, CPU twin):

- a second equal sun at the same direction doubles the sky radiance;
- the 10⁻⁴ cut and the cap keep the right suns, agreeing with R07's cut on a shared fixture;
- two views with the same camera have equal tables;
- a star seen at the zenith from Earth's surface is dimmed by e^(−τ) of the column.

Acceptance: `pnpm test` and `just test-render` pass, and by hand a binary sky on an Earth fixture
shows both twilights, recorded here with UHD 620 timings.

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

### R08.T9 Aerial perspective on terrain

- On the low setting, R05's deferred aerial perspective applies to terrain alone.
- On the high setting it applies to everything opaque.
- Beyond the volume's 32 km reach (Hillaire 2020 §5.4, confirmed) the march takes over.
- `aerialPerspective(view)` exposes the volume and transmittance to R11.

Tests (no GPU):

- a point at the volume's far edge takes the same in-scattering from volume and march to 2%;
- the low setting touches terrain alone.

Acceptance: `pnpm test` passes, and by hand R05's descent shows fog continuous through 32 km,
recorded here with the pass's time.

### R08.T10 The medium of a body

`assemble.ts`, `opticsWorker.ts`, `AtmosphereCache.ts` build, from `BodyAppearance` and the body's
surface section, in a module worker, cached as Design note 14 says:

- the column;
- the molecular, absorber and aerosol terms;
- the labels of Design note 12.

An airless body (`SurfaceState::Airless`) draws no atmosphere and no label. Until P14.T24.a is on
the wire, generated bodies draw no atmosphere and show the section's own `NOT YET MODELLED`, and
the task runs on the fixtures of Design note 16: Earth, Mars with hand dust, Venus at 92 and at
58 bar, a hand Titan, and a hand giant.

Tests:

- a withheld section gives no medium and `atmosphereNotResolved`;
- an absent inventory gives no aerosol term and `aerosolsNotYetModelled`;
- the same inputs give the same key and a changed inventory another;
- results transfer, and the cache evicts least recently used within its stated size;
- each fixture's optics time is recorded, on a quiet machine.

Acceptance: `pnpm test` passes, and by hand a flight past every fixture is recorded here.

### R08.T11 The thin benchmarks

Record, by hand on the UHD 620 at 720p (low) and on the discrete target when available, against
the budget ([Performance budget](../../brainstorming/rendering-and-planets.md#performance-budget)):

- the per-frame atmosphere time, against 2–4 ms low and 0.5–1 ms discrete;
- the per-planet table time, against about 1 ms and under 0.1 ms;
- the table bytes a planet, against 2 MB.

R05's `just descent-spike` runs with the generalised atmosphere, and its percentiles are recorded
against R05's. Where the low setting misses, `TABLE_SIZES` changes here. Acceptance: the record is
here, and `pnpm test` passes.

### R08.T12 The reference path tracer

- **R08.T12.a The tracer and its benchmarks.** `crates/hyperion-fit/src/atmosphere/` holds a
  spherical backward Monte Carlo in `f64`. It reads a spectral case (`serde_json`, a workspace
  dependency, added to the crate) and has:
  - next-event estimation to each sun, and Russian roulette;
  - an explicit detector cone;
  - an optional Stokes mode for Rayleigh;
  - reproducible results for any thread count.

  Tests:
  - an absorbing-only medium gives Beer–Lambert to 10⁻⁹;
  - one and four threads give identical results.

  Slow tests (`just test-slow`) run the benchmarks of Design note 10:
  - Garcia and Siewert's Haze L and Cloud C1, Natraj et al.'s Rayleigh tables (Stokes),
    Kokhanovsky et al. 2010 and IPRT Phase A, each to 3σ with σ ≤ 0.3%;
  - Loughman et al. 2004 within its 2–4% spread.

  The scalar-against-vector Rayleigh difference is recorded as a finding for the owner.
  Acceptance: `cargo test -p hyperion-fit atmosphere` and `just test-slow`.

- **R08.T12.b Cases and references.** The case format and a bless-style vitest that writes the
  cases of Design note 16 from the client's optics. The command `hyperion-fit atmosphere-reference`
  traces them, and the references are committed with sample counts, times and load average.
  - A vitest asserts that every case equals what the optics produce now, and names the commands to
    regenerate both files.
  - The metric of Design note 10 is written once as `gate.ts`, with its own tests on synthetic
    inputs (the floor, the 3σ term, the cone average).
  - Sanity tests on the references:
    - the Venus-class case's surface downward flux is 2–4% of the top's at the LSFR's sun, with
      the surface sky red-shifted (Tomasko et al. 1980, 2.5%);
    - the Titan-class case's is 5–15% (Tomasko et al. 2008, marked to be re-read);
    - Earth's and Mars's aggregates are recorded.

  Acceptance: `pnpm test` passes and `just fit-check` is unaffected.

### R08.T13 Where the analytic term drifts

A vitest applies `gate.ts` to the CPU twin's thin tables with the analytic term on every case. A
slow `hyperion-fit` sweep traces single-layer media over scattering optical depth,
single-scattering albedo and asymmetry, and sets `THICK_MS_THRESHOLD` where the analytic term first
fails the metric. `classifyRegime` uses that threshold and the split at optical depth 10. Tests:

- Earth and Mars classify `thin` and pass the gate, or the finding is recorded and raised for a
  ruling;
- the threshold and the sweep's table are in its doc comment.

Acceptance: `pnpm test` passes, and the sweep is recorded here.

### R08.T14 The view-dependent multiple-scattering bake

`thick/discreteOrdinates.ts` and `thick/bake.ts` bake J_ms(h, μ₀, μ_v, m) per Design note 9: a
spectral solve at `BAKE_WAVELENGTHS_NM`, converted on storage. The kernels gain the thick table's
read path (catalogued). The label shows while the bake runs, and the table is cached per world.
Tests:

- Garcia and Siewert's Haze L and Cloud C1 through the solver, to their published digits at 10⁻³;
- the Venus-class cases (92 and 58 bar) and the Titan-class case, through the CPU twin with the
  baked table, pass `gate.ts` per geometry, with m = 0 alone and m = 0..1 both run and which is
  needed recorded;
- the bake time on the UHD 620's host is under `BAKE_CEILING_S` on a quiet machine, or the Risks'
  fallback is taken and recorded.

A PythonicDISORT plane-parallel cross-check is run once by hand and recorded. When the gates pass,
`classifyRegime` routes `thickScattering` worlds to the bake. Acceptance: `pnpm test` and
`just test-render` pass.

### R08.T15 The cloud-deck split

- **R08.T15.a Above the deck.** The deck as a reflecting boundary, with its bidirectional
  reflectance per channel from the solver over the deck's layers. Tests:
  - a thick non-absorbing deck reflects its incident flux to 1%;
  - the giant-deck case passes the gate.
- **R08.T15.b Below the deck.** The plane-parallel downwelling table by altitude, view angle and
  sun angle gives sky and aerial perspective beneath. Tests:
  - the Venus-class surface passes the gate and the flux sanity test of R08.T12.b;
  - the table's surface flux equals the solver's;
  - `classifyRegime` routes `cloudDeck` worlds once both gates pass.

  By hand, the Venus-class surface and a descent through its deck are recorded here.

Acceptance: `pnpm test`.

### R08.T16 Gas giants and the disc reflectance

- **R08.T16.a Giants.** Once P14.T24.d is on the wire, the giant's medium comes from its envelope,
  drawn inside 10⁹ m through R08.T15's split, with the deck as its surface and the limb by the
  march. Until then the task runs on the hand giant fixture. Tests:
  - the medium has no ground, and its deck lies at the published pressure;
  - the Jupiter fixture's T(1 bar) is 166 K ± 10% with CH₄/H₂ ≈ 2 × 10⁻³;
  - its limb shell of about ten scale heights is resolved at 2.5 × 10⁸ m (the brainstorm's
    figure, under
    [The scales the view spans](../../brainstorming/rendering-and-planets.md#the-scales-the-view-spans)).
- **R08.T16.b `DiscReflectanceTable`.** Baked spectrally from the medium and tables, and read by
  R07's `bodyDisc.wgsl`. Tests:
  - at 10⁹ m the disc's integrated radiance agrees with the full passes' to 5% for the Jupiter and
    Earth fixtures;
  - its geometric albedo is reported beside R07's `BodyPhotometry`.

  By hand, a Jupiter fixture crossing 10⁹ m is recorded here.

Acceptance: `pnpm test` and `just test-render`.

### R08.T17 Verification pass

Run the by-hand scenes on the UHD 620, and on the discrete target when available, on a quiet
machine. Record every figure here and in its doc comment, and run the spectral check. Acceptance:
`just ci`, `just test-slow` and `just test-render` pass, and the records are here.

## Verification

- **Physics, automatic:**
  - Bucholtz's 4.51 × 10⁻²⁷ cm² to 1%;
  - Earth's 4.85, 11.5 and 28.7 × 10⁻⁶ m⁻¹;
  - each gas's reference state;
  - the column's hydrostatic and profile checks;
  - Mie against Wiscombe's cases;
  - the aggregates against optool;
  - every table's CPU twin against the reference on the stated metric (T13–T16), in `just ci`.
- **Reference, slow:** the tracer against Garcia and Siewert, Natraj, Kokhanovsky, IPRT A and
  Loughman, under `just test-slow`.
- **GPU, software:** every kernel compiles and agrees with its twin on SwiftShader under
  `just test-render`, asserting properties, never images.
- **By eye, recorded:**
  - Earth's sky from the ground at noon and sunset, and from orbit through the terminator;
  - the limb from 400 km;
  - Mars's butterscotch day and blue sunset;
  - the Venus-class surface and deck;
  - a Titan-class haze;
  - a Jupiter-class limb across 10⁹ m;
  - a binary star's two twilights;
  - fog through a descent;
  - stars dimmed near the horizon.
- **Spectral error:** the reference at 15 wavelengths against its three-channel run at Earth and
  Mars. Expect about 0.003 in u′v′ by day and up to 0.02 at low sun with the fitted triple (0.036
  with 680/550/440). Bruneton 2017's implementation (BSD-3) is the cross-check at Earth. A visible
  difference goes to the owner.
- **Benchmarks, recorded on a quiet machine:** per-frame and per-planet times on both settings,
  table bytes, optics-worker times, bake times, and the references' trace times.

## Generator version

No change to generated output and no bump. The atmosphere is presentation computed in the client
from figures plan 14 generates. The asks of R08.T1 are plan 14's tasks, with plan 14's bumps.
Client caches key on `ATMOSPHERE_OPTICS_VERSION` instead. The plan reserves nothing in the
generator, and the reference's sampling needs no domain tag.

## Risks and open points

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
  realism ruling wants them, they are later tasks.
- **The reference in `hyperion-fit`** is a new kind of output for that crate. If plan 15's owners
  prefer, it moves to a sibling offline tool, and nothing else changes.
- **Knowledge.** Until a sensors plan builds the body-level overlay, the server grants the detail
  level asked, so `ATMOSPHERE: NOT RESOLVED` is exercised only by tests and by requests that ask for
  less.
