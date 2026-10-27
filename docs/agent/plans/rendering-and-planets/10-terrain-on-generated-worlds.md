# Plan R10: Terrain on Generated Worlds

- **Milestone:** Rendering milestone RM5 (with R09).
- **Depends on:**
  [R05 Terrain geometry and the descent spike](05-terrain-geometry-and-descent-spike.md) and
  [R09 The surface generator](09-surface-generator.md). For the photorealistic style's shading it
  reads [R07](07-lit-bodies-styles-and-main-screen.md) (`body_brdf`, the horizon and eclipse terms
  `sphere_irradiance` and `eclipse_visible`, and the lit regimes), and [R08](08-atmospheres.md)'s
  aerial perspective where the body has an atmosphere.
  [R11](11-surface-detail-clouds-oceans-rings-stills.md) builds on this plan's hooks. Through them
  it reads R01–R04 and galaxy plans [12](../galaxy-generation/12-retarded-observation-alerts.md) and
  [14](../galaxy-generation/14-planetary-systems.md).
- **Brainstorm sections covered** (by heading, in
  [the rendering brainstorm](../../brainstorming/rendering-and-planets.md)): step 8, "Terrain", of
  "Suggested order of attack"; "The line between truth and decoration" for the material class and
  its physical properties; "Materials, and what a surface looks like" for the class baked beside the
  height and blended, never re-decided, by the shader; "Level-of-detail consistency, and why
  collision agrees"; "Knowledge, and the surface seed" for the readouts and the survey coverage
  readout; "Two styles of one renderer" and "Two deployments, one scene" for the wireframe's
  depth-only terrain at 4 px with contours; "Performance budget" for the horizon-map shadows, the
  height-texture cache and the terrain rows of the ladder; the items of "Testing" named "Collision
  agrees with what is drawn" and "The descent" as far as real terrain adds to them.

## Goal

When this plan is done, a generated world that the ship has surveyed is drawn from the server's
coarse field and the shared height function, on R05's quadtree, in both render styles. The client
fetches the surveyed coarse-field chunks R09 sends, holds them per body, and hands them to its
height workers, which bake each patch's heights, normals, material class weights, survey mask and
horizon map. The material class is authoritative: `hyperion-surface` decides it at every point, with
the albedo, friction and bearing capacity that landing and the consoles read, and the shader only
blends what the worker baked. Every class is lit by one photometric law, lunar-Lambert, with class
albedos scaled on the server so that the body's disc and its terrain agree. Collision reads one
function, the piecewise-linear ground through the finest level's vertices, and a test shows the
finest mesh agrees with it to the height texture's `f32` step, about 2 mm. Every coarser level stays
inside R09's stated bound on generated worlds, and that bound, not R05's provisional ratio, selects
the levels. Unsurveyed ground stays undrawn as terrain: the body's reference surface stands there,
and the survey edge is marked. Readouts of elevation, slope and slant range quote the ground only as
well as the ship has measured it, `~2140 m ± 180 m` until a close survey resolves the point. The
view carries the survey's coverage and resolution as a readout. The wireframe draws the same terrain
once, depth only, at a 4 px tolerance, with contours from the interpolated height. Terrain shadows
come from horizon maps everywhere, combined with cascades near the camera on the high setting. The
patch cache is sized per setting from measured selections.

## Scope and non-goals

In scope:

- `hyperion-surface`: the material classes, their property table, the bearing capacity and the
  authoritative classifier; the class weights coarser levels bake; the class-albedo scale; the
  ground wrapper collision reads; the surveyed readings with their uncertainty; the horizon-map
  bake.
- R05's bake and worker moved onto R09's height function and extended: class weights, the survey
  mask, horizon maps, the per-body level bounds handed to selection, and the hooks R11 adds to.
- The client's coarse-field store: requesting R09's `surface_field` for the bodies that need it,
  assembling the chunks, keeping the coverage and its resolution, posting to each worker once and
  per chunk, and releasing on departure.
- The photorealistic style's terrain: class shading through R07's `body_brdf`, the survey mask and
  edge, the coarse class map R07's disc samples, and the hand-over from disc to terrain.
- The wireframe style's terrain: depth-only patches at 4 px, contours evaluated in the same pass,
  the interval stated, the grounded-body rule in this style.
- Terrain shadows: horizon maps on both settings, soft by the star's disc, and on the high setting
  cascaded shadow maps near the camera combined with them.
- The view's survey coverage readout, the survey edge, and the readouts of the ground on the view's
  line of sight (`ELEVATION`, `SLOPE`, `SLANT RANGE`, `MATERIAL`, `RESOLUTION`) with coverage- and
  resolution-gated uncertainty.
- The patch cache's sizes, formats and slot layout per setting, and the resident-memory check.
- Draft UX guide entries these need, for the sign-off queue (the owner's delegation).

Non-goals:

- The coarse pass, the per-query height function, `level_bound_m`, `unresolved_rms`, the chunk
  format, the coverage record and the detail seed: R09's. Knowledge gating on the server: R09's, on
  P12.T7's store.
- The quadtree, patch selection by τ, morphing, culling, the patch cache's eviction rule, the worker
  pool and streaming priority: R05's. This plan feeds R05's selection its real bounds and sizes its
  cache; it does not rebuild either.
- GPU decoration, textures of any kind below the band limit, triplanar projection and repetition
  hiding, scatter and authoritative rocks, clouds and cloud shadows, oceans and sea glint: R11's,
  through this plan's hooks (Design note 9).
- Aerial perspective and the atmosphere over terrain: R08's. This plan exposes the terrain pass to
  it.
- A flight model, contact dynamics, landing gear and the sensors that survey on the way down. The
  ground function is what they will call; nothing here calls it but tests. Landing hazard margins
  (3σ and the like) are theirs, multiplying the σ this plan states.
- A server request for readouts on consoles that draw no terrain. No such console exists yet; the
  pure function is the one a later request will wrap (Design note 7).
- The consolidated performance runs: R12's. This plan records its own benchmarks.
- Vegetation and any biosphere class: a later biosphere owes it. Lifeless ground keeps bare classes.

## Provides

Signatures are sketches, named to be grepped. Rust paths are under `hyperion_surface` unless a crate
is named. R09's types (`FieldView`, `PartialField`, `CoarseField`, `Synthesiser`, `SynthCache`,
`BandLevel`, `DetailSeed`, `ResolutionCode`), R05's (`PatchKey`, `Face`, `FaceUv`, `bake_patch`,
`BakeOptions`, `PatchBake`, `finest_surface_height`, `NormalScale`), R09's `height::HeightSample`
(moved there by R09.T4, re-exported by `test_planet`) and R11's `scatter::ExtraLayer` are theirs.
Positions and directions in the surface crate are body-fixed `[f64; 3]` in metres, as R05's are,
since the crate depends on base alone (R11 aliases the type `BodyFixed`); the client holds them as
R05's `BodyFixedVec3` (`view/terrain/planet.ts`, an alias of `geometry/vec3.ts`'s `Vec3`) and, at
R02's boundary, as a `ViewPosition` of kind `body_fixed`. `BodyFixedPosition` and
`BodyFixedVector` are the sim's Rust types (`coords/body_fixed.rs`), not the client's (re-validated
at `c6d77cac`).

### `material`

```rust
/// Authoritative ground classes. Discriminants are wire- and golden-stable `u8`s; a new class
/// appends, and any change to the set, the table or the classifier is a generator-version change.
/// As built by T1.a (Risks, "Deviations in T1.a, as built"): `WaterIce` is `Glacier` and
/// `VolatileFrost` is `Frost`, the forms of the cell's ice entry whatever its species; the codes
/// are unchanged. A `coded_enum!` (`ALL`, `u8::from`, `TryFrom<u8>`).
pub enum MaterialClass {
    Bedrock = 0, Regolith = 1, Soil = 2, Sand = 3, Sediment = 4, VolcanicRock = 5, Melt = 6,
    Glacier = 7, Snow = 8, SeaIce = 9, Frost = 10, OrganicSediment = 11, Seabed = 12,
    Evaporite = 13, Dust = 14,
}
/// The form and the substance: `entry` is the index of R09's header palette entry the ground is
/// made of (`field::MaterialPalette`, R09.T2's follow-up B). `FrostSpecies` is gone: the ice
/// entry names the species (decision-composition §1.8, folded in at the re-validation).
pub struct Material { /* class: MaterialClass, entry: u8 */ }   // new(class, entry) -> Option
/// The codes of R09's `PaletteEntry::phase_row`: 0 no law (`UNASSIGNED_PHASE_ROW`), 1 `Regolith`
/// (R07's `moon` template), 2 `Ice` (`airless-ice`), 3 `Lambert` (`lambert`: f ≡ 1, L ≡ 0), which
/// the form chooses for the consolidated forms (decision-r10-limb); L(α) = L(0) × McEwen 1996's
/// profile. `limb_parameter(a)` is 0 for Lambert and the particulate rule for the others;
/// `normal_albedo_ceiling()` is 1 for Lambert and 1.35 for the others, T10.a's clamp.
pub enum PhaseLaw { Regolith = 1, Ice = 2, Lambert = 3 }
// for_family, template, lommel_seeliger_share, limb_parameter, normal_albedo_ceiling
/// How a form's ground scatters: `Glacier`, `Snow`, `SeaIce` and `Evaporite` are consolidated
/// (the Lambert law), every other form particulate (`MaterialClass::photometric_form`).
pub enum PhotometricForm { Consolidated, Particulate }

pub struct MohrCoulomb { /* cohesion: Pascals, friction_angle: Radians */ }
pub struct ShearLayer { /* top: Metres, bottom: Metres, strength: MohrCoulomb, porosity: f64 */ }
pub enum FrictionModel { Constant(f64), ByTemperature(&'static [(Kelvin, f64)]) }  // tan δ, pad
/// A form's rows for one mechanics family, each figure with its `Citation`.
pub struct FormRow {
    /* shear: &'static [ShearLayer] (by depth, empty where unknown), interface_friction:
       Option<FrictionModel>, compressive_strength: Option<Pascals> (ice), albedo_factor:
       Option<f64> (on the entry's A_N), normal_albedo_v: Option<AlbedoSpan>,
       citations: &'static [Citation] */
}
pub fn form_row(class: MaterialClass, family: MechanicsFamily) -> &'static FormRow;
pub struct MaterialProperties {
    /* the form's `FormRow`, and the entry's resolved values (R09's `PaletteEntry`):
       normal_albedo_bvr: [f64; 3] (A_N in B, V and R, times the form's factor), phase_law:
       PhaseLaw with its PhaseLawOrigin (Palette, FamilyDefault or Form), grain_density, transition:
       Kelvin, mechanics: MechanicsFamily; each layer's bulk density from the grain density */
}
pub fn properties(material: Material, palette: &MaterialPalette)
    -> Result<MaterialProperties, ResolvePropertiesError>;
/// L(0) of a particulate lunar-Lambert law from the normal albedo: the particulate rule L_p, 1 up
/// to 0.60, 0.6696 ÷ x − 0.1160 to 0.38 at 1.35, 0.38 beyond (Design note 8; decision-r10-limb).
pub fn limb_parameter(normal_albedo: f64) -> f64;
// LUNAR_LIMB_ONSET_NORMAL_ALBEDO 0.60, BRIGHT_FROST_NORMAL_ALBEDO 1.35,
// BRIGHT_FROST_LIMB_PARAMETER 0.38, PARTICULATE_NORMAL_ALBEDO_CEILING 1.35
/// The f ≡ 1 case of the energy bound, exact for the Lambert law.
pub fn directional_hemispherical_albedo(normal_albedo: f64, limb: f64, mu0: f64) -> f64;
/// Ultimate bearing capacity under a footing (Design note 2); `None` where the table has no data.
pub fn bearing_capacity(p: &MaterialProperties, gravity: MetresPerSecondSquared,
    footing_width: Metres) -> Option<Pascals>;
pub fn ultimate_bearing_capacity(strength: MohrCoulomb, bulk_density: KilogramsPerCubicMetre,
    gravity: MetresPerSecondSquared, width: Metres) -> Pascals;
pub fn interface_friction(p: &MaterialProperties, temperature: Kelvin) -> Option<f64>;

/// The authoritative class at a point, from the finest level's slope, altitude, R09's climate and
/// surface-state classes, the ice field, and the cell's substance byte and the header's palette.
/// As built by T1.b (Risks, "Deviations in T1.b, as built"): a `GroundMaterial`, the form always
/// and the entry where the palette names one (signoff-9 §17.5's `MATERIAL REGOLITH · —`).
pub fn material_at<F: FieldView>(synth: &Synthesiser<'_, F>, cache: &mut SynthCache,
    at: FaceUv) -> Result<GroundMaterial, QueryHeightError>;
pub struct GroundMaterial { /* class: MaterialClass, entry: Option<u8> */ }  // material() -> Option<Material>
pub const MASSIVE_ICE_SHARE: f64;          // ½: glacier (or airless regolith of ice) or frost (T1.b)
pub const THIN_ATMOSPHERE_PRESSURE: Pascals;  // 30 kPa, the sim's, for `Dust` (T1.b)
pub const CLIFF_SLOPE: Radians;            // 40°, Design note 1
pub const TRANSITION_HALF_WIDTH: Radians;  // 5°, the visual blend band either side
pub const SNOW_SHED_SLOPE: Radians;        // 60°, Schaerer 1977, p. 286 (T1.a, as built)
/// Presentation weights at a point over up to eight materials, each a (form, entry) pair (Design
/// note 3); a patch's palette is T6.a's `TerrainLayers`, gathered from its vertices'. As built by
/// T2 (Risks, "Deviations in T2, as built"): each slot a `GroundMaterial`, the form always and the
/// entry where the palette names one, as `material_at` answers; slots in falling weight, ties by
/// discriminant, the first the largest (at the finest level, strictly, the authoritative class).
/// A texel of band level n is a cell of quadtree level n + 6.
pub struct ClassWeights { /* palette: [Option<GroundMaterial>; 8], weights: [u8; 8] summing to 255 */ }
// palette, weights, iter, weight(material), largest, len, is_empty; MAX_CLASSES 8,
// WEIGHT_TOTAL 255, SLOPE_LAG 5 cm (the omitted structure function's small-lag slope)
pub fn weights_at<F: FieldView>(synth: &Synthesiser<'_, F>, cache: &mut SynthCache,
    at: FaceUv, level: BandLevel) -> Result<ClassWeights, QueryHeightError>;
/// The per-body scale on class albedos that makes the body's geometric albedo p (Design note 8).
pub struct AlbedoScale { /* scale: f64, residual: f64 */ }
pub fn albedo_scale(fractions: &ClassFractions, palette: &MaterialPalette,
    geometric_albedo_bvr: [f64; 3]) -> Result<AlbedoScale, SolveAlbedoScaleError>;
/// The solve on the albedos a ground's exposure gives (R10.T1.c); `albedo_scale` is it at
/// `GroundExposure::NONE`, and the server solves it at `GroundExposure::of_header`.
pub fn exposed_albedo_scale(fractions: &ClassFractions, palette: &MaterialPalette,
    exposure: GroundExposure, geometric_albedo_bvr: [f64; 3])
    -> Result<AlbedoScale, SolveAlbedoScaleError>;
/// What a ground is exposed to: a maturity and an organic cover, each in [0, 1] or absent.
pub struct GroundExposure { /* maturity: Option<f64>, organic_cover: Option<f64> */ }
// NONE, of_header, with_maturity, with_organic_cover, maturity, organic_cover
/// `properties` with the albedo weathered to the exposure's maturity (`Melt` fresh), times the
/// form's factor, and blended with the organic cover where the entry takes one.
pub fn exposed_properties(material: Material, palette: &MaterialPalette,
    exposure: GroundExposure) -> Result<MaterialProperties, ResolvePropertiesError>;
/// Whether an entry takes the cover: an `Ice` formula holding carbon, CO₂ excepted, or `N2` beside one.
pub fn takes_organic_cover(palette: &MaterialPalette, entry: &PaletteEntry) -> bool;
/// The maturity at a point, m_local: m_body less the largest young-crater deficit there.
pub fn maturity_at<F: FieldView>(synth: &Synthesiser<'_, F>, cache: &mut SynthCache,
    at: FaceUv) -> Result<Option<f64>, QueryHeightError>;
/// One crater's maturity at fresh share s: m_body − s max(0, m_body − maturity_after(t_c, k)).
pub fn crater_maturity(body_maturity: f64, crater_age: Gigayears, weathering_rate: f64,
    fresh_share: f64) -> f64;
/// The fresh share's parts: f_ex below the mature layer d_m, and the halo p(x).
pub fn excavated_share(surface_age: Gigayears, crater_age: Gigayears, diameter: Metres) -> f64;
pub fn reworked_depth(exposure: Gigayears) -> Metres;
pub fn halo_share(distance: f64, outer: f64) -> f64;
// HALO_LENGTH_RIM_RADII 3, HALO_EXTENT_RIM_RADII 10, EXCAVATION_DEPTH_RATIO 0.084,
// REWORKING_DEPTH_AT_ONE_MYR 0.022 m, REWORKING_EXPONENT 0.45
/// The inverse halo (the halo follow-up): x* where the fresh share falls to s, by `ln_1p`.
pub fn halo_reach(share: f64, outer: f64) -> f64;
/// A body's maturity mix: nodes in falling maturity from m_body, their weights summing to 1.
pub struct MaturityMix { /* nodes: Vec<MaturityNode>, the deficit law F */ }
// body, nodes, share_above
pub struct MaturityNode { /* maturity: f64, weight: f64 */ }             // maturity, weight
/// The mix in closed form, F_c exp(−Λ_s) cut at 52 nodes; `None` without a maturity and a rate.
pub fn maturity_mix(synth: &Synthesiser<'_, CoarseField>) -> Option<MaturityMix>;
/// The solve over the mix, each material one term a node; the halo-free solve where none moves.
pub fn haloed_albedo_scale(fractions: &ClassFractions, palette: &MaterialPalette,
    exposure: GroundExposure, mix: &MaturityMix, geometric_albedo_bvr: [f64; 3])
    -> Result<AlbedoScale, SolveAlbedoScaleError>;
// AlbedoScale::halo_excess_v(): the mean V excess the haloes add at the solved scale (0 without
// a mix)
```

### `ground` and `reading`

```rust
/// The authoritative ground: R05's `finest_surface_height` over R09's synthesis, with the
/// triangle it came from. Collision reads this and nothing else. As built R05's interpolant
/// returns the height alone (its lattice and split are private), so T3 adds the triangle's query
/// beside it in `patch::collision` (as built: the crate's `mesh_point` and `finest_surface_point`,
/// returning `MeshPoint`). The ground is at p's foot on the datum, along its normal (T3's
/// deviations).
pub fn ground_at<F: FieldView>(synth: &Synthesiser<'_, F>, cache: &mut SynthCache,
    p: &[f64; 3]) -> Result<GroundPoint, QueryHeightError>;            // body-fixed, metres
pub struct GroundPoint { /* height: Metres (above R07's reference spheroid, along its normal),
    normal: [f64; 3] (body-fixed, unit),
    triangle: FinestTriangle,
    as built also dir: [f64; 3] (p's foot) and point_height: Metres (p's own); getters */ }
/// As built, a type alias of R05's `patch::collision::MeshTriangle`, with getters and `vertices()`.
pub struct FinestTriangle { /* key: PatchKey (finest level), x: u8, y: u8, upper: bool */ }

/// A field view that knows each held cell's survey code (as built, T5): a survey's code, `NONE`
/// for a margin cell, `None` for a cell not held. Implemented for `PartialField`.
pub trait SurveyView: FieldView { fn resolution(&self, cell: PatchKey) -> Option<ResolutionCode>; }
/// What a readout may quote at a point, given the survey resolution the ship holds there; private
/// fields with getters (`is_resolved()` for `resolved`).
pub struct SurveyedReading {
    /* elevation: Metres, elevation_sigma: Metres (R09's level-keyed `omitted_rms` at the
       reading's level), slope_rms: Radians, slope_sigma: Radians, slope_baseline: Metres,
       resolution: Metres (the cell code's, for T12's RESOLUTION, as ruled),
       resolved: bool (sigma is zero: the survey reaches the band limit) */
}
pub fn surveyed_reading<F: SurveyView>(synth: &Synthesiser<'_, F>, cache: &mut SynthCache,
    at: FaceUv, slope_baseline: Metres)
    -> Result<Option<SurveyedReading>, QueryHeightError>;          // None: unsurveyed
/// Where a line of sight meets the ground, as far as the survey can say: `NO GROUND` and
/// `NOT SURVEYED` apart (as ruled, `decision-r10-t5-closed-forms.md`).
pub enum LineOfSight { Ground(SlantRange), NoGround, NotSurveyed }
pub struct SlantRange { /* range: Metres, sigma: Option<Metres> (None when grazing),
    resolved: bool, hit_m: [f64; 3] (body-fixed), at: FaceUv (the hit's foot); getters,
    `is_grazing()` (sigma is None) and `is_resolved()` */ }
pub fn slant_range<F: SurveyView>(synth: &Synthesiser<'_, F>, cache: &mut SynthCache,
    origin_m: &[f64; 3], direction: &[f64; 3], max: Metres)
    -> Result<LineOfSight, QueryHeightError>;
pub const SLOPE_BASELINES: [Metres; 3];   // 2 m (default), 5 m, 100 m, Design note 7
pub const GRAZING_FACTOR: f64;             // 2: grazing where sin γ_s < 2√v, Design note 7
```

As built (T5's deviations, ruled in `decision-r10-t5-closed-forms.md`): both read the survey from
the view through `SurveyView`, so `surveyed_reading` takes no `survey`; the σ and the slope's v are
R09's level-keyed closed forms, which T5 added to R09's `Synthesiser` (`omitted_variance`,
`omitted_structure_function`, `omitted_rms`, R09's Provides).

### `bake`

```rust
/// What this plan adds to R05's `PatchBake`. The palette is the union of the patch's vertices'
/// `ClassWeights` materials, `GroundMaterial`s as T2 built them (R10.T2's record).
pub struct TerrainLayers {
    /* palette: [Option<GroundMaterial>; 8], weights: Vec<[u8; 8]> (65 × 65),
       survey_mask: Vec<u8> (65 × 65: 255 surveyed, 0 not), horizon: HorizonMap,
       bound: Metres, sigma: Metres, extra: Vec<scatter::ExtraLayer> (R11's, through `BakeHook`) */
}
pub struct HorizonMap { /* azimuths: 8, near: 65 × 65 × 8, far: 9 × 9 × 8, elevations as
    snorm16 of e ÷ 90°, two to a u32, Design note 10 */ }
pub fn encode_horizon(elevation: Radians) -> i16;   // WGSL `pack2x16snorm`'s rounding, integer only
pub fn decode_horizon(code: i16) -> Radians;
/// R11 adds per-patch layers (the rock list) through this, in the same bake and cache. R11's
/// `RockBakeHook` owns its own `SynthCache`, so the hook takes nothing more. T6.a declares
/// `scatter::ExtraLayer` as an empty `#[non_exhaustive]` enum, which R11 owns and gives its
/// `Rocks(Vec<Rock>)` variant.
pub trait BakeHook { fn bake(&self, key: PatchKey, heights: &[f32], out: &mut Vec<ExtraLayer>); }
/// The face-wide far-field lattice: its points keyed by position alone, the same bits at
/// every level; its values a level's (Design note 10; main's ruling for R10.T8.a, 2026-10-15).
pub fn far_field_point(face: Face, level: u8, i: u32, j: u32) -> FaceUv;
/// As built by T6.a (Risks, "Deviations in T6.a, as built"): the bake over a client's field, R05's
/// arrays and this plan's layers; `WeightsLayer::{Baked, Skipped}`; `TerrainLayers`' fields
/// private, with getters and `is_complete()` (no vertex took the reference surface); the mask's
/// `SURVEYED` (255) and `UNSURVEYED` (0); `HorizonMap` empty until T8.a. As built by T8.a (Risks,
/// "Deviations in T8.a, as built"): `bake/horizon.rs` (`pub mod horizon`, its items re-exported by
/// `bake`); `TerrainBakeOptions` gains `horizon: HorizonLayer::{Baked, Skipped}`; the azimuths are
/// the patch grid's `AZIMUTH_STEPS` at k × 45° of `grid_azimuth(face, at, towards)`; `HorizonMap`'s
/// `near_words`, `far_words` (4 words a texel, `pack2x16snorm`'s halves), `near(x, y, k)`,
/// `far(a, b, k)`, `elevation_at(x, y, azimuth)` (the shader's lookup at a vertex) and
/// `is_complete()` (the far field read only ground the view holds); `NO_HORIZON`, `AZIMUTHS`,
/// `FAR_SIDE`, `FAR_STRIDE`, `WORDS_PER_TEXEL`, `ZENITH_TOLERANCE`.
pub struct TerrainBakeOptions { pub patch: BakeOptions, pub weights: WeightsLayer }
pub struct TerrainBake { /* patch: PatchBake, layers: TerrainLayers */ } // patch, layers, into_parts
pub fn bake_terrain<F: SurveyView>(synth: &Synthesiser<'_, F>, cache: &mut SynthCache,
    key: PatchKey, opts: &TerrainBakeOptions, hooks: &[&dyn BakeHook])
    -> Result<TerrainBake, QueryHeightError>;
```

_After R10.T1.c's client half (Risks, "Deviations in T1.c's client half, as built"):
`TerrainLayers` also holds `maturity()`, 65 × 65 `[u16; 2]` (the unorm16 codes at n and n − 1;
empty without a maturity or with the weights skipped), with `encode_maturity` and
`decode_maturity` in `bake`; `material` adds `maturity_cut`, `maturity_at_level`,
`patch_maturity`, `unresolved_terms` (`UnresolvedTerms`), `unresolved_mix` (`UnresolvedMix`),
`maturity_nodes`, `maturity_node_share`, `maturity_node_albedo` and `MATURITY_NODES` (16); and
`worker` adds `WorkerFields::maturity_nodes` (`DrawnNodes`), `ClassMapClass::nodes` and
`ClassMapTexel::maturity`._

The worker's WebAssembly entry points, on R05's binding, gain `field_init`, `field_chunk`,
`field_release` and a `bake` that takes R05's `BakeOptions` with this plan's settings, and answer
`field_init` with the body's `LevelBounds` (below). They replace R05's single `postField(bytes)`
with these per-body calls. For the readouts they also gain `reading`, `slant_range` and `material`,
the wasm faces of `surveyed_reading`, `slant_range` and `material_at` with `properties` (T6.b).
As built, R05's module (`src/wasm.rs`) serves the test planet alone: `bakePatch`, `levelTable`,
`surfaceHeightM` and `omittedSigmaM`, each taking `Ridges`, which the spike keeps (R09 Design note
18); the per-body exports go beside them, under the same camel-case `js_name` rule. _As built by
T6.b (Risks, "Deviations in T6.b, as built"): the per-body exports are the methods of one
`SurfaceFields` object a worker makes, over the crate's new `worker::WorkerFields` (a
`PartialField`, its level bounds and a `SynthCache` a body): `fieldInit`, `fieldChunk` and
`fieldRelease`, each block answered with the cells it surveyed anew and the body's level table in
`levelTable`'s layout; `bake`, with R10's layers and the cells the mask holds unsurveyed;
`reading`, `slantRange` and `material`; and `heldBytes`. A body is its system's raw ID and its
index, and each query is given the body's detail seed, which the field does not carry._

### Client (`apps/hyperion/src/renderer/src/view/terrain/`)

- `coarseFieldStore.ts`: `CoarseFieldStore` with `want(body, reason)`, `release(body)`,
  `coverage(body): CoverageView`, `onRevision`; it drives R09's `surface_field` and posts to R05's
  `HeightWorkerPool`.
- `coverage.ts`: `CoverageView` (`resolutionAt(cell)`, `surveyedFraction()`, `bestResolution()`),
  `surveyEdges(face, level)` for the edge mark, and the GPU coverage mask per face. _As built
  (Risks, "Deviations in T10.c, as built"): `surveyEdges(coverage, face)`, the level the
  coverage's own; the mask is `CoverageMask` (`update`, `survey`, `restore`, `release`), and the
  mark `surveyEdge.ts`' `surveyEdgeMark` and `surveyEdgeNote`._
- `handover.ts` (as built, R10.T10.e; Risks, "Deviations in T10.e, as built"): `handoverRangeM`,
  the decision (`handsOver`, and `handOverRegimes` over a frame's discs), `handoverRangeRatio`,
  `handoverReliefM`, `limbDistanceM`, `pixelAngleRad`, `handoverTauPx`, and `storeReliefSource`
  over a `CoarseFieldStore` as a `HandoverReliefSource`; with R07's `photoreal/renderer.ts`
  `FrameTerrain`, the frame's terrain input. R10.T16 ("The live terrain path") consumes them.
  _Since R10.T10.g the hand-over counts the presented width's centre pixel
  (`BodyFrameOptions.presentedPx`, `PhotorealFrame.presentedPx`)._
- The disc's normal map (as built, R10.T10.g; Risks, "Deviations in T10.g, as built"): the
  surface crate's `WorkerFields::normal_map` (`NormalMap`, the module's `normalMap`), carried in
  the workers' `class-map` answer; R07's `bodies/discNormals.ts` (`NormalMapTexels`,
  `packNormalMap`, `normalMapNormalAt`, `normalMapTexture`), held by `ClassMapStore` beside the
  class map as `ClassMapDiscSurface.normals` and read by the disc (`bodyDisc.wgsl`, binding 14)
  and by T10.f's point integral.
- `levelBounds.ts`: `LevelBounds { hard: Float64Array; sigma: Float64Array }` per body, and
  `selectionBound(bounds, level, rule)`. As built, R05's selection reads its bound from the body's
  level table (`PlanetGeometry.levels`, four values a level: bound, lowest and highest height,
  spacing; `planet.ts`'s `levelBoundM`), and `select.ts`'s `selectionErrorM` adds the chord
  sagitta before `screenSpaceErrorPx`; so `selectionBound` fills that table's bound column.
- `classMap.ts`: the per-body coarse class-weights map R07's disc samples.
- `shaders/terrainLit.wgsl` (the file R07 names) with `terrainMaterial.ts`: palette and weights to
  `body_brdf`'s per-class law, the survey mask, skirts at the survey edge, and the hooks R11 fills:
  `terrain_decoration`; `terrain_shadow_factor(p) -> f32`, the direct sun's lit fraction from this
  plan's terrain shadow, into which R11's cloud shadows multiply;
  `terrain_ring_shadow(p, sun) -> vec3f`, 1 by default, which R11 fills for a ringed body with its
  `ring_shadow_on_body(p, sun) -> vec3f` per channel, the type R07's call uses, and which also
  multiplies the direct sun only; and
  `terrain_sky_factor(p) -> vec3f`, 1 by default, a factor on the ground's diffuse sky irradiance
  (R08's `atmosphere_sky_irradiance` where the body has an atmosphere, zero otherwise), into which
  R11 puts a cloud deck's diffuse transmittance (Design note 11). The direct sun on terrain is R08's
  `atmosphere_sun_transmittance` times the three shadow terms.
- `shaders/terrainWire.wgsl` with `terrainWire.ts`: the depth-only pass and its contours;
  `contourInterval(aglM, previous)`, `contourPhase(patchOriginHeightM, intervalM)`.
- `shadows/horizon.wgsl`, `shadows/cascades.ts`: the two shadow paths and their combination.
- `readouts.ts`: `formatWithUncertainty`, `formatElevationReading`, `formatSlopeReading`,
  `formatSlantRange`, and the `SurveyLine` and `TerrainReadout` components in the view's label
  block.
- `cacheBudget.ts`: `bytesPerPatch(options)` and `patchCountModel(camera, tau, viewport)`, the
  pure count model of Design note 15. The budgets themselves are R05's `TerrainSettings.cacheBytes`
  in `view/quality/qualitySetting.ts`'s `SETTINGS` (400,000,000 B high, 64 MiB low, as built),
  the one list of R05's Design note 26, not a second `PATCH_CACHE_BYTES`.
- `session.ts` (R10.T16): `TerrainSession`, one a source (the open universe's scene, or
  `TERRAIN TEST`'s fixture) and engine: the `CoarseFieldStore`, one `HeightWorkerPool` in lanes,
  each wanted body's `CoverageMask`, class map and detail seed, each handed-over body's
  `TerrainPass`, the surveys the label block reads, and its memory report; made by
  `displays/view/useTerrainSession.ts` in `ViewPanels`.
- `frameTerrain.ts` (R10.T16): `terrainLightingOf(record, rotation, exposureScale)`, a disc
  record's lights in the body-fixed axes, and the live `FrameTerrain` a view's frame takes.
- `surveyLookup.ts` (R10.T16.b): `SurveyedCells`, the cells a body's blocks surveyed, and the
  `SurveyLookup` selection reads beside its height ranges (a patch surveyed where any coarse cell
  its vertices' masks read is).
- `residentHeights.ts` (R10.T16.c): `ResidentHeights`, a pass's heights of its resident bakes that
  hold the survey's edge and a coarse copy of the rest, for the edge's rim and the note's line of
  sight; R10.T11's contours' nadir height reads it.
- `lib/format.ts`' `formatSurfaceRange` (R10.T16.c; decision-r10-t16-review §1): a range to the
  ground in `SLANT RANGE`'s units, which the `SURVEY EDGE` notes take and R10.T12's `SLANT RANGE`
  will.

### Test helpers

Two helper modules, one on each side of R04's crate boundary (`hyperion-surface` depends on base
alone and never on the sim, by R04's entry in `.claude/rules/rust-dev.md`):

- `crates/hyperion-surface/tests/common/worlds.rs`: `synthetic_worlds()`, R09's
  `testing::synthetic_field` worlds, fast enough for the goldens on all three targets; and
  `flat_field(height)`, `cliff_field(height, strike)` built with R09's `FieldBuilder` for the
  horizon and classifier tests. As built, `testing` is a feature that the surface crate's own
  integration tests do not see, so their commands pass `--features testing` (the browser recipes
  already do). `FieldBuilder::elevation` sets one value a coarse cell, which R09's approximating
  spline blends over about two cells (R09's "Deviations in T4, as built"), so a `cliff_field`
  steeper than 40° needs a body small enough for two cells to span less than the cliff's run.
- `crates/hyperion-sim/tests/common/surface_worlds.rs`: `sample_worlds()`, R09's reference worlds
  (`planetary::surface::reference::{earth_like, mars_like, moon_like, ceres_like}`) plus an icy
  moon and a locked world, `reference::europa_like` (level 6) and `reference::locked_earth_like`
  (level 8), each run through the coarse pass under `reference::SEED` once per test binary
  (`SampleWorld::field()`). The sim depends on the surface crate, so these tests call this plan's
  functions directly. As built, each world's `tier()` says which tests may run it: the reference
  Earth and the locked world are `Tier::Slow`, and Mars, the Moon, Ceres and the icy moon
  `Tier::Fast`. They run natively; the cross-target goldens use `synthetic_worlds()`.
  `reference::*` is behind the sim's `testing` feature (`coarse_pass(seed, &inputs)`), so
  `cargo test -p hyperion-sim` needs `--features testing`, and the module is declared under
  `cfg(feature = "testing")` in `tests/common/mod.rs`. R09.T16 built it, and
  `tests/surface_coarse_golden.rs` ties each world's field to its golden (R09's "Deviations in
  T16, as built"). The order of tasks says what waits on it.
  Under nextest each test runs in a process of its own, so "once per test binary" holds within one
  test (R09.T16's note). On a world wet now the coarse land hypsometry is its construction's by
  rule (decision-r09-t14-choices, T14.c follow-up 2 §2), so a test here that checks land
  hypsometry after erosion reads R09.T12.a's bands. _As built (R10.T1.b's `sample_worlds()`
  follow-up, 2026-10-23): `sample_world(name)` is the sample world of a name, panicking with the
  name where none has it. `surface_material_worlds.rs` and `surface_ground_worlds.rs` read it, and
  T2's, T5's and T10.a's `sample_worlds()` follow-ups read it in place of a lookup of their own._

## Consumes

Names are the owning plans' as they stand on disk; the owner is authoritative, and where a name has
changed by the time this plan runs only the call sites here change. Re-validated at `c6d77cac`
against R05, R07, R08 and R09 as built (Risks, "Re-validated at `c6d77cac`"). R09.T8 has landed
(`c280f604`): `height_at`, `bake_patch`, the level bounds and the closed forms are built, under the
names R09's "Deviations in T8, as built" records. R09.T19.b has landed (`7392c064`): the
`surface_field` answer, its deltas since `have_revision`, and `surface_revisions` on the scene
topic are built, under the names in R09's "Deviations in T19.b, as built" (updated 2026-10-15).
R09.T9 has landed (`070da8ee`), and its goldens pin R09's heights and closed forms on all three
targets at header 21 (below).

- **R01:** the engine adapter, its WGSL-only material path, its adapter feature report
  (`GpuCapabilities` in `view/engine/platform.ts`, with `depthClipControl`, commented as for R10's
  cascades, and `timestampQuery`) and the headless SwiftShader smoke harness, with its no-f16 and
  no-subgroup runs; `MemoryCategory` (`view/engine/memory.ts`), which had neither
  `coarse-field-gpu` nor `shadows` (T7 added the first; T9.b adds the second where it creates the
  layers, decision-r10-t9-split).
- **R02:** the view's camera, per-patch origin convention and rotation-only view matrix; the
  wireframe style's line tokens, casing and label block (`displays/view/viewRun.ts`'s
  `labelStatements(run, terrain)` and `labelLines`, drawn by `ViewLabelBlock.tsx`); the DOM list
  (`ViewMarkList.tsx`, a windowed `listbox` of `viewRun.ts`'s `MarkRow`); the graticule; the nine
  drafted guide items, of which this plan extends items 1, 3, 5 and 7. Body-fixed vectors reach the
  view as a `ViewPosition` of kind `body_fixed` (`view/coords/position.ts`); the client has no
  `BodyFixedPosition` or `BodyFixedVector`, which are the sim's Rust types.
- **R03:** the binary frames the chunks ride, their backpressure, and the scene topic carrying R09's
  `surface_revisions`: R09.T19.b's optional list of `SurfaceRevisionDto { body, revision }` on the
  scene notification, one entry per surveyed body with its latest revision, merged by body keeping
  the larger (updated 2026-10-15). As built, `packages/protocol/src/requests.ts`'s
  `RequestClient.requestBulk`, taking `(body, manifestOf, options)`, hands over whole answers
  only, each `partial_response` with its chunks through `options.onPartial` and the last through
  `outcome`, so forwarding a block as its frames arrive
  (Design note 16) takes a per-chunk option there, which T7 adds; `MAX_BULK_CHUNKS` (257) and
  `MAX_BULK_PAYLOAD_BYTES` (64 MiB) are in `packages/protocol/src/bulk.ts`. The client's scene
  model is `lib/scene/useScene.ts` and `sceneWire.ts`'s `applySceneNotification`; the fake socket
  is `renderer/src/test/FakeWebSocket.ts` (with `binaryFrames.ts`'s `binaryFrame`).
- **R04:** the `hyperion-surface` crate with its `clippy.toml` and relaxed-SIMD `compile_error!`;
  the base crate's `math`, with R04.T4.c's `math::j0` for Design note 7, `rng` and `units`; both
  wasm targets' fast goldens (`just test-wasm-fast`, which `just ci` runs); the testkit's
  `include_str!` arm (`golden!`, `check_embedded`), `GoldenWriter`, `f32_digest` and `f64_digest`,
  and `order::assert_order_independent(keys, f)`; and R04 Design note 12's conventions for every
  test in `hyperion-surface`: the `wasm_bindgen_test as test` import, a `native_only` module for a
  test that cannot run on `wasm32-unknown-unknown` (one that reads files), and every
  `should_panic` test stating `expected` in the crate's own `tests/panics.rs`.
- **R05:** `cube` and `geometry` (`Face`, `FaceUv`, `PatchKey`, `finest_level`, `MAX_LEVEL`,
  `FINEST_SPACING_M`); `patch::{HeightSource, bake_patch, BakeOptions, PatchBake}`,
  `patch::{VertexPath, NormalScale, finest_surface_height}`, generic over `HeightSource` (R05.T4.a,
  T4.c), which this plan implements for R09's `Synthesiser` (Design note 4); the level bound's role
  in selection (its Design note 15) and `screenSpaceErrorPx`; `selectPatches`, `morphHold`,
  `GroundContact` and the grounded-body rule; `PatchCache` and `resolveDrawSet`; `HeightWorkerPool`,
  whose single `postField(bytes)` this plan replaces with per-body `field_init`, `field_chunk` and
  `field_release` (T7 on the render thread, T6.b in the worker); `normals` packed as `rg16float`
  (its Design note 5) and `PatchCache` built on a `SlotLayout` of this plan's (its Design note 10);
  `terrainPass.ts`; `terrainAnnunciation`; the scripted descent, `descentProfile.ts`, `metrics.ts`
  and the results files under `docs/measurements/descent-spike/`. As built:
  - `HeightSource` (`patch.rs`) is `type Cache`, an `Error` bounded by
    `std::error::Error + Send + Sync + 'static`, `figure()`, `height(cache, dir, level: u8)`,
    `level_bound_m(level: u8)` (documented as "the level's hard error bound"),
    `height_range_m(key: PatchKey)` and a default `prepare_cache(cache, key)`; only `TestPlanet`
    and a test fixture implement it.
  - `PatchBake` also carries `origin_height_m` and `skirt_depth_m` (`level_bound_m` plus the `f32`
    step plus `skirt_m`), and `height_range_m: (f32, f32)`.
  - `collision::finest_surface_height` and `collision::mesh_height(source, dir, level, cache)`
    return the height alone; no query returns the triangle (T3 adds one).
  - Selection reads the bound from `PlanetGeometry.levels` (`planet.ts`, `LEVEL_TABLE_STRIDE` 4:
    bound, lowest and highest height, spacing; `levelBoundM`, `levelHeightRangeM`), and
    `selectionErrorM` adds the chord sagitta; the wasm module's `levelTable(ridges)` fills it for
    the test planet.
  - `slotLayout.ts`: `slotLayout(fields, budgetBytes)`, `terrainSlotFields(terrain)`,
    `terrainSlotLayout(terrain)`, `MIN_SLOTS` (6); `SlotFieldName` is `heights`, `offsets`,
    `normals` and `horizon-map` (0 B until sized), with R10's class weights and survey mask still
    to add. Normals are stored with the atlas's one-texel gutter, 17,956 B a patch at the mesh's
    resolution and 68,644 B doubled (R05's "Deviations in T8, as built").
  - The pool (`workers/pool.ts`) takes `{ workers, createWorker, bake: BakeSettings }`;
    `BakeSettings` and `TerrainPassOptions` carry the test planet's `ridges`; `postField` sends one
    whole buffer, which the worker (`heightBake.ts`'s `answerRequest`, `height.worker.ts`) holds in
    JavaScript; the spike calls it (`view/spike/spikeRun.ts`). The scripted fake worker is in
    `pool.test.ts`.
  - The settings: `view/quality/qualitySetting.ts`'s `TerrainSettings` (`tauPx`, `renderHeightPx`,
    `normals`, `vertexPath`, `cacheBytes`); high is τ 1, double normals, `BakedOffsets`,
    400,000,000 B; low is τ 2 at 720 px, mesh normals, `FaceDifferences`, 64 MiB.
  - `gpu/vertexEmulation.ts` (`faceDifferencePositionF32`, `faceDifferenceMorphF32`) forms the
    vertices in `Math.fround`, for T15.
- **R07:** in `shaders/litBody.wgsl`,
  `struct LunarLambert { a: vec3f, l: f32, s: vec3f, table_row: u32 }` (`table_row`, not `template`, a
  WGSL reserved word: the row of `phase_factor_table` holding the law's f, R07.T4.c as built),
  `body_brdf(law: LunarLambert, mu0, mu, alpha) -> vec3f` (R07.T4.c),
  `sphere_irradiance(h, phi, horizon) -> f32` with its local-horizon argument (R07.T6.a, T6.c) and
  `eclipse_visible`; `DiscSurface` and its `class-map` case
  `{ weights: TextureHandle; laws: ReadonlyArray<PhotometricLaw>; elsewhere: PhotometricLaw }`,
  the type in `appearance/bodyAppearance.ts` and its builders in `bodies/discSurface.ts`, read by
  `bodyDisc.wgsl` (R07.T8.b), as built: the map's layout (a 2D array of `rgba8unorm`,
  N × N texels a face of R05's cube sphere under `uvToSt`, class k in channel k mod 4 of layer f +
  6⌊k ÷ 4⌋, at most 16 classes, `MAX_DISC_CLASSES`), `packClassMap`, `classMapSurface` and
  `classMapTextureSpec`, and
  `LitBodyInput`'s optional `surface` and `rotation`, which the view's `litBodiesOf` passes (T10.d);
  `BodyFigure`, the reference spheroid every height here is
  measured from, along its normal (R07's Design note 19); `BodyAppearance`, `LitRegime`
  (`"point" | "disc" | "mesh"`) and `lawFor(p: Rgb, q: Rgb, template: PhaseTemplateId)`; the star's
  angular radius per view. R07 creates those signatures with default inputs; this plan's T8.b (the
  horizon from the horizon map), T10.b (a `LunarLambert` per class, their `body_brdf` summed per
  texel by weight) and T10.d (the class map) supply the inputs through them unchanged. As built:
  - `phase_factor_table` is an `rgba32float` texture (`bodyDisc.wgsl` binding 1, `DISC_TEXTURES` in
    `bodies/draw.ts`), one row a law of 361 texels from 0° to 180°, f in r, g and b and alpha unused
    (written 0), filled by `appearance/litBodyProbe.ts`'s `packPhaseFactorRows`; `PhotometricLaw`
    (`appearance/law.ts`) has `lommelSeeligerShare` and no curve. The L(α) channel is T10.b's.
  - `sphere_irradiance` clamps `horizon` at 0, and `litBody.wgsl` takes WGSL's built-in
    trigonometry, which stays on main's deferred list (`decision-r08-f32.md` §9).
  - `classMapTextureSpec` files the map under `MemoryCategory` `"other"`; `litBodiesOf`
    (`displays/view/photorealFrame.ts`) sets `rotation` where known and never `surface`, and
    `discSurfaceOf(appearance)` is always uniform. `PhotorealInputs` is built in
    `displays/view/viewFrameDrawer.ts`, which `ViewDisplay.tsx` makes; `photorealFrame` passes
    `depthWriters: []`, "R10's terrain and lit craft add their footprints".
  - The `mesh` regime is `bodies/smoothMesh.ts` with `shaders/smoothMesh.wgsl` and
    `SMOOTH_MESH_MATERIAL` (`bodies/draw.ts`), which already include R05's
    `terrain/shaders/patchVertex.wgsl`; `discEclipseVisible` is in `view/lighting/discEclipse.ts`;
    `pointFlux` and `oblateAlbedoScale(share, cOverA)` in `bodies/draw.ts` and `bodies/oblate.ts`.
- **R08:** aerial perspective applied to the terrain pass's output; R08.T9.b's
  `surfaceLighting.wgsl` with
  `atmosphere_sun_transmittance(altitude_m, mu_sun, latitude_rad, sun_azimuth_rad, curve) -> vec3f`,
  `atmosphere_sky_irradiance(altitude_m, mu_sun, latitude_rad, curve) -> vec3f` and, for
  planetshine, `atmosphere_source_transmittance`, which takes the source's angular radius before
  its `curve`, the terrain's direct and diffuse light where the body has
  an atmosphere, taking the texel's geodetic height and latitude, the sun's azimuth from local north
  (R08 Design note 17) and `curve : u32`, the light's absorber-curve slot (0 for a planetshine
  source; R08's Risks, "`atmosphere_sun_transmittance`'s fifth argument"); and the sun's refracted
  apparent elevation where it draws one (Design note 10).
  Every argument is a height above the datum (the patch's baked height, R05 Design note 5) and a
  cosine against the datum normal, never a radius or |P| − R. This plan's own WGSL follows R08
  Design note 18's rule for angles, through R08's `view/shaders/preciseTrig.wgsl`
  (`decision-r08-f32.md`). As built:
  - R08.T9.b is not built: the two names are R07's stubs in `litBody.wgsl`, four and three
    arguments, returning 1 and 0, which R08.T9.b replaces.
  - R08.T9.a's aerial perspective (`5264a413`): on the low setting (`AERIAL_PERSPECTIVE_SCOPE`
    `"terrain"`, `view/atmosphere/settings.ts`) lit terrain is drawn into an `AtmosphereScene`
    (`{ colour, depth, nearM, exposureScale }`) and
    `HillaireAtmosphere.drawFrame(view, suns, scene)` composites it; on the high (`"scene"`) the
    composite covers every opaque pass drawn
    before it. `aerialPerspective(view)` (null on low) and `aerialRead.wgsl`'s
    `aerialPerspectiveAt` serve only passes the composite does not cover.
  - `preciseTrig.wgsl` has `preciseAtan2(y, x)` for y ≥ 0 (a result in [0, π]), `preciseAtan`,
    `preciseSinCos` and `preciseCos`, and no `asin` or `acos`.
  - Refraction is not drawn: until it is, this plan reads the geometric elevation (R08's Risks).
  - R08.T18's thermal emission, which T10.b calls on terrain (decision-composition), is not built.
- **R09:** `field`'s `FieldView`, `PartialField`, `CoarseField`, `FieldHeader`, `SynthesisCell`,
  `ClimateCell`, `CoarseCrater`, `Cover` and `SYNTHESIS_MARGIN_CELLS`; `wire::decode_payload`;
  `PartialField::{is_surveyed, resolution}`, its `insert(&wire::DecodedBlock)`, and
  `ResolutionCode`; `FieldView::craters_reaching`, an iterator; `synth`'s `Synthesiser`,
  `SynthCache`, `BandLevel` and `level_bound_m`; `unresolved_variance`, `structure_function` and
  `unresolved_rms`, each taking `field: &impl FieldView` and a cell and returning `Option<_>`
  (`None` for a cell the view does not hold), and the level-keyed `Synthesiser::omitted_variance`,
  `omitted_structure_function` and `omitted_rms` (added by R10.T5), what a band level leaves out; `height::HeightSample`, with R05's `BAND_LIMIT_M` and
  `FINEST_SPACING_M`; `FieldHeader.albedo_scale: Option<f64>`, which this plan fills (T10.a);
  `SurfaceClass`; `DetailSeed`; the `surface_field` request with `SurfaceFieldDto` and `CoverDto`,
  whose ranges carry each cell's `ResolutionCode`; `surface_revisions` on the scene topic;
  `SurfaceService` for the albedo scale's call site; `testing::{synthetic_field, FieldBuilder}` and
  the reference worlds (`hyperion_sim::planetary::surface::reference`, in the sim's tests only);
  heights, elevations and sea level along the normal of R07's reference spheroid, with a and c in
  the header, and block 0 of every payload carrying the whole `FieldHeader` (its Design note 17).
  As built (R09's "Deviations … as built" records):
  - _Built._ `PartialField` implements `FieldView`; its `resolution` is `Some(ResolutionCode::NONE)`
    on a margin cell (held, not surveyed) and `None` on a cell not held, and
    `ResolutionCode::resolution()` gives metres (`finer()` orders codes, not `Ord`). The header is
    read through getters (`figure()`, `sea_level()`, `realised_relief()`, `albedo_scale()`,
    `palette()`, `crust_entry(crust)`, `main_liquid()`, `surface_age()`), and
    `CoarseField::with_albedo_scale(Option<f64>)` sets the scale without rebuilding the field. The
    palette is `MaterialPalette` of at most 15 `PaletteEntry`s, whose members are `substance`,
    `role`, `normal_albedo_bvr`, `phase_row`, `grain_density_kg_m3`, `transition` and `mechanics`
    (`PaletteRole`, `MechanicsFamily`),
    every `phase_row` 0 until this plan assigns its codes; `SynthesisCell::substances` packs the
    cell's ice and liquid entries (`ice_entry()`, `liquid_entry()`; the nibble names the species,
    the share its presence). A worker decodes one block at a time with `wire::decode_block`; a
    block's first 10 bytes are its frame (`BLOCK_MAGIC` `HYSF`, the format, its length as a `u32`
    at offset 6, `BLOCK_HEADER_BYTES` 33), so the render thread splits blocks without decoding.
    `Synthesiser::{new, height_at, field, seed, cell_at}`, `SynthCache::new`, `BandLevel::new` (a
    `Result`), `QueryHeightError::NotSurveyed(PatchKey)` alone, `unresolved_variance` with
    `UnresolvedVariance`'s getters `relief()`, `channels()`, `craters()` and `total()`, and the
    per-contribution structure functions `synth::channels::channel_structure_function` and
    `synth::craters::structure_function` (the relief has only its unit tables, `UnitStructure`);
    the slope R09 computes is a body-fixed gradient whose tangential part is the surface gradient,
    so a slope reader projects it first, as `patch::normals::surface_normal` does (R09's
    "Deviations in T4, as built"). `coarse_pass(seed, &inputs)`. On the wire (R09.T19.a):
    `SurfaceFieldRequest { universe, body, have_revision: Option<u32> }` (`null`, not absent),
    `SurfaceFieldDto` tagged by `state`, its `ready` arm holding `header` (a `SurfaceHeaderDto`),
    `revision`, `coverage` (a `CoverDto`) and `bulk`, or `not_modelled { section }`,
    `CoverRangeDto { start, end, resolution }`,
    and `SurfaceHeaderDto`'s scalars with `albedo_scale` (`null` until T10.a) and the palette as
    `PaletteEntryDto { substance, role }` alone (the optical and mechanical values ride block 0).
    The server's call site for the albedo scale is `surface/service.rs`'s `compute`, after
    `coarse_pass`, where the body's record (and so its photometry) is in hand (R09's "Deviations in
    T17, as built").
  - _Landed with R09.T8_ (`c280f604`; R09's "Deviations in T8, as built"; updated 2026-10-15):
    Provides' `synth/mod.rs` is `synth.rs`, with two public modules, `synth::levels` (`LevelSet`,
    `LevelBounds` with `new`, `set`, `bound_m` and `contribution_range_m`, the free
    `level_bound_m`, `shortest_wavelength` and `CRATER_BOUND_TAIL`) and `synth::bake`
    (`PatchHeights` and `PARENT_VERTICES`). `Synthesiser` is `Clone`, not `Copy`, and takes 1–10 ms
    to build. It gains `level_bound_m`, `bake_patch` (which gives `PatchHeights`, the source's
    view, not R05's `PatchBake`), `height_range_m`, `unresolved_variance`, `structure_function` and
    `unresolved_rms`, beside the free `structure_function` and `unresolved_rms`. `height_at` sums
    the base elevation, the octaves, the incision, the small craters and the coarse rims. There is
    no `QueryHeightError::NonFinite`: every part asserts its heights finite. `Synthesiser`
    implements R05's `HeightSource` since R10.T3 (Risks, "Deviations in T3, as built"). Main has
    accepted three as-built changes for T8 (Risks, "Re-validated at `c6d77cac`"): the bound is hard
    in its octaves and channels and a 1 − 10⁻⁹ per-point Chernoff quantile in its crater and rim
    terms; the structure function holds within 5% on belt cells only, channelled plains running
    15–37% high; and the bench is provisional at 14 µs a point and 77–82 ms a patch at L + 4.
  - _Landed with R09.T9_ (`070da8ee`; R09's "Deviations in T9, as built"): the golden heights and
    closed forms in `tests/golden/height/`, pinned on native, `wasm32-wasip1` and
    `wasm32-unknown-unknown` at header 21, which this plan's material and ground goldens follow.
  - _Landed with R09.T19.b_ (`7392c064`; R09's "Deviations in T19.b, as built"; updated
    2026-10-15): `surface_field` answers `ready { header, revision, coverage, bulk }` after the
    payload's binary frames, or `not_modelled { section }` with no bulk. With `have_revision` n,
    the payload is the delta since revision n; block 0 always carries the header, and `coverage`
    holds the delta's own cells alone. A `have_revision` above the body's revision is
    `bad_request` naming `have_revision`. The scene topic carries `surface_revisions`, a list of
    `SurfaceRevisionDto { body, revision }` merged by body, keeping the larger revision. Under
    `RecordInputs`, every generated body still answers `not_modelled`.
  - _Not built, and not R09.T8–T9's:_ the coarse pass's steps (R09.T11–T15) and R09.T16's
    `crates/hyperion-sim/tests/common/surface_worlds.rs`; until they land the reference worlds are
    T10's no-op pass, a smooth dry sphere with no fine relief.
- **R11:** nothing consumed; it fills this plan's `BakeHook` and shader hooks (`terrain_decoration`,
  `terrain_shadow_factor` with its cloud shadows, `terrain_ring_shadow` with R11's
  `ring_shadow_on_body`, and `terrain_sky_factor` with a cloud deck's diffuse transmittance).
- **Galaxy plan 14:** radius and reference surface, ocean fraction, ice fraction, `SurfaceState`
  with `SurfaceMaterial`, surface age, tectonic regime and volcanism level, `body_fixed_at`
  (P14.T14.c), and the rotation section on the wire (P14.T46.f), which the scene body carries as
  `rotation` once R07.T2.b reads it; without it a class map cannot be oriented and the disc shades
  with `elsewhere`, and the visual geometric albedo p that R07 asks of it. As built:
  `frames::body_fixed_at(frame, t) -> FrameRotation`; the rotation on the wire is
  `BodyRotationDto` on `BodySummaryDto.rotation` (built, read by R07.T2.b); the record's
  photometry has `geometric_albedo()` and `phase_integral()` (`Bands`), the wire's
  `BodyPhotometryDto` the albedo and no q; and the record's `Surface` is still an empty enum, so
  this plan reads the surface's figures through R09's field header, never the record.
- **Galaxy plan 12:** nothing directly; R09 gates coverage on P12.T7's store.

## Design notes

1. **The material classifier is `hyperion-surface` code and runs where the height runs** (researched
   2026-09-29). The brainstorm makes the class authoritative, "from slope, altitude, the coarse
   climate field and plan 14's ice fraction, from the same inputs as the height", and R09's Design
   note 18 leaves it to this plan over `FieldView` and `HeightSample`'s gradient. It sits beside the
   synthesis, under the same determinism discipline, and goldens pin it on native and both wasm
   targets. Its order of decision is fixed: `Seabed` below R09's water surface; the coarse pass's
   ice (`SeaIce` over the sea, `WaterIce` or `VolatileFrost` on land, of the cell's ice entry: the
   guide reads them `ICE`, a glacier or ice cap, and `FROST`, so T1 tells them apart by form
   whatever the species, and a nitrogen glacier is `ICE · N2`; signoff-9), since glaciers and ice
   caps stand in cliffs of their own; `Bedrock` above `CLIFF_SLOPE`; `Snow`, giving way to `Bedrock`
   above `SNOW_SHED_SLOPE`; `Melt` where the surface state is a magma ocean or a young volcanic
   province; `Evaporite` in arid basins whose base level the coarse pass filled (playas, among the
   brightest land surfaces); `Sediment` along channels and in the other filled basins; then by
   regime and R09's `SurfaceClass`: `Regolith` on an airless world, `Dust` on an arid thin-aired
   world where R09's wind field deposits, `Sand` and `Soil` by aridity, `OrganicSediment` for a
   Titan-like haze regime, `VolcanicRock` in older provinces. Loose material cannot rest above its
   static angle of repose, so steeper ground is exposed rock: `CLIFF_SLOPE` is 40° with a 5°
   transition band either side, covering the 35–45° of the literature. Kleinhans et al. 2011 (JGR
   116, E11004) measured about 40° for angular grains at 0.1, 0.38 and 1 g, with the static angle
   rising about 5° at low gravity, while Atwood-Stone and McEwen 2013 (GRL 40, 2929) found Martian
   dune lee faces at 30–34°, as on Earth; since the two disagree on gravity, the threshold does not
   scale with it. The Lunar Sourcebook (§9.1.10) gives dumped lunar soil nearly 40° and compacted
   soil about 45°. `SNOW_SHED_SLOPE`, about 50–60°, is from memory; T1.a finds a source or removes
   the rule and records the gap. Nothing is drawn from a stream, so the classifier needs no domain
   tag. _Folded in at the re-validation (decision-composition §1.8; R09's "Cold traps" ruling): each
   rule names its substance through R09's palette, frost or glacier of the cell's ice entry,
   `Seabed` of the floor's entry (the crust's, the bulk's or a `Deposit`'s), never its liquid's
   (signoff-9), `Melt` where R09's cell temperature exceeds the lithology's transition, and
   `Regolith`, `Sand` and `Dust` of the crust's entry or of a `Deposit` or `Cover` entry; and a
   cell's cold-trapped share, which is exposed ice and not the cold-trap area, lies on its shadowed
   ground (crater floors and pole-facing slopes), not evenly over the cell._
2. **Physical properties are data with sources, and bearing capacity is a function** (researched
   2026-09-29). Ultimate bearing capacity is not a constant of the ground: it grows with footing
   width and gravity, q_ult = c N_c ξ_c + ½ ρ g B N_γq ξ_γq (Lunar Sourcebook §9.1.9, after
   Durgunoglu and Mitchell 1975). The Sourcebook gives about 6,000 kPa for a 1 m footing on the
   Moon (§9.1.9, p. 517) and, in Fig. 9.36 (p. 518), a band of 3,000–11,000 kPa at the Apollo 11
   lunar module's footpads, nearly 1 m across, which applied only about 5 kPa (a factor of safety
   of 600–2,200). Those figures hold only for the soil a 1 m footing's failure zone reaches, one to
   two widths down: the near-surface pair (Table 9.12, 0–15 cm: c ≈ 0.52 kPa, φ ≈ 42°; Table 9.4:
   ρ ≈ 1.50 g/cm³) gives about 210 kPa (Vesić factors N_c ≈ 94, N_γ ≈ 156, square-footing shape
   factors about 1.9 and 0.6), while the 30–60 cm pair (c ≈ 3.0 kPa, φ ≈ 54°, ρ ≈ 1.74 g/cm³) gives
   about 5,400 kPa, and its range (c 2.4–3.8 kPa, φ 52–55°) about 3,000–8,000 kPa (researched
   2026-09-29 from Chapter 9 of the Sourcebook, read directly; high confidence in the arithmetic
   and the quotations, medium that the band was built from the deeper pair, which the book does
   not derive). So the table holds Mohr–Coulomb pairs and bulk densities by depth, and
   `bearing_capacity(props, g, width)` takes the layer at the failure zone's depth, about one
   footing width. The figures to settle source by source in T1.a: lunar regolith by Table 9.12's
   depth intervals (0–15, 0–30, 30–60 and 0–60 cm) with Table 9.4's densities; Martian soils from
   Viking (Moore et al. 1987, USGS PP 1389): drift φ ≈ 18°, c ≈ 1.6 kPa; crusty to cloddy φ ≈ 35°, c
   ≈ 1.1 kPa; blocky φ ≈ 31°, c ≈ 5.1 kPa; ice as a strength, 5–25 MPa unconfined at 10⁻³ s⁻¹ and
   rising as the temperature falls (Schulson and Duval 2009), with no Mohr–Coulomb term. Snow varies
   by orders of magnitude with density and stays missing. Pad friction is the interface friction tan
   δ against steel or aluminium: for granular classes from NAVFAC DM-7.02 Table 1 (0.20–0.40 from
   fine sandy silt to clean gravel) and Potyondy 1961's δ/φ ≈ 0.5–0.8, a lower bound once a pad has
   sunk in and passive earth pressure adds to it; for ice and frost a function of temperature, from
   about 0.01–0.05 near 0 °C at speed to 0.3–0.5 at −100 °C and below (Bowden and Hughes 1939;
   Kietzig, Hatzikiriakos and Englezos 2010), stored as two or more points and `None` outside them.
   Metal on bare rock has no primary source found (Byerlee's 0.6–0.85 is rock on rock) and stays
   `None` unless T1.a finds one. A value without a source is left out and the console shows it
   missing, never invented. Most of these figures are from memory in the research and are checked
   against the primaries named before they become code.
3. **Coarser levels bake weights, the finest level bakes the class.** A coarse patch that evaluated
   the class from its own band-limited slope would lose every cliff, since smoothing flattens them,
   and a cliffed range would change colour as the camera closed. So `weights_at` at level n gives
   the expected fraction of each finest-level class over the texel's footprint: for the slope rule,
   the probability that the finest slope exceeds `CLIFF_SLOPE`, from the resolved slope at level n
   and the variance of the slope in the bands finer than n, from R09's per-cell level-keyed closed
   forms (`Synthesiser::omitted_variance` and `omitted_structure_function`, per contribution, what
   level n leaves out of the finest level's ground; added by R10.T5,
   `decision-r10-t5-closed-forms.md`); for the rules on altitude,
   the same with the height variance. At the finest level the weights are the class, blended
   linearly across `TRANSITION_HALF_WIDTH` either side of each threshold for the image only, and
   their largest weight is always the authoritative class, ties broken by discriminant. A test holds
   the weights at every coarser level to within ten percentage points of the fractions counted by
   brute force from the finest level. This is the plan's reading of the brainstorm's rule that the
   shader never re-decides the material: the worker decides everything, and the shader only sums, by
   the weights it was handed, the radiances its classes reflect, each through its own law
   (Design note 8).
4. **Collision reads one function.** The brainstorm defines the authoritative ground as "the
   piecewise-linear surface through the finest level's vertices, on the same triangle diagonal as
   the mesh". R05 builds it, `finest_surface_height`, for its test planet, with the patch bake and
   the bound's role in selection. This plan keeps R05's code and names, points them at R09's
   `Synthesiser`, and wraps the interpolant as `ground_at` (the name R11 consumes), returning the
   triangle and its normal as well as the height. Across a cube-face edge the point belongs to the
   face R05's `xyz_to_face_uv` gives, ties broken as its Design note 2 says. The chord's sagitta at
   0.5 m spacing is L² ÷ 8R, about 5 nm at an Earth's radius, so interpolating in face coordinates
   or in space differs far below the 2 mm target. What this plan adds is goldens on generated
   worlds, the face-edge and corner cases on real fields, and the end-to-end agreement with the mesh
   (T15). R09's Design note 18 also gives this plan the retirement of R05's provisional height
   function from the client path; the spike keeps it for its own reproducibility.
5. **The stated bound is R09's hard bound; selection may take a statistical one** (researched
   2026-09-29). R09 provides `level_bound_m(header, level)`, a hard bound summed over the omitted
   contributions, and R05's Design note 15 makes a hard bound the selection contract with an
   explicit calibration factor as its fallback if it proves loose. Real terrain is where looseness
   shows. For summed octaves of gain g the hard tail is about B a_(n+1) ÷ (1 − g) and its RMS about
   σ_b a_(n+1) ÷ √(1 − g²), so with noise normalised to B ≈ 1 and σ_b ≈ 0.2–0.3 the observed 99.9th
   percentile over the hard bound is expected at about 0.3–0.5, lower with craters, whose bound is
   the largest omitted crater's depth that few points see. Patch count scales as the inverse square
   of that ratio, four to eleven times a perfect bound. _Measured by R10.T4.a (Risks, "Recorded per
   world and level (R10.T4.a)" and its "Deferred findings"): the 99.9th percentile is 0.000–0.074
   of ε on every sample world and level, not 0.3–0.5, so the stated bound draws some 180 to 28,000
   times a perfect bound's patches at most levels; and the deviations are heavy-tailed, so 4σ is
   no bound either (R10.T4.b's record applies item 30's rule to them)._ _decision-r10-item30
   (2026-10-26) keeps the hard bound at RM5. At the high setting's 604 patches its drawn error is
   1.3–3.8 px a pose (mean), 1.1–1.3 times a perfect level-wise bound's at the same budget, so the
   budget, not the bound, limits the picture. The 38–178 px τ′ is the stated error, not the drawn
   one (R10.T4.c's record; `levelBounds.test.ts` guards the ratio at 1.5)._ The reference renderers
   bound it the same way: chunked level of detail (Ulrich 2002) stores a hard maximum vertical error
   per chunk and refines while ρ = δ K ÷ D > τ; CDLOD (Strugar 2009) and Proland are
   distance-based; CesiumJS's geometric error for heightmap terrain is a horizontal-spacing
   heuristic, `getEstimatedLevelZeroGeometricErrorForAHeightmap`, with `maximumScreenSpaceError`
   defaulting to 2 px (verified in `TerrainProvider.js` and `Globe.js`). So the hard bound stays
   the stated and tested bound, which is what a sensor's stated error means. The candidate
   calibration, ruled to
   apply by T4.a's and T4.b's numbers (`signoff-12.md`, item 30), is that selection takes the
   smaller of the hard bound and 4σ of the omitted bands, σ from R09's level-keyed closed forms
   (`Synthesiser::omitted_rms` at the level, added by R10.T5). Over a patch's 4,225 vertices, taken
   as independent Gaussian deviations, the 1 − 1 ÷ 4,225 quantile of |x| is Φ⁻¹(1 − 1 ÷ 8,450)
   ≈ 3.68σ and the expected maximum of |x| about 3.81σ (median 3.77σ), so 4σ is a thin margin: one
   patch in four or so has a vertex past it, P(max |x| > 4σ) = 1 − (2Φ(4) − 1)^4,225 ≈ 0.23, and
   a bound exceeded in one patch in a thousand would need about 5.2σ (researched 2026-09-29; the
   quantiles computed, high confidence; real deviations are correlated and skewed, which T4.a's
   recorded maxima test). It is
   chunked level of detail's "maximum error within the chunk" taken statistically, it saves two to
   three times the patches, and what it lets through is a sub-τ pop that morphing hides; collision,
   which reads the finest level, and the readouts, which state 1σ, are untouched. Until the ruling,
   selection uses the hard bound. A per-patch bound, each bake computing its children's bounds from
   the amplitudes over their footprints as chunked level of detail stores δ per chunk, is adopted
   only if T4.a shows body-wide and per-patch bounds differing by more than 1.5 times. _As accepted
   by main for R09.T8, and as built (`c280f604`; R09's "Deviations in T8, as built"; updated
   2026-10-15): `level_bound_m` is hard in its octaves and channels, and its crater and rim terms
   are a 1 − 10⁻⁹ per-point Chernoff quantile, so "the hard bound" here reads as that stated bound;
   the finding is in Risks ("Re-validated at `c6d77cac`")._
6. **Knowledge gates coverage for the image, and coverage and resolution for every number**
   (researched 2026-09-29). The client holds exact cells wherever it holds any, so the image of
   surveyed ground is the ground at whatever detail the camera resolves; the survey's resolution
   never changes a drawn height. What the resolution changes is what a readout may quote.
   `surveyed_reading` evaluates the height at the band level matching the survey resolution of the
   point's cell and states as its uncertainty R09's `omitted_rms` at that level, one standard
   deviation of every contribution the level leaves out of the finest level's ground, each at the
   level's own cut, not `unresolved_rms` at `λ_n` (R10.T5, `decision-r10-t5-closed-forms.md`). It is 1σ and not an interval such as LE90 (1.6449σ, a
   Gaussian figure), because the unresolved heights are not Gaussian (craters and channels are
   skewed and heavy-tailed), because 1σ is the only figure the closed forms give, and because
   current practice states RMS: HiRISE's expected vertical precision (Kirk et al. 2008), MOLA and
   LOLA shot precision, and the ASPRS positional accuracy standards' second edition (2023). LE90
   survives mainly in military and legacy terrestrial products (DTED; SRTM's 16 m absolute
   specification). The value is rounded to its uncertainty by the Particle Data Group's rule: take
   the uncertainty's three leading digits; 100–354 keep two significant figures, 355–949 one,
   950–999 round up to 1000 and keep two; the value takes the same decimal place. So
   `~2140 m ± 180 m` and `~2100 m ± 500 m`. Neither number goes finer than the quantity's own
   precision; where the rule would, the uncertainty is rounded up to one step (the guide's Estimated
   exception, signoff-9). When the survey reaches the band limit the uncertainty is zero, the value
   is the ground itself at the ship-wide elevation precision of 0.1 m, and both the `~` and the
   `± …` go. Unsurveyed ground has no reading: `NOT SURVEYED` stands once in place of the readings
   (signoff-9).
7. **Slope and slant range state their basis** (researched 2026-09-29). The slope is the
   adirectional slope, the gradient magnitude from central differences over a baseline b along the
   local east and north axes; a two-point profile slope is smaller by about √2 on isotropic terrain
   (Shepard et al. 2001, as Beyer, McEwen and Kirk 2003, JGR 108, 8085, ¶25, cite it; corrected
   2026-10-14), and the adirectional one is what tilts a lander. Each unresolved band
   of variance σ_k² at wavenumber k adds per axis v = Σ 2 σ_k² (1 − J₀(k b)) ÷ b², the structure
   function at lag b (Shepard et al. 2001), which tends to 2σ_k² ÷ b² for bands much finer than b
   and to the band's gradient variance for bands much coarser; craters and channels take R09's
   structure function at lag b, not only a variance: the level-keyed one, of what the reading's
   level leaves out (`Synthesiser::omitted_structure_function`, R10.T5). The slope magnitude is then Rician
   about the resolved slope |g₀|, biased upward when |g₀| is below about 2√v, so the readout quotes
   the RMS slope √(|g₀|² + 2v) with the Rice standard deviation as its uncertainty; quoting |g₀|
   alone would understate slopes from orbit, the unsafe direction. The baseline is shown with the
   value: 2 m by default, the band limit, and the scale a lander's tilt is judged over
   (signoff-10), with 5 m and 100 m selectable. The first two are the lander-stability baselines of
   InSight's site selection, slopes at 2–5 m from HiRISE DEMs against a 15° limit (Golombek et al.
   2014, LPSC 45, 1499, verified), and the third stands near the same limit's 84 m scale for the
   landing radar (Golombek et al. 2017, Space Sci. Rev. 211, 5, §2.7); the 2014 abstract also reads
   slopes at about 300 m from MOLA (corrected 2026-10-14 to `SLOPE_BASELINES`' citations in
   `reading.rs`; "Deviations in T5, as built").
   The slant range to the ground on the view's line of sight is a march of the ray against the
   surface at the survey's resolution. Linearising the hit of p(t) = o + t d on z = h + δ gives dt ÷
   dδ = cos θ_s ÷ sin γ_s, with θ_s the resolved surface's slope and γ_s the grazing angle to the
   local surface plane, so σ_R = σ_h cos θ_s ÷ sin γ_s. The linearisation fails once unresolved
   relief can occlude the hit, so when sin γ_s < 2√v at b equal to the survey's cell size the
   readout drops its σ and says `GRAZING`: the true range is then biased short and its error
   one-sided. Consoles that draw no terrain would need a server request for the same numbers; none
   exists yet, and when one does it wraps these functions unchanged.
8. **One photometric law for every class, and the disc and terrain share it** (researched
   2026-09-29). Every class takes McEwen's lunar-Lambert law, r(i, e, α) = A_N f(α) [2 L(α) μ₀ ÷
   (μ₀ + μ) + (1 − L(α)) μ₀], with f(0) = 1, μ₀ = cos i, μ = cos e and A_N the normal albedo; L = 1
   is Lommel–Seeliger and L = 0 Lambert. For the Moon L(α) = 1 − 0.019 α + 2.42 × 10⁻⁴ α² − 1.46 ×
   10⁻⁶ α³ in degrees (McEwen 1996, LPSC XXVII, 841, Table 1, the same at 0.56 and 0.76 µm, fitted
   to Galileo SSI images at 19.5°–101° with L(0) = 1 imposed; the coefficients are USGS ISIS's
   `LunarLambertMcEwen.cpp`; McEwen 1991, Icarus 92, 298, is the earlier study of L(α) from Hapke's
   model). It reaches 0 at 103.9° and L is held at 0 beyond, where the fit has no data. McEwen 1996
   finds no difference in limb darkening between maria, highlands and bright Copernican craters, so
   across regolith albedos L(α) does not follow A_N, and only the brightest surfaces are
   limb-darkened at zero phase (Buratti 1984, Icarus 59, 392; decision-phase-curves; Buratti and
   Veverka 1983, Icarus 55, 93, propose an albedo-dependent form, which McEwen 1996 rejects for the
   Moon). The dark and moderate classes take the Moon's L(α); the bright ices' and evaporites' L(α)
   is settled in T1.a against the primaries and the A_h ≤ 1 bound below. Hapke's full model is not
   used per texel: its roughness term counts relief the mesh, normals and horizon map already
   resolve, and what it adds lies below the band limit. It is the law R07 adopts (its
   Design note 5), so R07's `body_brdf(law: LunarLambert, mu0, mu, alpha)` carries it and this plan
   calls it once per class, with that class's `LunarLambert` (T10.b). Each law's L(α) is tabulated
   beside its f, in the alpha channel of its `phase_factor_table` row, and f against that L(α):
   Φ_shape(α; L) = [L(α) Φ_LS(α) + ⅔(1 − L(α)) Φ_Lam(α)] ÷ [L(0) + ⅔(1 − L(0))], so p is unchanged,
   and inside each template's range Φ and q are too and only the limb profile moves; past it the
   held f draws L(α)'s own tail, which lowers q by at most 6.8 × 10⁻⁴ (R10.T10.b's record);
   `body_brdf` reads L from the row and `LunarLambert.l`
   is L(0) (decision-r07-t8b). A texel reflects its classes' `body_brdf` radiances summed by weight,
   each law through its own row, as R07's disc sums its class map's, not one law built from weighted
   parameters, which differs from the sum wherever classes of different A_N and L blend (1.7× at
   μ₀ = 0.5, μ = 0.1 for an even blend of regolith at L = 1 and snow at L = 0). At zero phase a
   uniform body's geometric albedo is p = A_N [L(0) + ⅔ (1 − L(0))], and averaged over viewing
   directions a patterned sphere's p is the plain area average of each element's, so the scale c on
   the class albedos that makes the body's p is the root of Σ a_i c A_N,i [L_i + ⅔ (1 − L_i)] = p
   over the classes' area fractions a_i. Clamping to the classes' ranges makes it piecewise linear
   in c, solved exactly between sorted breakpoints. The server holds the whole field, so
   `albedo_scale` runs there, on every cell, and c ships with the field in R09's
   `FieldHeader.albedo_scale`: the terrain's brightness then never shifts as the survey grows, and
   one scalar reveals nothing the disc's brightness does not. Matching p at zero phase is not enough
   for a hand-over that changes geometry only, since the laws differ at other phases and the pattern
   differs everywhere (an Iapetus, a Mars albedo map). So R07's disc samples a per-body coarse
   class-weights map, through the `class-map` case of its `DiscSurface` (R07.T8.b; filled by T10.d),
   T2's weights at level 4 or shallower, a few hundred kilobytes, through the same law, and disc and
   terrain agree at every phase by construction. The map covers surveyed cells only; unsurveyed
   texels take R07's uniform `lawFor(p, q, template)`, so the disc shows no pattern the ship has not
   seen. Plan 14's phase integral q then becomes a result of the classes, reported with its residual
   as the albedo's is. A_N is the normal albedo, the radiance factor at i = e = α = 0 (Hapke 2012,
   §10), in V; for a Lambert surface it equals the hemispherical albedo, and the law gives r = A_N
   there. Terrestrial snow and ice albedos are broadband (0.3–2.5 µm) hemispherical figures, which
   understate the visible because ice absorbs in the near infrared, so they are converted before
   use: the spectral albedo near 0.55 µm, at normal incidence, times the nadir anisotropy R(0, 0) ÷
   A_h (about 0.9–1.0 for snow, low confidence), and since bright classes have L near 0, A_N ≈ A_h
   after that (researched 2026-09-29: Warren 1982; Wiscombe and Warren 1980; Grenfell et al. 1994;
   Warren et al. 1998; Hudson et al. 2006). Each class's A_N and L together must reflect no more
   than they receive: the Lommel–Seeliger term's directional-hemispherical albedo is 4 A_N [1 − μ₀
   ln(1 + 1 ÷ μ₀)], 1.23 A_N at μ₀ = 1, so a bright A_N with a sizeable L breaks A_h ≤ 1, and T1.a
   tests it. Starting parameters per class, all settled in T1.a against the primaries (most from
   memory in the research): regolith A_N 0.07–0.20 with the lunar L(α); basaltic rock 0.05–0.15,
   felsic 0.2–0.35; dry soil and sediment 0.10–0.30, and wet ground about 0.5–0.65 times its dry
   albedo, the ratio rising with the dry albedo, by total internal reflection in the water film
   (Ångström 1925; Lekner and Dorf 1988, Appl. Opt. 27, 1278; Twomey et al. 1986), though the
   classifier has no wet state until R09's climate gives one; desert sand 0.25–0.45, Martian dune
   sand 0.10–0.15; Martian bright dust 0.18–0.25 in V (its broadband 0.25–0.32 is redder: Kieffer et
   al. 1977; Christensen et al. 2001; Mustard and Bell 1994; Bell et al. 2000); snow 0.95–0.98 fresh
   and 0.85–0.93 aged in the visible (Wiscombe and Warren 1980; Grenfell et al. 1994), against
   Warren 1982's broadband 0.80–0.95 and 0.5–0.7; clean glacier ice about 0.5–0.65 visible (Cuffey
   and Paterson 2010, Table 5.2, gives 0.34–0.51 broadband); sea ice about 0.75–0.85 bare and
   0.93–0.97 snow-covered in the visible (Perovich et al. 2002's broadband 0.5–0.7 and 0.80–0.87);
   nitrogen and methane frost 0.8–1.0 (Buratti et al. 2017, already normal reflectances in LORRI's
   band); carbon dioxide frost 0.4–0.8; organic sediment 0.05–0.2; evaporite 0.5–0.8. A range is the
   class's physical span; the class's value before scaling is its midpoint, which the scale c
   multiplies and the range then clamps. `Melt` is emissive, which is R07's or R11's, and `Seabed`
   lies under the ocean pass. _Folded in at the re-validation (decision-composition §1.8 and §1.9):
   a texel's A_N in B, V and R and its phase row are its material's palette entry's, which the
   server resolves (R09's `PaletteEntry::normal_albedo_bvr` and `phase_row`, every row 0 until T1.a
   assigns the codes), in place of the per-class figures above, which stay the classes' physical
   spans and T1.a's check on the entries; the ground's thermal emission, `Melt`'s and any cell's
   above R08.T18's visibility cut, is R08.T18's shared function, called by T10.b. How a form
   changes its entry's A_N (fresh snow and glacier ice of one H₂O entry) was a question in Risks,
   answered by a per-form factor in T1.a's table ("Deviations in T1.a, as built")._
   _Folded in from decision-r10-limb (2026-10-13; built under T1.a): "bright classes have L near
   0" becomes two kinds of form, and L(0) is a measured property of how the surface is built, not
   an energy bound's. The consolidated forms (`Glacier`, `Snow`, `SeaIce` and `Evaporite`, of any
   species or salt) take the Lambert law, `PhaseLaw::Lambert` (R07's `lambert` template: f ≡ 1,
   L ≡ 0, q = 3/2), which the form chooses whatever the entry's code; their measured spectral
   hemispherical albedos are their A_N with no conversion, since the law's A_h is A_N at every
   incidence, and the nadir anisotropy above is the law's error. The particulate forms (every
   other form, `Frost` of any species included) keep the entry's lunar-Lambert row, with L(0) =
   L_p(c A_N): 1 up to A_N 0.60 (Buratti 1984, as Buratti et al. 2017, p. 12, reports it), falling
   as 0.6696 ÷ x − 0.1160 to 0.38 at Enceladus's 1.35 (Verbiscer and Veverka 1994's Hapke
   parameters), and 0.38 beyond. The A_h ≤ 1 bound above is the f ≡ 1 case, exact only for the
   Lambert law. The bound is on the whole law with its phase factor: for every μ₀ in (0, 1],
   A_h(μ₀) = (A_N ÷ π) ∫ f(α) [2 L(α) ÷ (μ₀ + μ) + 1 − L(α)] μ dΩ ≤ 1. Energy is enforced on A_N,
   never through L, by each law's ceiling (`PhaseLaw::normal_albedo_ceiling`): 1 for Lambert's,
   exact, and 1.35 for the particulate rows, against which the Ice row's A_N × sup η is 0.97 under
   the rule. A normal albedo above 1 is physical (Enceladus's is 0.9–1.4). See "Deviations in
   T1.a, as built"._
   _The disc, recorded by R10.T10.g (decision-r10-t16-leans §1): R07's disc in its `class-map`
   case shades surveyed ground with the field's own normals as well as its classes, a per-body
   normal map at the field's level made by the workers beside the class map (one normal a
   surveyed cell, the height's at band max(L − 6, 0) at the cell's centre: the coarsest patches'
   doubled normals there, bit for bit), filed under `coarse-field-gpu`; each class's law takes
   μ₀ and μ on that normal while the hit, the limb and the projected area stay the spheroid's, and
   T10.f's point integral does the same. So disc and terrain agree at every phase by construction
   in their laws, classes and cell-scale slopes, except where the disc floors μ at half the
   spheroid's (a cell whose slope faces more than about 24° away from the camera at 90° of phase,
   near the limb, where the terrain draws its facet hidden or black); what the disc still leaves
   out is the sub-cell relief's shading and its cast shadows (Risks, "Deviations in T10.g, as
   built")._
9. **No textures in this plan.** The Materials section lists triplanar projection of rock and detail
   textures and two-scale repetition hiding. Anything a texture adds below the band limit is detail
   the simulation did not compute, which the brainstorm defines as decoration, and R11 owns it,
   zero-mean about this plan's class albedo, through the `terrain_decoration` hook of
   `terrainLit.wgsl`. This plan's lit terrain is shaded from baked data alone: the height, the
   normals at twice the mesh resolution on the high setting (R05's `NormalScale`), the class weights
   and their photometric parameters. It is the undecorated ground R11's decoration switch turns off
   to.
10. **Horizon maps are sun-independent, linear 16-bit, and their far field is a shared lattice**
    (researched 2026-09-29). A horizon map stores, per texel and azimuth, the horizon's elevation
    (Max 1988; Sloan and Cohen 2000), and the test compares the sun's elevation in that azimuth
    against it; nothing depends on the light, so a patch never rebakes for the sun or the body's
    rotation (the brainstorm's "needs rebaking only as the sun moves" is reported in the notes).
    Eight azimuths, interpolated linearly in azimuth; sixteen would double the memory, and the
    by-hand sunset decides whether narrow distant peaks need them. The encoding is the elevation
    itself, linear, as a signed normalised 16-bit value s = e ÷ 90° over [−90°, 90°], two azimuths
    to a `u32` word through WGSL's core `pack2x16snorm` and `unpack2x16snorm`, four words (16 bytes)
    a texel in the slot's storage buffer (Design note 15), and filtered bilinearly by hand from four
    reads (researched 2026-09-29, on IEEE 754 binary16 and the WGSL and WebGPU specifications). Its
    step is 180° ÷ 65,534, 9.9″, and its worst round-trip error 4.95″ at every elevation, signed for
    horizons below zero on peaks. The alternatives fail: eight bits of sin(e) step 0.45° against the
    Sun's 0.53° penumbra at 1 au, so sunsets would band; and any half-float encoding keeps a
    relative precision of 2⁻¹¹, so sin(e) in `rgba16float` steps 0.032° at 30°, 0.056° at 60° and
    0.16° at 80°, and passes half the Sun's radius above about 78°. `rgba16snorm` would filter in
    hardware but needs the optional `texture-formats-tier1`. The rule stated with the encoding
    bounds the image, not the relief: the lit fraction changes fastest at mid-transit, dF/de = 2 ÷
    (πρ), so a quantisation error δ_q moves it by at most 2 δ_q ÷ (πρ), 0.33% for the Sun at 1 au
    (under 1/255). A star is drawn with a soft penumbra only when ρ is at least 20″, about two steps
    (at 32″, a Sun at 30 au, the error is at most 10%, over a band a few centimetres wide); smaller
    stars take a hard step (_not built: R10.T8.b keeps R07's soft edge at every size, which
    converges to the step under 20″, by main's ruling of 2026-10-16; Risks, "Deviations in T8.b, as
    built", "No hard step under 20″"_). The terrain's own sampling and the eight azimuths'
    interpolation err far
    more in rough ground; the rule governs banding only. The near field is marched from the patch's
    own 65 × 65 heights, about 2.2 M compares, an estimated 2–6 ms in WebAssembly (provisional; T8.a
    measures it), or by Timonen and Westerholm 2010's convex-hull line sweep, O(n) per direction, if
    the march breaks the budget (_as built by R10.T8.c: the sweep reads a margin of 8 vertices of
    the neighbours' heights beyond each edge, Risks, "Deviations in T8.c, as built"_). The far field
    is not rays through the height function, which at 81
    points × 8 azimuths × about 45 steps is some 29,000 evaluations, an estimated 60–120 ms: each
    far sample is the bilinear interpolation of the level-m vertex heights, m chosen so the vertex
    spacing is about the step length times the growth factor less one, memoised by (face, level, i,
    j) in the worker's `SynthCache`. The far-field points lie on a face-wide lattice at a patch
    edge's spacing ÷ 8, defined by position alone and not by the patch's level, so points on a
    shared edge are the same point, and between patches of one level on one face the same value, so
    that the far field opens no seam there. Across a level change the finer map differs by its
    receiver's height and its finer near ground, as the near field does, and the shader blends a
    patch's map to its parent's by the morph factor, as the geometry is blended (T8.b). Across a
    face edge the azimuths turn with each face's grid, and the two sides differ by the eight
    azimuths' interpolation (main's ruling for R10.T8.a, 2026-10-15). Steps grow by 1.15–1.25
    from one texel beyond the patch edge, the curvature drop d² ÷ 2R is included, so an obstacle at
    d stands at arctan((h − h₀ − d² ÷ 2R) ÷ d), and the march stops at √(2 R Δh), Δh the body's
    realised relief range: about 505 km for an Earth with 20 km of relief. The shader takes the
    larger of near and far. The lit fraction is the share of the star's disc of angular radius ρ
    above the horizon, 1 − (arccos x − x √(1 − x²)) ÷ π with x = (e_sun − e_horizon) ÷ ρ clamped to
    [−1, 1] (checked: 0, ½ and 1 at x = −1, 0, 1), without limb darkening, which moves the
    half-light point by under 0.1ρ. On terrain R07's horizon term (`sphere_irradiance`, its Design
    note 6) takes its local horizon per disc sample from this map, through the `horizon` argument of
    R07's `sphere_irradiance(h, phi, horizon)` (R07.T6.a, T6.c; 0 on the smooth figure), carried
    into the element's frame as e − (e★ − (π ÷ 2 − φ)), so that R07's cut, whose visible share is
    this lit fraction, lies at x = (e★ − e) ÷ ρ (decision-r10-t8b §1), with R07's oracle as its test
    oracle; T8.b fills it. Where R08 draws the sun at its refracted elevation (on an Earth about
    0.57° at the horizon, more than the Sun's disc, and 0.16° at 5°; Bennett 1982), the shadow test
    uses the same apparent elevation, or shadows and the drawn sunset disagree by more than a solar
    diameter (as built R08 draws no refraction, so the test takes the geometric elevation until it
    does, R08's Risks). Max, Sloan and Cohen, Stewart 1998, Timonen and Westerholm and Bennett are
    from memory in the research; T8.a's bench and the by-hand sunset settle what they support.
11. **Terrain shadows are this plan's on both settings, and combine by minimum** (researched
    2026-09-29). The brainstorm's ladder gives the high setting cascaded shadows "with cloud shadows
    on terrain" and the low one the horizon map, and its budget's third rule builds the low setting
    beside the high. No plan in the set names the cascades, and terrain self-shadowing is this
    plan's feature, so the reading closest to step 8 is that both are built here; R11's cloud
    shadows multiply into the result through `terrain_shadow_factor` and a ringed body's ring shadow
    through `terrain_ring_shadow`, both on the direct sun only; the diffuse light a cloud deck
    passes reaches the ground as `terrain_sky_factor` on R08's `atmosphere_sky_irradiance`, so an
    overcast sky does not draw a sharp half-bright sun (R11's Design note on cloud shadows). The
    horizon map is applied everywhere on both settings; on the high one the lit fraction is
    min(horizon, cascade) where a cascade exists: the horizon's share m is R07's visible share at
    the cut it used, `disc_visible_fraction(h, φ + max(η, 0))`, and the cascade's c enters
    `terrain_shadow_factor` as min(1, c ÷ m), 1 where m = 0 (decision-r10-t8b §2), so cascades add
    what the horizon map cannot, casters that are not terrain (craft, R11's rocks and scatter) and
    sub-texel detail near the camera, and there is no seam at the last cascade, which only fades to
    lit over its last 10%. Cascades switch off when cascade 1's texel is coarser than the horizon
    map's, as in orbit. Splits follow Zhang et al. 2006's practical scheme, Cᵢ = λ n (f ÷ n)^(i ÷
    N) + (1 − λ)(n + (f − n) i ÷ N) with N = 4 and λ ≈ 0.75, over a shadowed range f = min(view far,
    max(2 km, 20 × height above ground)), continuous in that height. Each cascade is fitted to a
    bounding sphere of its frustum slice, whose radius does not change as the camera turns, snapped
    to whole texels (Valient 2008), with every light matrix built camera-relative in `f64` and
    narrowed to `f32`, since snapping at planetary coordinates would jitter. _As built by R10.T9.b,
    the cascades' light is the sun's disc's top edge, not its centre (decision-r10-t9b-penumbra):
    a receiver is in their shadow only where the disc's top edge is hidden, where m has reached 0,
    so min(m, c′) is m across the whole penumbra; the receiver's N·L, its offset and its shading
    keep the true sun (Risks, "Deviations in T9.b, as built")._ An orthographic light
    projection has linear depth, so reversed-Z buys nothing there: the maps are `depth32float`,
    whose worst step over a 50 km range is about 3 mm, and keep the project's greater-equal
    convention for uniformity only, which the code says so that no one tunes bias expecting
    precision. Casters in front of the near plane are pancaked through `primitive.unclippedDepth`
    where the adapter offers `depth-clip-control` (the UHD 620 offers it under the forced switches,
    by R01's probe of 2026-09-29, and so does SwiftShader, as R10.T9.b found, whose smoke withholds
    it on the `no-subgroups` variant so that both paths run; cascades are the high setting's alone;
    decision-r10-t9-split), and otherwise the near plane is pulled towards the sun by
    Δh ÷ sin(e_sun), capped. Acne is held by slope-scaled bias (`depthBias`,
    `depthBiasSlopeScale`, `depthBiasClamp`) and a normal offset of 1–1.5 texels of the cascade's
    world-space size; filtering is 2 × 2
    hardware PCF through `textureSampleCompare` (_as built `textureSampleCompareLevel`: the
    receiver runs in non-uniform control flow, where WGSL allows only that form_) with a few
    Poisson taps, since the Sun's penumbra at
    10 m from an occluder is about 9 cm and PCSS is unneeded. Four 2,048² `depth32float` layers are
    64 MiB, the brainstorm's figure. Zhang et al.'s formula, Valient's snapping and the bias recipes
    are from memory in the research and are checked against the sources in T9.a.
12. **Unsurveyed ground is the reference surface, and its edge is marked.** A patch's vertices whose
    coarse cell is not surveyed (R09's margin cells are held but not surveyed) carry a survey mask
    of zero, and the terrain shader discards fragments whose interpolated mask is below one half.
    Beneath them R07's `mesh`-regime body, the reference spheroid of its `BodyFigure`, is drawn with
    its own fragments discarded where the GPU's coverage mask says surveyed, so no terrain is
    claimed there and no reference surface shows through a surveyed valley. Terrain heights are
    measured along that spheroid's normal (R07's Design note 19; R09's Design note 17), so the two
    surfaces share one datum and the skirt spans only the terrain's own relief at the edge. Where
    terrain and reference surface meet at different heights, a skirt along the survey edge joins
    them, drawn in the reference surface's appearance, so the edge reads as an edge rather than as a
    cliff in the ground. The edge itself is symbology: a mark over the image, cased under guide item
    3, in both styles. The coverage mask is the brainstorm's "coarse field, GPU copy", one byte per
    coarse cell per face, at most 384 KiB at level 8.
13. **The disc hands over to terrain by the body's own relief.** R07 draws a body in its `disc`
    regime until R09's `realised_relief` subtends τ pixels; nearer, the `mesh` regime hands its
    geometry to R05's quadtree, from the root faces down, with τ the style's own: 1 px on the high
    setting, 2 px on the low and 4 px in the wireframe. At an Earth's 10 km of relief either side of
    the datum, over 1080p across 60° (5.45 × 10⁻⁴ rad a pixel), that is about 1.8 × 10⁷ m at 1 px
    and 4.6 × 10⁶ m in the wireframe. The first lies near the brainstorm's 2 × 10⁷ m (Two styles of
    one renderer), but that figure is a different one, where a 35 km coarse cell spans 4 px, and
    agrees only by coincidence. The resolved slopes and shadows at that level are negligible, which
    is why the same smooth law holds on both sides of it (Design note 8). _Not so at high phase: the
    fixture's level-6 cells, resolved over 2 to 3 px at the hand-over, change by up to 0.42 stop at
    90° (T16.d, F2), the shading of their mean slope. R10.T10.g gives the disc the field's normals
    (decision-r10-t16-leans §1)._ _As built by R10.T10.g, the premise recorded: the disc now shades
    surveyed ground with the field's own normals, one a surveyed field cell (the coarsest patches'
    cell-scale slope), so the law, the classes and the cell-scale slopes are the same on both sides
    of the switch, and the hand-over changes the geometry and the sub-cell shading alone. The
    quadtree takes the ground from the root faces down, but its steady terrain at the hand-over is
    the field's level, not the roots: on TERRAIN TEST (15.8 km of relief) selection keeps level-6
    patches, 64 quads a side over a cell, since level 5's bound (17.1 km) exceeds the relief, so
    the roots are only its first draw. The 0.42 stop of F2 was taken with the switch at about 94.5°
    and 102° of phase; the committed check (`--handover`) at 90° measures the figures in
    "Deviations in T10.g, as built"._ A body with no survey never hands over.
14. **The wireframe's contours are chosen as a scale is** (researched 2026-09-29). The wireframe
    draws its terrain once, depth only, at 4 px, and writes colour only where the interpolated
    height crosses a contour level. The interval is one deterministic rule of pose alone, so every
    client with the same camera states the same one: the 1-2-5 step nearest, in log terms, to a
    twentieth of the camera's height above the surveyed ground at its nadir (above the datum where
    the nadir is unsurveyed), never below 1 m. The nadir, not the view's centre, so the interval
    does not jump as the camera tilts towards the sky. A twentieth keeps steep ground legible: at a
    30° look-down the view's centre is at about twice the height, where a 1:1 slope's contours come
    about 20 px apart, and a fiftieth would crowd them to 8 px. It steps up when a twentieth of the
    height passes the geometric midpoint to the next step by 20%, and down 20% below it. Every fifth
    contour is an index contour, as on USGS maps; sea level on an ocean world is the coastline,
    drawn at R09's header `sea_level` whether or not it is a multiple of the interval. Widths, since
    colour alone may not carry meaning: minor 1 px and index 1.5 px in `--text-muted`, coastline 2
    px in `--text`; never dashed, since the guide reserves dashes for predictions; no contour labels
    in the image, since the readout's `ELEVATION` and the stated datum carry the values. The line is
    antialiased at constant width from u = (h_local + φ) ÷ CI: dist_px = |fract(u − 0.5) − 0.5| ÷
    length((dpdxFine(u), dpdyFine(u))), coverage = clamp(w ÷ 2 + 0.5 − dist_px, 0, 1), the
    gradient's length rather than `fwidth`, whose L1 norm varies widths by up to √2 with direction
    (after Golus 2021). The derivatives are taken in uniform control flow, before the survey mask's
    `discard`. A family is drawn in full at 6 px of spacing or more and fades out as its spacing
    closes to 3 px (CSS pixels), minor contours first, and is not scored for contrast in its fade;
    the guide says that surveyed ground without contours lies within one interval or is too steep
    for them, so that an absent line is not read as flat ground; this is the grazing case, where
    spacing collapses faster than any interval can follow. The height varying is patch-local, h −
    h_patch, with a per-patch phase φ = h_patch mod CI formed in `f64` on the CPU, because the `f32`
    height above the reference spheroid steps 2 mm at ±20 km and would quantise the derivative near
    the camera. The label block states `CONTOUR INTERVAL 200 m · INDEX 1 km`, the USGS margin form,
    with the `DATUM` on a line of its own in both styles (T12), and guide item 5's substitute for a
    scale bar, scaling to km with hysteresis. Unsurveyed ground has no contours; R02's graticule is
    drawn over the terrain's height where terrain is surveyed. The datum is the body's rotational
    spheroid, R07's reference body, from which every height in the patch is measured along its
    normal (R07's Design note 19, researched there with high confidence; R09's Design note 17
    measures its elevations and sea level the same way), stated as
    `DATUM REFERENCE SPHEROID 6378.1 km × 6356.8 km` with the equatorial and polar radii (a sphere,
    `DATUM REFERENCE SPHERE 6371.0 km`, until plan 14 sends a flattening); the sea level, a height
    above the spheroid, is stated beside it on an ocean world. The h ÷ 20 figure has no published
    precedent for perspective views and is settled by T11's check on steep and gentle terrain; the
    index and margin conventions are from memory in the research.
15. **The cache is sized from a count model and then measured** (researched 2026-09-29). A pure
    count model (a quadtree on an Earth-sized cube sphere, finest level 19 at R05's 17.7 m mean
    patch edge; a patch accepted at a distance of k times its level's edge, k = 5 at 1 px 1080p and
    60°, 1.67 at the low setting's 2 px at 720p, 1.25 for the wireframe's 4 px; exact horizon
    culling; frustum 60° × 34°) gives, all-round over a full turn and in the frustum:

    | Camera height | High, 1 px 1080p | Low, 2 px 720p | Wireframe, 4 px 1080p |
    | ------------- | ---------------- | -------------- | --------------------- |
    | 100 m         | 1,468 / 246–268  | 332 / 80–88    | 284 / 76              |
    | 1 km          | 1,216 / 200–210  | 260 / 64–68    | 212 / 52–56           |
    | 10 km         | 936 / 144–164    | 208 / 50–54    | 148 / 38–42           |
    | 100 km        | 604 / 92–116     | 152 / 38–42    | 104 / 26–30           |
    | 400 km        | 424 / 78–92      | 116 / 30–32    | 68 / 18–20            |

    A full level ring holds about 180 patches at k = 5. Quadtree granularity floors a ring at about
    36 patches once k falls below about 2, so the low setting selects about a quarter of the high
    one's patches, not a ninth, and the wireframe about a fifth, not a sixteenth (the brainstorm's
    figures, which hold for demand only while k exceeds about 3). Relief, the 2:1 neighbour rule and
    the forced region under grounded bodies are not modelled and add perhaps 20–40%. A patch uploads
    R05's heights and parent heights, 33.8 kB (65 × 65 × two `f32`, read from the slot's storage
    buffer, so `float32-filterable` is not needed); normals, 16.9 kB at mesh resolution or 66.6 kB
    at twice it (`rg16float` octahedral, core and filterable, angular error about 0.05°); class
    weights, 33.8 kB (two `rgba8unorm`); the horizon map, 67.6 kB (four `u32` words a texel, Design
    note 10); and the survey mask, 4.2 kB (one byte a vertex, four to a `u32`): about 156 kB on the
    low setting and 206 kB on the high. R05's `BakedOffsets` vertex path adds about 101 kB a patch
    while it stays R05's default. The sizing rule is max(1.3 × the all-round peak, 3 × the frustum
    peak), summed over the views that stream, plus the forced region. On the low setting that is
    about 430 slots, which 64 MiB holds at 156 kB with nothing to spare, and not with
    `BakedOffsets`; on the high setting 3 × the frustum peak is about 800 slots, 165 MB, but the
    rule's other term governs: 1.3 × the all-round peak at 100 m is about 1,900 slots, 390 MB. So
    the high setting's budget is about 400 MB, not the brainstorm's 128–256 MB, whose upper end
    holds only the frustum term; it sits inside the discrete ceiling of 2–3 GB and raises the
    brainstorm's GPU total of 0.6–1 GB by about 0.15–0.25 GB. For context only (the budgets stand,
    decided 2026-09-30 by a delegated decision, hardware item 5): the development machine's RTX
    3080 has 10 GiB of VRAM, shared with the local LLM, and the UHD 620 shares its laptop's system
    memory. The frustum term alone would fit 256
    MiB; T14 measures whether the all-round peak ever becomes resident, and adopts the frustum rule
    on the high setting only if it never does, recording which. `writeTexture` has no 256-byte row
    rule (only encoder copies do), so 520-byte rows upload as they are, but tiled layouts on Intel
    may pad a 65 × 65 texture to 1.8–2.2 times its size, so T14 measures driver-reported memory.
    Hence the layout: the cache is preallocated as fixed slots from the byte budget; heights, parent
    heights, the horizon map and the mask, all read by index, never sampled (updated 2026-10-16),
    live in storage buffers addressed by slot index, tight and untiled (the default
    `maxStorageBufferBindingSize` is 128 MiB); each storage binding is visible only to the stages
    that read it, so WebGPU's default eight
    a stage, never raised (Metal's adapters report 10 under Dawn), counts per stage: the lit
    material's vertex stage reads the heights, slot records, instances, contacts and, on
    `baked-offsets`, the offsets, and its fragment stage the class layers and the lights; the
    horizon map and the survey mask are each their own slot field (main's ruling for R10.T10.b,
    2026-10-15); normals and class weights, which want filtering, live in 2D arrays (at most the
    adapter's `maxTextureArrayLayers`, 256 by default) or atlases with a one-texel gutter; and each
    style's terrain is one instanced draw over R05's shared 65 × 65 index buffer with a slot index
    per instance, which also keeps the CPU's per-draw cost down. R05 already packs its octahedral
    normals as core, filterable `rg16float` (its Design note 5; the `rg16snorm` alternative would
    need `texture-formats-tier1`) and already builds `PatchCache` on this layout's fixed slots (its
    Design note 10); this plan supplies the `SlotLayout` and the budget, and the eviction rule stays
    R05's. _As built (R05's "Deviations in T8, as built" and T18): normals are stored with the
    atlas's one-texel gutter, 17,956 B at mesh resolution and 68,644 B at twice it, so a low slot
    is about 157 kB (160 kB with gutters on the class weights too) and 64 MiB holds 420–426 slots,
    under the rule's 430; and R05.T18 kept `BakedOffsets` on the high setting, so a high slot is
    about 310 kB and 400,000,000 B holds about 1,290 slots, against the rule's 1,900. T14 decides,
    as Risks' "The low setting's cache is at its edge" already has it for the low setting._
    _After R10.T1.c's client half: a low lit slot is 178,908 B and 64 MiB holds 375; a high lit
    slot is 330,996 B and 400,000,000 B holds 1,208 (the class layers' maturity codes and nodes,
    19,972 B a slot; Risks, "Deviations in T1.c's client half, as built")._
    _As built by T14.a (`cacheBudget.ts`): the model counts by the selection's own metric, and on
    it, on the exact horizon this note's premise names (`exactHorizon`), the note's high column is
    about 1.6 times low (model ÷ note ×1.53–1.91) and its low and wireframe columns near
    (×0.98–1.25 and ×0.89–1.33); on the boxes' horizon, as `selectPatches` culls until R10.T14.e,
    ×1.75–2.21, ×1.29–1.75 and ×1.36–1.90. The sizing rule's figures from it are T14.a's record's,
    the constants provisional for T14.b (decision-r10-t14a and its #56 follow-up)._
    _As amended by R10.T14.e: on the boxes' horizon the columns are now ×1.60–1.92, ×1.06–1.38 and
    ×1.05–1.72 the note's; on the exact one ×1.51–1.82, ×0.98–1.25 and ×0.89–1.33 (T14.e's
    record)._
    _As measured by R10.T14.b (TERRAIN TEST's moon on the RTX 3080, at the spike's 1,398 × 793 and
    806 × 431 canvases; decision-r10-t14b-constants): on the hard bound, whose k_n is 270–540 at
    L4–L16 against this note's 5, the rule in its union form asks 8,666 high lit slots (2.70 GB),
    2,996 low lit (455 MiB) and 3,764 wireframe, the frustum term governing everywhere and the high
    setting's all-round set never resident (11% at most), so its rule is the frustum rule; the
    constants are held at 400,000,000 B, 64 MiB and 520 slots until R10.T4.b's statistical bound
    and, for the low setting, the UHD 620 run._

16. **The client fetches what the camera can resolve, and nothing more.** A body's field is wanted
    once it is the camera's local body or within the hand-over range of Design note 13 of any view's
    camera, and released when every view has left twice that range. The client sends R09's
    `surface_field` with `have_revision`, and a rise in the body's revision in `surface_revisions`
    on the scene topic sends it again for the new cells only. A chunk here is one of R09's
    self-contained payload blocks (at most `MAX_BLOCK_BYTES`, 1 MiB): the render thread reassembles
    R03's 256 KiB binary frames and, at each block boundary, forwards the block, without waiting for
    the whole payload as R09's Design note 17 allows. Blocks are decoded in the workers, each
    holding a `PartialField` per body: block 0 of every payload, delta or whole, carries the whole
    `FieldHeader` (R09's Design note 17), so `field_init` posts block 0 and `field_chunk` each block
    after it, and no block waits for the terminal `SurfaceFieldDto`. Cells arrive exact and whole
    with their margins, so a new chunk never changes a baked height, and only patches whose survey
    mask it changes are rebaked. Patches waiting on a chunk of a surveyed region count toward R05's
    `TERRAIN: STREAMING`; unsurveyed ground does not, because it is not coming. _As built: R03's
    `requestBulk` hands over whole answers (`onPartial`, then `outcome`), so T7 adds a per-chunk
    option there to forward a block at its boundary; a block's length is the `u32` at offset 6 of
    its 10-byte frame, so the render thread splits blocks without decoding them, and each worker
    decodes one with `wire::decode_block` (R09's "Deviations in T3, as built")._ _As built by
    T6.b, on main's ruling of 2026-10-14 (T6.a's question): a vertex outside the survey carries
    the true ground wherever the field can make it, a margin cell's being the server's to the bit,
    and the reference surface only where it cannot, the bake then incomplete. So a new block never
    changes a value at a vertex the field held before it, surveyed or margin, and a patch is
    rebaked on a block that surveys a cell holding one of its vertices outside the survey (its
    mask changes), or on any block while its bake is incomplete, and on no other block. Each
    worker answers a block with the cells it surveyed anew, and each bake carries its completeness
    and the cells its mask holds unsurveyed (Risks, "Deviations in T6.b, as built")._ _From T8.b
    (decision-r10-t8a §3): also on a block that brings a cell, surveyed or margin, that the patch's
    horizon map's far field was refused for. Each bake carries those cells, and each block's change
    the cells it brought that the field did not hold._ _From R10.T16: the session wants and releases
    the fields from every view's camera (T16.a); selection never refines ground with no surveyed
    cell (T16.b); and the GPU mask takes a block's cells once its rebakes land._

## Tasks

T1–T5 are `hyperion-surface` code and run in order after R09's synthesis lands; T6 moves R05's bake
onto it. T7 (the client's field store) needs only R09's chunks and R05's workers and can run beside
T1–T5, though its worker tests need T6.b's entry points. T8 (horizon maps) needs T6.a. T9 (cascades)
is split (decision-r10-t9-split): T9.a, pure, needs nothing; T9.b follows T9.a, T11, T12 and
R10.T8.c, so the high setting has its horizon map to combine with.
T10 (lit terrain) needs T2, T6 and T7, and its shadow term from T8 and T9; T10.f needs T10.d. T11
(the wireframe) needs T6 and T7. T12 (readouts) needs T5, T6.b and T7. T13 (guide drafts) is docs
only, can be written at any time, and lands before T11 and T12, which are built to the drafts; it
keeps its number because the roadmap cites `R10.T13`. T14 (cache sizes) follows T10 and T11. T15
closes. T16 (the live terrain path, new at the scoping of 2026-10-16, `decision-r10-live-path.md`)
wires T7's store, T6.b's fields, T10.b's pass, T10.c's survey edge, T10.d's class map and T10.e's
hand-over into `VIEW`; it follows T10.c–f and precedes T11 and T12, which are built on its session,
and T14.b and T15, which measure on it. There is no interface-reconciliation task: this plan is
re-validated against R05 and R09 as built when its turn comes.

As re-validated at `c6d77cac` (Risks), with the critical path's order (R09's synthesis, then this
plan, then R09's server and coarse pass):

- **Startable now.** T13 (docs). T7, against R09.T19.a's types, R03's `FakeWebSocket` and the pool's
  scripted fake worker: T7 builds the render thread's half of the per-body calls (the pool's
  methods in place of `postField` and their messages in `workers/messages.ts`), and the height
  worker holds the blocks in JavaScript, as it holds R05's field today, until T6.b's module takes
  them; its live path waits on R09.T19.b. T1.a, whose tables read nothing of the synthesis (R09's
  palette types are built); its albedo rows took main's answer (Risks).
- **After R09.T8, and R09.T9 for goldens.** T1.b, T2 and T3, then T5 and T6.a; T6.b after T6.a, T5
  and T1.b. Each is built on `synthetic_worlds()` and `FieldBuilder` fields.
- **After R09.T16.** The coarse pass follows this plan, so until R09.T11–T16 land the reference
  worlds are a smooth dry sphere and `tests/common/surface_worlds.rs` does not exist. T4.a, which
  measures the real coarse pass's output, waits on R09.T16 whole; and the `sample_worlds()` cases
  of T1.b, T2, T5 and T10.a (their `crates/hyperion-sim/tests/` binaries) follow R09.T16 under the
  same task IDs, after each task's `synthetic_worlds()` part lands (a follow-up, as R09's T5 had).
  T3's `surface_ground_worlds` asserts finiteness, which the no-op worlds already exercise (as
  built, on the reference worlds through `coarse_pass`, moving onto `sample_worlds()` when it lands).
  T5's `surface_reading_worlds` holds its pooled case on `synthetic_worlds()`'s four bodies (as
  built), and its `sample_worlds()` case follows R09.T16.
  _R09.T16 is built (2026-10-23, 062a847f): `tests/common/surface_worlds.rs` exists and the
  reference worlds are no longer a smooth dry sphere, so T4.a and the `sample_worlds()` cases of
  T1.b, T2, T3, T5 and T10.a can start on it, each binary keeping its own list until its
  follow-up moves it; its tiers are as Provides' test helpers give them (Mars fast, the locked
  world slow)._
- **T4.b** builds `levelBounds.ts` and the selection rule after T6.b, on a fixture from
  `synthetic_worlds()`, and records the owner's counts after T4.a's `level-bounds.json`.
- **T6.a's bench** waits on R09's follow-up after T8, the fused interpolant pass that main has
  queued before this plan's quiet bake timing; meanwhile its figure is recorded provisional.
- **New T1.c** (decision-composition) follows T1.b and T1.d, as a Rust half now and a client half
  after T16 (decision-r10-t1c-maturity).
- **T10.a** follows T1.b; its server test of `surface_field`'s answer waits on R09.T19.b. **T10.b**
  draws the atmosphere through R07's stubs until R08.T9.b lands, and the ground's thermal emission
  waits on R08.T18.
- **T16** after T10.c–f land, then T11 and T12 on its session; T14.c, then T14.b, after it, T14.a
  free, T14.e after T14.a, and T14.d and T14.e before T14.b's measurements; T14.f after T10.g and
  before T1.c's client half; T15.a free
  (decision-r10-t15a-scope), and T15.b after the serve gate opens (decision-bump-plan-after-22
  §1.4).

Rust files are under `crates/hyperion-surface/src/` and client files under
`apps/hyperion/src/renderer/src/view/terrain/` unless a path says otherwise. Every test in
`hyperion-surface` follows R04's Design note 12: the `wasm_bindgen_test as test` import, a
`native_only` module for any test that reads files, and `should_panic` tests, each stating
`expected`, in the crate's `tests/panics.rs`. Golden files end in `.golden` and are read through the
testkit's embedded arm. Every GPU check is by hand and recorded in the task's plan entry with the
machine, setting and date, as the brainstorm's Testing section requires; no golden images. Every
timing, bench or by hand, is taken on a quiet machine, with no other test suite or lane running, and
recorded with the load average; a figure taken under shared load is marked provisional and
re-measured quiet before any decision rests on it. The timings this plan quotes from its research
(2–6 ms for the near-field horizon march, 60–120 ms for ray-marched far fields, 3–6.5 ms for the
station wireframe) are reasoned estimates, not measurements, and are provisional until the tasks
that own them measure them.

### R10.T1 Material classes

- **R10.T1.a The class set and its properties.** `MaterialClass` (with `Evaporite` and `Dust`),
  `Material { class, entry }`, `MohrCoulomb`, `FrictionModel`, `MaterialProperties`,
  `properties(material, &palette)`, `limb_parameter`, `bearing_capacity`, `interface_friction`
  (Design notes 2 and 8; decision-composition §1.8: `FrostSpecies` is gone, and `properties` joins
  the class's table to the entry's resolved values in R09's `PaletteEntry`, by its `mechanics`
  family where the class's rows are per family). It also assigns the codes of R09's
  `PaletteEntry::phase_row`, 0 in every field until then, each naming a law's phase row and its
  L(α) (Design note 8), which T10.b builds and the server's palette sources fill; the codes are
  generated output, under the table's version. Each figure is
  re-checked in its primary (the Lunar Sourcebook's Table 9.12 page, USGS PP 1389 Table 7, NAVFAC
  DM-7.02 Table 1, Kietzig et al. 2010's Fig. 2, the albedo sources of Design note 8) and cited in
  the doc comment; a figure without a source leaves its field `None`. `CLIFF_SLOPE` 40° and
  `TRANSITION_HALF_WIDTH` 5° with Kleinhans et al. 2011 and Atwood-Stone and McEwen 2013;
  `SNOW_SHED_SLOPE` with a source or removed, recorded either way. Files:
  `material/{mod,class,properties,bearing}.rs`. Tests: discriminants are stable (a table test on the
  `u8`s); every property is finite and in its physical range; every row with a value names its
  source; `bearing_capacity` for a square 1 m footing on lunar regolith at 1.62 m/s², which takes
  the 30–60 cm layer (c = 3.0 kPa, φ = 54°, ρ = 1,740 kg/m³), lies within the Sourcebook's
  3,000–11,000 kPa and within ±50% of its 6,000 kPa, and the same formula on the 0–15 cm pair (c =
  0.52 kPa, φ = 42°, ρ = 1,500 kg/m³) gives 150–300 kPa (Design note 2); with the factor formulas
  the code cites, both values are pinned to 1%, since the Sourcebook's band is too wide to catch a
  factor error; it grows with width and gravity; every class's A_N and L(0) keep the law's
  directional-hemispherical albedo at μ₀ = 1 at most 1 (Design note 8); `interface_friction` for ice
  interpolates between its points and is `None` outside them. Acceptance:
  `cargo test -p hyperion-surface material`.
  Records: [10-terrain-on-generated-worlds/records/R10.T1.a.md](10-terrain-on-generated-worlds/records/R10.T1.a.md).
- **R10.T1.b The classifier.** `material_at` with the decision order of Design note 1, over R09's
  `Synthesiser` and the finest level's gradient, projected into the tangent plane first (R09's
  "Deviations in T4, as built"), reading the cell's substance byte (`SynthesisCell::ice_entry`,
  `liquid_entry`) and the header's palette (`crust_entry`, the `Deposit` and `Cover` roles) as
  Design note 1's folded-in rules have it, with the cold-trapped share on the cell's shadowed
  ground. Files: `material/classify.rs`, both helper modules of Test helpers, `material/golden.rs`,
  a `cfg(test)` module of the crate as `ground/golden.rs` is, writing its goldens under
  `crates/hyperion-surface/tests/golden/material/` (a thousand points on each of
  `synthetic_worlds()`; corrected 2026-10-14; R09.T9's pattern), and
  `crates/hyperion-sim/tests/surface_material_worlds.rs`.
  Tests: a `cliff_field` is `Bedrock` above 40° and not below; on `sample_worlds()`, a warm pole
  has no ice cap and a locked world's ice lies where R09's field puts it (the brainstorm's two
  examples under Materials); `Seabed` exactly below the water surface; an arid filled basin is
  `Evaporite`; the Pluto-like and lava-world `FieldBuilder` worlds of decision-composition take
  their frost and `Melt`; the order of rules is total; the goldens match on native,
  `wasm32-wasip1` and `wasm32-unknown-unknown` through R04's checks. The `sample_worlds()` cases
  follow R09.T16 (the order of tasks). Bumps `GENERATOR_VERSION` (see Generator version).
  Acceptance: `cargo test -p hyperion-surface material` and
  `cargo test -p hyperion-surface --lib material::golden` (the goldens alone; corrected 2026-10-14;
  R09.T9's pattern), `cargo test -p hyperion-sim --features testing --test surface_material_worlds`,
  and `just test-wasm-fast` runs the goldens on both wasm targets (the orchestrator's `just ci`
  runs all three).
  Records: [10-terrain-on-generated-worlds/records/R10.T1.b.md](10-terrain-on-generated-worlds/records/R10.T1.b.md).
- **R10.T1.d The space-weathering law** (decision-p14-t51d-weathering §1, 2026-10-16; lane
  r10-weathering, after R09.T2.f; built early for P14.T54.b, as R09.T7.d was for P14.T51.f).
  Records: [10-terrain-on-generated-worlds/records/R10.T1.d.md](10-terrain-on-generated-worlds/records/R10.T1.d.md).
  - **The law.** Hapke 2001's npFe⁰ absorption, α = (36π z ÷ λ) φ ρ_h ÷ ρ_Fe, added to the
    equivalent slab's internal transmission:
    - Θ = Θ₀ exp(−τ), τ = m f (M_Fe ÷ M_FeO)(ρ_h ÷ ρ_Fe)(36π z(λ) ÷ λ) Λ;
    - applied to the entry's fresh w, which the reference form's `A_N`(w) inverts;
    - given as ratios of the form's `A_N`, `A_S` and `A_h`;
    - the identity at τ = 0 or for an opaque grain.
  - **The fit.** `hyperion-fit`'s `space_weathering` fits Λ by least squares, each residual
    weighted by its anchor's 10–90% spread, to the Moon's mature highland and mare normal albedos
    in B, V and R (Dollfus and Bowell 1971, Table 4, at 600 nm, taken to the bands by the paper's
    own whole-Moon spectral law, Eq. 11, Ā ∝ λ^0.83, one colour for both terrains; the task
    computes each band's centre and 10–90% bounds from the 600 nm values;
    decision-r10-t1d-anchor-colour) from the fresh `feldspathic` and `basalt_fe` rows at m = 1. Each
    row is taken at its anchor soil's FeO_T, 0.055 and 0.16 (Morris 1980, Table 1: the mature
    Apollo 16 soils and the Apollo 11, 12 and 17 mare-plain soils), the manifest's explicit
    parameters (decision-p14-t51d-weathering, addendum 2026-10-19). It reports the implied y_sat
    against Morris 1980's Eq. 4 at T51.d's X = 75. It emits `tables/space_weathering.rs`, stamped
    `since-generator-version: 22`, the version in force, since nothing at 22 reads it, and P14.T54.b
    restamps it `--since 24` when it wires the law (main's ruling, 2026-10-19: `hyperion-fit check`
    refuses a table ahead of `GENERATOR_VERSION`, as P14.T49.c's `substance_albedo` was held).
  - **The scale.** `field_albedo_scale` solves c on the albedos weathered at the header's
    maturity, and R10 draws clamp(c × W(…)).
  - _Files:_ `material/weathering.rs`, `tables/space_weathering.rs`, `tables/mod.rs`,
    `crates/hyperion-fit/src/tasks/space_weathering.rs` with its manifest, and
    `crates/hyperion-server/src/surface/service.rs`.
  - _Tests:_
    - the identity at m = 0, f = 0 and for `Lambert`;
    - monotone non-increasing in m and f;
    - B darkens relatively more than R;
    - never above the fresh value, and `A_h` at most 1;
    - the highlands inside their spread in B, V and R and the maria inside in B and at the lower
      edge in V (at least the P10 bound less σ_V ÷ 8, computed from the anchors) with one Λ; the
      maria's R and colour stated as a known miss (the form's dark end and the stand-in fresh
      rows); y_sat within a factor 3 of Morris's Eq. 4; and the fresh/mature contrast of each
      terrain stated against fresh craters (Dollfus and Bowell 1971, Table 4);
    - the golden on native and both wasm targets.
  - _Accept:_ `cargo test -p hyperion-surface material::weathering`;
    `cargo test -p hyperion-fit space_weathering`; `cargo run -p hyperion-fit -- check`;
    `just test-wasm-fast`.
- **R10.T1.c Maturity and cover.** New at the re-validation (decision-composition §1.8 and
  §7.5); ruled 2026-10-20 (decision-r10-t1c-maturity). Each material's `A_N` and colour come from
  R10.T1.d's law at the ground's maturity, blended with the organic cover, by one shared function
  in `material/maturity.rs`. The server's solve, the worker's photometry and class map (at
  m_body) and its point query (at m_local) all call it, so main-thread readers take the worker's
  values, never a TypeScript copy. `properties` stays the fresh join; `Melt` stays fresh.
  Records: [10-terrain-on-generated-worlds/records/R10.T1.c.md](10-terrain-on-generated-worlds/records/R10.T1.c.md).
  - _The maturity at a point:_ m_local = m_body − max over craters of s × max(0, m_body −
    `maturity_after`(t_c, k)), from the header's `maturity` and `weathering_rate` and each
    crater's age t_c (`SmallCrater::age`, `CoarseCrater::age`). A crater's fresh share is
    s = g f_ex p(x):
    - p(x) is 1 inside the rim and (e^(−(x − 1) ÷ 3) − e^(−3)) ÷ (1 − e^(−3)) out to 10 rim
      radii (Grier et al. 2000's radially averaged OMAT), shifted to 0 at the reach (1 + a) R for
      a coarse crater, which the field carries no farther;
    - f_ex = (1 − q)², q = d_m ÷ 0.084 D, d_m = 0.022 m ((t_s − t_c) ÷ 1 Myr)^0.45: the share of
      a paraboloid excavation cavity below the mature layer (Morris 1978; Melosh 1989), so that a
      crater under about 10 m on old ground exposes nothing fresh;
    - g is the crater's kept relief, as the synthesis computes it: the gardening that wears a
      small crater mixes its ejecta away (Grier et al. 2000: small craters "age" faster).

  - _The cover:_ (1 − c) A + c A_cover, with c the header's `organic_cover` and A_cover the first
    organic `Cover` entry's `A_N`. It applies on every form of an `Ice` entry whose formula holds
    carbon, CO₂ excepted, and of `N2` beside one (P14.T51.d's `organic_ground` on the palette's
    keys).
  - _The split:_
    - this Rust half now, landing before P14.T54.b (if it does not, T54.b leaves the maturity and
      the rate absent);
    - a second Rust follow-up (decision-r10-t1c-halo), in which the solve takes each material's
      albedo averaged over the body's young-crater haloes, so that the drawn mean stays p: the
      body's maturity mix, P(D ≤ d) = F_c(d) exp(−Λ_s(d)) over its small craters' octaves (a
      Poisson field under the largest-deficit rule) and its coarse craters' list, cut at 52
      maturity nodes with each bin's mean deficit kept, each material one term a node in the
      exact piecewise-linear solve (generator-versioned: at 24 if ready, else the next bump);
    - a client half after R10.T16 and with or after that follow-up, which bakes a per-texel
      maturity with the same function and blends between Rust-computed albedos
      (`bake/layers.rs`, `worker.rs`, `wasm.rs`, `heightBake.ts`, `terrainMaterial.ts`,
      `terrainLit.wgsl`, `classMap.ts`, `bodies/draw.ts`), with its check by hand on the RTX 3080.
      It is presentation, so it owes no bump. _As built (2026-10-22; Risks, "Deviations in T1.c's
      client half, as built"), its acceptance adds
      `cargo test -p hyperion-surface --lib worker::golden` and the ignored
      `maturity_drawn_mean_is_the_body_s_mix_mean`, `maturity_layer_s_time_per_patch` and
      `maturity_nodes_s_time`, each `cargo test -p hyperion-surface --lib <name> -- --ignored`._

  - _Files (the Rust half):_ `material/{maturity,mod,albedo,properties,classify,golden}.rs`,
    `synth.rs` (an accessor), `worker.rs`, `testing.rs`, `testing/worlds.rs`,
    `tests/golden/material/maturity.golden`, `tests/panics.rs` and
    `crates/hyperion-server/src/surface/service.rs`.
  - _Tests:_
    - the fresh/mature contrast is stated, not asserted, against Dollfus and Bowell 1971's Tycho,
      Proclus and Kepler at each crater's own age. It is a known miss (D2) until T49.i's whole-soil
      fresh rows, and is asserted within the measured range after their refit. Its invariants and
      as-built values are pinned;
    - one crater's profile; k scaling; f_ex and g; overlaps;
    - the immature shares, with a guard, and Grier et al. 2000's count printed;
    - the identity, to the bit, with no maturity and no cover;
    - `Melt` fresh;
    - zero cover gives the substance's own albedo;
    - the solve on the exposed albedos;
    - the golden on native and both wasm targets.

  - _Accept:_ `cargo test -p hyperion-surface material::maturity`;
    `cargo test -p hyperion-surface --lib material::golden`;
    `cargo test -p hyperion-surface --test panics maturity`;
    `cargo test -p hyperion-server albedo_scale`; `just test-wasm-fast`.
  - _The halo follow-up:_ files `material/{maturity,albedo,mod,golden}.rs`,
    `material/albedo/tests.rs`, `tests/golden/material/halo.golden`, `tests/panics.rs` and
    `crates/hyperion-server/src/surface/service.rs`. Tests: the inverse halo; the mix's shape;
    the identity to the bit; one coarse crater's closed form; convergence; the haloed scale below
    the halo-free one; the brute force over 10⁴ points (the stated excess within max(10%, 0.2%) of
    the drawn one, the drawn mean within 1% of p, the shares); the server's solve; the golden on
    native and both wasm targets. Accept: `cargo test -p hyperion-surface material::maturity`,
    `--lib material::albedo`, `--lib material::golden`, `--test panics maturity`;
    `cargo test -p hyperion-server albedo_scale`; `just test-wasm-fast`.

### R10.T2 Class weights for coarser levels

Records: [10-terrain-on-generated-worlds/records/R10.T2.md](10-terrain-on-generated-worlds/records/R10.T2.md).

`ClassWeights`, `weights_at` (Design note 3), with the slope-exceedance probability from R09's
level-keyed closed forms at level n, `Synthesiser::omitted_variance` and
`omitted_structure_function` (what the level leaves out of the finest level's ground; added by
R10.T5, `decision-r10-t5-closed-forms.md`), and the palette of up to eight classes a patch keeps. As built, `unresolved_variance` and the channels' and craters' own structure functions
exist, and the whole `structure_function` is R09.T8's (landed, `c280f604`: the free function and
`Synthesiser::structure_function`; updated 2026-10-15); the closed forms are held once per body,
since R09's crater octaves cost about a millisecond to build and the per-cell forms rebuild them
(R09's "Deviations in T7.b, as built"). Files: `material/weights.rs`,
`crates/hyperion-sim/tests/surface_weights_worlds.rs`. Tests:
at the finest level the largest weight is `material_at`'s class at every sampled point, ties
included; weights sum to 255; on `sample_worlds()` the weights at levels n − 1 to n − 6 from the
finest, and at the class map's level (level 4, Design note 8), agree with brute-force class
fractions over each texel's footprint (sixty-four finest points a texel at n − 1 to n − 6, 4,096
sampled points a texel at the class map's level) to within ten percentage points in the mean over
a thousand texels a level (slow); a patch never needs more than eight classes on `sample_worlds()`,
and if one does the test names it. The same agreement is first held on `synthetic_worlds()`, whose
full belts put about a third of their area above 40° at 2 m and plain styles none (R09's
"Deviations in T5's second follow-up"); the `sample_worlds()` binary follows R09.T16. Acceptance:
`cargo test -p hyperion-surface material::weights`,
and the slow case under `just test-slow -E 'binary(surface_weights_worlds)'`.

### R10.T3 The authoritative ground on real terrain

Records: [10-terrain-on-generated-worlds/records/R10.T3.md](10-terrain-on-generated-worlds/records/R10.T3.md).

R05's `finest_surface_height`, pointed at R09's `Synthesiser` through R05's `HeightSource` trait
(R05.T4.a, T4.c), which this task implements for it (R09's `BandLevel` from the `u8` level,
`SynthCache` as the cache, `QueryHeightError` as the error), and wrapped as `ground_at` with
`GroundPoint` and `FinestTriangle` (Design note 4), on R05's diagonal and `xyz_to_face_uv`. The
trait's `level_bound_m` and `height_range_m(key)` forward R09.T8's `level_bound_m(header, level)`
and `Synthesiser::height_range_m(patch)`, and `figure()` the header's (R09.T8 has landed,
`c280f604`, and as built the trait's `level_bound_m` reads the bounds the synthesiser holds, bit
for bit the free `level_bound_m(header, level)`: Risks, "Deviations in T3, as built"; updated
2026-10-15). R05's interpolant returns the height alone, so this task adds beside it, in
`patch::collision`, the query that also gives the triangle, with `finest_surface_height` its
height. Files:
`ground.rs`, R05's `patch` module, `src/ground/golden.rs` (an in-crate `cfg(test)` module) with
`tests/golden/ground/<world>.golden` (corrected 2026-10-15). Tests: at
a vertex `ground_at` equals R09's `height_at` there bit for bit; on an analytic plane field it is
exact; points on a shared edge get the same height from either triangle; points either side of a
cube-face edge and at a cube corner agree to 1 µm; the normal is the triangle's; the chord sagitta
of Design note 4 (L² ÷ 8R at 0.5 m on 6,371 km, about 5 nm) keeps face-coordinate and spatial
interpolants within 1 µm; `ground_at` is finite at every sampled point of `synthetic_worlds()` and
of `sample_worlds()` (the latter in `crates/hyperion-sim/tests/surface_ground_worlds.rs`), and
asserts finiteness before it returns (the brainstorm's NaN hazard). The goldens, on
`synthetic_worlds()`, are asserted on native and both wasm targets. No bump: the ground reads
heights that R09's goldens already pin (R09.T9's, landed in `070da8ee`, on all three targets at
header 21). Acceptance:
`cargo test -p hyperion-surface --lib ground`, which runs the goldens too, an in-crate `cfg(test)`
module (`src/ground/golden.rs`; corrected 2026-10-14; "Deviations in T3, as built"), and
`cargo test -p hyperion-sim --features testing --test surface_ground_worlds`.

### R10.T4 The level bound on generated worlds

- **R10.T4.a The bound measured.** R09's `level_bound_m` on `sample_worlds()`, the real coarse
  pass's output rather than R09's synthetic fields, with σ per level from R09's level-keyed closed
  forms, `Synthesiser::omitted_rms` at each level (Design note 5; added by R10.T5). Files: `crates/hyperion-sim/tests/level_bound_worlds.rs`, which also writes each world's
  per-level hard bound and σ as a fixture for T4.b, `fixtures/level-bounds.json` under the client's
  `view/terrain/`. Tests: for ten thousand positions a world and every pair (n, n +
  k), |h_n − h_(n+k)| stays under `level_bound_m(n)` (slow). Recorded per world and level in this
  task's plan entry: the observed 99.9th percentile over the bound (expected 0.3–0.5), the
  per-patch maximum over 4,225 vertices over the bound, and the spread between the body-wide and a
  per-patch bound. _Re-validated at `c6d77cac`: this task waits on R09.T16 (the reference worlds
  through the whole coarse pass, and `tests/common/surface_worlds.rs`) and on R09.T8, whose bound as
  accepted is hard in its octaves and channels and a 1 − 10⁻⁹ per-point quantile in its crater and
  rim terms, so the test holds that stated bound and records which term each worst case came
  from._ Acceptance: `just test-slow -E 'binary(level_bound_worlds)'` passes, and the
  figures are recorded.
  Records: [10-terrain-on-generated-worlds/records/R10.T4.a.md](10-terrain-on-generated-worlds/records/R10.T4.a.md).
- **R10.T4.b Selection by the real bound.** The worker answers `field_init` with the body's
  `LevelBounds` (hard and σ per level, σ R09's level-keyed `Synthesiser::omitted_rms`, added by
  R10.T5); `levelBounds.ts` holds them per body and `selectionBound` gives R05's
  `screenSpaceErrorPx` its ε_n under the rule in force (the hard bound until T4.b applies
  `signoff-12.md` item 30's rule), for each style's τ: 1 px high and 2 px low (the brainstorm's
  lean; CesiumJS's default `maximumScreenSpaceError` is 2 px, though its error is a spacing
  heuristic), and 4 px wireframe.
  As built, selection reads the bound column of the body's level table (`PlanetGeometry.levels`,
  stride 4), and its error is that bound plus R05's chord sagitta (`selectionErrorM`), so
  `selectionBound` fills that column and `field_init`'s answer carries the rest of the table, the
  lowest and highest height by level that culling reads before a patch is baked (lean: the union of
  the six root patches' `height_range_m`, R09.T8's). The test planet keeps its own `levelTable`.
  _As built by T6.b (Risks, "Deviations in T6.b, as built"): every block taken, not `field_init`
  alone, is answered with the body's level table in `levelTable`'s layout, its heights a row each
  from the held cells' elevations and the level's own contributions rather than the root patches'
  union; there is no σ column, which T4.b makes; the pool keeps the latest table
  (`fieldLevels(body)`)._
  Files: `levelBounds.ts`, R05's `planet.ts` and `select.ts`. Tests (Vitest, pure): a level is
  selected exactly where its selection error subtends at most τ, under either rule, taken as a
  parameter; R05's
  history-independence test passes with real bounds; the grounded-body rule still yields the finest
  level. Recorded in this task's plan entry, from T4.a's `level-bounds.json` fixture through
  `selectPatches` over R05's scripted descent (`descentProfile.ts`): the patches selected at
  τ = 1 px per world under the hard bound and under min(hard, 4σ), and applies `signoff-12.md`
  item 30's rule to them: min(hard, 4σ) where it saves 1.5 times or more on every world and the
  per-patch maxima pass 4σ in at most one patch in three, else the hard bound.
  Acceptance: `pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain` and the counts
  recorded.
  Records: [10-terrain-on-generated-worlds/records/R10.T4.b.md](10-terrain-on-generated-worlds/records/R10.T4.b.md).
- **R10.T4.c The σ survey only where the rule reads it** (added 2026-10-26 by
  decision-r10-item30 §6; before RM5). The hard bound stays at RM5 (item 30 applied, no
  calibration factor), so nothing reads σ, which costs some 25 s of every worker's time for a whole
  level-8 field (T4.b's record). `field_init` carries a flag set from
  `SELECTION_RULE === "statistical"`, false today. With it false the worker skips
  `LevelSigma::survey` and answers σ = 0 at every level, the "no σ read" case `selectionBound`
  already maps to ε_n. With it true, σ is computed as T4.b built it. There is no lazy or background
  survey; a switch of rule starts the fields again with the flag. Files: `worker.rs` and `wasm.rs`
  in `hyperion-surface`, and the client's `levelBounds.ts` and
  `workers/{messages,heightBake,pool}.ts`. Tests:
  Records: [10-terrain-on-generated-worlds/records/R10.T4.c.md](10-terrain-on-generated-worlds/records/R10.T4.c.md).
  - the worker's (`worker::`): with σ off, σ is 0 after every block and `omitted_variance_in` is
    never reached; with σ on, the existing tests pass bit for bit;
  - `fieldsWasm.test.ts` under both flags;
  - the flag follows `SELECTION_RULE`;
  - selection under the hard rule is identical with σ zeroed and with the fixture's σ;
  - a guard in `levelBounds.test.ts` (Vitest, pure): on each fixture world, over R05's descent at
    30 s steps, at 1920 × 1080 and 60°, τ = 1 px, the 604-patch budget and flat boxes, the hard
    rule's mean drawn error is at most 1.5 times a perfect level-wise bound's (the ruling's §5.4).

  Item 30's second-condition test is kept. The golden `worker/moon_like.golden` runs with σ on and
  does not move; no bump. Acceptance: `cargo test -p hyperion-surface --lib worker::` and
  `pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain` pass, with the worker's
  time per level-8 field recorded before and after.

### R10.T5 Surveyed readings

Records: [10-terrain-on-generated-worlds/records/R10.T5.md](10-terrain-on-generated-worlds/records/R10.T5.md).

`SurveyedReading`, `surveyed_reading`, `SlantRange`, `slant_range`, `SLOPE_BASELINES` (Design notes
6 and 7), over R09's level-keyed `omitted_rms` at the reading's level, its
`omitted_structure_function` at lag b (both added by this task, as ruled in
`decision-r10-t5-closed-forms.md`, since R09's `unresolved_rms` at `λ_n` counts less than a level
leaves out) and the coverage resolution per cell (`PartialField::resolution`), with J₀ through
base's `math::j0` (R04.T4.c), inside R09's crater form. As built: the closed forms are held once
per body by the `Synthesiser`, as T2's are; `PartialField::resolution` is `Some(ResolutionCode::NONE)`
on a margin cell, so "unsurveyed" is `!is_surveyed(cell)`, and `ResolutionCode::resolution()`
gives the metres; the resolved slope is the gradient's tangential part (R09's "Deviations in T4,
as built"). The pooled case on `sample_worlds()` follows R09.T16; it is first held on
`synthetic_worlds()`. Files: `reading.rs`, `crates/hyperion-sim/tests/surface_reading_worlds.rs`.
Tests: at a band-limit survey the elevation is `ground_at`'s, the uncertainty zero and `resolved`
set; at an orbital
survey, pooled over about 200 cells × 100 points a world on `sample_worlds()`, the RMS of
`ground_at` less the quoted value is within 10% of the pooled predicted σ, and the per-cell z-scores
have unit variance within 15% (slow); per band, the slope uncertainty tends to √2 σ_k ÷ b for b
much larger than the band's wavelength and to the band's gradient σ for b much smaller (Design note
7's limits); the RMS slope is never
below |g₀|; an unsurveyed cell gives `None`; the slant range to a plane is exact and its σ follows
cos θ_s ÷ sin γ_s; a ray under the `GRAZING` threshold has no σ; a ray that leaves the surveyed
region before it hits gives `LineOfSight::NotSurveyed`, and one that misses the body's ground
gives `LineOfSight::NoGround` (as ruled, `decision-r10-t5-closed-forms.md`; the σ is R09's
level-keyed `omitted_rms`, the slope's v its `omitted_structure_function`). Acceptance: `cargo test -p hyperion-surface reading`, and the
pooled case under `just test-slow -E 'binary(surface_reading_worlds)'`.

### R10.T6 The bake on real terrain

- **R10.T6.a The Rust bake.** R05's `bake_patch`, pointed through T3's `HeightSource` implementation
  at R09's `Synthesiser` and its `Synthesiser::bake_patch` (heights, parent band and gradients) and
  `SynthCache`, and extended with `TerrainLayers`: the class palette and weights (T2), the survey
  mask from the `PartialField`'s `is_surveyed`, the patch's hard bound and σ, the horizon map once
  T8 lands, and R11's `BakeHook`, with `scatter::ExtraLayer` declared as an empty
  `#[non_exhaustive]` enum for R11 to own and extend; normals at R05's `NormalScale`, which R05
  already packs as `rg16float` (its Design note 5). Files: `bake/{mod,layers}.rs`, R05's `patch`
  module. Tests: the bake is the same bytes whatever order a patch and its neighbours are baked in
  (`assert_order_independent`); heights equal R09's `height_at` narrowed to `f32`; the survey mask
  is 0 exactly on vertices of unsurveyed or margin cells; inserting a block that adds cells changes
  no baked height of a patch already baked (Design note 16); a change of a cell's `ResolutionCode`
  alone leaves every bake byte-identical (the image is the same at every survey resolution that
  covers it, Design note 6). Bench `bake/patch_65`: the brainstorm's 40 ms a patch on two cores
  (about 10 µs a point with its gradient, Performance budget), recorded with and without the class
  weights on the development machine (a Ryzen 7 3700X) and, by the owner, on the UHD 620 laptop's
  i7-8665U, whose cores the brainstorm's figure budgets. The 40 ms stays the laptop's figure: the
  desktop records its own and fails only if over it (decided 2026-09-30, Risks). _Re-validated at
  `c6d77cac`: `Synthesiser::bake_patch` is R09.T8's (landed, `c280f604`; it gives `PatchHeights`;
  updated 2026-10-15), whose own bench is provisional at 77–82 ms a patch and 14 µs a point at
  L + 4, so this bench's quiet figure
  follows R09's fused-interpolant follow-up, which main queued before it; meanwhile the figure is
  recorded provisional. The bench is a `[[bench]]` of its own (`benches/bake.rs`, on the synthetic
  worlds, so `required-features = ["testing"]` as R09's `synth` bench has it)._ Acceptance:
  `cargo test -p hyperion-surface bake`, and the bench's figure in this task's entry
  (`just bench -- bake/patch`, on a quiet machine, under the heavy lock, or recorded provisional).
  Records: [10-terrain-on-generated-worlds/records/R10.T6.a.md](10-terrain-on-generated-worlds/records/R10.T6.a.md).
- **R10.T6.b The worker binding.** The worker's entry points on R05's binding:
  `field_init(body, header)`, `field_chunk(body, block)` (one R09 payload block, Design note 16),
  `field_release(body)`, holding one `PartialField` per body; `bake`, taking R05's `BakeOptions`
  with this plan's settings; and, for T12's readouts, `reading(body, at, slope_baseline)`,
  `slant_range(body, origin, direction, max)` and `material(body, at)` (the class with its
  `MaterialProperties`), each answering `NotSurveyed` where the field has no cell. `field_init`
  answers with the body's `LevelBounds`. R05's provisional height function leaves the client path
  (R09's Design note 18); `hyperion_surface::height`, where R09.T4 moved `HeightSample`, stays, as
  does `LatticeCache` in `noise`, which R09.T4 did not move (it already lived there). As built,
  R05's module serves the test planet (`bakePatch`, `levelTable`, `surfaceHeightM`,
  `omittedSigmaM`, each with `Ridges`), which the spike keeps: the per-body exports go beside them,
  `BakeSettings` and `TerrainPassOptions` name the height source (the test planet with its
  `ridges`, or a body) in place of `ridges` alone, and `heightBake.ts`'s `HeightModule` gains the
  new exports. T7 has built the render thread's half (the pool's per-body calls and their
  messages), so this task builds the worker's: `answerRequest` takes the blocks into the module,
  each through `wire::decode_block` and `PartialField::insert`, and answers `field_init` with the
  level table. Files: R05's binding, R05's `view/terrain/workers/*.ts`. Tests: in the worker test,
  init, two blocks and a release leave no field held; a block posted before its `field_init` is
  refused; each reading entry point equals the native function on a fixture field.
  Acceptance: `just gen-surface`, then
  `pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain/workers`.
  Records: [10-terrain-on-generated-worlds/records/R10.T6.b.md](10-terrain-on-generated-worlds/records/R10.T6.b.md).

### R10.T7 The client's coarse-field store

Records: [10-terrain-on-generated-worlds/records/R10.T7.md](10-terrain-on-generated-worlds/records/R10.T7.md).

`CoarseFieldStore` and `CoverageView` (Design note 16): wanting and releasing bodies from the views'
cameras with hysteresis, R09's `surface_field` with `have_revision`, re-requests on
`surface_revisions`, forwarding chunks to every worker, the coverage and its resolution per cell,
the GPU coverage mask per face (Design note 12), allocated under the `MemoryCategory`
`coarse-field-gpu`, which this task adds to R01's union under R12's name (R12 Design note 6), and
the link's loss keeping what was received and
marking the coverage stale. As built (R09.T19.a; Consumes): the request is
`SurfaceFieldRequest { universe, body, have_revision }` (`null` for none) and the answer
`SurfaceFieldDto`'s `ready` arm (`header`, `revision`, `coverage`, `bulk`) or `not_modelled`;
`requestBulk` gains a per-chunk option beside `onPartial`, through which the store splits blocks
at each block's length (offset 6 of its frame) and posts each as it completes; `CoverDto`'s ranges
carry codes, which a TypeScript twin of `ResolutionCode::resolution` turns into metres, pinned to
R09's `tests/golden/wire/codes.golden` `resolution_m` rows. This task builds the render thread's
half of the per-body calls: `HeightWorkerPool`'s calls for `field_init`, `field_chunk` and
`field_release` in place of `postField` (the pool keeping a body's blocks for a replacement worker,
as it keeps R05's field now) and their messages in `messages.ts`, keeping the spike's call working
(`view/spike/spikeRun.ts`, whose 15 MB `syntheticField()` stands in for a field's memory and is no
payload; T7 records how); until T6.b the height worker holds the blocks in JavaScript, as it
holds R05's field today. R09.T19.b has landed (`7392c064`), so the server answers
`surface_field` with the payload and its deltas, pushes `surface_revisions` as
`SurfaceRevisionDto`s, and answers generated bodies `not_modelled` under `RecordInputs`; the tests
drive these through the fake socket in those shapes (Risks, "Deviations in T7, as built", its
"With R09.T19.b" item; updated 2026-10-15).
Files: `coarseFieldStore.ts`, `coverage.ts`; R05's `workers/{pool,messages,heightBake}.ts` and
`view/spike/spikeRun.ts`; R01's `view/engine/memory.ts`; R03's
`packages/protocol/src/requests.ts`. Tests (Vitest with R03's `FakeWebSocket` and
`pool.test.ts`'s scripted worker): a body within range is requested once and its chunks reach
every worker in arrival order; a revision rise fetches only new cells; leaving and returning
inside the hysteresis band fetches nothing; every worker gets `field_init` once and every chunk;
releasing a body frees it in every worker; the resident total of worker copies is reported, for
T14; the per-chunk option sees every chunk in order before the outcome settles. Acceptance:
`pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain` and
`pnpm --filter @hyperion/protocol test`.

### R10.T8 Horizon-map shadows

- **R10.T8.a The bake.** `HorizonMap`, `far_field_point` and the bake of Design note 10 in
  `bake/horizon.rs`, called by T6's bake on both settings. Tests: a `flat_field` on a sphere has a
  horizon at the curvature drop's angle and never above zero; a `cliff_field` of height H at
  distance d gives arctan((H − d² ÷ 2R) ÷ d) within one quantisation step; over at least 10⁶
  elevations spanning [−90°, 90°], ±90°, 0 and the neighbours of code boundaries included,
  `decode_horizon(encode_horizon(e))` is within 90° ÷ 32,767 ÷ 2 (4.95″) of e, the decode is
  monotonic in the code, and `encode_horizon` equals WGSL's `pack2x16snorm` rounding on the same
  inputs (a fixture the TypeScript twin of T8.b also reads); for the Sun at 1 au (ρ = 959.6″) the
  lit fraction from the decoded horizon is within 1/255 of the exact one at every e; azimuth
  interpolation is continuous; the far field is bit-identical at shared lattice points of adjacent
  patches of one level on one face, and the lattice's points and the march's step distances are the
  same bits at every level that holds them (decision-r10-t8a §1); the far field is independent of
  the cache's state, with bakes in any order; the march stops at √(2 R Δh). Bench `bake/horizon`: at
  most a quarter of the patch's 40 ms, recorded; over it, the near field moves to the line sweep.
  Presentation-only: no golden across targets, though the bake is deterministic. The far field's
  level-m heights cost R09.T8's per-point figure (provisional at 14 µs, against the 2–4 µs Design
  note 10's estimates assumed), which the bench records. Acceptance:
  `cargo test -p hyperion-surface bake::horizon` and the bench's figure recorded
  (`just bench -- bake/horizon`, in T6.a's `benches/bake.rs`).
  Records: [10-terrain-on-generated-worlds/records/R10.T8.a.md](10-terrain-on-generated-worlds/records/R10.T8.a.md).
- **R10.T8.b The shadow lookup.** `shadows/horizon.wgsl`: the sun's apparent direction from
  `body_fixed_at`, its elevation against the datum's normal ν and its grid azimuth in the patch's
  face's grid frame (`grid_azimuth`, T8.a; R08's lookups keep their azimuth from local north), the
  azimuth through `preciseAtan2`, R07's star and R08's refraction where drawn; the compare against
  the interpolated horizon, carried into the element's frame as e − (e★ − (π ÷ 2 − φ)), e★ the
  sun's elevation above the datum and φ its angle from the terrain's normal, and passed as the
  `horizon` argument of R07's `sphere_irradiance(h, phi, horizon)` (the signature and its default
  of 0 on the smooth figure are R07.T6.a's and T6.c's, used unchanged), whose visible share is the
  soft lit fraction of Design note 10, applied once (decision-r10-t8b §1–2). The horizon is
  blended from the patch's map to its parent's by the vertex's morph factor k, the parent's slot
  and quadrant in the instance record's spare words, the parent read where k > 0 from the same
  `horizon-map` buffer, and the own map where no parent's map is resident (decision-r10-t8a §1).
  The worker bakes the map for the lit style and hands it over, with its refused cells; a patch is
  baked again on a block that brings one of them, surveyed or margin, and each block's change
  carries the cells it brought that the field did not hold (decision-r10-t8a §3; Design note 16).
  The horizon map binds as the `horizon-map` slot field's storage buffer, fragment only
  (decision-r10-t10b-r09-t2e §5). The sun's elevation is π ÷ 2 less its zenith angle
  `preciseAtan2(|s × ν|, s·ν)`, since as built `preciseAtan2(y, x)` takes y ≥ 0 and returns
  [0, π] (R08 Design note 18); R07's function keeps WGSL's built-ins, a deferred correction to R07
  (decision-r10-t8b §2; _settled by R07.T6.d, `61e156b3`_). R08 draws no refraction yet, so the
  sun's direction is the geometric one.
  R07's clamp of a negative horizon then takes the element's own plane, exactly, where that hides
  more. WGSL's `asin` inherits atan2's 4,096 ULP, about 50″ at 30°, so no elevation here is formed
  by it. Files: `shadows/horizon.wgsl`, `shadows/litFraction.ts`, and the call in
  `terrainLit.wgsl`; R07's `litBody.wgsl` is unchanged. In the crate, `bake/horizon.rs`,
  `worker.rs` and `wasm.rs`; in the client, `workers/{messages,heightBake,rebake,pool}.ts`,
  `slotLayout.ts`, `gpu/uniforms.ts` (with the terrain pass's instance packing) and
  `shaders/{terrainPatch,terrainLit}.wgsl`. Tests: with a horizon of 0 the terrain's
  `sphere_irradiance` equals R07's smooth-figure value bit for bit, and with the map's horizon in
  the element's frame it equals R07.T6.a's oracle to 10⁻⁶ where the disc is wholly up or down, and
  an integral over the disc above the datum's horizon and the element's plane within 1.5 × 10⁻³ of
  face-on over slopes to 45°, every bearing and horizons to 60°; the lit-fraction formula as a pure
  TypeScript twin against hand values (0 and 1 beyond ±ρ, one half at e_sun = e_horizon); the
  blend's twin, `gridAzimuth` against the crate's fixture, and the rebake rule in the crate and on
  the module (decision-r10-t8a §1–3); R01's smoke harness compiles and runs the pass on
  SwiftShader with and without f16, every texel finite. By hand, recorded: a sunset over a
  generated range on the low setting, on the development machine and, by the owner, on the UHD
  620, whose GPU time, with the blend, is the one held against the budget's 0.3–0.5 ms
  (Performance budget, Shadows row); whether eight azimuths show missing peaks; and three places
  to look: a coarse–fine edge, with the blend, a same-level patch edge (Risks, "Edges between far
  points") and a face edge near a cube corner (decision-r10-t8a §1). Acceptance:
  `pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain/shadows` and the smoke run.
  Records: [10-terrain-on-generated-worlds/records/R10.T8.b.md](10-terrain-on-generated-worlds/records/R10.T8.b.md).
- **R10.T8.c The sunset's steps at patch edges** (new, decision-r10-t16-leans §9 and §16). The
  shading normal morphs with the height: blended to the parent's normals tile by k, as
  `terrainLit.wgsl` blends the horizon, so that a fine patch at k = 1 shades as its parent at a
  coarse–fine edge (`terrainPatch.wgsl` reads the patch's own tile at every k today). Then a scratch
  build tinting each patch by its level and its skirts and fins apart, at 228–236 s of
  `TERRAIN TEST` looking north, on both settings, the primary alone and with a photorealistic
  instrument, places what remains among T8.b's deferred candidates (the near field cut at patch
  edges, the parent blend, skirts and fins lit without the horizon), and the task fixes it. The
  multi-view morph invariant: a test of a view that draws, beyond its band's end, a coarse leaf
  beside a finer one another view selected, then its remedy (bands from the levels each view draws,
  or the morph held at a coarse–fine edge). Tests: at k = 1 a fine patch's shading normal equals its
  parent's along their edge; the invariant; `just test-render --part=6`. By hand, hidden on the RTX
  3080: the sunset at the same moments; then the owner's UHD 620 look.
  Records: [10-terrain-on-generated-worlds/records/R10.T8.c.md](10-terrain-on-generated-worlds/records/R10.T8.c.md).

### R10.T9 Cascaded terrain shadows on the high setting

- **R10.T9.a Cascades** (split from T9.b by decision-r10-t9-split; pure, no GPU work).
  `shadows/cascades.ts` (Design note 11): the spec of four 2,048² `depth32float` layers (the
  brainstorm's 64 MB, 67,108,864 B) for the `MemoryCategory` `shadows`, which T9.b adds to R01's
  union under R12's name where it creates them; PSSM splits with λ = 0.75 from the view's near plane
  over the shadowed range f(h), the view's far infinite as built; bounding-sphere fitting;
  whole-texel snapping, each cascade's grid carried from its own snapped centre of the frame before
  by whole texels, so that the sun's turn slides it against the ground by at most that turn times
  the cascade's radius (with no frame before, from the camera's place rounded to a body-fixed
  lattice of cascade 1's diameter, a power of two); light matrices built camera-relative in `f64`
  and narrowed to `f32` once; `unclippedDepth` where `depth-clip-control` is offered, and otherwise
  the near plane pulled towards the sun by the caster reach min(Δh ÷ sin e★, √(2 R Δh));
  slope-scaled bias and a normal offset of one texel, as data; the cascades' weight, 1 where cascade
  1's texel is at most half the horizon map's at the nadir and 0 where it is coarser, linear
  between; the sources of Design note 11 re-checked. Tests (pure): the splits are monotonic and
  continuous in the height above ground; each sphere holds its slice; snapping moves a cascade by
  whole texels only, a carried grid never re-grids however the camera moves, the sun's turn slides
  it by at most that turn times the cascade's radius, and the body's rotation moves nothing in its
  axes; the fitted radius does not change as the camera turns; the matrices map the sphere and the
  depth range as stated, camera-relative; the weight is 0 where cascade 1's texel is coarser than
  the horizon map's. Acceptance:
  `pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain/shadows`.
  Records: [10-terrain-on-generated-worlds/records/R10.T9.a.md](10-terrain-on-generated-worlds/records/R10.T9.a.md).
- **R10.T9.b The high setting's shadow term** (after T9.a, T11, T12 and R10.T8.c; on the development
  machine). The `shadows` member of R01's `MemoryCategory` and the layers' creation: one set of four
  for the engine (64 MiB), drawn again for each photorealistic view that hands a body over on the
  high setting, before its terrain; the engine's depth-only layered target, a comparison sampler,
  `primitive.unclippedDepth` and `depthBiasClamp`; a caster pass for each cascade, drawing the
  view's selected patches with that view's own morph factors, culled by T9.a's caster reach; each
  view's four snapped centres kept from frame to frame and handed back to T9.a, and dropped when the
  view, its body or the pass is new; a list of other casters' draws for each cascade, for craft and
  R11's rocks. The horizon map everywhere and the cascades by `min`: min(1, c′ ÷ m), 1 where m = 0,
  with c′ = 1 − w (1 − c) and w T9.a's weight, in a function of its own from the camera-relative
  position, beside R11's `terrain_shadow_factor(p)`, whose signature stays (a body-scaled `f32` p
  places a point only to about 0.4 m on an Earth); the direct sun takes their product. Each cascade
  blends into the next over its last 10% and the last fades to lit; 2 × 2 PCF with Poisson taps; the
  normal offset and bias tuned by hand within 1–1.5 texels. The smoke harness's category list gains
  `shadows`. By hand, recorded, hidden on the discrete target (the development machine's RTX 3080):
  1080p against the budget's 1.5–3 ms; no double-dark band at a cascade's edge in a sunset scene
  (`TERRAIN TEST` looking north at 228–236 s); the sun's residual crawl at `TERRAIN TEST`'s 60 s/s;
  whether the radius needs steps against shimmer in a descent. Smoke run on SwiftShader, which lacks
  `depth-clip-control` and so runs the pulled-back path. Acceptance: the smoke run passes and the
  figures are recorded.
  Records: [10-terrain-on-generated-worlds/records/R10.T9.b.md](10-terrain-on-generated-worlds/records/R10.T9.b.md).

### R10.T10 The lit style on generated terrain

- **R10.T10.a Class photometry.** `albedo_scale` in `material/albedo.rs` (Design note 8): the exact
  piecewise-linear solve of Σ a_i c A_N,i [L_i + ⅔ (1 − L_i)] = p over the whole field, with the
  residual. _The limb ruling (decision-r10-limb §2, folded in): the solve uses each class law's
  L(0) of c A_N, L_i = `PhaseLaw::limb_parameter(c A_N,i)`, so p_i = c A_N,i [⅔ + ⅓ L_i]. For a
  particulate class it is linear in c between breaks where c A_N,i crosses 0.60 and 1.35
  (`LUNAR_LIMB_ONSET_NORMAL_ALBEDO`, `BRIGHT_FROST_NORMAL_ALBEDO`), since x L_p(x) is linear
  there; a consolidated class, under the Lambert law, has p_i = ⅔ c A_N,i. Each c A_N,i is clamped
  to its form's span, where one exists, and to its law's `normal_albedo_ceiling` (1 for Lambert's,
  1.35 for the particulate laws). The as-built rule's breaks at 0.815 and 1 are retired._ It is
  called by R09's `SurfaceService` once the field is built, with plan 14's visual
  geometric albedo p (the photometry section R07.T1 asks of plan 14), so that c ships in
  `FieldHeader.albedo_scale`. Filling the header changes the payload, which R09's Design note 17
  puts under `GENERATOR_VERSION`; as built it bumps nothing, since the payload's layout freezes only
  at the first send and no generated body is served before it (main's ruling for T10.a, 2026-10-14:
  Generator version, and Risks, "Deviations in T10.a, as built"; updated 2026-10-15). As built
  (R09's "Deviations in T17, as built"): the call site is `surface/service.rs`'s `compute`, after
  `coarse_pass`, where the body's record is in hand, its p the photometry's `geometric_albedo()`
  (by band); the tests that compare the served field with `coarse_pass` bit for bit
  (`tests/surface_service.rs` and two in `surface::service`) apply the same `with_albedo_scale`.
  Per band or one c with its residual per band (decision-composition §1.8) was asked in Risks,
  since the header holds one `Option<f64>`; main answered one c, fitted in V, with the B and R
  residuals stated and recorded. Files:
  `material/albedo.rs`, R09's `hyperion_server::surface` service,
  `crates/hyperion-sim/tests/surface_albedo_worlds.rs`, a server test. Tests: on each of
  `sample_worlds()` the area-weighted p of the scaled classes equals the body's p to 1% where the
  ranges allow, and the residual is stated where they do not; the clamped solve equals a bisection
  reference; no scaled class's c A_N exceeds its law's `normal_albedo_ceiling` (the limb ruling);
  the solve depends on the class fractions only; the server's `surface_field` answer
  carries `albedo_scale` as `Some(c)` equal to `albedo_scale(field, p)`, and `None` while plan 14's
  p is `not_modelled` (R09.T19.b has landed, `7392c064`, so the `surface_field` answer and its
  block 0 are tested over the socket in `crates/hyperion-server/tests/surface_knowledge.rs`: Risks,
  "Deviations in T10.a, as built"; updated 2026-10-15; the `sample_worlds()` binary follows
  R09.T16). Acceptance: `cargo test -p hyperion-surface material::albedo`,
  `cargo test -p hyperion-sim --features testing --test surface_albedo_worlds` and
  `cargo test -p hyperion-server surface`.
  Records: [10-terrain-on-generated-worlds/records/R10.T10.a.md](10-terrain-on-generated-worlds/records/R10.T10.a.md).
- **R10.T10.b The terrain shader.** `terrainMaterial.ts` and `shaders/terrainLit.wgsl`: the palette
  and weights to one lunar-Lambert law per class (A_N × c, L(0), and f(α) and L(α) from the class's
  row) through `body_brdf`, summed by weight, the normals at the setting's resolution, the shadow
  term from T8 or T9, the hooks `terrain_decoration`, `terrain_shadow_factor`, `terrain_ring_shadow`
  and `terrain_sky_factor` for R11 (each defaulting to no effect), the direct sun through R08's
  `atmosphere_sun_transmittance` and the sky through its `atmosphere_sky_irradiance`, and R08's
  aerial perspective on the output. Each call passes the texel's geodetic height above
  `BodyFigure`'s spheroid, its geodetic latitude (from the spheroid normal against the pole) and,
  for the transmittance, each sun's azimuth from local north, as R08 Design note 17's signatures
  require. As built (Consumes, R08): each call also passes the light's `curve` slot (0 for a
  planetshine source), and planetshine's direct term goes through
  `atmosphere_source_transmittance`; until R08.T9.b lands the two names are R07's four- and
  three-argument stubs in `litBody.wgsl`, which this task calls as R07's discs do. The aerial
  perspective comes from drawing the lit terrain into the `AtmosphereScene` that
  `HillaireAtmosphere.drawFrame` composites on the low setting (scope `"terrain"`), and before that
  composite on the high (`"scene"`). The terrain's footprint joins `photorealFrame`'s
  `depthWriters`. The ground's thermal emission, `Melt`'s and any texel's above R08.T18's
  visibility cut, goes through R08.T18's shared function once it lands (decision-composition).
  Each class of the patch's palette has its own
  `struct LunarLambert { a, l, s, table_row }` (a = A_N × c, l = L(0), the class's s and row),
  passed to R07's `body_brdf(law: LunarLambert, mu0, mu, alpha)`, whose signature R07.T4.c creates
  and this task uses unchanged. _The limb ruling (decision-r10-limb, folded in): `LunarLambert.l`
  is `PhaseLaw::limb_parameter(c A_N)` of the class's law, and the class laws are three, Regolith,
  Ice and Lambert (f ≡ 1, alpha 0, from R07's `lambert` template, provisional and labelled as R07
  has it), with a row for each distinct (law, L(0)), the law fixing its template and s: f is
  tabulated against L(α) = L(0) × McEwen's profile, so a particulate class at L(0) < 1 (a bright
  frost) takes a row of its own, and the Lambert law one row whatever its classes' albedos; row 0 is
  the body's uniform law (main's ruling for R10.T10.b, 2026-10-15); a consolidated class's row is
  Lambert's whatever its entry's code._ The
  texel reflects the classes' `body_brdf` radiances summed by its
  weights, skipping zero weights, not one `LunarLambert` built from weighted parameters, which
  differs from the sum by the covariance of A_N and L: 1.7× too bright at μ₀ = 0.5, μ = 0.1 for an
  even blend of regolith and snow (Design note 8). One `phase_factor_table` row per class law (its
  template, s and L(α)): f in r, g and b, L(α) in a; `body_brdf` reads L(α) from the row and
  `LunarLambert.l` carries L(0), so no row is built per texel and no f is taken from a neighbouring
  L (decision-r07-t8b). This task builds the channel in R07's files: `PhotometricLaw` gains an
  optional `lommelSeeligerCurve` (absent: L constant at `lommelSeeligerShare`, which is L(0)) in
  `appearance/law.ts`; Φ_shape with L(α) in `shapes.ts`, `phase.ts` (`lawFor`'s q) and `brdf.ts`;
  the spheroid integral at L(α) and `oblateAlbedoScale` at L(0) in `bodies/oblate.ts` and
  `bodies/draw.ts`; `shaders/litBody.wgsl` and `shaders/bodyDisc.wgsl`. It moves R07's L = 1
  templates (`moon`, `mercury`, `airless-ice`, `snowball`, `magma`) to McEwen 1996's L(α), which
  changes no p, and no q or point flux inside each template's range (past it the held f draws L(α)'s
  own tail: Risks, "Deviations in T10.b, as built"). Files: `terrainMaterial.ts`,
  `shaders/terrainLit.wgsl`; R07's `appearance/law.ts`, `shapes.ts`, `phase.ts`, `brdf.ts` and
  `templates.ts`, `bodies/oblate.ts` and
  `bodies/draw.ts`, `shaders/litBody.wgsl` and `shaders/bodyDisc.wgsl`, with
  `appearance/litBodyProbe.ts`'s `packPhaseFactorRows`, which writes the rows' alpha (0 as built);
  `displays/view/photorealFrame.ts` for the footprint. Tests: a pure test that a
  texel of two classes in weights w and 1 − w reflects w r₁ + (1 − w) r₂ to 10⁻⁶, r₁ and r₂ being
  the classes' own `body_brdf` radiances in R07.T4.c's twin, that for an even blend of regolith
  (A_N 0.12, L 1) and snow (A_N 0.9, L 0) sharing one f(α), at μ₀ = 0.5, μ = 0.1 it is that sum and
  not the 1.7× brighter law of the mixed parameters (unchanged by the limb ruling: snow is at L =
  0, and the shared f is the test's own construction), that a zero weight adds nothing, and that
  the largest weight's class is the one a readout names; the energy bound with f (the limb
  ruling, §3), in R07.T4.c's twin: for each row, with L(0) from the rule at A_N on a grid up to
  its law's `normal_albedo_ceiling`, A_h(μ₀) computed with the row's f and L(α) over μ₀ ∈ {1, 0.5,
  0.2, 0.1, 0.05, 0.02, 0.01, 0.005} is at most 1; for the Ice row at L(0) 0.38, sup η = 0.720 ±
  0.01 and η(1) = 0.636 ± 0.01 (η = A_h ÷ A_N); for the Lambert row, η ≡ 1 to 10⁻⁶; a one-class
  palette's texel equals the per-body
  law of that class in R07.T4.c's twin; the smoke harness renders a generated world's patch set on
  SwiftShader with every texel finite; a law without a curve draws R07's disc and twin unchanged
  (the row's alpha is its L); L(α) is 1 at 0°, 0.6084 at 30° and 0.1859 at 90°, and 0 from 103.9°;
  under it, the Moon template's p equals the constant-L law's exactly, its point flux to 10⁻⁴ at
  30°–120°, and its q falls by the held tail's 1.80 × 10⁻⁴, pinned to 5 × 10⁻⁶ and within 2 × 10⁻⁴
  (main's ruling for R10.T10.b, 2026-10-15), and its
  CPU-rasterised disc flux equals its point's to 1% at 30°, 90° and 120°. Acceptance:
  `pnpm --filter hyperion test` (the client suite: the Files reach R07's `appearance/`, `bodies/`
  and `shaders/`, which the view's `lighting`, `photoreal`, `post` and `scenes` tests also read) and
  `just test-render` (the WGSL).
  Records: [10-terrain-on-generated-worlds/records/R10.T10.b.md](10-terrain-on-generated-worlds/records/R10.T10.b.md).
- **R10.T10.c The survey edge.** The survey mask's discard, the skirts at the survey edge, and the
  reference surface drawn beneath with the inverse mask from the GPU coverage mask (Design note 12),
  in `terrainLit.wgsl` and the `mesh`-regime body's material; the edge mark over the image from
  `surveyEdges`, the guide's `SURVEY EDGE`. It first builds the per-stage storage visibility of
  Design note 15, unless R10.T8.b has built it: `StorageBufferSpec` gains an optional `stages`
  (`"vertex"`, `"fragment"` or both, both by default), `resourceLayoutEntries` writes each entry's
  own visibility, and the lit material declares `heights`, `slots`, `instances`, `contacts` and, on
  `baked-offsets`, `offsets` vertex only, and `class_layers` and `terrain_lights` fragment only; the
  mask is its own `survey-mask` slot field (65 × 65 bytes, four to a `u32`), bound to the vertex
  stage if read per vertex and interpolated, or to the fragment stage if bilinear by hand; if the
  split cannot be built, the mask joins R05's `heights` field, read as `u32` words through
  `bitcast<f32>`, never the reverse (main's ruling for R10.T10.b, 2026-10-15). Files:
  `shaders/terrainLit.wgsl`, `coverage.ts`, R07's `mesh`-regime material (`bodies/smoothMesh.ts`,
  `shaders/smoothMesh.wgsl`, `SMOOTH_MESH_MATERIAL` in `bodies/draw.ts`), `view/engine/types.ts`
  (`StorageBufferSpec.stages`), `view/engine/webgpu/materials.ts` (`resourceLayoutEntries`),
  `view/engine/platform.ts` (`requiredLimits`' doc: no binding-count limit is raised),
  `slotLayout.ts` (the `survey-mask` field, 4,228 B a slot) and `gpu/resources.ts`. Tests:
  `surveyEdges` returns exactly the boundary between surveyed and unsurveyed cells on a fixture
  coverage, margin cells counted unsurveyed; the smoke harness renders a half-surveyed patch set
  with no fragment where both terrain and the reference surface write; in `gpu/resources.test.ts`
  (the terrain's, not `view/engine/webgpu/resources.test.ts`) and `terrainPass.test.ts`, every
  terrain material, on both vertex paths and in lit and spike shading (and T11's wire once it
  lands), counts at most 8 storage buffers per stage by its declared visibility, and each binding's
  declared stages match the entry points that use it (a static check over the composed WGSL; the
  smoke harness's pipeline creation on SwiftShader validates it too). By
  hand, recorded: the skirt heights seen at the edge on a flattened generated world, which stay
  within the terrain's local relief. Acceptance:
  `pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain src/renderer/src/view/engine`
  (the `view/engine/` files it changes) and the smoke run. _As built (Risks, "Deviations in T10.c,
  as built")._
  Records: [10-terrain-on-generated-worlds/records/R10.T10.c.md](10-terrain-on-generated-worlds/records/R10.T10.c.md).
- **R10.T10.d The class map on the disc.**
  Records: [10-terrain-on-generated-worlds/records/R10.T10.d.md](10-terrain-on-generated-worlds/records/R10.T10.d.md).
  - **The map.** `classMap.ts` builds the per-body coarse class-weights map over surveyed cells
    that R07's disc samples (Design note 8), under `coarse-field-gpu` with the coverage mask. It
    goes through R07.T8.b's construction as built. `classMap.ts` supplies each texel's weights
    to `packClassMap` and `classMapSurface` (R07's `bodies/discSurface.ts`):
    - `weights_at` at the cell's centre, byte ÷ 255;
    - `null` where the cell is unsurveyed;
    - a partly surveyed cell's weights times its surveyed fraction.

    `laws` are one per class, from Design note 8's parameters and T10.a's scale (at most 16,
    `MAX_DISC_CLASSES`). `elsewhere` is R07's uniform `lawFor(p, q, template)`. `classMap.ts`
    owns the texture: it remakes it after a device loss and releases it with the map. Folded in
    at the re-validation (decision-composition §1.8): the map's classes are the body's (form,
    entry) pairs, at most 16, which retires R07's "a body carrying all three `FrostSpecies` would
    need 17". As built, `classMapTextureSpec` files the map under `"other"`; this task gives it
    `coarse-field-gpu`.

  - **The reconstruction.** The disc and `rasteriseDisc` read the map by a survey-masked bilinear
    reconstruction, which replaces T8.b's nearest read (decision-r07-t8b):
    - A sample in an unsurveyed texel takes `elsewhere` alone.
    - A sample in a surveyed texel interpolates the full share vector (the classes and
      `elsewhere`'s 1 − Σw) over the four nearest texel centres. It counts surveyed texels only
      and renormalises.
    - Each face carries a one-texel gutter from its neighbours (N + 2 texels a side, the gutter
      corners unsurveyed), so the reconstruction is continuous across face edges.

    No surveyed pattern shows past the survey's cells, and the cells show no blocks.

  - **The view.** `litBodiesOf` (`displays/view/photorealFrame.ts`) passes each body's `surface`
    from the class-map store and its `rotation` from the scene body's `rotation` (body-fixed to
    the body frame, whose axes are the galactic ones), taken at the body's drawn time (R07.T10.a).
    `viewFrameDrawer.ts`, which `ViewDisplay.tsx` makes and which builds `PhotorealInputs`, passes
    the store. As built, `litBodiesOf` never sets `surface`, and `discSurfaceOf(appearance)`
    (`appearance/bodyAppearance.ts`, where `DiscSurface` is declared) is always uniform.
  - **Until the rotation arrives.** The scene body's rotation comes from P14.T46.f's rotation
    section, which R07.T2.b reads. Until then `rotation` is absent, and a live class map shades
    with `elsewhere`, as R07 built it. _As built, both are built (`BodyRotationDto` on
    `BodySummaryDto.rotation`), so a body has a rotation wherever its record's section is `ok`._
  - **Files.** `classMap.ts`; R07's `bodies/discSurface.ts`, `bodies/discShading.ts` and
    `shaders/bodyDisc.wgsl`; `displays/view/photorealFrame.ts`, `displays/view/viewFrameDrawer.ts`
    and `displays/view/ViewDisplay.tsx`.
  - **Tests:**
    - The class map holds no texel for an unsurveyed cell.
    - R07's disc under a class map of one uniform class equals its `lawFor` disc to 10⁻⁴.
    - At a surveyed texel's centre the shares are its own. A sample in an unsurveyed texel
      carries no surveyed weight.
    - A linear ramp of weights across a face edge stays linear to 10⁻⁶.
    - The half-surveyed two-class map's disc flux equals the area-weighted fluxes to 1%.
    - `litBodiesOf` passes a rotation where the scene body has one and none where it is `null`,
      and such a body's disc shades with `elsewhere`.
    - The smoke harness renders a disc with a half-surveyed class map, every texel finite, and
      equal to `rasteriseDisc` within R07.T8.a's tolerance.
  - **Acceptance.** `pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain
src/renderer/src/view/bodies src/renderer/src/displays/view` and `just test-render`.
  - _As built (Risks, "Deviations in T10.d, as built")._
- **R10.T10.e The disc hands over.** R07's `disc` regime gives way to the quadtree at the range of
  Design note 13, per view and style. Tests: the handover range as a pure function (about
  1.8 × 10⁷ m for 10 km of relief at 1080p and 1 px, and 4.6 × 10⁶ m at the wireframe's 4 px), and a
  body with no survey never hands over; per class law, the disc's and the terrain's I/F at the same
  (μ₀, μ) agree to 10⁻⁶ at 30° and 90° of phase, one row read by both. By hand, recorded: at phase
  angles of about 30° and 90°, the body's mean brightness across the handover changes by less than
  1/3 stop and, averaged over each class-map texel, by less than 1/3 stop on a fully surveyed
  generated world; the residuals of T10.a against plan 14's p and q. Acceptance:
  `pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain` and the recorded check.
  _As built (Risks, "Deviations in T10.e, as built"): the hand-over is `view/terrain/handover.ts`,
  R07's regime `terrain` and an optional terrain input to the frame, measured to the body's limb;
  the terrain's class laws take the disc's figure scale; the live pipeline that would feed the
  input is not built (asked of main), so the check by hand ran scripted on the Moon-like fixture
  and stays pending on screen._
  Records: [10-terrain-on-generated-worlds/records/R10.T10.e.md](10-terrain-on-generated-worlds/records/R10.T10.e.md).
- **R10.T10.f The point under a class map.**
  Records: [10-terrain-on-generated-worlds/records/R10.T10.f.md](10-terrain-on-generated-worlds/records/R10.T10.f.md).
  - **What it does.** R07's point regime (`pointFlux` in `bodies/draw.ts`) integrates the body's
    class map wherever its `surface` is a class map and its `rotation` is known, so that the flux
    stays continuous at the 3 px switch (R07 Design note 5, T8.a's 1%; decision-r07-t8b).
    Elsewhere it keeps the photometry's law, which equals `elsewhere`.
  - **The integral.** It uses the disc's own law arithmetic in `f64`, summed over lit, visible
    surface elements in body-fixed axes: each element's shares, as T10.d reconstructs them, times
    each law's I/F, μ and the element's area, on the spheroid where the figure is one. Each light
    takes its eclipse term from the centre, as now. _R07.T10.b: the point now takes each light's
    `discEclipseVisible`, the eclipse averaged over its disc, not the centre's term, so that it
    meets the disc at 3 px through an eclipse (R07's "Deviations in T10.b, as built")._ Texels are
    sub-sampled as the bound needs.
  - **Caching.** The result may be cached on the sun's and the camera's body-fixed directions
    while it stays within 0.3% of a fresh integral.
  - **Files.** R07's `bodies/draw.ts` and `bodies/discSurface.ts`.
  - **Tests**, on a synthetic Iapetus (a 70° dark cap about the apex at A 0.04, the rest 0.55):
    - Facing the leading side, the trailing side and a pole, at 0°, 60° and 120° of phase, the
      point's flux equals the CPU rasteriser's summed disc flux at the switch to 1%.
    - It equals an `f64` brute-force integral over 2 × 10⁵ Fibonacci directions to 0.3%.
    - A one-class map equal to the uniform law gives the uniform point to 10⁻⁶.
    - A class map without a rotation gives the photometry's point exactly.
    - A cached value is within 0.3% of a fresh one.
  - **Acceptance.** `pnpm --filter hyperion exec vitest run src/renderer/src/view/bodies` and
    `just test-render`.
  - _As built (Risks, "Deviations in T10.f, as built")._
- **R10.T10.g The hand-over's photometry** (new, decision-r10-t16-leans §1 and §13). The hand-over
  is measured at the presented width's centre pixel, as the field's want is, so that the resolution
  controller never moves it. The disc shades surveyed ground with the field's own normals: a
  per-body normal map at the field's level over its surveyed cells, made by the workers beside the
  class map (`coarse-field-gpu`), read by R07's disc in its `class-map` case and by T10.f's point
  integral, so that disc and terrain share their cell-scale slopes at the switch. A cross-fade over
  the hysteresis band only if a texel still changes by more than 1/3 stop. The survey's +0.04 stop
  rise at the terrain's first draw at 90° traced (each pixel's law and shading normal, in a scratch
  build) and fixed if it is a law or albedo mismatch. T10.e's harness committed as a check-only
  variant of the terrain run, for T15. Tests: the hand-over range independent of the frame's
  internal scale; with the normal map the disc's I/F equals the terrain's at the same cell normal,
  per class, to 10⁻⁶; the point's flux at the 3 px switch within R07's 1% with the normals; the
  harness at 30°, 90° and a view near the terminator (incidence 75° or more), both settings, every
  texel of at least 2 shared pixels within 1/3 stop at 30° and 90°, the terminator's recorded.
  Design note 13's premise and Design note 8's disc recorded.
  _As built (Risks, "Deviations in T10.g, as built"): the normal map is the surface crate's
  `WorkerFields::normal_map`, carried in the workers' `class-map` answer; the disc takes μ₀ and μ
  on it, the area on the spheroid's, μ floored at half the spheroid's; the point's elements take
  k × k normals at levels 7 and 8; the rise is the survey's 1.6° regional slope, not a law or an
  albedo (traced on the CPU twins); the check is `--views-check --terrain --handover`; every judged
  cell is within 1/3 stop at 30° and 90° on both settings (worst 0.093 stop), and the terminator
  keeps 2 cells over on high, deferred; no cross-fade (ruled)._
  Records: [10-terrain-on-generated-worlds/records/R10.T10.g.md](10-terrain-on-generated-worlds/records/R10.T10.g.md).

### R10.T16 The live terrain path

Records: [10-terrain-on-generated-worlds/records/R10.T16.md](10-terrain-on-generated-worlds/records/R10.T16.md).

New at the scoping of 2026-10-16 (`decision-r10-live-path.md`): no task made the app's half of
Design notes 12, 13 and 16, so T7's store, T6.b's fields, T10.b's pass, T10.c's survey edge,
T10.d's class map and T10.e's hand-over were built and tested, but nothing in `VIEW` made them
(their records: "not wired yet", "the first consumer's"). This task wires them, in four subtasks
of one batch, after T10.c–f and before T11 and T12, which are built on it. T10.e's names
(`FrameTerrain`, `handover.ts`, the `terrain` regime) are its record's. Every GPU and Electron run
here is hidden (offscreen), never on the owner's display; those on the development machine's RTX
3080 are recorded with the machine, setting and date, and the owner's UHD 620 runs stay his, by
hand. Generator version untouched (22): presentation alone, its one new golden re-blessed at the
version in force.

- **R10.T16.a The terrain session.**
  Records: [10-terrain-on-generated-worlds/records/R10.T16.a.md](10-terrain-on-generated-worlds/records/R10.T16.a.md).
  - **What it holds.** `TerrainSession` (`view/terrain/session.ts`), for one universe, one engine
    and one request client:
    - the `CoarseFieldStore` (T7) over one `HeightWorkerPool`, of Design note 11's default worker
      count (`clamp(⌊hardwareConcurrency ÷ 4⌋, 1, 3)`, as `defaultSpikeWorkers` gives it), one a
      session whatever the views and bodies;
    - for each wanted body: its `CoverageMask` (T10.c), fed by the pool's `onFieldChange` runs and
      the store's coverage as T10.c built it; its class map (T10.d's
      `ClassMapStore.want(body, detailSeed)`); and its detail seed;
    - each wanted body's survey for T12's `SurveyLine`: the store's status, coverage and header,
      stale while the link is down.
  - **Where it is made.** `useTerrainSession` (`displays/view/useTerrainSession.ts`) in
    `ViewPanels` makes it for the stage's source: the open universe's scene, over the link's request
    client (`useServerLink`), or `TERRAIN TEST`'s, over its fixture (T16.d); no other kept scene has
    one. It is disposed with the engine, the universe and a change of source, so that a stage
    remounted on a new scene of the same source keeps its fields. `VIEW` hidden disposes the engine,
    and the session with it; shown again, the fields are fetched again (T14 states the cost).
  - **Its inputs.**
    - The link: `setLinkUp` from `useServerLink().status`.
    - The scene's `surface_revisions`: R03's scene model passes each notification's list on
      (`lib/scene/sceneWire.ts`'s `applySceneNotification` keeps it on its update, and `useScene`
      hands it to a listener, not to state, so that it renders nothing) to `noteSurfaceRevisions`.
      A resubscription or a sequence fault asks every kept field again with its revision, as the
      link's return does.
    - The detail seed: each wanted body's `BodyHooksDto.detail_seed`, from one `body_detail`
      (`toBodyDetailRequest`, `lib/system/bodiesWire.ts`) a want; a kept scene's body carries its
      own. A body whose hooks are not modelled is wanted for its coverage alone, with no class map
      and no terrain. R08.T10.a, which also reads `body_detail`, shares the fetch: whichever lands
      second takes the other's.
    - The views. Each view's loop, the primary's and each instrument's, tells the session, each
      frame it draws, its camera (pose, field of view, presented size), its style, the setting, its
      `ViewBudget.streamPriority` and its last plan's regimes. The primary's frame then steps the
      session once: `follow` over every view's `FieldCandidate`s, `local` from the view's camera
      frame (`sceneFrameFor`) and `rangeRatio` from T10.e's `handoverRangeRatio` at the view's
      style's τ (`handoverTauPx`). As main ruled for T7, a body with no header is wanted only as a
      view's local body.
  - **What it gives the views.** T10.d's `classMaps` and T10.e's relief source
    (`PhotorealInputs.reliefs`), through `ViewStage`, `viewFrameDrawer.ts` and `useInstruments` to
    every view's frames, the instruments' included; `ViewPanels`' `classMaps={null}` goes.
  - **Memory.** The masks and maps are filed under `coarse-field-gpu`. For T14 and R12's memory line
    (R12 Design note 6) the session reports the workers' copies (`fieldResidentBytes`), the pool's
    kept blocks (T7's "The pool's own copy") and its GPU bytes by category.
  - **Teardown**, in order: the passes, the class-map store, the masks, the store (its requests
    cancelled and its fields released), then the pool (its workers terminated).
  - **Files.** `view/terrain/session.ts`, `displays/view/useTerrainSession.ts`, with their tests;
    `displays/view/ViewDisplay.tsx`, `viewFrameDrawer.ts` and `useInstruments.ts`; R03's
    `lib/scene/sceneWire.ts` and `useScene.ts`; `view/terrain/coarseFieldStore.ts` where a seam
    needs it.
  - **Tests** (Vitest, R03's `FakeWebSocket`, `test/scriptedHeightWorker.ts`,
    `test/countingRenderEngine.ts`):
    - it is made for the server's scene of the open universe and for `TERRAIN TEST`, and for no
      other kept scene; after its disposal no worker, request, timer, listener or GPU handle is left
      (the counting engine's tally back to zero);
    - a new scene of the same source asks for no field again;
    - a view's local body is asked for with no header held; another body only once some view is
      within its hand-over range in that view's style, released once every view is beyond twice
      it; a wireframe view (4 px) and a photorealistic one (1 px) are each judged by their own;
    - a revision rise on the scene topic asks for the delta alone; a link lost and back, and a
      resubscription, ask again with the revision held;
    - the detail seed is asked once a want and reaches the class-map store; a body without hooks
      gets no class map;
    - a block's surveyed cells reach the body's mask before the store's answer; after a device
      loss the masks hold the same cells and no request is asked again;
    - every view's lit bodies take the class map and the relief, the instruments' included;
    - one pool a session, of Design note 11's count.
  - **Acceptance.** `pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain
src/renderer/src/displays/view src/renderer/src/lib/scene`.

- **R10.T16.b A body's terrain pass, shared by the views.**
  Records: [10-terrain-on-generated-worlds/records/R10.T16.b.md](10-terrain-on-generated-worlds/records/R10.T16.b.md).
  - **The pool's lanes.** `HeightWorkerPool.lane(bake: BakeSettings)` gives a pass its own lane
    (`reprioritise`, `onBaked`, `onFailed`, `waitingCount` and `terminate`) over the pool's one
    queue, workers and fields.
    - A lane's demand replaces its own alone.
    - The queue takes forced requests first, then the larger w × ρ ÷ τ whatever the lane (Design
      note 24's priority is a weighted excess, comparable across lanes), then the lane's order and
      the key's.
    - A lane's bakes reach its own listeners alone.
    - Terminating a lane drops its queued, waiting and in-flight requests and leaves the fields,
      the class-map queries and the other lanes.
    - The spike's pool, made with its one `bake`, is a pool of one lane and keeps its tests.
    - `TerrainPoolFactory` gives the pass a lane, so `TerrainPass`'s rebuild after a device loss
      takes a new lane of the same pool and the fields are untouched.
  - **The views.** `TerrainPass` takes the views that hand its body over (T10.e's `terrain`
    regime in each view's last plan).
    - Selection runs once a step over them all (`selectPatches`' `views`, at most
      `MAX_SELECTION_VIEWS`). Each view brings its own τ (`handoverTauPx`), presented size (R05
      Design note 23) and weight (`PRIMARY_VIEW_WEIGHT` or `SECONDARY_VIEW_WEIGHT`, from
      `ViewBudget.streamPriority`), over one cache and one lane.
    - Each view draws its own `DrawItem`: its own instance records (the draw set's patches its
      frustum meets, relative to its own camera, at its own morph bands, `morphRangeM` at its τ and
      size), its own lights buffer and its own annunciation debounce.
    - The spike's single view keeps its behaviour.
  - **The level table.** Each block's level table (`FieldChange.levels`, the pool's `fieldLevels`)
    reaches the pass through `TerrainPass.setLevels`, which reselects. Until T4.b, selection reads
    the table as built, its bound column the hard bound.
  - **The rebake rule** (Design note 16 as built by T6.b and T8.b).
    - The pass keeps, for each resident patch, what `needsRebake` reads of its layers (`complete`,
      `horizonComplete`, `unsurveyedCells` and `horizonRefusedCells`).
    - At each `FieldChange` of its body, the pass asks again for each patch the rule names, at its
      last priority, while the old bake stays drawn. The new bake replaces it in its slot
      (`PatchCache.insert` of a resident key).
    - The in-flight rule is the pool's as built (`staleInFlight`).
    - The body's mask takes the block's newly surveyed cells once the rebakes that block named have
      landed, or at once where no pass draws the body. Neither surface then leaves a hole along the
      survey's advancing edge (T10.c's deferred "A hole while a rebake is due", settled).
  - **Unsurveyed ground.** Selection takes a survey lookup beside its height ranges (an optional
    `SelectionInput` member, from the body's coverage). A patch with no surveyed field cell under
    it is exact, since the reference surface is the ground there (Design note 12). It is not
    refined, demanded or drawn, and the body's figure draws it. No bake is made of ground that is
    not coming, and it counts toward no `TERRAIN: STREAMING`; patches waiting for a block of a
    surveyed region (the lane's `waitingCount`) do count (Design note 16). _From
    decision-r10-t16-contracts §2–3:_ the lookup is conservative (a patch is surveyed if any coarse
    cell its vertices' masks read is surveyed), every new selection member is optional, and a
    caller with no lookup gets today's selection bit for bit; `Selection.nearestSurveyM` (metres,
    never under one finest patch, `+∞` with no lookup or no surveyed cell) gives the move rule a
    distance only where no selected patch above the finest level does. Unsurveyed ground wins over
    the forced region: a patch with no surveyed cell is never forced, demanded, refined or drawn,
    wherever a contact stands; a contact astride the edge forces exactly its surveyed patches, under
    Design note 9's rule in full. Collision's ground there is `ground_at` over the server's whole
    field, not the reference surface, which is the picture's ground alone. The mask's invariant: in
    every view where the mask is given, each cell the GPU mask holds surveyed lies under a drawn
    patch, selected or standing in, whose bake's mask holds that cell. A drawn patch of cell code 0
    (six or more levels coarser than the field) whose bake holds a vertex outside the survey cannot
    hold its cells at its vertices, so the view's mask is withheld while it sees one; that raises
    no `TERRAIN: STREAMING` by itself (decision-r10-t16-review §2).
  - **Contacts.** The scene's craft over a handed-over body are its `GroundContact`s where R05's
    Design note 9 has them grounded or descending (`isDescending`: under 1 km over the datum and
    closing, or under 30 s from contact), at their hulls' bounding radii (`TEST HULL`'s until hulls
    are defined). The craft's height and ground point come from one function, the datum's until
    T14.c gives it the workers' ground (decision-r10-t16-contracts §2).
  - **Faults.**
    - A worker that fails is replaced as built.
    - Once every worker is lost (`lostWorkers` equal to the pool's size), every view that hands a
      body over states `TERRAIN NOT AVAILABLE: height computation failed, reopen VIEW to retry`
      (`TERRAIN_NOT_AVAILABLE`, `annunciation.ts`) on its label block's fault line, in
      `--status-caution`, after any graphics fault; its terrain annunciation is then `null`
      (decision-r10-t16-contracts §7, the string and its guide row ruled). The passes draw nothing
      more and the bodies their figures. The session is made again when `VIEW` is next shown, which
      is the retry.
  - **The releases** (decision-r10-t16-contracts §1). R01's `RenderEngine` gains `releaseMesh` and
    `releaseMaterial`, append-only, as R06.T13.h added `releaseBuffer` and `releaseTexture`;
    `ResilientEngine` forwards them and drops a lost engine's handles. `TerrainPass.dispose`
    releases every handle the pass made on the current device (through `TerrainResources.dispose`:
    the slot buffers and records, the normals atlas, the instance, contact, indirect and lights
    buffers and the phase table; then the patch mesh, the material and the tables of no air), and a
    view that stops handing the body over releases its own draw resources.
  - **Files.** `view/terrain/workers/pool.ts`, `terrainPass.ts`, `select.ts`, `cache.ts` (where
    the replacement needs it), `grounded.ts`, `view/terrain/session.ts`; `view/spike/spikeRun.ts`
    for the factory's type; the smoke harness's new group. R01's
    `view/engine/{types,resilientEngine}.ts`, `view/engine/webgpu/{engine,meshes}.ts`,
    `test/{fakeRenderEngine,countingRenderEngine}.ts` and `displays/view/check/probedEngine.ts`;
    `displays/view/ViewLabelBlock.tsx` (its `faults` list) and `InstrumentView.tsx`.
  - **Tests** (Vitest, pure, and over the scripted worker and the counting engine):
    - the lanes, each sentence above; the spike's pool and pass tests pass unchanged;
    - two views at one pose request each patch once; a patch only an instrument wants ranks at a
      quarter of the primary's; each view draws only the patches its frustum meets, at its own
      morph bands; R05's history-independence test holds with two views; a view that leaves
      releases nothing the other draws;
    - a widened level table reselects, and the bounds are its own;
    - after a block, exactly the resident patches `needsRebake` names are asked again, each drawn
      until its new bake replaces it, and none the rule passes over; on the module, over
      `fieldsWasm.test.ts`'s golden blocks, a patch passed over bakes the same bytes;
    - the mask takes a block's cells only once the block's rebakes have landed;
    - a patch with no surveyed cell is not refined, demanded or drawn, and a camera over unsurveyed
      ground raises no `STREAMING`; a patch waiting for its margin raises it;
    - after a device loss the cache and the draws are made again and a new lane is taken, and the
      scripted workers see no `field-init` or `field-chunk`;
    - with every worker lost there is no demand and no draw, and the fault stands on each handing
      view's fault line, after a graphics fault, with no `TERRAIN: STREAMING`;
    - a craft at 900 m closing is a contact and its region is forced; one at 2 km receding is not;
      a contact over unsurveyed ground forces nothing, one astride the edge exactly its surveyed
      patches; the mask's invariant holds block by block, a block under a resident stand-in
      included;
    - the releases: R01's engine tests of `releaseMesh` and `releaseMaterial`, and over the counting
      engine a pass made, drawn, restored and disposed leaves no handle of its own, 50 hand-overs
      and releases leave the tallies where they started, and a view that leaves releases its own;
      the smoke harness makes and disposes a pass three times with the allocation tally unchanged
      (decision-r10-t16-contracts §1–3 list the rest);
    - the smoke harness's `R10.T16 the shared pass` (part 6): two views of one lit pass over the
      Moon-like fixture's survey on SwiftShader, every frame finite and each view's terrain at its
      pose; and a device destroyed mid-stream, after which the terrain returns with no block posted
      again.
  - **Acceptance.** `pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain
src/renderer/src/view/spike` and `just test-render --part=6`.

- **R10.T16.c Terrain in `VIEW`.**
  Records: [10-terrain-on-generated-worlds/records/R10.T16.c.md](10-terrain-on-generated-worlds/records/R10.T16.c.md).
  - **The frame's terrain.** The session gives each view's photorealistic frame its own
    `FrameTerrain` (T10.e).
    - `draws(plan)`: for each `MeshBodyPlan` whose `role` is `handed-over` (T10.e's `terrain`
      regime), the body's pass's draw for this view, with:
      - `TerrainView` from the plan's body: `rotation` the `LitBodyInput.rotation` the plan drew
        the figure with, so that the terrain, its mask, its class map and the reference surface
        share one drawn rotation (R07.T10.a's `rotationAt`); `cameraM` the body's `centreM`
        negated (the camera from the body's centre, along the galactic axes, which are the body
        frame's); the camera's orientation; the field of view; and the presented size;
      - `TerrainFrameInput.lighting` from the body's disc record (`plan.discs[mesh.index]`): its
        lights, occluders and planetshine sources rotated from the galactic axes into the
        body-fixed ones by the inverse of that rotation, its `elsewhere` and the frame's
        pre-exposure (`terrainLightingOf`, pure); `sunDirectionBodyFixed` and `sunIlluminanceLx`
        the brightest light's; and the body's atmosphere tables where R08.T10.a's medium holds
        them (`TerrainFrameLighting.atmosphere`), the tables of no air otherwise.
    - `surveyCoverage(body)`: the body's mask, given only while this view's draw set of the body
      leaves nothing missing and draws no patch of cell code 0 with a vertex outside the survey
      (decision-r10-t16-review §2). The reference surface stays whole until the terrain draws all
      the surveyed ground the mask would discard.
    - The pass is made at a body's first hand-over in any view and disposed when the store releases
      its field, not at a hand-back. A body whose rotation is not known is never handed over: its
      figure and disc stand.
  - **The aerial perspective** stays R08.T10.b's. The terrain draws in the scene target's `bodies`
    pass (T10.e), which R08.T10.b's composite covers on both scopes once it draws the atmosphere in
    the view. Until then the view draws no atmosphere over anything, and a generated body reads
    `ATMOSPHERE: NOT YET MODELLED`, as R08 has it. Whichever of this subtask and R08.T10.b lands
    second binds the handed-over body's tables and tests the composite over the terrain.
  - **The label block.** Each view's `labelStatements(run, terrain)` takes the annunciation of the
    body it hands over (`TERRAIN: STREAMING` or `TERRAIN: DETAIL LIMITED`), the primary's and each
    instrument's its own; T16.b's fault goes through the label block's fault lines, after any
    graphics fault, not through `labelStatements`, whose parameter stays R05's
    `TerrainAnnunciation | null` (decision-r10-t16-contracts §7).
  - **The survey edge**, in both styles: `DrawOptions.surveyEdges` (T10.c), from each wanted body's
    coverage in view, its rim at max(drawn height, 0). The drawn height comes from the resident
    bakes whose survey mask holds the edge, their heights kept on the render thread for those
    patches alone, and is the datum where none is resident.
  - **The list's notes.** Each view's list notes each body whose survey edge is in its picture,
    `SURVEY EDGE <designation> <range>`, nearest first, in `SLANT RANGE`'s units (`m` below 1 km,
    `formatSurfaceRange` in `lib/format.ts`), outside the options and unselectable, with the
    options' accessible-name form, stale with the list (the guide's Views; `signoff-12.md` item 28;
    decision-r10-t16-review §1). Files: `view/terrain/surveyEdge.ts`, `lib/format.ts`,
    `displays/view/ViewMarkList.tsx`, `viewRun.ts`, `ViewDisplay.tsx`, `useInstruments.ts` and
    `InstrumentControls.tsx`; tests: the formatter's units, figures and hysteresis, the note's two
    forms, the list's notes (order, names, never an option, stale, none without an edge) and
    `VIEW`'s primary and instrument lists in both styles.
  - **The instruments** draw the same terrain, from their own cameras at their own τ and weight,
    through the same session.
  - **The footprints.** A handed-over body writes depth as T10.e's `terrain` regime counts it. T10.b's
    `PhotorealInputs.terrainFootprints` is fed only if T10.e leaves the body's footprint to it, and
    is removed otherwise, so that no footprint counts twice.
  - **Files.** `view/terrain/session.ts`, a new `view/terrain/frameTerrain.ts`;
    `displays/view/ViewDisplay.tsx`, `viewFrameDrawer.ts`, `useInstruments.ts`, `InstrumentView.tsx`
    and `photorealFrame.ts`; the callers of R02's `view/wireframe/drawList.ts`.
  - **Tests:**
    - under the identity rotation, `terrainLightingOf` gives the record's lights unchanged; under a
      rotation it gives their directions rotated, exactly; with a one-class palette at the sub-solar
      point, the terrain's I/F equals the disc record's to 10⁻⁶ (T10.e's agreement, now through the
      live path);
    - a body's `TerrainView` is its plan's: the camera's offset from its centre and the rotation,
      bit for bit;
    - a body handed over in one view draws its terrain there, and in another view only where that
      view hands it over too; its pass is made once, at the first hand-over, and disposed at the
      field's release, not at a hand-back; a body with no rotation is not handed over;
    - the mask is given only once the view's draw set of the body has nothing missing;
    - each view's label block carries its own annunciation;
    - the survey edge's rim stands at the resident bake's height where one holds the edge, and on
      the datum elsewhere, in both styles;
    - the smoke harness's `R10.T16 the frame` (part 6): a photorealistic frame with the fixture's
      body handed over, the terrain drawn over the reference surface, no pixel drawn by both on the
      datum, and every texel finite.
  - **Acceptance.** `pnpm --filter hyperion test` (its files reach `displays/view`,
    `view/photoreal`, `view/bodies` and `view/wireframe`) and `just test-render`.

- **R10.T16.d The terrain test scene and the hidden run.**
  Records: [10-terrain-on-generated-worlds/records/R10.T16.d.md](10-terrain-on-generated-worlds/records/R10.T16.d.md).
  - **The scene.** A kept scene, `TERRAIN TEST` (`view/scenes/terrainScene.ts`, in `SCENE_OPTIONS`),
    under `TRAINING`.
    - Its bodies: a Sun-like star, and an airless Moon-like body, `TEST MOON`, of the fixture
      field's figure, detail seed and body ID, with a rotation and a hand-made photometry.
    - Its camera is scripted from beyond the hand-over range down to a few kilometres over the
      survey and across its edge, with a sunset over the surveyed ground at its `SCENE CLOCK` rate.
    - Its field is served by a fixture request client (`requestBulk` for `surface_field`) from
      committed payloads: a first payload, then a delta at a revision rise that the scene's
      notifications announce, as R09.T19.b answers them.
    - The payloads are a golden of the worker's writer (`crates/hyperion-surface/tests/golden/worker/`,
      header 22, re-blessed with the worker's golden at the version in force), read with `?raw` as
      `smoke/litTerrain.ts` reads `moon_like.golden`. They are `synthetic_worlds()`'s Moon-like world
      with relief, two surveys and a margin's edge in view, no larger than the crate's largest golden
      today (about 360 kB). _From decision-r10-t16-contracts §4–5:_ a new
      `worker/terrain_test.golden`, its header `crate::generator_version()`, with `moon_like.golden`
      byte-identical and the version test pinning the two names; at least two payloads, a first and a
      delta at the next revision that extends the first survey, several blocks each, the survey
      crossing a cube-face edge within a few cells of a cube corner; each payload's revision, cover
      and `change` lines and the header's DTO values beside its blocks, held to the server's own
      conversion by a `hyperion-server` test; at most 361,605 B (`cube_sphere.golden`'s size), held
      by a native-only test; read lazily on the client. The option carries the fixture
      (`SceneOption.terrain`) and the body its seed (`ViewBody.detailSeed`); `KeptScene` is
      untouched. `TERRAIN TEST` sits in system `5e5e000000000000`, `TEST MOON` its `.0003`, its ID,
      seed and figure read from the golden; every other body takes `.0000` or an index of 7 and up.
      Revisions only rise, whatever the script does.
  - **The stand-in's caller.** `just place-ship` gains `--survey <source> <resolution>`, which sends
    R09's `survey_pass` (R09's Risks, "The survey stand-in") over the target body's cells, so that a
    served world can be surveyed by hand once the serve gate opens. Nothing else sends it.
  - **The guide.** Drafts, for the owner's sign-off under his delegation:
    - the `TERRAIN TEST` scene row;
    - `TEST MOON`'s place in the designation row, and the `SCENE` row's list;
    - the survey line's state while a body's field is refused or unreadable (the store's
      `failure`), lean `SURVEY COVERAGE NOT AVAILABLE: survey not received`, for T12 to build.

    Built to the leans, and the guide edited as the sign-off directs. T16.b's fault and its guide
    row are ruled (decision-r10-t16-contracts §7) and applied as adopted text, with no sign-off
    item.

  - **The hidden run.** The views-check harness gains a terrain run,
    `just views-check --hidden --terrain`, offscreen on `TERRAIN TEST`.
    - One launch a setting (decision-r10-t16-contracts §6): `--terrain` runs at its `--setting`,
      and with none the script makes two launches, high then low, its exit the first non-zero one.
      On high: a photorealistic primary alone through the hand-over, the descent and the sunset;
      the same with a photorealistic and a wireframe instrument; and a device loss forced
      mid-stream, the harness destroying the device as R01's loss tests do. On low: the primary
      alone; the primary with a wireframe instrument; and the forced loss. Its own phase, record and
      results types (`TerrainCheckPhaseName`, `writeTerrainResults`), `ViewsCheckLaunch.terrain`
      required; full runs under `docs/measurements/terrain-check/`, smoke runs under
      `target/views-check/terrain/`. _Since R10.T10.g the launch's kind is one field,
      `ViewsCheckLaunch.run` (`"several-views"`, `"terrain"` or `"handover"`)._
    - The forced loss is expected exactly once: one `device-lost` with reason `destroyed` inside the
      loss phase's window, then one restore; the faults verdict excludes that one alone. After it
      the terrain is drawn again, no `surface_field` asked and no `field-init` posted, and no
      `field-chunk` but the blocks of the answer in flight, each once to each worker, and the masks
      hold the same cells.
    - It holds every frame drawn and finite, the terrain drawn once handed over, no field asked for
      again after the loss, and the session torn down at the end. "No pixel drawn by both surfaces
      on the datum" is held by T16.c's part-6 group, not by the hidden run.
    - It records, provisional, the patches drawn, the bakes a second, the frame times and the
      session's memory report.
  - **By hand, hidden, recorded** on the development machine (RTX 3080), each in its own task's
    entry:
    - T8.b's sunset over the survey, at its three places;
    - T10.c's edge;
    - T10.d's disc under a live survey;
    - T10.e's mean brightness across the hand-over at about 30° and 90° of phase, over the surveyed
      texels; its fully surveyed world waits for a served one (T15).

    The owner's UHD 620 runs of the same stay his.

  - **Files.** `view/scenes/terrainScene.ts` and its fixture client; `displays/view/viewRun.ts`
    (`SCENE_OPTIONS`); `displays/view/check/`; `apps/hyperion/scripts/viewsCheck.sh`; `src/main/`
    for the option; `src/tools/placeShip.ts`; `crates/hyperion-surface/src/worker/golden.rs`.
  - **Tests:**
    - the scene's script, body and fixture: a first payload, then a delta at a revision rise,
      through the session to the workers;
    - `place-ship --survey`'s request, through the tool's fake server;
    - the console-ux skill's lint, contrast and glyph checks of the drafted strings;
    - decision-r10-t16-contracts §4–6's: the golden, its two names and its size; the server crate's
      agreement test; the session for `TERRAIN TEST` sends no frame over `FakeWebSocket`, its body
      IDs and seed are the golden's, and a loop of the script asks for no field again; the launch
      reader, the phase refusals, the faults verdict with no loss, one inside the window, one
      outside, two, and a GPU process's exit; two launches with no `--setting`;
    - the hidden run.
  - **Acceptance.** `cargo test -p hyperion-surface --features testing --lib worker::golden`;
    `pnpm --filter hyperion exec vitest run src/renderer/src/view/scenes src/renderer/src/displays/view
src/tools`; `just views-check --hidden --smoke --terrain` (exit 0); the console-ux checks; and the
    figures recorded.

- **R10.T16.e The live path's small fixes** (new, decision-r10-t16-leans §8 and §11). A guarded
  failure of the session's step or of a view's survey edges lasting `ANNUNCIATION_ONSET_MS` faults
  the session's terrain as lost workers do, with
  `TERRAIN NOT AVAILABLE: terrain computation failed, reopen VIEW to retry` on each handing view's
  fault line (the `console.error` stays, once). Every `--views-check` launch shows `MEASUREMENT` in
  place of `TRAINING`. The terrain run fails if the session's `terrainFaults` is ever non-empty. The
  guide's `TERRAIN NOT AVAILABLE`, `TRAINING` and `MEASUREMENT` rows as the ruling gives them.
  Tests: a step throwing for 250 ms raises the fault on each handing view and stops the terrain, a
  single throw does not; a views-check launch's header reads `MEASUREMENT` on every display; a
  terrain fault fails the run's verdict; the console-ux checks.
  Records: [10-terrain-on-generated-worlds/records/R10.T16.e.md](10-terrain-on-generated-worlds/records/R10.T16.e.md).

- **R10.T16.f The image's datum gives way over surveyed ground** (new, 2026-10-22, from main's
  graticule triage at `aa8e63e8`; a blocker, low in severity: a visibly wrong picture).
  Records: [10-terrain-on-generated-worlds/records/R10.T16.f.md](10-terrain-on-generated-worlds/records/R10.T16.f.md).
  - **The cause.** In the photorealistic style the symbology's overlay draws a body's own lines,
    its limb and graticule, at the datum's height without T11's give-way. `viewFrameDrawer.ts`
    built the overlay before the plan, `overlaySubmission` with no terrain, so
    `WireframeRenderer.frame` fell back to `NO_WIREFRAME_TERRAIN`: the lines took `lines`, not
    `linesSurveyed`, and the occluder sphere stayed whole. T16.c's terrain depth, drawn before the
    lines (`renderer.ts`), hides the line where the ground stands above the datum and not where it
    lies below it; `terrainDepth.wgsl`'s push-back (`occluderSlopePx` plus 2⁻¹⁶ of the depth) lets
    it show over ground a few hundred metres above the datum too. Seen in r10-maturity's look
    (R10.T1.c's finding 19): descent-150s and lowest-180s show a long line over the low plain,
    descent-100s dashes.
  - **The rules it breaks.** Design note 12's "no reference surface shows through a surveyed
    valley"; T11's give-way, built for the wireframe alone; the guide's graticule sentence speaks
    of the wireframe only.
  - **The fix.** The overlay is packed with each handed-over body's surveyed datum: the mask, the
    axes (`rotationColumns` of the rotation the plan drew the body with) and the stretch (its
    equatorial radius over its polar), so that its lines take `linesSurveyed` and its occluder
    `occluderSphereSurveyed`, as the wireframe's do. The overlay is built after the plan:
    `PhotorealFrame.overlay` becomes a builder of the datums (`PhotorealOverlay`), which
    `PhotorealRenderer.render` calls with `surveyedDatumsOf(plan, bodies)` once it has made the
    plan. `FrameTerrain` stays as T10.e and T16.c built it. Of the two shapes the triage named
    (this, or a `FrameTerrain.surveyedDatums()` beside `symbologyDepth()` with the renderer packing
    the overlay), this changes R07's interface least: one field's type, against a new method on
    `FrameTerrain` and the same deferred overlay. And it reads the mask the reference surface was
    discarded with: each handed-over `MeshBodyPlan`'s own `role.coverage` (its mask and rotation,
    read from `surveyCoverage` before the plan), so that the datum gives way exactly where the
    figure does, where a `surveyCoverage` read again after the frame terrain's draws might answer
    otherwise (main, 2026-10-22).
  - **The decision** (main's triage). With the give-way the graticule simply disappears over
    surveyed ground in the image, as the reference surface does. Drawing it on the terrain's height
    there is new scope and is not built (recorded in the Risks' deferred findings for T16.f).
  - **The side benefit.** The datum's occluder sphere no longer hides marks in surveyed valleys
    below the datum, such as a craft's outline on a crater floor.
  - **Files.** `view/photoreal/overlay.ts` (`overlaySubmission`'s `surveyed`, `surveyedDatumsOf`),
    `view/photoreal/renderer.ts` (`PhotorealOverlay`), `view/bodies/draw.ts` (`coverageOf`
    exported), `displays/view/photorealFrame.ts`, `displays/view/viewFrameDrawer.ts`,
    `smoke/terrainFrame.ts`.
  - **Tests:**
    - `overlay.test.ts`: a surveyed body's own lines, casing and stroke, are drawn with
      `linesSurveyed` and its occluder with `occluderSphereSurveyed`, each binding its mask and
      taking its stretch, every other line R02's and still cased; with no body surveyed no draw is
      surveyed; `surveyedDatumsOf` gives a body handed over with a mask the role's mask, its
      rotation's columns and its figure's a ÷ c, several bodies each their own, and none for a body
      promoted, handed over without a mask, or missing from the bodies;
    - `renderer.test.ts`: the renderer builds the symbology from its plan, a body handed over with
      its mask giving way in the plan's rotation and figure; no datum for a disc or a body handed
      over without a mask; no symbology built while its pipelines are being made; the earlier
      overlay tests through the builder;
    - `frameTerrain.test.ts`: through the live path, the datum gives way on the mask the frame
      terrain gave the plan, in the plan's rotation, even where the pass withholds the mask after
      the draws; none while the frame terrain withholds it;
    - `viewFrameDrawer.test.ts`: a photorealistic frame of `VIEW` handing a body over with its mask
      binds that mask to the symbology's `linesSurveyed` and `occluderSphereSurveyed` draws;
    - the smoke harness's `R10.T16 the frame` (part 7): over the image, a line of the body's own on
      the datum over the surveyed ground gives way, and one beyond the survey's edge stands; drawn
      again with no datum giving way (the overlay as built before), both stand.
  - **Acceptance.** `pnpm --filter hyperion test` (its files reach `view/photoreal`,
    `view/terrain`, `view/bodies` and `displays/view`); `just test-render` (part 7's new check);
    before and after captures, hidden and offscreen under `just _locked` and the GPU lock, of TEST
    MOON weathered at descent-100s, descent-150s and lowest-180s on high and low
    (`viewsCheck.sh --hidden --terrain --weathered`), the line gone over the surveyed plain.

- **R10.T16.g The datum's limb stroked inside the figure over the image** (new, 2026-10-27, from
  `decision-r08-approximate-plate.md`, item 2; a blocker, low in severity: a visibly wrong picture).
  Records: [10-terrain-on-generated-worlds/records/R10.T16.g.md](10-terrain-on-generated-worlds/records/R10.T16.g.md).
  - **The cause.** T16.f's overlay strokes each body's limb centred on it, as the wireframe does,
    so half its cased width lies over the image beyond the body. A cased line covers pixels up to
    δ = w ÷ 2 + c + 0.5 device px from its centreline (`lineStroke.wgsl`'s coverage, the casing
    drawn w + 2c wide): 3.5 px for the thin limb at a line scale of 2. On `TEST EARTH` at 51 Mm
    that covers the air's whole limb glow, about 2 px (plan 08's T10.b records, _The datum over
    the limb_).
  - **The rule** (main's ruling, 2026-10-27, under the owner's delegation). In the photorealistic
    style the datum's limb is stroked inside the figure, the outer edge of its casing on the limb,
    and no datum line or casing is drawn beyond the figure, with or without the body's air, so
    that the limb does not move when the air arrives. The wireframe centres its limb, as it does
    every line. Main's rulings on the brief's two calls (§7): a body whose apparent radius is under
    2(2 + √2) δ_limb (24 device px at a line scale of 2, 35 at 3) has no limb or graticule over
    the image, a cut with no hysteresis; the oblate figure's datum is R02.T19's, after this task.
  - **Files.** `view/wireframe/bodies.ts` (`DatumInside`, `DATUM_INSIDE_MARGIN_PX`,
    `DATUM_OVER_IMAGE_RADIUS_PER_REACH`, `datumOverImage`, `graticule`'s `inside`),
    `view/wireframe/drawList.ts` (`DrawOptions.datumInside`, `casingDevicePx`, `strokeReachPx`,
    `datumInsideAt`), `view/photoreal/overlay.ts`, `displays/view/viewFrameDrawer.ts`, the guide's
    bullet after "The datum's lines, …", and the smoke harness's new `smoke/datumFrame.ts`, in
    `smoke/page.ts`'s part 11 (part 8 had no room on the laptop that built it: Risks).
  - **Tests:**
    - `bodies.test.ts`: on `TEST EARTH` at 51 Mm, 1920 × 1080 at 60°, at the field's centre and
      near a corner, at line scales of 2 and 3: the inside limb's points and chords from δ_limb to
      δ_limb + 0.5 px inside the outline, each point on the sphere's near side; every meridian
      and parallel at least its own δ inside, its cut ends at its own δ, under the limb's casing,
      there and in a 120° field with the limb by its corners; the option off leaves the limb on
      the horizon, as built; no datum over the image just under the least radius, and the datum
      at it; the ruling's points, the datum at 24 and 35 device px of apparent radius and none at
      23 and 34, at line scales of 2 and 3, the band covering at most half the disc at the least
      radius;
    - `drawList.test.ts`: the option reaches both batches of every body, which keep their names
      and `body`, and nothing else; the list as built with it off; a body too small loses its
      batches and keeps its occluder sphere; the reaches at the overlay's casing;
    - `overlay.test.ts`: on `TEST EARTH` at 51 Mm, no segment of the body's batches in
      `overlayDrawList`'s list, widened by its `widthPx` ÷ 2 + `casingWidthPx` + 0.5, passes the
      outline, at line scales of 2 and 3; the limb's least margin within a twentieth of a pixel,
      so the inset's casing is the casing drawn; the centred datum, the control, passes it;
    - `viewFrameDrawer.test.ts`: a photorealistic frame's symbology packs the inside datum, and a
      wireframe frame's the centred one;
    - the smoke harness's `R10.T16.g the datum inside the figure` (part 11), on a sphere of R05's
      Earth with R05's Earth medium: every pixel whose centre's line of sight misses the sphere
      holds the same bytes with the inside datum and with none; pixels inside the outline differ;
      the air's glow stands beyond the limb; the centred datum changes pixels beyond it.
  - **Acceptance.**
    - `pnpm --filter hyperion exec vitest run view/wireframe view/photoreal displays/view/viewFrameDrawer`;
    - `just test-render --part=11`; `pnpm typecheck`, `pnpm lint` and `pnpm test`;
    - the `console-ux` checks on the changed renderer files and the guide;
    - by hand, hidden, `just views-check --hidden --atmosphere --setting high`: the glow at
      `TEST EARTH` at 51 Mm shows beyond the limb's casing.
  - _Built 2026-10-27_ (Risks, "Deviations in T16.g, as built").

### R10.T11 The wireframe's terrain

Records: [10-terrain-on-generated-worlds/records/R10.T11.md](10-terrain-on-generated-worlds/records/R10.T11.md).

Depth-only terrain in the wireframe style at τ = 4 px from the same bounds and bakes (heights and
mask only, the mask from the same `survey-mask` slot field as T10.c's), as one instanced draw, with
the contours of Design note 14 evaluated in the same pass,
the interval and index stated in the label block (`CONTOUR INTERVAL 200 m · INDEX 1 km`; the `DATUM`
line is T12's, in both styles) through R02's label API (as built `displays/view/viewRun.ts`'s
`labelStatements`, drawn by `ViewLabelBlock.tsx`), analytic line antialiasing and no MSAA, and the
patches under every grounded body in view at the finest level with morph held at zero, as in the lit
style (Performance budget, the second rule). Unsurveyed ground has no contours, and the survey edge
is drawn. Files: `terrainWire.ts`, `shaders/terrainWire.wgsl`. Tests (pure): `contourInterval` walks
the 1-2-5 sequence from the nadir's height above ground with ±20% hysteresis and never falls below 1
m; `contourPhase` with the patch-local height equals the `f64` contour levels to 1 mm at ±20 km;
each family is drawn in full at 6 px of spacing or more and fades to nothing at 3 px, in CSS pixels
times the stroke scale, minor before index; the index contour is 1.5 px; the wireframe's selection
is within a factor of two of `patchCountModel`'s ratio to the lit style, about a fifth; the
grounded-body rule holds in this style. Smoke run on SwiftShader. By hand, recorded: a station
wireframe at 1080p on the UHD 620, by the owner, during the low fast pass of the scripted descent
with the height workers busy, by `timestamp-query` with developer features, p50, p95 and p99 GPU
time per pass and the GPU clock from `intel_gpu_top`, with the patch and triangle counts, against
the brainstorm's 60 fps and its 4–9 ms for the whole view (Two deployments, one scene; the
research's provisional estimate is 3–6.5 ms with analytic antialiasing); and legibility on steep and
gentle terrain, including that the graticule is told from the contours, on a small body too. If it
is not, the graticule is left out over surveyed terrain, the finding is recorded, and the guide's
graticule sentence (the contours' bullet under "Views") is changed to match in the same commit
(signoff-9). Acceptance: `pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain`, the
smoke run, and the figures recorded.

_From R10.T16 (`decision-r10-live-path.md`, D10): built on T16's session. The wireframe takes a pass
of its own shading a body (`HeightSourceSetting.weights` `"skipped"`, its bakes skipping the class
weights and the horizon map as T6.b and T8.b built), through a lane of the session's pool, selected
at 4 px over the wireframe views that hand the body over; "the same bounds and bakes" reads "the
same bounds, and bakes of its own shading". Its draw joins each wireframe view's
`WireframeRenderer.render` through the session, as T16.c's `FrameTerrain` joins the photorealistic
frame's, and its contours' nadir height comes from the pass's resident bakes. Its legibility check
may take T16.d's `TERRAIN TEST`._

_From R10.T16 (decision-r10-t16-review §2): wherever a wireframe view takes the survey mask, the
same code-0 rule withholds it._

### R10.T12 Coverage and terrain readouts on the view

Records: [10-terrain-on-generated-worlds/records/R10.T12.md](10-terrain-on-generated-worlds/records/R10.T12.md).

`SurveyLine` in the view's label block: the survey's coverage of the body and its finest resolution
(`SURVEY COVERAGE 12.4 % · FINEST 2.0 m`, after the star chart's census line), or
`SURVEY COVERAGE: PENDING`, `SURVEY COVERAGE NONE` or `TERRAIN: NOT YET MODELLED`, each value stale
with its `S` when the link is lost. The resolution at the line of sight (the brainstorm's "states
the survey's resolution beneath its centre") stands in the readout as `RESOLUTION`. The `DATUM`
line, with its `SEA LEVEL` on a world with a sea, is stated once in both styles. `TerrainReadout`,
under `GROUND AT LINE OF SIGHT`: `ELEVATION`, `SLOPE` with its `· BASELINE 2 m` part and
`SLANT RANGE` (`SLANT RANGE FROM CAMERA` whenever the camera is off the hull) from
`surveyed_reading` and `slant_range`, run in a worker at the guide's 4 Hz, in `output` elements on
their `--surface-0` plates, with `~` and `± …` until resolved, rounded by Design note 6's rule,
`NOT SURVEYED` once in place of the readings over unsurveyed ground, `NO GROUND` where the line of
sight misses the body's ground, `GRAZING` where it applies, the material as its form and its
substance (`MATERIAL REGOLITH · Fe-RICH BASALT`; the seabed's substance the floor's; the em dash for
a substance not computed), and `RESOLUTION`. The material's properties: `ULTIMATE BEARING CAPACITY`
with its `· PAD WIDTH 1.0 m`, from `bearing_capacity` at the body's surface gravity, with the slope
argument the later T1 follow-up gives it before this task builds the reading; and `PAD FRICTION`,
from `interface_friction`, with its `· AT` temperature where the friction depends on it. That
temperature is the climate field's mean for the month the view's time falls in, carried to the
point's elevation by the header's lapse rate. Also the line-of-sight mark, the tripod, with its
give-way and its list note `GROUND AT LINE OF SIGHT`. Both are built to the guide as signed off in
`signoff-10.md` (this task's drafts, below). The DOM list notes the survey edge while it is in view,
as `SURVEY EDGE` with its nearest range, a note that cannot be selected. _Built by R10.T16.c
(decision-r10-t16-review §1): the `SURVEY EDGE` notes and their length formatter. T12 puts
`GROUND AT LINE OF SIGHT` first among the notes, gives `SLANT RANGE` the same formatter's units,
and its by-hand look at 1280 × 720 includes the longest note._ The task also renames the
descent spike's `SITE HEIGHT` to `SITE ELEVATION`, in `ELEVATION`'s form, and makes the guide's row
edit in the same commit (signoff-9). Build to the guide's rows as signed off on 2026-10-13
(`signoff-9.md`) and 2026-10-14 (`signoff-10.md`), and make that file's guide edit (h), the
capacity's slope clause, in the commit that builds the reading. As built, the label block is
`viewRun.ts`'s `labelStatements` and `ViewLabelBlock.tsx`, and the DOM list `ViewMarkList.tsx` over
`viewRun.ts`'s `MarkRow`; the material is named by its form and its entry's substance through
P14.T49.f's `substanceName` (`lib/substances.ts`; the roadmap's Conventions), never by a registry
key. Files:

- `readouts.ts`, `SurveyLine.tsx`, `TerrainReadout.tsx` and their tests;
- `displays/view/viewRun.ts` and `ViewMarkList.tsx`;
- `view/wireframe/symbology.ts` and `view/wireframe/drawList.ts`, where the tripod is built and
  drawn first of the marks, beneath the reticles, and their tests;
- the survey edge's ticks (T10.c's `surveyEdges` and T11's edge in the draw list), which yield to
  the tripod, built by whichever of T10.c, T11 and T12 lands last, with its test;
- `lib/format.ts` (`formatBearingCapacity`, beside `formatPressure`) and its test;
- R05's binding, where T6.b's `material` face does not already answer the two properties and the
  temperature from the body's gravity, the pad width, the slope and the month;
- `view/spike/DescentSpike.tsx` and its test.

Tests (Vitest and Testing Library): the formats of T13's drafts for each state, `~2140 m ± 180 m`
among them; the PDG rounding at its breakpoints (354, 355, 949, 950), and the round-up to one step
where it would go finer than the quantity's precision (0.12 m → `± 0.2 m`); no value is shown finer
than its uncertainty; the field keeps its fixed width, and the value's unit its place, from
Estimated to resolved and stale; `SITE ELEVATION` in `ELEVATION`'s form; the two properties' lines
resolved, Estimated, stale and Missing, with and without `· AT`; `formatBearingCapacity`'s two
figures and its switch at 1000 kPa with hysteresis; each field's width and its unit's place from
Estimated to resolved and stale; `PAD FRICTION — · AT 102 K` on glacier ice of H2O at 102 K and
`PAD FRICTION —` on snow; the tripod's geometry (three ticks at 120°, one from below,
0.1875–0.4375 rem, moved out by the outline's shift); its leaving within 0.5 rem of a body's symbol,
a craft's ticks or the flight path marker, and its return only beyond 0.75 rem; the survey edge's
ticks absent within 0.5 rem of its ticks; no tripod at `NO GROUND` or without a readout; its note
first in the list, unselectable, with its accessible name; stale on link loss; keyboard reach of the
readout's target through the DOM list; the console-ux skill's lint, contrast and glyph checks pass,
with no `σ` anywhere (B612 has none); at a finest-level bake of a synthetic world whose patch holds
at least three classes, at every vertex, `dominantSlot` of the baked weights (`terrainMaterial.ts`,
T10.b) names the palette slot whose (form, entry) is the worker's `material` answer at that vertex,
ties broken as `dominantSlot` breaks them (main's ruling for R10.T10.b, 2026-10-15); between
vertices the texel blends and no claim is made. By hand, recorded: in both styles at a device-pixel
ratio of 1, at the 80% and 100% interface scales, the tripod's open centre, the tripod over the
survey edge, and the tripod beside the flight path marker at 0.5 rem; and at 1280×720 the label
block's longest lines, `ULTIMATE BEARING CAPACITY ~0.95 MPa S · PAD WIDTH 1.0 m` and
`PAD FRICTION ~0.08 S · AT 243 K S`. If the centre closes at either scale, the ticks start 0.25 rem
out, keeping their length, and the guide's figures change in the same commit (signoff-10).
Acceptance:
`pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain` and the console-ux checks.

_From R10.T16 (`decision-r10-live-path.md`, D10): the readings (T6.b's `reading`, `slantRange` and
`material`) are asked of the session's pool, whose workers hold the body's field, through a query of
the pool's beside `classMap`, at 4 Hz for each view's line of sight (the primary's in T12;
decision-r10-t12-leans §3); `SurveyLine` reads the session's survey of the view's `FRAME` body
(the store's status, coverage and header, stale while the link is down), with the refused field's
state as signed off (decision-r10-t16-leans §12). Its by-hand checks may take T16.d's
`TERRAIN TEST`._

_Ruled (decision-r10-t12-leans):_

- `SLOPE` at `BASELINE 2 m` alone; its selector is R10.T12.a's (§1).
- The capacity stands beside `PAD FRICTION` on any slope below φ, its row stating that it takes the
  pad as holding (§2).
- The primary alone states the survey line, `DATUM`, readout, tripod and list note. The pool's
  readout query takes the view, and an instrument in its slot states none (§3, R10.T12.a).
- The stale forms (§4).
- No readout, `DATUM` or tripod under `TERRAIN NOT AVAILABLE`, the readout's queries guarded as
  T16.e's (§5).
- `AT`'s month from the seasonal orbit's mean anomaly at the time the view lights the body, through
  `lib/orbit.ts`'s phase (§6).
- The readout from the field header's first answer, `NONE` included (§7).
- The slope capacities' golden under `material/` at the version in force (§8).

The by-hand look at 1280 × 720 adds both slots at 16 rem, the primary's tripod between them, and
the primary's block's height with zero, one and two slots open (§3).

_Ruled (decision-r10-view-block-room §3, §4):_ the survey line, `DATUM` and the readout stand on
the label block's lower plate, which R10.T12.b builds, as its groups with their yield lines, and
T12 lands after R10.T12.b; its by-hand look is that ruling's §12, in place of the 1280 × 720 look
above.

- **R10.T12.a The readout's `BASELINE` selector, its line where no line of sight forms, and its
  labels kept whole** (new; decision-r10-t12-leans §1, decision-r10-t12a-scope). Three of R10.T12's
  follow-ups, client-only:
  Records: [10-terrain-on-generated-worlds/records/R10.T12.a.md](10-terrain-on-generated-worlds/records/R10.T12.a.md).
  - _The selector_ (`StyleControl.tsx`, `ViewDisplay.tsx`, `groundReadout.ts`, `viewLayout.ts`): a
    row on the `STYLE` panel after the style's buttons, `BASELINE` on the left of `2 m`, `5 m` and
    `100 m` (`SLOPE_BASELINES`), the chosen one pressed and underlined as the presets are; a
    display control acting at once on the `CONTROLS` view's readout, so standing while that view is
    the `PRIMARY` view, whatever its readout's state, and left out with an instrument as the
    `CONTROLS` view, which states none in its slot; `2 m` as the display mounts, kept as the view's
    style is, never stale; no key. The primary's readout is asked at the chosen baseline from the
    next ask, `SLOPE`'s `· BASELINE` stays the answer's, and `ULTIMATE BEARING CAPACITY` keeps its
    2 m (signoff-10's (h)). `FULL_MIN_HEIGHT_REM` takes the row: 49.5 rem, within its 52.5 rem
    bound.
  - _The line where no line of sight forms_ (`readouts.ts`, `groundReadout.ts`,
    `useTerrainLines.ts`): where the readout could stand but `lineOfSightOf` forms none, the
    readout's heading and, once in place of the readings, `NOT AVAILABLE: ROTATION not yet
modelled`, `NOT AVAILABLE: camera more than 6.68 AU from the body` (`MAX_SIGHT_M` in the view's
    range form) or `NOT AVAILABLE: camera at the body's centre` (`MIN_SIGHT_ORIGIN_M`), by its
    cause; `DATUM` with it, no tripod, no list note, no `S`, nothing asked; the plate's readout
    part, reckoned at the readout's tallest form.
  - _The labels kept whole_ (`TerrainReading.tsx`, `readouts.ts`, `styles.css`): on the lower plate
    a reading's label never shrinks; its value stands beside it where the label and the value's
    widest unbroken run share a line, breaking at its middle dots, and otherwise under it; a label
    breaks only where it alone is wider than the plate; `DATUM`'s figure breaks after its `×`. The
    upper plate and the instruments' blocks are unchanged.
  - _The guide_, in the commit: decision-r10-t12a-scope's U1 to U4.

  Tests (Vitest and Testing Library): the row's three buttons and its pressed state, its choice
  kept across `CONTROLS` changes, the row left out with an instrument as the `CONTROLS` view and
  standing while no view is drawn; the ask at the chosen baseline from the next publish, `SLOPE`'s
  `· BASELINE` the answer's and the capacity's line unchanged; the full layout's least height and
  its bound; the no-sight cause for each of `lineOfSightOf`'s `null`s and none wherever it forms a
  sight; each no-sight line under its heading with `DATUM`, no tripod, no note, no `S` on loss of
  the link, and none under the terrain fault or with no survey held; the readout part's tallest form
  in each no-sight state; the 6.68 AU figure from `MAX_SIGHT_M`; `DATUM`'s two runs; the readings'
  class and the stylesheet's rule; the console-ux checks. By arithmetic, in the record: the row's
  width, column B's tallest state, and each reading's width beside its label against the plates at
  1920 × 1080 at 100%, 125% and 150% and at 1280 × 720 at 100%, alone and beside the slots. By hand,
  hidden, at landing (main's, on the desktop): decision-r10-t12a-scope §9. Acceptance:
  `pnpm --filter hyperion exec vitest run src/renderer/src/displays/view
src/renderer/src/view/terrain`, the console-ux checks, `just smart-ci origin/rendering-and-planets`.
  Priority: medium; after R10.T12, before T15.

- **R10.T12.b `VIEW`'s room** (new, decision-r10-view-block-room). The `PRIMARY` view's label block
  as two plates and the order in which its parts give way, the instrument slots held where the
  stage cannot hold them, and the wireframe instrument's `CONTOUR INTERVAL`, before R10.T12's
  label-block work lands:
  Records: [10-terrain-on-generated-worlds/records/R10.T12.b.md](10-terrain-on-generated-worlds/records/R10.T12.b.md).
  - _The plates_ (`ViewLabelBlock.tsx`, `ViewDisplay.tsx`, `styles.css`): the upper plate as built,
    with `CONTOUR INTERVAL` after `FOV`; a lower plate at the stage's bottom left, inset 0.5 rem and
    standing on its foot, whose groups R10.T12 fills (the survey line, `DATUM`,
    `GROUND AT LINE OF SIGHT`), each with its yield line; both taking the width left of the standing
    slots, 0.5 rem apart; mark labels clear of both.
  - _The order_ (one pure function of the plates' laid-out heights, the stage and the standing
    slots, each lower group reckoned at its tallest): the readout, with its tripod and note; then
    the survey line's values and `DATUM`; then the upper plate's lines before `STARS` scroll, with
    their position and total (`formatListPosition`), keeping `FRAME`–`CAMERA` in view at their
    first position; then the slots are held. Each returns with a line to spare. Below the floor the
    scroll may go to one line, then the whole upper plate, held at its foot. The `STARS` line and
    those after it, the statements, the count line, the faults, the survey line's states and the
    yield lines never give way. The scroller is a focusable region only while it scrolls, and the
    plate takes the pointer only there.
  - _Held slots_ (`ViewDisplay.tsx`'s `roomForSlot`, `InstrumentView.tsx`, `InstrumentsPanel.tsx`):
    a slot the stage cannot hold whole (for the window, the interface scale, its statements or the
    primary's room) is held, drawing nothing and not rendered, its camera, style and target kept,
    and returns as it was with a line to spare; `INSTRUMENT 2` first, as its title row with `CLOSE`
    and `NOT AVAILABLE: no room at this window size` while `INSTRUMENT 1` stands; where no slot can
    stand, each is held whole and its `INSTRUMENTS` row says so, under the disclosure row while
    that panel is folded. This ends decision-r07-t19-layout §5's "its foot past the stage's" and the
    scale-change defect R10.T11 found (an instrument opened at 100% drawn at x = −78 at 150%, the
    primary's block 36 px wide).
  - _The instrument's interval_ (`InstrumentView.tsx`, `viewRun.ts`, `viewFrameDrawer.ts`): a
    wireframe instrument drawing contours states its `CONTOUR INTERVAL` after its `FOV`; views
    whose cameras stand at one point over one body state one interval, a view taking its held
    interval from a view already at that point (tolerance at most 1 m, recorded).
  - _The guide_, in the commit: decision-r10-view-block-room's G-A to G-E.

  Tests (Vitest): the order at each step and its return, on heights; the scroller's position line,
  its focus, and the lines it never takes; each yield line's text; a held slot's stub, its row and
  its standing line, and its return as it was; a slot held on a scale change from 100% to 150% and
  back; `INSTRUMENT 2` held before `INSTRUMENT 1`; the plates never wider than the width left of
  the standing slots; the instrument's interval line; one interval for two views at one point
  through a band's crossing, and two for views at different heights; the console-ux checks. By hand,
  hidden and never on `:0`: `TERRAIN TEST` at about 110 s and 192 s on both settings, the primary
  wireframe and photorealistic, in `CHASE` with `EASED CAMERA MOVES` on and in `SEAT`, with zero,
  one and two instruments, at 1920 × 1080 at 100%, 125% and 150% and at 1280 × 720 at 100%, 125%
  and 150%, with a scratch lower plate of R10.T12's tallest lines; the scale change with an
  instrument open; measured, every plate and slot within the stage's inset, the plates apart, no
  slot past the stage, and what gives way against the ruling's §3 table; nothing cut. Acceptance:
  `pnpm --filter hyperion exec vitest run src/renderer/src/displays/view` and
  `src/renderer/src/view/terrain`, the console-ux checks, `just ci`. Priority: high; r10-wireframe's
  next task after R10.T11; before R10.T12 lands, and so before T15.

#### The drafts, for the owner

_Drafted by lane 11 (2026-10-13); signed off 2026-10-14 under the owner's delegation
(`signoff-10.md`): both drafts amended, and the guide edited as that file directs._ They are the two
drafts the task's sentence on the material's properties and the mark owes. Each gives its wording
for the guide, where it goes, and the guide rows it extends, by section heading and line in
`docs/frontend/ux-guidelines.md` as it stands at `fe12a05e`, with R10.T13's rows in it (signoff-9).
Each lists its questions with this draft's lean, and ends in its own marker. The guide is not edited
here: once the owner signs a draft off, it is applied as the owner directs, and the task builds to
it. Every string proposed passes the console-ux skill's glyph and lint scripts ("Checks", below).
Over a Moon-like regolith plain under a 410 m survey, with the default pad, the readout would read:

```text
GROUND AT LINE OF SIGHT
ELEVATION                 ~2140 m ± 180 m
SLOPE                     ~12° ± 4° · BASELINE 2 m
SLANT RANGE               ~41.2 km ± 0.3 km
MATERIAL                  REGOLITH · UNLISTED MATERIAL
ULTIMATE BEARING CAPACITY ~3.4 MPa · PAD WIDTH 1.0 m
PAD FRICTION              ~0.20
RESOLUTION                410 m
```

The substance is `feldspathic`'s, which `substanceName` does not name yet (R10.T13's Draft 5); the
capacity is lunar regolith's at the Moon's 1.62 m/s², on the point's 12° slope over 2 m (3,432 kPa;
_corrected by R10.T12 from the draft's level `~5.4 MPa`, T1.a's slope follow-up, finding 10_), and the
friction NAVFAC's for silt (Draft 1).

- **Draft 1. The material's properties: `BEARING CAPACITY` and `PAD FRICTION`.**
  - _The quantities, as built_ (R10.T1.a; `crates/hyperion-surface/src/material/`, `bearing.rs` and
    `properties.rs`).
    - `bearing_capacity(properties, gravity, footing_width)` is the ultimate bearing capacity, Pa,
      under a square or circular footing of that width resting on level ground: Terzaghi's equation
      with Vesić's factors and shape factors, from the layer of the form's shear table at the depth
      of one footing width, at the body's gravity (Design note 2). Its test pins it on lunar
      regolith at 1.62 m/s² under a 1 m footing: 5,368.6 kPa from the 30–60 cm layer. The top
      15 cm's pair gives 206.5 kPa under the same footing, so a narrow pad, whose failure zone stays
      near the surface, bears far less: about 100 kPa under 0.1 m. _On a slope (T1.a's slope
      follow-up, decision-r10-slope-capacity):_ `bearing_capacity_on_slope` takes the slope too,
      for a pad resting flush on it, and gives `bearing_capacity`'s figure at 0; the readout takes
      it at the point's slope over 2 m (signoff-10 (h)).
    - `interface_friction(properties, temperature)` is the friction coefficient tan δ of a metal
      pad on the ground, with no unit. On a soil it is a constant, NAVFAC DM-7.02 (1986) Table 1's
      factor for steel: 0.20 on regolith, soil and dust, 0.25 on sediment and 0.30 on sand. On
      water ice (a glacier, sea ice) it follows the temperature between Liefferink et al. 2021's
      points, from 176 K to 272 K (−97 °C to −1 °C, read by eye from their Fig. 1, as
      `properties.rs` records), 0.039 at its least, near 265 K, and 0.71 at 176 K. Outside them it
      is `None`, as it is on the colder ground of most icy bodies.
    - Either is `None` where the form's row has no source for it (`form_row`). The capacity is
      `None` on every form but regolith, dust and soil of silicate, the three with a shear table.
      The friction is `None` on every form but those three, sand and sediment of silicate, and
      glacier and sea ice of water ice: so on snow, a frost, bedrock, volcanic rock, melt, an
      evaporite, a seabed, an organic sediment, a regolith of ice, and every form of a metal, a
      salt or an organic (`FormRow::UNMODELLED`).
  - _Wording,_ sub-bullets of the terrain readout's bullet in "Views" (lines 700–708), whose list
    of readings becomes `ELEVATION`, `SLOPE`, `SLANT RANGE`, `MATERIAL`, `BEARING CAPACITY`,
    `PAD FRICTION` and `RESOLUTION`: "After `MATERIAL` stand two of the ground's properties, from
    its material and the body: what it will bear before it fails in shear, and how a landing pad
    grips it.
    - `BEARING CAPACITY` is the pressure under which the ground fails beneath a pad: its ultimate
      bearing capacity, with no factor of safety, at the body's surface gravity, on level ground
      (_amended: on the ground's slope at the point over 2 m, for a pad that holds there,
      signoff-10 (h) and decision-r10-t12-leans G1; the guide holds the adopted text_).
      It is not a safe load: a pad sinks into the ground at far lower pressures, which are not
      modelled.
      It is on the console's pressure ladder, `Pa`, `kPa` and `MPa`, switching at 1000 with
      hysteresis, to two significant figures, and the pad it is taken under follows a middle dot:
      `BEARING CAPACITY 5.4 MPa · PAD WIDTH 1.0 m`. The pad is the own ship's; until the ship has
      pads it is 1.0 m.
    - `PAD FRICTION` is the coefficient of friction of a metal pad on the ground, a ratio with no
      unit, to two decimal places: `PAD FRICTION 0.20`. Where it depends on the ground's
      temperature, as an ice's does, the temperature it is taken at follows a middle dot, in whole
      kelvin: `PAD FRICTION 0.08 · AT 243 K`. That is the ground's temperature at the point for the
      month, from the survey's climate field; the day's cycle is not modelled.
    - Each is Estimated, its `~` without an uncertainty, since none is computed, until the survey
      resolves the point, as `ELEVATION` is: `BEARING CAPACITY ~5.4 MPa · PAD WIDTH 1.0 m`. On loss
      of the link each value the server's data gives goes stale with its `S`, the temperature among
      them, while the pad's width, a constant and not a live value, does not:
      `PAD FRICTION ~0.08 S · AT 243 K S`. Each value keeps a fixed field, sized for its longest
      Estimated reading with its `S`: `~990 kPa S`, `~0.71 S` and `272 K S`.
    - Where no source gives the figure for the ground's form, or the temperature lies outside its
      source's, the value is Missing, the em dash, since it is not computed: `BEARING CAPACITY —`,
      `PAD FRICTION —`. A part follows it only where the gap depends on it, as the friction of an
      ice colder than its source does on the temperature: `PAD FRICTION — · AT 102 K`.
    - Neither is ever `0` for a gap, and neither stands apart from the readings: `NOT SURVEYED` or
      `NO GROUND` stands once in place of them all."
  - _Rows_ (kind `Label`).
    - `BEARING CAPACITY`: in a view's terrain readout, the pressure under which the ground at the
      point fails beneath a pad of the stated `PAD WIDTH`: its ultimate bearing capacity, with no
      factor of safety, at the body's surface gravity, on level ground, since a slope's lowering of
      it is not modelled. Not a safe load: a pad's sinking, at far lower pressures, is not
      modelled either. Two significant figures in `Pa`, `kPa` and `MPa`, switching at 1000 with
      hysteresis (`BEARING CAPACITY 5.4 MPa · PAD WIDTH 1.0 m`); Estimated until the survey
      resolves the point; the em dash where no source gives it for the ground's form. Spelled out,
      and never `BEARING` alone, which the guide's prose keeps for a direction.
    - `PAD WIDTH`: after `BEARING CAPACITY` alone, the width of the pad it is taken under, a square
      pad's side or a round pad's diameter, to two significant figures: the own ship's pads', or
      1.0 m until the ship has pads. A constant, never stale.
    - `AT`: after `PAD FRICTION` alone, the ground's temperature the friction is taken at, the
      month's at the point from the survey's climate field, in whole kelvin (`· AT 243 K`).
    - `PAD FRICTION`: in a view's terrain readout, the coefficient of friction of a metal landing
      pad on the ground at the point (its sources measured steel): the ratio of the force that
      slides the pad to the load on it, with no unit, to two decimal places (`PAD FRICTION 0.20`).
      After a middle dot, `AT` and the ground's temperature, where the friction depends on it
      (`· AT 243 K`). Estimated until the survey resolves the point; the em dash where no source
      gives it for the ground's form or its temperature. Spelled out, never `μ` or `tan δ`, which
      neither face holds.
  - _The lines by state._
    - Resolved: `BEARING CAPACITY 5.4 MPa · PAD WIDTH 1.0 m`, `PAD FRICTION 0.20` and, on glacier
      ice of H2O at 243 K, `PAD FRICTION 0.08 · AT 243 K`.
    - Estimated: `BEARING CAPACITY ~5.4 MPa · PAD WIDTH 1.0 m`, `PAD FRICTION ~0.20`.
    - Stale: `BEARING CAPACITY ~5.4 MPa S · PAD WIDTH 1.0 m`, `PAD FRICTION ~0.08 S · AT 243 K S`.
    - Not computed: `BEARING CAPACITY —` on sand or ice; `PAD FRICTION —` on snow;
      `PAD FRICTION — · AT 102 K` on glacier ice of H2O at 102 K, as on a Europa.
    - Not surveyed, no ground: neither line, the readout's one status standing for every reading.
  - _The figures behind the examples_, from the formula, computed apart from the code in Python's
    `math` (2026-10-13), each checked again by the lane's science check: lunar regolith at
    1.62 m/s², 5,370 kPa under a 1.0 m pad (`5.4 MPa`) and 104 kPa under 0.1 m (`100 kPa`); Mars's
    drift at 3.71 m/s², of bulk density 1,200 kg/m³, 39 kPa under 1.0 m, and Viking Lander 2's
    crusty to cloddy soil, of 1,400 kg/m³, 301 kPa (`300 kPa`), both from the top 0.22 m, the depth
    Viking sampled, taken deeper, as `shear_layer_at` takes its deepest layer below it; glacier ice
    of H2O at 243 K, 0.080, between Liefferink et al.'s points at −38 °C and −30 °C.
  - _Sources for the wording._ "Ultimate" and "with no factor of safety" describe what `bearing.rs`
    computes: the pressure at which the ground fails in shear, nothing divided out. The Lunar
    Sourcebook keeps the two apart: the allowable bearing capacity, limited by settlement, "is
    significantly less than the ultimate bearing capacity" (Carrier, Olhoeft and Mendell 1991,
    §9.1.9, pp. 517–519, as the lane's science check read it). The 1.0 m pad is the footing the
    Sourcebook gives its lunar figure for, about 6,000 kPa (p. 517), and about the width of the
    Apollo 11 lunar module's footpads, nearly 1 m across, under which it reads 3,000–11,000 kPa
    (p. 518), both as `bearing.rs` gives them. The temperatures are the guide's kelvin (lines
    234–239): the source's −97 °C and −1 °C, read by eye to about 2 °C, are 176 K and 272 K. A
    ratio with no unit is shown bare, as `ECC` is (line 938; `BodyRecordReadings.tsx`).
  - _Goes in_ "Graphs, schematics and spatial displays", "Views": the terrain readout's bullet
    (lines 700–708) takes the two readings into its list and the wording as its sub-bullets; the
    rows go after the material form row (line 1050).
  - _Extends:_
    - "Views", lines 700–708 (the readout, its order, `NOT SURVEYED`, `NO GROUND` and staleness);
    - "Voice and nomenclature": lines 822–823 (one name per thing; an abbreviation only from the
      list), 858–877 (the material reading these follow), 878 (a reading of parts joined by `·`)
      and 879–880 (real engineering terms: geotechnics' ultimate bearing capacity and interface
      friction);
    - "Typography", lines 134–141 (no Greek letter; a quantity named in words, so never `μ` or
      `tan δ`);
    - "Numbers, units and time": lines 188–193 (a unit with every value, which a ratio has not, as
      `ECC`, line 938), 218–219 (one unit and precision for a quantity; the precision the operator
      can act on), 234–239 (kelvin, a source's degrees Celsius converted), 240–241 (scaled units
      with hysteresis) and 253 (a fixed field width with its status marks);
    - "Data states": the Stale, Missing and Estimated rows (lines 359, 360 and 362), lines 366–367
      (a lost link makes every live value stale) and 414–415 (a single value not computed, inside a
      section that is shown, is the em dash);
    - "Principles", item 5 (lines 26–28: never a number the simulation does not have, never a
      hidden estimate or gap);
    - the rows `MATERIAL` (line 1049), the material forms (line 1050), `SLOPE` and `BASELINE`
      (lines 1061–1062: a condition after a middle dot, the console's own setting never stale),
      `NOT SURVEYED` (line 1066) and `NO GROUND` (line 1067).
  - _Amends_ the `BASELINE` row (line 1062), which reads "`2 m` by default (the band limit and a
    pad's scale)": with `PAD WIDTH 1.0 m` in the same readout, "pad" would name two sizes. Its
    aside becomes "(the band limit, and the scale a lander's tilt is judged over)", after Design
    note 7's InSight ladder.
  - _Excepts_ nothing. The Estimated value's rounding (lines 220–233) needs a `±`, and these have
    none.
  - _Questions for the sign-off_, each with its lean.
    - _Estimated under a coarse survey._ The material is quoted at coverage alone (signoff-9),
      since it is a name; these are numbers, and Design note 6 gates every number by coverage and
      resolution. Lean: `~` until the point is resolved, then the plain value. Resolved, it is not
      an estimate of the simulation's quantity but the very figure landing reads (the brainstorm's
      properties "that landing and the consoles read"), though its table's figures are generic for
      the form. The alternatives are the material's state, never `~`, and `~` always, which would
      mark that genericness (the Estimated row's "derived", line 362).
    - _A figure not computed._ Lean: the em dash, as the guide's lines 414–415 have a single value
      the generator does not compute, and as signoff-9 set a substance not computed
      (`MATERIAL REGOLITH · —`). The alternative, `NOT YET MODELLED` in the value's place, is the
      guide's form for a section, and would stand on most icy ground.
    - _Precision._ Lean: two significant figures for the capacity, since its source holds it only
      to within a factor of about two (the Sourcebook's 3,000–11,000 kPa about its 6,000), on the
      console's one pressure ladder (`lib/format.ts`'s `formatPressure`), whose three figures are
      a surface pressure's; the friction to two decimal places, as NAVFAC states it.
    - _"Ultimate" in the label._ Lean: `BEARING CAPACITY`, the row saying it is the ultimate, with
      no factor of safety. The alternative is `ULTIMATE BEARING CAPACITY`, three words.
    - _The pad._ The own ship's hull is not defined (`TEST HULL`, line 1034), so it has no pads.
      Lean: a fixed 1.0 m, stated on the line, with no selector. The alternative is a `PAD WIDTH`
      selector, as `BASELINE` has one, which would meet the steps of Risks' deferred finding.
    - _The temperature._ Lean: the climate field's month at the point (R09's `ClimateCell`, taken
      to the point's elevation), stated only where the friction depends on it. Stated on every
      friction, it would suggest that a soil's constant varies with it.
    - _Slope._ The capacity is a level footing's on level ground; on a slope it is lower (Vesić's
      ground-inclination factors, (1 − tan β)² on the self-weight term for a slope β below φ, in
      EM 1110-1-1905's Table 4-6, p. 4-9, the table `bearing.rs` takes its shape factors from, as
      the lane's science check read it; about 0.62 at 12°), which T1.a's function does not take.
      Lean: say so in the row, and leave T1.a as built. _Ruled 2026-10-14 by main, on the issue:
      the lean. The row
      says so as drafted, and T1.a stays as built. Vesić's ground-inclination factors, with the
      footing-base tilt factors that go with them, are a deferred finding for a later T1
      follow-up, which adds the slope argument and its test before this task builds the readout
      (Risks)._
    - _The friction's kind._ NAVFAC's are unfactored friction factors at the onset of sliding, and
      Liefferink et al.'s a slow kinetic friction under a model skate's blade (`properties.rs`);
      neither was measured under a pad. Lean: one label, its row naming no kind and saying that
      its sources measured steel, since the two kinds cannot be told apart on one line without a
      word the console would have to add for this reading alone.
  - _Signed off (signoff-10): amended._

- **Draft 2. The line-of-sight mark.**
  - _The marks it must not resemble._ The guide names the ship-wide symbol set, ten closed
    outlines (lines 763–782), among them the feature's Greek cross (lines 784–790); the selection's
    bracket reticle, four corners of a square in `--accent`, and the destination's four chevrons
    in `--target`, each pointing at the mark (lines 557–567 and 613–616, rows 1069 and 1155); a
    craft's four open cardinal ticks outside its mark (`TICKS TARGET`, row 1068); the survey edge's
    ticked line (lines 715–718); a feature's circle at its true radius (lines 786–787); an apparent
    position's tick and dashed line (lines 527–528); the off-scale arrowhead (lines 358 and 511);
    the drawn unit glyphs, `☉` a circle with a dot, `⊕` a circle with a cross, and `⌘` (lines
    160–167); and on a spatial display the axis triad's three arrows and the range rings (lines
    568–572). The client draws three more the guide does not name: the view's flight path marker,
    a circle 0.375 rem in radius with two level wings and an upright fin in `--text`
    (`view/wireframe/symbology.ts`, R02.T12.c), and on the galaxy map the cursor, an upright cross
    with a gap at its point in `--accent`, and the chart centre, a small diagonal cross in `--text`
    (`displays/galaxy/GalaxyMapView.tsx`). The reticles, a craft's ticks and the crosses have
    four-fold symmetry, every symbol is closed, and the crosses and the flight path marker are what
    a centre mark would most likely copy.
  - _Where the point falls._ The view's projection is centred on its line of sight
    (`view/camera/projection.ts`), so the point is always the picture's centre, where
    `PREVIOUS TARGET` and `NEXT TARGET` also set an aimed target (line 1089). So the mark shares its
    place with an aimed target's reticles and, whenever the view looks along the ship's velocity,
    as on a straight-in descent, with the flight path marker, which stands where the velocity
    projects.
  - _Wording,_ a bullet of "Views" after the terrain readout's (line 708): "Where the view's line of
    sight meets the ground its terrain readout reads, the view marks the point, in both styles,
    with a tripod: three short straight ticks pointing at the point, one from directly below it and
    two from above, each 60° from the upward vertical, the point itself left open. No other mark
    is three open ticks about an open point: the reticles, a craft's ticks and the crosses are
    four-fold; the triangles and the hexagon, which share its symmetry, are closed outlines; and a
    spatial display's axis triad meets at its origin, with arrowheads and labels. So the tripod is
    never read as a reticle, a craft or a body.
    - _Size and stroke._ Its ticks reach 0.375 rem from the point, the radius of an unresolved
      contact's symbol, so that it stands inside the selection's bracket about any body as a
      symbol does, and start 0.125 rem from it, each 0.25 rem long, as a craft's tick is. They are
      drawn at the `1.5px` outline of a mark sized in `rem`, moved out as a symbol's line is so
      that the centre stays open (see "Layout"), with their `--surface-0` casing in both styles.
    - _Colour._ `--text`, as the flight path marker and a craft's ticks are: it marks a reading,
      and is neither available nor selected (`--accent`), nor commanded (`--target`), nor an alert.
    - _Place._ It stands where the line of sight falls, at the picture's centre, and is never
      moved, since a moved mark would mark ground the readout does not read; so it never nears the
      view's edge. Where the view's chrome covers the centre, a label block's plate or an
      instrument's slot over the `PRIMARY` view, it is covered with the picture. No mark's label is
      placed over it: a label that would cover it stands on another side of its mark, or is not
      shown, as one the chrome would cover.
    - _Beside other marks._ It is drawn over the ground's own lines, the contours, the graticule
      and the survey edge, whose line its casing cuts, so that its ticks stay whole and are never
      read as the edge's, which hang from the line on one side alone. It is drawn beneath the
      reticles: the selection's bracket and the destination's chevrons about an aimed body stand
      round it. Where a body's symbol, a craft's ticks or the flight path marker come within
      0.25 rem of its ticks, it is left out, since that mark then stands where the line of sight
      is, and the two together would read as one new symbol, a circle or a triangle with spokes.
    - _When it is drawn._ It is drawn with the readout, over surveyed ground and over ground the
      ship has not surveyed alike, where the readout reads `NOT SURVEYED` of the point it marks. It
      is unchanged under `GRAZING`: its place is exact, and what is uncertain is how far along the
      line of sight the ground lies, which the readout states. At `NO GROUND` it is not drawn, there
      being no point, nor where the view states no terrain readout. It comes and goes with the
      readout's own updates, as a cut, so that the mark and the readout never disagree.
    - _While stale._ Unchanged: it marks the picture's centre, the point the readout reads from the
      same pose, whatever that pose's age, and staleness is carried by the readings' `S` and
      `POINTING`'s, never by a mark's colour.
    - _In the list._ While the readout has a point, drawn or left out for another mark, the
      view's list notes it as `GROUND AT LINE OF SIGHT`, the first of its notes, before
      `SURVEY EDGE` and `INTEGRATED STARLIGHT`. Its accessible name gives its place, "Ground at
      line of sight, at the view's centre". It is a note outside the list's options: not a
      contact, it cannot be selected, and it carries no range, since the readout's `SLANT RANGE` is
      the point's."
  - _Rows._
    - `TRIPOD GROUND AT LINE OF SIGHT`, kind `Legend`, after `CHEVRONS DESTINATION` (line 1069):
      "A view's mark where its line of sight meets the ground its terrain readout reads: three open
      ticks pointing at the point, one from below and two from above, 120° apart. Four ticks mean
      another craft (`TICKS TARGET`), and corner brackets the selection (`BRACKET SELECTED`)."
    - `GROUND AT LINE OF SIGHT` (line 1059) gains: "Also the view's list note of the point's mark
      (`TRIPOD GROUND AT LINE OF SIGHT`), which cannot be selected."
  - _Contrast._ `contrast.py text surface-0` gives 13.57:1, above the 6:1 a stroke that carries
    meaning needs, and the view's black ground is darker than `--surface-0`. At the floor of 2
    device px a view's stroke wholly covers its brightest pixel (lines 75–76), so the pair holds
    as scored.
  - _Goes in_ "Graphs, schematics and spatial displays", "Views", after the terrain readout's
    bullet (line 708) and before the survey edge's (line 709); the rows as above.
  - _Extends:_
    - "Colour": the token table (lines 45–59), lines 62–65 (6:1), 66–82 (a stroke as drawn,
      against its casing over an image), 83 (never by colour alone: the shape and the list's note
      carry it) and 106–110 (every mark over the image cased);
    - "Layout", lines 301–309 (widths times the larger of the device-pixel ratio and 2; a mark
      sized in `rem` at no less than 2 device px; an open mark keeps its hole);
    - "Graphs, schematics and spatial displays": lines 520–523 (a small fixed symbol set, shape for
      type), 557–567 (the bracket and the chevrons, told apart by shape) and 576–577 (the canvas
      paired with a DOM list);
    - "Views": lines 608–616 (what a view keeps of the symbol set and reticles), 625–626 (its DOM
      list), 660–674 (both styles; casings; a label never over its mark's reticle, never shown in
      part; changes as cuts), 676–680 and the `INTEGRATED STARLIGHT` row (line 1079: a list note
      that is not a contact), 700–708 (the readout's point, moved by the camera, never by a
      pointer) and 709–725 (the survey edge's mark and its note);
    - "Motion and sound", lines 800–802 (readouts at about 4 Hz) and 805–812 (a view never stops;
      cuts);
    - the rows `GROUND AT LINE OF SIGHT` (line 1059), `SURVEY EDGE` (line 1055), `TICKS TARGET`
      (line 1068), `CHEVRONS DESTINATION` (line 1069), `PREVIOUS TARGET` and `NEXT TARGET` (line
      1089), and `BRACKET SELECTED` (line 1155).
  - _Questions for the sign-off_, each with its lean.
    - _The tripod's way up._ Lean: one tick below and two above, with no strong reason between the
      two: either way up its ticks point along one triangle's vertex directions (the planet's
      here, the neutron star's inverted), both closed outlines, and where the flight path marker or
      a contact comes close, it gives way either way up. Inverted is the alternative.
    - _Over unsurveyed ground._ Lean: drawn, since `NOT SURVEYED` is the readout's reading of that
      point, and the operator steers by the mark onto surveyed ground. Left out, the mark would
      come to mean "surveyed".
    - _A contact on the point._ Lean: left out within 0.25 rem of it, its note standing. Drawn
      beneath, a craft's lower tick would continue the tripod's, and a body's outline would close
      round it as a new symbol.
    - _The flight path marker on the point._ Lean: left out within 0.25 rem of it, as for a
      contact, since the tripod's ticks, reaching 0.375 rem, would end on the inside of the
      marker's circle, 0.375 rem in radius, and the two would read as a circle with three spokes;
      the note stands, and on a straight-in approach the marker itself then shows where the line of
      sight meets the ground. The alternatives are to draw the tripod beneath it, or to set its
      ticks outside the marker's circle, which would make it the largest point mark on the view.
    - _The survey edge through the point._ The tripod's ticks are the edge's length and colour
      (0.25 rem, `--text`). Lean: the tripod over the edge, its casing cutting the edge's line, as
      drafted. The alternative is ticks of another length, so that the two differ by length too.
    - _The note's range._ Lean: none, since the readout's `SLANT RANGE` is the point's, from the
      camera, and a second range, from the own ship as the list's are, would give two figures for
      one point. The alternative is `SURVEY EDGE`'s, measured as the list's ranges are.
    - _Labels clear of it._ Lean: as drafted, which extends the label rule (lines 669–674) from a
      mark's own reticle to this mark.
    - _Its name._ Lean: `TRIPOD`, a shape's word as `TICKS`, `CHEVRONS` and `BRACKET` are, joined
      to the readout's heading so that the point keeps one name.
  - _Signed off (signoff-10): amended._

- **Followed from the sign-off** (`signoff-10.md`):
  - this task's files, tests and by-hand checks, now in its text above;
  - the `ULTIMATE BEARING CAPACITY` row's level-ground clause, replaced by signoff-10's (h) in the
    commit that builds the reading on the T1 follow-up's slope argument;
  - the roadmap README's "Awaiting the owner" (signoff-10, (p6));
  - and a guide row for the flight path marker, which the client draws and the guide does not name
    (Risks, "Deferred findings (2026-10-13 rule) (R10.T12's drafts)").

- **Checks** (2026-10-13, this worktree).
  - `glyphs.py "· — ~ ± °" '\u00a0'`: every character outside ASCII the drafts put on screen,
    with the tilde and the no-break space of the `±` row (exit 0):

    ```text
    char  code     B612   Mono   name
    ·     U+00B7   yes    yes    MIDDLE DOT
    —     U+2014   yes    yes    EM DASH
    ~     U+007E   yes    yes    TILDE
    ±     U+00B1   yes    yes    PLUS-MINUS SIGN
    °     U+00B0   yes    yes    DEGREE SIGN
          U+00A0   yes    yes    NO-BREAK SPACE
    ```

  - `glyphs.py "μ δ"`: the friction's symbols, which the drafts name in words instead (exit 1):

    ```text
    char  code     B612   Mono   name
    μ     U+03BC   no     no     GREEK SMALL LETTER MU
    δ     U+03B4   no     no     GREEK SMALL LETTER DELTA
    ```

  - `ux_lint.py` on a scratch `.ts` file, outside the worktree and not committed, holding every
    string the drafts propose as a string literal (exit 0), after a control string in a second
    file there (`"Please wait, loading the PAD FRICTION"`) drew its `[voice]` error, so the file
    was read:

    ```text
    0 error(s), 0 check(s) in 1 file(s) scanned. Heuristic: confirm each line against the guide.
    ```

  - `contrast.py text surface-0` (exit 0):

    ```text
    text on surface-0: 13.57:1 (minimum 6:1) PASS
    ```

  - _Reviews._ The `ux-reviewer` agent found nothing must-fix, and five should-fix points and six
    nits, all applied: the three-fold claim, the flight path marker's give-way, the staleness
    reason, the `~` question's third alternative, `BASELINE`'s pad, the row's ladder, the pad's
    staleness, `AT`'s row, the field widths, the survey edge, and two citations. The
    `science-checker` agent found nothing must-fix, and six should-fix points and five nits, all
    applied: "what it will bear", the slope factors' table, the forms without figures, Viking's
    depth, the friction's kinds, the flight path marker's place, and the figures' rounding. It
    could not reach NAVFAC DM-7.02 or Vesić's primaries, which stand as `properties.rs` and
    `bearing.rs` cite them.

### R10.T13 Guide drafts for the owner

Drafts, in one pass, of what this plan adds to R02's nine items: `±` as one standard deviation of an
Estimated value, written `~2140 m ± 180 m` with the unit on both numbers and a space either side of
`±` (NIST SP 811 §7.7, verified; `±` is in B612 and B612 Mono, `σ`, `≈` and `Δ` are not, checked
with the console-ux skill's glyph script), as a nomenclature entry; an exception to "the same
quantity uses the same unit and precision everywhere" for an Estimated value rounded to its
uncertainty, its field keeping its fixed width; the readout names `ELEVATION` (spelled out, since
`ELV` means a spatial display camera's elevation, an angle, and the brainstorm's `ELEV` would be a
second abbreviation of the word), `SLOPE` with its `· 2 m BASELINE` part, `SLANT RANGE` (`RANGE` is
taken by the chart's drive range), `DATUM`, `GRAZING` and `NOT SURVEYED`, after ICAO's distinction
of elevation, altitude and height; the survey line's wording; the material reading's wording (its
form and its substance, named by P14.T49.f's `substanceName`); the survey edge as a mark in both
styles; the contour widths and tokens, `CONTOUR INTERVAL`, `INDEX`, and the statement that minor
contours are omitted where closer than 4 px, as guide item 5's substitute for a scale bar. The
client is built to the drafts, as the galaxy plans' slice was. Files: a draft section in this task's
entry, not the guide. Acceptance: the drafts are written and **the owner signs off**, after which
the guide edit is made as the owner directs.

#### The drafts, for the owner

_Drafted by lane 5 (2026-10-13); signed off 2026-10-13 under the owner's delegation
(`signoff-9.md`): draft 6 adopted, drafts 1–5 and 7 amended, and the guide edited as that file
directs._ Each draft gives its wording for the guide, where it goes, and the guide rows it extends
or excepts, by section heading and line in `docs/frontend/ux-guidelines.md` as it stands at
`be8f8a6f`. Each ends in its own marker. The guide is not edited here: once the owner signs a draft
off, it is applied as the owner directs, and T11 and T12 are built to it. The drafts extend R02's
item 1 (Views), item 3 (outlines for symbology), item 5 (scale without a scale bar) and item 7
(limited detail). They also fill item 2's honesty rule, whose "survey coverage" Draft 4 supplies.
Every string proposed here passes the console-ux skill's glyph and lint scripts ("Checks", below).
Over a surveyed plain, a label block would show them as below, the substance as `substanceName`
names it today (Draft 5), and the contour line in the wireframe style alone:

```text
SURVEY COVERAGE  12.4 % · FINEST 2.0 m
DATUM            REFERENCE SPHERE 6371.0 km · SEA LEVEL -2.4 m
CONTOUR INTERVAL 200 m · INDEX 1 km
GROUND AT LINE OF SIGHT
ELEVATION        ~2140 m ± 180 m
SLOPE            ~12° ± 4° · BASELINE 2 m
SLANT RANGE      ~41.2 km ± 0.3 km
MATERIAL         SOIL · UNLISTED MATERIAL
RESOLUTION       410 m
```

- **Draft 1. `±`, one standard deviation of an Estimated value** (a nomenclature row).
  - _Row._ `±`, kind `Notation`: "After an Estimated value, its uncertainty: one standard
    deviation of it, the unit written on both numbers and a space either side of the sign,
    `~2140 m ± 180 m`. The figure after `±` is a standard deviation and not a confidence interval;
    it is never to be read as a 68 % interval, a tolerance or a limit, and never as LE90 or a 95 %
    figure. It stands only with the `~`, and both go when the value is resolved. The spaces in the
    reading are no-break, U+00A0, so that it never breaks across a line. Both numbers are rounded
    as the Estimated value's exception gives. `σ` is never shown, nor `≈` or `Δ`, since neither
    B612 face has them; `±`, from Latin-1, is in both."
  - _Sources._ The form is NIST SP 811 (2008) §7.7, "Clarity in writing values of quantities":
    "63.2 m ± 0.1 m or (63.2 ± 0.1) m but not: 63.2 ± 0.1 m or 63.2 m ± 0.1". SP 811 does not say
    what the figure after `±` means. The meaning, and the caveat the row states, are the GUM's
    (JCGM 100:2008 §7.2.2, item 4): "where the number following the symbol ± is the numerical value
    of (the combined standard uncertainty) uc and not a confidence interval". Its NOTE adds that
    "The ± format should be avoided whenever possible", since it "may be confused with expanded
    uncertainty". The console keeps `±` because B612 has no `σ`. The GUM's concise form,
    `2140(18) m`, would set parentheses, which the guide keeps for a symbol's subscript and a
    formula's group (lines 141, 213–216, 752–754). The nomenclature list defines the figure once,
    as §7.2.2 allows ("if uc is defined elsewhere in the document"). The figure is a 1σ RMS, not
    LE90, by Design note 6; and since the unresolved heights are not Gaussian, a 68 % reading would
    be wrong too. The uncertainty is a quantity of its own, not the far end of a span (line
    227–229), so it carries its own unit.
  - _Glyphs._ `glyphs.py "± σ ≈ Δ"`, run 2026-10-13 in this worktree (exit 1, for the three
    missing):

    ```text
    char  code     B612   Mono   name
    ±     U+00B1   yes    yes    PLUS-MINUS SIGN
    σ     U+03C3   no     no     GREEK SMALL LETTER SIGMA
    ≈     U+2248   no     no     ALMOST EQUAL TO
    Δ     U+0394   no     no     GREEK CAPITAL LETTER DELTA
    ```

  - _Goes in_ the nomenclature list (from line 776), before the readout names of Draft 3.
  - _Extends_ "Data states", the Estimated row (line 347, "Prefixed with `~`"), which gains its
    uncertainty; "Numbers, units and time", line 187–188 (a unit on every value, with a space) and
    line 227–229 (a span takes one unit; an uncertainty is no span); "Typography", lines 127–137
    (`±` among Latin-1's signs; Greek letters, `≈` and `∆` among what neither face holds).
  - _Signed off (signoff-9): amended._

- **Draft 2. The Estimated value rounded to its uncertainty** (an exception).
  - _Wording,_ after "The same quantity uses the same unit and precision everywhere on the ship":
    "The one exception is an Estimated value stated with its uncertainty (`±`). It is rounded to
    the uncertainty, not to the quantity's precision, so that it shows no digit the measurement
    does not hold. The uncertainty's three leading digits decide: 100 to 354 keep two significant
    figures, 355 to 949 keep one, and 950 to 999 round up to 1000 and keep two. The value is
    rounded to the same decimal place: `~2140 m ± 180 m`, `~2100 m ± 500 m`, `~2100 m ± 1000 m`.
    Neither number is shown finer than the quantity's own precision. An uncertainty that would
    round below one step of it is shown as one step: `~2143.7 m ± 0.1 m`, at an elevation's 0.1 m.
    The field keeps its fixed width, sized for the longest Estimated form with its `~`, sign,
    digits, `±` and uncertainty. The reading stands right-aligned in it, so that the value does not
    move as the survey resolves it. Once resolved, the value takes the quantity's own precision,
    and the `~` and the `± …` go."
  - _Source._ The rule is the Particle Data Group's (Review of Particle Physics 2024, Introduction,
    §5.3 "Rounding"; Design note 6): "if the three highest order digits of the error lie between
    100 and 354, we round to two significant digits. If they lie between 355 and 949, we round to
    one significant digit. Finally, if they lie between 950 and 999, we round up to 1000 and keep
    two significant digits. In all cases, the central value is given with a precision that matches
    that of the error." The floor of one step is this draft's own.
  - _Goes in_ "Numbers, units and time", as the bullet after line 217–218.
  - _Excepts_ "Numbers, units and time", line 217–218 ("The same quantity uses the same unit and
    precision everywhere on the ship"). _Keeps_ line 238 ("A field has a fixed width sized for its
    longest possible value and its status marks") and "Typography", line 183 (numbers
    right-aligned).
  - _Signed off (signoff-9): amended._

- **Draft 3. The readout names.**
  - _The vertical distances._ The names follow ICAO's distinction of three vertical distances. It
    is ICAO Doc 4444, PANS-ATM, 16th edition (2016), Chapter 1, "Definitions", which carries all
    three; Annex 2, 10th edition (2005), Chapter 1, defines altitude and height but not elevation.
    The definitions are as follows.
    - An altitude is "The vertical distance of a level, a point or an object considered as a
      point, measured from mean sea level (MSL)."
    - An elevation is "The vertical distance of a point or a level, on or affixed to the surface of
      the earth, measured from mean sea level."
    - A height is "The vertical distance of a level, a point or an object considered as a point,
      measured from a specified datum."

    A body has a stated `DATUM` and often no sea. So the ground's distance above the datum is its
    `ELEVATION`: ICAO's elevation, with the datum in place of mean sea level. Planetary cartography
    measures the same way. The IAU's working group uses a reference surface "for measuring
    geometric height (elevation)" (Archinal et al. 2018, Celest. Mech. Dyn. Astron. 130:22, §7).
    LRO LOLA's gridded topography reads "HEIGHT, i.e. elevation in meters" above a reference sphere
    (`LDEM_4.LBL`). A craft's or camera's distance above the ground, or above the datum, is a
    height (`HEIGHT ABOVE SITE`, line 976). No reading here is an altitude, which ICAO keeps for
    mean sea level.

  - _The point._ A view's terrain readout reads the ground where the view's line of sight first
    meets it. It reads no pointer cursor, since nothing may depend on hover. The camera's own keys
    and drag move the point, and `PREVIOUS TARGET` and `NEXT TARGET` aim it. On the label block,
    the readings stand under the heading `GROUND AT LINE OF SIGHT`, in the order `ELEVATION`,
    `SLOPE`, `SLANT RANGE`, `MATERIAL` (Draft 5) and `RESOLUTION` (Draft 4), at the guide's 4 Hz.
  - _Rows_ (kind `Label`, but `GRAZING`, `NOT SURVEYED` and `NO GROUND`, which are kind `Status`):
    - `GROUND AT LINE OF SIGHT`: the heading of a view's terrain readout. It reads the ground where
      the view's line of sight first meets it. Stated while the view's `FRAME` is a body's and the
      body has a surface.
    - `ELEVATION`: the ground's height above the body's `DATUM`, along its normal, at that point.
      It is in metres at every value, to 0.1 m, with `-` below the datum and no `+` above it, as a
      map's elevations are (`2143.7 m`, `-410.0 m`). It is Estimated, with its uncertainty, until a
      close survey resolves the point (`~2140 m ± 180 m`). Spelled out: `ELV` is a spatial display
      camera's elevation, an angle, and `ELEV` would be a second abbreviation of one word. Never
      `ALTITUDE` or `HEIGHT`, which ICAO gives to other distances; `HEIGHT` is also the `GALACTIC`
      frame's +z reading.
    - `SLOPE`: the ground's slope at that point over a stated baseline. It is the adirectional
      slope that tilts a lander: the gradient's magnitude from central differences along local
      east and north (Design note 7). In whole degrees, with its baseline after a middle dot:
      `SLOPE 7° · 2 m BASELINE`. While the point is not resolved, it is the RMS slope, Estimated,
      with its uncertainty: `SLOPE ~12° ± 4° · 2 m BASELINE`.
    - `BASELINE`: the horizontal distance a `SLOPE` is taken over: `2 m` by default (the band limit
      and a pad's scale), `5 m` or `100 m` (Design note 7). Also the readout's selector of it, a
      display control.
    - `SLANT RANGE`: the distance from the camera, along its line of sight, to the ground it first
      meets. It is in `m` below 1 km, then in `km` and `Mm` as a view's ranges are, to three
      figures, with hysteresis (`SLANT RANGE 412 m`). It is Estimated, with its uncertainty, while
      the survey has not resolved the ground there (`~41.2 km ± 0.3 km`), or carries `GRAZING` in
      the uncertainty's place. It is measured from the camera, so whenever the camera is off the
      hull it reads `SLANT RANGE FROM CAMERA`. Never `RANGE`: the chart's curve labels take it for
      the drive range, and a view's list heads its own column with it. Nor `DIST`.
    - `GRAZING`: after a `SLANT RANGE` and a middle dot, where the line of sight meets the ground so
      obliquely that relief finer than the survey could stand in front of the point. That is where
      the grazing angle's sine is under twice the unresolved slope's standard deviation over the
      survey's cell (Design note 7). The range is Estimated and stands without an uncertainty,
      since the true range may be shorter by more than one: `SLANT RANGE ~41.2 km · GRAZING`.
      Never on a resolved range.
    - `DATUM`: the surface every terrain height on a view is measured from, along its normal. It is
      the body's rotational spheroid, with its equatorial and polar radii to 0.1 km and the unit on
      both (`DATUM REFERENCE SPHEROID 6378.1 km × 6356.8 km`). Until the body's flattening is sent
      it is a sphere, with its radius (`DATUM REFERENCE SPHERE 6371.0 km`). It is stated once, on
      the label block, in either style, while the readout stands. On a world with a sea it is
      followed, after a middle dot, by its `SEA LEVEL`, a height above the datum in `ELEVATION`'s
      form (`· SEA LEVEL -2.4 m`).
    - `NOT SURVEYED`: where the ground at the point has not been surveyed, `NOT SURVEYED` stands
      once in place of the readout's readings, under its heading. It is steady, in `--text`, as
      `NOT RESOLVED` stands in place of a section the granted detail withholds. Never "unknown" or
      "no data". The view draws no terrain and no contours there, and the body's reference surface
      stands in its place (Draft 6).
    - `NO GROUND`: where the line of sight meets no ground within the readout's reach, `NO GROUND`
      stands once in place of the readings, as `NO LIGHT` does for a lightless object.
  - _Amends_ "Data states", "Limited detail is an annunciation" (lines 375–377). Its "since the
    view's readouts come from the server" becomes "since the view's readouts come from the server's
    data and the height function, not from pixels", as the brainstorm's item 7 has it: these
    readouts are computed in the client's worker from the server's field.
  - _Goes in_ the nomenclature list, after `FROM CAMERA` (line 935).
  - _Extends:_
    - the `ELV` row (line 784), which these leave as it is;
    - `DRIVE RANGE` (line 796) and `FROM CAMERA` (line 935, "where there is no own ship to measure
      from"). The second gains a slant range, always measured from the camera;
    - `NO BODIES`, `NO SYSTEMS IN MAP`, `NO LIGHT` (line 1012), and `GALACTIC` (line 1017,
      whose `HEIGHT` these do not reuse);
    - "Data states": the Missing row (line 345), the Estimated row (line 347), the section states
      (lines 379–399) and staleness (lines 351–352);
    - "Numbers, units and time": lines 225–226 (units scaled with hysteresis), 230–236 (`-` always,
      `+` only where direction matters), 237 (digits grouped from five) and 239 (angles in degrees);
    - "Controls and commanding": lines 477–480 (the keyboard; nothing on hover alone);
    - "Motion and sound": lines 707–709 (readouts at about 4 Hz);
    - "Voice and nomenclature": line 729–730 (an abbreviation only from the list) and line 765 (a
      reading of parts joined by `·`).
  - _Leaves_ the descent spike's `SITE HEIGHT` (line 977) as R05 has it, although by this
    distinction it is an elevation (a question below).
  - _Signed off (signoff-9): amended._

- **Draft 4. The survey line.**
  - _Wording,_ a bullet of "Views": "While a view's `FRAME` is a body's and the body has a
    surface, its label block carries the ship's survey of that body, as the star chart carries its
    census line: `SURVEY COVERAGE 12.4 % · FINEST 2.0 m`.
    - `COVERAGE` is the share of the body's surface surveyed. It is in per cent to three
      significant figures, in E notation below 0.001 % (`2.00E-5 %`). From 99.95 % until the whole
      body is surveyed it is `ABOVE 99.9 %`, and only then `100 %`. Where no cell is surveyed it is
      `NONE`, with no `FINEST` part, since none is a value: `SURVEY COVERAGE NONE`.
    - `FINEST` is the finest resolution surveyed anywhere on the body. It is a length to two
      significant figures, in `m` below 1 km and in `km` above, with hysteresis.
    - Until the body's survey first arrives the line reads `SURVEY COVERAGE: PENDING`. Where this
      generator version computes no surface for the body, it reads
      `SURVEY COVERAGE: NOT YET MODELLED`.
    - On loss of the link its values go stale, in `--text-muted` with one trailing `S`.
    - `DECORATION ON`, where it stands, follows it.
    - The survey's resolution at the line of sight sets every reading's uncertainty, so it stands
      with those readings, in `FINEST`'s form, as `RESOLUTION 410 m` under
      `GROUND AT LINE OF SIGHT`."
  - _Rows._
    - `SURVEY COVERAGE`, kind `Label`: the share of a body's surface the ship has surveyed, on a
      view's label block, with its `FINEST` after a middle dot.
    - `FINEST`, kind `Label`: after `SURVEY COVERAGE` alone, the finest survey resolution held
      anywhere on the body. A literal word, where "best" would be a judgement.
    - `RESOLUTION`, kind `Label`: in a view's terrain readout, the survey's resolution at the
      point it reads, or `NOT SURVEYED`.
  - _Goes in_ "Graphs, schematics and spatial displays", "Views", after the star limit's bullet
    (lines 611–627); the rows after `DECORATION ON` (line 934).
  - _Extends_ "Colour", "The rendered image is data" (lines 92–104). Its `DECORATION ON` stands
    "beside its survey coverage", which this line is. It also extends:
    - the `DECORATION ON` row (line 934);
    - "Views", lines 611–627 (the label block's notes on what has not arrived);
    - "Data states": staleness (lines 344 and 351–352), and none as a value (line 395, with the
      `NONE` of lines 831 and 888);
    - the `STARS: PENDING` row (line 922) and `NOT YET MODELLED` (line 815);
    - "Numbers, units and time", lines 208–211 (ladders with hysteresis; E notation off the
      ladder);
    - "Voice and nomenclature", line 755–758 (a fraction named in words, a space before `%`).
  - _Signed off (signoff-9): amended._

- **Draft 5. The material reading.**
  - _Wording,_ a bullet of "Voice and nomenclature", after the phase bullet (lines 750–754): "The
    ground's material at a point is one reading of two parts, joined by a middle dot: first its
    form, then its substance (`MATERIAL REGOLITH · Fe-RICH BASALT`, `MATERIAL SNOW · H2O`,
    `MATERIAL FROST · CO2`).
    - The form is how the substance lies, a word from a fixed list: `BEDROCK`, `REGOLITH`, `SOIL`,
      `SAND`, `SEDIMENT`, `VOLCANIC ROCK`, `MELT`, `ICE`, `SNOW`, `SEA ICE`, `FROST`,
      `ORGANIC SEDIMENT`, `SEABED`, `EVAPORITE` and `DUST`. It is the form the view draws most of at
      that point, which is also the authoritative one there.
    - The form is the ground's class, not a substance or a phase, and keeps its word, as a
      structure layer does. A form word holds no common name of a formula: `ICE · H2O`, never
      `WATER ICE`. `MELT` is ground above its lithology's solidus, which petrology calls melt; it is
      not `liquid`, a word that would claim it is above its liquidus.
    - The substance is the ground's palette entry's. It is named only by the console's list of
      substance names, in a label's case. A formula keeps its case (`H2O`, `CO2`, `Na2CO3`), a name
      in words is upper-cased (`MARS-TYPE DUST`), and a substance the list does not name reads
      `UNLISTED MATERIAL`, never its key.
    - Where the body's composition is not modelled (its palette is empty) the reading is
      `MATERIAL NOT YET MODELLED`.
    - Prose and other readings still name a molecular ice as `CO2 ice`, by its phase. The form's
      reading is the ground's, and the two are not rivals."
  - _Real outputs_ of `substanceName(key, "upper")` today (`lib/substances.ts`, P14.T49.f, run
    2026-10-13):
    - formula keys name themselves: `H2O`, `CO2` and `Na2CO3`;
    - name keys take their names: `mars_dust` gives `MARS-TYPE DUST` and `tholin` gives `THOLIN`;
    - the lithology keys the reference and generated palettes use today give `UNLISTED MATERIAL`:
      `basalt`, `basalt_fe`, `granite`, `feldspathic` and `phyllosilicate`. Their names land with
      plan 14's T49.b rows, and plan 14 drafts `basalt_fe` as `Fe-rich basalt` (`Fe-RICH BASALT`)
      by signoff-5.

    So today a basaltic plain reads `MATERIAL REGOLITH · UNLISTED MATERIAL`.

  - _Rows._ `MATERIAL`, kind `Label`: the ground's form and substance at the point a view's terrain
    readout reads, joined by a middle dot. Beside it, one row, kind `Material form`, lists the
    fifteen form words with their meanings, the classes of this plan's `MaterialClass` (Design
    note 1):
    - `BEDROCK`, rock exposed, as on ground steeper than loose material can rest on;
    - `REGOLITH`, an airless body's loose, impact-broken layer;
    - `SOIL` and `SAND`, loose ground by aridity;
    - `SEDIMENT`, the fill of channels and basins;
    - `VOLCANIC ROCK`, an older volcanic province's;
    - `MELT`, ground above its lithology's solidus;
    - `ICE`, a glacier's or an ice cap's;
    - `SNOW`;
    - `SEA ICE`, ice over a sea;
    - `FROST`, a frost of the ice entry's species;
    - `ORGANIC SEDIMENT`, the fallout of an organic haze;
    - `SEABED`, the ground under a sea;
    - `EVAPORITE`, the salts of a dried basin;
    - `DUST`, the fines that wind lays.
  - _Goes in_ "Voice and nomenclature", after lines 750–754; the rows after `UNLISTED MATERIAL`
    (line 933).
  - _Extends:_
    - "Voice and nomenclature": lines 731–745 (a substance named by the console's list,
      `UNLISTED MATERIAL` where it names none, never a key); 746–749 (no common name for a formula,
      inside another name too); 750–754 (a phase is a word beside the name, `CO2 ice`); 762–764 (a
      structure layer keeps its word); 765 (a reading of parts joined by `·`);
    - "Typography", lines 143–145 (a formula keeps its case in an upper-case label);
    - the `UNLISTED MATERIAL` row (line 933), and a ring's `COMPOSITION` (`POROUS ICE`, `ROCK`;
      line 883), the precedent for a form word.
  - _Signed off (signoff-9): amended._

- **Draft 6. The survey edge, a mark in both styles.**
  - _Wording,_ a bullet of "Views": "Where a body's surveyed terrain meets ground the ship has not
    surveyed, the view marks the survey's edge, in both styles. Unsurveyed ground then never looks
    surveyed, and the step between the two is never read as a cliff.
    - Unsurveyed ground shows the body's reference surface, the `DATUM`, with no terrain, contours
      or reading claimed there. A skirt in the reference surface's appearance joins the two where
      their heights differ.
    - The edge is drawn along the surveyed terrain's border, as the chart's query edge marks where
      fetched data ends: a solid `--text` line `1px` wide with short ticks pointing into the
      unsurveyed side. Each tick is `0.25rem` long, one every `1rem` of the drawn line. The line is
      never dashed.
    - It is symbology. Over the photorealistic image it carries its `--surface-0` casing, as every
      mark does, and in the wireframe it is cased as the wireframe's lines are. Nearer ground in
      front of it hides it, as a limb hides a graticule.
    - While any of it is in view, the view's list notes it as `SURVEY EDGE`. The note gives the
      range to the edge's nearest point in the picture, measured and labelled as the list's ranges
      are (from the own ship, else `FROM CAMERA`), to three figures. It is a note outside the
      list's options, like `INTEGRATED STARLIGHT`: not a contact, and it cannot be selected."
  - _Row._ `SURVEY EDGE`, kind `Label`: the mark where a body's surveyed terrain ends, in both
    styles. It is a ticked `--text` line, its ticks towards the unsurveyed side, and the view's list
    notes it with its nearest range (`SURVEY EDGE 4.21 km`).
  - _Goes in_ "Graphs, schematics and spatial displays", "Views", after the bullet on what a view
    keeps (lines 597–610); the row after `SURVEY COVERAGE`.
  - _Extends:_
    - "Colour", "Outlines for symbology, and only for symbology" (lines 105–110, R02's item 3: a
      mark over the image is cased);
    - "Graphs, schematics and spatial displays", lines 552–554 ("The edge of the fetched data is
      drawn, so that unfetched space never looks empty"), as a principle. A view is not a spatial
      display (lines 593–596), so the mark is taken over from plan 05's D13 query edge ("a solid
      `--text` hairline with short outward ticks every 10°, the look of a chart boundary"), its
      ticks spaced along the drawn line instead of by angle;
    - line 503 (dashes are predictions);
    - "Views": lines 597–610 (what a view keeps), lines 644–658 (both styles, the black ground, the
      casings), lines 660–664 and the `INTEGRATED STARLIGHT` row (line 947), a list note that is
      not a contact;
    - lines 666–669 (every list entry carries a range) and 671–672 (the list names every kind in
      words).
  - _Signed off (signoff-9): adopted._

- **Draft 7. Contours, their interval and index.**
  - _Wording,_ a bullet of "Views", after the paragraph on both styles (lines 644–658): "In the
    wireframe style a view draws surveyed terrain as its contours, lines of equal `ELEVATION` above
    the `DATUM`. It draws nothing else of the ground, which is depth only and hides every line and
    star behind it.
    - _The lines._ Minor contours are solid `--text-muted` lines `1px` wide. Every fifth is an index
      contour, solid `--text-muted` `1.5px`, a step heavier, as a graticule's equator is. On a world
      with a sea, the coastline lies at the sea level, whether or not that falls on the interval,
      and is solid `--text` `2px`. So each kind differs from the others by width, not by colour
      alone.
    - _How they are drawn._ Each is drawn at its width as "Layout" gives it, constant across the
      picture. None is ever dashed, since dashes are predictions, or labelled in the picture, since
      the readout's `ELEVATION` and the `DATUM` carry the values. A contour in `--text-muted` is not
      stale, as an orbit is not. On the wireframe's black ground a contour is the ground's own line,
      and uncased; drawn over the photorealistic image, it is cased as any mark is.
    - _The interval._ The label block states the interval and the index once, in the form of a
      map's margin: `CONTOUR INTERVAL 200 m · INDEX 1 km`, each in `m`, and from 1 km in `km`, with
      hysteresis. The interval is a step of 1, 2 and 5 times a power of ten, never below 1 m. It is
      set by the camera's height above the ground beneath it, so that every view in one pose states
      the same one.
    - _Where contours are left out._ Where a family's lines would stand closer than `4px` (in CSS
      pixels, scaled as line widths are), the family is left out. It fades out between `6px` and
      `3px` of spacing, the minor contours first and then the index. The coastline is always drawn.
      A contour is scored as drawn in full; it is not scored in its fade, as a stroke is not scored
      near an open end. So surveyed ground without contours is steeper than the interval can show
      there, never flat, and ground with no contours at all is not surveyed.
    - _Scale._ With the field of view, the interval is the wireframe's statement of scale in place
      of the 1-2-5 scale bar. The field of view gives the picture's angular scale, and the interval
      its vertical one.
    - _The graticule._ The graticule is still drawn over surveyed terrain, and is told from the
      contours by its geometry: it crosses contours, which never cross one another."
  - _Rows._
    - `CONTOUR INTERVAL`, kind `Label`: on a wireframe view's label block, the height between
      adjacent contours, a 1-2-5 step (`CONTOUR INTERVAL 200 m`), with its `INDEX` after a middle
      dot.
    - `INDEX`, kind `Label`: after `CONTOUR INTERVAL` alone, the height between index contours,
      five intervals (`INDEX 1 km`). Not a belt's `SIZE INDEX`.
  - _Contrast._ `contrast.py text-muted surface-0` gives 7.22:1 and `contrast.py text surface-0`
    13.57:1, each above the 6:1 a stroke that carries meaning needs. The view's ground is black,
    darker than `--surface-0`, so either pairing is higher there.
  - _Goes in_ "Views", after lines 644–658; the rows after `FOV` (line 907).
  - _Extends:_
    - "Views": lines 644–647 ("its marks, graticules and contours included, follow this guide's
      ordinary rules for colour, stroke and contrast"), and lines 666–669 ("A view cannot carry the
      1-2-5 scale bar ... In its place the view states its horizontal field of view", R02's item 5),
      to which the interval is added;
    - the `FOV` row (line 907, "a view's statement of scale in place of a scale bar");
    - "Colour", lines 62–82 (a stroke meets its ratio as drawn; not scored near an open end);
    - "Graphs, schematics and spatial displays": line 486 (lines told apart by style first), lines
      503 and 512 (dashes are predictions), and lines 571–576 (a selected orbit wider, since colour
      alone must not carry it; `--text-muted` is not stale);
    - "Layout", lines 286–294 (widths times the larger of the device-pixel ratio and 2).
  - _Excepts_ nothing. It departs from Design note 14 in three places, each a question below: the
    index contour's width, the fade's scoring, and the datum on a line of its own (Draft 3).
  - _Signed off (signoff-9): amended._

- **Questions for the sign-off**, each with this draft's lean.
  - _Unsurveyed readings._ Design note 6, and the life-and-planet-content brainstorm in two places,
    write the em dash with `NOT SURVEYED`. Draft 3 sets `NOT SURVEYED` once in place of the
    readings, as `NOT RESOLVED` stands for a section, since the em dash means "not computed". Lean:
    Draft 3's form, with Design note 6 and those brainstorm sentences brought in line.
  - _The index contour._ Design note 14 makes it `2px` `--text-muted` beside a `2px` `--text`
    coastline, so the two would differ by colour alone (lines 573–574). Lean: index `1.5px`, as
    drafted. The alternative is a `3px` coastline.
  - _The contours' fade._ A fading contour is drawn below its 6:1. Lean: exempt it in its fade, as
    drafted, since T11 fades it. The alternative is a cut at `4px`.
  - _The graticule over contours._ Minor contours and the graticule's lines are both `1px`
    `--text-muted`. Lean: keep the graticule, told apart by geometry as drafted, and add to T11's
    recorded legibility check that the graticule is told from the contours, on a small body too.
    The fallback is leaving the graticule out over surveyed terrain.
  - _The datum's line._ Design note 14 states `CONTOUR INTERVAL 200 m · INDEX 1 km · DATUM …` as
    one line. T12 wants the datum stated once, in both styles. Lean: `DATUM` on its own line, as
    the USGS margin puts its datum under its interval.
  - _The datum's radii._ Design note 14 writes `6378.1 × 6356.8 km`, which NIST SP 811 §7.7 rules
    out ("51 mm × 51 mm × 25 mm but not: 51 × 51 × 25 mm"). Lean: `6378.1 km × 6356.8 km`, as
    drafted.
  - _The resolution at the line of sight._ T12 puts it on the survey line. Lean: in the readout
    group as `RESOLUTION`, beside the uncertainties it sets, since a bare figure on the survey line
    would read as a distance.
  - _A slant range off the hull._ It is measured from the camera. Lean: `SLANT RANGE FROM CAMERA`
    whenever the camera is off the hull, widening `FROM CAMERA`'s meaning from "no own ship".
  - _A mark at the line of sight._ The view draws none today, so the readout's point is unmarked.
    Lean: T12 draws one, of a shape no other mark uses (not the selection's bracket, nor the
    galaxy cursor's or chart centre's cross), drafted with T12 for the owner.
  - _The material under a coarse survey._ The class comes from the finest level, so it can name
    `BEDROCK` on a cliff the survey has not resolved. Lean: name the class drawn, as Verification
    has the readout do, gating it by coverage but not by resolution, since it is not a number.
  - _`SEABED`'s substance._ Design note 1 puts `Seabed` "under its liquid entry". If that entry is
    the material's, the reading says `SEABED · H2O`, a floor made of H2O. Lean: the seabed's entry
    is the floor's own, the crust's or a deposit's, and the sea is named by its coastline and its
    `SEA LEVEL`.
  - _The material's properties._ T12 shows "its properties as a reading", whose names are not
    drafted here: bearing capacity and pad friction, with their footing width and temperature.
    Lean: T12 drafts them for the owner before it builds them.
  - _Precision._ `ELEVATION` 0.1 m (Design note 6); `SLOPE` whole degrees; `SLANT RANGE` three
    figures; resolutions two. Lean: as drafted.
  - _`ELEVATION`'s sign._ The issue's form, `~2140 m ± 180 m`, sets no `+`, while the spike's
    `SITE HEIGHT` (line 977) is signed. Lean: no `+`, as maps write it.
  - _`SITE HEIGHT`._ By ICAO's distinction it is an elevation. Lean: rename it `SITE ELEVATION` in
    `ELEVATION`'s form when the spike next changes (R05's row; `DescentSpike.tsx`). It is not this
    task's to change.
  - _NIST and the guide's spans._ The same §7.7 also rules out "0 – 5 V", the form of the guide's
    spans (`66.0 – 137 Mm`, line 227–229). The drafts do not touch the span rule. Lean: leave it, a
    separate question for the owner. _Ruled 2026-10-19 (`signoff-12.md`, item 18): both forms kept
    as the console's choice, stated in the guide; a span inside a sentence takes "to"._

- **If signed off, these follow** (other tasks' text, not changed here): Design note 6 (the
  unsurveyed form); Design note 14 (the index width, the fade's scoring, the datum's line and its
  radii's units); T11 (its fade test's thresholds in CSS pixels, its legibility check);
  T12 (`RESOLUTION` in the readout group, `SLANT RANGE FROM CAMERA`, the point's mark, the
  properties' names); the life-and-planet-content brainstorm's sentences on `NOT SURVEYED`; R05's
  `SITE HEIGHT`; and the roadmap README's "Still awaiting" entry for R10.T13.

- **Checks** (2026-10-13, this worktree).
  - `glyphs.py "± · × ° — ~" '\u00a0'`: every character outside ASCII that the drafts put on
    screen, with the tilde and the no-break space (exit 0):

    ```text
    char  code     B612   Mono   name
    ±     U+00B1   yes    yes    PLUS-MINUS SIGN
    ·     U+00B7   yes    yes    MIDDLE DOT
    ×     U+00D7   yes    yes    MULTIPLICATION SIGN
    °     U+00B0   yes    yes    DEGREE SIGN
    —     U+2014   yes    yes    EM DASH
    ~     U+007E   yes    yes    TILDE
          U+00A0   yes    yes    NO-BREAK SPACE
    ```

  - `ux_lint.py` on a scratch `.ts` file, outside the worktree and not committed, that holds every
    string the drafts propose as a string literal (exit 0):

    ```text
    0 error(s), 0 check(s) in 1 file(s) scanned. Heuristic: confirm each line against the guide.
    ```

### R10.T14 Cache sizes

- **R10.T14.a The count model and the slot layout.** `patchCountModel` as a pure function of camera,
  τ and viewport (Design note 15, after the research model's rule); `bytesPerPatch`;
  the budget per setting, provisionally 64 MiB low and about 400 MB high (Design note 15), in R05's
  `TerrainSettings.cacheBytes` (`view/quality/qualitySetting.ts`, the one list), as fixed slots in
  the layout of Design note 15 (storage buffers for unfiltered data, arrays or atlases for filtered
  data), handed to R05's `PatchCache` as its `SlotLayout`. As built, `slotLayout.ts`'s
  `SlotFieldName` holds `horizon-map` at 0 B and no class-weights or survey-mask field: each is
  added by the task that first uploads it (T8.b, T10.b, T10.c), and this task sizes them. Files:
  `cacheBudget.ts`, R05's `cache.ts`, `slotLayout.ts` and `view/quality/qualitySetting.ts`. Tests
  (pure): `bytesPerPatch` matches Design note 15's formats
  for each setting and vertex path; `patchCountModel` reproduces the table of Design note 15 within
  10%; the cache never evicts a patch under a grounded body at the chosen size during a replay of
  the descent's selections. Acceptance:
  `pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain`.
  _Ruled (decision-r10-t14a, 2026-10-20, and its #56 follow-up):_ the model is built on the
  selection's own metric (R05's box distance at the premise's bound, without the balance, the
  forced region or the budget), and takes the margin a side, the bound's ratio and a relief
  half-height as inputs; its test holds Design note 15's table to [0.5, 2] a cell on the exact
  horizon, the box-horizon ratios recorded beside it, since no one faithful reading meets all three
  columns within 10% (on the exact horizon the model's high column is ×1.53–1.91 the note's at
  k = 5, and ×1.04–1.15 all-round and ×1.12–1.37 in the frustum at k = 4), and its validation is a
  cross-check against `selectPatches` on the premise planet; R05's margin along the normal passes
  coarse patches below the horizon; R10.T14.e fixes it; the constants stay provisional and
  unchanged; every GPU item is T14.b's but the compile lever, T14.d's.
  Records: [10-terrain-on-generated-worlds/records/R10.T14.a.md](10-terrain-on-generated-worlds/records/R10.T14.a.md).
- **R10.T14.c The descent over a generated world** (new at the scoping of 2026-10-16,
  `decision-r10-live-path.md`; before T14.b). R05's descent spike flies a body source:
  `--world terrain-test` takes `TERRAIN TEST`'s fixture body and field through T16's session (its
  store over the fixture client, its pass and its lanes); the test planet stays the default, bit for
  bit. Its main view is `VIEW`'s own frame drawer over the session, in either style (`--style`),
  stepped as `VIEW`'s primary is, with the spike's wireframe instruments beside it and no atmosphere
  (the body is airless). The landing site lies inside the survey, its track from the low pass's
  start to touchdown over surveyed ground. The profile's inputs are the workers': the site's height
  is the finest bake's ground at the site on R05's diagonal (collision's ground over the client's
  field, as the test planet's was T4.c's interpolant; not `slantRange`, which reads the survey's
  resolution and would put the hover inside the drawn ground by the omitted relief); each stretch's
  floor is its baked maximum plus ε_n from the workers' level table where every vertex is surveyed,
  and elsewhere the body-wide ceiling (the table's highest over its rows plus the header's realised
  relief and a metre, `reading.rs`' `ceiling_m`), since the table bounds only the cells held;
  `SITE ELEVATION` states the workers' reading at the site, in `ELEVATION`'s form. The session's
  contacts stand on the drawn ground of the pass's resident bakes where it is held
  (`heldGroundUnder`), the datum elsewhere (decision-r10-t16-contracts §2). A craft held in another
  body's frame is judged against the body it nears, its velocity chained through the system frame by
  each body's velocity at the epoch it is drawn at, the ship's local body's present velocity and
  every other body's retarded one (decision-r10-t16-leans §13; decision-r10-ship-frame, finding 4).
  T14.b measures on it, and T15's descent on a served world takes the same path once the serve gate
  opens. Files: `view/spike/` (`spikeRun.ts`, `spikeScene.ts`, `descentProfile.ts`'s inputs,
  `DescentSpike.tsx`) and `scripts/descentSpike.sh`; `view/terrain/grounded.ts` and `session.ts`;
  R02's `view/scene/model.ts` and `fromServer.ts` for the local body's present velocity; the guide's
  `SITE ELEVATION`, `SPIKE LIT`, designation, `DESCENT SPIKE`, `TERRAIN NOT AVAILABLE`,
  `SURVEY COVERAGE REJECTED` and `SYSTEM DATA INVALID` rows. Tests: the profile over the fixture clears its
  floors, its track from the low pass on over surveyed ground and its off-survey floors at the
  ceiling; a landing on a plateau more than 1 km above the datum is a contact; the frame chaining on
  a small moon near its planet whose Hill radius approaches its own radius, the local body's present
  velocity taken; `just descent-spike --smoke --world terrain-test` (hidden) exits 0. Acceptance:
  `pnpm --filter hyperion exec vitest run src/renderer/src/view/spike src/renderer/src/view/terrain src/renderer/src/view/scene`
  and the smoke. _Leans ruled: decision-r10-t14c-leans._
  Records: [10-terrain-on-generated-worlds/records/R10.T14.c.md](10-terrain-on-generated-worlds/records/R10.T14.c.md).
- **R10.T14.b The measured sizes.** Measure the all-round and frustum peaks per view on R05's
  scripted descent over a generated world (T14.c's, through T16's session), on both settings and in
  both styles, with a second view
  open; set each setting's `cacheBytes` from max(1.3 × all-round peak, 3 × frustum peak) summed
  over the streaming views plus the forced region, or the frustum rule on the high setting if the
  all-round peak is never resident (Design note 15), recording which, whether R05 chose
  `BakedOffsets` (as built R05.T18 keeps it on the high setting), and whether 64 MiB and about
  400 MB hold, or the finding; and record the resident
  total (the cache, the coverage mask, the class map, the cascades, each worker's field copy)
  against the ceilings of 1 GB low and 2–3 GB discrete, from driver-reported GPU memory as well as
  computed bytes.
  It also takes (decision-r10-t14a §1): a measurement at a 1 s/s scene clock beside 60 s/s, each
  frame's motion margin, bound ratio and box heights logged, so that the wireframe's "more than
  twenty times" (T11's deferred findings) is split into its factors and the wireframe's budget sized
  from it; the wireframe's share against the model's and Design note 15's ratios; the horizon's GPU
  time by an A/B with the layer off; eviction in place of rebakes, weighed against the undrawn
  rebakes' worker time and built if adopted; "The live path's costs" and the T16 records' measured
  costs; the owner's UHD 620 figures before the low setting is sized; and the binding ceiling
  (1,323 lit slots, 411,484,752 B, on `baked-offsets` under a default 128 MiB binding) for each
  machine. _After R10.T1.c's client half: the same 1,323 slots are 437,907,708 B at the 330,996 B
  slot._ If R10.T9.b has not landed, the cascades are counted at 67,108,864 B from T9.a's spec,
  labelled computed.
  Its sizing runs (the peaks, `cacheBytes`, the binding ceiling) follow R09.T8.d, which tightens
  the level table's reach and with it the horizon occluder and the boxes (decision-r10-t14c-leans
  F4; decision-r09-t8d-reach §5). Its other items need not wait. Its record states the occluder
  depth and T14.c's ceiling it ran with, and that the flare's demand figures come from a plunge.
  It runs an A/B: the same descent once more with a diagnostic occluder at the sampled extreme
  depth (never shipped), recording the slots the remaining looseness costs in each setting's
  all-round and frustum peaks; above 10% of a setting's sized all-round peak that is a finding to
  main before the low setting's budget is final.
  By hand, recorded, on a quiet machine: the peaks and totals on the development
  machine's RTX 3080 and, by the owner, on the UHD 620. Acceptance: the figures recorded and the
  constants updated, with `pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain`
  passing.
  _Ruled (decision-r10-t14b-measure, 2026-10-21): sized on the as-built selection's measured
  peaks (the boxes), counted once a second of script time by unbudgeted re-selections of the
  pass's own inputs, all-round by a horizon-only view, the forced region once; the A/B's
  diagnostic occluder counted in the same flight; the second view the spike's craft instrument
  as a session view; no views-check run, since TERRAIN TEST's track is a body-fixed function of
  script time and its clock reaches selection through nothing; eviction in place of rebakes
  deferred; the constants set by a second ruling on the figures._
  _Ruled (decision-r10-t14b-constants, 2026-10-21): the rule is read in its union form over the
  whole run, max(1.3 AR∪, 3 F∪) plus the forced region once, and on the high setting as the
  frustum rule, its all-round set never resident; on the hard bound, at the spike's canvases, it
  asks 8,666 high lit slots, 2,996 low lit and 3,764 wireframe, about seven times the provisional
  constants, which are held at 400,000,000 B, 64 MiB and 520 slots, since half the slots are the
  selection's draw budget, the high figure passes the brainstorm's 3 GB failure line and the low
  setting waits on the owner's UHD 620 run; they are re-sized once by this rule, within the
  resident ceilings and a timed run's GPU rows, after R10.T4.b's statistical bound; a layout past
  the device's storage binding keeps its slot count on `face-differences`, clamped to the
  binding's ceiling._
  Records: [10-terrain-on-generated-worlds/records/R10.T14.b.md](10-terrain-on-generated-worlds/records/R10.T14.b.md).
- **R10.T14.d The hand-over's compile stall** (new, decision-r10-t14a §1, from
  decision-r10-t16-leans §2; amended by decision-r10-t14d-stall, 2026-10-20). A hand-over stopped
  the page for 1,317 ms with no animation frame on low, a defect under the guide's Motion rule that
  a view never stops. The stall is R07's mesh-regime material `bodies:smoothMesh`, a
  `createMaterial` material whose pipeline is made synchronously on the GPU process's main thread at
  its first draw, the body's first frame in T10.e's `terrain` regime (1,547 ms on a cold driver
  cache); the terrain's own materials, asynchronous, only delayed its first draw (2,483 ms cold).
  Two levers:
  Records: [10-terrain-on-generated-worlds/records/R10.T14.d.md](10-terrain-on-generated-worlds/records/R10.T14.d.md).
  - the terrain's: its lit material, its depth material and the wireframe's are compiled when the
    terrain session is made, before any hand-over, so that the terrain draws on the hand-over's
    frame;
  - the bodies': R01's `RenderEngine` gains `prepareMaterial(material, targets, meshes)`, which
    makes a material's pipelines ahead of their first draws with `createRenderPipelineAsync` and
    keeps them in the material's own pipelines; for a `createMaterial` material it marks none
    pending, so that a draw before one is made makes it synchronously, as before, and draws on its
    first frame. `LitBodyRenderer` prepares `bodies:smoothMesh` on its patch mesh and
    `bodies:disc` and `bodies:discLimb` on its quad, for the scene target, as it makes its
    resources (as a view's renderer is made, and at each restore), so that a regime's first draw in
    a running view (the hand-over, and a body's first disc past the 3.3 px switch) compiles
    nothing. VIEW's opening, a disc drawn before the preparation lands, is R01's (deferred below).

  Then the terrain run gains a limit on each view's longest time between two of its draws, from
  the end of VIEW's opening (its views' preparations settled) to the teardown, every phase and each
  session's making and compile, outside the forced loss's restore; VIEW's opening is recorded, not
  judged. The limit is judged on every run, whatever the shader caches hold, and set from one cold
  launch a setting (the driver's cache and Dawn's blob cache both empty), under the heavy lock on a
  quiet machine: 1.5 times the worst judged gap, rounded up to 50 ms, held within 200 to 500 ms; a
  gap that would need more is a defect finding, not a higher limit (250 ms provisional until then;
  the worst gaps at T16 117 ms on high alone and 283 ms as the instruments open). Desktop: R01's
  engine (`view/engine/`), R07's `bodies/draw.ts` and the session, so `just test-render` and the
  hidden `just views-check --terrain` on both settings. Free now T16 has landed; before T14.b's
  quiet runs. Tests: the session compiles its materials at creation, and a hand-over then compiles
  none; a prepared `createMaterial` material's first draw makes no pipeline, a draw before its
  preparation lands makes its own, which is kept, and none of its draws is left out; a lost
  engine's material is refused; `LitBodyRenderer` prepares its three regime materials as it is made
  and after a restore; the terrain run fails on a gap over the limit in its judged span.
  Acceptance: the lane's `just smart-ci`, `just test-render` and the two cold hidden terrain runs,
  with the gaps, the load and each cache's state recorded.

- **R10.T14.e The patch boxes' margin along the normal** (new, decision-r10-t14a's #56 follow-up).
  R05's `packPatchBounds` (`bounds.ts`) adds its margin, the largest chord between neighbouring
  ring samples (a sixteenth of the patch's edge), to all three half-extents. Along the centre's
  normal it is not needed: the patch's highest point along that normal is its centre vertex at the
  top of its range (the support point of a convex figure in its own normal's direction), sampled
  exactly, and its lowest lies on its boundary, which departs from the chord between two samples by
  at most the chord's sagitta under a stated bound on the boundary's curvature. So the normal's
  half-extent takes that sagitta, rounded up, in place of the chord; the tangents and the sphere
  keep the chord. On the count model's premise at 100 m the chord passes 22% of the high setting's
  all-round leaves, wholly below the horizon (T14.a's record). Tests: every vertex of the 65 × 65
  grid and a 257 × 257 sampling of the surface, at both ends of the height range, lies inside the
  box, for patches of levels 0–19 over faces, edges and corners, on WGS 84, a sphere and a figure of
  flattening 0.1; `bounds.test.ts`'s oracle takes the same margins; the count model's box and
  exact-horizon counts, and R05.T13.a's fourteen descent windows' patch counts, before and after;
  the windows' pinned hashes (`demandRecord.test.ts`) and T14.a's box-metric table re-pinned, each
  move stated. No golden file and no generator version. Client-only, Vitest; the orchestrator runs
  `just test-render` at landing, since the smoke harness's terrain parts select through the boxes.
  After T14.a; before T14.b's measurements.
  Records: [10-terrain-on-generated-worlds/records/R10.T14.e.md](10-terrain-on-generated-worlds/records/R10.T14.e.md).
- **R10.T14.f The class-mapped point's first frame, off the render thread** (new,
  decision-next-lane-2026-10-21b §1–§2; a critical bug, from T10.f's deferred "A map's first point
  costs a frame" and T10.g's "The point's first frame at levels 7 and 8"). The first frame a body
  was drawn as a point under a class map made the point integral's elements on the render thread:
  their shares (98,304 `classMapSharesAt` calls) and, under the field's normal map, k² normals an
  element, about 0.3 s at field level 6, 0.6 s at 7 and 1.4 s at 8, stopping every view and the
  console's UI: 28 times R05 Design note 21's hitch row (none over 50 ms past 3 T) and 5.6 times
  T14.d's gap limit (decision-r10-t14d-stall §3). Each surveying block's new map made them again.
  The build leaves the frame:
  Records: [10-terrain-on-generated-worlds/records/R10.T14.f.md](10-terrain-on-generated-worlds/records/R10.T14.f.md).
  - the elements are made in slices of at most about 2 ms a frame, run by the terrain session's
    step, from the moment the class-map store takes a map; a newer map cancels the older's
    unfinished build, so that no queue grows; the frame's path never makes them;
  - until a map's elements are made its point is the previous map's, where that one's were made,
    and otherwise the uniform point of `elsewhere`: what was drawn before the map came, never NaN
    or blank;
  - the finished ratio is the synchronous path's bit for bit;
  - a light's evaluation on a cache miss at level 8 is measured (under time acceleration a fast
    rotator misses every frame): over 4 ms it is in scope, by the same lever or slicing;
  - the remake at each surveying block is checked, and the memory (4.7 and 19 MB) counted or
    recorded.

  It touches `view/bodies/classMapPoint.ts` and its test, `view/bodies/draw.ts`'s point path,
  `view/terrain/classMap.ts` (the store starts and cancels the builds) and its test, and the
  frame's slice hook in `view/terrain/session.ts`' step and its test. It is tested that the first
  point frame under a map whose elements are unmade draws the uniform point and queues the build;
  that the slices end at their budget on a fake clock; that the sliced elements' ratio is the
  synchronous path's bit for bit on level-6, 7 and 8 fixtures; that a replaced map's point stands
  in until its successor's are made, and a newer map cancels an unfinished build; that the store
  queues and cancels a map's build and stands the old map in at a remake; and that the session's
  steps make them. It is accepted on
  `pnpm --filter hyperion exec vitest run src/renderer/src/view/bodies src/renderer/src/view/terrain/classMap.test.ts src/renderer/src/view/terrain/session.test.ts`,
  the lane's `just smart-ci --hook-checks origin/rendering-and-planets`, the quiet bench at levels
  6, 7 and 8 recorded (the longest slice, the build's total, the frames to ready and the load), and
  the stand-in's step at readiness recorded on T10.f's synthetic Iapetus and on a remade level-8
  map. No golden, and no generator or protocol version: client presentation only. After T10.g;
  before T1.c's client half, which adds a maturity to the same shares.

- **From R10.T16's rulings** (decision-r10-t16-leans §2, §13 and §16, 2026-10-20). T14 also takes
  (routed by decision-r10-t14a §1: the compile lever to T14.d, the rest to T14.b):
  - _The compile lever_ (§2): the lit and depth materials compiled when the terrain session is
    made, for the whole-page stall at the hand-over (1,317 ms with no animation frame on low, a
    defect under the guide's Motion rule that a view never stops). Once it lands, the terrain run
    gains a limit on the longest gap outside the restore, which T14 sets from the quiet-machine
    runs (the worst gaps today 117 ms on high alone and 300 ms as the instruments open).
    _Amended by decision-r10-t14d-stall: the stall is R07's `bodies:smoothMesh`, compiled
    synchronously at the hand-over; the terrain's materials only delayed its first draw. T14.d
    builds both levers._
  - _Eviction in place of rebakes_ (§13, T16.b's second question): an undrawn resident patch the
    rebake rule names may be evicted instead of baked again, weighed beside the deferred
    "Undrawn rebakes cost worker time".
  - _The horizon's GPU time_ (§16): T8.b's lookup against the budget's Shadows row (0.3–0.5 ms),
    by an A/B with the horizon layer off (T8.b's question "The horizon layer as an argument"),
    since the views check times frames and the lookup is inside the terrain's lit pass.

### R10.T15 Verification pass

Split at the scoping of 2026-10-21 (decision-r10-t15a-scope): T15.a needs nothing of the serve
gate, since `ground_at` and the bake are the same functions on any field; T15.b is the served-world
pass.

- **R10.T15.a Collision agrees with what is drawn, end to end.** The collision-agrees test end to
  end: a TypeScript test that forms the finest patch's vertices with R05's vertex formation in
  `Math.fround` (`gpu/vertexEmulation.ts`), from a bake of a real patch committed as a fixture, and
  interpolates on R05's diagonal, agrees with `ground_at` from the same fixture to 2 mm plus the
  formation's stated error (the height texture's `f32` step at ±20 km is 2⁻⁹ m, 1.95 mm: the
  brainstorm's "about 2 mm", under The geometry), on both vertex paths, `FaceDifferences` over the
  client's own `patchTerms` and `BakedOffsets`' q₀; R05.T4.b's formation error is under 1 mm, so
  the bound is 3 mm. The fixture's files are `tests/golden/drawn_ground/<world>.golden` (Earth-like
  and Moon-like, three finest-level patches each, inside a face, on a face edge and at a cube
  corner), asserted on native, `wasm32-wasip1` and `wasm32-unknown-unknown`. Acceptance:
  `cargo test -p hyperion-surface --lib bake::drawn_ground`, blessed once with `HYPERION_BLESS=1`
  at the version in force and then passing without it; `just test-wasm-fast`; and
  `pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain/gpu`: every sample on both
  paths within 3 mm, the half check passing, the frames' check passing, and the control failing as
  expected.
  Records: [10-terrain-on-generated-worlds/records/R10.T15.a.md](10-terrain-on-generated-worlds/records/R10.T15.a.md).
- **R10.T15.b The served-world pass** (after the serve gate opens, decision-bump-plan-after-22
  §1.4, and after T4.a). R05's descent run on a served generated world at both settings, through
  T14.c's body source, once the serve gate opens (decision-bump-plan-after-22 §1.4), with T10.e's
  brightness check on a fully surveyed served world and R03.T15's 61-chunk check in the Electron
  renderer (R09.T19.c's pending item), which T16's session now asks for, with its metrics harness
  and the horizon map on (T8.b), recording the map's share of the workers' bake time and
  `bake/horizon`'s WebAssembly figure (main's ruling for R10.T8.a, 2026-10-15), patches a second
  sustained against the brainstorm's demand of (200 · v + 290 · |ḣ|) ÷ h, the level bound per level
  from T4.a, and pops or cracks seen; every golden of this plan asserted on the three targets.
  The brightness check is T10.g's hand-over check (`--views-check --terrain --handover`) with the
  served world as its `HandoverWorld`, which T10.g left unbuilt, judged as T10.g's except that
  its per-texel bound leaves out each cell where the disc's μ floor binds at the cell's centre
  (n′ · v < ½ μ_ν) and counts those cells in the record, the body's and the survey's means still
  judged over every pixel. Its terminator look is recorded, not judged. If it shows a line of
  terminator cells over 1/3 stop, a new R10.T10.h builds the lever, in this order: trace T10.g's
  cell 7810; then a coarse shadow term in the disc from the field's horizon toward the star; then
  the cross-fade over the hysteresis band, for what those two leave (decision-r10-t16-leans,
  "R10.T10.g terminator cross-fade"; T10.g's record and deferred findings).
  Acceptance: the lane's `just smart-ci` and `just test-wasm-fast`, this plan's slow binaries by
  name (`just test-slow -E 'binary(…)'`) and `just bench -- bake/` complete, and the descent's
  figures are recorded; `just ci` and the slow pass are the orchestrator's.

## Verification

- **Collision agrees with what is drawn:** the finest mesh and `ground_at` to 2 mm (T3, T15.a).
- **Level-of-detail consistency:** every coarser level inside R09's bound on generated worlds, both
  ratios recorded (T4.a); selection by that bound, history-independent (T4.b).
- **The ground drawn and the ground quoted agree:** the largest weight is the authoritative class at
  the finest level; coarse weights within ten points of brute force (T2); the readout names the
  class the shader draws most of (T10.b).
- **Photometry:** the scaled classes reproduce the body's p (T10.a); the handover changes brightness
  by under 1/3 stop at two phases (T10.e); a patterned body's point equals its disc at the 3 px
  switch to 1% (T10.f).
- **Knowledge:** unsurveyed ground has no terrain, no contours, no class-map texel and no reading; a
  readout's uncertainty matches the pooled RMS of the unsurveyed bands (T5, T12); the image is the
  same at every survey resolution that covers it (T6.a, T10.c, T10.d).
- **Determinism:** material and ground goldens on native and both wasm targets (T1.b, T3); bakes
  order-independent, the horizon far field bit-identical at shared points of one level on one face,
  and blended to the parent's map across a level change (T6.a, T8.a, T8.b; main's ruling for
  R10.T8.a, 2026-10-15).
- **Benches:** `bake/patch_65` (40 ms), `bake/horizon` (a quarter of it), taken on a quiet machine
  and recorded with the load average; a run under shared load is provisional and repeated.
- **By hand, recorded:** on the development machine (RTX 3080), the cascades (1.5–3 ms), the
  disc-to-terrain handover, the descent on a generated world and the cache peaks; by the owner on
  the UHD 620, the horizon-map pass (0.3–0.5 ms), the station wireframe at 60 fps at 1080p under
  load and the cache peaks.

## Generator version

T1.b bumps `GENERATOR_VERSION` once: the material class is new authoritative output, read by landing
and the consoles, and its goldens pin it. The class discriminants, the property table, the
classifier's thresholds and its order of rules are generator-versioned from then on; a new class
appends a discriminant. So are T1.a's codes for R09's `PaletteEntry::phase_row`, which the payload
carries (the re-validation; the layout's freeze at R09.T19's first send is R09's Generator
version). `ground_at` reads only heights R09's goldens pin, so T3 adds goldens without
a bump. T10.a bumps it a second time: the albedo scale ships in R09's payload header, whose format
and contents R09's Design note 17 puts under `GENERATOR_VERSION`, and R09's payload and coarse
goldens re-bless in the same commit; from then on the solve and the class albedo ranges are
generator-versioned too. _As built (main's ruling for T10.a, 2026-10-14): no bump. R09.T19.b's
ruling freezes the payload's layout only at the first payload served for a generated body, or a
client's first persisting of one, and no generated body is served before P14.T54.a, so T10.a
fills the header at 22. No golden of R09's moves (the server sets the scale after `coarse_pass`,
and the testing worlds' payloads keep `None`); T10.a's own goldens, `tests/golden/albedo/`, carry
22, and the solve and the ranges are generator-versioned from them (Risks, "Deviations in T10.a,
as built")._ The weights, the class map, the bounds' use in selection, the readings and
the horizon maps are presentation and move nothing generated; a change to them is a client change.
No domain tag is added: nothing here draws from a stream. The plan adds no protocol message of its
own; it reads R09's `surface_field` (with `CoverDto`'s per-range `ResolutionCode`) and R03's frames,
and fills R09's `FieldHeader.albedo_scale`, which R09 already reserves.

## Risks and open points

Task records as built: [10-terrain-on-generated-worlds/records/](10-terrain-on-generated-worlds/records/), one file per task, each linked from its entry under Tasks.

- **The local body's terrain sits ωτ ahead of its lighting (from R07.T10.a, ruled 2026-10-05).**
  R07 lights the ship's local body at its retarded time, T − τ, as the brainstorm draws every
  time-varying state, while its geometry, and so this plan's terrain, is drawn at the present.
  Once rotation is drawn, the terrain sits ωτ ahead of its lighting: about 2,100 km at a Jupiter's
  equator from its Hill sphere's edge (τ 168 s), about 38 km at τ 3 s. This plan decides whether
  to draw rotation-dependent state (the class map's orientation, shadows on terrain) at the
  retarded time too. _Settled for the terrain by R10.T16.c: the terrain, its mask, its class map
  and the reference surface take the body's one drawn rotation (R07.T10.a's `rotationAt`), and its
  lights are the body's lighting frame's, so the terrain adds no offset of its own; the ωτ between
  geometry and light stands as R07 ruled._
- **The selection bound's calibration is ruled by its numbers** (`signoff-12.md`, item 30). If
  T4.a's 99.9th-percentile ratios fall at the expected 0.3–0.5, the hard bound costs four to eleven
  times the patches of a perfect bound; selection by min(hard, 4σ) recovers two to three times. T4.b
  applies the rule to T4.a's recorded figures; until then selection uses the hard bound. _They fell
  at 0.000–0.074 (R10.T4.a's records, at the end of this section; Design note 5's pointer), and
  R10.T4.b's record applies the rule._ _decision-r10-item30 keeps the hard bound at RM5, with no
  calibration factor: at the high budget, on flat boxes, it draws 1.3–3.8 px a pose (mean), 1.1–1.3
  times a perfect bound's, against a stated τ′ of 11–51 px on the same boxes (38–178 px unbaked);
  and under it the workers read no σ (R10.T4.c's record)._
- **The low setting's cache is at its edge.** With 16-bit horizon maps a low-setting patch is
  about 156 kB, and the sizing rule's 430 slots fill 64 MiB exactly before relief and balancing. T14
  decides between a larger low cache (inside the 1 GB ceiling), a half-resolution near-field horizon
  (33 × 33, 17 kB) and the frustum rule, and records which. R05 choosing `BakedOffsets` would break
  the 64 MiB outright. _As built: 422 lit slots in 64 MiB (158,936 B a slot); the model's rule gives
  about 935 at τ_sel on the boxes' horizon, what the cache holds today, and about 676 on the exact
  one, the estimate after R10.T14.e, each with the descent's 71 forced patches (T14.a's record).
  T14.b decides after the UHD 620 run._ _After R10.T14.e: about 708 at τ_sel on the boxes' horizon
  (T14.e's record)._ _After R10.T14.b: 2,996 slots (455 MiB) measured on the hard bound, ×7.1 the
  422; held at 64 MiB until the UHD 620 run and R10.T4.b (decision-r10-t14b-constants)._
  _After R10.T1.c's client half: 375 lit slots in 64 MiB (178,908 B a slot), so the 2,996 are ×8.0
  the 375 (T1.c's client-half record and deferred finding 14)._
- **Coarse weights assume Gaussian slopes.** The slope-exceedance probability of Design note 3
  treats the unresolved slope as Gaussian; ridged and cratered terrain is not. The ten-point test is
  the guard; if it fails on a class of world, the fallback is counting a sparse sample of finest
  points, at a cost T6's bench states.
- **The survey edge's skirt** is a visible artefact by design. If players read it as geology despite
  the mark, the alternative is fading the terrain to transparency over a cell, which draws no
  invented ground either.
- **Figures from memory.** Most per-class albedos, friction and strength figures, and several shadow
  and contour conventions, were recalled rather than read by the research; each task that turns one
  into code checks it against the primary named in its design note first.

- **Asks of other plans, which the owners may not yet carry** (listed in the notes): of R05, nothing
  now: its interpolant, bake and bound take their height source as R05's `HeightSource` trait
  (R05.T4.a, T4.c), which T3 and T6.a implement for R09's `Synthesiser`, and its Design note 4
  carries this plan's per-patch figures (Design note 15); of R07, which lands first, `body_brdf`
  with per-texel lunar-Lambert parameters, its disc sampling this plan's class map, and
  `sphere_irradiance` taking a local horizon, are hooks R07 creates with defaults (R07.T4.c, T6.a,
  T6.c, T8.b) and this plan fills (T8.b, T10.b, T10.d); it also extends R07's files with an L(α)
  channel in the phase table (T10.b), a masked bilinear class map (T10.d) and the point's class-map
  integral (T10.f), and wires `surface` and `rotation` through R07's `litBodiesOf` (T10.d)
  (decision-r07-t8b), and a disc's pattern waits on plan 14's rotation (P14.T46.f, read by
  R07.T2.b); of R08, the sun's refracted elevation for the shadow test; of R11, that its cloud
  shadows enter through `terrain_shadow_factor`, its ring shadows through `terrain_ring_shadow` and
  its diffuse cloud light through `terrain_sky_factor` (R11's proposed names, adopted). R09's closed
  forms, per-cell resolution, header field and spheroid datum (its Design note 17), R05's
  `rg16float` normals and slot layout, and R04's `math::j0` are already carried and are listed under
  Consumes. Of R09's palette sources and P14.T49.c, from T1.a: an `Ice` entry's A_N is its
  substance's as fresh snow or fine frost (the forms' factors are taken against it), and a
  `phase_row` is filled by `PhaseLaw::for_family` until a substance's own law is known.
- **Shadows on the high setting** are taken here by the reading of Design note 11. If the roadmap
  gives them to R07 or R11 instead, T9 moves whole, and T8's horizon map stays here.
- **`Dust` is placed by a simple rule** (arid, thin-aired, where R09's wind deposits); if the coarse
  field cannot tell deposition from erosion well enough, `Dust` falls back into `Soil` with its
  lower bearing figures, and the class stays reserved.
- **Classes plan 14 cannot yet reach.** `OrganicSediment` (a Titan-like haze regime) and
  `VolatileFrost` of methane need species plan 14 does not produce: it gives only H₂O, CO₂, N₂ and
  Ar, and its Titan has no methane (R08's Design notes 13 and 16). They stay reachable only through
  R09's surface-state classes, and on no generated world until P14.T24.f's carbon speciation
  (R08.T1's draft) lands; T1.b's tests build them on a `FieldBuilder` field and record that no
  sample world has them. _P14.T24.f landed on 2026-10-10 (`4cb28c0b`, `324fbe34`); what now keeps
  them off generated worlds is R09's coarse pass (T11–T16) and plan 14's surface section
  (P14.T48.e), which `for_body` reads (the re-validation of `c6d77cac`)._
- **Vegetation** is absent until a biosphere exists; a living world's land is drawn bare, which is
  wrong for it and says nothing false about what the generator computed.
- **The hardware decisions, decided 2026-09-30 by a delegated decision**, as they fall on this
  plan:
  - _CPU budgets (item 3):_ the 40 ms patch bake (`bake/patch_65`, and the horizon's quarter of
    it) stays the UHD 620 laptop's figure. The development machine records its own and fails only
    if it is over the laptop's budget.
  - _Memory (item 5):_ the ceilings are unchanged: about 400 MB for the high setting's height
    cache (Design note 15) inside the 2–3 GB discrete ceiling. Only the context changes: the RTX
    3080's 10 GiB is shared with the local LLM.
- **Class-map discs and R07's cell pass** (R07.T8.d, 2026-10-06). R07's `disc cells` pass binds
  one class map a dispatch, so a class-map disc under 32 px sums its own cells in its draws, the
  serial chain T8.d removes from uniform discs (about 2.0 M cycles under 4 px). T10.d is to bring
  such discs into the pass: the frame's maps as one array texture with each record's layer in its
  record, or one dispatch a map. Open, for the orchestrator (R07's "Deviations in T8.d, as built").
  _Closed by R07.T8.e (`decision-r08-t11-frame.md` §2): one dispatch a set of bound textures, a
  class map with its normal map among them, so a class map's disc takes the pass (R07's
  "Deviations in R07.T8.e and R07.T8.f, as built")._
- **Composition, for the re-validation (decision-composition, 2026-10-09).** _Folded in at the
  re-validation of `c6d77cac`: Provides' `material`, Design notes 1 and 8, T1.a, T1.b, the new
  T1.c, T10.a, T10.b and T10.d; two questions it leaves are in that record._
  - `MaterialClass` stays the form, and `Material` becomes `{ class, entry }`, the entry being
    R09's palette index. `FrostSpecies` goes.
  - `properties(material, &palette)` takes the class's table with the entry's resolved A_N in B,
    V and R, phase row, grain density (the form's porosity by depth giving each layer's bulk),
    transition and mechanics family (science-r09-drainage.md §3).
  - The classifier reads the cell's substance byte and the header: frost or glacier of the ice
    entry, `Seabed` under the liquid entry, `Melt` above the crust's transition by R09's cell
    temperature, and regolith, sand and dust of the crust, `Deposit` or `Cover` entry.
  - R07's class map indexes the body's (form, entry) pairs, at most 16.
  - R10.T10.a solves the scale per band, or states one c with the residual per band.
  - R10.T10.b adds the ground's thermal emission through R08.T18's function.
  - New **R10.T1.c, maturity and cover**: regolith's A_N and colour modulated by exposure (crater
    and surface ages) and by plan 14's T51.d cover, with tests that a fresh crater's ejecta is
    brighter than mature regolith by the Moon's measured contrast, and that zero cover gives the
    substance's own albedo.
  - The Pluto-like and lava-world `FieldBuilder` worlds join T1.b's tests.
- **Re-validated at `c6d77cac`** (2026-10-13, lane r10-revalidate, the `revalidate-plan` skill),
  against R05, R07, R08's T9.a (`5264a413`) and R09 as built: R09.T1.a, T2 with its follow-ups A–D,
  T3, T4, T5 with its two follow-ups, T6.a, T6.b, T7.a–f, T10.a–d, T17, T18 and T19.a, with their
  "as built" records (unchanged at `888c6f8b`, whose two commits touch none of it). No brainstorm
  drift on this plan's topics since the plans were last brought in line with it
  (`git diff 3138e393 -- docs/agent/brainstorming/rendering-and-planets.md`: the wet
  epoch's years, the base spline's name, the crater map's and Venus's taper's figures, R08's
  adiabat and channels, and the sky's counts, none of them this plan's). What changed here:
  - _Names, paths and signatures_ (Provides, Consumes, the tasks). R09's palette entry's members
    (`substance`, `normal_albedo_bvr`, `phase_row`, `grain_density_kg_m3`, `transition`,
    `mechanics`), the header's getters, `decode_block` and the block's frame, `PartialField`'s
    margin code, `UnresolvedVariance`'s getters and the per-contribution structure functions,
    `coarse_pass(seed, &inputs)`, and R09.T19.a's wire types; R05's `HeightSource` as built, the
    triangle query T3 adds, selection's level table and chord sagitta, `slotLayout.ts`'s fields,
    the pool, worker and spike, and the settings' `cacheBytes` (no second `PATCH_CACHE_BYTES`);
    R07's `DiscSurface` file, `phase_factor_table`'s channels, `viewFrameDrawer.ts`, the mesh
    regime's files and `depthWriters`; R08's `curve` argument and `atmosphere_source_transmittance`,
    the aerial perspective's two paths, `preciseAtan2`'s range (T8.b's elevation now takes the
    zenith angle) and the geometric sun; R03's `requestBulk`, `FakeWebSocket` and the scene model;
    R02's label block and DOM list, and the client's body-fixed types; plan 14's rotation and
    photometry on the wire.
  - _Folded in, as their rulings direct:_ decision-composition §1.8 (`Material { class, entry }`,
    no `FrostSpecies`, `properties(material, &palette)`, the classifier's palette rules, the class
    map's (form, entry) pairs, the new T1.c, T10.a's bands and T10.b's thermal emission); R09's
    cold-trap rule for the classifier (Design note 1, T1.b); T1.a's codes for R09's `phase_row`
    (R09's "Deviations in T2", follow-up B); and the slope's projection into the tangent plane
    (R09's "Deviations in T4, as built").
  - _Commands._ The surface crate's integration tests and the sim's take `--features testing`;
    the wasm goldens run under `just test-wasm-fast`; T6.a's and T8.a's benches are named
    (`just bench -- bake/patch`, `bake/horizon`, in a new `benches/bake.rs`); T6.b runs
    `just gen-surface` first; T7 adds `pnpm --filter @hyperion/protocol test`; T15's acceptance
    names the lane's checks, `just ci` and the slow pass being the orchestrator's.
  - _Order._ The Tasks' opening paragraph now gives the order as built: T7, T13 and T1.a startable;
    T1.b, T2, T3, T5 and T6 after R09.T8 (and T9 for goldens); T4.a whole, and the
    `sample_worlds()` cases of T1.b, T2, T5 and T10.a, after R09.T16, since the coarse pass follows
    this plan on the critical path; T7 building the pool's half of the per-body calls and T6.b the
    worker's.
  - _Provisional until R09.T8/T9/T19.b land_ (Consumes, R09): the full `height_at`,
    `Synthesiser::bake_patch` and `height_range_m`, `level_bound_m`, the whole `structure_function`
    and `unresolved_rms`, `QueryHeightError::NonFinite`; the golden heights; and the `surface_field`
    payload, its revisions and `surface_revisions`. Their names are R09's Provides, not guessed.
  - _Pending re-validation:_ none beyond those names. T1.c's cover term waits on P14.T51.d.
- **Questions for "main" from the re-validation** (each touches a contract, the payload R09.T19.b
  is about to send for the first time, so each is cheapest ruled before that send):
  - _A form's albedo on one entry._ Snow and glacier ice are two forms of one H₂O entry, and
    `VolatileFrost` a frost grain of its ice's, but R09's `PaletteEntry` carries one A_N a band, at
    the one grain class the server resolves (decision-composition: "A_N in B, V and R at grain
    classes"). Either the entry carries A_N per grain class (a payload member), or the class table
    holds a per-form factor on the entry's A_N. Lean: the per-form factor, in T1.a's table under its
    generator version, from Design note 8's spans; no wire change.
  - _The albedo scale by band._ decision-composition §1.8 lets T10.a solve per band or keep one c
    with the residual per band stated; the header holds one `albedo_scale: Option<f64>`, and per
    band would be three. Lean: one c, fitted in V, with the B and R residuals stated; no wire
    change.
  - _The `sample_worlds()` cases as follow-ups._ T1.b, T2, T5 and T10.a land their synthetic-world
    parts first and their reference-world binaries after R09.T16, under the same task IDs, as R09's
    T5 had follow-ups. Lean: so; the alternative is new subtask letters.
  - _Answered by "main", 2026-10-13._ The first: a per-form factor in T1.a's class table, no wire
    change (built: "Deviations in T1.a, as built"). The second: one `albedo_scale`, fitted in V,
    with the B and R residuals stated and recorded (T10.a). The third: the `sample_worlds()` tests
    land later as commits under the same task IDs, not new subtask letters.
- **Deferred findings (2026-10-13 rule)** (the re-validation of `c6d77cac`; each leaves this plan's
  algorithm and contracts as they are, for "main"'s list).
  - _The bound's wording._ Main's acceptance for R09.T8 makes `level_bound_m` a 1 − 10⁻⁹ per-point
    quantile in its crater and rim terms, while R05's `HeightSource::level_bound_m` is documented
    as "the level's hard error bound", and T3 implements that trait over it. Lean: T3 rewords the
    trait's doc comment to the stated bound in its own commit (R05's file; no code moves). Design
    note 5, T4.a and T4.b carry a pointer here.
  - _Channelled plains' structure function._ R09.T8's closed form runs 15–37% high there, so T5's
    slope uncertainty and T2's cliff exceedance overstate channelled plains: the safe direction for
    a readout, but T2's ten-point check may fail on channelled cells, and the test names them. The
    fix is R09's channel follow-up.
  - _The bake's budget._ R09.T8's provisional 77–82 ms a patch at L + 4 (14 µs a point) already
    exceeds T6.a's 40 ms before the class weights; T6.a's quiet figure follows R09's
    fused-interpolant follow-up. T8.a's far field pays the same per-point cost.
  - _The cache's slots._ With R05's as-built normal gutter, a low slot is about 157–160 kB and
    64 MiB holds 420–426 slots against Design note 15's 430; with `BakedOffsets` kept on the high
    setting (R05.T18), a high slot is about 310 kB and 400 MB holds about 1,290 against 1,900.
    T14 decides, as planned.
  - _R03's client API._ `requestBulk` has no per-chunk delivery, so T7 adds an option beside
    `onPartial` in `@hyperion/protocol`: additive, and the first R10 change in R03's package.
  - _A peak's extended-source horizon._ R07's `sphere_irradiance` clamps a negative horizon at 0,
    so a peak's planetshine and wide-source term is the flat ground's; the sun's own lit fraction
    (Design note 10) keeps the signed horizon. Small; R07's file. _For the sun, settled by T8.b
    (decision-r10-t8b §1): in the element's frame the clamp takes the element's own plane exactly
    where it hides more, and a slope's signed horizon is carried by the conversion. Planetshine is
    not shadowed by the map (Deferred findings (R10.T8.b))._
