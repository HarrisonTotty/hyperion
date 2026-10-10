# Plan R13: The Hybrid Sky

- **Milestone:** Rendering milestone RM7, sequenced after RM3 and beside RM4 (numbered RM7 so that
  RM4–RM6 keep their numbers). R13.T1 and T1.b run before RM3 closes, and R13.T2 is RM3's interim
  (Design note 15).
- **Depends on:** [R06 The sky](06-the-sky.md) as built: its luminosity tables (T5), caps by
  direction (T7.b), shell plan and texel listing (T8.i), the band's march and the eye's own sky
  (T9.f, T9.j), the server (T11.a–c) and nearest-first delivery (R06.T11.d); [R07 Lit bodies,
  the photorealistic style, several views and the main screen](07-lit-bodies-styles-and-main-screen.md)
  for the label block's `STARS` reading and the view's list notes. Through R06 it reads galaxy
  plans 03 (cells, bounds, streams), 06 (tracks and photometry), 07 (extinction), 11 (pair
  evolution, through R06.T5.d's fit) and 12 (the retarded time). R13.T9's gate waits on the
  correction of R06.T5.f's count findings (`deferred-corrections.md`, "Galaxy physics and
  calibration").
- **Brainstorm sections covered** (by heading, in
  [the brainstorm](../../brainstorming/rendering-and-planets.md)): "The sky", its subsection "The
  star field is the galaxy, not a photograph" (the census, the band, "a statistical layer of
  unresolved stars, labelled as such") and the subsection "The hybrid sky: the census near,
  synthetic stars far" (signed off by the owner on 2026-10-08, its label wording still a draft;
  its amendment for the real limit, with R13.T1's and T1.b's measurements, signed off 2026-10-09
  by the sign-off agent under the owner's delegation, `signoff-brainstorm.md`); open questions 13,
  16 and 19 as they touch the census's reach and cost. The decision it answers is the owner's of
  2026-10-08 on the full sky's time (`decision-p11-t17c-bright.md` §4), and its feasibility study
  is `.git/rm23-orchestration/feasibility-hybrid-sky.md`. The real limit is the owner's of
  2026-10-09, after R13.T1's measurement (`decision-r13-guard-trip.md`).

## Goal

When this plan is done, the sky's census is exact only where exactness matters and affordable: out
to a real boundary per layer and direction: the lesser of a fixed real limit, 2,000 ly, and the
radius beyond which under one star brighter than a fixed ceiling V_P is expected (the limit decided
by the owner on 2026-10-09, `decision-r13-guard-trip.md`), every star is the galaxy's own, as R06
lists it today. Beyond it every star brighter than the request's cut is **synthetic**: drawn by the
server in fixed cells of the galaxy, by exact thinning of the generated galaxy's own density,
luminosity functions and dust, the same tables the band and the caps read, so that they are correct
in expectation, consistent with the band texel by texel, the same for every client, run, machine and
observer, stable as the camera moves and true in parallax. The band holds only the remainder. The
view draws synthetic stars as it draws listed ones and says that they are synthetic; they cannot be
selected, targeted or counted, and on approach the real census takes their region over. Near the Sun
the full sky then costs about 1.1–1.3 × 10⁴ CPU-s on the test fixture and 2.5–2.8 × 10⁴ on the
server's galaxy (R13.T1.b, measured at V_P 4.5 and 5.0). That compares with 1.0–1.5 × 10⁵ at the
ceiling's caps alone (R13.T1) and 0.4–1.0 × 10⁶ for the exact census. A camera's sky costs what the
eye's does.

## Scope and non-goals

In scope:

- The measurement of the real boundary from the caps alone, and a sampled bench of the real tier
  (T1); the same for the boundary held at the real limit, with the bright stars beyond it (T1.b).
- The real tier: each layer C–E capped at T7.b's caps computed at the ceiling and at the real
  limit, listed by the band texel's radius at every reply, with the T7.b gap closed (T2); RM3's
  interim is this task with the band for the rest.
- A colour–magnitude table in R06.T5's quadrature, and the light-consistent counts the synthetic
  tier draws from (T4).
- The march's ray profiles, kept for the synthetic tier (T3); the band needs no sums at the ceiling
  (Design note 9).
- `hyperion_sim::sky::synthetic`: the fixed cells, the magnitude-ordered thinning, the photometry
  and the walk (T5).
- Merge, N_max, overflow, glare, eye offsets and conservation with synthetic stars (T6).
- The protocol's fields, the server's jobs and every reply carrying the synthetic set (T7).
- The client's label note and list note, and their guide drafts for the owner (T8).
- The verification against the real census of this galaxy (T9).

Non-goals:

- Any change to the generator or to what a real system is. The sky only reads; no
  `GENERATOR_VERSION` bump.
- Sampling Milky Way catalogues or observed star counts. The synthetic stars sample the generated
  galaxy alone (Design note 1).
- Making a synthetic star a real system ("lazy realisation by record"): revisited after the
  deferred census levers land (the owner, 2026-10-08; Risks).
- Feature members (clusters, associations, the nuclear cluster). They wait on R06.T16.a; until
  then the field keeps their stars, and so does the synthetic tier.
- Blending a system's stars before the views' cull, in either tier (R06's, recorded in Risks).
- The census's own cost levers (`deferred-corrections.md`, "Census cost"), and the first sky's
  budget, which stays R06.T11.d's.
- Sensor consoles and charts: they read range queries and catalogues, never the sky's list.

## Provides

Rust paths are under `hyperion_sim` unless a crate is named. Signatures are sketches.

### `sky::synthetic` (new)

```rust
pub const SYNTHETIC_CEILING_V: f64;      // V_P: 4.5 (the owner, 2026-10-08); 5.0 in RM3's interim
pub const SYNTHETIC_LAYERS: [Layer; 3];  // C, D, E
pub const SYNTHETIC_CELL_LY: u32;        // the fixed grid's edge, 256–1,024 ly (R13.T5.b's bench)
pub struct SyntheticCellKey;             // a layer and the cell's coordinates; `cell_box()`, `word()`
pub struct SyntheticStar { /* key: SyntheticCellKey, index: u32 (its candidate number),
    apparent: GalacticPosition, distance: LightYears, emitted: UniverseTime, absolute_v: Magnitudes,
    v: Magnitudes, a_v: Magnitudes, colour: StarColour */ }
pub struct SyntheticPlan;                // the cells within the cut's caps, canonical order, as slabs
pub fn synthetic_plan(query: &SkyQuery, caps_at_cut: &[LayerCap]) -> SyntheticPlan;
impl SyntheticPlan { pub fn slabs(&self) -> impl Iterator<Item = SyntheticSlab>; pub fn len(&self) -> u64; }
pub fn synthetic_slab(galaxy: &Galaxy, tables: &LuminosityTables, profiles: &RayProfiles,
    query: &SkyQuery, boundary: &CompleteTo, slab: &SyntheticSlab,
    out: &mut Vec<SyntheticStar>) -> SyntheticTally;
pub struct SyntheticTally { /* per layer: cells, candidates, accepted, kept */ }
```

### In R06's modules

```rust
// sky::caps (R13.T1, built; Risks, "Deviations in T1, as built")
impl RayRadii { pub fn lesser(&self, other: &Self) -> Self; }   // each ray the lesser of the two
impl LayerCap { pub fn forced_by_ray(layer: Layer, rays: RayRadii) -> Self; }   // now public
impl CapCount {                          // the counts towards a boundary given ray by ray
    pub fn stars_beyond_toward(&self, layer: Layer,
        radius_toward: impl Fn(UnitVector) -> LightYears) -> f64;
    pub fn systems_within_toward(&self, layer: Layer,
        radius_toward: impl Fn(UnitVector) -> LightYears) -> f64;
    pub fn stars_within_and_beyond_toward(&self, layer: Layer,
        inside: impl Fn(UnitVector) -> LightYears, outside: impl Fn(UnitVector) -> LightYears) -> f64;
}
// sky::caps (R13.T1.b; test-only, `#[cfg(any(test, feature = "testing"))]`; built)
impl CapCount { pub fn cap_with_budget(&self, layer: Layer, budget: f64) -> LayerCap; }
    // `cap`'s rule with `ray_extents`' budget for the count beyond (`cap` takes 1.0)
// sky::caps (R13.T1.b, built; was crate-visible)
impl RayRadii { pub fn within(&self, edge_ly: f64) -> Self; }   // each ray held within an edge
// sky::caps (R13.T2.a, built; Risks, "Deviations in T2.a, as built")
pub const REAL_LIMIT_LY: f64 = 2_000.0;  // the owner, 2026-10-09; one of SHELL_EDGES_LY, which a test holds
pub fn real_boundary(at_ceiling: &CapCount, at_cut: &CapCount, caps_at_cut: &[LayerCap])
    -> Vec<LayerCap>;
    // C–E: each ray the least of the ceiling's cap, the cut's and `REAL_LIMIT_LY`, stating its
    // `expected_beyond` at the cut and its `bright_beyond`; A, B and the brown dwarfs: the cut's
    // caps, stating their `bright_beyond`. The plan at the query's ceiling widens C–E's cones by ρ
impl LayerCap { pub fn bright_beyond(&self) -> Option<f64>; }   // a real boundary's, else None
// sky::caps (R13.T2.b; decision-r13-t2b-ceiling)
pub const SYNTHETIC_CEILING_V: f64 = 5.0;   // R13.T2.b, built here until T5.a's sky::synthetic takes it
pub fn served_ceiling(cut: Magnitudes, ceiling: Magnitudes, caps_at_cut: &[LayerCap])
    -> Option<Magnitudes>;
    // `ceiling` where the cut is deeper; else the cut, unless C's, D's and E's caps at the cut
    // lie within REAL_LIMIT_LY on every ray, where a ceiling would hold nothing: None
// sky::census (R13.T2.a, built)
impl SkyQueryBuilder { pub fn synthetic_ceiling(self, v: Magnitudes) -> Self; }
    // refused (`BuildSkyQueryError::SyntheticCeiling`) unless finite and no fainter than the
    // cut (R13.T2.b; T2.a built it 0.5 mag brighter)
impl SkyQuery { pub fn synthetic_ceiling(&self) -> Option<Magnitudes>; }
impl SkyQuery { pub fn with_synthetic_ceiling(self, v: Magnitudes) -> Result<Self, BuildSkyQueryError>; }
    // R13.T2.b: the builder's rule on a query built, for the server's served ceiling
impl Completeness { pub fn lists_by_texel(&self) -> bool; }   // a layer not yet final, or C–E at a ceiling
// sky::census (R13.T6)
impl SkyCensus { pub fn synthetic(&self) -> &[SyntheticStar]; }   // by flux, then key, then index
pub fn merge_shells(parts, synthetic: Vec<SyntheticStar>, n_max: NonZeroU32,
    completeness: Completeness) -> SkyCensus;                      // N_max over both kinds
// sky::band (R13.T3)
pub struct RayProfiles;                  // each texel ray's node distances and A_V (f32), from the march
impl BandMarch { pub fn profiles(&self) -> Option<&RayProfiles>; }   // kept when a ceiling is stated
impl RayProfiles { pub fn a_v_toward(&self, spec: &BandSpec, direction: UnitVector,
    distance: LightYears) -> Magnitudes; }  // the texel `texel_of` gives, linear between nodes
// sky::luminosity (R13.T4)
pub const CMD_MAGNITUDE_STEP: f64;       // 0.25 brighter than CMD_FINE_LIMIT, 1.0 fainter
pub const CMD_FINE_LIMIT: f64;           // M_V +5.0: near the Sun no synthetic star is fainter (Design note 7)
pub struct StarClass { /* teff: Kelvin, log_g: f64, grid: AtmosphereGrid */ }
impl LuminosityFunction {
    pub fn synthetic_count_brighter_than(&self, m_v: Magnitudes, emitted_ago: Span) -> f64;
        // light-consistent counts: single-star counts × each 1-mag bin's corrected ÷ single light
    pub fn synthetic_count_bound(&self, m_v: Magnitudes) -> f64;   // over every snapshot, for thinning
    pub fn draw_class(&self, m_v: Magnitudes, emitted_ago: Span, u: f64) -> Option<StarClass>;
}
```

### Protocol (`hyperion_protocol::sky`, mirrored in `@hyperion/protocol`)

- `SkyResponse` gains `synthetic: u32` (the synthetic stars, which follow the `listed` ones in the
  bulk payload as 24-byte records of R06 Design note 17, so `stars_bytes` is 24 × (listed +
  synthetic)) and `synthetic_ceiling_v: Option<f64>` (the ceiling its real boundary is at: V_P,
  or the cut where the cut is brighter; absent where the reply has no real boundary at a ceiling,
  as where a ceiling would hold no ray, R13.T2.b). Each `SkyLayerCensusDto` gains its real
  boundary, in the per-ray form R06.T11.d gives `complete_to_ly`, and its `expected_beyond` is
  stated at the cut beyond it; at a ceiling `cap_ly` and the final reply's `complete_to_rays_ly`
  are that boundary, a reply not final states it on each ray below its edge, and no second table
  is sent (`decision-r13-t2b-note.md` §5).
- `SkyResponse` also gains `real_limit_ly` (`REAL_LIMIT_LY`), and each `SkyLayerCensusDto`
  `bright_beyond`, the expected count brighter than V_P beyond its real boundary; both are absent
  where the reply states no ceiling (R13.T2).
- No new kind, no byte of the star or texel records changes, and `PROTOCOL_VERSION` stays (an
  additive field, R03 Design note 12).

### Client (`apps/hyperion/src/renderer/src/view/sky/`)

`SkyModel.synthetic` (the index range of the synthetic stars), drawn, baked, culled and spritten as
listed stars; `skyLabelValue(limitV, kind, gaps, notes)` composing the `STARS` line's notes in the
order `STREAMING`, then the synthetic or the interim note, then `NOT YET MODELLED`; while an
instrument is open, the primary's line holds its limit and the first of them that holds, and an
instrument's its limit and `NOT YET MODELLED` (decision-r06-t11f-stars-line); the view list note
`SYNTHETIC STARS` beside `INTEGRATED STARLIGHT`, in both styles.

### Test helpers

`crates/hyperion-sim/tests/sky_hybrid.rs`: the boundary record (T1), the capped boundary's record
(T1.b), the census-against-synthetic comparison (T9). A test-only salt (`SyntheticSalt`,
`#[cfg(any(test, feature = "testing"))]`) that re-keys the synthetic stream, for ensembles of
independent realisations.

## Consumes

Names are as built at `rendering-and-planets` b6cb51b1 unless marked; R13.T1 re-checks each against
the tree before building. **Re-checked by R13.T1 at cb724aca (2026-10-08) and again at 090d880d
(2026-10-09):** every name below is in the tree as written, with the corrections marked "(R13.T1)".

- **R06** (`06-the-sky.md`):
  - `sky::luminosity::{LuminosityTables, LuminosityFunction, TablesPlan, MAGNITUDE_STEP,
EMITTED_AGO_YEARS}`, `get_at`, `age_for`, `count_brighter_than`, `light_fainter_than`,
    `colour_sums_fainter_than` (crate-visible) and `pair_light`; R06.T5.d's per-1-mag-bin pair
    correction inside `TablesPlan::assemble`'s finish step.
  - `sky::caps::{CapCount, CapResolution, LayerCap, RayRadii, RayExtinctions, layer_caps_over,
CAPPED_LAYERS}`: `CapCount::measure` at any cut, `caps`, `spheres`, `stars_beyond`,
    `systems_within`, `stars_within_and_beyond`; R06.T11.g's count in jobs, `CapCount::plan_over`
    and `CapCountPlan::{count_rays, join}`, which T2.b's second count takes (R13.T1). What R13.T1
    added to `sky::caps` is under Provides ("sky::caps (R13.T1)").
  - `sky::census::{SkyQuery, SkyQueryBuilder, CensusPlan, Completeness, census_plan_of,
merge_shells, SkyCensus, SkyStar}` and T8.i's listing by the band texel's radius
    (`Completeness`, `BandSpec::texel_of`); `brute_force_sky` (`tests/common/sky.rs`) and
    `SkyQuery::{with_caps_forced, with_caps_forced_per_layer}` (`sky/census/query.rs`; R13.T1).
  - `sky::band::{BandSpec, BandMarch, march_rows, sum_rows, CompleteTo}`,
    `BandSpec::largest_texel_radius` (a method, not a free item; R13.T1) and T9.j's eye light;
    `sky::limits::{Glare, eye_offsets, eye_cut}`; `sky::colour::{star_colour, StarColour,
AtmosphereGrid, surface_gravity}` and `StarColour::reddened`.
  - The server's sky pipeline (`crates/hyperion-server/src/{requests,compute}/sky.rs`,
    `bulk/sky.rs`), T11.c's switch (`--serve-sky`, `HYPERION_SERVE_SKY`, off by default; R13.T1),
    and **R06.T11.d**, built at 497a2a9d (R13.T1): shells as bulk jobs
    (`compute::sky::delivery_steps`), `SkyLayerCensusDto::{complete_to_ly, complete_to_rays_ly,
is_final}` and `SkyResponse::is_final`, both `final` on the wire, with R06.T11.g's inner shell
    edges at 125 and 250 ly (`SHELL_EDGES_LY`) at 090d880d.
  - The decision records the plan rests on: `decision-r06-census-cost.md`,
    `decision-r06-census-cost-signoff.md`, `decision-r06-t8i-listing.md`,
    `decision-r06-t8h-warm.md`, `decision-p11-t17c-bright.md`.
- **R07:** the label block's `STARS` reading (`view/sky/viewSky.ts`'s `viewSkyLabelV`, not
  `viewSkyLabel`, and `view/sky/label.ts`'s `skyLabelValue(limitV, limitKind, gaps, standing)`,
  whose notes `skyLineNotes(gaps, standing)` composes since R06.T11.f; R13.T1) and
  the view's notes under its label block (`displays/view/viewRun.ts`'s `photorealStatements`,
  beside `displays/view/useViewSky.ts` and `displays/view/useInstruments.ts`). `INTEGRATED
STARLIGHT` is R06 Design note 23's note and the guide's row; no client code composes it at
  cb724aca nor at 090d880d (R13.T1), so R13.T8 composes `SYNTHETIC STARS` beside it where R06's
  owner composes it.
- **Galaxy plan 03:** `galaxy::bounds::CellBox` and `Component::bound` (the per-component density
  bound on a cell, nearest corner and arm phase), the layer shares (`ShareMatrix`), and the rule
  that no cell straddles an axis plane.
- **Galaxy plan 01** (through `hyperion-base`): `rng::Stream::open(seed, tag, ObjectKey)`, its
  uniform and exponential draws, and `rng/tags.rs`'s registry, which gains one tag.
- **Galaxy plans 06, 07, 11, 12:** through R06's tables, colour table and band only. Nothing here
  builds a track, a sightline or a pair.
- **The tables lane:** the correction of R06.T5.f's single-star count findings (C 0.966 ± 0.004,
  E 1.164 ± 0.034, realised over tabulated), for R13.T9's gate.

## Design notes

1. **The generated galaxy is the source and the judge** (the owner, 2026-10-08). The synthetic
   stars sample the generated galaxy's own density components, layer shares, star formation history
   and age distributions, luminosity functions and dust: the models and tables the generator, the
   census and the band already use. No Milky Way catalogue or observed star count is sampled. The
   test of correctness is consistency with what this galaxy's real census would list (R13.T9).
   Milky Way figures in this plan (Hipparcos counts, Earth's bright stars) are plausibility checks
   only, never targets to tune to. The method has precedent in population synthesis, which builds
   synthetic star catalogues from a galaxy model's densities and isochrones: the Besançon model
   (Robin et al. 2003, A&A 409, 523), TRILEGAL (Girardi et al. 2005, A&A 436, 895) and Galaxia
   (Sharma et al. 2011, ApJ 730, 3); here the model is the generated galaxy itself.
2. **Three tiers, one boundary per texel.** For each band texel and layer C–E, with R(u) the
   layer's real boundary towards the texel's centre (Design note 3):
   - **real:** stars nearer than R(u), to the request's cut, from the census, as R06 lists them;
   - **synthetic:** stars at or beyond R(u) brighter than the cut, drawn by Design note 5;
   - **band:** the rest, in expectation: everything fainter than the cut, and while a sky streams
     the pending real region (Design note 11).

   A star's tier is decided by the texel `BandSpec::texel_of` places it in (R06.T8.l's lookup and
   T8.i's listing rule), so the three share one boundary texel by texel and each star's light is
   added once (R06 Design note 11). A, B and the brown dwarfs are wholly real: their caps are
   11–260 ly near the Sun at the eye's and the camera's cuts.

3. **The real boundary is the lesser of T7.b's cap at the ceiling and a fixed real limit.** For
   layers C–E, R(u) is, ray by ray, the least of three radii: the cap at V_P, which
   `CapCount::measure` gives at cut V_P instead of the request's cut, widened as T7.b widens (R06
   Design note 9's criterion applied at the ceiling); the cut's own cap; and `REAL_LIMIT_LY`,
   2,000 ly, a fixed shell edge (`SHELL_EDGES_LY`) and twice the first drive's 1,000 ly range
   (decided by the owner on 2026-10-09, `decision-r13-guard-trip.md`, after R13.T1 measured the cap
   at V_P alone at 8.8–22.6 times its estimate; Design note 4). So **every star nearer than R(u) is
   the galaxy's own**: every star brighter than V_P within 2,000 ly, and near the Sun every star
   within the drive's range on at least nine rays in ten (R(u)'s 10th percentiles there are
   1,146–1,597 ly on both galaxies). Beyond 2,000 ly, stars brighter than V_P are synthetic too.
   Near the Sun that is 75 of the sky's 880 brighter than V 4.5, 72 of them E's, the brightest
   expected about V 2.1 (R13.T1.b, measured; 0.09 brighter than V 1). Each reply states the count
   (`bright_beyond`). The owner kept the limit on 2026-10-09 after R13.T1.b's guard tripped on that
   count (`decision-r13-t1b-guard.md`, option A). Where the cap at V_P lies within the limit, under
   one star brighter than V_P a layer is expected beyond it, as before. The limit is one constant,
   lifted to the cap at V_P once the census levers make that affordable (Risks). One radius, not
   two cuts: a real tier complete to V_P beyond R(u) would cost about what the full cut costs,
   since at version 21 the census generates `Bright` pairs whatever its cut (E's `Bright` median is
   M_V −5.0; at 2 kly a cut of 5 instead of 8 lowers the listable share of `Bright` pairs from about
   0.85 to 0.55, read from the quantiles of `decision-p11-t17c-bright.md` §2.2), and the caps at
   V_P are where such a census ends anyway.
   - **The listing is by the band texel's radius at every reply**, the final one included (T8.i's
     rule (b), `decision-r06-t8i-listing.md`). With R(u) at the ceiling thousands of stars lie just
     beyond it, so the one-shot rule ("every star of the cells it opens") would count them twice,
     against the synthetic tier.
   - **No gap.** T7.b's gap, a star within R(u_T) of its texel but in a cell the cones about its own
     direction left closed, is bounded today by the count beyond the caps, under one star. Beyond a
     ceiling's cap that bound is gone (_estimate_: ten to a few tens of stars near the Sun). R13
     closes it exactly by fix (i) of that record: each ray's cone widened by ρ, the band's largest
     texel radius. Ruled 2026-10-09 (`decision-r13-guard-trip.md` §1). Near the Sun it costs
     0.6–8.5% more systems (E 6.2%) and closes the gap to 0 at every point, cut and ceiling R13.T1
     measured. Spherical boundaries are neither exact (their gap reaches 0.31 stars, and in the
     nuclear disc at V<sub>P</sub> 5.5 they leave more than one star brighter than V<sub>P</sub>
     beyond) nor cheaper by cost (1.02–1.70 times). On rays held at `REAL_LIMIT_LY` the radii are
     uniform, so the gap there is zero by T8.i's rule (`all_reach`); fix (i) still widens every
     cone, at no cost where the neighbours share the limit.
4. **The ceiling V_P** is a server constant, `SYNTHETIC_CEILING_V`, stated in every reply that
   has a real boundary, as the request's cut where that is brighter (R13.T2.b,
   `decision-r13-t2b-ceiling.md`). It is
   not a client setting: a bridge shares one sky and every ship takes the same rule. With the real
   limit (Design note 3) it sets the boundary wherever the cap at V_P lies within 2,000 ly, and the
   synthetic share. R13.T1 measured the cap at V_P near the Sun at the eye's cut, by ray on the
   fixture, and the real tier's cost without the limit (Risks, "Deviations in T1, as built"; the
   costs have no fix (i), which adds about 6%). R13.T1.b measured the costs at the limit near the
   Sun, by the job threads' CPU time with T1's fix (i) factors, at 0.85–1.10 times
   `decision-r13-guard-trip.md` §2.3's estimates:

   | V_P | Cap at V_P by ray, median (10–90%): C / D / E (ly)              | Without the limit, CPU-s: fixture / server's galaxy | At 2,000 ly, CPU-s (R13.T1.b): fixture / server's galaxy | Stars brighter than V 6.5 beyond the cap at V_P |
   | --- | --------------------------------------------------------------- | --------------------------------------------------- | -------------------------------------------------------- | ----------------------------------------------- |
   | 4.0 | 1,038 (944–1,038) / 1,519 (1,141–2,222) / 2,443 (1,255–5,753)   | ~6.3 × 10⁴ (_estimate_) / —                         | —                                                        | 4.85%                                           |
   | 4.5 | 1,416 (1,287–1,416) / 2,075 (1,287–2,763) / 2,511 (1,416–7,177) | 9.49 × 10⁴ / 1.02 × 10⁵                             | 1.10 × 10⁴ / 2.48 × 10⁴                                  | 2.32%                                           |
   | 5.0 | 1,758 (1,758–1,934) / 2,343 (1,597–3,782) / 2,838 (1,597–8,139) | 1.45 × 10⁵ / 1.43 × 10⁵                             | 1.31 × 10⁴ / 2.82 × 10⁴                                  | 0.71%                                           |

   At 2,000 ly, 12.0% of the stars brighter than V 6.5 near the Sun lie beyond the boundary at
   V_P 4.5, and 11.4% at 5.0. Among them are 75 brighter than V 4.5, against the record's estimate
   of 10–60 (R13.T1.b).

   **The guard tripped.** The feasibility study had estimated the real tier at 4.5–5.8 × 10³ CPU-s
   at 4.5 and 1.0–1.25 × 10⁴ at 5.0, with spherical boundaries. R13.T1 measured 8.8–22.6 times
   that, at both ceilings, for the eye and the camera, on both galaxies. E's rays reach
   2,500–9,500 ly out of the plane, and at version 21 their cost is generation, which no cut
   cheapens (`decision-r13-guard-trip.md` §2). **The owner answered on 2026-10-09 with the real
   limit** (option A of that record, as recommended), for RM3's interim and for R13 alike. At the
   limit the real tier is 2.5–8 times R06.T17's ruled 4,000 CPU-s target, recorded, not gated.
   R13.T1.b measured it. The cost held its guard (0.85–1.10 times the estimate), and so did the
   brightest star beyond (about V 2.1). But 75 stars brighter than V 4.5 lie beyond 2,000 ly near
   the Sun, against the guard's 60. **The owner kept the limit on 2026-10-09**
   (`decision-r13-t1b-guard.md`, option A, as recommended), with the measured figures in place of
   the estimates.

   **Decided by the owner on 2026-10-08: V_P is 4.5** (`feasibility-hybrid-sky.md` §11, question
   1, as recommended), and 5.0 in RM3's interim (Design note 15). Every star brighter than V 4.5
   within 2,000 ly stays real (near the Sun the fixture's sky holds 880 brighter than V 4.5, the
   constellation-forming ones, against some 900 in a Sun-like sky, a plausibility check from
   Hipparcos's 1,608 to V 5 at 0.49 dex a magnitude; 75 of them lie beyond the limit, R13.T1.b),
   and 88.0% of naked-eye stars stay real near the Sun (88.6% at RM3's 5.0, and 97.7% at the cap at
   V_P alone; R13.T1.b and R13.T1, measured). The boundary depends on V_P and the limit, not on the
   cut, so the eye and every camera share one partition, and one census serves both.

5. **World-anchored synthetic cells.** The synthetic stars live in a fixed grid of cells of
   `SYNTHETIC_CELL_LY`, aligned to the galaxy's axes so that no cell straddles an axis plane, each
   with its own stream: tag `sky.synthetic`, keyed by the layer and the cell's word. In each cell and
   layer the stars form a Poisson process in position x, component c and absolute magnitude M with
   intensity Σ_c n_c(x) s_c,L ∂N_c,L(M; a(x)) ÷ ∂M, the density times the layer's share times the
   light-consistent luminosity function (Design note 8) at the light age a. It is drawn exactly:
   - **The bound.** Λ̂(M) = V_cell Σ_c n̂_c s_c,L N̂_c,L(<M), with n̂_c the generator's own
     component bound on the cell (`Component::bound`) and N̂ the function's count bound over every
     light-age snapshot (`synthetic_count_bound`), which bounds every interpolated age.
   - **In order of magnitude.** The candidates are the arrivals of a unit-rate Poisson process,
     Λ_k = E₁ + … + E_k with E_j exponential, mapped through Λ̂⁻¹ (Kingman 1993, _Poisson
     Processes_, the mapping theorem; Λ̂ is continuous, piecewise linear over the 0.05 mag bins),
     so the k-th is the k-th brightest under the bound.
   - **Marks and thinning.** Each candidate draws its component with probability n̂_c s_c,L
     ∂N̂_c(M) ÷ ∂Λ̂(M) at its M and a uniform position in the cell (marks, by the marking theorem),
     and is accepted with probability n_c(x) ∂N_c(M; a(x)) ÷ (n̂_c ∂N̂_c(M)), a ratio at most one
     (Lewis and Shedler 1979, Nav. Res. Logist. Q. 26, 403). Thinning needs no ordering; the
     ordering is what lets a walk stop early.
   - **Words.** Each candidate takes a fixed set of words on the cell's stream: its arrival, its
     component, its position (three), its acceptance and its colour. So candidate k is the same for
     every observer, time and cut.
   - **Truncation.** An observer stops at the first candidate fainter than the cell's limit,
     cut − DM(the cell's nearest distance), which no later candidate can beat, since A_V ≥ 0. The
     walk costs what the visible stars cost.
   - **Extent.** The cells within the cut's caps (T7.b); beyond them under one star a layer brighter
     than the cut is expected, and the band holds all light, as today.

   A sky-anchored draw (per texel and distance shell) was weighed and rejected: it has no
   parallax, gives two ships different realisations of one region, and must be keyed by an
   observer region whose edges reshuffle the whole far sky (`feasibility-hybrid-sky.md` §5).

6. **Stability as the observer moves.** A synthetic star keeps its world position, magnitude and
   colour bits for every observer that keeps it. Only its acceptance depends on the observer,
   through the light age, against a fixed word, and a star's light age moves by at most the
   observer's move in light-years, and by the time asked within ±H. A population's light changes
   by about 10⁻⁵ within ±H (`sky/luminosity.rs`'s module docs); the brightest bins of the youngest
   component may move some 10⁻⁴–10⁻³ per thousand years (an estimate from the phases' durations,
   which R13.T5.a measures), so a star almost never flips. R(u) moves with the observer:
   - within a system nothing visible changes: the boundary moves by au, and a star at 500 ly shifts
     by 0.002 px over 30 au;
   - a jump of Δ swaps about (dN/dr) Δ ÷ 2 stars at the boundary, all fainter than V_P where R(u)
     is the cap at V_P: _estimate_ some 200 for 10 ly near the Sun at the eye's cut, some 40 of
     them brighter than the eye's limit. The same jump moves a real star at 1,000 ly by up to
     0.57°, and the client swaps a sky whole by a cut, so the swaps hide in a change that is real;
   - at the limit, a jump swaps stars of every brightness, including a few brighter than V_P: a
     synthetic bright star is gone once a jump brings its region within 2,000 ly, and the real
     stars there take over (Risks).

   No hysteresis: it would make a reply depend on the ship's history, and a reply is a pure
   function of its query (R06 Design note 12).

7. **A synthetic star's photometry.**
   - **M_V** from the light-consistent counts in 0.05 mag bins (the arrival's inverse). Near the
     Sun no synthetic star is fainter than about M_V +5, since R(u) lies beyond some 300 ly there
     (DM 6.6) and the cut is never deeper than `MAX_CUT_V` (11.0). Where dense fields hold R(u)
     nearer (the bulge, the nuclear disc), fainter synthetic stars are drawn too, so the tables
     cover every magnitude.
   - **Colour** from the colour–magnitude table (Design note 8): a `StarClass` (T_eff, log g, grid)
     drawn by the candidate's colour word, then `star_colour` and `StarColour::reddened`. The bins'
     mean colour alone would not do: one M_V mixes blue dwarfs and red giants, and the camera band
     term (+0.11 at O5V, −0.70 at M2V) and the eye's colour offset are not linear in colour.
   - **Extinction** from the band's own ray profiles: the A_V of the texel `texel_of` places the
     star in, at its distance, linear between the march's nodes (`RayProfiles`). So the synthetic
     tier and the band read the same dust in every texel. The centre ray is a point sample of the
     realised dust, so over the sky the counts are unbiased; within a texel every synthetic star
     takes one A_V, so dust structure below the band's 1.4° becomes texel-sized patches of
     faint-star density (Risks).
   - **V** = M_V + DM + v★(A_V) A_V with the star's own V secant, as the census cuts a real star;
     the band subtracts at the solar point's, the sliver R06's band already carries (addendum item
     4 of `decision-r06-t9b-band.md`).
   - **Time.** Each star reads the functions at its own light age through `age_for`, as the band
     does, and is placed at its drawn position with no drift: a synthetic position is a draw, not an
     orbit.
8. **The colour–magnitude table and the light-consistent counts** (R13.T4). R06.T5's quadrature
   already visits every track sample with its state, so it can also count each sample, by its
   weight, into a cell of M_V (0.25 mag to `CMD_FINE_LIMIT`, 1 mag fainter) × log T_eff × grid,
   with the cell's count-weighted mean log g, per component bin, layer C–E and snapshot. It is
   stored as a CDF over T_eff and grid per M_V bin, within 16 MiB beside the tables' 68.8 MB (R06
   Risks, "the tables' bits"). Pair evolution enters as R06.T5.d's does: each 1-mag bin's
   single-star counts are scaled by its corrected over single-star light, so the synthetic light
   equals the band's light by construction, and pair products take the single-star colour
   distribution of their bin, the assumption T5.d's sub-bin spread already makes. These counts are
   not the shipped counts, which take only the pair-evolved increase so that the caps stay
   conservative: the synthetic tier must be unbiased, the caps conservative.
9. **The band with a ceiling** (R13.T3). When the query states a ceiling, `march_rows` keeps only
   the ray profiles, each ray's A_V at its nodes, and no third set of sums. Beyond R(u), a reply
   with synthetic stars holds the light fainter than the cut, which R06's sums already give, since
   every star brighter than the cut there is synthetic (Design note 2). RM3's reply, without
   synthetic stars, holds all the light, as R06.T9.b does. _Estimate:_ some 8 MB of profiles at
   64², and with no sums added, little of the march's time (T3 records it).
10. **A synthetic star is a listed star for every purpose but its identity.** The brightest N_max of
    both kinds are sent, by flux, a real star before a synthetic one at equal flux, then by key;
    the rest are overflow, splatted into the band as points. Synthetic stars brighter than the eye's
    cut glare (`Glare::of_listed`) and take eye offsets (`eye_offsets`); T9.j's eye light is
    unchanged. The views cull, bake and sprite them as listed stars (R06 Design note 20).
11. **Streaming.** Synthetic stars lie only beyond the final R(u), and every reply, the first
    included, carries the same synthetic set, computed once after the march. The pending real
    region of a reply not yet final is the band's, as R06.T11.d has it. So no synthetic star swaps
    while a sky arrives, and the first reply already shows the far field as points.
12. **What a synthetic star is to the player.** A drawing, like the band, labelled.
    - It cannot be selected, targeted or flown to. No star in the sky can: the view's list holds no
      stars (R06's non-goals), and jump targets are chosen on the `GALAXY` and `SYSTEM` displays or
      by designation (`single-player-experience.md`, "Jump drives"), which show real systems.
    - On approach R(u) moves out, the census lists the real stars there, and the synthetic ones are
      gone from the next reply. A synthetic star never becomes a system.
    - A tie to a real cell is only statistical: the cell's real count of bright stars is unknown
      until its systems are generated, which is the census's own cost. A tie to a real record (a
      real position and identity with a statistical brightness) would cost a walk of every far
      record, at least about 10³ CPU-s at the eye and 10⁴ at the camera, and a new fitted table
      (Risks).
    - The brainstorm's "a star the player jumps to is the star they were looking at" holds for the
      real stars only. **The owner amended it so on 2026-10-08** (the brainstorm's "The hybrid
      sky", signed off), to every star brighter than V_P and every star within R(u). With the
      real limit it holds for every star within R(u), as amended by the owner on 2026-10-09
      (`decision-r13-guard-trip.md`; the brainstorm's amendment signed off 2026-10-09 by the
      sign-off agent, owner's delegation): every star brighter than V_P within 2,000 ly, and near
      the Sun every star within one jump. The real-record variant is revisited after the deferred
      census levers land (`deferred-corrections.md`, "Census cost").
    - **Synthetic stars appear in the naked-eye view as in every camera, labelled** (decided by the
      owner on 2026-10-08, `feasibility-hybrid-sky.md` §11, question 2).
13. **What the view says** (R13.T8; the wording is a draft for the UX decision agent, by T15's
    route, and the owner signs off).
    - On the `STARS` line's composed-note slot, after `STREAMING` and before `NOT YET MODELLED`, in
      the annunciation form of `TERRAIN: STREAMING` (steady, `--text`, no status colour, neither a
      data state nor an alert). Draft wording, naming the limit's effect
      (`decision-r13-guard-trip.md` §3 A): `STARS V 7.4 mag EYE · DISTANT STARS: SYNTHETIC` (its
      subject ruled with the interim note's, `decision-r13-t2b-note.md`; its state word stays a
      draft).
    - In the view's list notes, `SYNTHETIC STARS` beside `INTEGRATED STARLIGHT`, in both styles,
      since the wireframe draws stars too: drawn singly from the galaxy's own statistics, not at any
      system's position, and never read as stars that can be selected or counted.
    - The guide's "Views" gains a third kind of star beside the two it names.
    - RM3's interim, with no synthetic stars, names what it leaves to the band in the
      `DETAIL LIMITED` form the census-cost sign-off advised for a labelled rule (its question 4):
      `STARS V 7.4 mag EYE · DISTANT STARS: DETAIL LIMITED` (ruled 2026-10-09,
      `decision-r13-t2b-note.md`), while a reply states a ceiling. It names no figure, since R(u)
      differs by layer and direction, lies at or within `REAL_LIMIT_LY`, and lies far within it in
      the nuclear disc. Its guide row names the ceiling, R(u)'s rule and the limit. The synthetic
      note keeps its subject and changes only its state word.
    - The guide rows add that in dusty directions, stars fainter than V_P already leave the real
      tier from nearer than the limit.
14. **Determinism.** The synthetic tier follows the sim-determinism skill: one new tag, its word
    layout pinned by a test, every transcendental through `math`, sums in a fixed order, cells in
    canonical order, and any split of the cells into jobs giving the same bits. It is sim code on
    the server (R06 Design note 1), so every client of a server receives the same bits, and every
    server on Linux, macOS or Windows computes them. Nothing persists: a reply is recomputed, and a
    table refit or a code change moves the synthetic sky as it moves the band.
15. **RM3's interim is R13.T2** (`feasibility-hybrid-sky.md` §10). **Decided by the owner on
    2026-10-08:** RM3 ships the real tier at V_P = 5.0 with the band for the rest (Design note 9's
    reply without synthetic stars), labelled by Design note 13's interim note, and the server's
    sky is on by default once R13.T2, with its real limit of 2,000 ly (decided 2026-10-09), and
    R06.T11.d have both landed (R06.T11.d, "The default switch and the interim"). When R13.T7
    lands, V_P moves to 4.5 and the synthetic stars fill everything brighter than the cut beyond
    R(u); the real tier's code does not change again. The owner's choice of the interim rested on
    an estimate of 11–14 minutes; R13.T1 measured the real tier at V_P 5.0 without the limit at
    1.43–1.45 × 10⁵ CPU-s, 2.7 h on 15 workers (Design note 4). Measured at the limit near the Sun
    at the eye's cut (R13.T1.b): the full sky 1.31 × 10⁴ CPU-s on the fixture and 2.82 × 10⁴ on the
    server's galaxy, 15 and 31 minutes on 15 workers (55 minutes and 2.0 h on the laptop's 4). The
    band holds 11.4% of the naked-eye stars: 1,070 brighter than V 6.5, 75 of them brighter than
    V 4.5.
16. **What it costs** (the real tier's near the Sun measured by R13.T1.b; elsewhere, and the rest,
    _estimates_ from R13.T1.b's counts and `feasibility-hybrid-sky.md` §4 and §7; T5.b and T9
    measure):
    - the real tier at V_P 4.5, at the limit: 1.10 × 10⁴ CPU-s near the Sun at the eye's cut on the
      fixture and 2.48 × 10⁴ on the server's galaxy (12 and 28 minutes on 15 workers), against
      1.00–1.07 × 10⁵ at the cap at V_P alone with fix (i) (from R13.T1) and 0.4–1.0 × 10⁶ for the
      exact census. A camera's costs what the eye's does, since the boundary does not depend on the
      cut (T1: 0.98–1.00 times). In the inner disc, about 1.6 × 10⁵ CPU-s (3 h; R13.T1.b's estimate
      at T1's costs per system, not benched);
    - the synthetic pass: about 1–5 CPU-s at 7.95 and 5–25 at 10.06;
    - the first sky: R06.T11.g's as built (6.96 s and 94.6 CPU-s to 125 ly), unchanged by the limit,
      but for a second caps count at V_P over the same rays, split as T11.g splits the first, and
      the synthetic pass, 10–40 CPU-s together;
    - warm after a jump: the real tier's warm ratio stays T8.n's (about 95–100% of cold at version
      21), so after a jump the tier costs about its cold cost again; the synthetic pass needs no
      cache.

## Tasks

T1 comes first and can run before RM3 closes; T1.b follows it (decided 2026-10-09). T2 needs T1.b,
whose guard tripped and was answered (V_P is ruled: 5.0 in RM3, 4.5 from T7; the real limit is
ruled: 2,000 ly, kept by the owner on 2026-10-09 after T1.b's measurement); it is RM3's interim and
lands before RM3 closes. T2.a needs only T1.b and R06 as built, and T2.b needs R06.T11.d. T4 needs
nothing of R13 and can run beside T1–T3.
T3 needs T2.a. T5.a needs T4; T5.b needs T3, T4 and T5.a. T6 needs T5.b. T7 needs T6, T2.b and
R06.T11.d. T8 needs T7. T9 needs T7 and T8, and its gate the tables lane's correction of R06.T5.f's
findings.

Rust files are under `crates/hyperion-sim/src/` unless a path says otherwise.

### R13.T1 The boundary measured

Re-check every Consumes name against the tree, correcting this plan where it differs. Then measure,
from `CapCount` alone and with no census, at the six points of `caps_converge_in_rays`, at the eye's
cut as R06.T9.d computes it, at 7.95 and at 10.06, for V_P in {4.0, 4.5, 5.0, 5.5}:

- per layer C–E: R(u) by ray (median, 10th and 90th percentiles, largest) and as the sphere at the
  same count (`CapCount::spheres` at V_P), each against the cut's caps;
- the expected stars brighter than V 5, 6, 6.5, 7 and the cut beyond R(u), per layer: those the
  synthetic tier and the band take;
- the systems within R(u), by ray, as spheres, and by ray with cones widened by the texel radius ρ
  (fix (i), Design note 3);
- with T7.b's gap measurement (`decision-r06-t8i-listing.md` §2, "cheap measurement"): the expected
  stars between `radius_toward(u)` and the radius towards u's texel centre, both ways, at the full
  cut, by ray.

Then one sampled bench (`HYPERION_SKY_BENCH_SAMPLE`) near the Sun of the real tier at V_P 4.5 and
5.0, at the eye's cut and at 10.06, with caps from `real_boundary`'s rule built in the bench: each
layer's records, generated and listed, and the CPU-s.

Record every figure in this plan's Risks, beside the feasibility study's estimates. A decision agent
then rules, under the delegation: spheres or rays for R(u), and fix (i) or not, whichever is exact
and cheaper. V_P is the owner's ruling (Design note 4); a real-tier cost measured at more than twice
the estimate at 4.5 or 5.0 goes back to the owner before T2 builds.

Files: `crates/hyperion-sim/tests/sky_hybrid.rs` (new; the slow test
`the_hybrid_boundary_is_recorded`, which records, and asserts only that R(u) is within the cut's caps
on every ray and that the expected count brighter than V_P beyond it is under one a layer),
`crates/hyperion-sim/benches/sky.rs` (`sky/census_near_sun_ceiling`), `.config/nextest.toml` (its
slow-test entry), this plan. Acceptance: `just test-slow the_hybrid_boundary_is_recorded`,
`cargo bench -p hyperion-sim --no-run`, the bench recorded, `just ci`.

### R13.T1.b The capped boundary measured

New (decided 2026-10-09, `decision-r13-guard-trip.md` §6); after T1, before T2. Measure, from
`CapCount` alone and with no census, at the six points of `caps_converge_in_rays`, at the eye's
cut as T1 takes it, at 7.95 and at 10.06, for V_P 4.5 and 5.0 and real limits of 1,000, 2,000 and
4,000 ly, with R(u) the cap at V_P held ray by ray within the limit (`RayRadii::lesser`, Design
note 3):

- the C–E systems within R(u), by ray, with cones widened by ρ (fix (i));
- per layer, the expected stars beyond R(u) brighter than V 1, 2, 3, 4, V_P, 6.5 and the cut
  (`CapCount::stars_beyond_toward`): what `bright_beyond` states and the synthetic tier takes;
- for the record only, T7.b's rule for D and E with a budget of 3, 10 and 30 stars beyond instead
  of one (`CapCount::cap_with_budget`, `ray_extents`' `budget`): R(u), the systems within and the
  counts beyond, as above, for a later ruling on a yield-ordered budget (Risks).

Then one sampled bench (`HYPERION_SKY_BENCH_SAMPLE`) near the Sun of the real tier at the 2,000 ly
limit, at V_P 5.0 and 4.5, at the eye's cut, on the fixture and on the server's galaxy, its caps
built as T1's bench builds them (`real_boundary_rule`) and held at the limit: each layer's records,
generated and listed, and the CPU-s, scaled by T1's fix (i) factors where the bench has no widened
cones.

Record every figure in this plan's Risks, beside `decision-r13-guard-trip.md`'s estimates (§2.3,
§2.4). **The guard.** A decision agent reports to the owner before T2 builds if, near the Sun at
2,000 ly, any of these holds:

- more than 60 stars brighter than V 4.5 lie beyond R(u), C–E together, at either ceiling;
- more than 0.5 are expected brighter than V 1.0;
- the capped tier costs more than twice §2.3's estimate.

Files: `crates/hyperion-sim/tests/sky_hybrid.rs` (the slow test
`the_capped_boundary_is_recorded`, which records, and asserts only that R(u) lies within the cut's
caps and the limit on every ray, and that each count beyond it is at least the count beyond the cap
at V_P alone), `sky/caps.rs` (`CapCount::cap_with_budget`, test-only),
`crates/hyperion-sim/benches/sky.rs` (`sky/census_near_sun_limit/{eye,served_eye}_{4.5,5.0}`),
`.config/nextest.toml` (its slow-test entry), this plan. Acceptance:
`just test-slow the_capped_boundary_is_recorded`, `cargo bench -p hyperion-sim --no-run`, the bench
recorded, the guard's verdict reported to the orchestrator, `just ci`.

**Done (e098c6eb), and its guard tripped** on its first count. Near the Sun at 2,000 ly, 75.4 stars
brighter than V 4.5 lie beyond R(u) at V_P 4.5, and 74.8 at 5.0, against 60. The other two counts
held: 0.093 are brighter than V 1.0, and the cost is 0.85–1.10 times the estimate. **The owner kept
the limit on 2026-10-09** (`decision-r13-t1b-guard.md`, option A, as recommended). The measured
figures replace the estimates in Design notes 3, 4, 15 and 16. That record composes one more
boundary exactly from T1.b's rows: a reach past the limit by yield for D and E, its option C. Risks
keep it for the limit's lifting.

### R13.T2 The real tier at the ceiling (RM3's interim)

- **R13.T2.a The census.** `SkyQueryBuilder::synthetic_ceiling` (refused unless finite and brighter
  than the cut by at least 0.5 mag, naming its field), `SkyQuery::synthetic_ceiling`,
  `sky::caps::{real_boundary, REAL_LIMIT_LY}` (Design note 3, in T1's ruled form, taking the
  limit), and `census_plan_of` and the server plan taking it for C–E when a ceiling is stated:
  shells to R(u), C–E listed by the band texel's radius at every reply, fix (i)'s widened cones
  (ruled 2026-10-09; the slow test's `widened_toward` is the reference scan), each layer's
  `expected_beyond` stated at the cut beyond R(u) (`CapCount::stars_beyond` at the cut), and its
  `bright_beyond`, the expected count brighter than V_P beyond R(u). With no ceiling every bit is
  R06's. Tests:
  - with no ceiling, T8.e's identity tests, T8.i's shell tests and T9.b's conservation test pass
    unchanged;
  - with a ceiling, shells merged equal the census with caps forced to R(u) by the texel rule, bit
    for bit, and the last shell is the one-shot census under the same rule;
  - no listed C–E star lies at or beyond its texel's R(u), in any reply, nor at or beyond
    2,000 ly;
  - no gap: over a small census (an observer near the Sun, forced small radii, a high cut), every
    star `brute_force_sky` finds within its texel's R(u) is listed; with fix (i), it passes over the
    limit's uniform rays and the ceiling's varying ones;
  - R(u) is within the cut's cap and `REAL_LIMIT_LY` on every ray; where the limit holds no ray (the
    nuclear disc), the expected count brighter than V_P beyond it is under one a layer;
  - each layer's `bright_beyond` equals `CapCount`'s count brighter than V_P beyond R(u);
  - `REAL_LIMIT_LY` is one of `SHELL_EDGES_LY`;
  - the band with `CompleteTo` at R(u): listed, overflow and band light within 1% of R06's at the
    cut's caps (T9.b's conservation, at the new radii).

  Files: `sky/caps.rs`, `sky/census/{query,cell,merge}.rs`. Acceptance:
  `cargo test -p hyperion-sim sky::caps`, `cargo test -p hyperion-sim sky::census`,
  `just test-slow the_census_is_its_oracle_1000_ly_from_the_sun caps_converge_in_rays`, `just ci`.

- **R13.T2.b The server, the reply and the interim label.** The server states a ceiling on every
  request (decided 2026-10-09, `decision-r13-t2b-ceiling.md`): `SYNTHETIC_CEILING_V` (5.0 until
  R13.T7), or the request's cut where the cut is brighter, so that no served sky's C–E census
  reaches past `REAL_LIMIT_LY` at any cut. Where the cut is at or brighter than V<sub>P</sub> and
  its own C, D and E caps lie within `REAL_LIMIT_LY` on every ray, a ceiling would hold nothing,
  and the query states none (`sky::caps::served_ceiling`): the plan and reply are R06's, with no
  `synthetic_ceiling_v`, `real_limit_ly` or `bright_beyond`. Forced caps keep the ceiling the
  query states. T2.a's refusal loses its 0.5 mag margin: a ceiling is refused only if it is not
  finite or is fainter than the cut. The server computes both caps counts over one
  `RayExtinctions`, the second at the stated ceiling, each count split as R06.T11.g splits the
  first, then `real_boundary` in one job. The response states `synthetic_ceiling_v` (the ceiling
  stated) and `real_limit_ly`, and for each layer its real boundary, its `expected_beyond` at the
  cut and its `bright_beyond` (`just gen-protocol`). The client composes the interim note while a
  reply states a ceiling and carries no synthetic stars (`DISTANT STARS: DETAIL LIMITED`;
  `decision-r06-t11f-stars-line.md` §2 for its place, and `decision-r13-t2b-note.md` for its
  wording, its guide row and the guide's other edits, drafted until a sign-off agent accepts
  them). The sampled cold bench of the served sky near the Sun, at the limit, on the fixture and
  on the server's galaxy, is recorded for R06.T17's full-cold figure. Tests:
  - a sky near the Sun returns the sim's census at the ceiling bit for bit;
  - the reply states the ceiling, the limit and each layer's boundary, count beyond and
    `bright_beyond`;
  - the query states the lesser of `SYNTHETIC_CEILING_V` and its cut;
  - over tables of no star, a query at a cut of 8.0 states 5.0, and one at 4.6, whose caps lie
    within the limit, states none and is planned and answered as R06's, bit for bit, with no
    ceiling fields;
  - `the_served_ceiling_holds_the_limit_at_every_cut` (`decision-r13-t2b-ceiling.md` §8);
  - the label's composition with `STREAMING` and `NOT YET MODELLED` by
    `decision-r13-t2b-note.md` §2's table, the note keyed on `synthetic_ceiling_v` alone; and,
    hidden and recorded, the primary's `L · I` (3 lines) and `L · S` (4) within the 4 lines
    measured at 1280 × 720.

  Files: `crates/hyperion-sim/src/sky/caps.rs` (`served_ceiling` and its test),
  `crates/hyperion-sim/src/sky/census/query.rs` (the refusal), `crates/hyperion-protocol/src/sky.rs`,
  `crates/hyperion-server/src/{requests,compute}/sky.rs`, `crates/hyperion-server/tests/sky.rs`,
  `packages/protocol/src/`, generated bindings, `apps/hyperion/src/renderer/src/view/sky/label.ts`,
  `docs/frontend/ux-guidelines.md` (the draft row). Acceptance: `cargo test -p hyperion-sim sky::caps`,
  `cargo test -p hyperion-sim sky::census`, `cargo test -p hyperion-protocol sky`,
  `cargo test -p hyperion-server --test sky`, `pnpm --filter @hyperion/protocol test`,
  `pnpm --filter hyperion exec vitest run src/renderer/src/view/sky`, `just ci`.

### R13.T3 The band's kept profiles

Design note 9. `march_rows` keeps the profiles only, each ray's A_V at its nodes (`RayProfiles`,
`BandMarch::profiles`), when the query states a ceiling, with no ceiling sums; `sum_rows` takes
whether the reply carries synthetic stars, and beyond R(u) holds Design note 9's light.
`RayProfiles::a_v_toward` reads the texel `texel_of` gives, linear in distance between nodes. Tests:

- with no ceiling, T9.f's and T9.j's tests and bits are unchanged;
- each kept profile equals `profile`'s value at every node, bit for bit;
- a reply without synthetic stars equals RM3's interim band bit for bit, and one with them plus the
  expected synthetic light (the march's slots beyond R(u), all the light less the light fainter
  than the cut) equals it to rounding, texel by texel;
- any split of the rows gives the same bits.

Record the march's heap and time at 64² near the Sun with and without the ceiling. Files:
`sky/band.rs`, `benches/sky.rs` (`sky/band_near_sun/march_ceiling`). Acceptance:
`cargo test -p hyperion-sim sky::band`, `cargo bench -p hyperion-sim --no-run`, `just ci`.

### R13.T4 The colour–magnitude table

Design note 8, in R06.T5's quadrature (`TablesPlan`'s accumulation and finish step): the
colour–magnitude CDF per component bin, layer C–E and snapshot; the light-consistent counts
(`synthetic_count_brighter_than`), their bound over snapshots (`synthetic_count_bound`), and
`draw_class`. `StarClass` gives `star_colour` its arguments. Tests:

- the table summed over T_eff and grid equals the light-consistent counts per 0.25 mag bin, to
  10⁻¹² relative;
- each 1-mag bin's light-consistent count times its stars' V light equals its corrected light within
  the bin width's error (2.3%), and its single-star counts equal `count_brighter_than`'s where the
  bin takes no pair correction;
- each 1-mag bin's mean chroma and ρ drawn through `star_colour` agree with
  `colour_sums_fainter_than`'s within the T_eff binning's error (record it; 1% in chroma is the
  expectation);
- the bound is at least every snapshot's and every interpolated light age's count, at every edge;
- `parallel_build_equals_serial` holds, and every existing field's bits are unchanged
  (`standard_nodes_match_the_full_build_where_the_tables_are_read` passes as it stands);
- the heap stays within +16 MiB and the build within +10% (recorded).

Files: `sky/luminosity.rs`, `sky/colour.rs`. Acceptance: `cargo test -p hyperion-sim sky::luminosity`,
`just test-slow standard_nodes_match_the_full_build_where_the_tables_are_read
luminosity_matches_realised_cells`, `just ci`.

### R13.T5 The synthetic stars

- **R13.T5.a The cells and the thinning.** `sky::synthetic`'s grid, keys, the tag `sky.synthetic`
  in `rng/tags.rs` (and the registries' disjointness test), the bound Λ̂, the arrivals, the inverse
  by bisection over the 641 edges, the component, the position, the acceptance and the truncation
  (Design note 5), and `synthetic_plan` streaming the cells within the cut's caps as slabs.
  `SyntheticSalt` for tests. Tests:
  - **exact in expectation:** over 200 salts of a few cells near the Sun, in the bulge and in the
    halo, the mean counts per component, 0.5 mag bin and sub-volume equal the intensity's integral
    within 3σ;
  - the acceptance ratio never exceeds one over 10⁵ candidates;
  - **truncation:** a fainter limit keeps every star a brighter one kept, bit for bit;
  - **observer independence:** two observers 10 ly apart keep the same position, M_V and component
    bits for every star both accept;
  - the word layout (`synthetic_words_follow_the_layout`);
  - any split and order of the slabs gives the same bits (`order::assert_order_independent`);
  - recorded: the largest relative change of a light-consistent count bin between consecutive
    snapshots, per layer, and the stars that flip over a 10 ly move in the 200 salts (Design note
    6).

  Files: `sky/synthetic.rs` (new), `sky/mod.rs`, `rng/tags.rs`. Acceptance:
  `cargo test -p hyperion-sim sky::synthetic`, `just ci`.

- **R13.T5.b The photometry and the walk's cost.** Design note 7: the real-boundary test by texel,
  the profile's A_V, V, the window beyond R(u) (every star brighter than the cut), the class and
  colour, the emitted time, and `SyntheticStar` and `SyntheticTally`. Tests:
  - no kept star is nearer than its texel's R(u), or at or fainter than the cut;
  - its A_V is `a_v_toward`'s bit for bit, and its colour the class's `reddened` at it;
  - over 64 salts near the Sun, the mean light of the kept stars per texel equals the march's light
    beyond R(u) brighter than the cut within 3σ, and over the sky within 1%;
  - the mean counts per layer and 1-mag bin of apparent V equal `CapCount`'s expectation at the
    same boundary within 3σ.

  Bench `sky/synthetic_near_sun` at 7.95 and 10.06, and `sky/synthetic_nuclear_disc`: cells,
  candidates, accepted, kept and CPU-s, at cells of 256, 512 and 1,024 ly; `SYNTHETIC_CELL_LY` takes
  the cheapest at 10.06, recorded. Files: `sky/synthetic.rs`, `benches/sky.rs`. Acceptance:
  `cargo test -p hyperion-sim sky::synthetic`, `cargo bench -p hyperion-sim --no-run`, the benches
  recorded, `just ci`.

### R13.T6 Merge, overflow, glare and conservation

Design note 10. `SkyCensus` holds the synthetic stars; `merge_shells` takes them, with N_max over
both kinds; `sum_rows` splats both kinds' overflow; `Glare::of_listed` and `eye_offsets` take both.
Tests:

- listed, synthetic, overflow and band light together equal the interim reply's listed, overflow
  and band light within 1% near the Sun at 7.95 and 10.06, and to 10⁻³ in the mean over 64 salts;
- N_max is cut by flux across both kinds, a real star before a synthetic one at equal flux;
- every reply of a request carries the same synthetic set, and the last reply is the one-shot
  census with it;
- T9.j's `the_eyes_light_is_independent_of_the_census_radius` holds with synthetic stars;
- order independence of the merge over parts and slabs.

Files: `sky/census/{merge,mod}.rs`, `sky/band.rs`, `sky/limits.rs`. Acceptance:
`cargo test -p hyperion-sim sky::census`, `cargo test -p hyperion-sim sky::band`,
`cargo test -p hyperion-sim sky::limits`, `just ci`.

### R13.T7 The protocol and the server

The response's `synthetic`, the payload's synthetic records after the listed ones
(`encode_sky_payload`), and their decoding in `@hyperion/protocol` (`just gen-protocol`). The server
runs the synthetic plan's slabs as bulk jobs after the march, merges them into every reply
(Design note 11), and sets `SYNTHETIC_CEILING_V` to 4.5, the owner's value; the switch
`HYPERION_SKY_SYNTHETIC` (on by default, off giving T2's interim) lets T9 turn it off if it fails.
Tests:

- a sky near the Sun returns the sim's listed and synthetic stars bit for bit, and every reply the
  same synthetic set;
- `stars_bytes` is 24 × (listed + synthetic), and a truncated payload is an error naming its length;
- the switch off gives T2.b's replies bit for bit;
- the first reply's time near the Sun is recorded against R06.T17's first-sky budget.

Files: `crates/hyperion-protocol/src/sky.rs`, `crates/hyperion-server/src/{requests/sky.rs,
compute/sky.rs,bulk/sky.rs,config.rs}`, `crates/hyperion-server/tests/sky.rs`,
`packages/protocol/src/{sky,index}.ts`, generated bindings. Acceptance:
`cargo test -p hyperion-protocol sky`, `cargo test -p hyperion-server --test sky`,
`pnpm --filter @hyperion/protocol test`, `just ci`.

### R13.T8 The client and the guide drafts

`SkyModel.synthetic`; the `STARS` line's synthetic note, replacing the interim note when a reply
carries synthetic stars (Design note 13); the list note `SYNTHETIC STARS` in both styles; the guide
drafts (the "Views" third kind, the note's row and the list note's row) through the UX decision
agent, for the owner's sign-off. Tests:

- the label's composition with every combination of `STREAMING`, the synthetic or interim note and
  `NOT YET MODELLED`;
- the list note in both styles, and its absence without synthetic stars;
- a synthetic star is culled, baked and spritten as a listed star, its flux kept to 10⁻⁶ relative.

A hidden Electron check that the composed line fits the label block at 1280 × 720 and in the
instrument slots, recorded. Files: `apps/hyperion/src/renderer/src/view/sky/{model,label}.ts`,
`apps/hyperion/src/renderer/src/displays/view/{useViewSky,useInstruments}.ts`,
`docs/frontend/ux-guidelines.md`. Acceptance: `pnpm --filter hyperion exec vitest run
src/renderer/src/view/sky`, `pnpm --filter hyperion exec vitest run src/renderer/src/displays/view`,
`pnpm format:check`, `just ci`.

### R13.T9 Verification against the census

The owner's test (Design note 1), run by name and recorded:

- **`the_synthetic_tier_matches_the_census`** (slow): at R06.T5.f's eight observers on the solar
  circle, the inner-bulge point and a halo point (0, 26,000, 15,000) ly, in shells [R₁, R₂] per
  layer (C and D 300–600 ly, E 500–1,000 ly) at 10.06 and 7.95. The real side is the census forced to
  R₂ less the census forced to R₁, by star distance; the synthetic side is the tier with R(u) forced
  to R₁ and clipped at R₂, over 64 salts. Per layer, 1-mag bin of apparent V (the bins reaching the
  bright end, since stars brighter than V_P are synthetic beyond the limit) and |b| band: counts,
  light, the T_eff histogram and the shares above A_V 2, 5 and 10. Gate: each count and light ratio
  within 1 ± 3σ, once the tables lane has corrected R06.T5.f's findings; until then the ratios are
  recorded, not gated. The census's σ is its Poisson σ times √1.5, since its systems are Poisson
  but their stars come in multiples (Σk² ÷ Σk is 1.46 for D and 1.54 for E, R06 Risks, "R06.T5.f's
  measurements, as built"), combined with the tables' pair σ.
- **`the_synthetic_seam_is_continuous`** (slow): near the Sun at 10.06, counts per magnitude in thin
  shells either side of R(u), in the plane and above it, on rays where R(u) is the cap at V_P and on
  rays held at the limit, real against synthetic, and the scatter of
  those counts between neighbouring texels, real against synthetic: the texel-sized patches of
  Design note 7 are what it measures. A difference beyond 3σ is a finding; its remedies are
  per-star sightlines for candidates near the cut, or a finer band.
- **Benches** (sampled, `HYPERION_SKY_BENCH_SAMPLE`): the full hybrid sky cold near the Sun at the
  eye's cut and at 10.06, the time to each reply, the nuclear disc, and the same at
  `HYPERION_WORKERS=4`; the synthetic share of the stars brighter than the eye's limit.
- **A golden**, `crates/hyperion-sim/tests/golden/sky/synthetic_near_sun.golden`: the synthetic
  stars of a few cells for a pinned observer near the Sun (keys, indices, positions to 10⁻⁶ ly, V
  to 10⁻⁶ mag), at the current generator version.
- **By hand, hidden:** the photorealistic view near the Sun, with the asserted star counts per
  magnitude in annuli about R(u), recorded (no golden image).

Record the figures in this plan's Risks. The brainstorm's subsection already carries R13.T1's and
T1.b's measured boundary and costs (signed off 2026-10-09 by the sign-off agent, owner's
delegation); where T9's figures differ from that text, draft the edit for a sign-off agent. Files:
`crates/hyperion-sim/tests/sky_hybrid.rs`, the golden, `.config/nextest.toml`, this plan.
Acceptance: `just test-slow the_synthetic_tier_matches_the_census the_synthetic_seam_is_continuous`,
`just bench -- sky/synthetic`, `just ci`.

## Verification

- **Exact in expectation:** the thinning against its intensity (T5.a), the kept stars' light and
  counts against the band's slots and `CapCount` (T5.b), and the total light against the interim
  reply (T6).
- **Consistent with the census of this galaxy:** counts, light, colour and extinction in shells,
  real against synthetic, at ten observers (T9), gated once R06.T5.f's tables are corrected.
- **One boundary:** no listed star beyond its texel's R(u) or the real limit, no synthetic star
  within it, no gap (T2.a, T5.b), and the band's light beyond R(u) to rounding (T3).
- **Determinism and stability:** observer independence, truncation, job-split independence, the
  word layout (T5.a), the golden (T9), and every reply's identical synthetic set (T6, T7).
- **Cost:** the boundary and the real tier (T1, T1.b, T2.b), the synthetic pass (T5.b) and the
  served sky (T9), each with its cut, its machine and whether it is provisional.
- **By hand, recorded:** the label at 1280 × 720 (T8) and the view's seam (T9).

## Generator version

No change to generated output and no bump. The sky only reads. The plan reserves one domain tag,
`sky.synthetic`, keyed by a synthetic cell's word, which no generated stream opens, so no existing
output moves. The luminosity tables gain fields (T4), which change their fingerprint and leave every
existing field's bits; nothing generated reads them. R06.T17's census golden is taken without a
ceiling (the oracle's census); R13.T9 adds the synthetic golden, re-blessed whenever the tables, the
colour table or the synthetic code moves, as R06's sky goldens are.

## Risks and open points

- **The estimates** (Design notes 4 and 16) are arithmetic on the decision records' figures, with
  self-similar distance distributions and per-record costs independent of distance: about a factor
  of two, and leaning low, since ignoring extinction places brighter stars too near and the discs'
  scale heights fall inside the boundaries; T7.b's rays pull the other way. T1 replaces them before
  T2 builds. **T1 measured them (2026-10-09; "Deviations in T1, as built", below):** the spheres at
  V<sub>P</sub> lie 1.6–3.2 times as far as estimated, and the real tier costs 8.8–22.6 times the estimate at
  both ceilings, for the eye and the camera, on the fixture and on the server's galaxy. Design note
  4's guard has tripped. **The owner answered on 2026-10-09** (`decision-r13-guard-trip.md`,
  option A): real stars to 2,000 ly (Design notes 3 and 4). The capped costs are that record's
  estimates, about ±30%, and T1.b measures them before T2 builds. **T1.b measured them
  (2026-10-09; "Deviations in T1.b, as built", below):** the capped tier costs 0.85–1.10 times
  the estimate on both galaxies, but 75 stars brighter than V 4.5 lie beyond 2,000 ly near the
  Sun, more than the guard's 60. T1.b's guard tripped, and the owner kept the limit on 2026-10-09
  (`decision-r13-t1b-guard.md`, option A).
- **The synthetic bright stars beyond 2,000 ly** (Design note 3): 75.4 brighter than V 4.5 near
  the Sun at V<sub>P</sub> 4.5 and 74.8 at 5.0, at the tables' expectation (R13.T1.b). That is
  8.6% of the sky's 880. Of them 71.7 are E's; the brightest is expected about V 2.1, and 0.093
  are brighter than V 1.0. The owner kept the limit with them, on 2026-10-09
  (`decision-r13-t1b-guard.md`). A synthetic bright star vanishes from the sky once the ship comes
  within the limit, and the real stars there take over. It is labelled, unselectable and never a
  jump target, but a navigator comparing views may notice. Lifting the limit cures it.
  - Elsewhere at the limit there are 49 a quarter turn round the solar circle and 115 on its far
    side. There are 123 of 167 2,000 ly above the Sun, and 423 in the inner disc (0.40 brighter
    than V 1.0).
  - E's realised count is 1.164 times its tables' (R06.T5.f). So the census itself would find some
    12 more near the Sun, and the synthetic tier draws 14% too few of E's until the tables lane
    corrects them.
- **The inner disc at the limit**: about 1.6 × 10⁵ CPU-s at V<sub>P</sub> 4.5 and 1.9 × 10⁵ at 5.0
  (3.0 and 3.5 h on 15 workers), unbenched (R13.T1.b's estimate at T1's costs per system near the
  Sun). R06.T17 benches it. The lever, if needed, is a yield-ordered budget per layer (T7.b's own
  λ rule with an expected count of k > 1 beyond for D and E, which T1.b records), under its own
  ruling. T1.b's rows show that a budget alone costs more there, not less: 6.8 × 10⁵ CPU-s at a
  budget of 10 and 3.9 × 10⁵ at 30, against 1.6 × 10⁵ at the limit. So the inner disc's lever is
  a nearer bound (at 1,000 ly about 3.2 × 10⁴) or a cap on cost (`decision-r13-t1b-guard.md`
  §1.7). _T1.b: about 1.6–1.9 × 10⁵ CPU-s at T1's costs per system, with 423 stars brighter than
  V 4.5 beyond 2,000 ly there (below)._
- **The server's galaxy against the fixture.** A fixed limit's cost scales with local density,
  unlike the ceiling's rule. The server's galaxy is 2.46 times as dense locally, so the capped tier
  costs it about 2–2.5 times the fixture's. Both are generated galaxies, and the generated galaxy is
  the judge. _T1.b measured 2.25 times at 4.5 and 2.14 at 5.0 (below)._
- **The limit is lifted** to the cap at V_P when the census levers land (`deferred-corrections.md`,
  "Census cost", "R13's real limit"): the real tier at the ceiling's caps is re-measured then, and
  the limit moves out once that costs no more than the capped tier. It is one constant, with no
  redesign.

  A partial step is measured (`decision-r13-t1b-guard.md`, option C). The limit is kept as a floor,
  and D and E reach past it where their yield-ordered cap at a budget of 30 does:
  max(min(R<sub>1</sub>, 2,000 ly), R<sub>30</sub>) by ray.
  - Near the Sun it leaves 16.6 stars brighter than V 4.5 beyond.
  - It costs about 2.5 × 10⁴ CPU-s on the fixture and 3.2–3.9 × 10⁴ on the server's galaxy
    (_estimates_), but about 4.3 × 10⁵ in the inner disc.

  It is weighed again with the census levers, with a cap on cost.

- **Realised against expected.** R06.T5.f's findings (C 0.966 ± 0.004, E 1.164 ± 0.034, realised
  over tabulated) would show as a density step at R(u): about 3.5% more synthetic C stars and 14%
  fewer E stars than the census would list. T9's gate waits for the tables lane's correction; until
  then the ratios are recorded. With the real limit, T5.f's E correction matters more, since more
  of E's sky is synthetic: E's synthetic stars are 14% short until it is corrected.
- **Dust resolution.** One profile per 1.4° texel at 64², a point sample: unbiased over the sky,
  but dust structure below 1.4°, which the real census's per-star sightlines see, becomes
  texel-sized patches of faint-star density beyond R(u), visible in a narrow camera field across a
  dust edge. T9 gates the scatter between texels.
- **Pair products' colours** follow their bin's single-star distribution (Design note 8); T9's T_eff
  histogram tests it.
- **Companions** share a pixel in the real tier beyond about 500 ly (2,000 au at 500 ly is 13″) and
  are independent in the synthetic tier. The counts per star match, since the census and the views
  keep and cull each star alone; blending before the cull would be R06's change, for both tiers.
- **Feature members.** When R06.T16.a lands with P09.T2.c, the field gives up φ, and features
  beyond R(u) need a rule of their own: drawn from each feature's own model, or listed by a real
  member census. A task of the T16.a integration; until then `CLUSTERS: NOT YET MODELLED`.
- **The camera's sky** is 47.6% synthetic near the Sun at the limit at V_P 4.5 and 37.0% at 5.0
  (R13.T1.b), against 38.0% and 22.8% beyond the cap at V_P alone (R13.T1), of some 3.9 × 10⁵
  stars to V 10.06, and its N_max overflow the norm. The camera-budget ruling owed by R06.T17
  rules on the real tier at the real boundary (T1 measured the camera's at 0.98–1.00 times the
  eye's), not on the exact census.
- **Multiplayer.** Two ships far apart see the same world-anchored synthetic stars, but each its own
  real boundary, so a star real for one may be absent for the other, replaced by synthetic ones.
  Inherent to any hybrid.
- **The first sky** stays R06.T11.g's (6.96 s and 94.6 CPU-s to 125 ly as built), whatever the
  limit; R13 adds 10–40 CPU-s to it (Design note 16).
- **The brainstorm's principle**, "the star field is the galaxy", is amended, not kept whole.
  **Decided by the owner on 2026-10-08** (`feasibility-hybrid-sky.md` §11, all as recommended):
  - V_P is 4.5, and 5.0 in RM3's interim;
  - RM3 ships R13.T2 with the band for the rest, and the sky is on by default once it and
    R06.T11.d land;
  - synthetic stars appear in the naked-eye view too, labelled;
  - the promise covers the real stars only, and the real-record variant is revisited after the
    deferred census levers.

  **Decided by the owner on 2026-10-09** (`decision-r13-guard-trip.md`, option A, as
  recommended): C–E are real to 2,000 ly at most, for RM3's interim and R13 alike, so every star
  brighter than V_P is real within 2,000 ly only.

  **Kept by the owner on 2026-10-09** after R13.T1.b (`decision-r13-t1b-guard.md`, option A, as
  recommended). 75 stars brighter than V 4.5 lie beyond the limit near the Sun, not 10–60, and 88%
  of the naked-eye stars stay real, not 92–97%.

  The brainstorm's subsection is signed off. Its label wording: T2.b's is ruled
  (`decision-r13-t2b-note.md`); T8's state word stays a draft for the UX decision agent. Both guide
  rows are drafted until a sign-off agent accepts them. Its amendment for the real limit is signed
  off (2026-10-09, the sign-off agent under the owner's delegation, `signoff-brainstorm.md`,
  source 2) and written into the subsection's text, the draft at its end removed: the boundary is
  the nearer of the ceiling's radius and 2,000 ly, the promise holds within 2,000 ly, 88% of the
  naked-eye stars are real, and about half of a camera's stars to V 10 (48%) are synthetic.

- **Lazy realisation by record**, to be revisited after the deferred census levers land (the
  owner, 2026-10-08), so that every point could be a system:
  real positions and identities with a brightness drawn from each record's mass, age and \[Fe/H\],
  exact on approach for single main-sequence stars to the table's accuracy and statistical for
  evolved stars and pairs, at a walk of every far record (at least about 10³ CPU-s at the eye and
  10⁴ at the camera) and a new fitted table.
- **Open for decision agents:** the cell size (T5.b); texel or bilinear extinction, if T9's seam
  test asks (T5.b, T9); the label's wording (T8); a reach past the limit by yield, or a cap on cost
  for the inner disc, with the census levers (T1.b records the budgets;
  `decision-r13-t1b-guard.md` §1.5–§1.7). **Ruled 2026-10-09:** spheres or rays for R(u), and
  fix (i): rays, with each ray's cone widened by ρ, at every ceiling, point and cut
  (`decision-r13-guard-trip.md` §1; Design note 3; T1's figures below, "Spheres or rays, and
  fix (i)").
- **Deviations in T1, as built, and its measurements (2026-10-09).** Measured at
  `rendering-and-planets` 4dec82b4, so with P11.T17.c's held floor (about +18% on a cold census)
  and R06.T11.d. Then rebased onto 090d880d (R06.T11.f and T11.g). T11.g tests each of its splits
  bit for bit against the one-job form, and the slow test, run again there, prints every figure as
  before (399 s). The fixture is `milky_way_like` with seed
  0x0926_0000; the server's galaxy is seed 0x4d2 with its full potential. Both are at the epoch.
  Every count is the caps' count's expectation, with no census; every cost is a sampled census.
  **The owner's guard has tripped** (Design note 4). The real tier costs 8.8–22.6 times the
  feasibility study's estimate, at both ceilings, for the eye and the camera, on both galaxies.
  T2 waits for the owner. _Answered on 2026-10-09: the real limit of 2,000 ly (Design notes 3 and
  4), with R13.T1.b before T2._
  - **The build.**
    - `sky::caps` gains three things (Provides, "sky::caps (R13.T1)"):
      - `RayRadii::lesser`: each ray the lesser of two radii on one lattice, which is Design note
        3's "never beyond the cut's own cap" applied ray by ray.
      - A public `LayerCap::forced_by_ray`, which was test-only, for a census to a boundary the
        count does not give.
      - `CapCount::{stars_beyond_toward, systems_within_toward, stars_within_and_beyond_toward}`:
        the counts towards a boundary given ray by ray as a closure, such as a texel's centre,
        cones widened by ρ, or a sphere held within rays.

      The cap-taking counts now call the new ones with the same loop and arithmetic, so their bits
      are unchanged (determinism audit). A radius that is negative or not finite panics. The unit
      tests check the closures, bit for bit, against caps that hold the same boundary, check
      `lesser` ray by ray and towards 10⁴ directions, and check the refusals.

    - `tests/sky_hybrid.rs` holds the slow test `the_hybrid_boundary_is_recorded`. It runs on six
      threads, one a point, natively only, since wasm32 has no threads; its slow-profile override
      takes 6 slots. It took 447 s at `CPUQuota=400%`. It asserts only T1's two claims, and both
      hold everywhere (below). The six points and the recount's resolution moved to
      `tests/common/sky.rs` (`CAPS_POINTS`, `caps_recount`), which `sky_caps.rs` now shares.
      R06.T7.c's slow test in `sky::caps` keeps its own copy, `CONVERGENCE_POINTS`, now
      cross-referenced.
    - `benches/sky.rs` adds `sky/census_near_sun_ceiling/{eye,camera}_{4.5,5.0}`, and the same
      four as `served_*` on the server's galaxy.
      - C to E are censused to Design note 3's real boundary, which the bench builds by the same
        rule as `real_boundary` (`real_boundary_rule`). A, B and the brown dwarfs go to the cut's
        caps.
      - The census moved into `census_of_plan`, split from `census`. It keeps each layer's census
        time, and for this bench alone each job thread's CPU time (`CpuTime::Measured`,
        `ThreadCpu`).
  - **Deviations.**
    - **The eye's cut takes the request's illumination**, as the server has computed it since
      R06.T11.d, which landed after this plan was written. Near the Sun it is V 8.179 on the
      fixture (8.282 without the illumination) and V 7.766 on the server's galaxy. R(u) depends on
      V<sub>P</sub> alone. So only the counts at the cut, the cut's caps, the gap and the spheres'
      cost (they are held within the cut's caps) move with it.
    - **The bench runs on two galaxies.** `decision-r06-t11d-first-sky.md` ("The bench's galaxy")
      asks T1 to record the real tier on the galaxy served beside the fixture's. The bench builds
      that galaxy as the server's tests and bench do:
      `Galaxy::new(Seed::new(0x4d2)).with_full_potential()`.
    - **The guard reads CPU time.**
      - The bench prints each layer's census time twice. The first is its jobs' wall time, the
        measure the other benches use. The second, on Linux, is the CPU time the job threads spent
        on the sampled cells (`/proc/thread-self/schedstat`), read through one open file per job
        and outside the wall-time window. Off Linux no clock opens, and the bench prints wall time
        alone.
      - The guard reads the CPU time.
      - Earlier runs at cb724aca, before the merge, timed by wall clock at a load of about 15–19,
        read 0.89–1.85 × 10⁵ CPU-s. They are superseded.
    - **The costs are provisional.** The machine was shared: the runs began at loads of 6.3 (the
      fixture) and 12.9 (the server's galaxy), under the `schedutil` governor. CPU time removes
      the waits, but not SMT or clock contention. The smallest ratio, 8.8, is 4.4 times the guard,
      beyond any load effect.
    - **The census is sampled**, 1 block of cells in 50 on the fixture and 1 in 100 on the server's
      galaxy, then scaled. The sampling error is not measured. The timing repeats, though: at one
      ceiling the eye's and the camera's runs census the same sampled C–E cells, since R(u) is the
      ceiling's, and their CPU-s agree to 1.7% on the fixture and 0.01–0.08% on the server's
      galaxy.
    - **The benched tier has no fix (i).** Fix (i) adds 0.6–8.5% of the systems (E 6.2% near the
      Sun at 4.5), so the costs below are a few percent low.
    - **R(u) never needed the hold.** At no point, cut or ceiling did a ray's cap at V<sub>P</sub>
      lie beyond the cut's: 0 rays were held. So `lesser` changes nothing measured; it stays as
      the rule's guarantee.
    - **Acceptance as run.** The slow test ran from its slow-test-profile binary, not through
      `just test-slow`, whose heavy lock the lane rules keep for timed runs and full suites. The
      nextest override and `hyperion-fit check --rerun-fast` run at integration, as `just ci`
      does. `cargo bench -p hyperion-sim --no-run` passed.
    - **The record is a digest.** It holds the near-Sun rows at the eye's cut in full and the
      other points' rows at 4.5 and 5.0. The slow test prints all 216 rows (six points, three cuts,
      four ceilings, three layers), deterministically.
  - **The boundary near the Sun** (fixture). This is at the eye's cut, V 8.179, with the caps by
    the eye's visibility. Radii are in ly. Everything is by ray unless marked as the sphere, and
    every count is taken at the 3,072-ray recount. "Beyond" is the expected count brighter than
    V 5 / 6 / 6.5 / 7 / the cut beyond R(u): the stars that the synthetic tier and the band take.
    "> V<sub>P</sub>" is the expected count brighter than V<sub>P</sub> beyond R(u), at the caps'
    own count and then recounted. The gap and its mirror are at the cut; the spheres' gap there is
    0–0.002. The cut's own caps there have ray medians (10–90%) and largest rays of C 7,444
    (5,584–9,925) and 12,023 ly, D 6,146 (5,073–8,193) and 9,925 ly, and E 4,610 (3,805–13,232)
    and 41,804 ly.

    | V<sub>P</sub> | Layer | R(u) median (10–90%), largest | Sphere | Beyond: V 5 / 6 / 6.5 / 7 / 8.18    | > V<sub>P</sub> | Systems within | Sphere ÷ rays | Fix (i) ÷ rays | Gap / mirror  |
    | ------------- | ----- | ----------------------------- | ------ | ----------------------------------- | --------------- | -------------- | ------------- | -------------- | ------------- |
    | 4.0           | C     | 1,038 (944–1,038), 1,038      | 1,038  | 9.87 / 82 / 249 / 719 / 8,300       | 0.920 / 0.882   | 1.005 × 10⁶    | 1.126         | 1.007          | 8.47 / 10.7   |
    | 4.0           | D     | 1,519 (1,141–2,222), 2,222    | 2,020  | 7.49 / 63.2 / 178 / 449 / 2,970     | 0.701 / 0.607   | 1.096 × 10⁶    | 1.250         | 1.031          | 5.62 / 7.77   |
    | 4.0           | E     | 2,443 (1,255–5,753), 8,416    | 6,327  | 2.17 / 13.4 / 31.1 / 67.6 / 333     | 0.399 / 0.276   | 4.465 × 10⁶    | 1.303         | 1.060          | 1.16 / 6.92   |
    | 4.5           | C     | 1,416 (1,287–1,416), 1,416    | 1,416  | 4.08 / 35.0 / 99.8 / 300 / 3,540    | 0.896 / 0.829   | 2.324 × 10⁶    | 1.131         | 1.006          | 2.54 / 4.77   |
    | 4.5           | D     | 2,075 (1,287–2,763), 3,344    | 2,763  | 2.38 / 32.8 / 106 / 276 / 1,760     | 0.677 / 0.570   | 2.365 × 10⁶    | 1.290         | 1.032          | 3.67 / 4.48   |
    | 4.5           | E     | 2,511 (1,416–7,177), 9,557    | 7,177  | 0.738 / 5.20 / 12.6 / 29.7 / 180    | 0.380 / 0.248   | 6.357 × 10⁶    | 1.191         | 1.062          | 0.709 / 0.922 |
    | 5.0           | C     | 1,758 (1,758–1,934), 1,934    | 1,934  | 0.764 / 16.0 / 43.5 / 119 / 1,660   | 0.854 / 0.764   | 5.020 × 10⁶    | 1.188         | 1.010          | 2.95 / 3.04   |
    | 5.0           | D     | 2,343 (1,597–3,782), 5,548    | 3,437  | 0.293 / 5.60 / 19.2 / 63.5 / 622    | 0.425 / 0.293   | 5.213 × 10⁶    | 1.014         | 1.047          | 5.99 / 7.61   |
    | 5.0           | E     | 2,838 (1,597–8,139), 11,939   | 8,139  | 0.215 / 1.77 / 4.50 / 11.1 / 80.6   | 0.338 / 0.215   | 9.018 × 10⁶    | 1.089         | 1.063          | 0.478 / 0.691 |
    | 5.5           | C     | 2,356 (2,356–2,593), 2,593    | 2,356  | 0.390 / 4.17 / 18.4 / 52.8 / 581    | 0.864 / 0.777   | 1.044 × 10⁷    | 0.949         | 1.010          | 0.814 / 1.84  |
    | 5.5           | D     | 2,593 (1,767–4,610), 6,146    | 4,610  | 0.0558 / 1.54 / 6.63 / 25.0 / 320   | 0.448 / 0.317   | 8.143 × 10⁶    | 1.339         | 1.042          | 1.60 / 2.51   |
    | 5.5           | E     | 2,854 (1,767–9,018), 19,416   | 9,925  | 0.0723 / 0.697 / 1.97 / 5.18 / 43.9 | 0.358 / 0.231   | 1.204 × 10⁷    | 1.255         | 1.061          | 0.334 / 0.528 |

    R(u) does not depend on the cut. At 7.95 and 10.06 the rows differ in their counts at the cut,
    their gaps and the spheres' cost, since the spheres are held within the cut's caps: E's
    sphere ÷ rays at 4.0 is 1.303, 1.332 and 1.416 at the eye's cut, 7.95 and 10.06. Those rows
    match a run before the merge to every printed digit.

  - **The share beyond R(u)** near the Sun, by ray: layers C to E beyond R(u), as a share of every
    layer's whole sky at any distance. The study's figures are at the cut 7.95.

    | V<sub>P</sub> | V 5   | V 6   | V 6.5 | V 7   | To the eye's cut, 8.18 | To 7.95 | To 10.06 | Study: V 6.5 / to 7.95 |
    | ------------- | ----- | ----- | ----- | ----- | ---------------------- | ------- | -------- | ---------------------- |
    | 4.0           | 1.21% | 2.99% | 4.85% | 7.46% | 19.56%                 | 15.89%  | 52.3%    | 17–22% / 67–75%        |
    | 4.5           | 0.44% | 1.37% | 2.32% | 3.66% | 9.22%                  | 7.83%   | 38.0%    | 8–11% / 42–53%         |
    | 5.0           | 0.08% | 0.44% | 0.71% | 1.17% | 3.98%                  | 3.15%   | 22.8%    | 3.6–4.7% / 18–21%      |
    | 5.5           | 0.03% | 0.12% | 0.29% | 0.50% | 1.59%                  | 1.26%   | 10.5%    | 1.0–1.5% / 9–11%       |

    The whole sky there holds 1,619 stars to V 5, 5,308 to V 6, 9,430 to V 6.5, 16,560 to V 7,
    59,352 to the eye's cut, 46,585 to 7.95 and 3.94 × 10⁵ to 10.06.

  - **The other points**, at each point's eye's cut, for V<sub>P</sub> 4.5 and then 5.0. The
    columns are: the C–E systems within R(u); the share beyond R(u) of the whole sky to V 6.5 and
    to the cut; and the C–E gap, by ray, at the cut. The largest count brighter than V<sub>P</sub>
    beyond R(u), over all the point's rows, is 0.861 in the nuclear disc, 0.863 a quarter turn
    round, 0.875 on the far side, 0.854 in the inner disc and 0.992 2,000 ly above the Sun
    (0.776–0.972 recounted).

    | Point (eye's cut)                    | Systems within         | Share to V 6.5 / to the cut  | Gap        |
    | ------------------------------------ | ---------------------- | ---------------------------- | ---------- |
    | Near the Sun (8.179)                 | 1.10 × 10⁷; 1.93 × 10⁷ | 2.32% / 9.22%; 0.71% / 3.98% | 6.92; 9.42 |
    | Nuclear disc (5.807)                 | 1.10 × 10⁶; 1.53 × 10⁶ | 0.26% / 0.09%; 0.09% / 0.04% | 0.18; 0.07 |
    | Solar circle, a quarter turn (8.109) | 8.47 × 10⁶; 1.42 × 10⁷ | 2.01% / 8.44%; 0.77% / 4.05% | 8.18; 3.15 |
    | Solar circle, far side (7.730)       | 1.32 × 10⁷; 2.16 × 10⁷ | 0.90% / 3.44%; 0.33% / 1.31% | 3.10; 2.89 |
    | Inner disc (6.722)                   | 2.43 × 10⁸; 3.48 × 10⁸ | 0.41% / 0.49%; 0.20% / 0.24% | 2.35; 1.09 |
    | 2,000 ly above the Sun (8.540)       | 2.08 × 10⁷; 3.50 × 10⁷ | 7.69% / 32.2%; 3.14% / 16.6% | 19.3; 15.9 |

    Layer by layer, in the near-Sun table's terms, with the spheres' gap last:

    | Point          | V<sub>P</sub> | Layer | R(u) median (10–90%), largest | Sphere | > V<sub>P</sub> | Systems within | Sphere ÷ rays | Fix (i) ÷ rays | Gap / mirror  | Sphere gap |
    | -------------- | ------------- | ----- | ----------------------------- | ------ | --------------- | -------------- | ------------- | -------------- | ------------- | ---------- |
    | Nuclear disc   | 4.5           | C     | 37 (34–45), 45                | 41     | 0.848 / 0.784   | 5.283 × 10⁵    | 1.286         | 1.014          | 0.069 / 0.05  | 0          |
    | Nuclear disc   | 4.5           | D     | 50 (45–66), 80                | 60     | 0.664 / 0.551   | 2.640 × 10⁵    | 1.403         | 1.025          | 0.067 / 0.076 | 0.002      |
    | Nuclear disc   | 4.5           | E     | 73 (66–128), 207              | 128    | 0.656 / 0.566   | 3.093 × 10⁵    | 1.812         | 1.041          | 0.046 / 0.059 | 0.003      |
    | Nuclear disc   | 5.0           | C     | 42 (38–50), 56                | 46     | 0.808 / 0.733   | 7.235 × 10⁵    | 1.221         | 1.018          | 0.018 / 0.018 | 0.002      |
    | Nuclear disc   | 5.0           | D     | 50 (46–74), 99                | 74     | 0.633 / 0.503   | 3.560 × 10⁵    | 1.311         | 1.027          | 0.03 / 0.027  | 0.004      |
    | Nuclear disc   | 5.0           | E     | 90 (74–159), 257              | 159    | 0.628 / 0.537   | 4.503 × 10⁵    | 1.443         | 1.046          | 0.027 / 0.025 | 0.004      |
    | A quarter turn | 4.5           | C     | 1,269 (1,154–1,269), 1,396    | 1,269  | 0.830 / 0.756   | 1.827 × 10⁶    | 1.072         | 1.017          | 4.73 / 4.27   | 0          |
    | A quarter turn | 4.5           | D     | 1,689 (1,269–2,472), 2,719    | 2,247  | 0.652 / 0.540   | 1.655 × 10⁶    | 1.092         | 1.039          | 3.2 / 2.75    | 0          |
    | A quarter turn | 4.5           | E     | 2,472 (1,396–6,410), 9,384    | 6,410  | 0.373 / 0.244   | 4.989 × 10⁶    | 1.199         | 1.058          | 0.247 / 1.75  | 0.001      |
    | A quarter turn | 5.0           | C     | 1,732 (1,431–1,732), 1,906    | 1,732  | 0.791 / 0.689   | 3.875 × 10⁶    | 1.158         | 1.016          | 1.8 / 2.6     | 0          |
    | A quarter turn | 5.0           | D     | 2,097 (1,431–3,382), 3,722    | 3,074  | 0.538 / 0.407   | 3.524 × 10⁶    | 1.138         | 1.043          | 1.25 / 3.69   | 0          |
    | A quarter turn | 5.0           | E     | 2,539 (1,431–7,267), 10,653   | 7,996  | 0.361 / 0.228   | 6.795 × 10⁶    | 1.410         | 1.060          | 0.102 / 0.871 | 0.001      |
    | Far side       | 4.5           | C     | 1,396 (1,269–1,396), 1,396    | 1,396  | 0.875 / 0.799   | 3.468 × 10⁶    | 1.025         | 1.006          | 0.939 / 1.15  | 0          |
    | Far side       | 4.5           | D     | 2,044 (1,396–2,720), 3,291    | 2,720  | 0.542 / 0.427   | 2.937 × 10⁶    | 1.234         | 1.037          | 1.76 / 5.05   | 0          |
    | Far side       | 4.5           | E     | 2,720 (1,535–7,052), 12,491   | 7,757  | 0.396 / 0.207   | 6.804 × 10⁶    | 1.287         | 1.058          | 0.398 / 0.908 | 0.001      |
    | Far side       | 5.0           | C     | 1,906 (1,732–1,906), 1,906    | 1,906  | 0.806 / 0.704   | 7.048 × 10⁶    | 1.096         | 1.016          | 2.07 / 2.24   | 0          |
    | Far side       | 5.0           | D     | 2,308 (1,574–3,383), 4,507    | 3,383  | 0.443 / 0.320   | 4.901 × 10⁶    | 1.194         | 1.044          | 0.598 / 2.97  | 0          |
    | Far side       | 5.0           | E     | 2,794 (1,732–7,998), 14,194   | 9,683  | 0.345 / 0.177   | 9.639 × 10⁶    | 1.510         | 1.053          | 0.223 / 1.73  | 0.001      |
    | Inner disc     | 4.5           | C     | 1,532 (1,267–1,685), 1,685    | 1,532  | 0.802 / 0.709   | 3.778 × 10⁷    | 1.065         | 1.020          | 0.896 / 0.792 | 0          |
    | Inner disc     | 4.5           | D     | 2,243 (1,532–2,985), 3,972    | 2,985  | 0.436 / 0.308   | 3.675 × 10⁷    | 1.204         | 1.049          | 1.12 / 1.66   | 0          |
    | Inner disc     | 4.5           | E     | 3,283 (1,854–7,035), 12,458   | 9,362  | 0.267 / 0.152   | 1.687 × 10⁸    | 1.240         | 1.066          | 0.33 / 0.243  | 0.002      |
    | Inner disc     | 5.0           | C     | 1,903 (1,572–2,093), 2,093    | 2,093  | 0.834 / 0.739   | 7.324 × 10⁷    | 1.224         | 1.020          | 0.546 / 0.606 | 0          |
    | Inner disc     | 5.0           | D     | 2,535 (1,729–3,715), 4,948    | 3,715  | 0.366 / 0.244   | 6.159 × 10⁷    | 1.191         | 1.056          | 0.482 / 0.757 | 0.001      |
    | Inner disc     | 5.0           | E     | 3,376 (1,903–7,980), 14,161   | 10,631 | 0.357 / 0.186   | 2.133 × 10⁸    | 1.197         | 1.061          | 0.063 / 0.086 | 0.004      |
    | 2,000 ly above | 4.5           | C     | 1,298 (1,298–1,429), 1,429    | 1,429  | 0.928 / 0.876   | 6.574 × 10⁵    | 1.140         | 1.016          | 7.46 / 4.28   | 0          |
    | 2,000 ly above | 4.5           | D     | 805 (805–3,377), 4,499        | 3,377  | 0.551 / 0.421   | 2.949 × 10⁶    | 1.136         | 1.071          | 10.5 / 6.84   | 0          |
    | 2,000 ly above | 4.5           | E     | 886 (805–9,665), 22,846       | 11,701 | 0.375 / 0.184   | 1.717 × 10⁷    | 1.354         | 1.085          | 1.32 / 0.457  | 0.001      |
    | 2,000 ly above | 5.0           | C     | 1,945 (1,945–2,141), 2,141    | 2,141  | 0.955 / 0.871   | 2.614 × 10⁶    | 1.233         | 1.017          | 3.17 / 2.27   | 0          |
    | 2,000 ly above | 5.0           | D     | 1,205 (1,205–4,188), 6,146    | 4,610  | 0.475 / 0.325   | 6.667 × 10⁶    | 1.305         | 1.060          | 11.8 / 5.93   | 0          |
    | 2,000 ly above | 5.0           | E     | 3,458 (1,094–12,023), 37,983  | 14,563 | 0.364 / 0.240   | 2.573 × 10⁷    | 1.518         | 1.073          | 0.881 / 0.552 | 0.007      |

  - **The real tier's cost** (the bench, near the Sun). The cost is in CPU-s by the job threads'
    CPU time, with the generated records alongside, each layer's count scaled by the sample. The
    whole includes the two caps' counts and the plan, 23–25 s, with the total by wall time after
    it. "On 15" is the whole ÷ 15 workers, which ignores imbalance.

    | Galaxy, request, cut    | V<sub>P</sub> | C                  | D                   | E                    | Whole (by wall time) | Study         | Times the study | On 15 |
    | ----------------------- | ------------- | ------------------ | ------------------- | -------------------- | -------------------- | ------------- | --------------- | ----- |
    | Fixture, eye, 8.179     | 4.5           | 1,609 (5.54 × 10⁵) | 12,800 (1.62 × 10⁶) | 80,433 (7.00 × 10⁶)  | 94,867 (104,683)     | 4,500–5,800   | 16.4–21.1       | 1.8 h |
    | Fixture, eye, 8.179     | 5.0           | 3,346 (1.15 × 10⁶) | 31,299 (3.91 × 10⁶) | 109,870 (9.64 × 10⁶) | 144,539 (166,819)    | 10,000–12,500 | 11.6–14.5       | 2.7 h |
    | Fixture, camera, 10.06  | 4.5           | 1,543 (7.48 × 10⁵) | 12,751 (1.64 × 10⁶) | 78,958 (7.00 × 10⁶)  | 93,276 (93,664)      | 5,400–7,540   | 12.4–17.3       | 1.7 h |
    | Fixture, camera, 10.06  | 5.0           | 3,363 (1.40 × 10⁶) | 31,231 (4.00 × 10⁶) | 109,031 (9.65 × 10⁶) | 143,649 (154,755)    | 12,000–16,250 | 8.8–12.0        | 2.7 h |
    | Server's, eye, 7.766    | 4.5           | 3,975 (1.31 × 10⁶) | 17,769 (2.28 × 10⁶) | 79,971 (7.06 × 10⁶)  | 101,740 (103,968)    | 4,500–5,800   | 17.5–22.6       | 1.9 h |
    | Server's, eye, 7.766    | 5.0           | 6,411 (2.18 × 10⁶) | 25,241 (3.19 × 10⁶) | 111,759 (9.90 × 10⁶) | 143,438 (144,031)    | 10,000–12,500 | 11.5–14.3       | 2.7 h |
    | Server's, camera, 10.06 | 4.5           | 3,919 (1.78 × 10⁶) | 17,830 (2.31 × 10⁶) | 79,955 (7.07 × 10⁶)  | 101,731 (102,166)    | 5,400–7,540   | 13.5–18.8       | 1.9 h |
    | Server's, camera, 10.06 | 5.0           | 6,554 (2.74 × 10⁶) | 25,280 (3.24 × 10⁶) | 111,691 (9.92 × 10⁶) | 143,553 (144,172)    | 12,000–16,250 | 8.8–12.0        | 2.7 h |

    Each layer's records past the floor, generated records, accepted stars and accepted stars
    within R(u) towards their band texel (those R13.T2.a lists), scaled by the sample:

    | Galaxy, request  | V<sub>P</sub> | C                                           | D                                           | E                                         |
    | ---------------- | ------------- | ------------------------------------------- | ------------------------------------------- | ----------------------------------------- |
    | Fixture, eye     | 4.5           | 2.54 × 10⁶ / 5.54 × 10⁵ / 24,950 / 24,700   | 2.85 × 10⁶ / 1.62 × 10⁶ / 17,900 / 17,750   | 8.09 × 10⁶ / 7.00 × 10⁶ / 4,500 / 4,450   |
    | Fixture, eye     | 5.0           | 5.44 × 10⁶ / 1.15 × 10⁶ / 26,600 / 26,550   | 7.02 × 10⁶ / 3.91 × 10⁶ / 19,550 / 19,550   | 1.12 × 10⁷ / 9.64 × 10⁶ / 4,550 / 4,450   |
    | Fixture, camera  | 4.5           | 2.54 × 10⁶ / 7.48 × 10⁵ / 133,500 / 129,250 | 2.85 × 10⁶ / 1.64 × 10⁶ / 69,300 / 67,700   | 8.09 × 10⁶ / 7.00 × 10⁶ / 20,900 / 20,650 |
    | Fixture, camera  | 5.0           | 5.44 × 10⁶ / 1.40 × 10⁶ / 171,550 / 168,550 | 7.02 × 10⁶ / 4.00 × 10⁶ / 92,250 / 91,450   | 1.12 × 10⁷ / 9.65 × 10⁶ / 21,400 / 21,100 |
    | Server's, eye    | 4.5           | 6.14 × 10⁶ / 1.31 × 10⁶ / 21,200 / 21,100   | 4.04 × 10⁶ / 2.28 × 10⁶ / 21,100 / 21,000   | 8.17 × 10⁶ / 7.06 × 10⁶ / 3,300 / 3,300   |
    | Server's, eye    | 5.0           | 1.03 × 10⁷ / 2.18 × 10⁶ / 22,000 / 22,000   | 5.67 × 10⁶ / 3.19 × 10⁶ / 21,500 / 21,500   | 1.15 × 10⁷ / 9.90 × 10⁶ / 3,400 / 3,400   |
    | Server's, camera | 4.5           | 6.14 × 10⁶ / 1.78 × 10⁶ / 227,300 / 222,700 | 4.04 × 10⁶ / 2.31 × 10⁶ / 109,700 / 106,000 | 8.17 × 10⁶ / 7.07 × 10⁶ / 21,000 / 20,900 |
    | Server's, camera | 5.0           | 1.03 × 10⁷ / 2.74 × 10⁶ / 266,500 / 261,300 | 5.67 × 10⁶ / 3.24 × 10⁶ / 114,300 / 112,400 | 1.15 × 10⁷ / 9.92 × 10⁶ / 21,100 / 21,000 |
    - **Where the cost lies.** At 4.5 on the fixture, E generates 86% of its records, and it takes
      85% of the cost for 10% of the eye's stars.
    - **The server's galaxy.** Its local density is 2.46 times the fixture's, by the count's
      systems within 500 ly, in each of C, D and E. Its R(u) by ray at 4.5 is C 1,386
      (1,146–1,386), 1,386; D 1,677 (1,260–2,454), 2,699; E 2,231 (1,386–5,256), 11,255. At 5.0 it
      is C 1,720 (1,421–1,893), 1,893; D 1,893 (1,292–3,052), 3,357; E 2,521 (1,564–5,955), 14,068.
      That is nearer than the fixture's in D and E. So its real tier costs about the same as the
      fixture's, although it is denser: 1.07 and 0.99 times for the eye at 4.5 and 5.0, and 1.09
      and 1.00 for the camera. This is a record, not a finding: the generated galaxy is the judge.
    - **Memory.** The bench's process held about 176 MiB when sampled. Its 10G cap was set before
      it had run and was far more than it needed.

  - **Against the estimates.**
    - **R(u).** The study's spheres at 4.5 were C 740–850, D 1,180–1,290 and E 2,240–2,510 ly.
      The measured spheres are 1,416, 2,763 and 7,177 ly, 1.7–3.2 times the study's. At 5.0 the
      study had 1,050–1,180 / 1,600–1,730 / 3,110–3,420 ly, and the spheres are 1,934, 3,437 and
      8,139 ly, 1.6–2.6 times. E's ray median is the study's own, 2,511 ly at 4.5 (1.00–1.12
      times) and 2,838 at 5.0 (0.83–0.91 times). But E's rays reach far out of the plane: their
      90th percentile is 7,177 ly and the largest 9,557 ly at 4.5. That is the study's §3.4 bias
      from ignoring extinction, far larger than the factor of two it allowed.
    - **The work.** Near the Sun at 4.5, C–E have 1.35 × 10⁷ records past the floor, 9.6 times
      the 1,000 ly identity test's 1.41 × 10⁶; E's alone are 108 times its 7.48 × 10⁴.
    - **Cost per record.** Per record past the floor it is C 0.63, D 4.5 and E 9.9 ms, against the
      study's 0.22, 2.2 and 6.6 ms. Per generated record it is C 2.9, D 7.9 and E 11.5 ms.
    - **The share left beyond.** At 4.5, 2.3% of the stars brighter than V 6.5 lie beyond R(u),
      and 7.8% of those to 7.95, against the study's 8–11% and 42–53%: 0.21–0.29 times its share
      to V 6.5 and 0.15–0.19 times its share to 7.95.
    - **The camera.** Its real tier costs 0.98–1.00 times the eye's at one ceiling, against the
      study's 1.2–1.3. R(u) does not move with the cut, and C, which the deeper cut lists more of,
      is cheap.
    - **From 4.5 to 5.0** the cost rises 1.41–1.54 times, against the study's 2.2.
    - **Wall time.** On 15 workers the real tier takes 1.7–1.9 h at 4.5 and 2.7 h at 5.0, against
      the study's 5–6.5 and 11–14 minutes. On 4 workers it is 6.5–7.1 h and 10 h.
    - **Against the exact census.** The exact census by ray costs 0.4–1.0 × 10⁶ CPU-s. The
      hybrid's real tier at 4.5 is 3.9–10.7 times cheaper, against the study's 80–200.
    - **Other ceilings** (_estimate_). The fixture's measured per-system costs near the Sun at 4.5
      are C 0.69, D 5.41 and E 12.65 ms per system within R(u), at the recount. Applied to 5.0's
      systems, they predict its measured C–E census to 0.9%. They put V<sub>P</sub> 4.0 at about
      6.3 × 10⁴ CPU-s and 5.5 at about 2.0 × 10⁵. At every ceiling measured, E's far rays hold most
      of the work.
    - **Elsewhere** (_estimate_, at the same per-system costs, not benched). The inner disc's
      2.4 × 10⁸ C–E systems within R(u) at 4.5 would cost about 2.4 × 10⁶ CPU-s. The 2.1 × 10⁷
      systems 2,000 ly above the Sun would cost about 2.3 × 10⁵.
  - **Spheres or rays, and fix (i)** (for the decision agent, under the delegation).
    - **The gap by ray.** These are the stars brighter than the cut that lie within the radius
      towards their texel's centre and beyond their own ray's radius, C to E summed. Near the Sun
      it is 6.9 at the eye's cut at 4.5 and 9.4 at 5.0; 4.6 and 6.8 at 7.95; and 121 and 194 at
      10.06. At the other points it reaches 19 at the eye's cut (2,000 ly above the Sun), 31 at
      7.95 and 1,485 at 10.06 (the inner disc). Its mirror is of the same size. So rays alone fail
      R13.T2.a's "no gap" test.
    - **Fix (i)** widens the cones by ρ, the 64² band's largest texel radius. It closes the gap to
      0 at every point, cut and ceiling. It costs 1.006–1.085 times the rays' systems: C 1.006–1.023, D 1.019–1.071, E 1.038–1.085.
    - **Spheres** at V<sub>P</sub>, held within the cut's caps, cost 0.949–3.226 times the rays'
      systems: 0.95–1.44 near the Sun, with E at 3.2 in the nuclear disc and 2.6 above the Sun.
      They are not exact:
      - their gap reaches 0.306 stars (E, 2,000 ly above the Sun, the eye's cut, V<sub>P</sub>
        5.5), and up to 0.007 at 4.5 and 5.0 (mirror 0.009), since the hold within the cut's caps
        is by ray;
      - in the nuclear disc at 5.5 they leave more than one star brighter than V<sub>P</sub> beyond
        them (C 1.140 and E 1.031 at the caps' own count), which breaks T1's second claim.
    - **Which is cheaper.** Layer by layer at the eye's cut, spheres cost less than rays with
      fix (i) in 7 of the 72 cases:
      - near the Sun, D at 5.0 (1.014 against 1.047) and C at 5.5 (0.949 against 1.010);
      - the nuclear disc, C at 5.5 (0.960 against 1.019);
      - a quarter turn round, C at 4.0 (0.958 against 1.016), C at 5.5 (0.989 against 1.022) and
        D at 5.5 (1.027 against 1.051);
      - the far side, C at 5.5 (0.966 against 1.016).

      Weighted by each layer's measured cost per system near the Sun, spheres cost 1.02–1.70
      times rays with fix (i) at every point and ceiling: 1.14 near the Sun at 4.5 and 1.02 at 5.0.

    - So rays with fix (i) are exact everywhere measured, and the cheaper by cost at every point
      and ceiling. T1's lean is rays with fix (i).
  - **For T2, pending the owner** (observations, not changes to T2).
    - **R13.T2.a.**
      - The bench's rule, `real_boundary_rule(at_ceiling: &[LayerCap], at_cut: Vec<LayerCap>)`,
        takes the caps at the ceiling, where Provides' `real_boundary(at_ceiling: &CapCount,
caps_at_cut: &[LayerCap])` takes their count. The rule is the same: for C to E, each ray
        of the cap at V<sub>P</sub> held within the cut's (`RayRadii::lesser`) and made a cap
        (`LayerCap::forced_by_ray`); A, B and the brown dwarfs at the cut's caps.
      - Fix (i)'s widened cones, if ruled, have a reference scan in the slow test's
        `widened_toward`.
      - Each layer's `expected_beyond` at the cut beyond R(u) is `CapCount::stars_beyond` on the
        cut's count.
      - T1's two claims hold at all six points. The count brighter than V<sub>P</sub> beyond R(u)
        is at most 0.992 at the caps' own count (0.972 recounted), and no ray of R(u) lies beyond
        the cut's cap.
      - The determinism audit noted, as a consider, that nothing pins the bits of the caps'
        `expected_beyond` today, and that `real_boundary`'s per-ray radii will reach the wire.
    - **R13.T2.b.** The served real tier near the Sun at the interim's V<sub>P</sub> 5.0 costs
      1.43 × 10⁵ CPU-s, about 2.7 h on 15 workers, not 11–14 minutes. The reply's counts beyond
      R(u) on the fixture at V<sub>P</sub> 5.0 are these:
      - at the eye's cut 8.18: C 1,660, D 622 and E 81 brighter than the cut, 2,363 in all (3.98%
        of the sky to the cut);
      - at 7.95, like for like with the study: C 993, D 419 and E 56, 1,468 in all (3.15%).

      At both cuts, 67 of them are brighter than V 6.5. The study's figures were 8,000–10,000, of
      them about 450 brighter than V 6.5.
  - **For the owner** (records, for the orchestrator to relay).
    - The guard has tripped at both ceilings, as above.
    - Design note 15's "the sky on by default once R13.T2 and R06.T11.d land" rested on the
      estimate of 11–14 minutes.
    - Design note 4's "near R06.T17's ruled 4,000 CPU-s target" does not hold: at 4.5 the real
      tier is 24–25 times that target.
    - The brainstorm's signed-off "The hybrid sky" puts the census at some thousands of
      CPU-seconds, which the measurement contradicts. Its "nine in ten naked-eye stars stay real"
      is conservative: 97.7% are measured near the Sun at 4.5.
    - **Answered by the owner on 2026-10-09** (`decision-r13-guard-trip.md`, option A): real stars
      to 2,000 ly, for RM3 and R13 alike, with T1.b first. Design notes 3, 4, 15 and 16 take the
      limit and its estimated cost. The brainstorm's three corrections (its cost, "within
      2,000 ly" on its promise, and nine in ten still holding at about 92–97%) are a draft for the
      owner, marked at the end of its subsection "The hybrid sky".
- **Deviations in T1.b, as built, and its measurements (2026-10-09).** Measured at
  `rendering-and-planets` f569fb08, on the fixture (`milky_way_like`, seed 0x0926_0000) and, for
  the bench, the server's galaxy (seed 0x4d2 with its full potential), both at the epoch. Every
  count is the caps' count's expectation, at the 3,072-ray recount unless marked "at the caps'
  count", with no census; every cost is a sampled census. **The guard has tripped** on its first
  count: near the Sun at 2,000 ly, 75.4 stars brighter than V 4.5 lie beyond R(u) at
  V<sub>P</sub> 4.5 and 74.8 at 5.0, C to E together, against the guard's 60. The other two hold:
  0.093 are expected brighter than V 1.0 (against 0.5), and the capped tier costs 0.85–1.10 times
  `decision-r13-guard-trip.md` §2.3's estimate (against 2). T2 waited for the owner, who kept the
  limit on 2026-10-09 (`decision-r13-t1b-guard.md`); its text is unchanged.
  - **The build.**
    - `sky::caps` gains the test-only `CapCount::cap_with_budget` (Provides). `cap` now runs the
      same rule as `cap_budgeted` at a budget of one, so its bits are unchanged (determinism
      audit); a unit test holds a budget of one to `cap` bit for bit, and each larger budget's
      rays at or within the smaller's, and two refuse a budget of none and a NaN.
    - `tests/sky_hybrid.rs` holds the slow test `the_capped_boundary_is_recorded` beside T1's,
      which share a helper that measures the six points on six threads. It took 333–378 s at
      `CPUQuota=400%`, and two runs printed the same rows. It asserts only T1.b's two claims, and
      both hold everywhere. Its nextest override takes 6 slots.
    - `benches/sky.rs` adds `sky/census_near_sun_limit/{eye,served_eye}_{4.5,5.0}`. Its census
      holds C to E within the limit by T1's `real_boundary_rule`, which takes an optional limit;
      T1's benches pass none and print as before.
  - **Deviations.**
    - **`RayRadii::within` is public** (Provides). The bench is outside the crate and needs it to
      hold R(u) within the limit. T2.a's `real_boundary` can use it as it is.
    - **The slow test needs the sim's `testing` feature**, for `cap_with_budget`. It is compiled
      only where the feature is on: in every workspace run (`just test-slow`, `just ci`), through
      `hyperion-fit`'s dependency, and with `--features testing`. A run of `-p hyperion-sim`
      alone leaves it out, and T1's test stays.
    - **The bench has no fix (i).** It scales C to E's CPU time by T1's fix (i) factors near the
      Sun, as the task asks. Within the limit those overstate it, since rays held at the limit
      widen nothing: the slow test's own factors at 2,000 ly near the Sun are C 1.006, D 1.009
      and E 1.006 at 4.5, and C 1.010, D 1.006 and E 1.005 at 5.0. With them the fixture's tier
      is 10,643 and 12,699 CPU-s instead of 10,964 and 13,141. T1 measured fix (i) on the
      fixture alone, so the server's galaxy takes the fixture's factors too.
    - **Samples.** The census took 1 block of cells in 10 on the fixture and 1 in 20 on the
      server's galaxy, finer than T1's 50 and 100, since the capped tier is a tenth of the work.
      C's census measures their spread. The limit holds none of C's rays, so C's plan is T1's on
      both galaxies, and only the sample differs. Against T1's, C's generated records are 0.94
      and 0.97 times on the fixture at 4.5 and 5.0, and 0.81 and 0.89 on the server's galaxy;
      its CPU time is 0.90, 0.96, 0.77 and 0.87 times. So the costs are good to about 10% on the
      fixture and 20% on the server's galaxy, and the cost's verdict, under half the guard, does
      not move. Its accepted stars are good only to a factor of a few: C's on the server's galaxy
      are 1.56 times T1's, and E's 12,920 against T1's 3,300 over a superset of its cells, from
      646 and 33 sampled stars.
    - **The costs are provisional, but clean.** The benches ran under the heavy-test lock after
      the integration `just ci` had finished, with 15 workers: the fixture's from a load of 4.2,
      the server's galaxy's from 12.4, raised by the fixture's run just before. The guard reads
      the job threads' CPU time. The review's fixes to the bench's report came after the run;
      they change nothing it prints on Linux.
    - **T1's test, moved onto the shared helper,** printed its 216 rows as before (396 s).
    - **The record is a digest.** It holds the near-Sun sums at the eye's cut for every limit, the
      per-layer rows near the Sun at 2,000 ly, the other points' sums at 2,000 ly, and the
      near-Sun budget sums. The slow test prints 1,440 rows and 36 guard lines (six points, three
      cuts, two ceilings, ten kinds of boundary, three layers and their sum), deterministically.
    - **Acceptance as run.** The slow test ran from its slow-test-profile binary, not through
      `just test-slow`, whose heavy lock the lane rules keep for timed runs and full suites.
      `cargo bench -p hyperion-sim --no-run` passed. `just ci` runs at integration.
  - **The boundary near the Sun** (fixture, the eye's cut V 8.179). R(u) does not depend on the
    cut there (no ray of the cut's caps holds it), so the counts brighter than V 1–6.5 are the same
    at 7.95 and 10.06. Each row is C to E together: the stars beyond R(u) brighter than each
    depth, the count brighter than V<sub>P</sub> at the caps' own count (what the layers'
    `bright_beyond` would sum to), the share of the whole sky beyond R(u) to V 6.5 and to the
    cut, and the systems within R(u) by ray, with fix (i) ÷ rays. The whole sky there holds
    11.4 stars brighter than V 1, 39.6 to V 2, 138 to V 3, 476 to V 4, 880 to V 4.5, 1,619 to
    V 5, 9,430 to V 6.5 and 59,350 to the cut.

    | V<sub>P</sub> | Limit (ly) | Beyond: V 1 / 2 / 3 / 4 / 4.5 / 5 / 6.5 / 8.18             | > V<sub>P</sub>, caps' count | Share to V 6.5 / the cut | Systems within (fix (i)) |
    | ------------- | ---------- | ---------------------------------------------------------- | ---------------------------- | ------------------------ | ------------------------ |
    | 4.5           | none       | 0.00002 / 0.010 / 0.077 / 0.42 / 1.65 / 7.2 / 218 / 5,470  | 1.95                         | 2.31% / 9.22%            | 1.104 × 10⁷ (1.044)      |
    | 4.5           | 1,000      | 0.503 / 3.02 / 14.9 / 61.0 / 123 / 251 / 1,950 / 22,900    | 130                          | 20.7% / 38.7%            | 1.29 × 10⁶ (1.000)       |
    | 4.5           | 2,000      | 0.093 / 0.86 / 6.09 / 34.3 / 75.4 / 161 / 1,130 / 10,500   | 81.9                         | 12.0% / 17.7%            | 3.849 × 10⁶ (1.007)      |
    | 4.5           | 4,000      | 0.0085 / 0.16 / 1.54 / 10.7 / 25.6 / 62.1 / 660 / 7,580    | 29.3                         | 7.00% / 12.8%            | 6.443 × 10⁶ (1.019)      |
    | 5.0           | none       | 0.00001 / 0.0002 / 0.039 / 0.20 / 0.43 / 1.27 / 67 / 2,360 | 1.62                         | 0.71% / 3.98%            | 1.925 × 10⁷ (1.045)      |
    | 5.0           | 1,000      | 0.503 / 3.02 / 14.9 / 61.0 / 123 / 251 / 1,950 / 22,900    | 267                          | 20.7% / 38.7%            | 1.29 × 10⁶ (1.000)       |
    | 5.0           | 2,000      | 0.093 / 0.85 / 6.06 / 34.2 / 74.8 / 157 / 1,070 / 8,580    | 172                          | 11.4% / 14.5%            | 6.628 × 10⁶ (1.009)      |
    | 5.0           | 4,000      | 0.0085 / 0.15 / 1.51 / 10.5 / 24.6 / 56.9 / 532 / 4,800    | 66.2                         | 5.65% / 8.09%            | 1.153 × 10⁷ (1.024)      |

    At 1,000 ly the limit holds every ray of C to E, so the ceiling changes only the count
    brighter than V<sub>P</sub>. At 7.95 the share to the cut at 2,000 ly is 16.3% and 13.5%; at
    10.06 it is 47.6% and 37.0%.

    Layer by layer at 2,000 ly, in the same terms, with the rays the limit holds of 1,536 and the
    gap at the cut by ray (with fix (i) it is 0 at every point, cut, ceiling and limit):

    | V<sub>P</sub> | Layer | R(u) median (10–90%), largest; held | Beyond: V 1 / 2 / 3 / 4 / 4.5 / 5 / 6.5 / 8.18          | > V<sub>P</sub>, caps' count | Systems within (fix (i)) | Gap   |
    | ------------- | ----- | ----------------------------------- | ------------------------------------------------------- | ---------------------------- | ------------------------ | ----- |
    | 4.5           | C     | 1,416 (1,287–1,416), 1,416; 0       | 0 / 0.010 / 0.070 / 0.25 / 0.83 / 4.08 / 99.8 / 3,540   | 0.896                        | 2.324 × 10⁶ (1.006)      | 2.54  |
    | 4.5           | D     | 2,000 (1,287–2,000), 2,000; 773     | 0 / 0.00001 / 0.015 / 0.69 / 2.81 / 8.06 / 183 / 3,010  | 3.02                         | 1.183 × 10⁶ (1.009)      | 0.562 |
    | 4.5           | E     | 2,000 (1,416–2,000), 2,000; 979     | 0.093 / 0.85 / 6.01 / 33.4 / 71.7 / 148 / 848 / 3,950   | 77.9                         | 3.425 × 10⁵ (1.006)      | 0.063 |
    | 5.0           | C     | 1,758 (1,758–1,934), 1,934; 0       | 0 / 0.0001 / 0.038 / 0.17 / 0.32 / 0.76 / 43.5 / 1,660  | 0.854                        | 5.020 × 10⁶ (1.010)      | 2.95  |
    | 5.0           | D     | 2,000 (1,597–2,000), 2,000; 942     | 0 / 0.000001 / 0.014 / 0.68 / 2.79 / 8.00 / 182 / 2,970 | 8.60                         | 1.259 × 10⁶ (1.006)      | 0.212 |
    | 5.0           | E     | 2,000 (1,597–2,000), 2,000; 1,001   | 0.093 / 0.85 / 6.01 / 33.4 / 71.7 / 148 / 848 / 3,940   | 162                          | 3.493 × 10⁵ (1.005)      | 0.037 |

  - **The guard's figures**, near the Sun at 2,000 ly, C to E:
    - **brighter than V 4.5: 75.4 at V<sub>P</sub> 4.5 and 74.8 at 5.0, against 60: tripped.**
      E holds 71.7 of them, D 2.8 and C 0.8. They are 8.6% of the sky's 880, against the
      record's estimate of 10–60 (§2.4: "D adds 1–2, C none"). E's rays reach past 2,000 ly on
      979 of 1,536 rays at 4.5, and between 2,000 ly and E's own R(u) lie 71.5 of its stars
      brighter than V 4.5 (0.25 beyond R(u) alone).
    - brighter than V 1.0: 0.093 at either ceiling, against 0.5. The brightest is expected at
      about V 2.1: 0.86 lie beyond brighter than V 2, and 6.1 brighter than V 3 (the record's
      V 1.5–3).
    - the cost: 0.85–1.10 times §2.3's estimate, against 2 (below).
  - **The capped tier's cost** (the bench, near the Sun, the eye's cut). The cost is in CPU-s by
    the job threads' CPU time, each layer's census scaled by the sample, with its generated
    records; the whole includes the two caps' counts and the plan, 23–25 s. "With fix (i)" scales
    C to E by T1's factors; "On 15" and "On 4" divide it by the workers, ignoring imbalance.

    | Galaxy, cut     | V<sub>P</sub> | C                  | D                   | E                   | Whole  | With fix (i) | §2.3's estimate | Times it  | On 15 / on 4     |
    | --------------- | ------------- | ------------------ | ------------------- | ------------------- | ------ | ------------ | --------------- | --------- | ---------------- |
    | Fixture, 8.179  | 4.5           | 1,455 (5.23 × 10⁵) | 5,589 (7.18 × 10⁵)  | 3,492 (3.15 × 10⁵)  | 10,560 | 10,964       | 10,000–12,000   | 0.91–1.10 | 12.2 / 46 min    |
    | Fixture, 8.179  | 5.0           | 3,208 (1.12 × 10⁶) | 5,836 (7.47 × 10⁵)  | 3,543 (3.19 × 10⁵)  | 12,612 | 13,141       | 13,000–15,000   | 0.88–1.01 | 14.6 / 55 min    |
    | Server's, 7.766 | 4.5           | 3,066 (1.07 × 10⁶) | 10,578 (1.36 × 10⁶) | 10,128 (9.27 × 10⁵) | 23,798 | 24,783       | 27,000–29,000   | 0.85–0.92 | 27.5 min / 1.7 h |
    | Server's, 7.766 | 5.0           | 5,592 (1.93 × 10⁶) | 11,135 (1.43 × 10⁶) | 10,260 (9.34 × 10⁵) | 27,012 | 28,238       | 31,000–33,000   | 0.86–0.91 | 31.4 min / 2.0 h |
    - The bench's R(u) is the slow test's, from the same count at V<sub>P</sub>: on the fixture
      the limit holds 773 and 942 of D's rays and 979 and 1,001 of E's; on the server's galaxy
      510 and 677 of D's and 936 and 966 of E's; and none of C's on either. The server's galaxy's
      R(u) by ray at 4.5 is C 1,386 (1,146–1,386), 1,386; D 1,677 (1,260–2,000), 2,000; and
      E 2,000 (1,386–2,000), 2,000. At 5.0 it is C 1,720 (1,421–1,893), 1,893; D 1,893
      (1,292–2,000), 2,000; and E 2,000 (1,564–2,000), 2,000.
    - Per generated record it costs C 2.8, D 7.8 and E 11.1 ms on the fixture at 4.5, T1's own
      2.9, 7.9 and 11.5: the limit cuts the records, not their cost. D now takes 53% of it and E
      33%, against E's 85% at R(u) alone.
    - The server's galaxy costs 2.25 and 2.14 times the fixture's, inside the record's 2–2.5.
    - Each layer's records past the floor, generated records, accepted stars and accepted stars
      within R(u) towards their band texel (those R13.T2.a lists), scaled by the sample (the
      stars good only to a factor of a few, above):

      | Galaxy   | V<sub>P</sub> | C                                         | D                                         | E                                         |
      | -------- | ------------- | ----------------------------------------- | ----------------------------------------- | ----------------------------------------- |
      | Fixture  | 4.5           | 2.41 × 10⁶ / 5.23 × 10⁵ / 25,750 / 25,510 | 1.26 × 10⁶ / 7.18 × 10⁵ / 14,940 / 14,750 | 3.66 × 10⁵ / 3.15 × 10⁵ / 2,640 / 2,540   |
      | Fixture  | 5.0           | 5.32 × 10⁶ / 1.12 × 10⁶ / 27,370 / 27,250 | 1.32 × 10⁶ / 7.47 × 10⁵ / 14,940 / 14,790 | 3.70 × 10⁵ / 3.19 × 10⁵ / 2,650 / 2,550   |
      | Server's | 4.5           | 4.89 × 10⁶ / 1.07 × 10⁶ / 32,980 / 32,920 | 2.40 × 10⁶ / 1.36 × 10⁶ / 25,440 / 25,360 | 1.07 × 10⁶ / 9.27 × 10⁵ / 12,920 / 12,820 |
      | Server's | 5.0           | 9.01 × 10⁶ / 1.93 × 10⁶ / 33,960 / 33,940 | 2.53 × 10⁶ / 1.43 × 10⁶ / 25,460 / 25,400 | 1.08 × 10⁶ / 9.34 × 10⁵ / 12,920 / 12,820 |

    - Against R(u) alone (T1, by CPU time): 9.0 and 11.5 times cheaper on the fixture, 4.3 and
      5.3 on the server's galaxy, at 4.5 and 5.0.

  - **The other points** at 2,000 ly, at each point's eye's cut, C to E, at V<sub>P</sub> 4.5
    and then 5.0: the stars beyond R(u) brighter than V 4.5 and than V 1, the share of the whole
    sky beyond R(u) to V 6.5, the systems within R(u) by ray, and their cost at T1's near-Sun
    costs per system, C 0.69, D 5.41 and E 12.65 ms (_estimate_; near the Sun it gives 1.23 and
    1.47 × 10⁴ against the bench's 1.06 and 1.26 × 10⁴, by ray).

    | Point (eye's cut)                    | > V 4.5     | > V 1         | Share to V 6.5 | Systems within         | CPU-s (_estimate_)   |
    | ------------------------------------ | ----------- | ------------- | -------------- | ---------------------- | -------------------- |
    | Near the Sun (8.179)                 | 75.4; 74.8  | 0.093; 0.093  | 12.0%; 11.4%   | 3.85 × 10⁶; 6.63 × 10⁶ | 1.2 × 10⁴; 1.5 × 10⁴ |
    | Nuclear disc (5.807)                 | 1.90; 0.567 | 0.002; 0.0003 | 0.26%; 0.09%   | 1.10 × 10⁶; 1.53 × 10⁶ | 5.7 × 10³; 8.1 × 10³ |
    | Solar circle, a quarter turn (8.109) | 49.4; 48.9  | 0.053; 0.053  | 9.58%; 9.00%   | 3.27 × 10⁶; 5.45 × 10⁶ | 1.2 × 10⁴; 1.4 × 10⁴ |
    | Solar circle, far side (7.730)       | 115; 114    | 0.278; 0.278  | 5.82%; 5.51%   | 5.52 × 10⁶; 9.19 × 10⁶ | 1.7 × 10⁴; 2.0 × 10⁴ |
    | Inner disc (6.722)                   | 423; 423    | 0.397; 0.397  | 5.24%; 5.11%   | 5.71 × 10⁷; 8.91 × 10⁷ | 1.6 × 10⁵; 1.9 × 10⁵ |
    | 2,000 ly above the Sun (8.540)       | 123; 122    | 0.172; 0.172  | 71.1%; 69.7%   | 1.13 × 10⁶; 2.89 × 10⁶ | 3.9 × 10³; 5.4 × 10³ |

    In the nuclear disc the limit holds no ray, and each layer's count brighter than
    V<sub>P</sub> beyond R(u) is under one (T2.a's claim there). In the inner disc and 2,000 ly
    above the Sun the limit holds C too at 5.0 (539 and 340 rays). Above the Sun the disc lies
    beyond 2,000 ly, so seven tenths of its naked-eye sky is beyond. Four of the six points pass
    60 stars brighter than V 4.5 beyond 2,000 ly, though the guard is read near the Sun alone.

  - **The budgets for D and E** (for the record only, near the Sun at the eye's cut, C at T7.b's
    budget of one). C to E as above, with the cost at T1's costs per system (_estimate_):

    | V<sub>P</sub> | Limit (ly) | Budget | D R(u) median (10–90%) | E R(u) median (10–90%), largest | > V 1   | > V 4.5 | Share to V 6.5 | Systems within | CPU-s (_estimate_) |
    | ------------- | ---------- | ------ | ---------------------- | ------------------------------- | ------- | ------- | -------------- | -------------- | ------------------ |
    | 4.5           | none       | 1      | 2,075 (1,287–2,763)    | 2,511 (1,416–7,177), 9,557      | 0.00002 | 1.65    | 2.31%          | 1.10 × 10⁷     | 9.5 × 10⁴          |
    | 4.5           | none       | 3      | 1,558 (1,170–2,283)    | 2,283 (1,287–5,930), 8,687      | 0.0001  | 3.54    | 3.07%          | 7.95 × 10⁶     | 6.3 × 10⁴          |
    | 4.5           | none       | 10     | 1,170 (967–1,558)      | 1,886 (1,063–4,453), 7,177      | 0.0014  | 11.8    | 5.66%          | 5.56 × 10⁶     | 3.9 × 10⁴          |
    | 4.5           | none       | 30     | 799 (660–967)          | 1,416 (879–3,039), 5,930        | 0.018   | 41.5    | 13.0%          | 3.77 × 10⁶     | 1.9 × 10⁴          |
    | 4.5           | 2,000      | 1      | 2,000 (1,287–2,000)    | 2,000 (1,416–2,000), 2,000      | 0.093   | 75.4    | 12.0%          | 3.85 × 10⁶     | 1.2 × 10⁴          |
    | 4.5           | 2,000      | 3      | 1,558 (1,170–2,000)    | 2,000 (1,287–2,000), 2,000      | 0.093   | 75.6    | 12.1%          | 3.67 × 10⁶     | 1.1 × 10⁴          |
    | 4.5           | 2,000      | 10     | 1,170 (967–1,558)      | 1,886 (1,063–2,000), 2,000      | 0.094   | 80.6    | 13.8%          | 3.09 × 10⁶     | 7.9 × 10³          |
    | 4.5           | 2,000      | 30     | 799 (660–967)          | 1,416 (879–2,000), 2,000        | 0.103   | 100     | 19.4%          | 2.72 × 10⁶     | 5.6 × 10³          |
    | 5.0           | none       | 10     | 1,597 (1,089–2,129)    | 1,934 (1,198–5,548), 8,139      | 0.0004  | 3.85    | 2.72%          | 1.01 × 10⁷     | 5.9 × 10⁴          |
    | 5.0           | none       | 30     | 1,198 (899–1,451)      | 1,597 (989–4,581), 7,396        | 0.0035  | 13.9    | 5.72%          | 7.89 × 10⁶     | 3.7 × 10⁴          |
    | 5.0           | 2,000      | 30     | 1,198 (899–1,451)      | 1,597 (989–2,000), 2,000        | 0.095   | 81.9    | 13.7%          | 5.69 × 10⁶     | 9.2 × 10³          |

    A budget drops the poorest intervals first, so for its cost it leaves fewer bright stars
    beyond than a distance does: at 4.5 a budget of 10 with no limit leaves 11.8 brighter than
    V 4.5 for about 3.9 × 10⁴ CPU-s, and a limit of 4,000 ly leaves 25.6 for about 3.7 × 10⁴.
    It holds no distance: D's R(u) falls to 660–967 ly at a budget of 30, inside the first
    drive's 1,000 ly.

  - **Against the estimates** (`decision-r13-guard-trip.md` §2.3, §2.4; Design notes 3, 4, 15
    and 16).
    - **The cost**: 0.85–1.10 times §2.3's, on both galaxies at both ceilings; the arithmetic held.
    - **The bright stars beyond 2,000 ly**: 75 brighter than V 4.5 against 10–60; D adds 2.8
      against 1–2, and C none, as estimated. Brighter than V 5.0: 157–161 against 20–100. The
      brightest at about V 2.1, inside the estimated V 1.5–3.
    - **The naked-eye share**: 12.0% of the stars brighter than V 6.5 lie beyond R(u) at 4.5 and
      11.4% at 5.0, against 3–8%. So 88.0–88.6% of the naked-eye stars are real near the Sun,
      against the plan's and the brainstorm draft's 92–97% ("nine in ten" barely misses).
    - **At 1,000 ly**: 123 brighter than V 4.5 (against 60–120), 0.503 brighter than V 1.0, the
      brightest at about V 1.4 (against V 0.5–1.5), and 20.7% of the naked-eye stars (against
      12–18%), at about 2.6 × 10³ CPU-s (_estimate_; §2.3's 2.4–2.5 × 10³).
    - **The camera**: at 10.06, 52.4% of its stars are real at 4.5 and 63.0% at 5.0 (the record's
      about 55% and two thirds).
    - **The inner disc** at the limit: about 1.6–1.9 × 10⁵ CPU-s (_estimate_, the record's
      2–3 × 10⁵), with 423 stars brighter than V 4.5 beyond; at 1,000 ly about 3.2 × 10⁴ (the
      record's 3–4 × 10⁴), with 1,410.
  - **For T2, pending the owner** (observations, not changes to T2).
    - **R13.T2.a.**
      - `RayRadii::within` is public, so `real_boundary` holds each ray within `REAL_LIMIT_LY`
        with it after `RayRadii::lesser`, as the bench's `real_boundary_rule` does.
      - Fix (i) at the limit opens 0.7–0.9% more C to E systems near the Sun, against 4.4–4.5%
        at R(u) alone, and still closes a gap of 3.2 stars at 4.5 (C 2.54, D 0.56, E 0.06) and 3.2
        at 5.0 to 0. So T2.a's "no gap" test needs the widened cones over the limit's rays too.
      - Each layer's `bright_beyond` is `stars_beyond` of R(u) on the count at V<sub>P</sub>:
        near the Sun on the fixture C 0.896, D 3.02 and E 77.9 at 4.5, and C 0.854, D 8.60 and
        E 162 at 5.0. It is never the forced cap's `expected_beyond`, which `forced_by_ray` sets
        to 0.
      - Where the limit holds no ray (the nuclear disc) each layer's count brighter than
        V<sub>P</sub> beyond R(u) is under one, as T2.a's test asks.
      - The determinism audit asks T2.a to pin: a golden of R(u)'s per-ray bits for C to E at a
        coarse count with the ceiling and the limit, with each layer's `bright_beyond` and
        `expected_beyond` bits (the "equals" test by bits, not a tolerance); a partial census
        through the 2,000 ly shell with rays held exactly at that edge, since `REAL_LIMIT_LY` is
        `SHELL_EDGES_LY[4]` and `all_reach` and the shell walk decide at equality; and `lesser`
        then `within` against `within` then `lesser`, bit for bit. No golden pins any cap's
        radii today.
    - **R13.T2.b.** The served real tier near the Sun at the interim's V<sub>P</sub> 5.0 costs
      about 1.3 × 10⁴ CPU-s on the fixture and 2.8 × 10⁴ on the server's galaxy, 15 and 31
      minutes on 15 workers (55 minutes and 2.0 h on 4). Its band holds 11.4% of the naked-eye
      stars near the Sun (1,070 brighter than V 6.5), 14.5% of the eye's, and 37.0% of a
      camera's at 10.06. The reply's `bright_beyond`, C to E, is 172 at 5.0 near the Sun.
  - **For the owner** (records, for the orchestrator to relay; no change to T2 here).
    - **T1.b's guard has tripped** on its first count: 75.4 and 74.8 stars brighter than V 4.5
      lie beyond R(u) near the Sun at 2,000 ly, against 60. The other two counts hold.
    - The guard is read near the Sun, but at 2,000 ly the far side of the solar circle (115),
      2,000 ly above the Sun (123) and the inner disc (423, with 0.40 brighter than V 1.0) pass
      60 too; a quarter turn round holds 49.
    - The plan's and the brainstorm draft's "about 92–97% of naked-eye stars stay real" at the
      limit is 88.0–88.6% measured.
    - Recorded beside the limit for a ruling: a limit of 4,000 ly (25.6 brighter than V 4.5,
      7.0% of the naked-eye stars, about 3.7 × 10⁴ CPU-s at 4.5), and a budget of 10 or 30 for
      D and E with no limit (11.8 or 41.5, 5.7% or 13.0%, about 3.9 or 1.9 × 10⁴), each an
      _estimate_ of cost at T1's costs per system near the Sun.
    - **Answered by the owner on 2026-10-09** (`decision-r13-t1b-guard.md`, option A, "Keep
      2,000 ly"): the limit is kept. The measured figures replace the estimates in Design notes 3,
      4, 15 and 16 and in the brainstorm's draft.
- **Deviations in T2.a, as built (2026-10-09).** Built on the lane at 19605fa0, with
  `rendering-and-planets` merged at 1d7a6bd7 and dd71c4eb, whose changes touch no sky path, on
  the fixture (`milky_way_like`, seed 0x0926_0000), at the epoch. No
  generated output moves and `GENERATOR_VERSION` stays 21. One golden is new,
  `sky/real_boundary.golden`.
  - **The build.**
    - `sky::caps` gains `REAL_LIMIT_LY`, `real_boundary` and `LayerCap::bright_beyond` (`None` for
      every cap but a real boundary's), and, crate-visible, `REAL_BOUNDARY_LAYERS` (C, D, E),
      `real_boundary_of` (`census_plan`'s caps at a ceiling) and `RayCones`, the cones a walk opens
      cells by, with `RayRadii::cones_widened_by`. R06.T7.b's cone and index constants moved from
      `CapLattice` into `RayCones`: widened by none they are R06's, bit for bit (a unit test pins
      them); widened by `INDEX_BALL_RAD` or more, no ball goes through the index.
    - `sky::census` gains `SkyQueryBuilder::synthetic_ceiling`, `SkyQuery::synthetic_ceiling`,
      `BuildSkyQueryError::SyntheticCeiling` and `Completeness::lists_by_texel`. Forced caps keep
      the ceiling.
      - At a ceiling `census_plan` measures the caps' rays once, counts them at the cut (by the
        eye's visibility where asked) and at the ceiling, and takes `real_boundary` of the two.
        Its counts are `layer_caps`' and `layer_caps_by_visibility`'s, bit for bit (two tests).
      - `census_plan_of` and `census_plan_with_edges` (the server's plan) widen C's, D's and E's
        cones by the query's band's ρ wherever the query states a ceiling, and list them by the
        radius towards each star's texel at every reply, the final one included.
    - `merge.rs` changes in its docs only: a census at a ceiling is merged by `merge_shells`, whole
      too, to its plan's `complete()`. `cell.rs` is unchanged; a cell's census reads no cap.
  - **Deviations.**
    - **`real_boundary` takes the cut's count too:**
      `(at_ceiling: &CapCount, at_cut: &CapCount, caps_at_cut: &[LayerCap])`, where Provides had
      `(at_ceiling, caps_at_cut)`. Each C–E cap's `expected_beyond` is `at_cut`'s `stars_beyond`
      beyond R(u) (T1.b's "For T2"), which the caps alone cannot give. `caps_at_cut` stays, so
      that a server that draws a cap a job (R06.T11.g) does not draw them twice; debug builds check
      that they are `at_cut`'s. A real cap keeps its cut's rule bound.
    - **The widening is the plan's.** Provides put fix (i)'s cones in `real_boundary`. As built
      the caps carry R(u) alone, and the plan widens C to E wherever the query states a ceiling,
      by its own band's ρ (1.27° at 64²), the band its listing reads.
    - **`bright_beyond` is stated for A, B and the brown dwarfs too:** their count brighter than
      V<sub>P</sub> beyond their caps at the cut (A 3.5 × 10⁻⁵, B 0 and the brown dwarfs 3 × 10⁻⁹
      at the golden's coarse count at 4.5). Their caps are otherwise the cut's, bit for bit. A reply
      states `bright_beyond` per layer (Protocol).
    - **`band.rs`, `limits.rs`, the bench and `.config/nextest.toml` are touched**, though not in
      the task's Files.
      - `sum_rows` requires the census's band and observer wherever `lists_by_texel` holds, which
        with no ceiling is "not yet final" exactly. So a final census at a ceiling is summed at its
        own band too, and a test refuses it at another.
      - The conservation test at the new radii is in `sky::band`'s tests beside T9.b's, outside the
        acceptance's filters (run by name below).
      - `limits.rs` gains the test-only `EyeVisibility::of_limits`, a visibility that varies over
        the sky, for the test of the visibility's path.
      - `benches/sky.rs` takes the sim's `REAL_LIMIT_LY` in place of its own copy, so the limit is
        one constant. Its `real_boundary_rule` stays, for T1's benches without the limit.
      - The two census tests on four threads join R06.T8.i's `threads-required = 4` override.
    - **The no-gap oracle is in the crate.** It runs `generate_cell` and
      `census_record(…, Bound::Ignored, …)` over every cell, as `brute_force_sky` does:
      `brute_force_sky` is an integration-test helper, the acceptance's filter `sky::census` runs
      unit tests, and integration tests cannot build radii by ray. Its geometry is scaled. Rays
      reach 150 ly on one side of a tilted plane, held at a limit of 140 ly, and 40–60 ly
      elsewhere, to V 11, on a band of 2² texels (ρ about 35°). At such radii a cell's ball is wider than a 64² texel, so the
      gap cannot arise at the standard band. R06's cones there miss 5 stars, and the widened ones
      none, of 1,025 within R(u) towards texels at the limit and 147 below it. The index's branch
      at the 64² ρ is held by R06.T7.b's ball test against a scan, widened by 0, ρ, 5° and 7°.
    - **The 2,000 ly claims are tested on the plan, not on a census**, which no unit test can
      afford there. The census tests scale the limit: 100 ly (shells to 40 and 70 ly, then R(u),
      as `REAL_LIMIT_LY` is `SHELL_EDGES_LY[4]`), 140 ly (no gap) and 150 ly (conservation, against
      R06's census with every layer at 200 ly, standing for the cut's caps). At 2,000 ly with
      `SHELL_EDGES_LY`, rays from 1,000 to 2,500 ly, some exactly at 1,000, held at the limit, give
      C to E shells to 125, 250, 500 and 1,000 ly, then to R(u), with no 2,000 ly shell of their
      own; done to 1,000 ly each is complete to it in every direction; and every reply is complete
      within the limit towards 10⁴ directions. So too with every ray held at the limit.
    - **R(u)'s claims are tested at the cut 7.95**, near the Sun and in the nuclear disc at
      (0, 150, 0) ly, at the standard count: R(u) does not depend on the cut there (T1).
  - **Measured** (the fixture, cut 7.95, the standard count; the figures are T1.b's).
    - Near the Sun the limit holds 773 and 942 of D's 1,536 rays and 979 and 1,001 of E's at
      V<sub>P</sub> 4.5 and 5.0, and none of C's. `bright_beyond` is C 0.896, D 3.025 and E 77.94
      at 4.5 and C 0.854, D 8.601 and E 162.4 at 5.0 (171.9 C to E); `expected_beyond` at 7.95 is
      C 2,365, D 2,279 and E 3,519 at 4.5 and C 1,059, D 2,250 and E 3,517 at 5.0.
    - In the nuclear disc the limit holds no ray, and each layer's count brighter than
      V<sub>P</sub> beyond R(u) is under one: C 0.848, D 0.664 and E 0.656 at 4.5, and C 0.808,
      D 0.633 and E 0.628 at 5.0.
    - Conservation: listed, overflow and band light at R(u) is +0.26% against R06's at the cut's
      caps (9.146 against 9.123 × 10⁻⁴ lx), the list's light falling from 7.61 to 5.53 × 10⁻⁵ lx.
    - The widened cones open 2.5%, 1.2% and 0.3% more of C's, D's and E's cells for rays between
      400 and 1,500 ly held at 1,000 ly (a plan's count, not the server's).
  - **Acceptance as run.** `cargo test -p hyperion-sim --lib` filtered to `sky::caps`,
    `sky::census` and `sky::band` passed: 142 tests, 4 slow ones ignored, in 412 s.
    The slow tests ran from their slow-test-profile binaries, capped at `CPUQuota=400%`, not
    through `just test-slow`, whose heavy lock the lane rules keep for timed runs and full suites,
    on the tree before the review's fixes, which change no path they run:
    `the_census_is_its_oracle_1000_ly_from_the_sun` passed in 1,520 s and `caps_converge_in_rays`
    passed in 660 s. `cargo bench -p hyperion-sim --no-run` and
    `cargo test -p hyperion-server --test sky` passed. `just ci`, with the wasm32-wasip1 run of the
    new golden, runs at integration.
  - **For T2.b.**
    - The server's second count goes through `CapCount::plan_over` at V<sub>P</sub> over the cut
      count's `RayExtinctions`, then `real_boundary(&at_ceiling, &at_cut, &caps_at_cut)` in one
      job, and `census_plan_with_edges` on a query that states the ceiling.
    - Every reply, the final one too, is merged by `merge_shells` to its plan's completeness. A
      census at a ceiling merged by `merge_census` would list C to E beyond R(u); the server's two
      one-pass `merge_census` callers (`compute/sky.rs`, `requests/sky.rs`, both in tests) stay
      off the ceiling.
    - The builder refuses a ceiling within 0.5 mag of the cut. Where a request's cut is shallower
      than V<sub>P</sub> + 0.5 (5.5 in RM3, 5.0 from T7; the nuclear disc's eye's cut is 5.81), T2.b
      decides what the server states (asked of "main"; lean: no ceiling there, so the reply's
      `synthetic_ceiling_v` is absent and the cut's own caps, then some tens to hundreds of ly,
      bound the census). _Ruled 2026-10-09 (`decision-r13-t2b-ceiling.md`): no margin. The server
      states V<sub>P</sub>, or the cut where the cut is brighter, and none where the cut is at or
      brighter than V<sub>P</sub> and its C–E caps lie within the limit (R13.T2.b)._
- **The ceiling at a shallow cut** (ruled 2026-10-09, `decision-r13-t2b-ceiling.md`, for
  R13.T2.b). T2.a's builder refused a ceiling within 0.5 mag of the cut. A sky whose cut is
  shallower than V<sub>P</sub> + 0.5 (5.5 in RM3, 5.0 from T7) would then have been served at
  R06's caps, past the real limit. The margin had no source or measurement: it came with the
  plan's first draft, when a ceiling only opened a window between V<sub>P</sub> and the cut.
  Nothing in R(u)'s rule degenerates as V<sub>P</sub> nears the cut. At V<sub>P</sub> = cut,
  R(u) is the cut's caps held at the limit and C–E's `bright_beyond` is their `expected_beyond`;
  the census reads only whether a ceiling is stated.
  - **Near the Sun** such cuts come from requests the protocol allows and the client does not
    send: an eye of field factor above about 11–17, or a camera's limit under 5.5. Without a
    ceiling they would cost R06's census to the caps at V 5–5.5, whose E rays reach
    11,939–19,416 ly. That is about 1–2 × 10⁵ CPU-s (_estimate_ from T1's 1.45 × 10⁵ at the caps
    at V 5.0), against at most the interim's 1.31 × 10⁴ (2.82 × 10⁴ on the server's galaxy): 4–15
    times, for a sky that sees less.
  - **In the nuclear disc** the eye's own cut is shallow, nearer the centre than T1's point
    (5.807 there). The stakes are small. At a cut 0.31 mag deeper than the ceiling (proxy: T1's
    V<sub>P</sub> 5.5 row at 5.807) the ceiling keeps 82% of the cut's C–E systems. It leaves 3.2
    stars brighter than the cut to the band, of the sky's 23,795. It costs about 1.2 × 10⁴ CPU-s
    against R06's 1.4 × 10⁴ (_estimates_ at T1's near-Sun costs per system).
  - **So T2.b** states V<sub>P</sub>, or the cut where the cut is brighter, and refuses only a
    ceiling fainter than the cut. It states none where the cut is at or brighter than
    V<sub>P</sub> and the cut's C–E caps lie within the limit on every ray; that reply is R06's,
    exact, with no ceiling fields and no note. No served sky's C–E census reaches past
    `REAL_LIMIT_LY`, and the eye and every camera keep one partition at every cut deeper than
    V<sub>P</sub>.
  - **Rejected:** a ceiling clamped to cut − 0.5, which would pull R(u) inside the cap at
    V<sub>P</sub> and break "every star brighter than V<sub>P</sub> within 2,000 ly is real"; the
    lean (no ceiling below V<sub>P</sub> + 0.5), except in the exact form above; and a smaller
    margin above zero.
  - **Ruled 2026-10-09** (`decision-r13-t2b-note.md`): `DISTANT STARS: DETAIL LIMITED`, true in
    all three regimes, since it names no figure and no brightness. Its row names the ceiling,
    R(u)'s rule and the limit. At a ceiling, with an instrument open, the primary's final `L · I`
    is one line shorter than `L · S`. No reading exceeds the 4 lines measured.
- **Deviations in T2.b, as built (2026-10-09).** Built on the lane with `rendering-and-planets`
  merged at ce4e73e5 and 914400b0, whose changes touch no sky path. No generated output moves:
  `GENERATOR_VERSION` stays 21, no golden moves, and `PROTOCOL_VERSION` stays 2.
  - **The build.**
    - The sim gains `sky::caps::{SYNTHETIC_CEILING_V, served_ceiling}` and
      `SkyQuery::with_synthetic_ceiling`. The builder refuses a ceiling only if it is not finite
      or is fainter than the cut (`ceiling_fits`), and `MIN_CEILING_DEPTH_MAG` is gone. Its test
      is `a_synthetic_ceiling_is_finite_and_no_fainter_than_the_cut`, and the ruling's
      `the_served_ceiling_holds_the_limit_at_every_cut` is beside T2.a's in `sky::caps`.
    - The server's `compute::sky::plan` takes the request's query, which states no ceiling, and
      returns it at the ceiling it is served at, with its plan. Where the cut is deeper than
      V<sub>P</sub>, both counts are planned in one job, and the ceiling's rays are queued behind
      the cut's. Otherwise the ceiling's count is planned once the cut's caps show that a ceiling
      at the cut holds some ray (`counted_at`). `real_boundary` runs in one job, and every reply
      merges by `merge_shells`, as T2.a left it.
    - The reply states `synthetic_ceiling_v`, `real_limit_ly` and each layer's `bright_beyond`
      (`LayerCap::bright_beyond`), all optional, read absent as no ceiling, and written with no
      key without one. There is no second per-ray table (`decision-r13-t2b-note.md` §5).
    - The client has `SKY_DETAIL_LIMITED_NOTE`, the note kind `detail-limited`,
      `detailLimited(response)` (`synthetic_ceiling_v` present) and
      `SkyLineStanding.detailLimited`.
    - The guide gains the new row, the two "Views" edits and the `INTEGRATED STARLIGHT` clause,
      each marked drafted. Prettier re-padded the whole table for the new row, which is wider than
      any before it.
  - **Deviations.**
    - **`SYNTHETIC_CEILING_V` is the sim's `sky::caps` constant**, an `f64` as Provides has it,
      not yet in T5.a's `sky::synthetic`, so that the server and the sim bench read one constant
      (the plan-conformance review). The server serves it as `compute::sky::SYNTHETIC_CEILING`.
      T5.a moves it into `sky::synthetic`, and T7 sets it to 4.5.
    - **`SkyQuery::with_synthetic_ceiling` is new and public** (Provides). The server knows the
      ceiling only once it holds the cut's caps, and by then the query is built. The builder and
      this method share `ceiling_fits`.
    - **Forced caps take the served ceiling too.** A query that states none gets `served_ceiling`
      over its forced caps, each counted as its one radius, and a query that states one keeps it.
      A forced cap stands in for R(u): it is not held at the limit and states no `bright_beyond`.
      So a forced cap past 2,000 ly at a ceiling is censused past the `real_limit_ly` its reply
      states. That is the test seam only.
    - **The ceiling at the cut is counted again**, not taken from the cut's count, even at a
      uniform cut where the two are one count. That rare path keeps the one code path that the
      server galaxy's test holds bit for bit to the sim (the reviews' "consider"; about one caps
      count, 10–20 CPU-s).
    - **The server's tests run at the ceiling over forced caps.** Every sky in `tests/sky.rs`
      states V 5.0, and the sim's census each test holds a reply to is merged to its plan's
      completeness (`merge_shells`). So "a sky near the Sun returns the sim's census at the ceiling
      bit for bit" holds over forced 30 ly caps, with the galaxy's own tables in R06.T11.c's test.
      The derived caps at the ceiling are held to the sim's `census_plan` on the plan:
      `at_the_ceiling_the_caps_counts_in_jobs_are_the_sims_real_boundary` builds the server galaxy's
      tables, plans 7.95 (the early path) and 4.6 (`counted_at`, its ceiling 4.6, each C–E
      `bright_beyond` its `expected_beyond` bit for bit), and joins the `sky-tables` nextest group
      with 4 slots. A census to the derived caps is about 10⁴ CPU-s, beyond any test.
    - **R06's reply is answered at 4.6 and at 5.0** over forced 30 ly caps
      (`a_shallow_sky_within_the_limit_is_served_as_r06s`). Over tables of no star, 4.6 and 8.0 are
      planned in `compute::sky`'s unit tests.
    - **The reply's fields and the served rule are unit tests**, in `requests::sky`
      (`the_reply_states_the_ceiling_the_limit_and_each_layers_boundary`) and `compute::sky`
      (`the_served_ceiling_is_the_lesser_of_the_servers_and_the_cut`). `--test sky` does not run
      them, so they ran by name (below).
    - **Files touched though not in the task's list:** the server's `compute/mod.rs`; the
      protocol's `envelope.rs` (a test literal); `displays/view/useViewSky.ts` and its test, since
      the hook builds `SkyLineStanding`; `benches/sky.rs`; `.config/nextest.toml`; and R06's
      Design note 9.
    - **The bench is the eye's alone**, `sky/census_near_sun_served/{eye,served_eye}_5.0`. It is
      planned by the sim's `census_plan` at the ceiling, which the server's jobs match bit for bit,
      with fix (i) built in, so no factor scales it. Its census is merged as the server merges one
      at a ceiling (`census_of_plan` now takes `merge_shells` wherever the query states one). It
      is recorded, not gated (`Judged::guards`). T1 measured the camera's tier at 0.98–1.00 times
      the eye's.
  - **Measured.**
    - **`the_served_ceiling_holds_the_limit_at_every_cut`** (the fixture, uniform cuts, counts
      beyond R(u)).
      - Near the Sun at 4.6 the ceiling is 4.6. The limit holds 787 of D's rays and 968 of E's,
        and `bright_beyond` equals `expected_beyond`: C 0.904, D 3.775 and E 90.97.
      - At 5.0 the ceiling is 5.0: C 0.854, D 8.601 and E 162.43, 171.9 C to E.
      - At 5.4 (refused by T2.a) the ceiling is 5.0, with the same counts brighter than it, and
        brighter than the cut C 3.56, D 19.3 and E 278.
      - In the nuclear disc there is no ceiling at 4.6 or 5.0, since the cut's caps lie within
        the limit. At 5.4 the ceiling is 5.0, the limit holds no ray, and brighter than it C 0.810,
        D 0.637 and E 0.628 lie beyond (C 1.56, D 1.70 and E 1.34 brighter than the cut).
    - **The server's galaxy** (seed 0x4d2, full potential) near the Sun at the uniform cut 7.95,
      planned in jobs:
      - R(u) by ray is C 1,720 median (largest 1,893 ly, no ray held), D 1,893 (2,000; 677 rays
        held) and E 2,000 (966 held);
      - `bright_beyond` is C 0.727, D 7.70 and E 123.8, **132.3 C to E** against the fixture's
        171.9;
      - `expected_beyond` at 7.95 is C 1,011, D 1,406 and E 2,055.
    - **The served real tier's cold cost** (`sky/census_near_sun_served`, for R06.T17's full-cold
      figure). It was run on 2026-10-09 under the heavy-test lock at `CPUQuota=400%`, so 3
      workers, from release, at criterion's `--test`, by the job threads' CPU time, with the
      census sampled 1 block in 10 on the fixture and 1 in 20 on the server's galaxy:

      | Galaxy, the eye's cut | C (generated)      | D                  | E                  | Whole, plan 12.5–13.3 s in it | Cells in the plan | Wall for the sample |
      | --------------------- | ------------------ | ------------------ | ------------------ | ----------------------------- | ----------------- | ------------------- |
      | Fixture, 8.179        | 2,705 (1.13 × 10⁶) | 4,685 (7.50 × 10⁵) | 2,913 (3.20 × 10⁵) | **10,317 CPU-s**              | 994,423           | 360 s               |
      | Server's, 7.766       | 4,712 (1.97 × 10⁶) | 9,092 (1.44 × 10⁶) | 8,505 (9.35 × 10⁵) | **22,325 CPU-s**              | 817,508           | 421 s               |
      - The counts are T1.b's to the sample's precision. Accepted stars are C 27,370, D 14,940
        and E 2,650 on the fixture, and 33,960, 25,460 and 12,920 on the server's galaxy. The
        stars listed within R(u) towards their texel are 27,250, 14,790 and 2,550, and 33,940,
        25,400 and 12,820. Fix (i)'s cones add about 0.5% of generated records at the limit.
      - The whole is 0.79–0.83 times T1.b's 12,612–13,141 and 27,012–28,238 CPU-s. That is
        the run's 3 workers against T1.b's 15 on the machine's 8 cores with SMT: R06.T8.g recorded
        15 workers reading about 1.28 times what 3 at `CPUQuota=400%` read. At T1.b's 15 workers
        the served tier is so about **1.3 × 10⁴ CPU-s on the fixture and 2.8–2.9 × 10⁴ on the
        server's galaxy** (_estimate_), T1.b's own with fix (i): 15 and 32 minutes on 15
        workers.
      - The machine was shared (load 6–24 during the census runs), so the figures are
        provisional.

    - **The first sky** (the server's `sky_near_sun_cold`, under the heavy-test lock, release,
      15 workers, one iteration; the 1-min load 39 at its start from other lanes' builds, so
      provisional):
      - the first reply to 125 ly took **7.16 s wall and 96.4 CPU-s** (3,192 listed), against
        R06.T11.g's 6.96 s and 94.6 CPU-s and T17's 10 s and 150 CPU-s;
      - the caps and plan were done at 1.96 s (T11.g 1.76 s), from the eye at 0.52 s, so the second
        count adds about 0.2 s and 2 CPU-s;
      - the tables were held 11.7 s and 47.2 CPU-s after the open's answer;
      - 250 ly came at 16.9 s (242 CPU-s, 11,129 listed) and 500 ly at 70.1 s (1,026 CPU-s,
        28,549 listed);
      - the session's first reply came 19.3 s and 145 CPU-s after the open's answer (T11.g 17.95 s
        and 138.8).
    - **The hidden layout check** (`decision-r13-t2b-note.md` §4). It ran in the lane's harness on
      this worktree's build, offscreen and never shown, capped, through the GPU lock, with
      strings set into the kept scenes' `STARS` lines. Logs and shots are in
      `.git/rm23-scratch/r13/t2b/page/`.
      - At 1280 × 720, `L · I` takes 3 lines and `L · S` 4 on the primary beside two instruments,
        in PRECISION TEST and PHASE TEST, both styles, and free. The worst spare is 6 px
        (PRECISION TEST, photorealistic, while streaming), as T11.f measured, and 24 px once final.
        Every block fits, with nothing past the stage and no horizontal scroll. With no instrument
        open, `L · S · I · N` takes 2 lines.
      - At the full layout's least box (98.5 × 45.75rem), `L · I` takes 3 lines for the eye and 4
        for a camera. The camera's `V 10.0 mag CAM ·` leaves its dot on a line of its own, T11.f's
        known lone `·`, a views-lane follow-up. `L · S` takes 5 lines. Everything fits, with 158 px
        or more to spare.
  - **Acceptance as run.**
    - `cargo test -p hyperion-sim --lib` filtered to `sky::caps` and `sky::census` passed: 104
      tests, 4 slow ones ignored, in 293 s at `CPUQuota=400%`. Its doctests for `served_ceiling`
      and `synthetic_ceiling` passed.
    - `cargo test -p hyperion-protocol sky` passed (14), and so did
      `cargo test -p hyperion-server --test sky` (15, 228 s).
    - `cargo test -p hyperion-server --lib` filtered to `sky` passed (43, 158 s). That covers
      `compute::sky`'s and `requests::sky`'s new tests.
    - `pnpm --filter @hyperion/protocol test` passed, and so did
      `pnpm --filter hyperion exec vitest run src/renderer/src/view/sky src/renderer/src/displays/view/useViewSky`.
    - `just gen-protocol` was run. `just ci` runs at integration.
  - **For the sky switch's task** (the default flips on after T2.b): the root README (the
    `--serve-sky` row and the paragraph after it, lines 163–176) and the server's comments still
    say the switch flips when R06.T8.g lands. They are in `config.rs` (the module docs,
    `SkyService` and the builder's `sky_service`), `requests/mod.rs`, `requests/sky.rs`'s module
    docs and `tests/sky.rs`'s two switch-off tests.
