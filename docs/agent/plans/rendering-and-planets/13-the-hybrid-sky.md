# Plan R13: The Hybrid Sky

- **Milestone:** Rendering milestone RM7, sequenced after RM3 and beside RM4 (numbered RM7 so that
  RM4–RM6 keep their numbers). R13.T1 may run before RM3 closes, and R13.T2 is RM3's interim
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
  unresolved stars, labelled as such") and the drafted subsection "The hybrid sky: the census near,
  synthetic stars far" (for the owner's sign-off); open questions 13, 16 and 19 as they touch the
  census's reach and cost. The decision it answers is the owner's of 2026-10-08 on the full sky's
  time (`decision-p11-t17c-bright.md` §4), and its feasibility study is
  `.git/rm23-orchestration/feasibility-hybrid-sky.md`.

## Goal

When this plan is done, the sky's census is exact only where exactness matters and affordable: out
to a real boundary per layer and direction, set so that under one star brighter than a fixed
ceiling V_P is expected beyond it, every star is the galaxy's own, as R06 lists it today. Beyond
that boundary the stars between V_P and the request's cut are **synthetic**: drawn by the server in
fixed cells of the galaxy, by exact thinning of the generated galaxy's own density, luminosity
functions and dust, the same tables the band and the caps read, so that they are correct in
expectation, consistent with the band texel by texel, the same for every client, run, machine and
observer, stable as the camera moves and true in parallax. The band holds only the remainder. The
view draws synthetic stars as it draws listed ones and says that they are synthetic; they cannot be
selected, targeted or counted, and on approach the real census takes their region over. Near the
Sun the full sky then costs some thousands of CPU-seconds rather than 0.4–1.0 × 10⁶, and a camera's
sky to V 10 becomes affordable.

## Scope and non-goals

In scope:

- The measurement of the real boundary from the caps alone, and a sampled bench of the real tier
  (T1).
- The real tier: each layer C–E capped at T7.b's caps computed at the ceiling, listed by the band
  texel's radius at every reply, with the T7.b gap closed (T2); RM3's interim is this task with the
  band for the rest.
- A colour–magnitude table in R06.T5's quadrature, and the light-consistent counts the synthetic
  tier draws from (T4).
- The band's third pair of sums, the light fainter than the ceiling, and the march's ray profiles
  kept for the synthetic tier (T3).
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
- Making a synthetic star a real system ("lazy realisation by record"): a later refinement if the
  owner asks for it (Risks).
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
pub const SYNTHETIC_CEILING_V: f64;      // V_P: 4.5 recommended (owner's ruling; 5.0 in RM3's interim)
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
// sky::caps (R13.T2)
pub fn real_boundary(at_ceiling: &CapCount, caps_at_cut: &[LayerCap]) -> Vec<LayerCap>;
    // C–E: each ray the lesser of the ceiling's cap and the cut's (or spheres, T1's ruling);
    // A, B and the brown dwarfs: the cut's caps
// sky::census (R13.T2, T6)
impl SkyQueryBuilder { pub fn synthetic_ceiling(self, v: Magnitudes) -> Self; }
impl SkyQuery { pub fn synthetic_ceiling(&self) -> Option<Magnitudes>; }
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
  synthetic)) and `synthetic_ceiling_v: Option<f64>` (V_P, absent where the reply has no real
  boundary at a ceiling). Each `SkyLayerCensusDto` gains its real boundary, in the per-ray form
  R06.T11.d gives `complete_to_ly`, and its `expected_beyond` is stated at the cut beyond it.
- No new kind, no byte of the star or texel records changes, and `PROTOCOL_VERSION` stays (an
  additive field, R03 Design note 12).

### Client (`apps/hyperion/src/renderer/src/view/sky/`)

`SkyModel.synthetic` (the index range of the synthetic stars), drawn, baked, culled and spritten as
listed stars; `skyLabelValue(limitV, kind, gaps, notes)` composing the `STARS` line's notes in the
order `STREAMING`, then the synthetic or the interim note, then `NOT YET MODELLED`; the view list
note `SYNTHETIC STARS` beside `INTEGRATED STARLIGHT`, in both styles.

### Test helpers

`crates/hyperion-sim/tests/sky_hybrid.rs`: the boundary record (T1), the census-against-synthetic
comparison (T9). A test-only salt (`SyntheticSalt`, `#[cfg(any(test, feature = "testing"))]`) that
re-keys the synthetic stream, for ensembles of independent realisations.

## Consumes

Names are as built at `rendering-and-planets` b6cb51b1 unless marked; R13.T1 re-checks each against
the tree before building.

- **R06** (`06-the-sky.md`):
  - `sky::luminosity::{LuminosityTables, LuminosityFunction, TablesPlan, MAGNITUDE_STEP,
EMITTED_AGO_YEARS}`, `get_at`, `age_for`, `count_brighter_than`, `light_fainter_than`,
    `colour_sums_fainter_than` (crate-visible) and `pair_light`; R06.T5.d's per-1-mag-bin pair
    correction inside `TablesPlan::assemble`'s finish step.
  - `sky::caps::{CapCount, CapResolution, LayerCap, RayRadii, RayExtinctions, layer_caps_over,
CAPPED_LAYERS}`: `CapCount::measure` at any cut, `caps`, `spheres`, `stars_beyond`,
    `systems_within`, `stars_within_and_beyond`.
  - `sky::census::{SkyQuery, SkyQueryBuilder, CensusPlan, Completeness, census_plan_of,
merge_shells, SkyCensus, SkyStar}` and T8.i's listing by the band texel's radius
    (`Completeness`, `BandSpec::texel_of`); `brute_force_sky` and `SkyQuery::with_caps_forced`
    (`tests/common/sky.rs`).
  - `sky::band::{BandSpec, BandMarch, march_rows, sum_rows, CompleteTo, largest_texel_radius}` and
    T9.j's eye light; `sky::limits::{Glare, eye_offsets}`; `sky::colour::{star_colour, StarColour,
AtmosphereGrid, surface_gravity}` and `StarColour::reddened`.
  - The server's sky pipeline (`crates/hyperion-server/src/{requests,compute}/sky.rs`,
    `bulk/sky.rs`), T11.c's switch, and **R06.T11.d** (shells as bulk jobs, `complete_to_ly`,
    `final`, the per-ray tables), which is not built at b6cb51b1.
  - The decision records the plan rests on: `decision-r06-census-cost.md`,
    `decision-r06-census-cost-signoff.md`, `decision-r06-t8i-listing.md`,
    `decision-r06-t8h-warm.md`, `decision-p11-t17c-bright.md`.
- **R07:** the label block's `STARS` reading (`viewSkyLabel`, `view/sky/label.ts`'s
  `skyLabelValue`) and the view's list notes (`displays/view/useViewSky.ts`,
  `displays/view/useInstruments.ts`, where `INTEGRATED STARLIGHT` is composed).
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
   - **synthetic:** stars at or beyond R(u) with V_P ≤ V < cut, drawn by Design note 5;
   - **band:** the rest, in expectation: everything fainter than the cut, and beyond R(u) the light
     brighter than V_P (under one star a layer by Design note 3) and, while a sky streams, the
     pending real region (Design note 11).

   A star's tier is decided by the texel `BandSpec::texel_of` places it in (R06.T8.l's lookup and
   T8.i's listing rule), so the three share one boundary texel by texel and each star's light is
   added once (R06 Design note 11). A, B and the brown dwarfs are wholly real: their caps are
   11–260 ly near the Sun at the eye's and the camera's cuts.

3. **The real boundary is T7.b's cap at the ceiling.** For layers C–E, R(u) is the cap by ray that
   `CapCount::measure` gives at cut V_P instead of the request's cut, widened as T7.b widens, and
   never beyond the cut's own cap on any ray: R06 Design note 9's criterion applied at the ceiling.
   So **every star brighter than V_P, and every star nearer than R(u), is the galaxy's own**, to
   under one expected miss a layer, and each reply states it. One radius, not two cuts: a real tier
   complete to V_P beyond R(u) would cost about what the full cut costs, since at version 21 the
   census generates `Bright` pairs whatever its cut (E's `Bright` median is M_V −5.0; at 2 kly a
   cut of 5 instead of 8 lowers the listable share of `Bright` pairs from about 0.85 to 0.55, read
   from the quantiles of `decision-p11-t17c-bright.md` §2.2), and the caps at V_P are where such a
   census ends anyway.
   - **The listing is by the band texel's radius at every reply**, the final one included (T8.i's
     rule (b), `decision-r06-t8i-listing.md`). With R(u) at the ceiling thousands of stars lie just
     beyond it, so the one-shot rule ("every star of the cells it opens") would count them twice,
     against the synthetic tier.
   - **No gap.** T7.b's gap, a star within R(u_T) of its texel but in a cell the cones about its own
     direction left closed, is bounded today by the count beyond the caps, under one star. Beyond a
     ceiling's cap that bound is gone (_estimate_: ten to a few tens of stars near the Sun). R13
     closes it exactly, by fix (i) of that record (cones widened by the texel radius ρ) or by
     spherical boundaries, whichever R13.T1 measures cheaper.
4. **The ceiling V_P** is a server constant, `SYNTHETIC_CEILING_V`, stated in every reply. It is
   not a client setting: a bridge shares one sky and every ship takes the same rule. It sets the
   real tier's cost and the synthetic share (_estimate_, `feasibility-hybrid-sky.md` §2.2, near the
   Sun at the eye's cut, spherical boundaries; the boundaries and costs lean low, since the model
   ignores extinction, and T7.b's rays lower the cost):

   | V_P | R: C / D / E (ly)                       | Real tier, CPU-s | Synthetic share of stars brighter than V 6.5 |
   | --- | --------------------------------------- | ---------------- | -------------------------------------------- |
   | 4.0 | 520–610 / 860–960 / 1,620–1,840         | 2.0–2.7 × 10³    | 17–22%                                       |
   | 4.5 | 740–850 / 1,180–1,290 / 2,240–2,510     | 4.5–5.8 × 10³    | 8–11%                                        |
   | 5.0 | 1,050–1,180 / 1,600–1,730 / 3,110–3,420 | 1.0–1.25 × 10⁴   | 3.6–4.7%                                     |

   The recommendation is 4.5: every star brighter than V 4.5 stays real (in a Sun-like sky some
   900, the constellation-forming ones; a plausibility check from Hipparcos's 1,608 to V 5 at 0.49
   dex a magnitude), about 90% of naked-eye stars stay real, and the real tier lands near R06.T17's
   ruled 4,000 CPU-s target once its boundaries are by ray. The value is the owner's
   (`feasibility-hybrid-sky.md` §11, question 1). It depends on V_P, not on the cut, so the eye and
   every camera share one partition, and one census serves both.

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
   - a jump of Δ swaps about (dN/dr) Δ ÷ 2 stars at the boundary, all fainter than V_P: _estimate_
     some 200 for 10 ly near the Sun at the eye's cut, some 40 of them brighter than the eye's
     limit. The same jump moves a real star at 1,000 ly by up to 0.57°, and the client swaps a sky
     whole by a cut, so the swaps hide in a change that is real.

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
9. **The band with a ceiling** (R13.T3). When the query states a ceiling, `march_rows` keeps, per
   layer and edge, a third set of the five sums, the light fainter than V_P at the band's boundary
   (V_P − DM − v☉(A_V) A_V), beside the light fainter than the cut and all of it, and it keeps each
   ray's A_V at its nodes. Beyond R(u), a reply with synthetic stars holds the light fainter than
   the cut plus all the light less the light fainter than V_P; a reply without them (RM3's interim)
   holds all the light, as R06.T9.b does. _Estimate:_ +10–30% of the march's time, as T9.j's second
   sums added 31%, and some 8 MB of profiles at 64².
10. **A synthetic star is a listed star for every purpose but its identity.** The brightest N_max of
    both kinds are sent, by flux, a real star before a synthetic one at equal flux, then by key;
    the rest are overflow, splatted into the band as points. Synthetic stars brighter than the eye's
    cut glare (`Glare::of_listed`) and take eye offsets (`eye_offsets`); T9.j's eye light is
    unchanged. The views cull, bake and sprite them as listed stars (R06 Design note 20).
11. **Streaming.** Synthetic stars lie only beyond the final R(u), and every reply, the first
    included, carries the same synthetic set, computed once after the march. The pending real
    region of a reply not yet final is the band's, as R06.T11.d has it. So no synthetic star swaps
    while a sky arrives, and the first reply already shows the far faint field as points.
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
    - The brainstorm's "a star the player jumps to is the star they were looking at" then holds for
      every star brighter than V_P and every star within R(u) (the brainstorm draft; the owner's).
13. **What the view says** (R13.T8; drafts for the owner, by T15's route).
    - On the `STARS` line's composed-note slot, after `STREAMING` and before `NOT YET MODELLED`, in
      the annunciation form of `TERRAIN: STREAMING` (steady, `--text`, no status colour, neither a
      data state nor an alert). Draft wording: `STARS V 7.4 mag EYE · FAINT DISTANT STARS:
SYNTHETIC`.
    - In the view's list notes, `SYNTHETIC STARS` beside `INTEGRATED STARLIGHT`, in both styles,
      since the wireframe draws stars too: drawn singly from the galaxy's own statistics, not at any
      system's position, and never read as stars that can be selected or counted.
    - The guide's "Views" gains a third kind of star beside the two it names.
    - RM3's interim, with no synthetic stars, names what it leaves to the band in the
      `DETAIL LIMITED` form the census-cost sign-off advised for a labelled rule (its question 4),
      for example `STARS V 7.4 mag EYE · DISTANT STARS FAINTER THAN V 5.0: DETAIL LIMITED`.
14. **Determinism.** The synthetic tier follows the sim-determinism skill: one new tag, its word
    layout pinned by a test, every transcendental through `math`, sums in a fixed order, cells in
    canonical order, and any split of the cells into jobs giving the same bits. It is sim code on
    the server (R06 Design note 1), so every client of a server receives the same bits, and every
    server on Linux, macOS or Windows computes them. Nothing persists: a reply is recomputed, and a
    table refit or a code change moves the synthetic sky as it moves the band.
15. **RM3's interim is R13.T2** (`feasibility-hybrid-sky.md` §10). RM3 may ship the real tier at
    V_P = 5.0 with the band for the rest (Design note 9's reply without synthetic stars), labelled
    by Design note 13's interim note. When R13.T7 lands, V_P moves to the owner's value and the
    synthetic stars fill [V_P, cut); the real tier's code does not change again. _Estimate_ near the
    Sun at the eye's cut: the full sky 1.0–1.25 × 10⁴ CPU-s, 11–14 minutes on 15 workers, with about
    4–5% of the naked-eye stars in the band.
16. **What it costs** (_estimates_, `feasibility-hybrid-sky.md` §4 and §7; T1, T5.b and T9 measure):
    - the real tier at V_P 4.5: 4.5–5.8 × 10³ CPU-s near the Sun at the eye's cut (spheres), against
      0.4–1.0 × 10⁶ for the exact census; about 1.2–1.3 times that at the camera's 10.06;
    - the synthetic pass: about 1–5 CPU-s at 7.95 and 5–25 at 10.06;
    - the first sky: unchanged by R13 but for a second caps count at V_P over the same rays and the
      synthetic pass, 10–40 CPU-s together; the first shell stays R06.T11.d's;
    - warm after a jump: the real tier's warm ratio stays T8.n's (about 95–100% of cold at version
      21), of a tier some hundred times smaller; the synthetic pass needs no cache.

## Tasks

T1 comes first and can run before RM3 closes. T2 needs T1 and the owner's ruling on V_P; T2.a
needs only R06 as built, and T2.b needs R06.T11.d. T4 needs nothing of R13 and can run beside T1–T3.
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
and cheaper; the owner rules V_P with these figures.

Files: `crates/hyperion-sim/tests/sky_hybrid.rs` (new; the slow test
`the_hybrid_boundary_is_recorded`, which records, and asserts only that R(u) is within the cut's caps
on every ray and that the expected count brighter than V_P beyond it is under one a layer),
`crates/hyperion-sim/benches/sky.rs` (`sky/census_near_sun_ceiling`), `.config/nextest.toml` (its
slow-test entry), this plan. Acceptance: `just test-slow the_hybrid_boundary_is_recorded`,
`cargo bench -p hyperion-sim --no-run`, the bench recorded, `just ci`.

### R13.T2 The real tier at the ceiling (RM3's interim)

- **R13.T2.a The census.** `SkyQueryBuilder::synthetic_ceiling` (refused unless finite and brighter
  than the cut by at least 0.5 mag, naming its field), `SkyQuery::synthetic_ceiling`,
  `sky::caps::real_boundary` (Design note 3, in T1's ruled form), and `census_plan_of` and the server
  plan taking it for C–E when a ceiling is stated: shells to R(u), C–E listed by the band texel's
  radius at every reply, fix (i)'s widened cones if ruled, and each layer's `expected_beyond` stated
  at the cut beyond R(u) (`CapCount::stars_beyond` at the cut). With no ceiling every bit is R06's.
  Tests:
  - with no ceiling, T8.e's identity tests, T8.i's shell tests and T9.b's conservation test pass
    unchanged;
  - with a ceiling, shells merged equal the census with caps forced to R(u) by the texel rule, bit
    for bit, and the last shell is the one-shot census under the same rule;
  - no listed C–E star lies at or beyond its texel's R(u), in any reply;
  - no gap: over a small census (an observer near the Sun, forced small radii, a high cut), every
    star `brute_force_sky` finds within its texel's R(u) is listed;
  - R(u) is within the cut's cap on every ray, and the expected count brighter than V_P beyond it is
    under one a layer;
  - the band with `CompleteTo` at R(u): listed, overflow and band light within 1% of R06's at the
    cut's caps (T9.b's conservation, at the new radii).

  Files: `sky/caps.rs`, `sky/census/{query,cell,merge}.rs`. Acceptance:
  `cargo test -p hyperion-sim sky::caps`, `cargo test -p hyperion-sim sky::census`,
  `just test-slow the_census_is_its_oracle_1000_ly_from_the_sun caps_converge_in_rays`, `just ci`.

- **R13.T2.b The server, the reply and the interim label.** The server states the ceiling
  (`SYNTHETIC_CEILING_V`, 5.0 until R13.T7) on every request and computes both caps counts over one
  `RayExtinctions`; the response's `synthetic_ceiling_v`, each layer's real boundary and its
  `expected_beyond` at the cut (`just gen-protocol`); the client composes Design note 13's interim
  note; its guide row is drafted for the UX decision agent and the owner. The sampled cold bench of
  the served sky near the Sun is recorded for R06.T17's full-cold figure. Tests: a sky near the Sun
  returns the sim's census at the ceiling bit for bit; the reply states the ceiling and each layer's
  boundary and count beyond; the label's composition with `STREAMING` and `NOT YET MODELLED`.
  Files: `crates/hyperion-protocol/src/sky.rs`, `crates/hyperion-server/src/{requests,compute}/sky.rs`,
  `crates/hyperion-server/tests/sky.rs`, `packages/protocol/src/`, generated bindings,
  `apps/hyperion/src/renderer/src/view/sky/label.ts`, `docs/frontend/ux-guidelines.md` (the draft
  row). Acceptance: `cargo test -p hyperion-protocol sky`, `cargo test -p hyperion-server --test sky`,
  `pnpm --filter @hyperion/protocol test`, `pnpm --filter hyperion exec vitest run
src/renderer/src/view/sky`, `just ci`.

### R13.T3 The band's ceiling sums and kept profiles

Design note 9. `march_rows` keeps the third set of sums and each ray's A_V at its nodes
(`RayProfiles`, `BandMarch::profiles`) when the query states a ceiling; `sum_rows` takes whether the
reply carries synthetic stars, and beyond R(u) holds Design note 9's light. `RayProfiles::a_v_toward`
reads the texel `texel_of` gives, linear in distance between nodes. Tests:

- with no ceiling, T9.f's and T9.j's tests and bits are unchanged;
- each kept profile equals `profile`'s value at every node, bit for bit;
- a reply without synthetic stars equals RM3's interim band bit for bit, and one with them plus the
  expected synthetic light (the march's slots, fainter than V_P less fainter than the cut, beyond
  R(u)) equals it to rounding, texel by texel;
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
  the profile's A_V, V, the [V_P, cut) window, the class and colour, the emitted time, and
  `SyntheticStar` and `SyntheticTally`. Tests:
  - no kept star is brighter than V_P, at or fainter than the cut, or nearer than its texel's R(u);
  - its A_V is `a_v_toward`'s bit for bit, and its colour the class's `reddened` at it;
  - over 64 salts near the Sun, the mean light of the kept stars per texel equals the march's
    synthetic slot beyond R(u) within 3σ, and over the sky within 1%;
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
(Design note 11), and sets `SYNTHETIC_CEILING_V` to the owner's value; the switch
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
  to R₁ and clipped at R₂, over 64 salts. Per layer, 1-mag bin of apparent V and |b| band: counts,
  light, the T_eff histogram and the shares above A_V 2, 5 and 10. Gate: each count and light ratio
  within 1 ± 3σ, once the tables lane has corrected R06.T5.f's findings; until then the ratios are
  recorded, not gated. The census's σ is its Poisson σ times √1.5, since its systems are Poisson
  but their stars come in multiples (Σk² ÷ Σk is 1.46 for D and 1.54 for E, R06 Risks, "R06.T5.f's
  measurements, as built"), combined with the tables' pair σ.
- **`the_synthetic_seam_is_continuous`** (slow): near the Sun at 10.06, counts per magnitude in thin
  shells either side of R(u), in the plane and above it, real against synthetic, and the scatter of
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

Record the figures in this plan's Risks, and hand the measured boundary and costs to the brainstorm
draft for the owner. Files: `crates/hyperion-sim/tests/sky_hybrid.rs`, the golden,
`.config/nextest.toml`, this plan. Acceptance: `just test-slow the_synthetic_tier_matches_the_census
the_synthetic_seam_is_continuous`, `just bench -- sky/synthetic`, `just ci`.

## Verification

- **Exact in expectation:** the thinning against its intensity (T5.a), the kept stars' light and
  counts against the band's slots and `CapCount` (T5.b), and the total light against the interim
  reply (T6).
- **Consistent with the census of this galaxy:** counts, light, colour and extinction in shells,
  real against synthetic, at ten observers (T9), gated once R06.T5.f's tables are corrected.
- **One boundary:** no listed star beyond its texel's R(u), no synthetic star within it, no gap
  (T2.a, T5.b), and the band's ceiling split to rounding (T3).
- **Determinism and stability:** observer independence, truncation, job-split independence, the
  word layout (T5.a), the golden (T9), and every reply's identical synthetic set (T6, T7).
- **Cost:** the boundary and the real tier (T1, T2.b), the synthetic pass (T5.b) and the served sky
  (T9), each with its cut, its machine and whether it is provisional.
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
  T2 builds.
- **Realised against expected.** R06.T5.f's findings (C 0.966 ± 0.004, E 1.164 ± 0.034, realised
  over tabulated) would show as a density step at R(u): about 3.5% more synthetic C stars and 14%
  fewer E stars than the census would list. T9's gate waits for the tables lane's correction; until
  then the ratios are recorded.
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
- **The camera's sky** is mostly synthetic near the Sun (_estimate_ 93% of some 4.8 × 10⁵ stars to
  V 10.06), and its N_max overflow the norm. The camera-budget ruling owed by R06.T17 rules on the
  real tier at the ceiling (_estimate_ 6–7.5 × 10³ CPU-s at V_P 4.5), not on the exact census.
- **Multiplayer.** Two ships far apart see the same world-anchored synthetic stars, but each its own
  real boundary, so a star real for one may be absent for the other, replaced by synthetic ones.
  Inherent to any hybrid.
- **The first sky** stays R06.T11.d's (185–225 CPU-s at the 500 ly shell, against 150); R13 adds
  10–40 CPU-s to it.
- **The brainstorm's principle**, "the star field is the galaxy", is amended, not kept whole: the
  draft subsection is the owner's to sign off, with the ceiling and synthetic stars in the eye's
  view (`feasibility-hybrid-sky.md` §11).
- **Lazy realisation by record**, a later refinement if the owner wants every point to be a system:
  real positions and identities with a brightness drawn from each record's mass, age and \[Fe/H\],
  exact on approach for single main-sequence stars to the table's accuracy and statistical for
  evolved stars and pairs, at a walk of every far record (at least about 10³ CPU-s at the eye and
  10⁴ at the camera) and a new fitted table.
- **Open for decision agents:** spheres or rays for R(u), and fix (i) (T1); the cell size (T5.b);
  texel or bilinear extinction, if T9's seam test asks (T5.b, T9); the label's wording (T8).
