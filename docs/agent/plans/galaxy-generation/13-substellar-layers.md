# Plan 13: Brown dwarfs and rogue planets

## Header

- **Milestone:** M5.
- **Depends on:** 06 (stars: evolution, remnants and classes), and through it 01–05. By M5 plans
  07–12 have also landed; this plan uses plan 08's velocities through the hook plan 03 left and
  needs nothing else from them.
- **Brainstorm sections covered:**
  - "Between the stars": the bullets "Brown dwarfs", "Rogue planets" and "Interstellar comets and
    asteroids", and the closing sentence on the range query's mass floor.
  - "Identifiers": the paragraph on the two substellar layers (16 ly cells for brown dwarfs; 4 ly
    cells, k = −1 and the three spare bits for rogue planets; 1,024 per cubic light-year).
  - "The range query": "The two substellar layers ... come after layer A in the walk and only when
    the caller asks for them."
  - "Sizing the layers": the fourth caveat (brown dwarfs are not in the five layers).
  - "Covering every class of star": the row "Below 0.1 M☉" as far as it gives a brown dwarf its
    state.
  - "Planetary systems": the last paragraph (free-floating objects are systems with no star).
  - "Galaxy parameters": the remark that independent size draws overflowed the rogue-planet index.
  - "Decisions": "Interstellar space has content" and "Rogue planets follow the measured abundance".
  - "Suggested order of attack": step 11.

## Goal

When this plan is done the galaxy holds its free-floating substellar objects, and only those: a
brown dwarf bound to a star is a companion and belongs to plan 11, and a planet bound to a star
belongs to plan 14. Brown dwarfs (13 Jupiter masses to the hydrogen-burning limit) are placed in a
layer of 16 ly cells at one for every five or six stars, and rogue planets (a third of an Earth mass
to 13 Jupiter masses) in a layer of 4 ly cells at about 21 per star on a mass function falling
nearly as 1 ÷ mass, both by the same thinning, populations and ages as the stars, with the rogue
planets' abundance capped per galaxy so that the 16-bit cell index can never overflow. Each object
resolves from its ID, drifts like a system, and answers the range query when, and only when, the
caller lowers the mass floor by one or two steps. A brown dwarf has a state and a class from plan
06's cooling fits; a rogue planet has mass, age, population and metallicity, with bulk properties
and moons left to plan 14, which is handed the cooling fit extended down to giant-planet masses.
Interstellar comets and asteroids exist as a statistical density. In the `GALAXY` display the local
chart can show both kinds with their own symbols, selector steps, legend entries and readout units.

## Scope and non-goals

In scope:

- Generator-version parameters for the two abundances and the two mass functions.
- The per-galaxy cap on the rogue planets' abundance, from the index limit and the galaxy's own peak
  density.
- Two rows of the share matrix, placement of both layers, `resolve`, designations.
- The range query's substellar request and two extra mass-floor steps, the walk order, the census
  line and expected counts.
- A brown dwarf's state and class through plan 06; the cooling fit for giant planets that plan 14
  will call; the hook for plan 14 on both kinds.
- The interstellar small-body number density.
- Protocol, server and client changes that let the chart request and show the two kinds.
- Tests and benchmarks, including a rogue-planet cell at the galactic centre.

Non-goals:

- Bulk properties, atmospheres, moons and rings of rogue planets and brown dwarfs. Plan 14's stage
  supplies them; this plan fixes what it will be handed.
- Substellar members of features (clusters, the nuclear cluster, streams). The brainstorm gives the
  two layers to the grid only. See Risks.
- Brown-dwarf binaries. Every object here is single. See Risks.
- Brown dwarfs and planets bound to stars. A brown-dwarf companion is one of plan 11's bodies in the
  stellar slot of its system, and a bound planet is plan 14's. The abundance here counts
  free-floating objects only (Design note 6).
- Making interstellar comets and asteroids real around a ship. Only the density and a reserved tag
  exist.
- Microlensing by these objects. The brainstorm makes it a ray walked at query time, which is a
  sensor feature; the range query with the lowered floor is what it will use.

## Provides

Rust, under `hyperion_sim::galaxy::substellar` unless stated:

```rust
// params.rs: generator-version parameters
pub struct SubstellarParams { /* private */ }
impl SubstellarParams {
    pub const fn generator_default() -> SubstellarParams;
    pub fn brown_dwarfs_per_star(&self) -> f64;        // 1 ÷ 5.5
    pub fn rogue_planets_per_star(&self) -> f64;       // 21
    pub fn rogue_mass_slope(&self) -> f64;             // 0.96 in dN ÷ dlog M
    pub fn with_rogue_planets_per_star(self, n: f64) -> SubstellarParams; // tests and tuning
}
pub const BROWN_DWARF_MIN: JupiterMasses;              // 13
pub const BROWN_DWARF_MAX: SolarMasses;                // 0.08, the stellar layers' lower edge
pub const ROGUE_PLANET_MIN: EarthMasses;               // 1 ÷ 3
pub const ROGUE_PLANET_MAX: JupiterMasses;             // 13

// mass.rs
pub fn draw_brown_dwarf_mass(f: &dyn MassFunction, s: &mut Stream) -> SolarMasses;
pub fn draw_rogue_planet_mass(p: &SubstellarParams, s: &mut Stream) -> SolarMasses;
pub fn rogue_planets_above(p: &SubstellarParams, m: EarthMasses) -> f64;   // per star, closed form

// abundance.rs
pub struct SubstellarAbundance { /* per-system figures for one galaxy */ }
impl SubstellarAbundance {
    pub fn for_galaxy(galaxy: &Galaxy, p: &SubstellarParams) -> SubstellarAbundance;
    pub fn brown_dwarfs_per_system(&self) -> f64;
    pub fn rogue_planets_per_system(&self) -> f64;     // after the cap
    pub fn rogue_planet_cap_per_system(&self) -> f64;
    pub fn is_capped(&self) -> bool;
}

// small_bodies.rs
pub fn interstellar_small_body_density(galaxy: &Galaxy, p: &GalacticPosition) -> PerCubicAu;
```

Elsewhere in `hyperion-sim`:

- `stellar::substellar::giant_cooling(mass, age, comp) -> CoolingState` (luminosity, radius,
  effective temperature), the cooling fit for 0.3–13 M_Jup, continuous at its upper end with plan
  06's `stellar::substellar::cooling`. Plan 14 consumes both as "plan 13's cooling fits".
  `CoolingState` is this plan's type, beside plan 06's `cooling`, which returns a `StarState`. By
  ruling 34 there are two types, one per kind of object, and no conversion between them: a host (a
  star or a brown dwarf) is always a `StarState`, and a giant planet's interior a `CoolingState`.
- `imf::MassBand` gains `BrownDwarf` and `RoguePlanet`. `ShareMatrix::share` returns, for those two
  bands, objects per system of the population, not a share. `BandShares` stays five-band.
- `placement`: `SUBSTELLAR_LAYERS: [LayerSpec; 2]` (brown dwarfs at 16 ly, rogue planets at 4 ly),
  `layer_spec` answers for them, `resolve(SystemId)` accepts their IDs, and
  `SystemRecord::kind() -> SystemKind` with `SystemKind::{Stellar, BrownDwarf, RoguePlanet}` is
  derived from the layer. Plan 14's `HostKind` has the same three values and converts from it.
- `query`: plan 03's `SubstellarRequest` values other than `None` are accepted;
  `MassFloor::{BrownDwarfs, RoguePlanets}` are the two steps below `LayerA`; `check_index_headroom`
  covers the two layers.
- `Galaxy::substellar(&self) -> &SubstellarAbundance` and `Galaxy::mean_stars_per_system()`. Plan
  02 has the quadrature, the free function `galaxy::fates::mean_stars_per_system(f, fates) -> f64`,
  but `Galaxy` has no such method and holds no fates, so this plan adds the method over the
  galaxy's mass function and the fates `galaxy/params/derive.rs` builds (`ProvisionalFates` today;
  plan 06's `fates_for` after P06.T30.a, and plan 11's `MultiplicityFates` after P11.T1.d).
- `units`: `PerCubicAu`. (`JupiterMasses`, `EarthMasses` and their conversions are plan 01's.)
- Domain tags, as entries of plan 01's `domain_tags!` registry in `rng/tags.rs` under a "Plan 13"
  heading: `SUBSTELLAR_MASS: System = "substellar.mass"` and, reserved and unused,
  `INTERSTELLAR_SMALL_BODIES: Cell = "interstellar.small_bodies"`. The heading goes at the end of
  the list, because the macro's order fixes `tags::ALL`, which `tests/golden/rng/tags.golden` pins;
  the task that adds a tag regenerates that golden with `domain_tags_are_pinned`.

Protocol and client:

- `MassLayer` gains `BrownDwarf` and `RoguePlanet` (`"brown_dwarf"`, `"rogue_planet"` on the wire),
  which serves `SystemsInRangeRequest::min_layer`, `SystemRecord::layer` and `LayerCensus::layer` at
  once. A `substellar` group in `GalaxyParameters` with `substellar.brown_dwarfs_per_system`,
  `substellar.rogue_planets_per_system`, `substellar.rogue_planet_cap_per_system` and
  `substellar.rogue_planets_capped`.
- Client, under `apps/hyperion/src/renderer/src/`: `components/EarthGlyph.tsx` and
  `components/EarthMassUnit.tsx` beside plan 05's `components/SunGlyph.tsx` and
  `components/SolarMassUnit.tsx`. This plan is their only owner, and plan 14 consumes them. In plan
  05's `lib/format.ts`: `formatMassMearth(massMearth)` and `formatSubstellarMass(massMsun, layer)`,
  which returns the value and which of the two units it is in (Design note 14). The `triangle-down`
  value of `SymbolShape` (in `spatial/marks.ts`, its outline in `spatial/symbols.ts`); two steps in
  the `MIN MASS` group of `ChartControls`; legend, list, readout and census entries for the two
  kinds.

Convention for plan 14, matching its D3 (`body_index` = `slot << 8 | sub`, with slot `0x00` the
stellar level, where plan 06 puts the primary at index 0 and plan 11 its companions at 1–15): a
free-floating object is a system whose body `0x0000` is the object itself, its moons are the bodies
`0x0100`, `0x0200` and so on (slot `0x01` upward, sub `0x00`), and its rings are `0x0080`–`0x008F`.
This plan generates and stores no body index other than `0x0000`.

## Consumes

Re-validated against the code at `9d8e775` (see Risks): where an item is built its real path or
signature is given, and where it is not, the task that builds it is named.

- **Plan 01:** `id::{SystemId, Layer}` with `Layer::BrownDwarf` (value 5, 16 ly cells, 19-bit index)
  and `Layer::RoguePlanet` (value 6, 4 ly cells, k = −1, 45 cell bits, the three spare bits, 16-bit
  index) already encoded and canonical, `Layer::cell_size_ly()` and `Layer::index_bits()`;
  `Designation`; `rng` with the Poisson sampler (`Stream::poisson(mean)`, by inversion below a mean
  of 10 and PTRS above, to 2³¹), `PowerLaw::new(exponent, lo, hi)` for a density ∝ x^−exponent with
  both limits explicit (it returns a `Result`, and is drawn with `Stream::power_law(&law)`), and
  integer-threshold decisions; `Stream::open(Seed, DomainTag, ObjectKey)` and the `domain_tags!`
  registry; `units`, including `JupiterMasses`, `EarthMasses` and the constants of `units::consts`
  (`JUPITER_MASS_KG`, `EARTH_MASS_KG`, `SOLAR_MASS_KG`); `coords::CellSize::Ly4` and the position
  draw of its Design note 23 (`GenCell::position_from_words`); `hyperion-testkit` (the `golden!`
  harness with `GoldenWriter`, `stats`, `order::assert_order_independent`); `GENERATOR_VERSION`.
- **Plan 02:** `Galaxy`, `Fields` (`densities`, `layer_density`) and, in `galaxy/bounds.rs`, the
  `Fields` methods `component_bounds` and `layer_bound` with `CellBox`, for every component, which
  the substellar layers reuse unchanged; `galaxy::shares::ShareMatrix` (`share(band, population)`,
  `component_share(band, component)`); `imf::{MassFunction, MassBand, BandShares}`;
  `fates::StellarFates::mean_companions` and the free function
  `fates::mean_stars_per_system(f, fates)` built on it (plan 02's own tests put it at 1.33–1.45);
  `PotentialTables::tidal_radius(m: SolarMasses, p: &PointLy) -> Metres`.
- **Plan 03:** `placement` (`LayerSpec` and `STELLAR_LAYERS`, `layer_spec(layer)`, `CellKey` with
  `new` and `of`, `BuildCellKeyError::NotStellarLayer`, `ResolveSystemError::LayerNotGenerated`,
  candidate counts from bound × volume, candidate streams, population pick by the odds at the
  candidate's position, age draw from the population's distribution, ages from −H, `SystemRecord`,
  `resolve(galaxy, id)`, the clamp and `check_index_headroom(galaxy)` of its Design note 6), `query`
  with `RangeQuery`, `RangeResult`, `Census`, `MassFloor` and `SubstellarRequest` (the request enum
  exists already, with the builder's `substellar(request)`, and is rejected with
  `BuildRangeQueryError::SubstellarLayersUnavailable` until this plan), `LayerCounts` (seven
  entries), `expected_counts`, `cells_in_sphere`, `count_cells_in_sphere`, the padding and unborn
  filter, and the drift hook, `query::motion::epoch_velocity(galaxy, record)` (zero until plan 08).
- **Plan 04:** the request envelope (no new kind is added here); `MassLayer`,
  `SystemsInRangeRequest`, `SystemsInRange`, `Census`, `LayerCensus`, `SystemRecord`,
  `GalaxyParameters` with its groups, `ParameterValue`, the cell cache bounded by bytes; its
  statement under "Extending the convention" that `MassLayer` will grow. The optional request field
  `include_substellar` that it reserves there is not used (Design note 9).
- **Plan 05**, under `apps/hyperion/src/renderer/src/`: `spatial/symbols.ts` (`symbolOutline`,
  `SIZE_CLASS_REM`, the `OUTLINES` record), `spatial/marks.ts` (`SymbolShape`, today `circle`,
  `diamond`, `square` and `triangle`), in `displays/galaxy/`: `ChartControls.tsx`,
  `CensusReadout.tsx`, `SystemList.tsx`, `SystemReadout.tsx`, `SymbolLegend.tsx` and
  `parameterLabels.ts`; `lib/format.ts`, `lib/galaxy/wire.ts` (`layerIndex`, an exhaustive `switch`
  over `MassLayer`), `components/{SunGlyph, SolarMassUnit, UnitLabel}.tsx`, `RADIUS_STEPS_LY` in
  `spatial/scale.ts`, running down to hundredths of a light-year, and the guide's reservation of a
  drawn `⊕` (P05.T2.d).
- **Plan 06:** built at `9d8e775`: `ObjectKind::Substellar` (in `stellar::state`, re-exported from
  `stellar`). Not built yet, by the task that builds it: `stellar::substellar::cooling`, returning a
  `StarState`, valid 0.01–0.08 M☉ (its P06.T13, "shared with plan 13", in the `starB` lane in round
  7); the classification that gives L, T and Y (P06.T23.c); `system::draw_metallicity` (P06.T3,
  `starB`, round 7); `StellarBrief` (P06.T29.b) and its wire form `StellarBriefDto` behind the
  request flag `include_stellar` (P06.T33); and `lib/galaxy/starSymbols.ts` (P06.T35.b), whose
  Design note 17 already draws a substellar object as a circle.
- **Plan 08 (soft):** velocities per population through plan 03's drift hook, which is keyed by ID
  and population and so serves these layers unchanged.
- **Plan 09 (soft):** `features::shares::FeatureShares::field_factor(population, band)`, the factor
  1 − φ, which gains the two new bands (Design note 4).
- **Plan 15 (soft):** the fitting toolchain, if the giant-planet cooling fit of P13.T5.b needs a
  table and not only published power laws. Plan 15 is not built at `9d8e775`: there is no
  `just fit`, `just fit-check`, `tables.lock` or `PROVENANCE.toml`. What exists is plan 02's
  convention in `crates/hyperion-fit`: one task per module under `src/tasks/` with `fit()` and
  `render()`, run as `hyperion-fit run <task>` (`cargo run -p hyperion-fit -- run mge` today), a
  table under `crates/hyperion-sim/src/tables/` whose header names the tool, its inputs and its
  version (the README's convention, as `tables/mge.rs` has it), and a test in
  `crates/hyperion-fit/tests/` that renders the fit again and compares it with the committed table
  byte for byte. P15.T2 later registers existing tables under its own grammar.

## Design notes

1. **Module placement.** `hyperion_sim::galaxy::substellar` holds parameters, mass functions,
   abundance and the small-body density. Placement and the query stay in plan 03's modules and
   become generic over these two layers; no parallel placement code is written.

2. **Abundances are per star in the parameters and per system in the share matrix.** The brainstorm
   states both abundances per star, and placement works in systems. The conversion is the galaxy's
   own mean number of stars per system, 1 plus the mass-function average of plan 02's
   `mean_companions`, which plan 02's tests put at 1.33–1.45. Since its 2026-09-21 revision the
   brainstorm uses that ratio too: about 30 per system, six to a cell, and caps of 44 and 31 per
   star against 1,024 ÷ 16.3 and 1,024 ÷ 23.5 per system under the default mass function (38 and 27
   against 18.7 and 27 under Kroupa's). So rogue planets come to 28–30 per system and five and a
   half to six to a cell at the reference density. See Risks.

3. **One abundance for every population.** "The same populations and ages as the stars" is read as:
   each population's column of the two new rows holds the same number. The matrix allows a later
   version to thin rogue planets in the metal-poor halo; the measurement, made towards the bulge,
   does not yet justify it.

4. **The field density is the stars' field density.** The substellar rows multiply the same
   population densities as the stellar rows, including the factor 1 − φ that plan 09 introduced for
   systems inside features. Plan 09's `field_factor(population, band)` is per band, and for the two
   new bands it returns layer A's factor, the band of the stars these objects most resemble in
   number and origin. Objects born in clusters are therefore neither in the grid nor in the features
   for now. The alternative, the full budget without 1 − φ, would put free-floating objects where
   the young field has almost no stars, which contradicts "they follow the stars".

5. **The brown-dwarf band is [13 M_Jup, 0.08 M☉).** The brainstorm says 13–80 Jupiter masses, and
   0.08 M☉ is 83.8. Taking 80 literally would leave a gap that no layer owns, so the upper edge is
   the stellar layers' lower edge and "80" is read as its rounding.

6. **The brown-dwarf mass function is the substellar branch of the universe's mass function**,
   behind plan 02's `MassFunction`: for Kroupa (2001) a power law of index −0.3 below 0.08 M☉, for
   Chabrier (2003) the continuation of the log-normal. Only its shape is used. The normalisation is
   the abundance parameter, one per 5.5 stars, not continuity with the stellar branch, because the
   brainstorm fixes the count. The count is of free-floating objects only. The brainstorm's other
   figure, "one for every four or five stars, companions included" ("Sizing the layers"), is this
   one plus the brown-dwarf companions that plan 11 draws, about 0.04 per star, and P13.T9 reports
   the sum once plan 11's companions exist.

7. **The rogue-planet mass function** is dN ÷ dlog₁₀ M = Z × (M ÷ 8 M⊕)^−0.96 per star (Sumi et al.
   2023), that is dN ÷ dM ∝ M^−1.96, "falling nearly as 1 ÷ mass". With Z = 2.18 per dex per star
   the integral from ⅓ M⊕ to 13 M_Jup (4,131 M⊕) is 21.0, so the abundance parameter and the paper's
   normalisation agree, and only the abundance, the slope and the limits are parameters; Z is
   derived. Above 0.3 M_Jup the law gives 0.09 per star, inside the limit of one per four stars
   (Mróz et al. 2017), and 97.7% of the objects are below Neptune's 17 M⊕.

8. **The cap uses plan 03's headroom rule.** A 4 ly cell holds 64 ly³ and its index 65,536
   candidates. Plan 03's Design note 6 already requires, for every layer, that the largest possible
   bound × volume plus eight standard deviations stay under the index capacity, with the largest
   bound taken as the sum of every component's global peak, so no search is needed. For this layer
   that is a largest Poisson mean λ with λ + 8√λ ≤ 65,536, about 63,500, or 992 per cubic light-year
   in place of the brainstorm's nominal 1,024. A rogue-planet cell's bound is the abundance per
   system times the bound on the total system density, so the cap per system is 992 ÷ the sum of the
   components' peak densities (about 16 per ly³ at Milky Way values under the default mass function,
   up to about 24; 18.7 and 27 under Kroupa's). The effective abundance is the smaller of the
   parameter and the cap, and `check_index_headroom` then passes by construction. At the default 21
   per star the cap binds for no seed in plan 02's ranges, and a test asserts that, because it is
   what plan 02's coupled size draws were introduced to guarantee. The brown-dwarf layer needs no
   cap: its fullest 16 ly cell expects about 20,000 candidates against 2¹⁹.

9. **Asking and the floor.** The brainstorm says the layers are walked "only when the caller asks"
   and that the mass floor "gains two steps". Plan 03 already carries both: the builder's
   `substellar(SubstellarRequest)` is the asking, and `MassFloor` has room for the steps. They are
   tied so that they cannot disagree: a request of `BrownDwarfs` or `BrownDwarfsAndRoguePlanets`
   lowers a floor of `LayerA` to the matching step, a floor step below `LayerA` without the matching
   request is a build error, and a request with a floor above `LayerA` is a build error too, since
   the floor would cut the layers off. The walk goes E to A, then brown dwarfs, then rogue planets,
   and the census rule is unchanged: a layer whose expected count would exceed the caller's limit is
   dropped whole with everything after it. On the wire the single field `min_layer` carries both,
   because `MassLayer` gains the two values: a client that sends `brown_dwarf` or `rogue_planet` has
   asked. Plan 04 also reserved an optional `include_substellar` flag for this. It stays reserved
   and unused, because a second field could only ever repeat or contradict the first.

10. **Records are `SystemRecord`s.** A free-floating object is a system with no star, as the
    brainstorm says, so it has a `SystemId`, a position, a population, an age and a mass, and
    everything keyed by `SystemId` (drift, tidal radius, frames, designations, overlays) works
    without a special case. `SystemKind` is derived from the layer and never stored. (The name
    `ObjectKind` is plan 06's, for what a star is now.)

11. **Streams.** A candidate's streams are keyed by its ID, which contains the layer, so the
    position, acceptance, population and age draws reuse plan 03's tags and are independent of every
    stellar draw by construction. The mass draw uses a new tag, `substellar.mass`, because its
    distribution is not the stellar one and must be free to change alone.

12. **A rogue planet's state is plan 14's; the cooling fit it needs is built here.** Plan 06's fit
    is valid from 0.01 M☉ (10.5 M_Jup) up. Plan 14 derives a rogue planet as body `0x0000` and asks
    this plan for the cooling of giants from 0.3 M_Jup, so P13.T5.b and P13.T5.c extend the fit down
    to 0.3 M_Jup as `giant_cooling`. This plan itself returns, for a rogue planet, mass, age,
    population and metallicity and nothing derived, and the readout says so until plan 14
    (`BULK PROPERTIES: NOT YET MODELLED`), which is what the UX guide's rule on honest data asks
    for.

13. **Small bodies scale with the stars.** The density is 0.1 per cubic astronomical unit (the
    brainstorm's "perhaps one for every ten cubic astronomical units") at the reference density of
    0.003 systems per cubic light-year, times the local total system density ÷ 0.003, since such
    bodies are ejected from planetary systems. The brainstorm cites no source for the figure, and
    the doc comment says that it is the brainstorm's.

14. **Units on the consoles.** Every planetary mass is shown in M⊕, here and in plan 14 (its D19):
    the guide asks for one unit per quantity everywhere on the ship, and a second planetary unit
    would need a Jupiter sign that B612 lacks as it lacks `☉`. So a rogue planet's mass is in M⊕ (up
    to `4131 M⊕`, the guide grouping digits only from five), and a brown dwarf's is in M☉
    (`0.052 M☉`), the unit of the stellar sequence it continues. No Jupiter-mass unit enters the
    guide. The two new floor steps read `0.012 M☉` and `0.33 M⊕`.

15. **Symbols.** Plan 06 uses circle, ringed circle, diamond, triangle and square, and already draws
    `ObjectKind::Substellar` as a circle. A brown dwarf keeps the circle, at layer A's size class,
    because it is the faint end of the same sequence; the list, readout and legend name it. A rogue
    planet is a new type and gets a new `SymbolShape`, `triangle-down`, also at layer A's size
    class, the smallest at which plan 05 found an open outline still reads as open. No size class
    below A is added.

## Tasks

Parallelism: T1, T5.b, T5.c and T6 are independent of everything else here (T5.c needs T5.b). T2
needs T1. T3 needs T2. T4, T5.a and T5.d need T3 and are independent of each other. T7 needs T4 and
T5.a. T8.a and T8.b need only plan 05; T8.c and T8.d need T7. T9 needs T3–T5 and can run beside T7
and T8. Every domain tag is added to `rng/tags.rs` in the task that first opens a stream under it,
as plan 01 requires, at the end of the list (Provides).

**The vertical slice** (README, "The vertical slice to the `SYSTEM` display", ruling 33 of
2026-09-22) takes T5.b, T5.c, T8.a, T8.b and the `triangle-down` outline of T8.c ahead of the rest,
because plan 14 needs the giant-planet cooling fit, the Earth-mass unit and the planet symbol. Until
the layers are placed (T3), plan 14's `HostKind` is always `Stellar`, although all three of its
values exist.

### P13.T1 Parameters, units and mass functions

Build `substellar::params` and `substellar::mass`, on plan 01's `JupiterMasses` and `EarthMasses`.
Register `SUBSTELLAR_MASS` in `rng/tags.rs`; the mass stream is
`Stream::open(seed, tags::SUBSTELLAR_MASS, ObjectKey::from(id))`. `draw_brown_dwarf_mass` samples
the substellar branch of the `MassFunction` between the band edges; add that branch to plan 02's
`Kroupa` and `Chabrier` implementations as a separate method, leaving every existing method's output
unchanged. `draw_rogue_planet_mass` uses plan 01's `PowerLaw::new(1.96, lo, hi)` (its exponent is
the positive exponent of a density ∝ x^−exponent, here in dN ÷ dM) with both limits explicit, built
once (it returns a `Result`, which a constant law cannot fail) and drawn with
`Stream::power_law(&law)`. `rogue_planets_above` is the closed-form integral. Re-check the slope,
normalisation and lower mass against Sumi et al. (2023), the Jupiter limit against Mróz et al.
(2017), and the substellar indices against Kroupa (2001) and Chabrier (2003).

- Files: `crates/hyperion-sim/src/galaxy/substellar/{mod.rs,params.rs,mass.rs}`,
  `crates/hyperion-sim/src/rng/tags.rs`, plan 02's `imf` module.
- Tests: 10⁵ draws of each mass pass plan 01's Kolmogorov–Smirnov helper against the analytic
  distribution; all draws lie inside their band; with Z derived from 21 per star the value at 8 M⊕
  is 2.18 per dex to 2%; `rogue_planets_above(0.3 M_Jup)` is under 0.25 per star; the share below 17
  M⊕ is above 0.95; plan 02's `imf` goldens are unchanged.
- Acceptance: `cargo test -p hyperion-sim substellar` passes.

### P13.T2 Abundance, the per-galaxy cap and the share matrix

Build `substellar::abundance` by Design notes 2, 3 and 8. Add `Galaxy::mean_stars_per_system()`,
which `Galaxy` lacks: computed once, when the galaxy is built, with plan 02's existing quadrature
`galaxy::fates::mean_stars_per_system(f, fates)` over the galaxy's mass function and the fates that
`galaxy/params/derive.rs` builds (Provides). Add `MassBand::{BrownDwarf, RoguePlanet}`; every
exhaustive match on `MassBand` then fails to compile and is settled one by one: `BandShares` and
`MassFunction::sample_in_band` reject the two values (they are not part of the stellar
normalisation), `ShareMatrix::share` and `component_share` return the per-system abundance, the
conversion from `id::Layer` maps the two layers, and plan 09's `FeatureShares::phi` and
`field_factor` answer for them as for band A (Design note 4), if plan 09 has landed by then (it is
not built at `9d8e775`; if it has not, there is no factor 1 − φ yet, and plan 09's task adds the two
bands when it lands). `Galaxy` builds and holds a `SubstellarAbundance` and feeds it to its
`ShareMatrix`. Extend plan 03's `check_index_headroom` to the two layers.

- Files: `crates/hyperion-sim/src/galaxy/substellar/abundance.rs`, plan 02's `imf`, `shares` and
  `Galaxy`, plan 03's `placement` (headroom check only).
- Tests:
  - Milky Way fixture: brown dwarfs per system 0.23–0.27; rogue planets per system 27–31; the cap
    per star within 10% of 43 (992 ÷ 16.3 ÷ 1.42; 38 under Kroupa's); `is_capped()` false;
    `check_index_headroom` passes.
  - With `with_rogue_planets_per_star(60.0)` on the Milky Way fixture the effective figure equals
    the cap, the cap × the summed peak densities × 64 is at most the headroom mean, and
    `check_index_headroom` still passes.
  - Over 2,000 seeds (slow): the default abundance is never capped; the smallest cap per star found
    is recorded and lies within 20% of the brainstorm's 31; the brown-dwarf layer's fullest 16 ly
    cell has a mean under 10% of its 2¹⁹ index.
  - For the five stellar bands `ShareMatrix::share` is bit-identical to before.
- Acceptance: the tests pass; every stellar golden file is unchanged.

### P13.T3 Placement of the two layers

- **P13.T3.a Brown dwarfs.** Add `SUBSTELLAR_LAYERS` and make `layer_spec` answer for
  `Layer::BrownDwarf`: cell size from `Layer::cell_size_ly()` (16 ly), density and bound from
  `layer_density` and `layer_bound` with `MassBand::BrownDwarf`, population pick and age as for
  stars, mass from `draw_brown_dwarf_mass`. `CellKey::new` and `CellKey::containing` stop rejecting
  the layer with `BuildCellKeyError::NotStellarLayer`, `CellKey::of` stops returning
  `ResolveSystemError::LayerNotGenerated` for it,
  and plan 03's tests that assert the layer is not generated are replaced in the subtask that makes
  each of them false (here and in T3.b for `CellKey`, in T3.c for `resolve`). Acceptance: where the
  system density is 0.003 per ly³ a 16 ly cell averages 2.6–3.4 brown dwarfs (the brainstorm's "two
  or three", at this plan's stars per system); at five positions (plane at 26,000 ly, 3,000 ly above
  it, the bulge at 1,000 ly, an arm ridge, the halo at 40,000 ly) the count in a test volume lies
  inside plan 01's Poisson interval for the expected count; the population mix passes a chi-square
  test against the odds the stellar layers use at the same position; ages pass a Kolmogorov–Smirnov
  test against layer A's ages there.
- **P13.T3.b Rogue planets.** The same for `Layer::RoguePlanet`: 4 ly cells (k = −1), positions
  drawn as integer light-years within the cell (two bits per axis) plus an offset, candidate index
  in 16 bits with the ID's spare bits carrying the extra cell bits as plan 01 encodes them, mass
  from `draw_rogue_planet_mass`. `LayerSpec::cell_ly` is a `u32` and 4 fits; any code in plan 03
  that assumes a cell of at least 8 ly is found by a test that generates a cell at every corner of
  the root cube. Acceptance: the same three statistical checks; at a density of 0.003 systems per
  ly³ a cell averages 4.8–6 objects; every generated ID round-trips through plan 01's canonical
  decode; an ID of another layer with non-zero spare bits is still rejected.
- **P13.T3.c Resolution, designations, goldens.** `resolve` for both layers (recompute the candidate
  count, check the index, rerun the acceptance), `SystemRecord::kind`, designations through plan 01.
  The carve-out hook `catalogue_claims` is not called for these layers. Bump `GENERATOR_VERSION`.
  Acceptance: every object from a generated cell resolves to the same record; an index at or above
  the candidate count, and a rejected candidate's index, resolve to "no such system"; order
  independence (cells generated in two orders and an object resolved alone agree bit for bit); a
  golden file of twenty objects per layer for two seeds; objects with negative age exist in the
  young disc and are "no system yet" before their birth.
- **P13.T3.d Bound hunting.** Extend plan 03's violation hunt to 4 ly and 16 ly cells along arm
  ridges near the bar's ends and in the flared and central regions, with the debug assertion that
  density never exceeds the bound active. Acceptance: no violation in 10⁶ candidates per layer
  (slow).
- Files: plan 03's `placement` module and tests, `crates/hyperion-sim/tests/golden/`.

### P13.T4 Range query: the request and two floor steps

Accept `SubstellarRequest::{BrownDwarfs, BrownDwarfsAndRoguePlanets}`, add
`MassFloor::{BrownDwarfs, RoguePlanets}`, and implement the tie between them, the walk order and the
census of Design note 9 (new error variants `SubstellarNotRequested` and `SubstellarBelowFloor` on
`BuildRangeQueryError`; `SubstellarLayersUnavailable` is removed). Expected counts for the census
decision come from the same integral of the layer density over the sphere that the stellar layers
use: plan 03's `LayerCounts` has had seven entries since M1, the two substellar ones held at zero
(its design note 17), and `expected_counts` now fills them, only when the layers are requested.
`Census::complete_down_to` can now name a substellar layer, and `complete_above` then returns that
band's lower edge in M☉ (0.0124 for brown dwarfs, 1.0 × 10⁻⁶ for rogue planets). `cells_in_sphere`
and `count_cells_in_sphere` accept the two layers, and the cell budget counts their cells. Padding,
the time argument, the cell budget and the unborn filter apply unchanged. The merge hook for
non-grid sources gains nothing, since no feature holds substellar members.

- Files: plan 03's `query` module and tests.
- Tests: with the default request the result is bit-identical to the result before this plan and the
  query's statistics show no substellar cell touched; with `BrownDwarfs` a 50 ly query where the
  density is 0.003 per ly³ returns about 390–450 brown dwarfs (Poisson interval on the computed
  expectation) and the same stars; with `BrownDwarfsAndRoguePlanets` and a limit of 5,000 at 50 ly
  the rogue-planet layer is dropped whole and the census names the brown-dwarf step; at 10 ly it is
  kept and returns about 350–400; at the galactic centre with a 0.5 ly radius the expected count
  drives the decision and the result is the same across runs and cache states; results at t = ±1,000
  years contain the same IDs apart from those crossing the sphere; each contradictory builder input
  returns its error variant.
- Acceptance: the tests pass; plan 03's query benchmarks are unchanged at the default request.

### P13.T5 State of a brown dwarf, the giant cooling fit and the hook for plan 14

- **P13.T5.a Brown dwarfs through the stellar stage.** Route a brown-dwarf record through plan 06's
  stage: metallicity draw, then `stellar::substellar::cooling` at age plus clock time, then
  classification. Plan 06's fit starts at 0.01 M☉ and the band at 0.0124 M☉, so no extension is
  needed at this end; the deuterium-burning refinement that P06.T13 leaves to this plan is made here
  if the source gives it in closed form, and otherwise recorded as not modelled. Check that
  `PotentialTables::tidal_radius` accepts substellar masses. Tests: over 10⁴ brown dwarfs, none is
  classed earlier than M6, effective temperature falls monotonically with age at fixed mass and
  rises with mass at fixed age, state is continuous across ±H, and state is continuous in mass
  across 0.08 M☉ against a layer-A star of the same age and metallicity to 5%; a golden histogram of
  classes for the old thin disc and the halo, checked by eye against the expectation that old brown
  dwarfs are mostly T and Y; an Earth-mass object at 26,000 ly has a tidal radius of 0.04–0.08 ly.
- **P13.T5.b The giant-planet cooling data.** This plan builds the cooling fit for 0.3–13 M_Jup and
  plan 14 consumes it. The source is the giant-planet models of Burrows et al. (2001), the one the
  brainstorm cites for this range. First evaluate the paper's power laws for L, T_eff and R at 1
  M_Jup and 4.6 Gyr against Jupiter (effective temperature 100–160 K, radius within 10%). Recalled
  without the paper to hand, they miss Jupiter's luminosity severalfold, so expect to need the
  second route: a `hyperion-fit` task `giant_cooling`, written as plan 02's `mge` was, because plan
  15's toolchain does not exist (Consumes). It is `crates/hyperion-fit/src/tasks/giant_cooling.rs`
  with `fit()` and `render()`, run as `hyperion-fit run giant_cooling` (a new `Command` variant and
  `USAGE` line); it reads the published model grid, committed under
  `crates/hyperion-fit/data/giant_cooling/`, whose source (paper, table, retrieval date) the table's
  header records until P15.T2 brings `PROVENANCE.toml`; it fits log L and R on a grid of at most 12
  masses × 12 ages in log mass and log age; and it writes
  `crates/hyperion-sim/src/tables/giant_cooling.rs` under the README's header convention, marked
  provisional, with a `mod` line in `tables/mod.rs`. P15.T2 later registers it under its own
  grammar. Acceptance: either the power laws pass the Jupiter check and the doc comment records it,
  or `cargo run -p hyperion-fit -- run giant_cooling` writes the table and
  `cargo test -p hyperion-fit` passes. That test, `crates/hyperion-fit/tests/giant_cooling.rs`,
  renders the fit again and compares it with the committed table byte for byte, as `tests/mge.rs`
  does.
- **P13.T5.c Cooling of giant planets.** `stellar::substellar::giant_cooling` (signature under
  Provides) for 0.3–13 M_Jup and ages from 1 Myr, from T5.b's power laws or table (interpolated
  bilinearly in log mass and log age, continuous in both, no bins in age). It must join plan 06's
  fit at 0.0124 M☉: luminosity, radius and temperature agree to 5% there at 0.1, 1 and 10 Gyr, by a
  blend over 10–13 M_Jup if the two sources disagree. Tests: a 1 M_Jup object at 4.6 Gyr has an
  effective temperature of 100–160 K and a radius within 10% of Jupiter's; luminosity falls
  monotonically with age and rises with mass; state is continuous in age across ±H; golden values at
  nine (mass, age) points, written through `GoldenWriter`. Acceptance:
  `cargo test -p hyperion-sim stellar::substellar::giant` passes and plan 06's goldens are
  unchanged. _Slice:_ this needs P06.T13's `cooling`, which the `starB` lane builds in round 7, and
  lands after it merges. `CoolingState` is this plan's own type, with no conversion from or to
  `StarState` (ruling 34).
- **P13.T5.d The hook.** For a rogue planet the sim returns the record and its metallicity and no
  derived state. Document on `SystemKind` the body-`0x0000` convention and that plan 14's `HostKind`
  converts from it. Test: a rogue planet's summary has no stellar fields and serialises without
  them.
- Files: plan 06's `stellar/substellar.rs` and its tests, `crates/hyperion-fit/` and
  `crates/hyperion-sim/src/tables/giant_cooling.rs` if a table is needed,
  `crates/hyperion-sim/src/galaxy/substellar/mod.rs`.
- Acceptance (T5.a and T5.d): their tests pass under `cargo test -p hyperion-sim substellar`; plan
  06's goldens are unchanged.

### P13.T6 Interstellar small-body density

Build `substellar::small_bodies` by Design note 13, add `PerCubicAu`, register the reserved tag
`INTERSTELLAR_SMALL_BODIES` (scope `Cell`) in `rng/tags.rs` with a comment on what it is reserved
for: making such bodies real around a ship, keyed by a cell.

- Files: `crates/hyperion-sim/src/galaxy/substellar/small_bodies.rs`,
  `crates/hyperion-sim/src/units.rs`, `crates/hyperion-sim/src/rng/tags.rs`.
- Tests: 0.1 per au³ where the system density is 0.003 per ly³; proportional to the system density
  at three other positions; the domain-tag collision test still passes.
- Acceptance: the tests pass.

### P13.T7 Protocol and server

Add `BrownDwarf` and `RoguePlanet` to the wire's `MassLayer`. `min_layer` then carries the request
(the server maps it to the sim's `SubstellarRequest` and floor), `SystemRecord::layer` tells the
kind, and `LayerCensus` rows appear for the two layers with their mass bounds in M☉. For a brown
dwarf the record carries plan 06's `StellarBriefDto` when `include_stellar` is set; for a rogue
planet that field is absent, not zeroed. No request kind is added, so `REQUEST_KINDS` is unchanged,
and plan 04's reserved `include_substellar` stays unused (Design note 9). Add the `substellar` group
to `GalaxyParameters`: the three abundances as `Number { unit: None }` with origin `Derived`, and
`rogue_planets_capped` as `Text { value: "yes" | "no" }`. The server keeps its result limit as the
census limit and accounts a rogue-planet cell's bytes in the cell cache (a central cell holds some
35,000 records, over a megabyte). Run `just gen-protocol`. The generated `MassLayer` union then has
two more values, and `just ci` runs `pnpm typecheck`, so this task also settles the client's
exhaustive uses of it, with the least that compiles: `layerIndex`'s `switch` in
`lib/galaxy/wire.ts` (both new layers give layer A's index, as T8.c keeps) and the
`Record<MassLayer, …>` of band edges in `test/galaxyFixtures.ts`. The chart still offers only the
five stellar steps until T8.c.

- Files: `crates/hyperion-protocol/src/` (plan 04's galaxy messages), `crates/hyperion-server/src/`
  (plan 04's handlers and caches), `packages/protocol/src/`, and under
  `apps/hyperion/src/renderer/src/`, `lib/galaxy/wire.ts` and `test/galaxyFixtures.ts`.
- Tests: wire-form tests pinning the JSON of a request with each new `min_layer`, of a result
  holding one of each kind, and of the parameters group; a WebSocket integration test that the
  default request returns no substellar object and a lowered `min_layer` does; a cache test that
  inserting a central rogue-planet cell evicts by bytes and never exceeds the bound.
- Acceptance: `just gen-protocol-check` and `just ci` green.

### P13.T8 Client: symbols, selectors, legend, units

- **P13.T8.a UX guide.** Edit `docs/frontend/ux-guidelines.md`: allow the Earth mass as a unit,
  written `M` with the drawn `⊕` glyph that plan 05 reserved there; state Design note 14's rule for
  which objects use which mass unit; add the inverted triangle to the star chart's symbol set as
  "free-floating planet" and "brown dwarf" to the circle's meanings. Acceptance:
  `grep -n "⊕\|free-floating" docs/frontend/ux-guidelines.md` finds each edit, and Prettier passes
  on the guide. _Slice:_ by ruling 33 of 2026-09-22 the guide is the owner's to edit, so these
  entries are drafted for the owner beside plan 14's (the orchestration notes'
  `ux-draft-system-display.md`), and the client is built to the draft and marked for the owner's
  confirmation, as ruling 15's `DRIVE RANGE` row was. The acceptance above holds once the owner has
  made the edit.
- **P13.T8.b Glyph and formatting.** `components/EarthGlyph.tsx` (an inline SVG sized to the text, a
  circle with a cross, in `currentColor`) and `components/EarthMassUnit.tsx`, modelled on `SunGlyph`
  and `SolarMassUnit`, with the accessible name "Earth masses" on the unit as a whole; in
  `lib/format.ts`, `formatMassMearth` (two decimals below 10, one below 100, whole numbers above,
  grouped in threes from five digits as plan 05's `formatNumber` does) and
  `formatSubstellarMass(massMsun, layer)`. The character `⊕` is never typed into a string that
  reaches the screen. Tests: the unit's accessible name; `0.33`, `17.1` and `4131` from the
  formatter (not `4,131`, which the guide's grouping rule forbids); a brown dwarf formats in M☉ and
  a rogue planet in M⊕. Acceptance: `pnpm test` and `pnpm lint` green. _Slice:_ `EarthGlyph`,
  `EarthMassUnit` and `formatMassMearth` are built ahead of the rest (the `ui` lane, from round 7),
  to the owner's draft of T8.a. `formatSubstellarMass` waits for T7, because the substellar values
  of its `layer` argument arrive there. `UnitLabel` renders the wire's `Unit`, which has no
  Earth-mass value, so `EarthMassUnit` is used directly, as `SolarMassUnit` is in `SystemReadout`
  and `SystemList`.
- **P13.T8.c Chart symbols, selector and census.** `spatial/marks.ts`'s `SymbolShape` gains
  `triangle-down`, `spatial/symbols.ts` its outline, and every exhaustive `switch` over
  `SymbolShape` its case; plan 06's `lib/galaxy/starSymbols.ts` (P06.T35.b) gains the two kinds
  (Design note 15). T7 has already settled the exhaustive uses of `MassLayer` minimally: plan 05's
  `layerIndex` gives both new layers layer A's size class. `ChartControls`' `MIN MASS` group gains
  the steps `0.012 M☉` and `0.33 M⊕`, keyboard-operable like the others, below `0.08`, which no
  longer reads `ALL`; the lowest step does. Choosing a substellar step while the query radius would
  exceed the census limit is allowed, and `CensusReadout` (`COMPLETE ABOVE 0.012 M☉`) tells the
  operator what was dropped, as it does for stars. Tests: component tests for the selector steps and
  which one reads `ALL`, and for the census line in each unit; a pure test that `triangle-down` open
  and filled differ only by fill and share a hit area. Acceptance: `pnpm typecheck`, `pnpm lint` and
  `pnpm test` green. _Slice:_ the `triangle-down` outline, with its tests, is built ahead of the
  rest in round 7 (the `ui` lane), because plan 14 draws every planet with it; the rest of this
  subtask waits for T7.
- **P13.T8.d Legend, readout, list and parameters.** `SymbolLegend` gains the inverted triangle and
  the circle's new meaning and keeps `SYMBOLS NOT TO SCALE`. `SystemReadout` shows kind, mass in the
  unit of Design note 14, age, population and metallicity, class and temperature for a brown dwarf,
  and `BULK PROPERTIES: NOT YET MODELLED` for a rogue planet, never a blank or a zero.
  `SystemList`'s kind column names the two kinds in words, so that shape is never the only signal,
  and plan 06's `STARS` filter gains them. `parameterLabels.ts` gains the `substellar` group. Tests:
  component tests for the legend, the readout of each kind, the list and the parameter labels.
  Acceptance: `pnpm typecheck`, `pnpm lint`, `pnpm test` and `just ci` green; by eye, at 10 ly
  around a point in the plane the chart shows a few stars, about one brown dwarf per five stars and
  a few hundred rogue planets, and remains readable.
- Files: the named components and modules under `apps/hyperion/src/renderer/src/` and their tests.

### P13.T9 Statistical tests and benchmarks

Slow tests in `crates/hyperion-sim/tests/substellar_statistics.rs` and Criterion benches in
`crates/hyperion-sim/benches/substellar.rs`.

- Ratios in a 400 ly cube in the plane at 26,000 ly, Milky Way fixture: brown dwarfs ÷ stars within
  10% of 1 ÷ 5.5 and rogue planets ÷ stars within 10% of 21, with stars counted as systems ×
  `mean_stars_per_system()`; objects above 0.3 M_Jup under one per four stars. The test also prints
  free-floating brown dwarfs plus plan 11's brown-dwarf companions per star, for review against the
  brainstorm's "one for every four or five stars, companions included"; that figure is not asserted
  here, because the companions are plan 11's.
- Budgets: over ten random large volumes the summed counts of each layer match the integral of its
  density (Poisson interval), which is the brainstorm's density-against-the-field test for these
  layers.
- Determinism: two runs agree; the stellar goldens of plans 03 and 06 are unchanged.
- Benchmarks, with targets that are findings if missed: one rogue-planet cell touching the origin,
  about 35,000 candidates (20 ms); a 0.5 ly query at the origin with rogue planets requested (150 ms
  cold); a 10 ly query at a density of 0.003 per ly³ with rogue planets requested (5 ms cold over
  the stellar query); a 50 ly query with brown dwarfs requested (1 ms over the stellar query). If
  the first two miss, the recorded remedy is to test a candidate's distance from the padded sphere
  before evaluating its density, which changes no output because acceptance is independent per
  candidate, and applies only to cells not being cached whole.
- Acceptance: `just test-slow` green; `just bench` runs the four benches and the figures are
  recorded in the module docs.

## Verification

- `just ci`, `just test-slow` and `just bench` as listed. The brainstorm's requirements map as
  follows: layer sizes and ID layout (T3.b, T3.c), abundances and the per-galaxy cap (T2, T9), mass
  functions and the Jupiter limit (T1, T9), same populations and ages (T3.a, T3.b), state from the
  cooling fits (T5.a), the giant cooling fit handed to plan 14 (T5.b, T5.c), the two floor steps and
  walking only on request (T4), the statistical small-body density (T6), display changes (T8), the
  central-cell benchmark (T9).
- Golden, order-independence and bound tests as the brainstorm's Testing section requires of any
  placed layer (T3.c, T3.d).
- By eye in the `GALAXY` display: a 10 ly chart in the plane with the floor at its lowest step; the
  same at 1,000 ly from the centre with a radius of 2 ly; the parameters panel's abundances for
  several seeds.

## Generator version

- No existing object moves: the new layers have their own layer values in every ID, and the only new
  tag with draws is `substellar.mass`. `GENERATOR_VERSION` is bumped once, in P13.T3.c, when the
  layers become resolvable, because a universe gains objects. Goldens of earlier plans change only
  where they record the version.
- The abundances, the slope, the band limits, the headroom rule behind the cap and the giant cooling
  fit belong to the generator version; changing any of them changes candidate counts and so every
  substellar object.
- Reserved: the tag `interstellar.small_bodies`; per-population entries in the two share-matrix
  rows; body `0x0000` of a free-floating system for the object itself, with plan 14 taking slots
  from `0x01`. The two layers use the last layer values, so, as the brainstorm notes, the spare bits
  of the other layers are the only room left in the ID.

## Risks and open points

- **Stars per system: an inconsistency in the brainstorm, since resolved.** It converted 21 per star
  to "about 26 per system" (a ratio of 1.24), while its caps of 38 and 27 per star against 1,024 ÷
  18.7 and 1,024 ÷ 27 per system implied 1.40–1.44, and plan 02 computes 1.33–1.45 from the
  multiplicity the brainstorm itself specifies. The per-star figure is the measurement and the ratio
  is derived, so this plan uses the computed ratio throughout. The brainstorm's 2026-09-21 revision
  does the same: about 30 rogue planets per system and six to a cell. Under the default mass
  function it keeps "490 per cubic light-year at the centre", because the centre holds 0.87 times
  Kroupa's systems. The local census's 0.0019 systems and 0.0025 stars and white dwarfs per cubic
  light-year imply 1.30, and plan 11's companions may bring the model's ratio down towards that.
- **The measured abundance is uncertain by a factor of two either way.** At the upper end (42 per
  star) the cap binds for most seeds near the centre, which silently lowers the abundance of the
  whole galaxy, not only of the centre, because the share-matrix entry is one number per population.
  The parameters panel shows when that happens.
- **Features hold no substellar objects** (Design note 4). A globular cluster or the nuclear cluster
  therefore shows no brown dwarfs on a chart. Plan 09's member ID has spare mass-band values that
  could carry them; that is a later revision with a version bump.
- **Brown dwarfs are single.** Plan 11's multiplicity is not applied, although a tenth to a fifth of
  free-floating brown dwarfs are binaries. Applying it later changes no placement, only the systems'
  contents.
- **The upper band edge** is read as 0.08 M☉, not 80 M_Jup (Design note 5).
- **Cost at the centre.** A single central rogue-planet cell is tens of thousands of candidates and
  a 0.5 ly query touches at least eight. The benchmark targets are derived from plan 03's
  per-candidate cost and are untested until that code exists.
- **The giant cooling fit.** Burrows et al. (2001) give power laws for brown dwarfs and model curves
  for giant planets, and what this plan says of them is recalled, not re-read: P13.T5.b re-checks
  it. If the curves must be fitted, T5.b is a `hyperion-fit` task of about a day and T5.c the
  function. Plan 14 depends on the function's name, range and `CoolingState`, not on its internals.
- **The cap against the index limit.** The brainstorm states the limit as 1,024 per cubic
  light-year, the bare capacity of the index. Under plan 03's headroom rule the usable figure is 992
  (Design note 8), which lowers the caps by 3% and is inside the rounding of "about 44" and "31".
- **Updated for the 2026-09-21 density rulings.** The default mass function is now Chabrier's system
  function, with 0.87 times Kroupa's systems, so the peak system densities behind the cap fall to
  about 16 and 24 per cubic light-year. The caps rise to about 43 per star for the Milky Way fixture
  and 30–31 for the densest seed (Design notes 2 and 8, P13.T2's tests). The over-cap test now asks
  for 60 per star, because 42 no longer exceeds the Milky Way cap.
- **Two knobs for one request.** Plan 03 carries both a `SubstellarRequest` and room in `MassFloor`.
  Design note 9 ties them; dropping `SubstellarRequest` in favour of the floor alone would be
  simpler and is a change to plan 03's signature that its author chose to avoid.
- **Small-body density** rests on an unsourced figure in the brainstorm (Design note 13).
- **P13.T8.b and T8.c's outline, as built (round 7, `ui`).** `EarthGlyph` (a 10-unit SVG circle and a cross through its centre, `M5 1V9M1 5H9`, in `currentColor`, class `glyph` as `SunGlyph`) and `EarthMassUnit` (`M` and the glyph, `role="img"` named "Earth masses") are built, with tests that the sign is drawn and never typed. `formatMassMearth` gives `0.33` and `17.1` as specified, but **`4131`, not `4,131`**: the guide groups digits from five, as plan 05's T3.a record already applies everywhere, so `12,480` is grouped and four digits are not. Two decimals would read `0.00` below 0.005 M⊕ and show no significant digit below 0.01, so from there the mass is in E notation with three figures (`1.80E-9`), the guide's form for a value outside its unit's ladder; between 0.01 and 0.1 M⊕ a moon or a Mercury still gets one or two significant figures (`0.01` for the Moon), which is **for the orchestrator to rule** now that plan 14's D19 puts moons in this unit. `formatSubstellarMass` is not built: it switches on the substellar `MassLayer` values, which the protocol does not have yet. Of T8.c only the `triangle-down` outline is built: the triangle turned over, with the same centroid and so the same hit area (tested), open and filled differing only by fill. At size class 0 its open hole is 1.75 px across at 100% (the test asserts at least the 1.5 px outline) and 0.95 px at 80%, where it barely reads open; plan 14's Risks name a fourth outline as the fallback.
- **Re-validated at `9d8e775` for the vertical slice** (round 7, the `doc` lane). Plans 01–05 are
  built, plan 06 in part, plans 08, 09 and 15 not at all. The plan text now follows the code in
  these places:
  - `Galaxy` has no `mean_stars_per_system()` and holds no fates, but plan 02's quadrature exists as
    the free function `fates::mean_stars_per_system(f, fates)`, so T2 adds a method over it rather
    than a quadrature.
  - `PowerLaw::new` returns a `Result` and is drawn with `Stream::power_law`; the Poisson sampler is
    `Stream::poisson`.
  - The layer rejections T3 removes are `BuildCellKeyError::NotStellarLayer` (from `CellKey::new`
    and `containing`) and `ResolveSystemError::LayerNotGenerated`; the drift hook is
    `query::motion::epoch_velocity`.
  - `ObjectKind` lives in `stellar::state`.
  - Plan 15's toolchain does not exist, so T5.b's fit, if needed, follows plan 02's `mge`
    convention (`hyperion-fit run giant_cooling`, checked by `cargo test -p hyperion-fit`).
  - T7 must settle `layerIndex`'s exhaustive `switch` and the fixtures' `Record<MassLayer, …>`
    itself, since `just ci` type-checks the client after `just gen-protocol`.
  - New tags go at the end of `domain_tags!`.
  - The heaviest rogue planet reads `4131 M⊕`, not `4,131 M⊕` (Design note 14, T8.b, and plan 14's
    D19), because the guide groups digits only from five, as plan 05's `formatNumber` does.

  Pending re-validation, because what they read is not built:
  - T5.a (P06.T13, T23.c, P06.T3's `draw_metallicity`);
  - T5.c's join with P06.T13 at 0.0124 M☉;
  - T7's `StellarBriefDto` for brown dwarfs (P06.T33);
  - T8.c's `starSymbols.ts` and T8.d's `STARS` filter (P06.T35.b);
  - plan 09's `field_factor` (T2).

- **For the orchestrator to rule: one cooling state or two.** P06.T13's `substellar::cooling`
  returns a `StarState` (the `starB` lane builds it in round 7), T5.c's `giant_cooling` returns this
  plan's `CoolingState` (L, R, T_eff), and plan 14's P14.T11.d, T12.a and T27 read both. The
  options:
  - (a) keep both, and give `CoolingState` a `From<&StarState>`, so that plan 14 reads only
    `CoolingState` for every substellar host and body;
  - (b) `giant_cooling` also returns a `StarState`, with `Phase::Substellar`, so that one type
    serves and plan 14 reads `StarState`. `StarStateParts` then asks for a core mass, a mass-loss
    rate and a phase fraction, which mean nothing for a planet;
  - (c) plan 14 matches on the host kind and calls whichever fit applies, keeping two code paths.

  Ruled (ruling 34): two types and no conversion, so a host is always a `StarState` and a giant
  planet's interior a `CoolingState`, and P14.T12.a and P14.T11.d each accept only one of them.

- **The vertical slice** (README, "The vertical slice to the `SYSTEM` display (2026-09-23)"). This
  plan's tasks in it are T5.b, T5.c (after P06.T13), T8.a (as an owner's draft), T8.b (without
  `formatSubstellarMass`) and T8.c's `triangle-down` outline. Nothing here is placed yet, so plan
  14's `HostKind` is always `Stellar`, and brown dwarfs and rogue planets cannot be reached from the
  `SYSTEM` display.
- **`formatMassMearth` below 0.1 M⊕, as ruled (ruling 35.9; round 7b, `ui`).** Below 0.1 M⊕ a mass keeps two significant figures (`0.012` for the Moon, `0.0040`, `0.099`), and below 0.001 M⊕ it is in E notation with the same two (`1.6E-4` for Ceres, `1.8E-9`). Plan 13's two decimals hold from 0.1 up. Each step is decided on the rounded text, so 0.0996 reads `0.10` from either side and 0.000999 reads `0.0010`; zero reads `0.00`. `formatSci` gains a last optional `significantFigures`, three by default, for it. The guide writes E notation "with three significant figures", and the ruling's `1.6E-4` has two. The client follows the ruling, so the owner's draft should say that a mass below 0.001 M⊕ keeps two figures (**for the orchestrator**, with the draft).
- **Deviations in P13.T5.b–c, as built** (round 7, `giant`).
  - _The power laws fail the Jupiter check_, so T5.b took the second route. At 1 M_Jup and 4.6 Gyr
    Burrows et al.'s equation 1 gives 1.59 × 10⁻¹⁰ L☉, 13–18% of Jupiter's internal luminosity;
    equation 2 gives 36 K; equation 5 at equation 3's gravity gives 1.42 R_Jup. A unit test keeps
    the check.
  - _The grid is Sonora Bobcat_ (Marley et al. 2021, ApJ 920, 85; Zenodo record 5063476,
    CC BY 4.0), cloudless, at [M/H] = 0. It is the only candidate whose licence allows committing
    its numbers: Burrows et al.'s 1997 files ask to be told of any use, and Baraffe et al.'s COND
    tracks carry no licence. It also reaches old, cold planets, down to 100 K. The fit reads 455 rows,
    0.0005–0.011 M☉, committed with their source, retrieval date and checksums in
    `crates/hyperion-fit/data/giant_cooling/sonora_bobcat_nc+0.0_co1.0_mass.txt`.
  - _The tracks above 11.5 M_Jup are left out_, because they burn deuterium: 0.47 dex brighter
    than `cooling` at 12.6 M_Jup and 0.1 Gyr. Neither fit models deuterium burning.
  - _Two corners are extrapolated._ No grid reaches 0.3 M_Jup. Bobcat's lightest track is
    0.52 M_Jup and ends at 3 Gyr, and its 1.05 and 1.57 M_Jup tracks end at 6 and 10 Gyr. The fit is
    least squares for bilinear interpolation with a second-difference penalty of weight 10⁻⁴, and
    that penalty makes the table a power law in mass and age where there are no rows. Saturn's
    corner is therefore extrapolated. **For the orchestrator to rule.**
  - _The table and its residuals._ The table holds log₁₀ L and log₁₀ R, not R, at 12 nodes
    log-spaced over 0.3–13 M_Jup and 12 over 1 Myr–15 Gyr. The worst residuals are 0.046 dex in
    log L and 0.60% in R, both at 10.5 M_Jup (0.2 and 0.1 Gyr), with rms 0.008 dex and 0.12%.
  - _Past 15 Gyr the state follows Burrows et al.'s late-time age laws_, L ∝ t^−1.3 and
    R ∝ t^−0.056, as `cooling` does. Each mass's own last interval would have let masses change
    order in the blend by 37 Gyr. No object is that old, but the function takes any age.
  - _Jupiter and Saturn at 4.6 Gyr._ At 1 M_Jup the fit gives 104.6 K and 71,478 km, against
    Jupiter's internal 99–107 K and 71,492 km. At the fit's lower end of 0.3 M_Jup (Saturn has
    0.2994) it gives 63.6 K and 65,270 km. Saturn's internal flux is 77–84 K (Hanel et al. 1983;
    Wang et al. 2024) and its mean radius 58,232 km. So a coreless model with no helium rain is
    17–24% cold and 12% large, as Fortney et al.'s (2007) coreless 66,560 km confirms. Plan 14's
    T11.d uses Chen and Kipping's radius there.
  - _The API._ `giant_cooling(mass: JupiterMasses, age: Years, comp: &Composition) ->
Result<CoolingState, EvaluateGiantCoolingError>`, as P06.T13 returned a `Result`. The error
    has `MassOutsideFit` and `AgeOutsideLife`. The range is `GIANT_MIN_MASS` and `GIANT_MAX_MASS`
    (0.3 and 13 M_Jup, inclusive), and below 1 Myr the 1 Myr state is held. `CoolingState` has
    `luminosity()` in L☉, `radius()` in R☉ and `effective_temperature()` in K. They describe the
    planet's own cooling, as if isolated. The mass is in Jupiter masses because the range is, and a
    caller converts with `From`.
  - _Metallicity moves the luminosity as it moves `cooling`'s at 13 M_Jup and the same age._ That
    is Burrows et al.'s κ̂^0.35 once the object is degenerate. It is less while the object
    contracts: at Z = 10⁻⁴, −0.12 dex at 1 Myr against the term's −0.57. The radius is the table's
    at every metallicity. Bobcat's ±0.5 dex grids move L by a weaker 0–0.22 dex per dex, and they
    make metal-rich objects 0.5–2.5% smaller per dex. With Bobcat's dependence a metal-poor
    object's luminosity would fall with mass across the join. With `cooling`'s, the two sources
    differ there by the same amount at every metallicity. **For the orchestrator to rule.**
  - _The join._ Over 10–13 M_Jup, ln L and ln R are blended, linearly in log mass, to `cooling`'s
    closed form. That form is read from 10 M_Jup, 5% below its own 0.01 M☉. At 13 M_Jup, L and R
    equal `cooling`'s bit for bit. At 0.0124 M☉ they agree within 0.11% in L, 0.013% in R and
    0.026% in T at 0.1, 1 and 10 Gyr, at every metallicity. A smooth step would let L fall with
    mass at old ages, where the two sources differ by 0.16–0.20 dex. With the linear weight,
    d ln L ÷ d ln m stays above 0.09.
  - _Files._ `stellar/substellar.rs` is now `stellar/substellar/mod.rs`, unchanged except one doc
    line and the `mod giant` and `pub use` lines. `giant_cooling` is in `stellar/substellar/giant.rs`,
    so its tests run under `cargo test -p hyperion-sim stellar::substellar::giant`. The
    `stellar/substellar_cooling` golden is unchanged. The new golden `stellar/giant_cooling` pins
    nine (mass, age) points at [Fe/H] = 0 and −1. In `hyperion-fit`, `mge`'s float-literal helper
    moved to `tasks/render.rs` and learned negative numbers; the `mge` table is unchanged.
