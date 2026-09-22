# Plan 02: Galaxy model, from parameters to fields

- **Milestone:** M1.
- **Depends on:** [01](01-determinism-foundation.md).
- **Brainstorm sections covered:** "Galaxy parameters"; "Fields" (density, age and metallicity; the
  velocity field is plan 08 and the gas and dust field plan 07); "Populations"; "Sizing the layers";
  the bound rule of "Exact placement by thinning" (steps 2 and 3 of the thinning are plan 03); the
  tidal radius formula of "Coordinates"; the disc sub-division of "Orbits and time"; the halo table
  of "Streams and accreted structure" (smooth components only); the density-map passage of
  "Visualiser"; the bound checks and the Milky Way comparisons of "Testing" that concern mass,
  rotation and density.

## Goal

When this plan is done, `hyperion_sim::galaxy::Galaxy::new(seed)` turns a `Seed` into a complete,
immutable description of one barred spiral: its drawn and derived parameters, the mean present-day
mass of a system and the system count that follows from it, the potential reduced to tables
(circular speed, Ω, κ, potential, escape speed, tidal radius, the bar's pattern speed and the
central black hole's mass), the seven populations as closed-form number densities with their age and
metallicity distributions, the share matrix per layer and population, a true upper bound on every
density over any cell, and the column densities the galaxy map needs. Everything is a pure function
of the seed and the generator version. No star is placed here: plan 03 does that with what this plan
provides.

## Scope and non-goals

In scope:

- `GalaxyParams`: every parameter of "Galaxy parameters", drawn from streams of their own, with
  sizes coupled to masses, plus a Milky Way fixture for the comparisons.
- Mass functions behind one interface, band shares by integration, the share matrix.
- The mean present-day mass per system and the derived system count, on provisional stellar fates
  owned by this plan (see Design notes D4).
- The mass model and its potential tables, on a provisional Gaussian expansion (D6).
- The fields: densities, arms, age distributions (ages from −H, the `1 − φ` hook held at φ = 0),
  metallicity distributions, the halo as a marked mixture of smooth components, the old thin disc as
  five sub-discs with heights from the vertical Jeans equation.
- The bound rule: nearest-corner envelopes, the arm factor bounded from its phase range, and the
  general unimodal-factor rule as a trait.
- Column density, face-on and edge-on, as pure functions.

Not in scope:

- Placement, candidates, population pick, mass and age _draws_, the range query (plan 03). This plan
  supplies the distributions as functions of a uniform variate; plan 03 owns the streams.
- Velocities, dispersions, asymmetric drift, the bulge's Jeans table (plan 08).
- The gas and dust field (plan 07). Only the gas disc's mass and size enter here, for the potential.
- Any use of the accretion history, the globular-cluster count, the nuclear cluster's members, φ or
  the halo's discrete share (plans 09 and 10). They are parameters here and nothing more.
- Flared displaced classes (plan 08). The bound trait is shaped so that they fit without change.
- Stellar lifetimes and remnant masses proper (plan 06), multiplicity proper (plan 11), the fitted
  production tables (plan 15).
- Protocol messages, quantisation, caching and the CPU pool (plan 04).

## Provides

All under `hyperion_sim::galaxy`. Signatures are sketches.

### `galaxy` (root)

```rust
pub struct Galaxy { /* params, fates, mass model, tables, fields, shares */ }
impl Galaxy {
    pub fn new(seed: Seed) -> Self;                      // the default IMF, in-plane tables only
    pub fn with_mass_function(seed: Seed, kind: MassFunctionKind) -> Self;
    pub fn from_params(params: GalaxyParams) -> Self;    // fixtures and tests
    pub fn with_full_potential(self) -> Self;            // adds the (R, z) grid; about a second
    pub fn seed(&self) -> Seed;                          // plan 01's `rng::Seed`
    pub fn params(&self) -> &GalaxyParams;
    pub fn potential(&self) -> &PotentialTables;
    pub fn fields(&self) -> &Fields;
    pub fn shares(&self) -> &ShareMatrix;
    pub fn mass_function(&self) -> &dyn MassFunction;
    pub fn system_count(&self) -> f64;                   // expected, at the epoch, φ = 0
    pub fn mean_system_mass(&self, p: Population) -> SolarMasses;   // present-day, deaths included
    pub fn mean_formed_mass(&self) -> SolarMasses;       // mass formed per system, no deaths
}
pub struct PointLy { pub x: f64, pub y: f64, pub z: f64 }   // galactic frame, light-years
impl From<&coords::GalacticPosition> for PointLy;
pub enum Population { YoungThinDisc, OldThinDisc, ThickDisc, Bulge, LongBar, NuclearDisc, Halo }
pub const POPULATIONS: [Population; 7];
```

### `galaxy::imf`

```rust
pub const MASS_BAND_EDGES: [f64; 6] = [0.08, 0.5, 0.75, 2.5, 8.0, 150.0];   // M☉, layers A–E
pub enum MassBand { A, B, C, D, E }                       // From<id::Layer> is fallible
pub trait MassFunction {
    fn pdf(&self, m: f64) -> f64;                         // unnormalised ξ(m), per unit mass
    fn integral(&self, lo: f64, hi: f64) -> f64;          // ∫ξ dm, closed form or quadrature
    fn quantile_in(&self, lo: f64, hi: f64, u: f64) -> f64;   // inverse CDF restricted to [lo, hi]
    fn sample_in_band(&self, band: MassBand, stream: &mut Stream) -> f64;   // provided: one uniform
}
pub struct Kroupa; pub struct Chabrier { high_mass_scale: f64 }
pub enum MassFunctionKind { Kroupa, Chabrier }               // Chabrier (× 0.68) is the default
pub struct BandShares([f64; 5]);
impl BandShares {
    pub fn of(f: &impl MassFunction) -> Self;
    pub fn share(&self, b: MassBand) -> f64;
}
```

### `galaxy::ages`

```rust
pub struct AgeDistribution { /* mixture of uniform and exponential-history pieces */ }
impl AgeDistribution {
    pub fn min(&self) -> Years; pub fn max(&self) -> Years;   // min is −H for still-forming ones
    pub fn pdf(&self, age: Years) -> f64;
    pub fn cdf(&self, age: Years) -> f64;
    pub fn quantile(&self, u: f64) -> Years;                  // pure; the caller owns the stream
    pub fn sample(&self, stream: &mut Stream) -> Years;       // one uniform, then quantile
    pub fn born_fraction(&self) -> f64;                       // share with age ≥ 0 at the epoch
}
pub enum FeatureShare { None }                                // φ(age); plan 09 adds variants
```

### `galaxy::fates`

```rust
pub trait StellarFates {                                      // replaced by plans 06 and 11
    fn lifetime(&self, m: f64) -> Years;
    fn remnant_mass(&self, m: f64) -> f64;
    fn mean_companions(&self, m: f64) -> f64;
}
pub struct ProvisionalFates;
pub fn mean_present_mass(f: &impl MassFunction, fates: &impl StellarFates,
                         ages: &AgeDistribution) -> SolarMasses;
/// The same quadrature with nothing dead: initial mass of the primary plus its companions. Rates
/// quoted per solar mass formed (plan 09's Type Ia delay times, plan 11's class shares) use it.
pub fn mean_formed_mass(f: &impl MassFunction, fates: &impl StellarFates) -> SolarMasses;
```

### `galaxy::params`

`GalaxyParams::from_seed(seed: Seed, kind: MassFunctionKind) -> GalaxyParams`,
`GalaxyParams::milky_way_like() -> GalaxyParams`, `GalaxyParamsBuilder`, and getters returning
`units` newtypes for every entry of the table under P02.T5. Sub-structs: `DiscParams`,
`BulgeParams`, `BarParams`, `NuclearDiscParams`, `HaloParams` with `HaloComponentParams`,
`ArmParams`, `DarkHaloParams`, `GasDiscParams`, `NuclearClusterParams`, `AccretionHistory` with
`Progenitor`, `BlackHoleParams`. Derived getters: `system_count()`, `population_share(Population)`,
`population_mass(Population)`.

### `galaxy::potential`

```rust
pub struct MassModel { /* Gaussian components, NFW, black hole, nuclear cluster */ }
impl MassModel {
    pub fn new(params: &GalaxyParams) -> Self;
    pub fn v_circ_sq(&self, r: LightYears) -> f64;            // direct, (km/s)², in the plane
    pub fn enclosed_mass(&self, r: LightYears) -> SolarMasses;   // spherical
    pub fn potential(&self, r_cyl: LightYears, z: LightYears) -> f64;   // direct, (km/s)²
    pub fn vertical_force(&self, r_cyl: LightYears, z: LightYears) -> f64;
}
pub struct PotentialTables { /* in-plane tables, optional (R, z) grid */ }
impl PotentialTables {
    pub fn in_plane(model: &MassModel) -> Self;
    pub fn full(model: &MassModel) -> Self;
    pub fn v_circ(&self, r: LightYears) -> KilometresPerSecond;
    pub fn omega(&self, r: LightYears) -> PerYear;
    pub fn kappa(&self, r: LightYears) -> PerYear;
    pub fn potential(&self, r_cyl: LightYears, z: LightYears) -> Option<f64>;   // None without grid
    pub fn escape_speed_in_plane(&self, r: LightYears) -> KilometresPerSecond;
    pub fn escape_speed(&self, r_cyl: LightYears, z: LightYears) -> Option<KilometresPerSecond>;
    pub fn tidal_radius(&self, m: SolarMasses, p: &PointLy) -> Metres;   // SI: see D17
    pub fn bar_pattern_speed(&self) -> PerYear;
    pub fn bar_corotation(&self) -> LightYears;
}
```

The range query's padding speed of 1,000 km/s is plan 03's (`query::PAD_SPEED`), not a constant of
the potential.

### `galaxy::fields`

```rust
pub struct ComponentId(u8);                                   // index into Fields::components()
impl ComponentId { pub fn index(self) -> usize; }
pub struct Component { /* density form, normalisation, ages, metallicity */ }
impl Component {
    pub fn population(&self) -> Population;
    pub fn density(&self, p: &PointLy) -> f64;                // systems per ly³, all bands, φ = 0
    pub fn ages(&self) -> &AgeDistribution;
    pub fn metallicity(&self, p: &PointLy, age: Years) -> FehDistribution;   // mean and sigma
    pub fn halo_component(&self) -> Option<HaloComponentKind>;
}
pub const MAX_COMPONENTS: usize = 24;
pub struct Fields { /* components in a fixed order */ }
impl Fields {
    pub fn components(&self) -> &[Component];                 // fixed order, D18; at most 24
    pub fn component(&self, id: ComponentId) -> &Component;
    pub fn component_ids(&self) -> impl Iterator<Item = ComponentId>;
    pub fn densities(&self, p: &PointLy, out: &mut [f64; MAX_COMPONENTS]) -> f64;   // returns sum
    pub fn population_density(&self, pop: Population, p: &PointLy) -> f64;
    pub fn layer_density(&self, shares: &ShareMatrix, band: MassBand, p: &PointLy) -> f64;
    pub fn arms(&self) -> &arms::ArmGeometry;                 // shared with the gas field, plan 07
}
pub mod arms { pub struct ArmGeometry; pub struct SharpArm; pub struct GentleArm; }
pub mod vertical {                                            // a disc's cored profile, D9
    pub struct VerticalProfile;   // exponent, value, integral_to, effective_height, dispersion(_at)
}
```

### `galaxy::bounds`

```rust
pub struct CellBox { /* min corner in integer ly, edge in ly; never straddles an axis plane */ }
impl CellBox { pub fn new(min: [i32; 3], edge: u32) -> Result<Self, BuildCellBoxError>; }
pub struct ScalarRange { pub lo: f64, pub hi: f64 }
pub trait UnimodalFactor { fn sup(&self, range: ScalarRange) -> f64; }
impl Fields {
    pub fn component_bound(&self, id: ComponentId, cell: &CellBox) -> f64;
    pub fn component_bounds(&self, cell: &CellBox, out: &mut [f64; MAX_COMPONENTS]);
    pub fn layer_bound(&self, shares: &ShareMatrix, band: MassBand, cell: &CellBox) -> f64;
}
```

### `galaxy::shares`

`ShareMatrix::share(&self, band: MassBand, pop: Population) -> f64`;
`ShareMatrix::component_share(&self, band: MassBand, c: &Component) -> f64`, which looks up the
component's population; `ShareMatrix::uniform(&BandShares)`.

The matrix is keyed by population because the brainstorm's share is "one per layer and population".
Densities, bounds and the thinning pick are per component, because a sub-disc or a halo component
carries its own age distribution. The two fit together exactly: a component's density is already
normalised to its own share of its population's systems (a sub-disc's bin share, a halo component's
share of the halo), so a layer's density is Σ_c `component_share(band, c)` × density_c, which is
what `Fields::layer_density` and `Fields::layer_bound` compute and what plan 03's pick weights by.
The picked component gives the age distribution (`Component::ages`), the population
(`Component::population`) and, in the halo, the component mark (`Component::halo_component`).

### `galaxy::map`

```rust
pub enum MapView { FaceOn, EdgeOn }                           // edge-on looks along +y
pub enum MapSelection { AllSystems, YoungOnly }
pub struct MapSpec { /* view, selection, width_px, height_px, centre, extent in ly */ }
pub fn column_density_face_on(f: &Fields, x: f64, y: f64, s: MapSelection) -> f64;   // per ly²
pub fn column_density_edge_on(f: &Fields, x: f64, z_lo: f64, z_hi: f64, s: MapSelection) -> f64;
pub fn render_rows(f: &Fields, spec: &MapSpec, rows: Range<u32>, out: &mut Vec<f64>);
```

### Tables and test helpers

- `hyperion_sim::tables::mge::{MGE_EXP, MGE_BAR}` (provisional; plan 15 replaces them), in
  `crates/hyperion-sim/src/tables/mge.rs`, the path plan 15 keeps.
- `hyperion_sim::tables::gauss_legendre::{GL32_NODES, GL32_WEIGHTS, GL16_NODES, GL16_WEIGHTS}`.
- The crate `hyperion-fit` (`crates/hyperion-fit`), in its minimal first form: a thin `main.rs` over
  a library with one task, `hyperion-fit run mge`. Plan 15 extends this crate; it does not create
  it.
- Domain tags `galaxy.params.*` (scope `Galaxy`), listed under P02.T5 and registered in plan 01's
  `rng/tags.rs` under a "Plan 02" heading.

## Consumes

From plan 01 (determinism foundation), by the names of its Provides, which is authoritative:

- `hyperion_sim::math`: `exp`, `ln`, `log10`, `powf`, `powi`, `sin`, `cos`, `tan`, `atan2`, `tanh`,
  `hypot`, `erf`, `cbrt`. `f64::sqrt` is IEEE-exact and is called directly, as plan 01 states.
- `hyperion_sim::rng::{Seed, DomainTag, TagScope, ObjectKey, Stream, tags}`: streams are opened with
  `Stream::open(seed, tags::CONST, ObjectKey::galaxy())` or `ObjectKey::galaxy_item(n)`; tags are
  added to the single `domain_tags!` registry in `rng/tags.rs`; the samplers `uniform`,
  `uniform_in`, `normal`, `log_normal_dex`, `poisson` and `power_law`; `Stream::decide` with
  `Threshold::from_probability` for discrete choices.
- `hyperion_sim::units`: `LightYears`, `Metres`, `SolarMasses`, `Years`, `KilometresPerSecond`,
  `PerYear`, and the SI constants (`units::consts`) from which this plan derives G in its working
  units. If a newtype is missing, the task that first needs it adds it to `units` in the same style.
- `hyperion_sim::time::CLOCK_WINDOW_H`.
- `hyperion_sim::coords::GalacticPosition` (cell plus offset) for `PointLy::from`.
- `hyperion_sim::id::Layer` for `MassBand::try_from`.
- `GENERATOR_VERSION`; from the dev-only crate `hyperion-testkit`, `golden!` with `GoldenWriter`,
  `order::assert_order_independent` and `stats` (`chi_square_gof`, `ks_one_sample`,
  `assert_p_value`, `assert_poisson_count`, `ALPHA`); the slow-test marking `#[ignore = "slow: …"]`,
  `just test-slow`, `just bench`, `just bless`.

Nothing from any later plan. Plan 15 later extends the `hyperion-fit` crate that P02.T6.a creates,
and replaces the provisional tables.

## Design notes

**D1. Working units.** `galaxy` computes in light-years, solar masses, years and km/s, as the
brainstorm's "Coordinates" allows for generation code. G in those units is a `const` derived from
the SI constants in `units`, not typed as a literal. Public functions take and return newtypes; the
density hot path takes `&PointLy` and returns a bare `f64` documented as systems per cubic
light-year, because it is called millions of times and is summed across components.

**D2. One stream per parameter.** Each parameter is drawn from
`Stream::open(seed, tag, ObjectKey::galaxy())`, where `tag` is the constant that plan 01's
`domain_tags!` registry emits for `"galaxy.params.<name>"`, declared with scope `Galaxy`. Adding a
parameter later then moves no other. Discrete choices use the integer-threshold convention.
List-valued parameters (progenitors, halo components) use `ObjectKey::galaxy_item(n)` with the list
index as n. A parameter that needs several draws (a progenitor's orbit) takes them in a documented
order from its one stream.

**D3. Shares are of systems; masses follow.** The Populations table gives shares of the galaxy's
systems. With m̄ₚ the mean present-day mass per system of population p, the system count is N = M★ ÷
Σₚ shareₚ m̄ₚ and a population's stellar mass is N shareₚ m̄ₚ. The young share is not drawn: it is the
thin disc's share times the fraction of the declining formation history that falls in the last 100
Myr, which reproduces the brainstorm's 0.3–0.6% for timescales of 5–9 Gyr. The long bar's share is
30–40% of the combined bulge-and-bar share (see Risks R1).

**D4. Mean mass per system in M1: provisional fates, owned here.** The quadrature needs lifetimes,
remnant masses and companions, which arrive with plans 06 and 11. M1 uses `ProvisionalFates`:

- lifetime 10 Gyr × m^−2.5, floored at 3 Myr;
- remnant mass 0.109 m + 0.394 M☉ below 8 M☉ (Kalirai et al. 2008; re-check, not in the brainstorm's
  source list), 1.4 M☉ for 8–22 M☉, and min(0.4 m, 40 M☉) above;
- mean companions per primary 0.30, 0.60, 1.0 and 1.4 for primaries of 0.08–0.5, 0.5–1.5, 1.5–8 and
  over 8 M☉ (Duchêne and Kraus 2013; re-check), mass ratio uniform on 0.1–1, companions below 0.08
  M☉ not counted, each companion aged and evolved like a primary.

The acceptance test is the brainstorm's own figures: 0.55–0.59 M☉ per system under the default,
Chabrier's system function with its branch above 1 M☉ scaled by 0.68, and 0.48 under Kroupa's
(kept as Kroupa's), within 3% across the old populations; and the census as the arbiter between
the functions, 69% of all stars below 0.5 M☉ and 66–68% of primaries (Kirkpatrick et al. 2024;
Reylé et al. 2021; brainstorm, "Sizing the layers"). The fates sit behind the `StellarFates` trait so that plans 06
and 11 swap the implementation in one place. The consequence is stated plainly under Generator
version: N scales every density, so replacing the fates moves every star and bumps the version.
Before the first release that is free.

**D5. The mass function belongs to the generator version.** `Galaxy::new` uses the default,
Chabrier's system function with its branch above 1 M☉ scaled by the offline-fitted constant, a
provisional 0.68 until plan 15 fits it together with plan 11's companions (brainstorm, Decisions,
"2026-09-21: local density rulings", 2). Kroupa's stays supported: `with_mass_function` builds it,
for tests and for any version that chooses it.

**D6. The potential in M1, before plan 15's tables.** The brainstorm builds the potential from
Gaussian sums with dimensionless coefficients fitted offline. Plan 15 depends on this plan, not the
other way round, so M1 ships a first cut:

- The fitting tool's first cut is not an example inside the sim. The roadmap's code shape puts
  offline fitting in the separate crate `hyperion-fit`, which nothing depends on, and keeps the sim
  free of I/O. So P02.T6.a creates that crate in its minimal form, with one task: fixed log-spaced
  widths, non-negative weights by projected coordinate descent, no dependency but `hyperion-sim`
  (for `math`, so the table is bit-identical on every platform). Its output is committed as
  `crates/hyperion-sim/src/tables/mge.rs` with the header the roadmap requires. Plan 15 extends the
  crate (task registry, emitter, manifests, lock file), refits with free widths, and replaces the
  tables.
- Two one-dimensional profiles are enough for M1. `MGE_EXP` expands e^(−s). It serves a disc's
  radial profile, a disc's vertical profile (the product of the two expansions is a set of
  axisymmetric Gaussians with axis ratio t_k h ÷ s_i R_d), and the bulge. `MGE_BAR` expands the
  azimuthal average of the long bar's surface density at its nominal shape.
- The potential is axisymmetric, as the (R, z) tables imply. The boxy bulge enters as the spheroidal
  exponential with the same mass and the same second moments ⟨R²⟩ and ⟨z²⟩, found by one quadrature
  per galaxy. Plan 15 supplies a true two-dimensional expansion of exp(−m) per boxiness exponent.
- The old thin disc and the young disc enter the potential as one double exponential with the drawn
  mean height. The discs' vertical profiles are then solved in that potential in one pass, with no
  iteration (D9). The stars are cored in height (D9), but the potential keeps every disc exponential
  in height at its drawn height, which is the stars' effective height `Σ ÷ 2ρ₀`: the two have the
  same surface and mid-plane densities at every radius, so K_z agrees in the plane, where its slope
  is 4πGρ₀, and far above, where it is 2πGΣ. Between, the fixture's cored thin disc holds up to 5%
  of its 2πGΣ more within a given height, 4.6% of the whole K_z there, about 1.1 effective heights
  up. Holding the cored profiles in the potential would need a Gaussian expansion of each galaxy's
  tables and would make the solve an iteration; plan 15's fitted tables may replace the vertical
  expansion, a version bump.
- The NFW halo, the black hole and the nuclear cluster are spherical and are added in closed form or
  by a radial quadrature. They need no expansion.
- Every formula is pinned by a closed-form test (a spherical Gaussian against erf, a spherical
  exponential against its enclosed mass), so correctness does not rest on the fit.

**D7. In-plane tables first, the (R, z) grid on request.** Nothing in M1 reads Φ off the plane.
`PotentialTables::in_plane` builds v_c², its radial derivative and Φ(R, 0) on the radial grid and is
cheap. `PotentialTables::full` adds the 64 × 64 grid and costs about a second. `Galaxy::new` builds
the first. Plans 08–10 call `with_full_potential`. The server caches whichever it built.

**D8. The black hole's σ before plan 08.** M–σ reads the bulge's projected dispersion, which the
brainstorm takes from the Jeans table of plan 08. M1 computes it directly: a spherical isotropic
Jeans solution for the sphericalised bulge in the total in-plane rotation curve, projected and
averaged over the bulge's effective radius. Plan 08 replaces the estimator with its table's value,
which bumps the version. The relation is McConnell and Ma (2013), log₁₀(M ÷ M☉) = 8.32 + 5.64
log₁₀(σ ÷ 200 km/s), with 0.38 dex of scatter (re-check; not in the brainstorm's source list).
Because the black hole feeds the potential that σ is read from, σ is computed without the black hole
and the nuclear cluster, which change it by well under a per cent at the effective radius.

**D9. Sub-discs and every disc's cored profile.** Five sub-discs with age edges 0.1, 1, 2, 4, 7 and
10 Gyr. Each takes as its age the mean age of the formation history inside its bin, and as its
dispersion Sharma et al.'s (2021, MNRAS 506, 1761, eqs. 4 and 7, Table 2) exact heating law, not
its rounding: σ_z(τ, z) = s × 21.1 km/s × ((τ ÷ Gyr + 0.1) ÷ 10.1)^0.441 × (1 + 0.20 |z| ÷ kpc),
the rise stopping at 2.4 kpc, the reach of the heights it was fitted to (R18).
Each sub-disc's vertical profile is the vertical Jeans equation's solution for that dispersion in
K_z(R_ref, z) from `MassModel`, R_ref three thin-disc scale lengths: n(z) ÷ n(0) = (σ(0) ÷ σ(z))²
exp(−∫₀^|z| K_z ÷ σ² dz′), cored at the plane, tabulated once per galaxy (`fields/vertical.rs`) and
held at every radius. A disc's height is its effective height h = Σ ÷ 2ρ₀. The drawn mean height
(850–1,150 ly) is the old thin disc's effective height, the share-weighted harmonic mean 1 ÷ Σ wᵢ
÷ hᵢ of the sub-discs' (their mid-plane densities add), and it is met by the one dispersion scale s,
not by scaling the heights (brainstorm, Decisions, "2026-09-21: local density rulings", 1). The
young, thick and nuclear discs are cored the same way, each with a mid-plane dispersion of its own
that meets its drawn effective height. Plan 08 solves the same equation on the same profiles, so
age, height and vertical speed agree by construction.

**D10. The sharp arm.** The young disc's arm factor is 1 + f(R) A (g − 1), with g = exp(k (cos φ −
1)) ÷ I₀ₑ(k), where I₀ₑ(k) = e^(−k) I₀(k). The azimuthal mean of g is exactly 1 for any k, which is
the closed form the brainstorm asks for. The width is set in light-years by letting k depend on
radius: k(R) = (R sin p ÷ (n σ_w))², with n arms of pitch p, so that near a ridge g ≈ exp(−d² ÷
2σ_w²) in perpendicular distance d. Near the centre k is small and the factor degrades smoothly to a
cosine. The old disc takes 1 + f(R) a cos φ. The phase is φ = n (θ + ln(R ÷ L_bar) ÷ tan p): arms
trail a counter-clockwise rotation, and with two arms each ridge leaves the x axis at the bar's end.
The fade-in is f(R) = ½ (1 + tanh((R − L_bar) ÷ 0.1 L_bar)). The thick disc, the nuclear disc and
the other populations have no arms.

**D11. Normalisation and cuts.** Discs are normalised analytically over all space, as the brainstorm
accepts the loss of their tails beyond the cube. The boxy bulge and the bar are normalised by
quadrature or closed form over all space. A halo component is cut at ellipsoidal radius m = 65,000
ly (with axis ratios of at most 1 that lies inside the sphere of 65,000 ly) and normalised inside
the cut. A hard cut never rises, so the corner bound stays exact. The in-situ halo component is cut
at 50,000 ly.

**D12. Hooks held at zero.** Field density is budget × (1 − φ). `FeatureShare::None` makes φ = 0
until plan 09. The halo's discrete share (2–15%) is drawn now but the smooth components carry the
whole halo budget until plan 10. Both are generator-version changes when they switch on, which the
brainstorm already accepts ("Large features", last paragraph).

**D13. Age distributions.** Old thin disc: density proportional to exp(age ÷ τ) on 0.1–10 Gyr, split
by bin among the sub-discs. Young disc: the same history continued to −H, times 1 − φ(age). Thick
disc, bulge, bar: uniform on the table's ranges. Nuclear disc: 90% uniform on 8–12 Gyr, 5% on 1–8
Gyr, 5% on −H to 1 Gyr (Nogueras-Lara et al. 2020; re-check). Halo: one narrow uniform range per
component, inside 10–13 Gyr. A still-forming distribution is normalised so that the systems with age
≥ 0 equal the population's share; the unborn sliver, about 10⁻⁵ of the young population, is extra,
as the brainstorm's note on births in the window requires.

**D14. Metallicity is a distribution here, a draw later.** `Component::metallicity` returns mean and
sigma of [Fe/H] at a position and age. The draw belongs to the system stage (plan 06). Values the
brainstorm does not give are decided in P02.T7.e and marked for re-checking.

**D15. Parameters that exist only for the potential.** The gas disc (mass 10–20% of the thin disc's,
scale length 1.5–2 times the thin disc's, height 400 ly, no hole) and the nuclear cluster (mass
0.024 of the nuclear disc's with 0.2 dex of scatter, inner slope 1.3, break 10 ly, outer slope 3.5,
outside the population budgets) enter the mass model so that the rotation curve and the
enclosed-mass checks are right. Plans 07 and 09 own what they mean and may refine them.

**D16. Coupled sizes are clamped.** A size is its Milky Way value times (mass ÷ Milky Way mass)^⅓
times a log-normal scatter, clamped to the Populations table's range.

**D17. Tidal radius off the plane, and its unit.** Ω and κ are read at the spherical radius r = |p|
from the in-plane tables. The denominator 4Ω² − κ² is floored at 0.05 Ω², which is the floor the
brainstorm asks for where the rotation curve is nearly solid-body. The result is returned in
`Metres`, not in the working unit of D1. The radius is a sphere of influence, and everything that
reads it works in SI: plan 03's frame rule divides a `GalacticPosition::distance_to`, which is
`Metres`, by it; the system frame it bounds is in metres; and it runs from 2.7 au near the black
hole to a few light-years, where a light-year figure is the awkward one. The brainstorm's rule is SI
inside and other units at the edges, and this is not an edge.

**D18. Summation order is part of the output.** Bounds and densities are summed over components in
index order, and the component order is fixed and documented. Reordering is a version bump.

## Tasks

Order: T1, then T2, T3 and T4 (T2 ∥ T3; T4 needs both), then T5. After T5, T6 (potential) and T7.a,
T7.c–e (fields) run in parallel. T7.b needs T6.b. T8 (bounds) needs T7. T9 needs T2 and T7. T10
needs T7. T11 needs everything. Each task ends with `just ci` green and, if it changes generated
output, a bump of `GENERATOR_VERSION` with regenerated golden files.

### P02.T1 Module skeleton, constants and numerical helpers

Build `galaxy` with `PointLy`, `Population`, `POPULATIONS`, the working-unit constants (D1), and:

- `tables::gauss_legendre`: 16- and 32-node Gauss–Legendre nodes and weights as literal constants.
- `galaxy::quad`: `gl32(f, a, b)`, `gl16`, a composite `gl_panels(f, edges)`, a log-substituted
  variant for integrals over decades, and `bisect(f, lo, hi, iterations)` with a fixed iteration
  count so that results do not depend on a tolerance test.
- `galaxy::special`: `bessel_i0e(k)`, power series below k = 15 and the asymptotic series above.

Files: `crates/hyperion-sim/src/galaxy/{mod,consts,quad,special}.rs`,
`src/tables/{mod, gauss_legendre}.rs`, `src/lib.rs`.

Tests: weights sum to 2; `gl32` integrates x⁶² on [−1, 1] to 1 part in 10¹³; `bessel_i0e` against
twelve reference values to 10⁻¹²; continuity across k = 15; G in working units against 4.30091 ×
10⁻³ pc M☉⁻¹ (km/s)² converted.

Acceptance: `cargo test -p hyperion-sim galaxy::quad galaxy::special` passes; Clippy's
`disallowed_methods` finds no direct transcendental call.

### P02.T2 Mass functions and band shares

Build `imf` as under Provides. Kroupa (2001): ξ ∝ m^−1.3 on 0.08–0.5 M☉ and m^−2.3 above,
continuous. Chabrier (2003) system function: log-normal in log₁₀ m with centre 0.22 M☉ and width
0.57 up to 1 M☉, m^−2.3 above, continuous before the high-mass branch is multiplied by
`high_mass_scale`. `integral` is closed-form for power-law pieces (with the exponent-1 case handled)
and uses `erf` for the log-normal. `quantile_in` inverts piecewise, in closed form for power laws
and by `bisect` for the log-normal. Upper limit 150 M☉. `BandShares::of` integrates over
`MASS_BAND_EDGES`. No share is written down as a constant.

Files: `galaxy/imf.rs`.

Tests: shares against the brainstorm's table (Kroupa 76, 9.8, 11, 2.3, 0.64%; Chabrier with scale 1:
66, 12, 17, 3.7, 1.0%; the default, scale 0.68: 70, 12, 15, 2.6, 0.73%), each to the precision
printed; shares sum to 1; `quantile_in` is the inverse
of the restricted CDF at 1,000 points per band; a Kolmogorov–Smirnov test of 10⁵ quantile samples
per band; per-cell counts at the reference density 0.003 reproduce the table's "Per cell" column.

Acceptance: `cargo test -p hyperion-sim galaxy::imf` passes.

### P02.T3 Age distributions

Build `ages::AgeDistribution` as a mixture of pieces, each uniform or exponential-history (density ∝
exp(age ÷ τ)) on an interval, with closed-form `pdf`, `cdf` and `quantile`. Constructors:
`old_thin_disc(tau, bin)`, `young_disc(tau, FeatureShare)`, `uniform(lo, hi)`, `nuclear_disc()`,
following D13. `young_disc` and `nuclear_disc` start at −`CLOCK_WINDOW_H`. `born_fraction` gives the
share with age ≥ 0. `FeatureShare::None` is the only variant; the constructor's signature already
takes it so that plan 09 changes no caller. Also `young_fraction(tau)`: the share of a 10 Gyr
declining history that lies in the last 100 Myr.

Files: `galaxy/ages.rs`.

Tests: `young_fraction` is 0.32% at 5 Gyr and 0.55% at 9 Gyr; quantile inverts cdf; the five bins'
shares sum to 1 − `young_fraction`; a young-disc sample of 10⁶ has a negative-age share of H ÷ 100
Myr to within Poisson error; min of the young distribution is exactly −H.

Acceptance: `cargo test -p hyperion-sim galaxy::ages` passes.

### P02.T4 Provisional fates and the mean mass per system

Build `fates` per D4. `mean_present_mass` is a nested quadrature in ln m: for each primary mass the
expected present mass over the age distribution is m F(t(m)) + m_rem(m) (1 − F(t(m))), with F the
age CDF restricted to born systems, plus `mean_companions(m)` times the same expectation averaged
over the mass ratio. `mean_formed_mass` is the same quadrature with F = 1 everywhere, the mass
formed per system: it reads only the mass function and `mean_companions`, never a lifetime, so it is
one number per galaxy (`Galaxy::mean_formed_mass`), it does not change when plan 06's P06.T30 swaps
real lifetimes and remnant masses in, and it changes only when plan 11 replaces `mean_companions`.
Also `stars_below(f, fates, 0.5)`: the fraction of all stars, companions included, below a mass, for
the arbiter test of "Sizing the layers".

Files: `galaxy/fates.rs`.

Tests (the brainstorm's figures, "Galaxy parameters" first bullet and "Sizing the layers"): the
default, Chabrier (scale 0.68), 0.55–0.59 M☉ and Kroupa 0.48 ± 0.03 M☉ for a 10 Gyr declining
history; old populations within 3% of each other and the young disc 35–45% higher, under both;
stars per system 1.33–1.45; the arbiter is the census, 69% of all stars below 0.5 M☉ (Kirkpatrick et
al. 2024): Chabrier as published (66.9%) and scaled by 0.68 (70.9%) bracket it, its published share
of primaries below 0.5 M☉ (66%) lies in the census's 66–68%, and Kroupa's 76.4% fails;
`mean_formed_mass` equals `mean_present_mass` for an age distribution concentrated at zero age to
10⁻⁹ relative, and exceeds it for every population of the fixture.

Acceptance: `cargo test -p hyperion-sim galaxy::fates` passes. If a bracket fails, the constants of
D4 are tuned inside their sources' uncertainty and the change recorded in the doc comment; the
brackets are not widened.

### P02.T5 Galaxy parameters

Split in three.

**P02.T5.a Types, ranges and draws.** The structs under Provides, private fields, getters with
units, `GalaxyParamsBuilder` validating every range (`BuildGalaxyParamsError`). `from_seed` draws,
each on its own tag `galaxy.params.<name>` (D2). This task adds every tag below to `rng/tags.rs`
under a "Plan 02" heading, with scope `Galaxy` and the constant name the tag's upper-case form with
full stops as underscores (`GALAXY_PARAMS_STELLAR_MASS`). Where the table abbreviates a second
suffix (`.height_ratio`), it shares the first one's prefix (`thick.height_ratio`):

| Parameter (tag suffix)                | Distribution                                           |
| ------------------------------------- | ------------------------------------------------------ |
| `stellar_mass`                        | log-uniform 3–10 × 10¹⁰ M☉                             |
| `share.thick`, `share.bulge_bar`      | uniform 8–14%, 20–35%                                  |
| `share.bar_of_bulge`                  | uniform 30–40% of the above                            |
| `share.nuclear_disc`, `share.halo`    | uniform 1–2.5%, 0.7–1.4%                               |
| `sfh.timescale`                       | uniform 5–9 Gyr                                        |
| `thin.length.scatter`                 | normal, 0.05 dex (coupled, T5.b)                       |
| `thin.mean_height`, `young.height`    | uniform 850–1,150 ly, 130–200 ly                       |
| `thick.length_ratio`, `.height_ratio` | uniform 0.7–0.9 of thin length, 2.7–3.3 of thin height |
| `bulge.length.scatter`                | normal, 0.06 dex                                       |
| `bulge.b_over_a`, `bulge.c_over_a`    | uniform 0.5–0.7, 0.3–0.4                               |
| `bulge.boxiness`                      | uniform 3–4                                            |
| `bar.length.scatter`                  | normal, 0.05 dex                                       |
| `bar.width_ratio`, `bar.height`       | uniform 0.08–0.12 of half-length, 500–700 ly           |
| `bar.corotation_ratio`                | uniform 1.0–1.4                                        |
| `nuclear.length.scatter`              | normal, 0.04 dex                                       |
| `nuclear.height_ratio`                | uniform 0.3–0.5                                        |
| `nuclear_cluster.mass.scatter`        | normal, 0.2 dex (D15)                                  |
| `arms.count`                          | 2 or 4, equal odds                                     |
| `arms.pitch`                          | uniform 10–18°                                         |
| `arms.young_width`, `.young_fraction` | uniform 250–500 ly, 0.7–0.9 (σ_w and A of D10)         |
| `arms.old_amplitude`                  | uniform 0.10–0.30                                      |
| `gas.mass_fraction`, `.length_ratio`  | uniform 0.10–0.20, 1.5–2.0                             |
| `dark.f_star`                         | log-uniform 0.12–0.45                                  |
| `dark.concentration.scatter`          | normal, 0.11 dex                                       |
| `bh.scatter`                          | normal, 0.38 dex                                       |
| `metallicity.gradient`                | uniform −0.07 to −0.04 dex per kpc                     |
| `halo.*`                              | per component, below, tags listed after the table      |
| `accretion.*`                         | below, tags listed after the table                     |

The `halo.*` tags: `halo.in_situ.share`, `halo.in_situ.flattening`, `halo.in_situ.core`,
`halo.dominant.share`, `halo.dominant.flattening`, `halo.dominant.core`,
`halo.dominant.break_radius`, `halo.dominant.break_steepening`, `halo.lesser.count`,
`halo.lesser.share_total`, `halo.debris.share`, `halo.debris.slope`, `halo.debris.core`,
`halo.discrete_share`, and, keyed by `ObjectKey::galaxy_item(n)` with n the component's place in the
order in situ, dominant, lesser 1 to 5, debris: `halo.component.slope`, `halo.component.age` and
`halo.component.feh`, plus `halo.lesser.split` and `halo.lesser.flattening` with n the lesser
progenitor's number. The `accretion.*` tags: `accretion.last_major_merger`,
`accretion.recent.count`, `accretion.globular_count.scatter`, and, keyed by progenitor number (old
progenitors first, then recent ones): `accretion.progenitor.mass`, `accretion.progenitor.time` and
`accretion.progenitor.orbit`, whose stream yields apocentre, pericentre, inclination, node and phase
in that order.

Halo components ("Streams and accreted structure", table): each a cored, flattened, broken power
law. In-situ share 15–30%, flattening 0.45–0.55, core 1,500–3,000 ly; dominant merger 35–60%,
flattening 0.6–0.8, core 2,000–5,000 ly, break radius 52,000–91,000 ly (16–28 kpc) beyond which the
slope steepens by 1.5–2.5; lesser progenitors, 2–5 of them, 10–25% together split by a
stick-breaking draw, each with flattening 0.6–1.0 and its own age and [Fe/H]; globular-born debris
8–15%, slope 4.0–4.5, core 3,000–5,000 ly; discrete share 2–15%. Inner slopes otherwise 2.2–2.8
(Deason et al. 2011; Xue et al. 2015; Pila-Díez et al. 2015; Iorio et al. 2018; Medina et al.
2024). The shares are renormalised to sum to 1.

Accretion history: last major merger uniform 6–11 Gyr ago; the dominant and lesser progenitors
above; recent progenitors Poisson with mean 8, accreted within 6 Gyr, stellar masses from M^−1.45 on
10⁵–10⁹·⁵ M☉; each progenitor carries mass, time and an orbit (apocentre, pericentre, inclination,
node, phase) drawn from broad provisional ranges that plan 10 revalidates; globular count M₂₀₀ ÷ 6.5
× 10⁹ M☉ with 0.2 dex of scatter, clamped to 80–800 (Burkert and Forbes 2020; re-check).

Tests: every getter within its range over 10⁴ seeds; marginal distributions by Kolmogorov–Smirnov;
removing one draw in a test build changes no other parameter (order independence); builder rejects
each out-of-range value with the right error.

**P02.T5.b Derived quantities and coupling.** In `build()`: age distributions, m̄ₚ (T4), N and
population masses (D3), then the coupled sizes (D16): thin length 8,480 ly × (M_thin ÷ 3.4 ×
10¹⁰)^⅓, clamp 7,000–11,500; bulge a = 2,300 ly × (M_bulge ÷ 1.3 × 10¹⁰)^⅓, clamp 1,700–3,000; bar
half-length 16,000 ly × (M_bar ÷ 5.6 × 10⁹)^⅓, clamp 10,000–18,000; nuclear disc length 290 ly ×
(M_nd ÷ 1.05 × 10⁹)^⅓, clamp 200–400. The young disc shares the thin disc's length. Dark halo: M₂₀₀
= M★ ÷ (0.157 f★); log₁₀ c₂₀₀ = 0.905 − 0.101 log₁₀(M₂₀₀ h ÷ 10¹² M☉) plus scatter, h = 0.671, r₂₀₀
from 200 times the critical density (Dutton and Macciò 2014, NFW fit at redshift zero; re-check
coefficients). The black hole's mass is filled in by T6.e.

Tests: N within 0.5–1.8 × 10¹¹ over 10⁴ seeds under the default (M★'s 3–10 × 10¹⁰ M☉ over its
0.55–0.58 M☉ per system; 0.5–2.1 under Kroupa's); population masses sum to M★ to 1 part in 10¹²; sizes
correlate with masses at the expected slope; M₂₀₀ within 1.4 × 10¹²–5.3 × 10¹² × (M★ ÷ 10¹¹).

**P02.T5.c Milky Way fixture and golden file.** `GalaxyParams::milky_way_like()`: the default mass
function; M★ 6.0 × 10¹⁰ M☉; shares thick 10%, bulge and bar 31% with the bar 30% of that, nuclear
disc 1.75%, halo 1%; timescale 7 Gyr; thin length 8,480 ly and effective height 1,000 ly; bulge
2,280 × 1,440 × 820 ly, boxiness 3.5; bar half-length 16,000 ly, height 590 ly, corotation ratio
1.2; nuclear disc 290 ly by 93 ly; four arms at 12°; f★ 0.32; the halo's inner slopes 2.5 and the
dominant merger's break at 58,700 ly (18 kpc), steepening by 2.0 (Pila-Díez et al. 2015; Medina et
al. 2024); no scatter anywhere. Values may be tuned within the cited measurements so
that the checks of T11 pass, and each is cited (Bland-Hawthorn and Gerhard 2016; Wegg and Gerhard
2013; Wegg, Gerhard and Portail 2015; Launhardt et al. 2002; Sormani et al. 2022; McMillan 2017;
Licquia and Newman 2015). Golden file `tests/golden/galaxy_params.golden` for three pinned seeds.

Files: `galaxy/params/{mod,draws,derive,milky_way}.rs`, `tests/galaxy_params.rs`,
`tests/golden/galaxy_params.golden`.

Acceptance for T5, and for each subtask with its own tests:
`cargo test -p hyperion-sim galaxy_params` passes and `just ci` is green; plan 01's tag-collision
assertion and `rng/tags.golden` (re-blessed with the new tags as added lines only) pass; the golden
file is stable across two runs and across `--release`.

### P02.T6 Potential

**P02.T6.a The first-cut fitting tool and its tables.** Create the workspace crate
`crates/hyperion-fit` per D6, minimal: `Cargo.toml` (`[lints] workspace = true`; its only dependency
is `hyperion-sim`, through the workspace table; the workspace's `members = ["crates/*"]` picks the
crate up, and it is not added to `[workspace.dependencies]`, because nothing may depend on it); a
thin `src/main.rs` that parses `run mge [--out <path>]`, calls the library and maps its error enum
(`RunFitError`) to an exit code; `src/lib.rs`; `src/tasks/mge.rs` with `fit() -> MgeTables` and
`render(&MgeTables) -> String`. `run mge` writes the rendered source to `--out`, by default
`crates/hyperion-sim/src/tables/mge.rs`. The fit is single-threaded, uses no randomness, and takes
every transcendental from `hyperion_sim::math`, so its output is the same on every platform. It
targets e^(−s) on s from 0.02 to 12 (weights chosen so that relative error is uniform) and the bar
profile below, with 12 to 16 widths, log-spaced. Order of work: the crate first, which needs only
`math`; run it; then commit `src/tables/mge.rs` and declare `tables::mge` in the sim. The table
holds `MGE_EXP` and `MGE_BAR` as `[(weight, width); N]` under a header naming the tool
(`hyperion-fit run mge`), its inputs and its version 0. The bar profile is the azimuthal average of
the long bar's surface density at width ratio 0.1, level to 0.85 of the half-length with a Gaussian
end of 0.15 half-lengths, in units of the half-length.

Tests (in the sim, independent of the tool): Σ weight × Gaussian reproduces e^(−s) to 1% on 0.05–8
and the three-dimensional mass of the spheroidal exponential to 0.1%; the same for the bar profile
to 3% and its mass to 0.5%; all weights non-negative.

In `hyperion-fit`, the test `mge_table_is_reproduced` renders the fit and compares it byte for byte
with `include_str!` of the committed table.

Acceptance: `cargo test -p hyperion-fit` passes, so CI fails on a stale or hand-edited table;
`cargo run -p hyperion-fit -- run mge --out <scratch file>` writes the same bytes; the table tests
in the sim pass; `cargo tree -p hyperion-sim` and `cargo tree -p hyperion-server` do not list
`hyperion-fit`; the sim's `[dependencies]` still hold `libm` alone; `just ci` green.

**P02.T6.b Gaussian components: force and potential.** `potential::mge::Gaussian { mass, sigma, q }`
with, for ε = 1 − q² and the integrals over T from 0 to 1 by `gl32`:

- Φ(R, z) = −G M √(2 ÷ π) ÷ σ × ∫ exp(−T² (R² + z² ÷ (1 − εT²)) ÷ 2σ²) ÷ √(1 − εT²) dT;
- v_c²(R) = G M √(2 ÷ π) R² ÷ σ³ × ∫ T² exp(−T² R² ÷ 2σ²) ÷ √(1 − εT²) dT, and its R-derivative in
  the same form;
- K_z(R, z) = G M √(2 ÷ π) z ÷ σ³ × ∫ T² exp(…) ÷ (1 − εT²)^(3÷2) dT.

Valid for prolate components too (ε < 0), which the nuclear disc's tall Gaussians produce. Sources:
Binney and Tremaine 2008 §2.5; Emsellem, Monnet and Bacon 1994 and Cappellari 2002 (re-check; not in
the brainstorm's list). Builders: `double_exponential(mass, length, height)`,
`spheroidal_exponential(mass, a_r, a_z)`, `bar_disc(mass, half_length, height)`.

Tests: q = 1 against M(<r) = M [erf(x ÷ √2) − √(2 ÷ π) x e^(−x²÷2)]; a spherical exponential built
from `MGE_EXP` against M [1 − e^(−x)(1 + x + x² ÷ 2)] to 1% on 0.1–8 scale lengths; a thin double
exponential's v_c peaks at 2.1–2.3 scale lengths (Freeman 1970); K_z far above a disc tends to 2πGΣ
inside a few scale lengths; the derivative against a finite difference; Φ → −GM ÷ r at large r.

**P02.T6.c Spherical components.** `nfw::Nfw { m200, c, r200 }` with closed-form M(<r), Φ(r), v_c²
and derivative; `PointMass`; `BrokenPowerLaw` for the nuclear cluster with M(<r) and Φ by radial
quadrature tabulated at construction. Tests: NFW M(<r₂₀₀) = M₂₀₀; Φ finite at 0 and → 0 at infinity;
escape speed from the halo alone is finite; nuclear cluster mass converges to its parameter.

**P02.T6.d Mass model and tables.** `MassModel::new` assembles: thin-plus-young disc, thick disc,
gas disc, nuclear disc, bar disc, bulge spheroid (moment-matched, D6, with the dimensionless moments
of exp(−m) found by a two-dimensional `gl32` quadrature), NFW, nuclear cluster, black hole.
`PotentialTables`: a radial grid of 64 points log-spaced from 2⁻⁴ to 2¹⁸ ly holding v_c², dv_c² ÷ dR
and Φ(R, 0) for the extended components, interpolated by cubic Hermite in ln R, with the black hole
and the NFW halo added in closed form at lookup; `full` adds Φ on 64 × 64 in (R, |z|), bicubic in
the logarithms. Ω = v_c ÷ R; κ² = (1 ÷ R) dv_c² ÷ dR + 2 v_c² ÷ R²; escape speed √(−2Φ), with Φ → 0
at infinity for every component; `tidal_radius` per D17; `bar_corotation` = ratio × half-length and
`bar_pattern_speed` = v_c ÷ R there. `tidal_radius` converts its result to `Metres` through `units`.
Below the grid's inner edge the extended part is extrapolated as solid-body and the black hole
dominates.

Tests: interpolated against direct at 500 off-grid radii to 10⁻³; κ between Ω and 2Ω everywhere
outside 1 ly; κ → Ω near the black hole; tidal radius equals R (m ÷ 3M)^⅓ around a lone point mass;
the floor engages for a synthetic solid-body curve; golden values for three seeds.

**P02.T6.e The black hole.** The σ estimator of D8 (`potential/sigma.rs`): sphericalised bulge
tracer, isotropic Jeans σ_r²(r) = (1 ÷ ν) ∫ ν v_c² ÷ r dr, projection, mass-weighted mean inside the
effective radius (the projected half-mass radius, by bisection). `GalaxyParams` gains the black
hole's mass through a two-phase build: `MassModel` without it, σ, M–σ with the drawn scatter, then
the final model. Tests: the fixture's σ is 95–125 km/s and its black hole, without scatter, within a
factor of 2.5 of 4.3 × 10⁶ M☉; over 10³ seeds 90% of σ lie in 90–135 km/s.

Files: `galaxy/potential/{mod,mge,nfw,spherical,model,tables,sigma}.rs`, `src/tables/mge.rs`,
`crates/hyperion-fit/{Cargo.toml,src/main.rs,src/lib.rs,src/tasks/mge.rs,tests/mge.rs}`,
`tests/galaxy_potential.rs`, `tests/golden/galaxy_potential.golden`.

Acceptance for T6, and for each subtask with its own tests:
`cargo test -p hyperion-sim galaxy_potential` passes and `just ci` is green; `just bench` reports
`PotentialTables::in_plane` and `full` (targets 50 ms and 2 s; a miss is a finding).

### P02.T7 Fields

**P02.T7.a Discs and arms.** `fields/disc.rs`: `ExponentialDisc { n0, length, profile, arm }` with
density n0 exp(−R ÷ length) f(|z|) × arm factor, f the disc's cored `VerticalProfile` (T7.b, D9;
`fields/vertical.rs`), and n0 = count ÷ (4π length² h) with h its effective height. `fields/arms.rs`: `ArmGeometry` (count, pitch, bar half-length, fade), `phase(x, y)`, `SharpArm` and
`GentleArm` per D10. At R = 0 the phase is undefined and the factor is 1 because f(0) is below 10⁻⁴;
the code returns 1 there explicitly. Components: young disc (sharp), thick disc (none), nuclear disc
(none).

Tests: the azimuthal mean of each arm factor is 1 to 10⁻⁶ at 40 radii by 4,096-point sums; the
ridge's full width at half maximum is 2.355 σ_w to 3% at 20,000 and 40,000 ly; with two arms the
ridge at R = L_bar lies on the x axis; ridges trail (θ of the ridge falls as R grows); each disc
integrates to its count to 0.1% by brute-force quadrature over the cube plus the analytic tail, whose
column is 2h.

**P02.T7.b The old thin disc as sub-discs, and every disc cored.** Per D9: bins, shares and ages
from T3; each sub-disc's profile from the vertical Jeans equation under Sharma et al.'s exact law
with `MassModel::vertical_force`, tabulated (`VerticalProfile`); the dispersion scale s by `bisect`
so that the sub-discs' harmonic effective height is the drawn one. Five `ExponentialDisc`
components with `GentleArm`, each with its bin's `AgeDistribution`. The young, thick and nuclear
discs' profiles likewise, each meeting its drawn effective height. The profile's exponent must
never fall with |z| in floating point, or the discs' bounds carry a margin (T8).

Tests: for the fixture the effective heights rise with age, the youngest and oldest within a
quarter of the brainstorm's "320 ly at half a gigayear to 1,700 ly at ten" read at their mean ages;
their harmonic mean equals the drawn height to 10⁻⁹; every disc is cored and has its drawn effective
height; the dispersion scale lies in 0.6–1.6 for the fixture and for at least 99% of 10³ seeds,
with the median within a tenth of 1, and follows √(column × drawn height) (correlation over 0.95);
far from the plane, a double exponential fitted to the fixture's discs at R₀ over 250–3,000 pc, as
star counts are, gives Bland-Hawthorn and Gerhard's (2016, §5.1.3) thin disc of 300 ± 50 pc, thick
disc of 900 ± 180 pc and thick share of 4 ± 2%; the local mean mass per system at R₀ is the
census's 0.55–0.59 M☉; the profile's exponent never falls across any knot of its table, stepped a
unit in the last place at a time.

**P02.T7.c Bulge and long bar.** Bulge: n0 exp(−m), m = {[(|x| ÷ a)² + (|y| ÷ b)²]^(c∥÷2) + (|z| ÷
c)^c∥}^(1÷c∥), normalised by 6 a b c times the unit body's volume, itself a one-dimensional
quadrature. Bar: n0 L(|x|) exp(−y² ÷ 2σ_y²) exp(−|z| ÷ h), with L level to 0.85 of the half-length
and a Gaussian end of 0.15 half-lengths, normalised in closed form. Tests: both never rise with |x|,
|y| or |z| (10⁵ random pairs); the bulge varies by at most 20% across any 128 ly cell of the
fixture; the fixture's central density is 0.17–0.30 per ly³ and 0.12–0.55 over 10³ seeds (the
brainstorm's "about 0.26 (0.13–0.5 over the ranges)"); counts by brute-force quadrature to 0.5%.

**P02.T7.d The halo as a marked mixture.** One component per smooth halo component, a broken power
law: n0 (1 + m² ÷ a²)^(−γ÷2) with m² = x² + y² ÷ p² + z² ÷ q² and an inner slope γ of 2.2–2.8, the
dominant merger's slope steepening by 1.5–2.5 beyond its break at 16–28 kpc through a continuous
factor that never rises, cut per D11, normalised by radial quadrature. `Component::halo_component`
returns the kind, so that plan 03's population pick is also the component mark. The discrete share
is carried but not applied (D12). Tests: monotone; counts to 0.5%; the mixture's spherically
averaged slope between 20,000 and 60,000 ly is −2.1 to −3.0 (inner); nothing beyond 65,000 ly.

**P02.T7.e Metallicity and assembly.** `FehDistribution { mean, sigma }`. Thin discs: mean = 0.0 +
gradient × (R − 3 lengths), flat in age to 8 Gyr and then falling about 0.1 dex per Gyr (Bergemann
et al. 2014; Casagrande et al. 2011), clamped, sigma 0.20; thick −0.55,
0.25; bulge 0.0, 0.40; bar 0.0, 0.30; nuclear disc +0.1, 0.30; halo per component (in situ −0.6,
dominant −1.2, lesser drawn −2.0 to −1.0, debris −1.5; sigma 0.3). All marked for re-checking
against Bland-Hawthorn and Gerhard 2016. `Fields::new(&GalaxyParams, &MassModel)` assembles the
components in the fixed order young, sub-discs 1–5, thick, bulge, bar, nuclear disc, halo components
(D18), and `densities`, `population_density`, `layer_density`. Tests: the gradient at 26,000 ly is
the drawn one; the age–metallicity relation is flat to 8 Gyr and 0.1 dex per Gyr poorer beyond;
component count at most `MAX_COMPONENTS`; Σ population counts = N; golden densities at 20 pinned
points for three seeds; the nuclear disc's central density for the fixture is 12–19 per ly³.

Files: `galaxy/fields/{mod,disc,vertical,sub_discs,arms,bulge,bar,halo,metallicity}.rs`,
`tests/galaxy_fields.rs`, `tests/golden/galaxy_fields.golden`.

Acceptance for T7, and for each subtask with its own tests:
`cargo test -p hyperion-sim galaxy_fields` passes and `just ci` is green; a Criterion bench reports
`Fields::densities` at a disc point (target under 400 ns; a miss is a finding, since plan 03's 1–2
µs per sparse cell rests on it).

### P02.T8 Bounds

**P02.T8.a Cell geometry and envelopes.** `CellBox::new` rejects a box that straddles x = 0, y = 0
or z = 0. Methods: `nearest_corner()`, `farthest_corner()`, `r_cyl_range()` (from the nearest and
farthest corners, as the brainstorm states), `centre()`, `in_plane_half_diagonal()`. The envelope
bound of every component is its arm-free density at the nearest corner. It works for any
power-of-two edge, so the 4 ly, 16 ly and 4,096 ly grids of later plans reuse it. A disc's vertical
profile is a table (T7.b), not a closed form: its exponent must not fall with |z| in floating
point, between knots or across them, or the disc's bound carries a relative margin that covers the
rise.

Tests: for every component of three seeds and 2,000 random cells per layer size, the envelope's
maximum over a 17³ lattice plus the corners never exceeds the bound, and equals it at the nearest
corner to 1 part in 10¹² (to the margin and one rounding where the corner is subnormal); the cells
include the discs' tables where their segments change width, where the dispersion stops rising and
at their end.

**P02.T8.b The unimodal-factor rule and the arm bounds.** The `UnimodalFactor` trait and
`ScalarRange`. Phase range of a cell: the phase at the centre ± n × half-diagonal ÷ (R_min sin p),
since |∇φ| = n ÷ (R sin p); the whole circle if R_min is 0 or the half-width reaches π. `GentleArm`:
1 + f(R_max) a × (1 if the range reaches a ridge, otherwise the larger end's cosine, floored at 0),
and 1 + f(R_min) a cos(…) when that cosine is negative. `SharpArm`: g is bounded by 1 ÷
I₀ₑ(k(R_max)) if the range reaches a ridge, otherwise by exp(k(R_min)(cos φ* − 1)) ÷ I₀ₑ(k(R_max))
at the end φ* nearer a ridge; the factor's bound is 1 + f(R_max) A (g_sup − 1) when g_sup ≥ 1 and
1 + f(R_min) A (g_sup − 1) otherwise. A disc's bound is envelope bound × factor bound. The doc
comment states the flared-layer case (peak at h = z β^(1÷β)) as the intended second implementor,
without code.

Tests: each factor bound against a dense scan of the factor over 5,000 random cells; tightness: the
young disc's mean bound ÷ mean density over 128 ly mid-plane cells between the bar's end and 40,000
ly is under 3.5 (the brainstorm's 2.8 was for a different profile; the figure is reported).

**P02.T8.c Layer bounds and the violation hunt.** `component_bound`, `layer_bound` = Σ share × bound
in component order. The hunt (slow test, "Testing", bound checks): for five seeds chosen to include
the sharpest arms, two and four arms, and the longest and shortest bars, walk every arm ridge from
0.8 to 2 bar half-lengths in steps of 16 ly; for each layer size take the cells the ridge crosses
and their neighbours; maximise the true layer density inside each by a 33³ lattice refined by
coordinate ascent; assert it never exceeds the bound. The same for 10⁴ random cells per layer across
the galaxy, including the central 1,000 ly. Golden bounds for 50 pinned cells.

Files: `galaxy/bounds.rs`, `tests/galaxy_bounds.rs`, `tests/golden/galaxy_bounds.golden`.

Acceptance for T8, and for each subtask with its own tests:
`cargo test -p hyperion-sim galaxy_bounds` and `just test-slow` pass with zero violations, and
`just ci` is green.

### P02.T9 Share matrix and the `Galaxy` handle

`ShareMatrix` stored as band × column with the seven populations as the first columns;
`uniform(&BandShares)` fills every column alike, as the brainstorm states for M1. `Galaxy` bundles
everything per Provides; it is `Send + Sync`, holds no interior mutability, and its size is reported
by a test so that plan 04 can budget its cache.

Files: `galaxy/shares.rs`, `galaxy/mod.rs`, `tests/galaxy_handle.rs`.

Tests: columns sum to 1; `layer_density` summed over bands equals the total density; `Galaxy::new`
twice gives equal values; `Galaxy::new` is independent of what was built before it (order
independence); a bench of `Galaxy::new` (target 100 ms).

Acceptance: `cargo test -p hyperion-sim galaxy_handle` passes.

### P02.T10 Column density for the galaxy map

**P02.T10.a Face-on.** Discs: 2 h × n0 exp(−R ÷ length) × arm factor, h the effective height. Bar: closed form, 2 h.
Bulge and halo components: `gl32` in z after a substitution that maps the half-line (or the cut)
onto the unit interval, as the brainstorm's "32-node quadrature per pixel". `YoungOnly` selects the
young disc alone. Tests: against a 4,000-step brute-force integral at 200 points to 10⁻³ (bulge and
halo) and 10⁻¹² (closed forms); the face-on map's integral over the plane equals N to 0.5%.

**P02.T10.b Edge-on.** Line of sight along +y, so the picture shows x across and z up and the bar
lies in its plane. Across a pixel's height the z-integral is closed-form for every disc, whose
tabulated profile is exponential across each segment of its table (`VerticalProfile::integral_to`,
exact for the table), and for the bar, and a 4-node quadrature for bulge and halo; the result is divided by the
pixel's height, so a disc thinner than a pixel keeps its light. Along the line of sight: fixed
panels symmetric about y = 0 with `gl16` per panel. Components without arms take eight log-spaced
panels a side, from 16 ly to their cut or the cube's edge. Discs with arms take panels no wider than
512 ly out to eight scale lengths, so that no panel spans an arm unresolved, and a pixel whose
vertical integral of a disc is below 10⁻⁹ of the disc's central column skips that disc. The panel
scheme is a constant of the generator version. Tests: against brute force to 3 × 10⁻³ at 200 pixels
including mid-plane pixels 256 ly tall; the young disc's mid-plane pixel differs from a point sample
by the expected factor.

**P02.T10.c Rows.** `MapSpec` (validated: non-zero size, extent inside the cube) and `render_rows`,
which evaluates pixel centres face-on and pixel strips edge-on, so that plan 04's pool can split a
map by rows. Golden values for a 16 × 16 map of three seeds in both views. Bench: a 512 × 512
face-on map and a 512 × 256 edge-on map on one thread (targets 1 s and 5 s; a miss is a finding).

Files: `galaxy/map.rs`, `tests/galaxy_map.rs`, `tests/golden/galaxy_map.golden`,
`benches/galaxy.rs`.

Acceptance, for each subtask with its own tests: `cargo test -p hyperion-sim galaxy_map` passes and
`just ci` is green; for T10.c also `just bench` reports the two maps.

### P02.T11 Milky Way comparisons and seed sweeps

Slow tests in `tests/galaxy_milky_way.rs` and `tests/galaxy_sweeps.rs`, marked as plan 01 defines.
Every loop over 10³ seeds or more in this plan is one of them, those of T5.a, T5.b, T6.e and T7
included: they live in `tests/galaxy_sweeps.rs`, and the fast suite runs the same checks over 32
seeds with brackets wide enough for that sample. A parameter set costs 7–8 ms to build here, most of
it T6.e's σ estimator (R15), and plan 08's P08.T4.d replaces that estimator with a Jeans solution of
about 100 ms, so the sweeps must not move when it does. Run them only under `just test-slow`'s
optimised profile: in a debug build a 10⁴-seed sweep takes the better part of an hour.

At `GalaxyParams::milky_way_like()`:

| Check                                  | Accept                                                      |
| -------------------------------------- | ----------------------------------------------------------- |
| Enclosed mass at 1 pc                  | 4–7 × 10⁶ M☉ (model 5.3 × 10⁶)                              |
| at 4 pc                                | 0.9–1.8 × 10⁷ M☉ (Fritz et al. 2016; Feldmeier et al. 2014) |
| at 100 pc                              | 2.9–4.9 × 10⁸ M☉ (Sormani et al. 2020)                      |
| at 230 pc                              | 0.8–2.0 × 10⁹ M☉ (Launhardt et al. 2002)                    |
| at 1 kpc                               | 7.5–10.5 × 10⁹ M☉                                           |
| at 2 kpc                               | 1.8–2.6 × 10¹⁰ M☉ (Portail et al. 2017)                     |
| v_c(1 kpc) ÷ v_c(8 kpc)                | 0.75–1.1                                                    |
| v_c at 0.5, 1 and 2 kpc                | against measured curves, not the research model (see below) |
| v_c at 8 kpc                           | 215–245 km/s (Eilers et al. 2019)                           |
| Escape speed at 8 kpc                  | 545–605 km/s (brainstorm: 574)                              |
| Bar pattern speed                      | 33–41 km/s per kpc                                          |
| Tidal radius, 1 M☉ at 26,000 ly        | 3.7–5.1 ly                                                  |
| In-plane density at R₀ and z☉          | 0.0018–0.0021 per ly³, azimuthal mean (see below)           |
| Mid-plane stellar mass density at R₀   | 0.0375–0.0455 M☉ pc⁻³ (McKee et al. 2015: 0.0415 ± 0.004)   |
| Nuclear disc share and central density | 1.2–2.4%; 12–19 per ly³                                     |

Enclosed mass here is `MassModel::enclosed_mass` for the spherical components plus the mass of each
field component inside the sphere by quadrature of its true (not axisymmetrised) density.

The density row reads at R₀ = 26,670 ly (8.18 kpc; GRAVITY Collaboration 2019) and the Sun's
height, 20.8 pc (Bennett and Bovy 2019), and counts systems with a star or white dwarf: the 20 pc
census gives 0.00193 ± 0.00004 per ly³ (Kirkpatrick et al. 2024) and the 10 pc census 0.00184 ±
0.00011 (Reylé et al. 2021; brainstorm, Decisions, "2026-09-21: local density rulings", 3). The mass
density row excludes brown dwarfs, as the stellar layers do. After P02.T7's revision the fixture
gives 0.00297 and 0.061, 1.54 and 1.47 times these, because its stars' surface density at R₀ is 39
M☉ pc⁻² (R18). T11 tunes the fixture's Σ★ to 28–30 M☉ pc⁻², with a shorter thin scale length
(2.2–2.4 kpc; Bland-Hawthorn and Gerhard's 2.6 ± 0.5, Bovy and Rix's mass-weighted 2.15 ± 0.14) or
less stellar mass, within their cited measurements, and its rotation-curve and enclosed-mass checks
must absorb the change (through f★ and the gas disc, whose column at R₀, 6.6 M☉ pc⁻², is half the
measured 13.7 ± 1.6). A thin effective height nearer the top of its drawn range lowers the density
too, about 8% at 1,100 ly, and lifts the far-field thin disc from 259 to 286 pc.

The inner rotation curve row is added after T6: the fixture gives v_c = 150, 180 and 216 km/s at
0.5, 1 and 2 kpc, 8–11% below the brainstorm research model's 163, 202 and 239, while every enclosed
mass above is inside its measured bracket. The research figures are a model, not data. T11 takes the
brackets from published measurements of the inner Milky Way's circular speed (for example Portail et
al. 2017's dynamical model, or Sofue 2013, noting that terminal velocities inside the bar are biased
by non-circular motion), each with its citation. A value outside them is a finding against the model
(the inner Gaussian fit, the bulge's share or scale), never a reason to move the fixture's draws.

Over 4,000 seeds (parameters and `MassModel::v_circ_sq` only, no tables): v_c at 8 kpc has its
median in 225–255 km/s and at least 68% in 210–270 km/s; the median slope from 5 to 16 kpc is within
±4 km/s per kpc; v_c(1 kpc) stays under 300 km/s for 99% of seeds (the uncoupled draft reached 390);
the median of v_c(1) ÷ v_c(8) is 0.85–0.97. Over 1,000 seeds: the in-plane density at 26,000 ly lies
in 0.0008–0.008 for at least 98%; the total central density stays under 30 per ly³, so no layer's
index can overflow.

Files: the two test files.

Acceptance: `just test-slow` passes. A failing bracket is resolved by tuning the fixture within its
cited uncertainties or, if the model is at fault, by reporting it; brackets are not widened
silently.

## Verification

- `just ci` and `just test-slow` green.
- Golden files for parameters, potential tables, densities, bounds and map pixels, stable across
  debug and release and, once CI has them, across x86-64 and AArch64.
- The Milky Way table and the sweeps of P02.T11.
- The bound hunt of P02.T8.c with zero violations, and the tightness figure reported.
- Order independence: building galaxies in any order, or one parameter alone, gives equal results.
- Benchmarks recorded, not enforced: `Fields::densities` under 400 ns, `Galaxy::new` under 100 ms,
  full tables under 2 s, maps under 1 s and 5 s.
- By eye, once plan 05 exists: a face-on map reads as a barred spiral with arms leaving the bar's
  ends, and the young-only map shows narrow arms of even width.

## Generator version

This plan creates generated output, so its tasks bump `GENERATOR_VERSION` as they land (T5, T6.e,
T7, T8, and T10 for the map's panel scheme). It reserves, so that later plans move as little as they
can:

- one domain tag per parameter under `galaxy.params.*`, never renamed;
- the `FeatureShare` argument and the halo's discrete share, both held at zero;
- `ShareMatrix` columns beyond the seven populations, for displaced classes;
- the `UnimodalFactor` trait for flared layers and a peanut bulge;
- the accretion history, globular count, gas disc and nuclear cluster as parameters already drawn.

Known future bumps that originate here, each moving every star because N, the potential or the
discs' vertical profiles change: plan 06 replaces lifetimes and remnant masses in `StellarFates`;
plan 11 replaces the companion model; plan 15 replaces `MGE_EXP`, `MGE_BAR`, the bulge's spheroid
and Chabrier's scale, and may replace the discs' exponential vertical expansion in the potential
(D6); plan 08 replaces the σ estimator behind the black hole's mass; plan 09 turns on φ
and plan 10 the halo's discrete share; plan 07 may refine the gas disc. The bound's formulae, the
component order and the map's quadrature scheme belong to the version as well.

## Risks and open points

- **R1. "30–40% of the bulge's figure."** Read as: the long bar takes 30–40% of the combined
  bulge-and-bar share of 20–35%. The other reading, 0.3–0.4 times the bulge alone, would make the
  bar smaller by about a quarter. The reading chosen matches Portail et al. (2017).
- **R2. "Averaging 850–1,150 ly."** Read with the research note behind it: the seed draws the mean
  height and the Jeans solution sets the ratios (D9). If the scale factor strays far from 1, the
  model's disc mass and the measured heating law disagree, and T7.b reports it. Since the
  2026-09-21 rulings the height is the old thin disc's effective height and the factor is the
  dispersion scale on the heating law (R18).
- **R3. The brainstorm's "about five" sub-discs** against six ages in its research note. Five bins
  are used.
- **R4. Black hole and σ.** The brainstorm reads M–σ from the bulge's projected dispersion of
  105–115 km/s, which its own model takes from the axisymmetric Jeans table that plan 08 builds; M1
  cannot. D8's spherical isotropic estimator is the stand-in, and it may sit some km/s off the
  tabulated value, which at a slope of 5.64 is tens of per cent in mass. The relation's source
  (McConnell and Ma 2013) is not in the brainstorm's list and is re-checked when coded. The swap in
  plan 08 is a version bump, listed under Generator version.
- **R5. The moment-matched bulge** may miss the enclosed-mass brackets at 1–2 kpc. The fallback is a
  spherical component with the bulge's true enclosed mass by quadrature, which serves the in-plane
  tables equally well until plan 15.
- **R6. The exponential thin disc without a central hole** makes v_c at 2 kpc about 10% high. The
  brainstorm's research accepted this because a hole would break monotonicity; the bracket at 2 kpc
  allows for it.
- **R7. The sharp arm's contrast grows with radius**, because a fixed width against a growing arm
  spacing at mean one forces it. Peaks of 10–30 times the mean at the rim are expected for the young
  disc. If the map looks wrong by eye, the width can grow slowly with radius without touching the
  bound rule.
- **R8. `hyperion-fit` starts here**, not in plan 15, because plan 15 depends on this plan and the
  potential needs a table in M1. Plan 15's P15.T1 must extend the crate this plan creates and keep
  `run mge` and the table's path, and its P15.T3 must supply the long bar's expansion that `MGE_BAR`
  stands in for.
- **R9. Cost of `full` tables** (about 500 Gaussians × 4,096 points × 32 nodes). If it exceeds a few
  seconds, the vertical expansion is cut to fewer Gaussians for the grid only.
- **R10. Provisional ranges** the brainstorm does not give (arm width and fraction, halo cores and
  flattenings, thick-disc ratios, gas disc size, metallicity means, progenitor orbits, the odds of
  two against four arms) are decisions of this plan and are the first things to tune by eye.
- **R11. D4 re-checked (P02.T4).** Lifetimes are Raiteri, Villata and Navarro (1996) at Z = 0.02,
  not 10 Gyr × m^−2.5, and companions above 8 M☉ are Duchêne and Kraus's 1.0 (8–16 M☉) and 1.3,
  not 1.4: D4's law spread the old populations by 6.7% against the brainstorm's ±3%.
- **R12. Halo ages and accretion re-checked (P02.T5).** Independent draws let a component's stars
  be younger than the event that put them in the halo. Now the in-situ and dominant components'
  age centres are drawn on [max(10.5 Gyr, merger + 0.5 Gyr), 12.5 Gyr] and a lesser progenitor's
  accretion on [6 Gyr, min(12 Gyr, its youngest stars)], each on its own stream with one word, and
  the dominant merger's orbit takes an eccentricity of 0.85–0.95 (Belokurov et al. 2018) in place
  of the broad pericentre range. Where the ages move, so do the halo's mean mass per system and
  everything that follows from N, by about 10⁻⁵.
- **R13. The black hole and σ as built (P02.T6.e).** The estimator follows D8 exactly; a direct
  projection integral reproduces the fixture's 119.3 km/s to 4 × 10⁻⁵, and the sphericalised bulge
  is the sphere of the bulge's mass and central density (matching its second moment gives 123.6). At
  119.3 km/s McConnell and Ma's relation gives 1.13 × 10⁷ M☉, 2.6 times Sgr A*'s (4.297 ± 0.012) ×
  10⁶ (GRAVITY Collaboration 2022, A&A 657, L12): the Milky Way lies 0.421 dex below the relation,
  1.1 times its scatter. So the fixture's M–σ scatter is −0.421 dex, not T5.c's zero, its black hole
  is 4.30 × 10⁶ M☉, T6.e's factor of 2.5 is checked on that mass, and T11's enclosed mass at 1 pc
  comes to 5.15 × 10⁶. Plan 08 resets the offset when it replaces the estimator. Over 10³ seeds 86%
  of σ lie in 90–135 km/s (median 114), not 90%, because the isotropic spherical stand-in sits 5–10%
  above the axisymmetric value (R4); the sweep asserts 85% until plan 08.
- **R14. `MGE_BAR` against its tests (P02.T6.a).** A non-negative sum of centred Gaussians is
  log-convex in u², and the bar's Gaussian end is log-concave, so no such sum meets 3% locally. By
  linear programming over 14 or 16 log-spaced widths, the best maximum relative error is 12% out to
  one half-length and 30% out to 1.1, and holding even the level part to 10% puts the mass over 0.5%
  off. More or re-placed widths and relative weighting do not help, so the table stands. The 3% is
  read against the central value (2.9%), the mass is met (0.2%), and the mass inside a radius, which
  the rotation curve reads, is off by up to 6.6% just beyond the half-length (4.4% at best with the
  other two held), bounded at 7% by the test. P15.T3.a's 2% for the bar meets the same limit unless
  it allows negative weights or off-centre terms.
- **R15. Other as-built points (P02.T6).** `PotentialTables::full` takes 2.7 s against 2 s (R9):
  4,096 points × 767 Gaussians × 32 nodes. 44% of the evaluations regenerate their nodes, 32%
  because the point lies beyond the Gaussian's cut; sharing the other 12% along a column would save
  under 10%, so the vertical expansion is not cut. `GalaxyParams::from_seed` takes 7–8 ms, not "well
  under a millisecond" (T11), because T6.e's σ builds the black-hole-free model (1.1 ms) and reads
  its rotation curve at 16 radii (5.5 ms); a recomputed constant and unused node caches, about 0.3
  ms, were removed. `MassModel::enclosed_mass` holds the spherical components only, as T11 reads it,
  and `expanded_enclosed_mass` adds the Gaussians. The nuclear cluster's break is sharp, so its mass
  and potential are closed forms, not tabulated quadratures, and the bulge's moments are Beta
  functions, checked against the two-dimensional `gl32` quadrature. At Milky Way values v_c is 150,
  180 and 216 km/s at 0.5, 1 and 2 kpc, 8–11% under the research note's model (163, 202, 239), with
  every enclosed mass of T11 inside its bracket and v_c(1 kpc) ÷ v_c(8 kpc) 0.81 against the note's
  Milky Way 0.87. `GENERATOR_VERSION` is 3.
- **R16. Deviations in T7, as built (P02.T7).** `GENERATOR_VERSION` is 4, one bump for T7 with its
  validation; the goldens other than the new `galaxy_fields.golden` changed their header line
  only. The acceptance filter `galaxy_fields` selects only the golden test: T7 runs as
  `cargo test -p hyperion-sim --test galaxy_fields` and `--lib galaxy::fields`. The sub-bullets on
  the sub-discs' heights, the halo's slope, the metallicity constants and the local density,
  which the 2026-09-21 density rulings resolved, are retired; R18 records the revision, and the
  figures below are those of T7 as first built.
  - _The halo's cut is a sphere, not D11's ellipsoid in m (T7.d)._ It is the brainstorm's "out to
    65,000 ly" (50,000 in situ). The ellipsoid ended a component at q × 65,000 ly over the poles
    (45,000 for the fixture's dominant merger), so the spherically averaged slope between 20,000
    and 60,000 ly measured that truncation, not the halo. In the plane the two cuts coincide. p =
    1, as the parameters draw q only. The break B(m) = min(1, (m ÷ r_b)^−Δ) switches where m² ÷ r_b²
    exceeds 1, so every step of the profile is monotone in floating point too. The normalisation
    is 4π ∫₀¹ s(μ)⁻³ F(r_c s(μ)) dμ, F read from `Gl16Panel` partial integrals.
  - _Bulge central density over 10³ seeds (T7.c; under Kroupa's function, superseded by R18's
    scaled brackets)._ Median 0.33 per ly³, 93% in 0.14–0.63, range
    0.09–1.17; fixture 0.31. With the sizes coupled (D16) it is 1 ÷ (m̄ 6V(c∥) (b ÷ a)(c ÷ a) a₀³
    10^(3s)), free of the stellar mass, the share and N, and the 0.06 dex length scatter s enters
    it cubed. Kroupa's lower m̄ moves the median by 1.19, not the width. The tests assert at least
    nine in ten inside 0.14–0.63 and the median in 0.25–0.40. The normalisation is the closed form
    6abc V(c∥), V(p) = 4π B(2 ÷ p, 1 ÷ p + 1) ÷ p, checked against a quadrature.
  - _Monotonicity, for T8._ Every envelope is non-increasing in |x|, |y| and |z| analytically
    (disc and bar: exponentials of sums of monotone terms, the bar's level part through
    max(|x| − 0.85 L, 0); halo: m², the core, the continuous break and the sphere; bulge: m is a
    norm). In floating point the discs, the bar and the halo are monotone step by step given
    monotone `libm`, which unit-in-the-last-place stepping across the core, break and cut
    confirms. The bulge's M (1 + t^c∥)^(1 ÷ c∥) is not: stepping raises it by up to 7 × 10⁻¹⁵ of
    itself. T8's nearest-corner bound therefore takes a relative margin (2⁻⁴⁰ covers this and
    `libm`'s own rounding). Density is envelope × arm factor and `densities` is
    `Component::density`, bit for bit, for four and two arms; `densities` does not allocate;
    order independence is tested.
  - _Speed (T7 acceptance)._ `Fields::densities` takes 538 ns at 26,000 ly, 415 ns in the bulge and
    585 ns at 35,000 ly, against 400 ns: a finding. The halo's six components cost about 130 ns
    (ln_1p and exp each), the eight disc exponentials 80, the bulge 40, the arm point 35, and the
    sharp arm's `bessel_i0e` 47 ns at the bench point's k = 13, a power series whose chain is one
    multiply per term. Its asymptotic branch (k of 15–40) costs 50–120 ns and could lose about 70
    by moving its division off the chain, which would move `MGE_BAR` and the potential goldens, but
    only beyond 28,000 ly for the fixture; it is left. 29 `libm` calls set a floor near 300 ns.
    Already taken: cos φ by double angles, the fade as 1 ÷ (1 + e^(−2u)), the bulge's bracket as a
    binomial series below t^c∥ = 1/256, powers as `exp` of `ln_1p`.
  - _Interfaces._ `shares::ShareMatrix` (`uniform`, `share`, `component_share`) is built here for
    `Fields::layer_density`, with the seven population columns only; T9 adds the reserved ones and
    keeps the `Galaxy` handle. Beyond the Provides:
    `Component::{envelope, arm, shape, sub_disc, count, count_with_unborn}`,
    `Fields::{component_id, sub_disc_heights}`, `SubDiscHeights` (with `weighted_dispersions`),
    `Shape`, `BuildFieldError`, `arms::{Arm, ArmPoint}`, the `ArmGeometry` methods `fade`, `phase`,
    `phase_polar`, `phase_rate`, `ridge_azimuth`, `point` and `point_polar`,
    `SharpArm::new(geometry, width, fraction)` (plan 07's lanes and P08.T1's `with_width`) with `k`
    and `profile`, and `GentleArm::new`. cos φ is clamped to [−1, 1].
- **R17. Deviations in T8, as built (P02.T8).** The acceptance filter `galaxy_bounds` matches test
  names and selects only T8.c's golden test, `galaxy_bounds_are_pinned`: T8 runs as
  `cargo test -p hyperion-sim --test galaxy_bounds`, `--lib galaxy::bounds` and
  `--lib galaxy::fields::arms`, the slow tests under `just test-slow`.
  - _Cell geometry (T8.a)._ `CellBox::new` rejects an edge that is not a power of two
    (`EdgeNotPowerOfTwo`, 0 included) and a box reaching outside the root cube (`OutsideRootCube`),
    besides a box across a plane (`StraddlesPlane`); the axis is 0–2 as in `BuildGenCellError`. A
    power-of-two edge keeps `in_plane_half_diagonal` (edge × `FRAC_1_SQRT_2`, which rounds up)
    at or above the exact edge ÷ √2, which T8.b's phase range needs: for 340 other edges under
    5,000 it falls one unit in the last place short. The root cube keeps every envelope far from
    underflow, where a relative margin gives no headroom. The low-corner getter is `min_corner()`,
    because `min()` resolved to the derived `Ord::min`. `r_cyl_range` takes each end as
    `Site::new(corner).r`, the densities' own R, so no point of a cell lies outside it, bit for bit
    (tested). `ScalarRange` lands in T8.a, not T8.b, because `r_cyl_range` returns it; its fields
    stay public as the Provides has them, `ScalarRange::new` debug-asserts lo ≤ hi, and an infinite
    end is allowed. Beyond the Provides: `CellBox::{min_corner, edge, contains}`,
    `ScalarRange::{new, contains}`, `BOUND_MARGIN`, `Component::envelope_bound`.
  - _Margin (T8.a)._ Every envelope bound is the nearest corner's envelope × (1 + 2⁻⁴⁰), about
    9.1 × 10⁻¹³ (`BOUND_MARGIN`), for every component alike, not only the bulge that needs it
    (R16): 130 times the bulge's 7 × 10⁻¹⁵ and under the test's 10⁻¹².
  - _Tests (T8.a)._ The plan's check runs as three `#[ignore = "slow: …"]` tests, one per pinned
    seed: 2,000 random cells of each of the edges 4, 8, 16, 32, 64, 128 and 4,096 ly on a 17³
    lattice with its corners, plus targeted cells in every octant (the centre, the bar's end, the
    bulge's switch between its terms and its series limit t^c∥ = 1/256, each halo component's
    core, break and cut), each probe set also stepping the nearest corner outwards by up to 32
    units in the last place. They take about 45 s on four cores. The fast suite runs 12 cells per
    edge on a 9³ lattice and the targeted cells on 3³. Zero violations; the envelope bounds are
    pure and order-independent (tested). `GENERATOR_VERSION` stays 4 until T8.c bumps it once for
    T8 with `galaxy_bounds.golden`: nothing reads a bound before then.
  - _Arm bounds (T8.b)._ `ArmGeometry::phase_range`, `SharpArm::sup`, `GentleArm::sup`,
    `Arm::{sup, across}` and `ArmAcross` live in `fields/arms.rs`, beside the factors whose
    arithmetic they repeat step by step; `bounds.rs` holds the trait, the margins and
    `Component::bound` (envelope bound × arm factor bound), which T8.c's `component_bound` wraps.
    The trait's `sup` takes one range, but an arm factor also depends on R, so `ArmAcross` fixes
    the band of radii (from `r_cyl_range`) and takes the phase. Every phase is the range (−∞, ∞).
    The gentle bound is 1 + f(R_max) a c for c ≥ 0 and 1 + f(R_min) a c for c < 0, the plan's two
    cases (its floor at 0 is never reached). For plan 07's lanes, at R + δ with δ = d ÷ cos p ≥ 0:
    `phase_range`'s half-width still bounds the phase there, only the centre moves, to
    `phase_polar(R_c + δ, θ_c)`, and `SharpArm::sup` takes the shifted radii rounded outward.
  - _Margins (T8.b)._ Beyond the envelopes' 2⁻⁴⁰, the bound is never below the density as
    computed, bit for bit, because each step of `sup` repeats the factor's step on inputs no
    smaller (the argument is in `bounds.rs`, "Floating point"): the greatest cos φ over a range is
    raised by 2⁻³⁶ (1.5 × 10⁻¹¹) absolute, covering the densities' double-angle cos φ and the
    phase's rounding (under 10⁻¹⁴ apart; 4 × 10⁻¹⁴ at 150 radians, 20 ly from the axis), which
    loosens a sharp bound by at most k × 2⁻³⁶ ≤ 5 × 10⁻⁸; I₀ₑ(k(R_max)) is taken × (1 − 2⁻⁴⁰),
    because `bessel_i0e` rises where its sum changes form, by up to 1.33 × 10⁻¹⁴ where the
    asymptotic series takes a 31st term at k ≈ 15.004 and 5 × 10⁻¹⁵ at the switch at 15; the
    radii are widened by 2⁻⁵⁰ relative, since a density's k reads x² + y² and a bound's reads R²
    from √; and each arm factor's bound is raised by 2⁻⁵⁰ absolute, for `libm`'s last-bit steps
    where the factor nears 0 (arm fraction near 1), which a relative margin cannot cover. Each
    has a unit test with its headroom. `COS_SLACK` and `ROUNDING_SLACK` are `pub(crate)`.
  - _Tests (T8.b)._ Arm bounds against a 17 × 17 scan of the cell's face with every ridge
    crossing, over 5,000 random cells of every edge (half in the plane at 0.5–3 bar
    half-lengths), for 24 arms at the ends of their ranges: two and four arms, 10° and 18°, bars
    of 10,000 and 18,000 ly, sharp at (250 ly, 0.9) and (500 ly, 1.0), gentle at a = 0.3; the
    slow test scans every arm in every cell on 33 × 33. Zero violations. Tightness, the
    fixture's young disc over 3,000 random 128 ly mid-plane cells between the bar's end and
    40,000 ly: mean bound ÷ mean density 1.750, under the plan's 3.5 and the brainstorm's 2.8
    (whose profile differed); 1.75, 1.81 and 1.56 on the hunt's sharpest-arm seeds below.
  - _Layer bounds (T8.c)._ `Fields::component_bound` wraps `Component::bound`; `component_bounds`
    gives the same bits (tested), reading the nearest corner and the ranges of radius and phase
    once per cell and the sub-discs' shared arm bound once, and zeroes the slots past the last
    component. `layer_bound` folds share × bound from 0 in component order, as `layer_density`
    folds the densities, so once every component's bound holds, Σ weights ≤ the layer's bound bit
    for bit. Its margins sit inside the bound; P03.T4.b's padding by (1 + 10⁻¹²) before
    `Mark::pick_weighted` sits outside it, is not needed for that inequality, and its debug
    assertion fires only for a violation of more than 10⁻¹² beyond a bound already carrying the
    margins (`bounds.rs`, "A layer's bound"). `Fields::new`'s check that every arm shares the
    galaxy's geometry is now an `assert!`, since `component_bounds` reads one range of phase for
    all of them; `Shape::envelope_at` is `pub(crate)`. Below 2.2 × 10⁻³⁰⁸ the relative margin
    shrinks, and below about 3 × 10⁻³¹² it rounds away: inside the root cube only the bar's
    Gaussian end and the nuclear disc far from its centre get there, both monotone bit for bit;
    the bulge stays normal across the cube (tested).
  - _The hunt (T8.c)._ Five seeds from a scan of 1,500 (`0x0208_4a47_0000_0000 | n`), each
    asserted to keep its property: the sharpest two arms and the sharpest four (pitch over 17.5°,
    width under 260 ly), the longest bar (two arms), the shortest (four arms), and four arms at
    10° on the longest bar. Three builder galaxies at the edges of the ranges: two sharp arms on
    a 10,000 ly bar, four tight sharp arms, and the densest centre (smallest bulge, nuclear disc,
    bar and halo cores, nearest and steepest halo break). Per stellar layer: every ridge from 0.8
    to 2 bar half-lengths in steps of 16 ly of arc (8 ly for layer A, so that no neighbour is
    skipped), the cell holding each step and its eight neighbours in the plane on the side z ≥ 0
    (z < 0 gives the same bits; the cells above, which the plan counts as neighbours, share the
    ranges of radius and phase with a smaller envelope and are not taken); T8.a's targeted
    cells; 10⁴ random cells for the seeds and 2,000 for the builder galaxies, half in the central
    1,000 ly.
  - _The maximiser (T8.c)._ The plan's 33³ lattice in every cell would take about 1.7 × 10¹⁰
    evaluations per galaxy, hours. Instead, an m³ lattice (m = 5 for the young disc, 3
    otherwise) is refined by compass search along the axes and the plane's diagonals, both ways,
    from the lattice's best point and from the nearest corner, down to 2⁻²⁰ of the edge: the
    young disc and the youngest sub-disc (whose arm the others share) in every cell, with the
    layer read at the lattice and those maxima in ridge cells and refined in the random ones, and
    every component in the targeted cells. 1.86 million cells, zero violations, 84 s on four
    cores for all of T8's slow tests. The worst density ÷ bound is 1 − 9 × 10⁻¹³ (the margin at a
    corner) for every quantity, and exactly 1 where the bar's end is subnormal at the cube's
    edge. The fast suite runs one ridge cell in 40, the targeted cells and 6 random cells per layer
    on the fixture. `Search`, `ridge_cells` and `hunt_cell` are private to the test file; plan
    08's extension of the hunt (P08.T11) would move them to `hyperion-testkit`.
  - _Golden and version (T8.c)._ `GENERATOR_VERSION` is 5, the one bump for T8; the other goldens
    changed their header only. `galaxy_bounds.golden` pins 50 cells, the cell of each stellar
    layer holding ten places: the centre on both sides of each plane, the bulge, the bar and its
    end, the solar circle on and off an arm, above the disc, 61,000 ly across the bar (a subnormal
    bound) and the halo's cut. For the fixture every component's bound, the young disc's
    envelope bound, the ranges of radius and phase and the layer's bound; for the three pinned
    seeds the young disc's, the youngest sub-disc's and the layer's bounds. Stable in debug and
    release; `just test-wasm` not run (no wasmtime).
  - _Speed (T8.c)._ The plan sets no target; plan 03's 1–2 µs per sparse cell leaves about 1 µs
    beside `Fields::densities`. `Fields::layer_bound` takes 694 ns for a layer-A cell at the
    solar circle, 634 ns for layer E and 543 ns in the bulge (`component_bounds` 20–30 ns less),
    against 571 ns for `densities` on the same loaded machine (i7-8665U). A sparse cell's bound
    and one candidate's densities come to 1.1–1.3 µs before the Poisson draw: inside plan 03's 2
    µs, not its 1.
- **R18. Updated for the 2026-09-21 density rulings (P02.T7).** The owner adopted all six rulings
  of the brainstorm's Decisions entry "2026-09-21: local density rulings"; D4–D6, D9 and the texts
  of T2, T4, T5, T7, T8, T10 and T11 above are rewritten for them. `GENERATOR_VERSION` is 6, one
  bump for the whole revision: the parameters' goldens moved with the default mass function and
  the halo's ranges, the potential's with the populations' masses under the default, the fields'
  and the bounds' everywhere; the rest changed their header only. As built:
  - _Mass function (D4, D5, T2, T4)._ `MassFunctionKind::default()` is `Chabrier` (scale 0.68) and
    the fixture takes it; the tests read the default, with Kroupa's as a second case (the
    parameters' golden pins one seed under it). A 10 Gyr declining history holds 0.572–0.578 M☉
    per system (Kroupa's 0.498–0.503); the fixture 0.566 overall, its old populations 0.547–0.574
    and its young disc 0.790, 1.375 times its old thin disc; over 10⁴ seeds the old populations
    hold 0.544–0.576 and N is 0.528–1.775 × 10¹¹. All stars below 0.5 M☉: 66.9% under Chabrier's
    function as published, 70.9% scaled, 76.4% under Kroupa's; primaries: 66.3%, 69.7%, 76.1%.
    The fixture has 1.061 × 10¹¹ systems (1.217 under Kroupa's) and a black hole of 4.297 × 10⁶
    M☉ at σ = 119.28 km/s, Sgr A*'s still. The band-share test gains the brainstorm's default
    column.
  - _Cored profiles (D9, T7.a, T7.b)._ `fields/vertical.rs` holds `VerticalProfile` (the Jeans
    solution tabulated at 705 knots, every light-year to 128 ly, then 64 segments per octave to
    65,536 ly, its exponent linear between them; `exponent`, `value`, `integral_to`,
    `effective_height`, `dispersion`, `dispersion_at`, `gradient`, `gradient_reach`), and the
    crate's `VerticalForce` (moved from `sub_discs.rs`) and `JeansIntegral`. `DoubleExponential` is
    `ExponentialDisc { n0, length, profile, arm }`, `height()` its effective height and `profile()`
    its profile. `SubDiscHeights` keeps its name and its getter `Fields::sub_disc_heights`:
    `dispersions` are the law's, `scaled_dispersions` the profiles', `scale` the dispersion scale,
    `heights` and `unscaled` the effective heights at the scale and at 1; `weighted_dispersions`
    and the public `SubDiscHeights::solve` are gone. Every disc takes Sharma et al.'s rise of 0.20
    per kpc, which they find for the high-α stars too ("no special provision is needed to
    accommodate the thick disc stars", in their summary): an isothermal thick disc, as Bovy et al.
    (2012, ApJ 755, 115) measure mono-abundance populations, left the fixture's far-field fit with
    no thick disc (its h₂ at the fit's floor of 500 pc). The rise stops at 2.4 kpc, the reach of the heights
    Sharma et al.'s figures show (`DISPERSION_GRADIENT_REACH_KPC`): carried on, it gave every disc
    a tail falling as z⁻², and the fixture's thick disc a tenth of the halo's density 10 kpc above
    the Sun (now 1.7%). The thin, young and thick discs are solved at three thin-disc scale lengths,
    the nuclear disc at two of its own, the mass-weighted mean radius of an exponential disc. The
    profiles' only slope at the plane is −2γ, an e-fold in 2.5 kpc. For the fixture: effective
    heights 371–1,373 ly (364–1,341 unscaled; the youngest and oldest within a quarter of the
    brainstorm's heights at their mean ages, 340 and 1,560 ly, which T7.b's brackets now read),
    dispersion scale 1.019, mid-plane dispersions 6.5–20.1 km/s, and 2.74 km/s for the young disc,
    37.0 for the thick and 31.5 for the nuclear. Over 10³ seeds the scale has a median of 0.987,
    99.8% in 0.6–1.6 (0.64–1.72), and its logarithm follows half that of the column at R_ref times
    the drawn height (slope 0.535, correlation 0.974); the young disc's dispersion runs 1.8–5.1
    km/s, the thick's 22–61, the nuclear's 22–52.
  - _Against measurements (T7.b)._ At R₀ = 8.178 kpc, far from the plane, a double exponential
    fitted over 250–3,000 pc to the fixture's young, old and thick discs (least squares in ln n at
    56 heights, a grid then a compass search, h₂ > 1.3 h₁; not the nuclear disc or the halo) gives 258.6 pc, 985 pc and a thick share of
    2.3%, against Bland-Hawthorn and Gerhard's 300 ± 50, 900 ± 180 and 4 ± 2% (the thin disc 9 pc
    and the share 0.3 points from their edges; 286 pc at an effective height of 1,100 ly). In the
    plane the fixture has 0.0609 M☉ pc⁻³ of stars and remnants against McKee et al.'s 0.0415 ±
    0.004 (their 0.043 counts brown dwarfs, which no stellar layer holds; T11's row takes 0.0415),
    and 0.00303 systems per ly³, 0.00297 at the Sun's height, against the census's 0.00193: 1.47 and
    1.54 times, the fixture's Σ★ of 39 against the 25–27 M☉ pc⁻² these two need. T11 tunes Σ★; at
    its 28 the density at the Sun's height would be about 0.0021, the bracket's top. The local mean
    mass per system, 0.579 M☉, is asserted against the census's 0.55–0.59, and the density at the
    Sun's height is asserted below the plane's; the mass and number densities are printed, and
    T11's rows assert them. The census's figures (0.00193 per ly³, 66.5% of primaries below 0.5
    M☉ within 20 pc and 67.8% within 10 pc, 0.55–0.59 M☉ per system) are tallies from Kirkpatrick
    et al.'s (2024) Table 4, not printed there; the brainstorm's "0.00184 within 10 pc (Reylé et
    al. 2021)" is the same table's tally too.
  - _The potential (D6)._ It keeps every disc exponential in height at the drawn effective height;
    the fixture's cored thin disc differs from it by at most 5% of its 2πGΣ within a height, 4.6%
    of the whole K_z there, near 1.1 effective heights.
  - _Bounds (T8)._ No new margin: the profile's exponent is non-decreasing bit for bit, because
    its segments are found exactly (integer part, octave by `ilog2`, powers of two) and its knots
    are made continuous as rounded (`fields/vertical.rs`, "Floating point"). A unit test steps a
    unit in the last place at a time across every knot, and the table's end, of twelve profiles
    (two radii, isothermal and rising, 3–90 km/s); `galaxy_bounds.rs` adds targeted disc cells
    where the segments change width, where the rise stops, at 32,768 and 49,152 ly and at the
    cube's top. The young disc is subnormal far above the plane (from 12,000–24,000 ly) and the
    youngest sub-disc near the cube's top in some galaxies, where the relative margin rounds to at
    most one unit in the last place, so `assert_envelopes_bounded` allows a subnormal corner its
    margin plus one rounding; the nuclear disc no longer gets there. The builder galaxy with the
    densest centre takes the new halo ranges' ends (slopes 2.8, the break at 52,000 ly steepening
    by 2.5). T8's slow tests pass: 1.88 million cells in the hunt, zero violations, the worst
    density ÷ bound 1 − 9 × 10⁻¹³ for normal envelopes and 1 to twelve places where the young disc
    or the bar is subnormal; 213 s on the loaded machine.
  - _Halo (T5.a, T5.c, T7.d)._ Inner slopes 2.2–2.8, the dominant break 52,000–91,000 ly and its
    steepening 1.5–2.5 (tags and word counts unchanged); the fixture 2.5, 58,700 ly, 2.0. Its
    spherically averaged slope from 20,000 to 60,000 ly is −2.973, 0.03 inside the bracket, the
    in-situ component's cut at 50,000 ly and the steep debris taking it past the inner −2.5; the
    halo near the Sun is 2.6 × 10⁻⁶ per ly³. For the owner: Xue et al.'s inner slope is 2.1 ± 0.3
    and Medina et al.'s 1.88 in a spherical fit (2.05 in their Table 5), below 2.2; a range of
    2.0–2.8 would cover every measurement.
  - _Metallicity (T7.e)._ The thin discs' mean is still clamped to [−1.0, +0.5] dex, R16's
    reading of "clamped", the span of thin-disc stars; the brainstorm gives no clamp. Flat to 8 Gyr
    and 0.1 dex per Gyr poorer beyond, read from Bergemann et
    al.'s (2014) Fig. 6, which gives no number; sigma 0.20 from Casagrande et al.'s (2011) Table 1
    (σ 0.22, half the FWHM 0.19). The flat part is solar at three scale lengths, as the youngest
    local stars are (Nieva and Przybilla 2012: Fe 7.52 ± 0.03 against the Sun's 7.50), so the
    local mean over every age is −0.03 to −0.04 against the Geneva–Copenhagen survey's −0.06. For
    the owner: Bergemann et al. assign their old, metal-poor stars to the thick disc, and they say
    their result "does not support" Casagrande et al.'s flat relation to 12 Gyr, so the decline
    may belong to the thick disc rather than the thin.
  - _Scaled brackets (T5.b, T7.c, T7.e)._ Kroupa's figures times the bulge's and the nuclear
    disc's mean mass per system under Kroupa's over the default's, 0.866: the fixture's bulge
    centre is 0.268 per ly³ (0.17–0.30), over 10³ seeds median 0.290 and 93.5% in 0.12–0.55
    (0.08–1.02); its nuclear disc's 18.89 (12–19, 0.6% under the top), over 10³ seeds median 15.1
    (6.1–43.7); the total centre 19.3, over 10³ seeds median 15.6 and 1.3% above T11's 30 (maximum
    44.3). The in-plane density at 26,000 ly lies in 0.0008–0.008 for 99.7% of 10³ seeds
    (0.00106–0.00873). N's bracket is 0.5–1.8 × 10¹¹ under the default and stays 0.5–2.1 under
    Kroupa's. The parameters' tests hold every old population's mean mass per system to 0.54–0.58
    M☉ under the default (the young disc 0.77–0.83; 0.45–0.52 and 0.65–0.75 under Kroupa's): over
    10⁴ seeds the old populations give 0.544–0.576, up to 0.006 under the brainstorm's "about
    0.55". The 10³-seed sweep asserts the bulge's median centre in 0.22–0.35 (R16's 0.25–0.40
    scaled) and the dispersion scale's power of the column × height in 0.4–0.65 (0.535), and that
    the young, thick and nuclear discs meet their drawn heights to 10⁻⁹. The fast suite holds their
    dispersions over 8 seeds to 1–8, 15–70 and 10–80 km/s.
  - _Speed (T7 acceptance)._ `Fields::new` takes about 80 ms, not 45: a second table of K_z, at
    the nuclear disc's radius, and the bisections of the dispersions. `Fields::densities` takes
    about 590 ns at the solar circle, against 538–585 ns before and the 400 ns target, a finding:
    each disc reads its table at a height located once per point. Both measured on a machine
    loaded by other work, where Criterion's runs are not repeatable.
  - _Interfaces._ `ExponentialDisc::new` is `pub(crate)`, since only the crate builds a
    `VerticalProfile`; outside it a disc comes from `Fields`. `VerticalProfile`'s public methods
    take a signed height and read |z|; `integral_to` returns `LightYears`, `gradient_per_kpc` is γ
    and `gradient_reach` z_γ. `metallicity::THIN_DISC_REFERENCE_AGE` is gone and
    `THIN_DISC_FLAT_AGE` is new. `Gl16Panel::value`, added in T7's first build, stays public in
    `galaxy::quad`. The heating law's constants and `heating_law` are `pub` inside the private
    `sub_discs` module, so only the crate reaches them.
  - _For later plans._ Plan 08's Design note 3 solves the same Jeans equation on
    `VerticalProfile`, whose `dispersion_at` is σ₀ (1 + γ min(|z|, 2.4 kpc)) by construction at
    the reference radius, the law times the dispersion scale for the sub-discs; the young, thick
    and nuclear discs carry their own σ₀. P08.T2.a's comparison reaches 8 effective heights, beyond
    2.4 kpc for the oldest sub-discs and the thick disc, where the uncapped law runs about 12%
    above the profiles', so it must compare against the capped law. T10's edge-on columns read
    `VerticalProfile::integral_to`, which is exact for the table but has no bit-for-bit monotonicity
    argument, unlike `exponent`: a difference of two columns can come out a unit in the last place
    below 0 near a knot, so T10 clamps it at 0 or proves otherwise. Plan 09's Design note 21 bounds density ÷ g(z) at the
    height nearest the plane, which held for exponential discs; a cored disc's ratio to an
    exponential proposal peaks above the plane, so plan 09 must bound it anew when revalidated.
    T11 tunes the fixture's Σ★ (its table, above).
