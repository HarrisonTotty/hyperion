# Plan 07: Gas and dust field, extinction

## Header

- **Milestone:** M2.
- **Depends on:** 02 (galaxy model). It also extends what plans 01, 03, 04 and 05 built, which plan
  02 already depends on or which are complete by M2 (the server resolves system IDs through plan 03;
  the protocol and display tasks build on 04 and 05).
- **Brainstorm sections covered:**
  - "Between the stars": the bullets "Dust and gas" and "What dust and gas do".
  - "Fields": the bullet "Dust and gas".
  - "Galaxy parameters": the bullet "Gas".
  - "Supernova remnants: one route, not two": only what the shell window needs from the gas (density
    and pressure at a site, smoothed to the shell's scale; the pressure floor), and the sentence
    that a superbubble "is a hole in the gas field".
  - "Large features": the sentence that a nebula "adds to the dust field along a line of sight" (the
    hook only).
  - "Visualiser": "Dust lanes join the map when the dust field exists."
  - "Decisions": "Dust and gas do what physics says" and "The gas field has three phases and a
    pressure."
  - "Suggested order of attack": step 5.

## Goal

When this plan is done `hyperion-sim` has a gas and dust field: a pure function of seed and position
giving hydrogen density, thermal pressure, phase and dust content anywhere in the root cube, made of
three smooth phases, arm-following lanes, a central molecular disc and a mean-preserving log-normal
lattice noise. On top of it sit a numerical line integral that returns visual extinction, hydrogen
columns and per-band extinction between any two points (the same in both directions), a site
function for the supernova-shell window of plan 09, hooks through which plan 09's superbubbles and
clouds will modify the field, and the local density a sublight hazard model will read. In the app,
the `GALAXY` display gains an extinction layer on the galaxy map (dust lanes, alone or as an overlay
on the density map) and extinction and reddening figures in the local chart's readout.

## Scope and non-goals

In scope:

- The gas parameters of a galaxy, derived from plan 02's parameters plus draws on this plan's own
  streams.
- The smooth field: neutral disc with its hole, central molecular disc, warm ionised layer, hot
  corona, lanes.
- Lattice noise with exact interpolation, the log-normal, evaluation limited to a smoothing scale.
- Closed-form pressure with the corona's floor; phases and their filling factors.
- Dust-to-gas following metallicity; the hydrogen column per magnitude.
- The extinction line integral: symmetric, adaptive, budgeted, with a caller-owned noise cache.
- Cardelli–Clayton–Mathis wavelength dependence and a small band set.
- Hooks: gas modifiers (holes and clouds), the site function, local density for hazards, neutral
  hydrogen column for 21 cm emission.
- Map functions, protocol messages, server handling and client display changes for the two products
  above.

Non-goals:

- Superbubbles, molecular clouds, nebulae and their emission classes. They are plan 09's. This plan
  defines the `GasModifier` type they will produce and integrates it; until plan 09 the source of
  modifiers is empty.
- The shell window itself (Cioffi–McKee–Bertschinger and Tang–Wang branches, β, the turbulent
  floor). Plan 09 owns it and calls `GasField::state`.
- Any sublight hazard model. Only the density API exists here.
- Apparent magnitudes of stars, sensor horizons in play and retarded-time observation (plans 06 and
  12). They consume `Sightline`.
- What a faster-than-light drive does in gas. The brainstorm leaves it to the drive's brainstorm.
- Feeding the hole in the gas disc back into plan 02's potential (see Risks).
- X-ray photoelectric absorption. The hydrogen column it needs is provided; the cross-section is a
  sensor matter.

## Provides

All Rust paths are under `hyperion_sim::galaxy::gas` unless stated.

```rust
// params.rs
pub struct GasParams { /* private; getters below */ }
impl GasParams {
    pub fn from_galaxy(seed: Seed, params: &GalaxyParams) -> GasParams;
    pub fn milky_way_like() -> GasParams;       // pairs with GalaxyParams::milky_way_like()
    pub fn gas_mass(&self) -> SolarMasses;      // from plan 02's GasDiscParams
    pub fn radial_scale(&self) -> LightYears;   // R_g, from plan 02's GasDiscParams
    pub fn hole_scale(&self) -> LightYears;     // R_m
    pub fn neutral_height(&self) -> LightYears; // h_n, from plan 02's GasDiscParams
    pub fn warm_fraction(&self) -> f64;         // f_w
    pub fn warm_height(&self) -> LightYears;    // h_w
    pub fn molecular_disc(&self) -> MolecularDisc; // mass, scale length, height
    pub fn corona_density(&self) -> HydrogenPerCm3;
    pub fn pressure_floor(&self) -> KelvinPerCm3;
    pub fn pressure_height(&self) -> LightYears;
    pub fn sigma_ln(&self) -> f64;
    pub fn lane(&self) -> LaneParams;           // offset, width, fraction
}

// field.rs
pub struct GasField { /* params + normalisations + borrowed arm geometry and metallicity */ }
impl GasField {
    pub fn new(seed: Seed, galaxy_params: &GalaxyParams, fields: &Fields) -> GasField;
    pub fn mean_density(&self, p: &GalacticPosition) -> HydrogenPerCm3;        // no noise
    pub fn density(&self, p: &GalacticPosition, scale: SmoothingScale,
                   cache: &mut NoiseCache) -> HydrogenPerCm3;                   // noise applied
    pub fn density_with(&self, p: &GalacticPosition, mods: &[GasModifier],
                        cache: &mut NoiseCache) -> HydrogenPerCm3;              // hazard API
    pub fn pressure(&self, p: &GalacticPosition) -> KelvinPerCm3;              // P ÷ k
    pub fn phase(&self, n: HydrogenPerCm3, p_over_k: KelvinPerCm3) -> GasPhase;
    pub fn dust_per_hydrogen(&self, p: &GalacticPosition) -> f64;              // ζ, 1 at [M/H] = 0
    pub fn state(&self, p: &GalacticPosition, scale: SmoothingScale,
                 cache: &mut NoiseCache) -> GasState;       // plan 09's state_at and state_smoothed
    pub fn neutral_bound(&self, cell: &CellBox) -> HydrogenPerCm3; // mean neutral gas, for thinning
}
pub enum SmoothingScale { Full, AtLeast(LightYears) }
pub struct GasState { /* density(), pressure(), temperature(), thermal_sound_speed(), phase() */ }
pub enum GasPhase { Hot, Warm, Cold, Molecular }

// noise.rs
pub struct NoiseCache { /* caller-owned, fixed capacity, direct-mapped */ }
impl NoiseCache { pub fn with_capacity(entries: usize) -> NoiseCache; pub fn clear(&mut self); }
pub fn log_normal_factor(seed: Seed, p: &GalacticPosition, sigma_ln: f64,
                         scale: SmoothingScale, cache: &mut NoiseCache) -> f64;
pub const OCTAVE_WAVELENGTHS_LY: [u32; 5];     // 1_024, 512, 256, 128, 64

// modifiers.rs
pub enum GasModifier {
    Hole  { centre: GalacticPosition, radius: LightYears, interior: HydrogenPerCm3 },
    Cloud { centre: GalacticPosition, core_radius: LightYears,          // Plummer profile
            central_density: HydrogenPerCm3, dust_per_hydrogen: f64 },
}
pub trait GasModifierSource {
    fn modifiers_near_segment(&self, a: &GalacticPosition, b: &GalacticPosition,
                              out: &mut Vec<GasModifier>);
}
pub struct NoModifiers;                        // the only source until plan 09

// ccm.rs
pub fn extinction_ratio(wavelength: Micrometres)
    -> Result<f64, EvaluateExtinctionError>;   // A_λ ÷ A_V at R_V = 3.1; Err below 0.1 µm
pub enum Band { U, B, V, R, I, J, H, K, MidInfrared, Radio }
impl Band { pub fn wavelength(self) -> Option<Micrometres>; pub fn ratio(self) -> f64; }
pub const R_V: f64;                            // 3.1
pub const HYDROGEN_COLUMN_PER_MAG: f64;        // cm⁻² mag⁻¹, Bohlin, Savage and Drake 1978

// extinction.rs
pub enum NoiseMode { Mean, Realised }
pub enum Quality { Full, Budget(NonZeroU32) }  // maximum number of steps
pub struct Sightline { /* a_v, hydrogen_column, neutral_column, steps */ }
impl Sightline {
    pub fn a_v(&self) -> Magnitudes;
    pub fn in_band(&self, band: Band) -> Magnitudes;
    pub fn reddening(&self) -> Magnitudes;                  // E(B − V) = A_B − A_V
    pub fn hydrogen_column(&self) -> PerCm2;                // all gas, corona included
    pub fn neutral_hydrogen_column(&self) -> PerCm2;        // the 21 cm hook
}
pub fn sightline(field: &GasField, a: &GalacticPosition, b: &GalacticPosition,
                 mode: NoiseMode, quality: Quality, mods: &[GasModifier],
                 cache: &mut NoiseCache) -> Sightline;
pub fn horizon(field: &GasField, origin: &GalacticPosition, direction: UnitVector, band: Band,
               limit: Magnitudes, max_range: LightYears, quality: Quality,
               cache: &mut NoiseCache) -> LightYears;

// map.rs, shaped like plan 02's `galaxy::map` so that plan 04's banded map service can drive it
pub fn extinction_face_on(field: &GasField, x: f64, y: f64, pixel_ly: f64) -> Magnitudes;
pub fn extinction_edge_on(field: &GasField, x: f64, z_lo: f64, z_hi: f64) -> Magnitudes;
pub fn render_extinction_rows(field: &GasField, spec: &MapSpec, rows: Range<u32>,
                              out: &mut Vec<f64>);   // plan 02's `MapSpec`; its selection is unused
```

Other provisions:

- `Galaxy::gas(&self) -> &GasField`, added to plan 02's handle.
- `hyperion_sim::units`: `HydrogenPerCm3`, `KelvinPerCm3`, `PerCm2`, `Magnitudes`, `Micrometres`,
  which plan 01 does not define (plan 06 may add `Magnitudes` first; whichever task runs second
  reuses it).
- Domain tags `gas.params` and `gas.noise`, both of scope `Galaxy`, in plan 01's `domain_tags!`
  registry (`rng/tags.rs`, under a "Plan 07" heading). Parameters are drawn on
  `ObjectKey::galaxy()`, lattice values on `ObjectKey::galaxy_item(word)` with the packed word of
  Design note 8.
- Protocol, by plan 04's "Extending the convention" and nothing else: two request kinds,
  `extinction_map` and `extinction`, each a variant of `RequestBody` and of `ResponseBody` with the
  same `kind` string, both strings added to `REQUEST_KINDS`, failures as `request_error`. The bodies
  are `ExtinctionMapRequest` and `ExtinctionMap`, `ExtinctionRequest` and `ExtinctionResult` (with
  `ExtinctionTarget` and `TargetExtinction`). Also a `gas` group in `GalaxyParameters`, and `Unit`
  gains `PerCm3`, `KPerCm3` and `Mag`. No new `ClientMessage` or `ServerMessage` variant, and
  `PROTOCOL_VERSION` does not change.
- `@hyperion/protocol`: `decodeExtinctionMap(map: ExtinctionMap): DecodedExtinctionMap`, sharing the
  decoder of plan 04's `decodeDensityMap`, and the regenerated bindings. Requests go through plan
  04's `RequestClient` unchanged, which derives the response type from the kind; no per-request
  function is added.
- `compute::quantise_map_with_floor(&RawDensityMap, floor_log10, bits)` in the server, which plan
  04's `quantise_map` is rewritten to call with its own view-dependent floor, and
  `compute::ExtinctionMapService` beside `DensityMapService`.
- Test helper `gas_test::ensemble_mean(f, seeds)` in `crates/hyperion-sim/tests/common/mod.rs`,
  which averages a function of the seed with a standard error. Golden files follow plan 01's
  convention: `hyperion_testkit::golden!` and `crates/hyperion-sim/tests/golden/gas/<name>.golden`.

## Consumes

- **Plan 01, determinism foundation:** `math` (`exp`, `ln`, `powf`; `f64::sqrt` is used directly, as
  plan 01 allows), `rng::{Seed, Stream, ObjectKey, TagScope}`, the single `domain_tags!` registry in
  `rng/tags.rs`, `Stream::seek` for a parameter's fixed draw index, and the normal and uniform
  samplers, `units`, `coords::GalacticPosition` (cell plus offset), `to_cylindrical` and `CellSize`,
  the `hyperion-testkit` crate (`golden!`, `order::assert_order_independent`, `stats`), slow-test
  marking, `just bench`, `just bless`, `GENERATOR_VERSION`.
- **Plan 02, galaxy model:**
  - `GalaxyParams` and its `GasDiscParams` (mass as 10–20% of the thin disc's, scale length 1.5–2
    times the thin disc's, height 400 ly; plan 02's D15 says plan 07 owns what they mean), the bar's
    half-length from `BarParams`, the nuclear disc's scale length from `NuclearDiscParams`,
    `GalaxyParams::milky_way_like()`.
  - `Fields::arms() -> &arms::ArmGeometry` and `arms::SharpArm`, the factor 1 + f(R) A (g − 1) of
    plan 02's D10, constructed with this plan's own width σ_w and fraction A. If `SharpArm` can only
    be built from the young disc's parameters, P07.T3 adds a constructor taking σ_w and A.
  - The gas metallicity: the mean of `Component::metallicity(p, age = 0)` for the young thin disc's
    component, which is the metallicity of stars forming now.
  - `PointLy` and `From<&coords::GalacticPosition>`; from `galaxy::map`, `MapView`, `MapSpec` and
    the row-wise shape of `render_rows`, with the edge-on rule of P02.T10.b (integrate across the
    pixel's height, fixed panels along the line of sight); `galaxy::quad::{gl16, gl32, gl_panels}`
    and `tables::gauss_legendre`; `bounds::{CellBox, ScalarRange, UnimodalFactor}`.
  - The `Galaxy` handle, which gains `gas()`.
- **Plan 03, placement and range query:** `placement::resolve(galaxy, SystemId)` and
  `query::position_at(galaxy, record, t)`, used by the server to turn an `ExtinctionRequest` into
  end points.
- **Plan 04, server and protocol:** the request convention (`RequestBody`, `ResponseBody`,
  `REQUEST_KINDS`, `RequestError`, `ErrorCode::BadRequest` with its `field`, the rules of "Extending
  the convention"); `UniverseIdHex`, `SystemIdHex`, the wire `GalacticPosition` and `UniverseTime`;
  `DensityMap`'s geometry fields and wire conventions (design note 12: row order, code 0,
  little-endian 16-bit codes); `GalaxyParameters`, `ParameterGroup`, `Unit`;
  `compute::{CpuPool, DensityMapService, RawDensityMap, quantise_map, SingleFlight}`,
  `cache::{ByteLru, HeapBytes}`, `limits`; `decodeDensityMap`; `TestServer`, `TestClient`,
  `FakeWebSocket`.
- **Plan 05, `GALAXY` display:** `GalaxyMapPanel.tsx`, `GalaxyMapView.tsx`, `DensityLegend.tsx`,
  `lib/galaxy/{ramp.ts, mapGeometry.ts}`, `SystemReadout.tsx`, `useServerRequest` and
  `RequestStatus`, `UnitLabel.tsx` (exhaustive over `Unit`), `parameterLabels.ts`,
  `parameterReadings.ts`, `lib/format.ts` (`formatSci`), the UX guide's raster-field rule. Plan 05
  left `MapPopulation` open for a dust value; this plan does not use that opening (Design note 19).

## Design notes

Each of these is a decision the brainstorm leaves open. Figures marked "MW" are the values for the
Milky Way fixture. Every figure is a starting value that P07.T12 tunes against the brainstorm's
targets; the targets are binding, these are not.

1. **Module placement.** The field lives in `hyperion_sim::galaxy::gas`, beside `fields`, because it
   is a field of the galaxy and shares the arm parameters. It is reached through `Galaxy::gas()`.

2. **Units.** Density is hydrogen nuclei per cubic centimetre, because every source formula and the
   brainstorm use it. Mass density is 1.4 m_H × n_H (helium included). Pressure is carried as P ÷ k
   in K cm⁻³, the unit the measurements are quoted in. Lengths are light-years at this interface.
   One light-year is 9.4607 × 10¹⁷ cm. All are newtypes.

3. **Parameters plan 02 does not draw are drawn here**, on the tag `gas.params` with the galaxy as
   the object and one fixed draw index per parameter, so that nothing in plan 02's output moves and
   a parameter added later appends an index.

   | Parameter                             | Rule                                                 | MW           |
   | ------------------------------------- | ---------------------------------------------------- | ------------ |
   | Gas mass                              | plan 02's `GasDiscParams`, 10–20% of the thin disc's | 5.1 × 10⁹ M☉ |
   | Radial scale R_g                      | plan 02's `GasDiscParams`, 1.5–2 × the thin disc's   | 14,000 ly    |
   | Neutral scale height h_n              | plan 02's `GasDiscParams`                            | 400 ly       |
   | Hole scale R_m                        | 0.8–1.2 × the bar's half-length                      | 16,000 ly    |
   | Warm ionised share of the mass f_w    | 0.20–0.30, uniform                                   | 0.25         |
   | Warm ionised scale height h_w         | 2,500–3,500 ly, uniform                              | 3,000 ly     |
   | Molecular disc share of the mass f_c  | 3–10 × 10⁻⁴, log-uniform                             | 5 × 10⁻⁴     |
   | Molecular disc scale length R_c       | the nuclear disc's scale length                      | 290 ly       |
   | Molecular disc height h_c             | 0.15–0.25 × R_c                                      | 58 ly        |
   | Corona density n_cor                  | 0.5–1.2 × 10⁻³ cm⁻³, log-uniform (note 12)           | 10⁻³         |
   | Pressure floor P_cor ÷ k              | 300–500 K cm⁻³, uniform                              | 400          |
   | Pressure height h_P                   | generator-version constant                           | 1,500 ly     |
   | Pressure speed σ_P                    | generator-version constant                           | 5.5 km/s     |
   | Log-normal width σ_ln                 | 2.0–2.5, uniform                                     | 2.3          |
   | Lane offset d (inward, perpendicular) | 300–600 ly, uniform                                  | 450 ly       |
   | Lane width σ_w                        | 150–300 ly, uniform                                  | 200 ly       |
   | Lane fraction A of the neutral gas    | 0.08–0.20, uniform                                   | 0.12         |

4. **The smooth field.** With R, z cylindrical in light-years:

   ```text
   n_neutral(R, φ, z) = n_0 · exp(−R_m ÷ R − R ÷ R_g) · exp(−|z| ÷ h_n) · lane(R, φ)
   n_warm(R, z)       = w_0 · exp(−R_m ÷ 2R − R ÷ R_g) · exp(−|z| ÷ h_w)
   n_mol(R, z)        = c_0 · exp(−R ÷ R_c − |z| ÷ h_c)
   n_disc             = n_neutral + n_warm + n_mol
   n_mean             = n_disc + n_cor
   ```

   The factor exp(−R_m ÷ R) is the hole inside the bar, in the form McMillan (2017) uses for the
   Milky Way's gas discs; the neutral disc peaks at √(R_m R_g), near the bar's end. The warm layer
   takes half the hole scale because ionised gas fills the inner galaxy more evenly than neutral gas
   does. n_0, w_0 and c_0 follow from the mass budget: the gas mass of plan 02 is shared as (1 − f_w
   − f_c), f_w and f_c, and each normalisation is mass ÷ (1.4 m_H × the component's volume
   integral). The molecular disc's integral is 4π R_c² h_c. The other two have no elementary radial
   integral, so the radial part is a fixed quadrature in ln R, eight `gl32` panels with log-spaced
   edges from 1 ly to 20 R_g through plan 02's `galaxy::quad::gl_panels`, done once in
   `GasField::new`. The corona is outside the budget: it is a halo component, given by its density.
   With MW values this gives 0.70 cm⁻³ of neutral gas and 0.030 cm⁻³ of warm ionised gas in the
   plane at 26,000 ly, 1.1 mag per 3,000 ly there, 0.19 mag to the galactic pole, and 28 mag to the
   centre (13 from the disc, 15 from the molecular disc, 1 from the warm layer), checked with a
   scratch calculation while writing this plan.

5. **The molecular disc's mass is set by the brainstorm's thirty magnitudes, not by the Milky Way's
   central molecular zone.** The real zone holds 3–5 × 10⁷ M☉, which as a smooth disc would put over
   a hundred magnitudes in front of the centre. The real gas is in a few dense clouds that the line
   of sight to the centre mostly misses. So the smooth disc carries only the diffuse part, some 2–3
   × 10⁶ M☉, and the dense clouds are plan 09's features, which add their own dust locally as the
   brainstorm says. See Risks.

6. **Lanes reuse the arm factor at a shifted radius.** lane(R, θ) = S(R + d ÷ cos p, θ), where S is
   plan 02's `SharpArm` factor 1 + f(R) A (g − 1), built on the shared `ArmGeometry` with the lane's
   own width σ_w and fraction A, and p is the pitch angle. Shifting the radius by δR is a phase
   offset of n δR ÷ (R tan p), so this is the brainstorm's "phase offset" with the offset fixed in
   light-years, for the reason the brainstorm gives for arm widths. Because S averages 1 around the
   circle of radius R + δR, the lane factor averages exactly 1 around the circle of radius R, and
   lanes move no mass. A is the share of the neutral gas gathered into lanes; the peak contrast
   follows from it and is about 3 at 26,000 ly for MW values (four arms at 12°), with 1 − A between
   the arms. The arms trail and the gas overtakes them inside corotation, so the lane sits on the
   inner, concave edge: d is positive inward. The warm layer and the molecular disc carry no lanes.

7. **Noise that is mean-preserving at every point, not only on average.** Lattice values are
   standard normal draws keyed by (seed, `gas.noise`; octave and lattice coordinates). Within a
   lattice cell the octave's value is

   ```text
   g_k(x) = Σ_i w_i G_i ÷ √(Σ_i w_i²)
   ```

   with the eight trilinear weights w_i built from the quintic fade s(t) = t³(6t² − 15t + 10).
   Dividing by the root of the summed squares makes g_k exactly standard normal at every point (a
   sum of independent normals), where plain interpolation would lose up to seven eighths of the
   variance at cell centres and break the mean. Σ w_i² factorises per axis as (1 − s)² + s². The
   octaves combine as g = Σ a_k g_k with Σ a_k² = 1, so g is standard normal and the factor

   ```text
   F(x) = exp(σ_ln g(x) − σ_ln² ÷ 2)
   ```

   has expectation exactly 1 at every x. All arithmetic before the final `math::exp` is `+ − × ÷`
   and `sqrt`, which are IEEE-exact, and t is an offset divided by a power of two.

8. **Octaves.** Five, with lattice spacings 1,024, 512, 256, 128 and 64 ly, and a_k² ∝ λ_k^⅔ (more
   variance at large scales, as turbulence has), normalised. Spacings are powers of two so that
   lattice planes are cell faces of the light-year grid and t is exact. Nothing in the field varies
   faster than 64 ly, which is what bounds the integrator's step. The counter word packs octave (4
   bits) and three lattice coordinates (20 bits each, offset to unsigned), so sixteen octaves are
   reserved.

9. **Smoothing to a scale is dropping octaves and their variance.** `SmoothingScale::AtLeast(ℓ)`
   keeps the octaves with λ_k ≥ ℓ and uses σ_eff² = σ_ln² × Σ_kept a_k² in both places of F. That is
   the exact conditional mean of the full field given the coarse octaves, so it is still
   mean-preserving. The research behind the brainstorm drops octaves finer than about 250 ly for a
   field shell; plan 09 passes the shell's own scale.

10. **What the noise multiplies.** n(x) = n_disc(x) × F(x) + n_cor. The corona is an additive,
    un-noised floor: hot gas fills the voids. Dust follows the disc term only, because grains do not
    survive in the corona: the dust-bearing density is n_disc × F × ζ.

11. **Pressure.** P(R, z) ÷ k = P_cor ÷ k + (1.4 m_H σ_P² ÷ k) × n̄_disc(R, 0) × exp(−|z| ÷ h_P),
    where n̄_disc(R, 0) is the azimuthal mean in the plane (so lanes do not modulate it). It is the
    hydrostatic form "pressure is density times a velocity dispersion squared", closed-form, falling
    with height to the corona's floor. σ_P = 5.5 km/s is calibrated so that MW values give the
    measured thermal pressure of 3,800 K cm⁻³ in the plane at 26,000 ly (Jenkins and Tripp 2011, the
    figure the research behind the brainstorm adopted). The noise does not enter the pressure: the
    phases are in rough pressure balance, which is what makes rarefied gas hot.

12. **Phases** are labels derived from density and pressure through the equilibrium temperature T =
    (P ÷ k) ÷ (x n), with x = 2.3 particles per hydrogen nucleus for ionised gas and 1.1 for neutral
    gas: hot above 10⁵ K (n < P ÷ 2.3 × 10⁵ k), warm down to 5,000 K (n < P ÷ 5,500 k), molecular
    above 100 cm⁻³, cold between. With these thresholds a log-normal of σ_ln 2 to 2.5 around 0.75
    cm⁻³ at 3,800 K cm⁻³ puts 18% to 39% of the plane's volume in the hot phase, which is the
    brainstorm's "a fifth to two fifths". The corona itself must come out hot for every seed, which
    needs P_cor ÷ (2.3 k n_cor) above 10⁵ K: that is why the corona's density range stops at 1.2 ×
    10⁻³ cm⁻³ against a floor that can be as low as 300 K cm⁻³. Within the warm phase the neutral
    share is the smooth ratio n_neutral ÷ (n_neutral + n_warm) at that point; hot gas is fully
    ionised; cold and molecular gas are neutral. That rule gives the neutral hydrogen column in
    `Realised` mode. In `Mean` mode there is no local density to classify, so the neutral column is
    the integral of n_neutral + n_mol.

13. **Dust-to-gas is linear in metal abundance.** ζ(x) = 10^[M/H], with [M/H] the gas metallicity
    from plan 02's field at the position, held to at most +0.5 dex. One magnitude of visual
    extinction is 1.87 × 10²¹ × ζ⁻¹ hydrogen nuclei per cm² (5.8 × 10²¹ per magnitude of E(B − V),
    Bohlin, Savage and Drake 1978, with R_V = 3.1), the brainstorm's "about 2 × 10²¹".

14. **The integrator.** The segment is clipped to the slab |z| ≤ 8 h_w, outside which nothing but
    the dust-free corona remains (its hydrogen column is added in closed form). Inside, it is
    marched with two-point Gauss–Legendre steps of length min(Δ_noise, Δ_smooth): Δ_noise = 32 ly ×
    2^lod, and Δ_smooth is half the smallest local smooth scale (h_n or the lane width within 5 h_n
    of the plane, h_c within 3 R_c of the centre, h_w ÷ 4 elsewhere). `Quality::Full` is lod = 0.
    `Quality::Budget(N)` takes the smallest lod whose step count is at most N and evaluates the
    noise at `AtLeast(2 Δ_noise)`, so octaves the steps cannot resolve are replaced by their mean
    and not aliased. `NoiseMode::Mean` skips the noise entirely. The result is a pure function of
    its arguments, quality included, so a caller that needs one answer uses one quality. The server
    fixes one per use.

15. **Two-way visibility is exact.** `sightline` orders its end points canonically (by cell, then
    offset, lexicographically) before marching, so A(a, b) and A(b, a) are the same bits. `horizon`
    marches from the origin and may differ from `sightline` in the last bits; it is documented as an
    instrument range, not an identity.

16. **Holes and clouds.** A `Hole` replaces the field inside its sphere by its interior density with
    no dust; the integrator splits steps at the sphere's entry and exit, found from the
    segment–sphere quadratic. A `Cloud` is a Plummer ball, n_c (1 + r² ÷ a²)^(−5/2), added on top.
    Its column along a segment is algebraic: with b the line's impact parameter, c² = a² + b² and s
    the distance along the line from the point of closest approach, the column is n_c a⁵ × [s (3c² +
    2s²) ÷ (3c⁴ (c² + s²)^(3/2))] between the segment's two values of s. So clouds cost no steps and
    no transcendental function. Where holes overlap, the lowest interior density wins; clouds add.
    `GasField::state` takes no modifiers, because the shell test may read only fields and the star's
    own marks. Plan 09's `features::gas_overlay::FeatureGas` implements `GasModifierSource` and
    yields a `Hole` per superbubble and a `Cloud` per cloud and embedded region; with an empty list
    results are bit-identical. The interface sketch gave plan 09 a Gaussian cloud; the Plummer ball
    replaces it because its column is algebraic, and plan 09 as it now reads uses the Plummer form.

17. **Caching.** The sim holds no caches, so `NoiseCache` is a caller-owned direct-mapped table of
    lattice normals keyed by the packed counter word. Consecutive steps along a line share most of
    their lattice corners, which is where the saving is. The server adds a byte-bounded LRU of
    `Sightline` results keyed by (seed, generator version, end points, quality) through plan 04's
    `ByteLru`, as its design note 23 keys every cache.

18. **The gas field is static.** It is a snapshot at the epoch like the stellar fields, and is not
    advanced with the clock: over ±H the arm pattern turns by about a light-year at 26,000 ly, far
    below the lane width. End points move (systems drift), the medium does not.

19. **The map product is a separate extinction layer.** `ExtinctionMap` is the visual extinction
    through the whole galaxy along each pixel's line of sight, in magnitudes, from the mean field
    (lanes included, noise not, since a pixel is 256 ly and a column through the disc averages the
    noise). Attenuating the density map on the server was rejected: that map counts systems per
    square light-year, and a dimmed count is not a quantity a console could label. The client can
    draw the extinction layer alone or as a labelled overlay that darkens the density ramp, which is
    what puts dark lanes on the arms' inner edges. It is a request kind of its own
    (`extinction_map`) and not a new selector on `DensityMapRequest` or the `MapPopulation` value
    plan 05 left room for, because that response's floor and ceiling fields are named in systems per
    square light-year. The payload is quantised in log₁₀ of magnitudes like the density map, since
    face-on values span 0.01 to tens of magnitudes, by plan 04's quantiser with an explicit floor
    (`quantise_map_with_floor`), since plan 04's own floor is a number of decades below the ceiling
    and this one is a fixed 0.01 mag.

20. **Bands.** `extinction_ratio` takes a wavelength and is the real interface. `Band` is a
    convenience set at the Johnson–Cousins and near-infrared effective wavelengths (U 0.36, B 0.44,
    V 0.55, R 0.66, I 0.81, J 1.25, H 1.65, K 2.2 µm), a mid-infrared point at 10 µm on the extended
    infrared power law, and `Radio`, whose ratio is zero.

## Tasks

Parallelism: T1 → T2 → T3 is one chain. T4 and T7 are independent of it and of each other. T5 needs
T2. T6 needs T2–T5. T8 needs T6 and T7. T9 needs T6. T10 needs T8 and T9, and T10.b needs T11.a,
which has no dependency of its own and is done first so that no unit reaches the wire before the
guide allows it. The rest of T11 needs T10. T12 needs T8 and can run beside T9–T11.

### P07.T1 Gas parameters and units

Build `gas::params`: `GasParams`, `MolecularDisc`, `LaneParams`, `GasParams::from_galaxy` and
`GasParams::milky_way_like`, following Design note 3. The gas mass, R_g and h_n are read from plan
02's `GasDiscParams` and never redrawn. Register the domain tag `gas.params` (scope `Galaxy`) in
plan 01's `rng/tags.rs` under a "Plan 07" heading; each parameter is one uniform at its fixed draw
index, reached with `Stream::seek`. Add the newtypes `HydrogenPerCm3`, `KelvinPerCm3`, `PerCm2`,
`Magnitudes` and `Micrometres` to `units` where missing, with the constants `CM_PER_LIGHT_YEAR` and
`HYDROGEN_MASS_G` (CODATA; cite). Each range in the table is re-checked against the brainstorm's
bullet "Dust and gas" and, for the hole form, McMillan (2017).

- Files: `crates/hyperion-sim/src/galaxy/gas/{mod.rs,params.rs}`,
  `crates/hyperion-sim/src/units.rs`, `crates/hyperion-sim/src/rng/tags.rs`,
  `crates/hyperion-sim/src/galaxy/mod.rs`.
- Tests: every parameter inside its range over 2,000 seeds; the same seed gives the same parameters;
  changing the draw index order is caught by a golden file (`tests/golden/gas/params.golden`, three
  seeds); for every seed the corona alone, n_cor at the pressure floor, is hotter than 10⁵ K (Design
  note 12).
- Acceptance: `cargo test -p hyperion-sim gas::params` passes; plan 02's golden files are unchanged.

### P07.T2 Smooth components and mass normalisation

Build `gas::smooth`: the four components of Design note 4 without lanes, their normalisations by the
fixed quadrature of Design note 4 (plan 02's `quad::gl_panels` and `tables::gauss_legendre`; no new
table), `mean_density`, the azimuthal mean in the plane, and closed-form vertical columns (2 h ×
mid-plane density per exponential layer).

- Files: `crates/hyperion-sim/src/galaxy/gas/smooth.rs`.
- Tests: a brute-force numerical integral of 1.4 m_H × n_disc over the cube returns the gas mass to
  2%, for the MW fixture and 20 seeds. With MW values: the plane at 26,000 ly has 0.6–0.9 cm⁻³ of
  neutral and 0.025–0.035 cm⁻³ of warm ionised gas, and the molecular disc's central density is
  20–80 cm⁻³. For every seed: the neutral density inside R = R_m ÷ 8 is under 1% of its peak (the
  hole), and the corona is n_cor everywhere.
- Acceptance: the tests pass; no `f64` transcendental is called outside `math` (Clippy).

### P07.T3 Lanes from the arm geometry

Add the lane factor of Design note 6. If plan 02's `SharpArm` cannot be built with a caller's width
and fraction, add that constructor in plan 02's arms module, with a test there that the young disc's
factor is bit-identical when built through it.

- Files: `crates/hyperion-sim/src/galaxy/gas/lanes.rs`, plan 02's arms module if needed.
- Tests: the azimuthal mean of the lane factor is 1 to 10⁻⁹ at twenty radii; the lane ridge lies at
  smaller radius than the young-disc arm ridge at the same azimuth, by d ÷ cos p to 5%; the factor
  is 1 well inside the bar's half-length; between the arms it is 1 − A to 1%; at 26,000 ly with MW
  values its peak is 2.5–3.5.
- Acceptance: the tests pass; P07.T2's mass test still passes with lanes on.

### P07.T4 Lattice noise

- **P07.T4.a Lattice values and interpolation.** `gas::noise`: register `gas.noise` (scope
  `Galaxy`); the counter packing of Design note 8 as the word of `ObjectKey::galaxy_item` (rejecting
  coordinates outside the root cube; the lattice planes on the cube's far faces are inside the
  20-bit range), the standard-normal lattice value from plan 01's normal sampler, the quintic fade,
  the variance-normalised trilinear interpolation of Design note 7. Acceptance: at a lattice point
  the value equals the lattice normal exactly; the value is continuous across cell faces (difference
  under 10⁻¹² for points 10⁻⁹ ly either side); over 10⁵ seeds at one fixed interior point the sample
  passes plan 01's Kolmogorov–Smirnov helper against N(0, 1); golden values for six points.
- **P07.T4.b Octaves, the log-normal and the smoothing scale.** `OCTAVE_WAVELENGTHS_LY`, the
  amplitudes, `log_normal_factor`, `SmoothingScale`. Acceptance: Σ a_k² = 1 to 10⁻¹⁵; the ensemble
  mean of F over 10⁶ seeds at a fixed point is 1 within four standard errors, the standard error
  taken from the log-normal's own variance e^(σ²) − 1, for σ_ln of 2.0 and 2.5 and for `Full` and
  `AtLeast(250 ly)`; ln F has variance σ_eff² to 2%. These run under `just test-slow`.
- **P07.T4.c Noise cache and benchmark.** `NoiseCache`, with a test that results are bit-identical
  with a cache of any capacity, including one entry, and after `clear`. Criterion benches: one
  `Full` evaluation cold, and the mean cost per evaluation along a 32 ly-step line with a
  4,096-entry cache. Targets to validate, not promises: 2 µs cold, 0.5 µs on a line. Acceptance:
  `just bench` runs them and the figures are recorded in the module docs.

### P07.T5 Pressure and phases

`gas::pressure` and `gas::phase` by Design notes 11 and 12, with `GasPhase` and the neutral-share
rule. Re-check the 3,800 K cm⁻³ calibration against Jenkins and Tripp (2011). The floor exists for
the brainstorm's 2–4 Myr maximum shell window; here the check is that P ÷ k never falls below
`pressure_floor`, and at R = 26,000 ly is within 1% of it by |z| = 8 h_P, and P07.T12 pins the
window itself.

- Files: `crates/hyperion-sim/src/galaxy/gas/{pressure.rs,phase.rs}`.
- Tests: MW plane pressure at 26,000 ly within 3,400–4,200 K cm⁻³; monotone non-increasing in |z|;
  floor respected over 10⁴ random positions and 100 seeds; phase thresholds ordered for every
  pressure; the analytic hot filling factor at the MW plane is 0.17–0.41 across σ_ln 2.0–2.5 and
  rises with σ_ln; far above the disc (|z| = 20,000 ly) the phase is `Hot` for every seed.
- Acceptance: the tests pass.

### P07.T6 The `GasField` facade, state at a site, bound and hooks

- **P07.T6.a Facade and hooks.** Assemble `GasField` (`new`, `mean_density`, `density`,
  `density_with`, `pressure`, `phase`, `dust_per_hydrogen`, `state`), `GasState` with `temperature`
  (Design note 12) and `thermal_sound_speed` (c² = γ P ÷ ρ, γ = 5/3, ρ = 1.4 m_H n), and
  `gas::modifiers` (`GasModifier`, `GasModifierSource`, `NoModifiers`, the point rule of Design note
  16). `state` with `SmoothingScale::Full` is what plan 09 calls `gas::state_at`, and with
  `AtLeast(ℓ)` what it calls `gas::state_smoothed`. Add `Galaxy::gas()`. The hazard API is
  `density_with`, documented as the quantity a sublight radiation and erosion load is proportional
  to. Bump `GENERATOR_VERSION` here. Tests: `state` with `AtLeast(250 ly)` at 10⁴ positions has the
  same mean as `mean_density` within the statistical tolerance and a smaller log-variance than
  `Full`; the pressure in a `GasState` is never below the floor; a point inside a hole returns the
  interior density and outside it is unchanged; a cloud adds its central density at its centre; ζ is
  1 where [M/H] = 0 and never above 10^0.5; a determinism test (two fields from one seed agree bit
  for bit at 1,000 points; different seeds differ); a golden file (`tests/golden/gas/field.golden`)
  of density, pressure and phase at twelve positions for two seeds.
- **P07.T6.b Cell bound for the neutral gas.** `neutral_bound(cell)`, which plan 09 needs to thin
  clouds and star-forming regions against the dust field. It follows the brainstorm's rule under
  "Exact placement by thinning": the vertical factor exp(−|z| ÷ h_n) never rises with |z| and takes
  its nearest-corner value; the radial factor exp(−R_m ÷ R − R ÷ R_g) is unimodal in R with its peak
  at √(R_m R_g) and is bounded from the cell's range of R through plan 02's `UnimodalFactor`; the
  lane factor is `SharpArm`'s own bound over the cell's phase range, taken at the shifted radius.
  The molecular disc's nearest-corner value is added. The bound is on the mean field; a consumer
  that thins against the noisy field multiplies by its own cap on the log-normal factor. Tests: a
  violation hunt over 10⁶ random points in 10⁴ random 128 ly and 4,096 ly cells, weighted to the
  lanes near the bar's ends and to R near √(R_m R_g), finds none; the bound is within a factor of
  1.5 of the true maximum on cells away from lanes.
- Files: `crates/hyperion-sim/src/galaxy/gas/{field.rs,modifiers.rs,bound.rs}`, plan 02's `Galaxy`.
- Acceptance: `just ci` green; goldens of plans 02 and 03 unchanged apart from the version constant.

### P07.T7 Cardelli–Clayton–Mathis law and bands

`gas::ccm`: `extinction_ratio` as a(x) + b(x) ÷ R_V with x = 1 ÷ λ in µm⁻¹, in the paper's four
ranges: infrared 0.3 ≤ x < 1.1 (a = 0.574 x^1.61, b = −0.527 x^1.61), optical and near-infrared 1.1
≤ x < 3.3 (the two seventh-order polynomials in y = x − 1.82), ultraviolet 3.3 ≤ x ≤ 8 (with the
F_a, F_b terms above 5.9) and far ultraviolet 8 < x ≤ 10. Below x = 0.3 the infrared power law is
continued; above x = 10 the function returns an error variant, not an extrapolation. Coefficients
are copied from Cardelli, Clayton and Mathis (1989), equations 1–5, and re-checked digit by digit.
`Band`, `R_V`, `HYDROGEN_COLUMN_PER_MAG`.

- Files: `crates/hyperion-sim/src/galaxy/gas/ccm.rs`.
- Tests: ratio at V is 1 to 1%; at K (2.2 µm) 0.105–0.125, the brainstorm's "about a ninth"; A_B ÷
  A_V − 1 equals 1 ÷ R_V to 3%; continuity at x = 1.1, 3.3, 5.9 and 8 to 2%; monotone rising with x
  from 0.3 to 4.5; `Radio` is exactly zero; golden values at the ten bands.
- Acceptance: the tests pass. This task has no dependency and can be done first.

### P07.T8 The extinction line integral

- **P07.T8.a Integrator, mean mode, symmetry.** `gas::extinction`: slab clipping, canonical end
  point order, step rule with lod = 0, two-point Gauss–Legendre, `NoiseMode::Mean`, `Sightline` with
  `a_v`, `in_band`, `reddening`, `hydrogen_column`. Work in cell-plus-offset coordinates and form
  each sample position as integer cell plus offset, never as one `f64`. Acceptance: a vertical line
  from the plane to the cube's edge matches the closed-form column to 0.1%; A(a, b) and A(b, a) are
  bit-identical for 10⁴ random pairs; a zero-length segment returns zero; a segment wholly outside
  the slab takes no steps.
- **P07.T8.b Realised mode, quality and cost control.** `NoiseMode::Realised` with the `NoiseCache`;
  `Quality::Budget` with the lod rule and `AtLeast(2 Δ_noise)`. Acceptance: over 256 seeds the mean
  of `Realised` on a fixed 3,000 ly in-plane segment agrees with `Mean` within four standard errors,
  for `Full` and for `Budget(64)`; `Budget(N)` never takes more than N steps; results are identical
  with and without a warm cache.
- **P07.T8.c Modifiers.** Step splitting at hole boundaries and the algebraic Plummer column of
  Design note 16. Acceptance: a line through the centre of a hole of radius r in a uniform test
  field loses exactly the column of 2r; a cloud's column along a long line through its centre is 4 ÷
  3 × core radius × central density to 10⁻⁹, equal in both directions, and matches a brute-force
  numerical integral to 10⁻⁶ on 100 random segments; with an empty modifier list every result is
  bit-identical to the call without modifiers.
- **P07.T8.d Neutral column and horizon.** `neutral_hydrogen_column` by the rule of Design note 12,
  and `horizon`. Acceptance: neutral ≤ total on every line; on in-plane MW lines of 3,000 ly the
  neutral share is 0.8–1.0 in both modes; `horizon` for `Band::V` with a 5 mag limit, looking
  coreward in the MW plane from 26,000 ly in mean mode, is 7,000–13,000 ly; it is longer for `K`
  than for `V` on every tested line, and returns `max_range` for `Radio`.
- **P07.T8.e Benchmarks.** Criterion: the line from 26,000 ly to the centre, `Realised`, `Full`
  (target 3 ms); the same at `Budget(64)` (target 0.2 ms); 2,000 lines of 500 ly from one origin
  with a shared cache (target 100 ms on one core). Acceptance: recorded under `just bench`.

Files for all subtasks: `crates/hyperion-sim/src/galaxy/gas/extinction.rs`,
`crates/hyperion-sim/benches/gas.rs`.

### P07.T9 Extinction map functions

`gas::map`, from the mean field, in the shape of plan 02's `galaxy::map` (P02.T10):
`extinction_face_on`, `extinction_edge_on` and `render_extinction_rows` over plan 02's `MapSpec`
(whose `selection` is ignored), so that the server can split a map into bands of rows exactly as
plan 04's `DensityMapService` does. Face-on is closed form per sample (the sum of 2 h × ζ ×
mid-plane density over the layers, the molecular disc included), averaged over the pixel on a fixed
4 × 4 sub-grid because a lane is narrower than a pixel. Edge-on is the mean-mode integrator along +y
through the whole cube, integrated across the pixel's height by the closed form for each exponential
layer, as P02.T10.b does for the stellar discs, since the neutral disc is thinner than a pixel.

- Files: `crates/hyperion-sim/src/galaxy/gas/map.rs`.
- Tests: the face-on MW value at 26,000 ly, averaged around the circle, is 0.3–0.45 mag (twice the
  polar extinction); the face-on map's lane pixels exceed their inter-arm neighbours at the same
  radius by a factor of at least 1.5; the edge-on central pixel exceeds 25 mag; both views are
  symmetric under z → −z; rows rendered in two bands equal rows rendered in one, bit for bit; a 64 ×
  64 golden map for one seed (`tests/golden/gas/map.golden`).
- Acceptance: the tests pass; a 512 × 512 face-on map computes in under 1 s and an edge-on one in
  under 10 s on one core (bench, a finding if missed).

### P07.T10 Protocol and server

Every message here extends plan 04's request convention exactly as its "Extending the convention"
says: a variant on `RequestBody` and on `ResponseBody` with the same `kind`, the string in
`REQUEST_KINDS` (and the test that pins that list), a wire-form test for each inside its `request`
or `response` envelope, a handler, then `just gen-protocol`. No `ClientMessage` or `ServerMessage`
variant is added, no body carries a request ID, errors are `request_error` with plan 04's
`ErrorCode` and `field`, and `PROTOCOL_VERSION` stays as it is (plan 04's design note 15).

- **P07.T10.a Extinction map.** Kind `extinction_map`.
  `ExtinctionMapRequest { universe: UniverseIdHex, view: MapView, resolution: u16, bits: u8 }`,
  validated as plan 04 validates `DensityMapRequest` (resolutions 128–1,024, 8 or 16 bits).
  `ExtinctionMap` carries `universe`, `view`, `width_px`, `height_px`, `centre_ly`, `ly_per_px` and
  `bits` as `DensityMap` does, then `floor_log10_mag`, `ceiling_log10_mag` and `data_base64`, with
  plan 04's extents, row order and code conventions (its design note 12); code 0 is "at or below the
  floor", the floor is 0.01 mag and the ceiling the grid's maximum. Generalise plan 04's quantiser
  to `quantise_map_with_floor(&RawDensityMap, floor_log10, bits)` and have `quantise_map` call it,
  its tests and fixture unchanged. `compute::ExtinctionMapService`, shaped like `DensityMapService`:
  keyed by (`GalaxyKey`, view, resolution), the raw grid cached in the map cache's byte budget,
  built through `SingleFlight` as bulk jobs of 16 rows calling `render_extinction_rows`, quantised
  per request. `decodeExtinctionMap` in `@hyperion/protocol` shares the decoder of
  `decodeDensityMap` and returns `log10Mag(code)`. Tests: wire forms of request and response; a
  fixture `packages/protocol/fixtures/extinction_map_4x2.json` pinned from both languages; a map
  built with one worker equals one built with four; the second `get` is a cache hit.
- **P07.T10.b Gas parameters** (needs T11.a). A `gas` group in `GalaxyParameters` with every entry
  of Design note 3's table under dotted keys (`gas.mass`, `gas.hole_scale`, `gas.sigma_ln`, ...),
  each marked drawn, derived or fixed, and the `Unit` values `PerCm3`, `KPerCm3` and `Mag`; lengths
  in light-years, the mass in M☉ and σ_P in km/s use plan 04's existing units. Plan 05's
  `UnitLabel.tsx` switches exhaustively over `Unit`, so this task adds its three cases (set as T11.a
  prescribes) and the `gas` labels in `parameterLabels.ts` and `parameterReadings.ts`, or `just ci`
  would fail after `just gen-protocol`. Tests: wire form of the group; `UnitLabel` renders each new
  unit with an accessible name.
- **P07.T10.c Extinction request.** Kind `extinction`. `ExtinctionRequest` carries `universe`,
  `origin: GalacticPosition`, `time: UniverseTime` and `targets: Vec<ExtinctionTarget>`, with
  `ExtinctionTarget` tagged by `type`: `System { id: SystemIdHex }` or
  `Position { position: GalacticPosition }`, at most 64 targets (a position is some 130 bytes of
  JSON and plan 04's inbound frame limit is 16 KiB; the constant joins `limits.rs`).
  `ExtinctionResult { universe, origin, time, targets: Vec<TargetExtinction> }` in request order,
  `TargetExtinction` tagged by `status`:
  `Ok { a_v_mag, e_b_v_mag, a_k_mag, hydrogen_column_per_cm2, neutral_hydrogen_column_per_cm2 }` or
  `NoSuchSystem`. The handler checks the time against ±H, the origin and positions against the root
  cube and the target count (`bad_request` with the `field`); resolves each system target through
  plan 03's `resolve` and `position_at(.., time)`, as plan 04 requires of every ID from a client, a
  failure giving that target `NoSuchSystem` and not failing the request; runs `sightline` in
  `Realised` mode at a fixed `Quality::Budget(256)` with `NoModifiers`, as one interactive pool job
  with one `NoiseCache` per worker; and caches results in a `ByteLru` keyed by (seed, generator
  version, end points, quality). Tests: wire forms; integration tests over the WebSocket with
  `TestServer`: a request from position a to position b and the request from b to a return identical
  figures; an unknown system gives `NoSuchSystem` beside good targets; 65 targets give `bad_request`
  naming `targets`.
- Files: `crates/hyperion-protocol/src/galaxy.rs` and `envelope.rs`,
  `crates/hyperion-server/src/compute/` (`extinction_map.rs`, `density_map.rs`), the handlers and
  `convert.rs` and `limits.rs` of plan 04, `packages/protocol/src/generated/` by
  `just gen-protocol`, `packages/protocol/src/` for `decodeExtinctionMap`, and for T10.b the three
  client files named there.
- Acceptance: `just gen-protocol-check` and `just ci` green after each subtask.

### P07.T11 Client: map layer and chart readout

- **P07.T11.a UX guide** (no dependency; done before T10.b). Edit `docs/frontend/ux-guidelines.md`:
  allow as astronomers' units beside SI the magnitude (`mag`, if plan 06 has not already added it),
  the column density (`cm⁻²`), the number density (`cm⁻³`) and the pressure over Boltzmann's
  constant (`K cm⁻³`), each needing the owner's confirmation as plan 05's additions did; say that
  powers of ten and negative exponents are set with `<sup>` and subscripts such as the V of A_V with
  `<sub>`, using the `−` and `×` glyphs B612 has, never Unicode superscripts, after checking the
  bundled font files for them; allow a second raster quantity on the galaxy map with its own legend,
  and an overlay only when both legends are shown and the overlay is named on the display; add
  `EXTINCTION`, `DUST OVERLAY`, `A_V`, `E(B−V)`, `A_K` and `N_H` to the nomenclature list.
- **P07.T11.b Galaxy map.** A quantity selector on the map (`SYSTEMS`, `EXTINCTION`, and a
  `DUST OVERLAY` toggle available under `SYSTEMS`), keyboard-operable with its key shown, a display
  control and styled as one. The map is requested with `useServerRequest<"extinction_map">`, and
  `mapGeometry` is widened to take the geometry fields the two map responses share. The extinction
  layer uses the same single-hue ramp, logarithmic in magnitudes, with a legend titled
  `EXTINCTION A_V`, the unit `mag`, the words `LOG SCALE` and the stated floor, in the manner of
  plan 05's density legend. The overlay lowers each pixel's log₁₀ density by 0.4 A_V before the ramp
  is applied, clamped to the ramp's floor (a pure function in `lib/galaxy/ramp.ts`), shows both
  legends and the label `DUST OVERLAY: A_V, WHOLE LINE OF SIGHT`. Pending, rejected, timed-out and
  link-down states are plan 05's `RequestStatus`; a map kept on screen after the link drops is
  marked stale as the guide's data states require.
- **P07.T11.c Chart readout.** When a system is selected the chart requests its extinction from the
  chart centre at the chart's time (`useServerRequest<"extinction">` with one `System` target, so a
  newer selection supersedes the older request) and the readout gains four rows: `A_V`, `E(B−V)`,
  `A_K` in `mag` to two decimals, and `N_H` in `cm⁻²` as a mantissa and a power of ten. Until the
  response arrives, and for `NoSuchSystem` or a failed request, the rows show the guide's missing
  state, an em dash, with `RequestStatus` beside them, never zero.
- Files: under `apps/hyperion/src/renderer/src/`: in `displays/galaxy/`, `GalaxyMapPanel.tsx`,
  `GalaxyMapView.tsx`, a new `ExtinctionLegend.tsx` beside `DensityLegend.tsx`, `SystemReadout.tsx`
  and a new `useExtinction.ts` beside `useRangeQuery.ts`; `lib/galaxy/ramp.ts` and `mapGeometry.ts`;
  `test/galaxyFixtures.ts` (`anExtinctionMap`, `anExtinctionResult`); and their tests.
- Tests: component tests with `FakeWebSocket.serverAnswers` for the selector, the legend's unit, the
  overlay label, the four readout rows, the missing state while pending and a changed selection; a
  pure-function test for the overlay rule.
- Acceptance: `pnpm typecheck`, `pnpm lint`, `pnpm test` and `just ci` green; by eye, dark lanes lie
  on the inner edges of the young population's arms in the face-on overlay.

### P07.T12 Milky Way and statistical verification

Slow tests at MW parameters and over seeds, under `just test-slow`
(`crates/hyperion-sim/tests/gas_statistics.rs`). This task also tunes the MW starting values of
Design note 3 until all of them pass, changing parameters and never the targets.

- Mean-mode extinction along in-plane lines of 3,000 ly centred at R = 26,000 ly, averaged over 64
  azimuths: 0.8–1.3 mag, the brainstorm's "about one magnitude per 3,000 ly". The `Realised` average
  over the same lines and 32 seeds agrees with the mean-mode figure within four standard errors.
- From (26,000 ly, 0, 0) to the centre, mean mode: A_V of 24–38 mag and A_K of 2.6–4.5 mag, the
  brainstorm's "some thirty" and "about three". The same line in `Realised` mode over 256 seeds: the
  mean agrees within four standard errors, and the median and the 16th and 84th percentiles are
  recorded in the test's output and in this plan's Risks, because with σ_ln above 2 and a molecular
  disc only a lattice cell or two across, a typical seed's line lies well below the mean. A median
  under 10 mag is a finding for the owner, not a failure.
- The shell window's ceiling, which is what the pressure floor is for. With the closed form plan
  09's P09.T15.a will own, copied into the test until then (W = t_PDS × [¾ (v_PDS ÷ β c_net)^(10⁄7)
  - ¼], t_PDS = 1.33 × 10⁴ yr × n^(−4⁄7), v_PDS = 413 km/s × n^(1⁄7), at 10⁵¹ erg and solar
    metallicity, β = 2, c_net² = γP ÷ ρ + (8 km/s)²; Cioffi, McKee and Bertschinger 1988; re-check),
    the largest W over a fixed scan of n from 10⁻³ to 10 cm⁻³ at P = `pressure_floor` is 2–4 Myr for
    the MW fixture and for every seed of 200, and at the MW plane's pressure the largest W is 0.5–1
    Myr and falls at n of 0.1–0.5 cm⁻³. If the floor's drawn range of 300–500 K cm⁻³ misses the
    band, the range moves, not the band.
- Filling factors by Monte Carlo: over 10⁵ points in the plane at R = 20,000–30,000 ly the hot share
  is 0.20–0.40 for MW σ_ln, and above 0.95 at |z| = 20,000 ly; the molecular share in the plane is
  under 2%.
- Mean preservation in space: the volume average of F over a 16,384 ly cube sampled at 10⁶ points is
  1 within the tolerance implied by its correlated variance (documented in the test).
- Over 200 seeds: gas mass within 2% of plan 02's parameter; every parameter in range; plane
  pressure positive and above the floor.
- Determinism: a golden file (`tests/golden/gas/sightlines.golden`) of ten `Sightline`s for two
  seeds, `Full` and `Budget(64)`; order independence (the ten lines computed in two orders and
  singly, with shared and fresh caches, agree bit for bit).
- Acceptance: `just test-slow` green; the tuned MW values are written back into
  `GasParams::milky_way_like` and this plan's table.

## Verification

- `just ci`, `just test-slow` and `just bench` as above. The brainstorm's tests that fall to this
  plan are covered by P07.T4.b (mean preservation), P07.T12 (one magnitude per 3,000 ly, thirty and
  three magnitudes to the centre, a hot filling factor of a fifth to two fifths, the 2–4 Myr ceiling
  on the shell window that the pressure floor sets, determinism, order independence), P07.T7 (the
  infrared ratio and radio) and P07.T8.a (two-way visibility).
- By eye, in the `GALAXY` display: the extinction layer shows a disc with a hole inside the bar, a
  bright central spot, and lanes that start at the bar's ends and trail; the overlay puts the lanes
  on the inner edges of the young arms; edge-on, the layer is a thin dark band. Selecting systems on
  either side of the plane shows reddening rising towards the plane and the centre.
- A review check that nothing in plans 02 and 03 reads the gas field: stars never read it.

## Generator version

- Adding the field moves no star: all draws are on `gas.params` and `gas.noise`, and no existing
  stream gains a draw. `GENERATOR_VERSION` is still bumped once, in P07.T6, because a universe of
  the earlier version has no gas and readouts would differ. Golden files of earlier plans change
  only where they record the version.
- After that, any change to the parameter rules, the component forms, the octave table, the
  interpolation, σ_P, h_P, the phase thresholds or the integrator's step rule changes output and
  bumps the version. The integrator's rule is output because plan 09's shell test and plan 12's
  magnitudes will read it.
- Reserved: the two domain tags; the request kinds `extinction_map` and `extinction`; draw indices
  0–15 on `gas.params` in the table's order, later parameters appending; octave indices 5–15 in the
  noise counter; the `GasModifier` enum's two variants for plan 09; `SmoothingScale` as the only way
  a consumer selects a scale.

## Risks and open points

- **The corona's density and the pressure floor do not describe one gas.** A corona of 10⁻³ cm⁻³ at
  the 10⁶ K usually quoted has P ÷ k near 2,300 K cm⁻³, not the 300–500 the shell window's 2–4 Myr
  maximum needs. The brainstorm states both figures, so both are kept as independent parameters,
  which implies a corona near 2 × 10⁵ K far from the disc. The brainstorm never states the corona's
  temperature, so nothing it says is contradicted, and of its two figures the one with a stated
  outcome, the 2–4 Myr window borne out by the oldest known remnant, is the one P07.T12 pins. The
  density range is trimmed to 0.5–1.2 × 10⁻³ cm⁻³ so that the corona classifies as hot for every
  seed (Design note 12). A corona whose density falls outward would reconcile them and is a later
  refinement with a version bump. The window test uses plan 09's closed form and its 8 km/s
  turbulent term; at a floor of 400 K cm⁻³ that form gives a ceiling a little under 2 Myr, so
  P07.T12 may move the floor's range down towards 250–400, and plan 09's P09.T15.b then re-pins the
  cap with its production code.
- **The smooth molecular disc is a tenth of the real central molecular zone's mass** (Design note
  5), so that the mean extinction to the centre is the brainstorm's thirty magnitudes, which is the
  outcome the brainstorm states; it gives the central disc no mass. The rest must arrive as plan
  09's clouds. Plan 09's design note 19 places clouds in proportion to this plan's smooth neutral
  density plus a weighted molecular term, because the smooth molecular disc holds 5 × 10⁻⁴ of the
  gas and plain proportion would put only a cloud or two at the centre. Plan 09 solves the weight so
  that its clouds carry nine times `MolecularDisc`'s mass, and P09.T4.c tests it.
- **Thirty magnitudes is the mean, not a typical line.** The noise is the brainstorm's
  (mean-preserving, σ of 2–2.5), and the brainstorm also says the centre lies behind some thirty
  magnitudes. Both hold only on average: half of this plan's 28 mag comes from a molecular disc a
  few lattice cells across, where one log-normal factor with a median a fourteenth of its mean
  decides the realised figure. P07.T12 records the realised distribution. If the owner wants the
  typical seed to read thirty, the remedy is a narrower σ_ln inside the central few hundred
  light-years, which is a change to the brainstorm and not made here.
- **The mean and a typical line differ.** With σ_ln above 2 the median of the factor is a tenth of
  its mean or less, so a single short line is usually well below the mean extinction and
  occasionally far above it. That is the physics of a cloudy medium and the tests are written on
  means, but a console showing 0.002 mag to a star 50 ly away is expected, not a bug.
- **The hole is not in the potential.** Plan 02's potential treats the gas as a plain exponential
  disc. The gas is 15% of the thin disc, so the inner rotation curve is a few per cent high, which
  the research behind the brainstorm already accepted for the stars.
- **Plan 05's reservation is not used.** Plan 05 left `MapPopulation` open for a dust value on the
  density map request. Design note 19 explains why extinction is a request kind of its own. Plan
  05's union stays as it is.
- **Plan 02's interface.** This plan needs a `SharpArm` built with its own width and fraction, and
  reads the gas metallicity as the young disc's mean at age zero. Plan 02's potential keeps its own
  plain gas disc (its D15); the two agree in mass, scale length and height.
- **Band ownership.** Plan 06's `stellar::photometry` works in V and B − V only and defines no band
  type, so `Band` lives here. A later sensor plan may move it; see Design note 20.
- **Cost on long-range charts.** A chart of thousands of bright stars at 5,000 ly is thousands of
  lines of some 150 steps each. The budgeted quality and the pool keep it under a second, but the
  extinction request is capped at 256 targets for that reason, and plan 12 may want a shared radial
  cache.
- **Constant scale heights.** The real neutral layer flares outward. A flare is addable later as a
  height that grows with R; nothing here depends on the height being constant except the closed-form
  face-on column, which stays closed-form.
