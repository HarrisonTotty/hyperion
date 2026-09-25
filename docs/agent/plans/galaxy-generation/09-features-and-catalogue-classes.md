# Plan 09: Large features and catalogue classes

- **Milestone:** M3.
- **Depends on:** 06 (stars), 07 (gas and dust), 08 (velocities, kicks, displaced objects), 15
  (offline tables), and through them 01–05.
- **Brainstorm sections covered:** "Large features" with all four of its subsections except "Streams
  and accreted structure" (plan 10); the Kepler regime of "Orbits and time"; of "Events in time",
  the catalogue classes as carved-out hosts, "The supernova test with a clock" and "The centre";
  "The range query", fourth and fifth details (the centre's padding and full scan, the merge of
  non-grid sources); the feature and member layouts of "Identifiers" and "Dense features"; the
  matching lines of "Testing" (complementarity, time, Milky Way, budgets) and of "Open questions"
  (cluster fits, Type Ia tables); step 7 of "Suggested order of attack".

## Goal

When this plan is done the galaxy holds its large features. A feature catalogue on a 4,096 ly grid
places globular clusters, open clusters, OB associations, star-forming regions and molecular clouds
by the same thinning as the systems, and the field gives up to them the share φ, so that nothing is
counted twice. Every feature with members carries a nested grid and a table of member classes, so a
cluster is what it is today: segregated by mass, short of the dwarfs, neutron stars and black holes
it has lost, core-collapsed where it should be, with tails, multiple populations and velocities.
Globular clusters are drawn as they are now and their history is derived backwards. The galactic
centre is a feature of its own: a cusp defined as the integral of a distribution function, twelve
grid levels, members on Kepler orbits with two secular precessions, and a central black hole with
flares, tidal disruptions and a luminosity. Supernova remnants have one route: a shell belongs to
the star that died, its window is a closed form in the gas at the site, and one shared test with a
clock decides on every side whether the catalogue or the cell owns a system. Core collapse and Type
Ia hosts are the first two catalogue classes under the `111` prefix, and the luminous blue variables
the third. All of it is merged into the range query under the census rule, crosses the protocol, and
appears on the galaxy map and the local chart.

## Scope and non-goals

In scope:

- The feature catalogue, every kind in the brainstorm's table except streams and dwarf cores, and
  the φ rule with its budget test.
- "What is inside a cluster today", "Supernova remnants: one route, not two" and "Dense features" in
  full, including the ID layouts under the `0` prefix, the centre's layout under `10`, and the `111`
  layout.
- The Kepler regime within the black hole's sphere of influence, for the centre's members and for
  grid systems, and the centre's rule in the range query.
- The catalogue-class machinery, with the two supernova classes and the luminous blue variables as
  its users, light curves, the black hole's own events and luminosity.
- The merge of all of these into the range query, protocol messages, server caches and display
  additions.

Not in scope:

- Streams, dwarf cores and the global list itself (plan 10). This plan only registers the galactic
  centre in the form plan 10's list will hold, and leaves the `10` sub-kinds for streams and dwarf
  cores reserved.
- The four binary catalogue classes (stellar mergers, neutron-star mergers, X-ray binaries,
  accreting white dwarfs). Their class values are reserved here and plan 11 fills them. The luminous
  blue variables are single stars and are built here (P09.T43), as plans 06 and 15 expect. Binary
  orbits of members are plan 11; this plan only fixes which members are binaries.
- A shell's appearance reading nearby clouds at query time, which the brainstorm allows and does not
  require. A shell's state here reads the smooth field and its own feature's bubble only.
- Retarded-time observation and alerts (plan 12). Everything here is a function of an evaluation
  time, which is what plan 12 needs.
- Extinction by wavelength and the consoles' use of gas (plan 07 and later). This plan supplies the
  features' holes and local dust to plan 07's line integral and nothing more.
- Pinned content (`110`).

## Provides

All Rust paths are under `hyperion_sim::galaxy` unless they start with another module.

### Identifiers

Plan 01 already builds the bit layouts in `hyperion_sim::id`: `FeatureCell`, `FeatureRef`,
`MemberSlot`, `FeatureMemberId`, `CentreMemberId`, `CatalogueSystemId` and the `SystemIdKind`
dispatch. This plan adds what gives them meaning:

- `features::FeatureId`, a thin wrapper of `id::FeatureRef` with `designation()` and
  `object_word()`.
- `catalogue_classes::ClassId` and its registry, which plan 01 leaves to this plan:
  `CORE_COLLAPSE = 0`, `TYPE_IA = 1`, `LUMINOUS_BLUE_VARIABLE = 4`, and reserved for plan 11
  `STELLAR_MERGER = 2`, `NEUTRON_STAR_MERGER = 3`, `XRAY_BINARY = 5`, `ACCRETING_WHITE_DWARF = 6`
  (its fast hosts) and `ACCRETING_WHITE_DWARF_SLOW = 8`; `TIDAL_DISRUPTION_VICTIM = 7`, which exists
  only on the centre's feature-level list and never under the `111` prefix. This registry
  (`galaxy/catalogue_classes/mod.rs`) is the one place a value is allocated: values 0–8 are taken, 9
  upward are free, and a test asserts that no two names share a value. `ClassId::cell_log2_ly()`
  gives the class's catalogue cell size, which `resolve` checks with plan 01's
  `CatalogueSystemId::is_aligned_to`.
- `catalogue_classes::CatalogueCellKey` (class, cell at the class's cell size).
- Event tags in plan 01's `event_tags!` registry, inside the block 0x0200–0x02FF that plan 06 sets
  aside for this plan: `0x0200 CENTRE_FLARE`, `0x0201 TIDAL_DISRUPTION`, `0x0202 SUPERNOVA` (the
  one-shot explosion of a catalogue entry, bin 0, number 0). Each has its `DomainTag` of scope
  `Event` in `rng/tags.rs`.
- Domain tags under the prefixes `feature.`, `member.`, `centre.`, `snr.` and `class.`, each an
  entry of plan 01's single registry `rng/tags.rs` under a "Plan 09" heading, added by the task that
  first draws on it. Two are read on both sides of the carve-out and are named here: `snr.energy`
  (scope `System`) and `member.velocity` (scope `System`).

### Catalogue, shares and kinds

- `features::shares::FeatureShares`, held by `Galaxy` and reached by plan 02's fields through new
  variants of its `FeatureShare` enum: `phi(population, band) -> f64`,
  `phi_young(age: Years) -> f64`, `field_factor(population, band) -> f64`,
  `set_halo_discrete(phi: f64)` for plan 10. `phi` and `field_factor` take plan 02's `MassBand` and
  are defined for every value it will ever have: for a band below the five stellar ones (plan 13
  adds the brown-dwarf and rogue-planet bands) they return band A's value, through one private
  `stellar_band_or_a` mapping, so plan 13 changes nothing here. Plan 11's P11.T7 adds one more
  setter to this type, `set_class_shares(&ClassShareTable)`, folded into `field_factor` beside the
  Type Ia ancient share; it is plan 11's code in this plan's file `galaxy/features/shares.rs`.
  `features::shares::NurseryRates`.
- `features::catalogue::{FeatureProcess, FeatureKind, FeatureRecord, FeatureCatalogue}` with
  `FeatureCatalogue::cell(&Galaxy, FeatureCell) -> FeatureCellContents`,
  `resolve(&Galaxy, FeatureId) -> Option<FeatureRecord>`,
  `near(&Galaxy, centre, radius, &dyn FeatureCellCache) -> impl Iterator<Item = FeatureRecord>`,
  `walk_process(&Galaxy, FeatureProcess) -> impl Iterator<Item = FeatureRecord>` (plan 10 walks the
  globulars with it), `MAX_FEATURE_REACH: LightYears = 4,096`.
- `features::kinds::globular::GlobularMarks`, `features::kinds::open_cluster::OpenClusterMarks`,
  `features::kinds::nursery::{NurseryMarks, NurseryStage, Superbubble}`,
  `features::kinds::cloud::CloudMarks`.
- `features::emission::EmissionClass`, with variants `HiiRegion`, `ReflectionNebula`, `DarkCloud`,
  `RemnantShell` and `PulsarWindNebula`.
- `features::gas_overlay::FeatureGas`, an implementation of plan 07's `GasModifierSource` that turns
  superbubbles into `GasModifier::Hole` and clouds and star-forming regions into
  `GasModifier::Cloud`.

### Interiors, nested grids and members

- `features::cluster::ClusterModel` (`from_record`), with `escape_speed_central()`,
  `escape_speed_birth()`, `relaxation_time()`, `sigma(r)`, `tidal_radius()`, `is_core_collapsed()`,
  `black_hole_count()`, `encounter_rate()`.
- `features::interior::{MemberClass, ClassKind, ClassProfile, MemberClassTable}` with
  `MemberClassTable::bound(band, corner_radius)`, `MemberClassTable::expected(band)` and
  `MemberClassTable::pick(band, position, Mark) -> Option<MemberClass>`.
  `MemberClassTable::density(position) -> PerCubicLightYear` and
  `MemberClassTable::mean_member_mass(position) -> SolarMasses` serve plan 14, with
  `ClusterModel::sigma(r)`. The name keeps it apart from plan 08's `displaced::ClassTable`.
- `features::interior::MemberAbundances` (enrichment, light-element offsets, iron spread), beside
  plan 06's `Composition`, which carries [Fe/H] and the helium excess to the stellar stage.
- `features::nested::{NestedGrid, NestedCell, NestedLevel}`:
  `NestedGrid::new(width, cells_per_axis, levels)`, `cells_touching(sphere)`,
  `owner_of(local_position)`.
- `features::members::{MemberRecord, resolve_member, FeatureLevelList, FeatureMemberSource}`.
  `FeatureMemberSource` implements plan 03's `SystemSource`. A `MemberRecord` holds a plan 03
  `SystemRecord` (design note 20), the member's class, its `Composition` and its `MemberAbundances`.
  `features::members::sphere_of_influence(galaxy, &MemberRecord) -> Metres` is the smaller-radius
  rule that plan 14 reads.
- Variants of plan 03's `placement::SystemOrigin` (on every `SystemRecord` since M1, where its only
  variant is `Grid(ComponentId)`): this plan adds `FeatureMember(FeatureId)`, `CentreMember` and
  `CatalogueSystem(ClassId)`, and reserves the name `GlobalListMember` for plan 10, which adds it.
  None carries a component, and `SystemRecord::component()` is `None` for them.

### The galactic centre and motion

- `features::centre::{CentreModel, CentreProfile, DistributionFunction, CentreClass}`,
  `features::centre::{BlackHole, CentreMemberSource}`; `Galaxy::centre() -> &CentreModel`;
  `CentreModel::enclosed_mass(r)`, `influence_radius()`, `loss_cone_radius()`, `as_global_entry()`
  for plan 10.
- `motion::{Regime, KeplerOrbit, regime_of, position_velocity_at}`, plugged into the drift hook of
  plans 03 and 08. `KeplerOrbit::from_state`, `propagate(dt)`, `pericentre()`,
  `schwarzschild_rate()`, `mass_precession_rate()`.
- `motion::plunge_pad(r0, dt)` and `motion::full_scan_radius(dt)`.
- `features::centre::events::{flares, tidal_disruptions, luminosity_at}`.

### Supernova remnants and catalogue classes

- `snr::{SiteGas, ShellWindow, shell_window, WindowCaps, ShellEnvironment, SHELL_WINDOW_CAP}`,
  `snr::{ShellPhase, ShellState, shell_state_at, PulsarWindNebula, remnant_offset}`.
  `SHELL_WINDOW_CAP: Years` is the constant plan 08 introduced at 4 Myr; it moves here.
- `catalogue_classes::supernova::{DeathMarks, SupernovaInterval, claims}`,
  `catalogue_classes::supernova::{SupernovaEntry, SupernovaState, LightCurve}`, with plan 06's
  `stellar::remnant::SupernovaType` extended by this plan's
  `SupernovaKind { CoreCollapse( SupernovaType), TypeIa(IaChannel) }`;
  `claims(&Galaxy, &ExplosionSite, &DeathMarks, ShellEnvironment) -> bool` is the one shared test,
  over plan 08's `displaced::ExplosionSite`, and
  `DeathMarks::of(&Galaxy, &SystemStars, &ExplosionSite) -> DeathMarks` is the one constructor that
  the cells, the displaced bins and the catalogue all use.
- `catalogue_classes::type_ia::{DelayTimeDistribution, IaChannel, IaProgenitor, IaLeftover}` and
  `catalogue_classes::type_ia::ancient_share(population) -> f64`.
- `catalogue_classes::lbv::LbvProcess`, the luminous blue variables' class, over plan 15's
  `tables::lbv::LBV_SAMPLER`.
- `catalogue_classes::{ClassProcess, CatalogueClassSource, CatalogueClassCell}`. `ClassProcess` is
  the trait plan 11 implements for its classes. For plan 12:
  `CatalogueClassSource::cells_in_sphere(class, sphere)`, `expected_hosts_in_sphere(class, sphere)`
  and entries as `SystemRecord`s that carry their death time.

### Protocol, server, client

- Request kinds `FeaturesInRange`, `GalaxyFeatures`, `FeatureDetail` and `BlackHoleState` (on the
  wire `features_in_range`, `galaxy_features`, `feature_detail` and `black_hole_state`, the strings
  plan 04 reserves for this plan), each a new variant of plan 04's `RequestBody` and `ResponseBody`
  with a `…Request` struct and an entry in `REQUEST_KINDS`, with payload types `FeatureIdHex`,
  `FeatureSummary`, `FeatureKindWire`, `EmissionClassWire`, `ShellSummary`, `SupernovaStateWire`,
  `SystemOriginWire`, `ClassCountWire`.
- Server caches `FeatureCellCache`, `ClusterModelCache`, `NestedCellCache`, `CentreOrbitCache`,
  `CatalogueClassCellCache`, all byte-bounded.
- Client: `FeatureMark` and `ShellMark`, built on plan 05's `PointMark` and `SphereMark` in the
  general spatial view, the feature overlay of the galaxy map, `FeatureReadout`.

### Test helpers

- `features::testing::{milky_way_globulars, named_cluster}` (parameter sets for 47 Tucanae, M4,
  Palomar 5, ω Centauri, M15), `snr::testing::window_table`,
  `catalogue_classes::testing::assert_complementary`. `milky_way_globulars` is a reduced copy of the
  Baumgardt–Hilker tables (mass, r_h, r_c, position, orbit) committed as Rust source in the test
  module with its provenance in the header, because the sim may not depend on `hyperion-fit`, which
  holds plan 15's copy.

## Consumes

Names are those of the owning plans' "Provides" as they stood when this plan was written; where they
change, the owning plan wins and only call sites here change.

- **Plan 01:** `math`, `rng::{Seed, Stream, DomainTag, ObjectKey, tags}` with
  `Stream::open(Seed, DomainTag, ObjectKey)`, the `domain_tags!` registry in `rng/tags.rs` and the
  samplers, integer-threshold decisions (`Mark`, `Threshold`, `Mark::pick_weighted`), two-step event
  keys (`EventKey`), `units`,
  `time::{UniverseTime, Span, CLOCK_WINDOW_H, LIGHT_CROSSING_L, SourceHorizon}`, `coords`,
  `id::{SystemId, SystemIdKind, FeatureCell, FeatureRef, MemberSlot, FeatureMemberId}`,
  `id::{CentreMemberId, CatalogueSystemId, EventId, EventTag, Designation}` and the `event_tags!`
  registry, `CatalogueSystemId::is_aligned_to`, `GENERATOR_VERSION`, the golden harness and the
  statistical helpers of `hyperion-testkit`, slow-test marking, `just bench`, and the second CI
  architecture of P01.T12, on which this plan's goldens also run. Plan 01's decode already rejects
  inner nested cells, centre levels 12–15 and a feature-level slot with level or cell bits set, so
  this plan never generates such an ID and tests that it does not.
- **Plan 02:** `Galaxy`, `params::GalaxyParams` (dark halo, `AccretionHistory` with the last major
  merger, the progenitors and the globular count, the black hole's mass, `NuclearClusterParams`),
  `imf::{MassFunction, BandShares}`, the mean mass per system (`Galaxy::mean_system_mass`) and the
  mass formed per system (`Galaxy::mean_formed_mass`), `potential::PotentialTables` (v_c, Ω, κ, Φ,
  escape speed, tidal radius), the nuclear cluster's `BrokenPowerLaw`, `fields` (population
  densities, sub-discs, age distributions, metallicity, formation rates) with the `FeatureShare`
  hook held at φ = 0, `bounds`, `ShareMatrix`, `map`.
- **Plan 03:** `placement::{SystemRecord, resolve, ResolveSystemError}` with
  `SystemRecord::from_parts` for non-grid sources, `placement::SystemOrigin` (`#[non_exhaustive]`,
  extended here) and `KindNotGenerated`, which this plan's dispatch replaces for the `0`,
  `10`-centre and `111` kinds, candidate streams, the private carve-out hook `catalogue_claims`,
  `query::{RangeQuery, RangeResult, SystemHit, Census}`, `QuerySphere`, `LayerCounts`, the merge
  hook `SystemSource` (`expected_in_sphere`, `systems_in_sphere`, `suppresses`; it takes `&self`, so
  a source holds a reference to the caller's caches and the caches use interior mutability), the
  drift hook `epoch_velocity` and `position_at`, the padding rule (`PAD_SPEED`, `pad_for`).
- **Plan 04:** request IDs, the universe registry, the CPU pool, byte-bounded LRU caches, the
  TypeScript request layer. **Plan 05:** the general spatial view, the galaxy map, the `GALAXY`
  display's selectors and readout, the UX guide.
- **Plan 06:** stellar evaluation as a continuous function of age plus time;
  `lifetime(m0, &Composition, &StarDraws)`; `turn_off_mass(age, &Composition)`;
  `StarDraws::for_attempt(seed, star, attempt)` for conditional redraws;
  `Composition::from_fe_h(fe_h, helium_excess)`; the remnant outcome with `SupernovaType` and
  `ProgenitorAtDeath`; `stellar::remnant::KickLaw` and `StandardKickLaw`; neutron-star spin-down;
  the event constructions `PoissonBins` and `MonotonePhase`.
- **Plan 07:** `GasField::state(position, SmoothingScale, &mut NoiseCache) -> GasState` (density,
  pressure, temperature, sound speed), the corona's pressure floor, `GasField::neutral_bound`,
  `GasModifier`, `GasModifierSource` and the `NoModifiers` source this plan replaces.
- **Plan 08:** velocity laws of each population and halo component behind `epoch_velocity`
  (`KinematicTables::ellipsoid`); `kick_bins::speed_bin_shares` (the kick distribution by remnant
  kind and mode); `displaced::{explosion_site, ExplosionSite}` and the stub
  `recent_death_claims(galaxy, &site, &record) -> bool`, whose body this plan supplies, with
  `SHELL_WINDOW_CAP` (4 Myr there), which this plan takes over; `displaced::ClassTable` with the
  split between budget-fed `class_weight` and field-fed `stay_share`; the zero-weight hypervelocity
  class (`DisplacedKind::HypervelocitySurvivor`); `runaway::RunawayModel`;
  `SystemRecord::{placement_class, formation_site, kick_constraint, mark_attempt}`;
  `pad_speed(Layer)` with `UNBOUND_PAD_SPEED`.
- **Plan 15:** `tables::cluster_dynamics` (`BH_LOSS_BETA`, `BH_LOSS_PSI_SLOPE`,
  `BH_RELAXATION_PREFACTOR`, `EQUIPARTITION_EXPONENT`, `PULSARS_AT_47_TUC_GAMMA`,
  `PULSAR_GAMMA_EXPONENT`, `PULSAR_CORE_COLLAPSE_CAP`), `tables::type_ia_delay` (`DELAY_EDGES`,
  `YIELD_PER_SOLAR_MASS`, `CHANNEL_SHARE`, the two mass CDFs, `LAYER_SHARE`,
  `ANCIENT_LOSS_PER_SOLAR_MASS`; the explosion mark is plan 11's `tables::binary::IA_YIELD`),
  `tables::lbv::LBV_SAMPLER`, and plan 06's `tables::helium`, which this plan reaches only through
  `Composition`'s helium excess. Plan 15 moves this plan's scratch constants into its tables
  unchanged first (P15.T8, T9.a, T10.a), so no task here blocks on a fit; if this plan runs before
  those tasks, it commits the constants under the same names in the same modules, marked
  provisional.

## Design notes

Each note is a decision the brainstorm leaves open. None contradicts it.

1. **One young sequence.** Star-forming regions, OB associations and young bound clusters are one
   Poisson process of nurseries on the young disc, born at the young disc's formation rate on a mass
   function falling as M⁻² over 10²–10⁵ M☉. A nursery is a star-forming region while embedded (the
   first 3–5 Myr), then by an independent mark a bound open cluster (the bound fraction, 10–15%) or
   an unbound association that dissolves at an age drawn on 30–100 Myr. The kind is therefore a
   function of the evaluation time, as a star's state is. Reason: the three kinds' counts then agree
   with each other and with φ(age) by construction: about 2,100 nurseries per Myr gives 10⁴
   star-forming regions, 240–360 bound clusters per Myr and, with the association mass floor as a
   parameter of the generator version, tens of thousands of associations. An association's size is
   its expansion speed × age, 300 ly at 100 Myr and 3 km/s. The brainstorm lists the three as rows
   of one table with counts of their own and says only that "the catalogue splits by marking"; one
   process split by marks is that, and three independent processes could not keep their counts,
   φ(age) and the four-in-five rule consistent. The association's age limit is discussed under
   Risks.
2. **φ(age) is derived, not drawn.** φ(a) = f_n × [Γ_b m_b(a) + (1 − Γ_b)(1 − G(a))], where f_n ≈
   0.9 is the share of star formation in nurseries, Γ_b the bound fraction, m_b(a) the mean
   surviving mass fraction of bound clusters from Lamers's closed form, and G the distribution of
   association dissolution ages. G is tuned so that four in five core collapses fall inside
   features, which a test pins.
3. **Old open clusters follow the sub-discs.** Bound clusters older than 100 Myr are a process per
   sub-disc of the old thin disc, with a constant φ for each sub-disc. The age dependence inside a
   sub-disc is below anything a test could see.
4. **Which budget a globular belongs to.** In-situ clusters count against the bulge (metal-rich,
   inside the bulge's figure) or the thick disc; accreted ones against the halo. Their φ is of order
   10⁻⁴ for the bulge and thick disc and a few 10⁻² for the halo (some 2 × 10⁷ M☉ of accreted
   clusters in a halo of a few 10⁸ M☉), and is separate from the halo's discrete share of streams
   and dwarf cores, which plan 10 supplies through `FeatureShares::set_halo_discrete`. Reason: the
   brainstorm's rule is that a population's budget covers everything born in it, and it says where
   globulars were born (40% in situ in the bulge and thick disc, the rest with the accreted halo).
   The halo's mixture table has no row for living clusters, so their φ is taken from the four smooth
   components in proportion, and the accreted clusters' components follow their progenitors.
5. **The nuclear cluster is a budget of its own.** It is not one of the seven populations and no
   field gives up a share for it. Its mass and break radius are plan 02's `NuclearClusterParams`,
   since the potential needs them. Reason: the brainstorm's seven shares sum to 100% of the drawn
   stellar mass and the nuclear cluster is not among them, while its mass model and its member count
   (4–5 × 10⁷ systems "more") are given separately; carving it out of the nuclear disc instead would
   take 2–5% of that population, which the brainstorm nowhere asks for. The budget test treats it as
   an eighth budget, its mass ÷ the centre's own mean system mass. P09.T27 asserts that the fullest
   cell stays under the 8,192 index for every seed; if a heavy cluster breaks that, the parameter's
   range is narrowed in plan 02, not the grid here.
6. **Index space of a feature cell.** Each process draws its own Poisson candidate count in a cell,
   and the 14-bit index is the candidate number plus the counts of the processes before it, in the
   fixed order globular, old open clusters (one process per sub-disc, youngest first), nursery,
   cloud. Resolving an ID recomputes the counts of the processes before its own, eight at most.
   Globulars come first so that plan 10 can walk them alone.
7. **The galaxy map shows budgets.** Column density keeps integrating the budget, not the field
   times (1 − φ), because features follow their populations statistically.
8. **One effective escape speed per cluster.** Retention compares a kick with the cluster's central
   escape speed at birth times a constant that makes the profile-averaged retention right, computed
   once per generator version. A member's present position says nothing about where it was born.
9. **Profiles.** A class's profile is (1 + r² ÷ r_c²)^(−3q′ ÷ 2) × (1 − r² ÷ r_t²)² inside the tidal
   radius r_t and zero outside, normalised by a fixed 64-node quadrature when the cluster model is
   built. q′ = q^η for q below 1 and q above, with η =
   `tables::cluster_dynamics::EQUIPARTITION_EXPONENT`, provisionally 1 (the brainstorm's full
   equipartition), which is the partial-equipartition constant "Open questions" lists for an offline
   fit. Core-collapsed clusters replace the core by a cusp of drawn slope, softened at 10⁻³ ly so it
   stays integrable and bounded. Internal dispersions use the Plummer form with a = r_h ÷ 1.305.
10. **The tail's bound.** A tail is a Gaussian tube about a straight line, which rises and falls
    along the axes. Its cell bound is the axis density at the point of the cell's bounding sphere
    nearest the line, which is a true bound because the Gaussian falls with distance.
11. **Peri- and apocentre without integration.** A globular's orbit for the dissolution time comes
    from its energy and angular momentum in the mid-plane potential table, by bisection with a fixed
    iteration count. Orbit integration belongs to plan 10.
12. **The Kepler regime is a property of the system.** A system is in it if its epoch distance from
    the black hole is inside the influence radius, whatever the query's time, so no system changes
    regime during play. A grid system there takes plan 08's velocity as the initial condition, and
    one whose pericentre falls inside the loss cone resolves to "no such system", as a member would.
    The propagator uses universal variables, so bound and unbound orbits share one code path.
13. **The inclination mark thins after the position.** Members are placed under the bound of the
    spherical profile, then draw a velocity, then pass the inclination and loss-cone marks. The
    profile's normalisation is divided by the marks' mean acceptance so that the cluster's mass
    comes out right. This wastes a third of the candidates and needs no Bessel function in the
    bound.
14. **Dark remnants at the centre get distribution functions of their own**, one Eddington inversion
    per distinct profile (stars, black holes, the young inner disc), all in the same potential. The
    young disc has no inner hole, because a hole has no isotropic equilibrium.
15. **`CentreModel` and `FeatureShares` are built with `Galaxy`.** Both are small and every query
    near the centre needs them. The build is benchmarked and must stay under 100 ms.
16. **Feature-level lists are ordered by class.** Index 0 is member zero. From 1 upward come the
    list's classes, each with its own Poisson candidate count, in a fixed list order that is not the
    numeric order of `ClassId`: `TIDAL_DISRUPTION_VICTIM` (centre only), `CORE_COLLAPSE`, `TYPE_IA`,
    `LUMINOUS_BLUE_VARIABLE`, then plan 11's five in the order it registers them
    (`ACCRETING_WHITE_DWARF`, `ACCRETING_WHITE_DWARF_SLOW`, `XRAY_BINARY`, `STELLAR_MERGER`,
    `NEUTRON_STAR_MERGER`: the order of P11.T8's subtasks). A class registered later is appended, so
    plan 11 moves no victim, supernova or variable. The order is a constant array with a golden
    test.
17. **H II by expectation.** A nursery is an H II region if its expected number of living stars
    above 15 M☉ is at least one half, a reflection nebula if it has gas and no such stars. Walking
    band E of every young feature to check would cost more than the display is worth.
18. **Light curves are templates by type** with parameters belonging to the generator version: a
    neutrino burst, shock breakout, then a plateau and radioactive tail for hydrogen-rich
    progenitors, or a rise and tail for stripped ones and Type Ia. The brainstorm names no source,
    so the templates are documented as ours.
19. **Cloud statistics.** The brainstorm gives counts and sizes only. Clouds follow plan 07's smooth
    neutral and molecular density, with a mass function falling as M^−1.7 over 10⁴–10⁶·⁵ M☉ and a
    radius from constant surface density. Both are parameters of the generator version. Plan 07
    keeps only a tenth of the central molecular zone's mass in its smooth disc and expects the rest
    as clouds, and that disc holds 5 × 10⁻⁴ of the gas, so in plain proportion the centre would get
    a cloud or two. The cloud process's density is therefore the neutral density plus w times the
    smooth molecular disc's, with w not a free constant: `FeatureShares` solves it at build, in
    closed form from the two components' masses, so that the expected mass of clouds drawn from the
    molecular term is nine times `MolecularDisc`'s mass. P09.T4.c tests the clouds' mass inside
    1,000 ly of the centre.
20. **A member is a `SystemRecord` with extras.** Plan 03's merge hook returns `SystemHit`s over
    `SystemRecord`s, and a record stores a `placement::SystemOrigin` (plan 03, design note 18), not
    a mandatory component. `MemberRecord` therefore wraps a record built with
    `SystemRecord::from_parts`, whose origin is `FeatureMember(feature)`, `CentreMember` or
    `CatalogueSystem(class)` and whose population is that of the budget the member is drawn from
    (design notes 3 and 4; the young disc for a nursery; the nuclear disc's for the centre). No
    member carries a component, so no consumer can read one through a component's laws by mistake:
    `component()` is `None`. Stellar evaluation of a member never calls plan 06's
    `draw_metallicity`: it uses the member's own `Composition` with plan 06's pure functions. See
    Risks.
21. **Disc features are proposed in height, not uniformly.** A 4,096 ly cell is five to ten disc
    scale heights tall, so a uniform proposal under a nearest-corner bound spends most of its
    candidates above the disc: scratch figures put about 7,400 cluster candidates in the densest
    cell at Milky Way values against an index of 16,384, before nurseries and clouds are added and
    before the heaviest galaxy, which holds three times the systems. For the disc processes (old
    open clusters, nurseries, clouds) a candidate's height is therefore drawn from a symmetric
    exponential g(z) of scale H, the largest scale height among the process's components, truncated
    to the cell, and its x and y uniformly. It is accepted with probability density ÷ (B × g(z)),
    where B bounds density ÷ g over the cell: the envelope's nearest-corner value in x and y times
    the ratio at the height nearest the plane, which is where the ratio peaks because no component
    is taller than H. That is thinning with a non-uniform proposal, exact by the same theorem, and
    it cuts candidates about tenfold. Globulars keep the uniform proposal.
22. **One constant cap and the per-galaxy caps.** `SHELL_WINDOW_CAP` is a constant of the generator
    version, because plan 08's `explosion_site` prefilters with it before any galaxy is consulted.
    `WindowCaps::from_galaxy` gives the tighter per-environment suprema that the catalogue's
    thinning envelope uses, and asserts that none exceeds the constant. `claims` cuts a window at
    its environment's cap on every side alike, so the prefilter, the cells and the catalogue agree
    whatever the caps are. The constant stays at plan 08's 4 Myr, the top of the brainstorm's 2–4
    Myr, unless P09.T15.b finds a parameter set whose supremum is greater; that is a finding to
    report against the brainstorm before the constant is raised.
23. **Stream keys.** A feature's marks and its feature-level list draw on
    `ObjectKey::feature(FeatureRef::object_word())`; a feature cell, a nested cell and a
    catalogue-class cell on `ObjectKey::cell(word)`, where the word is the ID of the cell's
    candidate 0 with its index zeroed, as plan 03 does for the grid, with one domain tag per feature
    process so that processes sharing a feature cell share no stream (a nested cell's word already
    carries its band); a member or catalogue system on `ObjectKey::from(SystemId)`. The central
    black hole's events use its own ID, `0xF000_0007_0000_0000`, as the event subject.

## Tasks

The plan is eight phases. Within a phase tasks run in order unless marked parallel. Phases 2, 3 and
4 can run in parallel with each other once phase 1 is done; phase 5 needs phase 2; phase 6 needs
phase 5; phase 7 needs phases 4, 5 and 6; phase 8 follows phase 7, though P09.T39 can start as soon
as the Rust types it mirrors exist. One soft dependency crosses the parallel phases: until P09.T13
lands, `ClusterModel` takes a globular's birth mass and radius equal to today's, and P09.T11 waits
for P09.T13. Task IDs are stable because other plans cite them (P09.T13, P09.T28.c), so a task added
in validation keeps the next free number wherever it sits: P09.T43 is in phase 7.

Common to every task: every new draw opens its stream with `Stream::open(seed, tag, key)` on a new
domain tag (`feature.*`, `member.*`, `centre.*`, `snr.*`, `class.*`) that the task adds to plan 01's
registry `rng/tags.rs` under a "Plan 09" heading with the scope design note 23 implies, so the
collision test covers it; decisions compare a `Mark` with a `Threshold`; transcendental functions go
through `math`; figures are re-checked against the cited source and the citation goes in the doc
comment; goldens use `hyperion-testkit`; and `just ci` passes.

### Phase 1: the catalogue and φ

#### P09.T1 Identifiers and kinds

The bit layouts exist (plan 01, P01.T6.c and T6.d). This task adds `FeatureId` over `id::FeatureRef`
with a designation in the manner of plan 01's `Designation`; `FeatureProcess`; `FeatureKind`
(`Globular`, `OpenCluster`, `Association`, `StarFormingRegion`, `MolecularCloud`, `GalacticCentre`,
and `Stream`, `DwarfCore` reserved for plan 10); the `ClassId` registry with `cell_log2_ly()` per
class and a golden list that fails if a value is renumbered; the list order of design note 16 as a
constant array; `CatalogueCellKey`. It also reconciles this plan's "Consumes" with what plans 01–08
actually built, before other work starts. Files: `galaxy/features/{mod,ids}.rs`,
`galaxy/catalogue_classes/{mod,registry}.rs`. Tests: unique designations over a sampled feature
cell; registry golden. Acceptance:
`cargo test -p hyperion-sim features::ids catalogue_classes::registry` passes.

#### P09.T2 Feature rates and the φ table

- **P09.T2.a Rates and lifetimes.** `NurseryRates::from_galaxy`: nursery birth rate from the young
  disc's formation rate, the mass function, the bound fraction, the embedded duration, G(a).
  `open_cluster::dissolution_time(m0) = 1.3 Gyr × (m0 ÷ 10⁴ M☉)^0.62` and `present_mass(m0, age)`
  from Lamers et al. (2005), and `surviving_mass_fraction(age)` = m_b(a) by a fixed quadrature over
  the mass function. Files: `features/shares.rs`, `features/kinds/open_cluster.rs`. Tests: 240–360
  bound clusters born per Myr and about 10⁵ alive at Milky Way parameters, a third under 100 Myr, a
  tenth over 1 Gyr, mean life near 295 Myr.
- **P09.T2.b `FeatureShares`.** φ for the young disc as a function of age (design note 2), one
  constant per old sub-disc, the globulars' φ for bulge, thick disc and halo from the expected
  number × mean mass of the evolved Schechter function, per band where the class tables of phase 2
  deplete a band (until then uniform across bands, with a `TODO(P09.T9)` removed in that task).
  Tests: φ_young(3 Myr) within 0.85–0.95, φ_young(100 Myr) within 0.05–0.15, monotone falling.
- **P09.T2.c Apply to the field.** Through new variants of plan 02's `FeatureShare`, `fields`
  multiplies each population's density by `field_factor`, plan 08's `stay_share` takes the same
  factor while its budget-fed class weights do not, the young field's age distribution carries (1 −
  φ(a)) and is renormalised, and the share matrix takes the per-band factor. `map` keeps the budget.
  Bump `GENERATOR_VERSION`, regenerate goldens. Files: `galaxy/fields/*`, `galaxy/placement.rs`,
  goldens. Tests: the young field's age histogram against (1 − φ) × budget by chi-square; the map
  unchanged bit for bit. Acceptance: `just ci` green with regenerated goldens, and the diff of
  goldens reviewed to touch only counts, never the layout of an unaffected cell's survivors
  (candidates keep their streams).

#### P09.T3 The feature catalogue grid

- **P09.T3.a Densities and bounds.** For each `FeatureProcess` a closed-form number density of
  features: globulars a cored r^−3.5 (phase 3 supplies its parameters; a stub of zero until then),
  old open clusters per sub-disc, nurseries on the young disc with its sharp arms, clouds on plan
  07's smooth neutral density. Bounds over a 4,096 ly cell by plan 02's nearest-corner and
  unimodal-factor rule. Files: `features/catalogue.rs`. Tests: a bound hunt over 10⁵ random points
  per process in cells along arm ridges and at the bar's end; debug assertion density ≤ bound.
- **P09.T3.b Thinning, resolve, neighbours, walk.** Per cell and process: Poisson candidates from
  bound × volume on the cell's stream (one tag per process), each candidate its own stream keyed by
  `FeatureId`, a position as integer light-years plus offset, uniform for globulars and proposed in
  height for the disc processes (design note 21), acceptance density ÷ bound or density ÷ (B × g).
  Index offsets per design note 6. `resolve` reruns one candidate. `near` visits cells within
  radius + `MAX_FEATURE_REACH`. `walk_process` visits all 32,768 cells. The cache is a trait the
  caller implements. Tests: order independence, `resolve` agrees with `cell` for every survivor and
  returns `None` for rejected and out-of-count indices, golden file of one inner and one outer cell,
  heights of accepted disc features against the process's vertical law by Kolmogorov–Smirnov (the
  proposal must not show), a hunt for density ÷ (B × g) above 1 over 10⁵ points per process, and an
  assertion that the summed expected candidates of the fullest cell plus eight standard deviations
  stay under 16,384 over 200 seeds and for the heaviest galaxy the parameter ranges allow; the
  candidate count is clamped to the index with a `debug_assert!`, as plan 03 does.
- **P09.T3.c Counts and benchmark.** Slow test at Milky Way parameters: about 10⁵ open clusters, 10⁴
  star-forming regions, associations within 10⁴–1.5 × 10⁵, clouds in the thousands, each within
  Poisson error of the process's own expected count. Criterion benches: one inner-disc cell (target
  under 10 ms), the full globular walk (target under 0.3 s).

#### P09.T4 Marks of each kind

Parallel with each other after P09.T3.b.

- **P09.T4.a Open clusters.** Age (young: from the nursery; old: ∝ formation history × survival
  within the sub-disc), initial mass conditional on being alive at that age, present mass, half-
  mass radius (log-normal about 6–10 ly, generator-version parameter), concentration, metallicity
  from the population's law at the position and age, bulk velocity from the population's velocity
  law on the feature's stream. Tests: present-mass function against the closed form; every cluster's
  age below its dissolution time.
- **P09.T4.b Nurseries: star-forming regions, associations and superbubbles.** `NurseryMarks` (mass,
  bound mark, dissolution age, expansion speed 1–5 km/s, age spread up to 3 Myr, gas mass while
  embedded), `NurseryStage::at(age + t)`, size = expansion speed × age capped at 300 ly.
  `Superbubble`: radius 0.76 × (L_w t³ ÷ ρ)^⅕ (Weaver et al. 1977) with the wind and supernova power
  from the expected count of O and B stars and the smoothed gas density at the site, capped at
  blow-out (2.5 gas scale heights at that radius), interior density log-normal about 0.005 cm⁻³ at
  10⁶·² K. Members of an embedded region draw ages from −H. Tests: sizes 10–300 ly, bubble radii
  100–1,000 ly, stage transitions continuous in time.
- **P09.T4.c Molecular clouds and dark nebulae.** `CloudMarks` per design note 19, a Plummer-like
  gas profile, mean density 10²–10⁶ cm⁻³ towards the core, dust by the local dust-to-gas ratio. The
  process's density carries the molecular weight w of design note 19. Tests: sizes 50–300 ly, count
  in the thousands; the expected mass of clouds inside 1,000 ly of the centre is nine times plan
  07's `MolecularDisc` mass to 10%, and at Milky Way parameters clouds plus smooth disc there hold
  2–5 × 10⁷ M☉; over 200 seeds the realised cloud mass there is Poisson-consistent with the
  expectation; with w forced to 1 in a test build the same region holds under a tenth of that (the
  defect this guards against).
- **P09.T4.d Emission classes.** `EmissionClass::of(record, t)` per design note 17; dark cloud for
  clouds; remnant shell and pulsar wind nebula come from phase 4. Tests: table-driven.

#### P09.T5 Features in the gas field

`FeatureGas` implements plan 07's `GasModifierSource`: for a segment it looks up the features near
it and yields a `GasModifier::Hole` for each superbubble (radius and interior density from P09.T4.b)
and a `GasModifier::Cloud` for each cloud and embedded region (Plummer core radius, central density,
dust per hydrogen). Plan 07 integrates them; the server passes `FeatureGas` where it passed
`NoModifiers`. The shell window of phase 4 does not use modifiers: it reads the smooth field only.
Files: `features/gas_overlay.rs`, the server's call sites. Tests: a ray through a cloud's centre
gains the analytic column of a Plummer profile; a point in a bubble reads the interior density; far
from any feature plan 07's results are unchanged bit for bit.

#### P09.T6 Budget test, part one

Slow test, per population and band, at three seeds: expected field count plus expected feature
members (catalogue rate × mean members, from closed forms and not from sampling) equals the budget
to 10⁻⁶ relative; and the young disc by age in ten bins. A sampled check follows in P09.T38 once
members exist. Acceptance: `just test-slow` passes.

### Phase 2: cluster interiors

Pure functions of a `FeatureRecord`. Nothing is placed until phase 5.

#### P09.T7 `ClusterModel`

Structure and time scales of one cluster, built once per resolved feature and cached by the caller:
mass, half-mass and core radii, tidal radius from `PotentialTables` at the cluster's position, the
turn-off mass at its age and metallicity, the half-mass relaxation time t★ = 0.138 √(M r_h³ ÷ G) ÷
(⟨m⟩ ln Λ) with ⟨m⟩ = 0.45 M☉ and ln Λ = 10, the central escape speed √(GM ÷ r_h) × 10^(0.1055 +
0.2550u − 0.0769u²) with u = log₁₀(r_h ÷ r_c) (fit to the 157 clusters of the Baumgardt–Hilker
catalogue; re-check against the online catalogue), the birth escape speed (today's × √((M₀ ÷ M) ×
(r_h ÷ r_h0)), with M₀ and r_h0 from Lamers for open clusters and from P09.T13 for globulars), σ(r),
and the encounter rate Γ ∝ ρ_c^1.5 r_c². The prefactor 0.138 is
`tables::cluster_dynamics::BH_RELAXATION_PREFACTOR`. The nested grid's width needs the class table
and is chosen in P09.T21. Files: `features/cluster.rs`. Tests: the named clusters of
`features::testing` give 47.4, 62.2, 18.8, 48.9 and 2.1 km/s to 5%; open clusters of 10², 10³ and
10⁴ M☉ give about 1.1, 2.4 and 6.1 km/s.

#### P09.T8 The class device

- **P09.T8.a Profiles.** `ClassProfile` per design note 9: core or cusp, the exponent q′ from
  `EQUIPARTITION_EXPONENT`, the common taper, normalisation, radial cumulative distribution
  tabulated for inverse transform, flattening along the grid's z for the kinds that have it.
  Property test: never rises with radius; integrates to 1 to 10⁻⁹; the inverse transform reproduces
  the profile (Kolmogorov–Smirnov).
- **P09.T8.b `MemberClassTable`.** Per band a list of (class, expected count, profile).
  `bound(band, corner)` is the sum of count × profile at the nearest corner, plus the tail's bound.
  `pick` takes one `Mark`: below Σ count × profile(position) ÷ bound it selects a class by
  cumulative odds in the table's fixed order, otherwise it rejects, through plan 01's
  `Mark::pick_weighted`. The class is a derived mark and never enters an ID. `density(position)` and
  `mean_member_mass(position)`, summed over bands and classes, are what plan 14 reads. Tests: class
  frequencies at fixed positions by chi-square; the sum of odds never exceeds the bound in a hunt
  over 10⁶ positions.

#### P09.T9 Class counts

Each subtask adds rows to the class table and a closed form for their counts. Parallel after P09.T8,
except that P09.T9.d needs P09.T9.c.

- **P09.T9.a Living stars, white dwarfs and the depleted dwarfs.** Counts by band from the mass
  function evolved to the cluster's age (turn-off and lifetimes from plan 06). For 0.2–0.8 M☉ the
  present slope is −0.46 − 0.79 × (log₁₀ t_rh[yr] − 9) plus a per-cluster normal scatter of 0.54,
  clamped at the canonical −1.5 so that no cluster gains dwarfs; bands A and B are scaled by the
  ratio of the depleted to the canonical integral. White dwarfs are classes of bands C and D by
  final mass. Removes the `TODO` in `FeatureShares`. Test: at 47 Tucanae's slope band A holds a
  third (±0.05) of the canonical count.
- **P09.T9.b Neutron-star and white-dwarf retention.** Retained fraction = the kick law's
  distribution below the effective birth escape speed (design note 8), by a fixed quadrature over
  plan 06's `KickLaw` through plan 08's `kick_bins::speed_bin_shares`: ordinary mode on the star's
  own speed, low mode on the pair's velocity. White dwarfs' 1 km/s kick applies to open clusters.
  Tests: 18–26% at 100 km/s, 13–19% at 50, 8–12% at 20, under 1% for a 10⁴ M☉ open cluster; a few
  thousand neutron stars in the 47 Tucanae model, about a hundred in M4, none expected in Palomar 5.
- **P09.T9.c Black holes.** Retained at birth from the kick law with complete fallback unkicked
  (about four fifths). Mass fraction today f(t) = [(1 + ψ₁ f₀) e^(−β ψ₁ t ÷ t★) − 1] ÷ ψ₁, floored
  at zero, with f₀ = 0.06 × retention (Breen and Heggie 2013; Antonini and Gieles 2020). It is the
  solution, at constant mass and radius, of df ÷ dt = −β (1 + ψ₁ f) ÷ t★: black-hole mass is lost at
  β cluster masses per relaxation time, and the relaxation time shortens by 1 + ψ₁ f while black
  holes remain. β = 2.8 × 10⁻³ and ψ₁ = 147 are `BH_LOSS_BETA` and `BH_LOSS_PSI_SLOPE` of
  `tables::cluster_dynamics`, read from there and never written as literals, and with
  `BH_RELAXATION_PREFACTOR` in t★ they are the three constants plan 15's P15.T8.a fits against the
  CMC catalogue (Kremer and others 2020); these are the scratch values until then. The decay rate β
  ψ₁ = 0.41 is derived, not a constant of its own, and with f₀ near 0.05 the black holes are gone
  after about five relaxation times, inside the brainstorm's four to six. Black holes are a compact
  Plummer class of scale 0.1–0.3 r_h, and the drawn core radius is correlated with f. Tests: none in
  dynamically old models, tens to a few hundred in a typical massive one, thousands in ω Centauri's;
  f reaches zero between four and six t★ for retention between 0.6 and 1.
- **P09.T9.d Core collapse.** f = 0 and age above 14 t★ sets `is_core_collapsed`, and every class
  takes a cusp of slope drawn on −1.6 to −2. Test: about a fifth (0.12–0.28) of
  `milky_way_globulars` over the line (Trager et al. 1995).
- **P09.T9.e Binaries and recycled objects.** Binaries are classes by system mass with a fraction
  per kind and population (scratch: 5% first and 1% second population in globulars, 30% in open
  clusters; plan 11 replaces the numbers, not the device). Millisecond pulsars, X-ray binaries and
  blue stragglers are marks inside the neutron-star and binary classes with expected counts from Γ:
  40 × (Γ ÷ Γ_47Tuc)^0.7 pulsars, capped for core-collapsed clusters, with the three numbers read
  from `PULSARS_AT_47_TUC_GAMMA`, `PULSAR_GAMMA_EXPONENT` and `PULSAR_CORE_COLLAPSE_CAP`. Test:
  about 4,000 (2,000–8,000) pulsars over `milky_way_globulars`.
- **P09.T9.f Runaway factor.** A young cluster's living band-E count × (1 − f_ej(M)), with f_ej 15%
  rising to 38% at 10^3.5 M☉ (Oh et al. 2015), and a few per cent in band D. The ejected are plan
  08's runaways, already fed by the budget. Test: table-driven.
- **P09.T9.g Tails.** One class per band: a straight tube along the bulk velocity through the
  cluster, from the tidal radius to the grid's reach, Gaussian across with a width of one tidal
  radius, line density = mass-loss rate ÷ drift speed along the tail, mass-loss rate from Lamers or
  from P09.T13. Bottom-heavy band shares (what the cluster lost) and first population only. Bound
  per design note 10. Tests: bound hunt; the tail's expected count equals the mass lost in reach ÷
  drift speed.
- **P09.T9.h Multiple populations.** For globulars born above 10⁵ M☉ an independent mark splits each
  class into first and second population; the first's share is 0.62 − 0.30 × (log₁₀ M₀ − 5) held to
  0.1–0.7. The second has a smaller core radius (scratch factor 0.7) and the lower binary fraction.
  A second-population member draws an enrichment e in (0, 1]; `MemberAbundances` is linear in it:
  nitrogen up 0.5–1.2 dex and sodium 0.3–0.6, oxygen down 0.2–0.8 and carbon down, aluminium up and
  magnesium down only above 10⁶ M☉ and below [Fe/H] −1, iron spread in one cluster in six, helium Y
  = Y₀ + ΔY_max e² with ΔY_max from 0.01 at 10⁵ M☉ to 0.18 above 10⁶ (Milone and Marino 2022).
  Tests: shares at 10⁵, 10⁶ and 10⁶·⁵ M☉; helium never above 0.18; tails hold no second population.

#### P09.T10 Conditional member draws and velocities

`members::draw_member(feature, class, position, id) -> MemberRecord`: initial mass within the
class's sub-range (depleted slope where it applies), and for remnant classes a redraw loop on the
member's own streams, attempt after attempt, until remnant kind and kick satisfy the class (a
retained neutron star has a kick below the effective escape speed). The loop is bounded at 4,096
attempts with a debug assertion; its expected length is 1 ÷ retention. Age and metallicity are the
feature's (with the age spread for nurseries). Velocity = bulk + isotropic normal of σ(r) ÷ √q, cut
at the local escape speed. Sphere of influence: the smaller of the galactic tidal radius and the
same formula about the feature's centre, with the floor in a harmonic core (`sphere_of_influence`).
Plan 06's `SystemStars::generate(galaxy, record)` draws metallicity from the record's component,
which a member must not do, so this task adds
`SystemStars::generate_with(galaxy, record, &Composition)` beside it in `stellar/system.rs`, with
`generate` delegating to it, no output change. Files: `features/members.rs`, `stellar/system.rs`.
Tests: order independence; a retained neutron star's kick is always below the escape speed; velocity
dispersion by class against σ(r) ÷ √q.

#### P09.T11 Interior checks

Slow tests over `named_cluster` and `milky_way_globulars`: the figures of P09.T9 together, the
retention test of the brainstorm's kick law ("at least a tenth in a cluster with a birth escape
speed of 50 km/s"), and total black holes over the system of order 10⁴–10⁵.

### Phase 3: the globular system

#### P09.T12 Globulars as they are today

- **P09.T12.a Number, origin and place.** Expected number = dark halo mass ÷ 6.5 × 10⁹ M☉ with 0.2
  dex scatter (Burkert and Forbes 2020), read from plan 02's `AccretionHistory`, which draws it. An
  origin mark: 40% in situ, the rest among the accreted progenitors in proportion to mass, sharing
  the progenitor's halo component (Massari et al. 2019). Density a cored r^−3.5 with a core near
  3,900 ly so that the median radius is 5 kpc, flattened to 0.5 and inside 26,000 ly for the
  metal-rich in-situ clusters, cut at 65,000 ly. Replaces the stub of P09.T3.a. Tests: 80–800 over
  seeds, about 160 at Milky Way parameters, 85% of the untruncated law inside the cube.
- **P09.T12.b Mass, size, metallicity and age.** Mass from (M + Δ)⁻² e^(−(M + Δ) ÷ M_c) with Δ = 2.0
  × 10⁵ and M_c = 1.07 × 10⁶ M☉ over 10³–10⁷ M☉, by inverse transform of a tabulated cumulative
  function, the same at every radius. Half-mass radius 2.6 pc × (R ÷ kpc)^0.41 with 0.21 dex of
  scatter, concentration drawn and later corrected by P09.T9.c. Metallicity: 30% N(−0.55, 0.25), in
  situ; 70% N(−1.55, 0.35). Age 12.8 Gyr in situ, 10.5–13 Gyr accreted. Bulk velocity from the
  origin component's law (plan 08). Tests: Kolmogorov–Smirnov of mass and size against the closed
  forms.

#### P09.T13 Orbit and history inversion

Peri- and apocentre and eccentricity per design note 11. Dissolution time from Baumgardt and Makino
(2003) as a function of initial mass, apocentre and eccentricity. Initial mass by solving M = 0.70
M₀ (1 − t ÷ t_dis(M₀)) with 40 bisection steps. Birth half-mass radius from the expansion of Gieles,
Heggie and Zhao (2011). Outputs feed `ClusterModel` (birth escape speed, mass-loss rate for tails,
the first population's share). Destroyed clusters are not generated here: a test asserts that no
code path creates a globular with zero present mass. Tests: initial masses of `milky_way_globulars`
within 0.1 dex of the Baumgardt–Hilker catalogue's where tabulated; median birth escape speed about
twice today's.

#### P09.T14 Milky Way checks: the globular system

Slow test at Milky Way parameters over 50 seeds: mass function (turnover near 2 × 10⁵ M☉, the same
inside and outside 5 kpc), sizes against radius, median eccentricity 0.5–0.75 and median pericentre
1–2.5 kpc, central escape speeds with a median near 20 km/s and none above 100.

### Phase 4: supernova remnants and the clocked test

Pure functions. The processes that use them are in phase 7.

#### P09.T15 The shell window

- **P09.T15.a Closed form.** `shell_window(site: &SiteGas, energy, metallicity) -> ShellWindow`. The
  shell is distinct until its shock slows to β c_net, β = 2, c_net² = c_th² + σ², σ = 8 km/s, c_th²
  = γP ÷ ρ from the site's pressure and density. Radiative branch (Cioffi, McKee and Bertschinger
  1988): W = t_PDS × [¾ (v_PDS ÷ β c_net)^(10⁄7) + ¼], with t_PDS = 1.33 × 10⁴ yr E₅₁^(3⁄14)
  ζ^(−5⁄14) n^(−4⁄7) and v_PDS = 413 km/s n^(1⁄7) ζ^(3⁄14) E₅₁^(1⁄14). Hot branch (Tang and Wang
  2005), taken when the blast turns sonic before t_PDS: W = 0.41 t_c, with t_c their characteristic
  time from energy, pressure and sound speed. `SiteGas` comes from `GasField::state` with
  `SmoothingScale::AtLeast(250 ly)`, floored at the corona's pressure. β, σ and the smoothing scale
  belong to the generator version. Files: `galaxy/snr.rs`. Tests: `window_table` at P ÷ k = 3,800 K
  cm⁻³ reproduces 2.0, 3.5, 6.2, 7.5, 4.3, 1.9 and 0.35 × 10⁵ yr at n = 10⁻³ … 10⁴ to 25%;
  continuity across the branch; W → 0 at both ends of density; the longest windows, 0.5–1 Myr, fall
  at 0.1–0.5 cm⁻³ in the model's own pressure field.
- **P09.T15.b Caps.** `ShellEnvironment { Field, TypeIa, Bubble }` and `WindowCaps::from_galaxy`:
  the supremum of W over the galaxy's allowed gas states, per environment, by a fixed scan once per
  galaxy: 2–4 Myr in the field, about 0.5 Myr in a bubble. `SHELL_WINDOW_CAP` moves from plan 08's
  `displaced` to `snr` (design note 22), and building `WindowCaps` asserts that no cap exceeds it. A
  debug assertion that no window exceeds its cap, a slow test that hunts for a violation over 10⁶
  random sites, and a slow test that every cap is under `SHELL_WINDOW_CAP` over 200 seeds and at the
  corners of the gas parameters' ranges.

#### P09.T16 Shell evolution, emission and what is inside

- **P09.T16.a Radius, phase and emission.** `shell_state_at(window, age) -> ShellState`: radius and
  shock speed through free expansion, Sedov–Taylor, the pressure-driven snowplough and the
  momentum-conserving snowplough (Truelove and McKee 1999; Cioffi et al. 1988), continuous at the
  joins; inside a superbubble the hot branch with the window ended early at the bubble's wall.
  `ShellPhase` and emission: X-ray and radio while non-radiative, optical filaments while the shock
  is above 70 km/s, the 21 cm shell to the end. Tests: radii of 10–800 ly across the window table;
  radius monotone and continuous in age; a shell in a bubble is gone in about 10⁵ yr.
- **P09.T16.b What is inside.** `remnant_offset` = kick × age, with a bow-shock flag once it passes
  0.68 of the shell's radius (van der Swaluw et al. 2003). `PulsarWindNebula` present while plan
  06's spin-down power is above a threshold of the generator version, which comes to 10⁴–10⁵ yr.
  Tests: median offset near 200 ly and about half outside the shell for field remnants; most shells
  older than 10⁵ yr have no nebula.

#### P09.T17 The supernova test with a clock

`DeathMarks { death_time: T, energy, metallicity, fastest_member_speed, kind: SupernovaKind }` and
`claims(galaxy, site, marks, environment)`: true if and only if −(W_eff + L + H) < T ≤ +H. W is
`shell_window` at the site, W_eff = min(W, the environment's cap, 4,096 ly ÷ v_fast − (L + 2H))
floored at zero, so that the entry's whole lifetime, W_eff + L + 2H, is held to 4,096 ly ÷ its
fastest member's speed, as the brainstorm asks. v_fast is the largest galactic-frame speed any
member has after T: the system's velocity plus the kick, or a Type Ia survivor's speed. The thinning
cap of phase 7 is the environment's cap grown by L + 2H, the interval's full length.

`DeathMarks::of(galaxy, stars, site)` is the one constructor: T and the death kind from plan 06's
`SystemStars::death_time()`, the kick from `SystemStars::natal_kick()`, the metallicity from the
system's `Composition` (a member's own, design note 20), and the explosion energy from a log-normal
about 10⁵¹ erg (a parameter of the generator version) on `snr.energy` keyed by the system's ID. The
grid passes the `SystemStars` of its grid record, a feature those of its member, the catalogue those
of its own candidate, so every side evaluates the same function on the same kind of marks. `claims`
reads the smooth gas field (never plan 07's modifiers), the marks above, and for
`ShellEnvironment::Bubble` the bubble's interior state, which the caller passes from the member's
own parent feature. It never looks up a feature or another system, which keeps the dependency graph
acyclic; a test runs it with a `FeatureCellCache` that panics on use. `SupernovaInterval` exposes
the two ends for the processes of phase 7. Tests: membership is independent of any query time; a
property test that `claims` is a pure function of its arguments; boundary cases at T = +H, at T =
−(W_eff + L + H) and at the lifetime cap, where a survivor at 2,500 km/s holds W_eff under 0.23 Myr.

#### P09.T18 Type Ia as a class of layer D

- **P09.T18.a Rates and the ancient share.** `DelayTimeDistribution`: 2.13 × 10⁻¹³ (t ÷ Gyr)^−1.1
  per year per solar mass formed from 40 Myr, integrating to 1.3 × 10⁻³ (Maoz and Graur 2017),
  applied to each population's formation history and the mass formed per system, plan 02's
  `Galaxy::mean_formed_mass`. `ancient_share(population)`: the share of layer D that exploded before
  the interval and left nothing, 2–4%, which `ShareMatrix` removes from layer D's field share
  (version bump with P09.T35). Tests: 0.4–1 Type Ia a century over seeds, 0.40 at Milky Way
  parameters to 15%; a fifth of delays under 0.1 Gyr and 62% under 1 Gyr.
- **P09.T18.b The delay-first draw.** `IaProgenitor::draw(stream, population)`: age ∝ formation
  history × ψ, time of explosion within the interval, channel given the delay, the two masses given
  channel and delay (scratch closed forms until plan 15's samplers; the primary always at least 2.5
  M☉ so that it is layer D, and both lifetimes within the delay), and for a merger the separation
  after the common envelope from Peters (1964): a⁴ = (256 ⁄ 5) G³ m₁ m₂ (m₁ + m₂) t_insp ÷ c⁵ with
  t_insp = delay − lifetimes. `IaProgenitor::period_at(t)`. Tests: lifetimes plus inspiral equal the
  delay to a second; the orbital period a thousand years before a merger is 80–100 s.
- **P09.T18.c What a Type Ia leaves.** `IaLeftover` by channel, as defaults of the generator
  version: both destroyed 50%; a surviving donor at 1,900–2,500 km/s 30% (Shen et al. 2018); a
  hydrogen donor at 100–250 km/s under 5%; Iax with a partly burnt white dwarf 10% (Foley et al.
  2013). A recent survivor is member 1 of the entry at speed × age. Tests: channel frequencies.

#### P09.T19 State by evaluation time, and light curves

- **P09.T19.a State.** `SupernovaEntry::state_at(t) -> SupernovaState`: `Progenitor` (plan 06's
  star, or the inspiralling pair) before T; `Supernova { age, shell, remnant }` from T;
  `BareRemnant` beyond T + W_eff. The explosion is also an event: `EventId` with tag
  `0x0202 SUPERNOVA`, bin 0, number 0, on the entry's member 0, registered in `event_tags!` by this
  subtask, so that plan 12's alerts can name it. Tests: state continuous on either side of T except
  the explosion itself; one ID before and after; the event ID parses and round-trips.
- **P09.T19.b Light curves.** `LightCurve::luminosity(kind: SupernovaKind, age)` per design note 18,
  with plan 06's `SupernovaType` from the progenitor's envelope at death for a core collapse. Tests:
  light curves positive, peaked within 100 days, and under 10⁻³ of peak after ten years; continuity
  at the joins of each template.

### Phase 5: nested grids and member IDs

#### P09.T20 Nested grid geometry

`NestedGrid::new(width, cells_per_axis, levels)`, generic over the catalogue features' 16 cells and
eight levels and the centre's 32 and twelve. Level j is a block of cells of width w × 2ʲ centred on
the feature; its inner half per axis is the volume of level j − 1, which owns it; level 0 owns its
whole block. The width is a power of two, so every cell edge is an exact binary fraction, and the
feature's centre is a cell corner at every level. `cells_touching(sphere)` yields owned cells only,
level by level; `owner_of(position)` is its inverse. Files: `features/nested.rs`. Tests: the owned
cells of all levels tile the grid's cube exactly once (property test over random points); with w =
0.5 ly the reach is 512 ly; `cells_touching` equals a brute-force scan.

#### P09.T21 Member placement by band

`MemberClassTable::grid_width()`: the smallest power of two w, in light-years, for which no owned
cell and band of `NestedGrid::new(w, 16, 8)` expects more than 2,000 candidates, found by doubling
from 1 ⁄ 64 ly over the cells on the axes, where the bound peaks. It is part of the generated
output. Then, for a feature, band and owned cell: bound from `MemberClassTable::bound` at the cell's
corner nearest the centre; Poisson candidates from bound × volume on the cell's stream; per
candidate a stream keyed by its `FeatureMemberId`, a uniform position, one uniform for
`MemberClassTable::pick`, then `draw_member`. `resolve_member(galaxy, id)` reruns one candidate and
returns "no such system" for an index beyond the count or a rejected candidate; plan 03's `resolve`
dispatches reserved-layer IDs here. Debug assertion and test that no cell and band exceeds 8,192
candidates, with the count clamped as plan 03 clamps the grid's. Tests: every generated member's ID
round-trips through `SystemId::from_raw`, so no member sits in an inner cell that plan 01 rejects; a
sampled globular's radial profile per class against `ClassProfile` (Kolmogorov–Smirnov); a globular
of 10⁶ systems with a core of 240 per cubic light-year peaks near 600 members a cell; counts per
cell flat or falling outward; order independence; golden members of one open cluster and one
globular.

#### P09.T22 The feature-level member list

`FeatureLevelList::of(feature)`: members under plan 01's `MemberSlot::FeatureLevel` (band value 7).
Index 0 is member zero where the kind has one. From 1, classes in the list order of design note 16,
each a Poisson count on the feature's stream with positions by inverse transform of the class
profile's cumulative distribution, which is exact. This task builds the list and its resolution with
no class registered; P09.T30.b, T36 and T43 register theirs. The list's summed expected count plus
eight standard deviations must stay under 8,192, asserted when the list is built, and the count is
clamped. Tests: resolution, canonical IDs (level and cell bits zero), and that registering a second
class leaves the first's indices unchanged.

#### P09.T23 Feature members in the range query

`FeatureMemberSource`: for a query sphere, the features from `FeatureCatalogue::near`, then for each
the nested cells touching the sphere padded by plan 03's speed × |t| rule, band by band from the
coarsest mass band down. Expected count for the census = the sum of the visited cells' bounds ×
volumes, which errs high. Distances are tested at the query's time with members drifting in straight
lines at bulk plus internal velocity; members of embedded regions not yet born are dropped. Each
member counts in the layer of its band. Tests: a 50 ly query in a globular's core equals a
brute-force enumeration of the whole cluster; census decisions are independent of cache state;
bench: that query cold, target under 20 ms.

### Phase 6: the galactic centre

#### P09.T24 Profile and distribution function

- **P09.T24.a Profile and potential.** `CentreProfile`: the broken power law with inner slope 1.3, a
  break near 10 ly and outer slope 3.5 (Schödel et al. 2014; Gallego-Cano et al. 2018), continued
  inward to 10⁻³ ly, then r^−½, truncated at the grid's reach of 128 ly; mass and break from
  `NuclearClusterParams`; enclosed mass in closed form by pieces; potential of the black hole plus
  the cluster. Tests at Milky Way values: about 7,800 systems per cubic light-year at 3 ly under the
  default mass function (9,000 under Kroupa's), 4–5 × 10⁷ systems, fewer than three inside 10⁻³ ly,
  the stars outweigh the black hole near 10 ly.
- **P09.T24.b Eddington inversion.** `DistributionFunction::invert(profile, potential)` on a
  logarithmic grid of 256 radii, with the substitution that removes the square-root singularity
  (Binney and Tremaine 2008, eq. 4.46). f must be non-negative everywhere: a returned error, not a
  panic, if a drawn profile fails, and a test that no seed does. The density used everywhere from
  here on is the integral of f, tabulated, so positions and velocities agree by construction. Tests:
  the integral of f returns the profile to 10⁻⁴; a profile with a 0.03 ly core is rejected;
  dispersion 500 km/s (±10%) at 0.1 ly rising as r^−½ inside 3 ly; bench: the three inversions of
  design note 14 together under 50 ms.
- **P09.T24.c Velocity sampler.** Speed at radius r from v² f(Ψ − v² ⁄ 2) by rejection under a
  Beta(3⁄2, γ − ½) proposal in v² ÷ v_esc², direction isotropic, on the member's velocity stream.
  Tests: speed distributions at five radii (Kolmogorov–Smirnov against the numerical density in v);
  no speed above escape.

#### P09.T25 Marks: loss cone, flattening and rotation

A candidate with position and velocity computes its angular momentum. Loss cone: rejected if its
Kepler pericentre about the black hole is inside about 2 au × (M_bh ÷ 4.3 × 10⁶ M☉)^⅓, which removes
about 4 × 10⁻⁵. Inclination: accepted with probability exp(−k sin² i), k ≈ 0.84 for a flattening of
0.7, then a share of the retrograde survivors have their velocity reversed, which keeps energy and
sin² i. Both marks are integrals of the motion. The profile's normalisation is divided by the mean
acceptance (design note 13), computed by a fixed quadrature. Tests: axis ratio 0.65–0.75 from a
sample's second moments; net rotation of the sign of the galaxy's and of a magnitude within the
range of Feldmeier et al. (2014); removed share within a factor of two of 4 × 10⁻⁵.

#### P09.T26 The centre's classes

By the class device of P09.T8, with distribution functions per design note 14: old stars by band
with ages from the centre's own distribution (mostly over 8 Gyr); a young few per cent on an inner
disc (a class with large k and ages from −H to about 10 Myr); white dwarfs; neutron stars on the
stellar profile, widened, with retention from the kick law against the local escape speed (1,100
km/s at 0.1 ly, 210 at 10 ly) averaged over the profile, about a fifth; black holes on a slope of
1.75–2 with a break at half the stars', about nine tenths retained (Bahcall and Wolf 1976). Tests:
10⁴–4 × 10⁴ black holes inside the central parsec (Hailey et al. 2018); retention figures to a
third; the unretained are not generated here (they are in the bulge's displaced classes).

#### P09.T27 The twelve-level grid and the centre's members

`NestedGrid::new(1 ⁄ 256 ly, 32, 12)`: cells from 1 ⁄ 256 ly to 8 ly, reach 128 ly. Placement as
P09.T21 with the marks of P09.T25 after the class pick. `CentreMemberId`; the central black hole is
member zero of the feature-level list, a `MemberRecord` with its mass from `GalaxyParams`.
`CentreModel::as_global_entry()` gives plan 10 the first entry of its list. Plan 03's `resolve`
dispatches `SystemIdKind::Centre` here. Tests: the fullest cell and band expects about 1,400
candidates under the default mass function (1,700 under Kroupa's; under 8,192 for every seed); the
innermost cell about eighty; every member's ID round-trips through `SystemId::from_raw` (levels
0–11, no inner cell of 8–23); the black hole resolves from `0xF000_0007_0000_0000`; goldens of the
hundred innermost members.

#### P09.T28 The Kepler regime

- **P09.T28.a Propagator.** `KeplerOrbit::from_state(position, velocity, mu)` with μ = G × (M_bh +
  M★(< r₀)), propagation by universal variables with a fixed iteration count, valid for elliptic,
  parabolic and hyperbolic orbits. Files: `galaxy/motion.rs`. Tests: an S2-like orbit (semi-major
  axis 0.0158 ly, e = 0.88) gives a 16-year period, 120 au at pericentre and 7,800 km/s there;
  energy and angular momentum conserved to 10⁻¹² over ±1,000 yr; propagating by dt and then by −dt
  returns the start to 10⁻¹² relative; bit-identical goldens on both CI architectures.
- **P09.T28.b Regime and the drift hook.** `regime_of(record)` per design note 12, with the
  influence radius where the enclosed stars equal the black hole's mass. `position_velocity_at`
  replaces the straight line for Kepler-regime systems in plan 08's drift hook, for members and grid
  systems alike, and the grid's loss-cone carve-out. Bump `GENERATOR_VERSION`. Tests: positions at t
  = 0 unchanged for every golden system; at 9.9 and 10.1 ly the two regimes differ by less than 0.01
  ly after 1,000 yr.
- **P09.T28.c Sphere of influence at pericentre.** A Kepler-regime system's tidal radius is taken at
  its pericentre and is constant in time: 2.7 au for a semi-major axis of 0.01 ly. Test: that
  figure, and about 0.01 ly for a near-circular orbit at 3 ly.
- **P09.T28.d The two secular rates.** `propagate` turns the orbit in its plane by two rates × dt:
  the Schwarzschild advance 6πGM ÷ (c² a (1 − e²)) per orbit (`schwarzschild_rate`) and the
  retrograde precession from the enclosed stars (`mass_precession_rate`), from the orbit-averaged
  force of the r^−γ cusp tabulated in eccentricity once per galaxy (Binney and Tremaine 2008, §3.2;
  Merritt 2013, ch. 4, to re-check). Unbound orbits take neither. Bump `GENERATOR_VERSION`:
  positions away from the epoch move, those at the epoch do not. Tests: 12′ of advance per orbit for
  the S2-like orbit; against a direct integration in the smooth potential of black hole plus
  cluster, position error under a tenth of the tidal radius after 1,000 yr at 0.1, 1 and 5 ly; the
  profile test of P09.T31 is the acceptance of the pair.

#### P09.T29 The range query at the centre

`CentreMemberSource`: per level the sphere is padded by the radial plunge Δ(r₀, t) = r₀ − (r₀^(3⁄2)
− 1.5 √(2GM) |t|)^(2⁄3), and everything inside r_full = (1.5 √(2GM) |t|)^(2⁄3) is scanned in full.
Orbits are propagated to the query's time and tested there. The caller may supply a cache of orbital
elements per cell (`CentreOrbitCache`). The same pad applies to grid systems in the Kepler regime.
Tests: r_full of 0.31 ly and about 37,000 systems at a century (42,000 under Kroupa's); a query at t
= ±100 yr equals a brute-force scan of the inner 2 ly; bench: the century query with cached orbits,
target about 10 ms.

#### P09.T30 The black hole's own events

- **P09.T30.a Flares and disruptions.** On the black hole's event key (`EventKey::derive` with the
  black hole's ID as subject), with plan 06's `PoissonBins`: flares at about one a day with a
  power-law energy distribution (Neilsen et al. 2013), tag `0x0200 CENTRE_FLARE`, in bins of one
  day; tidal disruptions at 10⁻⁴ per year × (M_bh ÷ 4.3 × 10⁶ M☉)^−0.4 (Stone and Metzger 2016), tag
  `0x0201 TIDAL_DISRUPTION`, in bins of 1,024 years so that the source horizon is a few hundred
  bins. Both tags are registered in `event_tags!` here, inside plan 06's block for this plan. Tests:
  the same events in any order of asking; 0.06–0.2 disruptions expected in ±H and about 26 in the
  horizon at Milky Way values; about 365 flares a year.
- **P09.T30.b Victims and luminosity.** Each disruption inside the source horizon has a victim, a
  feature-level member of class `TIDAL_DISRUPTION_VICTIM`, first in the centre's list order, on a
  near-parabolic orbit whose pericentre passage is the event time; its mass is drawn from the
  centre's living classes and its state after the event is "no system". `luminosity_at(t)` =
  quiescence + flares + the t^(−5⁄3) tails of the disruptions in the horizon, by looking back over
  their bins. Tests: a victim resolves from its ID and is at pericentre at the event time; 0.3–3 ×
  10³⁹ erg/s a thousand years after a disruption (Ponti et al. 2010); the Kepler members' loss cone
  stays empty.

#### P09.T31 Centre tests

Slow tests. Members sampled in six radial shells from 0.01 to 100 ly keep the profile at t = −1,000,
−100, +100 and +1,000 yr (Kolmogorov–Smirnov per shell, and the flattening unchanged);
eccentricities thermal (mean 0.6–0.72); the brightest-band members inside 0.04 ly include S2-like
orbits (periods of 10–20 yr occur); mean spacing about 0.05 ly at 3 ly.

### Phase 7: catalogue classes and the merge

#### P09.T32 The catalogue-class grid

`ClassProcess`: a trait with a class ID, a cell size (512 ly × 2ᵏ, low cell bits zero for coarser
classes), a bound over a cell, a candidate draw under a cap, and the class's test.
`CatalogueClassCell` generation by thinning as everywhere else; plan 01's `CatalogueSystemId` with
the 24-bit candidate index and a 4-bit member; `resolve` for the `111` prefix, which answers
`NoSuchSystem` for an unregistered class, for a cell that fails
`is_aligned_to(class.cell_log2_ly())` (plan 01 leaves that canonical rule to this resolve), for an
index beyond the count and for a rejected candidate; a lookup that visits the cells within radius +
4,096 ly (eight rings at 512 ly), which the lifetime cap of P09.T17 makes sufficient. Files:
`galaxy/catalogue_classes/{mod,ids,grid}.rs`. Tests: with a toy class, order independence,
resolution, canonical IDs, no cell above 2²⁴ candidates.

#### P09.T33 The core-collapse class

- **P09.T33.a Field deaths.** `CoreCollapseProcess` on 512 ly cells: sites follow what dies in the
  field, which is the young field with its (1 − φ(a)) age factor. Envelope = site density bound ×
  (the field cap + L + 2H) × the largest death rate per system; a candidate draws its site, accepts
  on density ÷ bound, draws initial mass and metallicity, accepts on the death rate at that mass,
  draws T uniformly over (−(cap + L + H), +H], sets its age at the epoch to lifetime − T, builds its
  `SystemRecord` with `from_parts` and its `SystemStars` on its own streams with
  `StarDraws::for_attempt` until the death kind is a core collapse (bounded at 64 attempts,
  debug-asserted), and is kept if and only if `claims(.., ShellEnvironment::Field)` on
  `DeathMarks::of`. Member 0 is the system: progenitor, supernova or remnant by evaluation time, at
  the site + velocity × (t − T) before the explosion and at site + (velocity + kick) × (t − T) after
  it. Tests: entries per galaxy 3,000–21,000 more than distinct shells; 20–160 core collapses in ±H
  over seeds and about 40 at two a century; every kept entry's window is under its cap; no entry
  travels more than 4,096 ly from its site within its lifetime.
- **P09.T33.b Runaway and walkaway deaths.** A second site density in the same class: plan 08's
  runaway and walkaway classes of band E, whose members die away from their birthplace, with the
  death rate conditional on the class's ejection-age marks. The grid side of these classes is carved
  in P09.T35. Tests: their share of field core collapses against the closed form from
  `RunawayModel`; complementarity for sampled runaway candidates.

#### P09.T34 The Type Ia class and the ancient survivors

- **P09.T34.a The class.** `TypeIaProcess` on the same grid: sites follow every population's layer-D
  budget density weighted by its Type Ia rate, candidates use `IaProgenitor::draw`, the window reads
  the smooth gas at the site under `ShellEnvironment::TypeIa`, and the entry has member 1 where
  P09.T18.c leaves a recent survivor. The survivor's speed is the entry's v_fast in `claims`, so its
  lifetime cap holds the survivor within 4,096 ly of the site and the lookup of P09.T32 needs no
  further rings. Tests: 8–20 Type Ia in ±H; a third or more of distinct shells are Type Ia; no
  survivor of a kept entry is more than 4,096 ly from its site at any time in the entry's lifetime.
- **P09.T34.b Ancient survivors.** Ancient hypervelocity survivors are plan 08's hypervelocity class
  (`DisplacedKind::HypervelocitySurvivor`), registered there with zero weight and plan 15's
  `HYPERVELOCITY` form (the old stars' density convolved with 1 ÷ (4π r² v), on straight lines);
  this subtask gives it its weight, the Type Ia rate × the surviving-donor share × the crossing
  time, some tens of thousands inside the cube. The class lives in layer D's cells and moves at up
  to 2,500 km/s, above plan 03's `PAD_SPEED`, so in this same subtask `query::pad_speed(Layer::D)`
  is raised to plan 08's `UNBOUND_PAD_SPEED` (3,000 km/s), as plan 08's design note 27 requires of
  whoever gives the class weight; layers A–C keep `PAD_SPEED`. Padding changes which cells a query
  visits and no generated output. The version bump is P09.T35's. Tests: survivors inside the cube
  10⁴–10⁵; none younger than the interval's start, where the entries take over; `pad_speed(D)` is
  `UNBOUND_PAD_SPEED`; at t = ±H a 50 ly query equals the brute-force enumeration for 100 centres
  chosen to have a survivor within 15 ly outside the sphere at the epoch; the count of layer-D cells
  visited at |t| = H rises by under a third.

#### P09.T43 The luminous blue variables

`LbvProcess` (`ClassId::LUMINOUS_BLUE_VARIABLE`), on the same grid: hosts are layer-E systems whose
window near the Humphreys–Davidson limit, plan 06's `SystemStars::lbv_window()`, overlaps the source
horizon −(H + L) to +H. Sites follow the young field as in P09.T33.a and the band-E runaways; a
candidate draws its initial mass and age from plan 15's `tables::lbv::LBV_SAMPLER` (the conditional
mass density and the age window per mass node; a scratch table from plan 06's tracks at five masses
until P15.T10.a), then redraws its star on its own streams with `StarDraws::for_attempt` until the
window overlaps. The class test, `lbv_claims`, is asked by plan 03's `catalogue_claims` beside
`recent_death_claims`, core collapse first, and a system that passes both is a core-collapse entry,
so the two classes stay disjoint. It prefilters on the record alone, initial mass above the table's
lowest mass node and age under its longest window's end, so that only a few layer-E candidates in
ten thousand pay for a stellar evaluation. Inside nurseries and young open clusters the class goes
on the feature-level list after `TYPE_IA`. Giant eruptions are plan 06's
`StarEventKind::GiantEruption` on the host's own streams; nothing is added to them here. The
carve-out lands with P09.T35's version bump. Files: `galaxy/catalogue_classes/lbv.rs`. Tests: over a
10⁶-star layer-E sample the class test holds every star with a window inside the horizon and nothing
else; complementarity as in P09.T38; hosts per galaxy recorded in a golden summary.

#### P09.T35 The carve-out on the grid side

Plan 03's `catalogue_claims` already computes `displaced::explosion_site` for every layer-E record,
field or displaced, whose death time falls within (−(`SHELL_WINDOW_CAP` + L + H), +H], backing a
displaced candidate out to its birth site along a straight line, and passes it to plan 08's stub
`recent_death_claims(galaxy, &site, &record)`. This task replaces the stub's body: it generates the
record's `SystemStars`, builds `DeathMarks::of` and returns
`claims(galaxy, &site, &marks, ShellEnvironment::Field)`. Only records that pass plan 08's prefilter
pay for a stellar evaluation. A claimed candidate resolves to "no such system". Runaway classes do
the same, and the luminous blue variables' test of P09.T43 is asked beside it. Layer D's share loses
`ancient_share`; recent Type Ia need no carve-out here because plan 11's binaries redraw any
explosion before +H, which this task records as a requirement in plan 11's consumed interface and
guards with a `debug_assert` hook. Bump `GENERATOR_VERSION`, regenerate goldens. Tests: the
complementarity test of P09.T38 for sampled candidates; a grid star due to die at +1,500 yr is still
a grid star, dies in place and has a shell when evaluated then.

#### P09.T36 The recently dead inside features and the centre

- **P09.T36.a Nurseries and open clusters.** Registers `CORE_COLLAPSE` and `TYPE_IA` on the
  feature-level lists (P09.T22). The candidates are members of band E whose death time, lifetime
  minus the member's age, falls in the interval; the expected count is a closed form in the
  feature's band-E count, its age distribution and the bubble's cap; the window is the hot branch in
  the feature's own superbubble (`ShellEnvironment::Bubble`), ended at the wall; the band-E cells of
  P09.T21 resolve a claimed candidate to "no such system" by the same `claims`, with the bubble
  passed down from the parent feature. Tests: four in five (0.7–0.9) core collapses of a sampled
  galaxy are in features, which hold about a quarter (0.15–0.35) of distinct shells; a chart finds
  every shell of an association from the catalogue without touching its nested grid (asserted with a
  counting cache); complementarity between a feature's band-E cells and its list over 10⁴ sampled
  members.
- **P09.T36.b Globulars and the centre.** Globular clusters have no core-collapse entries and no
  shells; their Type Ia hosts are drawn from the delay-time distribution on their mass (0.1–0.5
  expected over the whole system) and are listed as hosts whose window is zero. The centre's list
  takes a handful of core collapses from its young class and its Type Ia from its old classes, and
  the centre's band-D and band-E cells carve them by the same test, with the smooth gas at the
  origin as the site. Tests: a handful (1–20) of entries at the centre at Milky Way values; no
  globular entry ever reports a shell.

#### P09.T37 Merge into the range query

`CatalogueClassSource` beside `FeatureMemberSource` and `CentreMemberSource`, all registered with
plan 03's merge hook and ordered after the grid's layer of the same band. Each merged system counts
in the layer its initial mass gives it, each source's expected count inside the sphere is added to
that layer's expected count before the census decision, and `SystemOrigin` is set. Shells are
returned as attributes of their systems, and a system is returned when its present position (the
remnant's, after the explosion) is inside the sphere. Tests: census lines identical whatever is
cached; a query at the very centre reports that nothing fits at 50 ly and a complete census at 0.05
ly; bench: the brainstorm's 50 ly query at Sun-like density stays under 5 ms cold with all sources
registered.

#### P09.T38 Complementarity, budgets and the Milky Way's shells

Slow tests. **Complementarity:** for 10⁵ sampled mark sets around the interval's ends and across the
clock window, exactly one of the cell and the catalogue claims the system, at evaluation times
before and after the death; the same for displaced and runaway candidates, which back out their site
first, inside features, and for the luminous blue variables' test. "Claims" on the catalogue side
means that the marks, handed to the class's own acceptance, are kept: both sides call the one
`claims`, and the test's point is that nothing else (a prefilter, a cap, a clamp) disagrees.
**Budgets, part two:** sampled counts of field plus members plus catalogue systems match each
population's budget within Poisson error in twelve test volumes. **Shells at Milky Way rates**
(about two core collapses and half a Type Ia a century): 3,000–30,000 distinct shells over seeds and
about 7,000 at those rates; one to two thousand non-radiative; counts by phase recorded in a golden
summary; sizes 10–800 ly; the longest window under `SHELL_WINDOW_CAP`, with the longest seen
recorded against the brainstorm's 2–4 Myr and the 4.3 Myr of the oldest known remnant.

### Phase 8: protocol, server and display

#### P09.T39 Protocol messages

In `hyperion-protocol`, as new kinds of plan 04's `RequestBody` and `ResponseBody` with their
strings added to `REQUEST_KINDS`; feature IDs travel as `FeatureIdHex`, 16 lower-case hex digits
like `SystemIdHex`. By plan 04's design note 15 a new request kind or an optional field does not
bump `PROTOCOL_VERSION`, so the fields added to the systems-within-range reply below are optional on
the wire. `FeaturesInRange { universe, centre, radius_ly, time, kinds }` → `FeatureSummary` list (ID
as 16 hex digits, kind, position, extent, age, mass, expected members, emission class, bubble
radius) with a per-kind census; `GalaxyFeatures { universe, kinds, time }` for the map (globulars,
the centre, bright shells; complete per kind or nothing, with the reason);
`FeatureDetail { feature }` → structure, class counts as `ClassCountWire`, black-hole count, core
collapse flag; `BlackHoleState { universe, time }` → mass, luminosity, recent disruptions. The
systems-within-range reply gains `origin: SystemOriginWire`, `member_class`, and for catalogue
systems `supernova: SupernovaStateWire` with a `ShellSummary` (radius, phase, emission, remnant
offset, nebula). `just gen-protocol`. Tests: wire-form tests per message, as the crate already has
for `hello`.

#### P09.T40 Server handlers and caches

- **P09.T40.a Caches and sources.** `FeatureCellCache`, `ClusterModelCache`, `NestedCellCache`,
  `CentreOrbitCache`, `CatalogueClassCellCache` over plan 04's `ByteLru`, holding epoch state only,
  with interior mutability because `SystemSource` takes `&self`. The `SystemsInRange` handler
  registers `FeatureMemberSource`, `CentreMemberSource` and `CatalogueClassSource`, and passes
  `FeatureGas` wherever it passed `NoModifiers`. Tests: eviction never changes a reply; a range
  query over the WebSocket inside a globular returns members with their origin.
- **P09.T40.b Handlers.** `FeaturesInRange`, `FeatureDetail` and `BlackHoleState` on the CPU pool;
  `GalaxyFeatures` computed in the background and cached per universe, like the density map. Tests:
  integration tests over the WebSocket for each message, including an unknown feature ID and a
  radius that fits nothing.

#### P09.T41 Client: feature marks and shells

- **P09.T41.a The guide and the marks.** `docs/frontend/ux-guidelines.md` gains a fixed symbol per
  feature kind (shape for type, colour kept free for status), a circle at true radius for a remnant
  shell and for a superbubble (a sphere is a circle in the orthographic view, plan 05's
  `SphereMark`), and the emission-class abbreviations. The local chart draws `FeatureMark` and
  `ShellMark` through the general spatial view. Projection and picking stay pure functions with unit
  tests.
- **P09.T41.b The galaxy map's overlay.** A feature overlay (globulars, the centre, bright shells)
  from `GalaxyFeatures`, with a legend, a per-kind toggle, and a stated "pending" while the server
  still computes it. Component tests for the toggle and the pending state.
- **P09.T41.c Lists and readouts.** The chart lists features in a second list with the same keyboard
  selection, and `FeatureReadout` shows designation, kind, distance, extent, age, mass, member count
  and emission class. A selected system's readout gains its origin, member class and supernova
  state. Component tests for the lists and readouts.

#### P09.T42 Client: the centre's fine radius steps

The query-radius selector and the scale bar run down in 1-2-5 steps to 0.01 ly and 0.001 ly, the
readout gives distances in light-years to four decimals or in AU below 0.01 ly, and the census line
is shown at every step. A `BLACK HOLE` readout shows mass and luminosity at the chart's time. Tests:
selector steps, unit switching, formatting under the guide's number rules.

## Verification

- `just ci` green after every task; `just test-slow` holds P09.T3.c, T6, T11, T14, T15.b, T31, T38
  and the 10⁶-star sample of T43.
- The brainstorm's Testing lines in scope map as follows: complementarity and the window cap
  (P09.T15.b, T38); the nuclear cluster's profile at ±1,000 years (P09.T31); cluster retention in
  the kick-law test (P09.T11); the globular system's mass function, sizes and orbits (P09.T14);
  shell counts by phase and the rates of events (P09.T33.a, T34.a, T38); S2-like orbits (P09.T28.a,
  T28.d, T31); state continuous in time and both event constructions in any order (P09.T19.a,
  T30.a); budgets (P09.T6, T38); bound checks (P09.T3.a, T3.b, T8.b, T9.g, T21); order independence
  and goldens (P09.T3.b, T21, T27, T32), which also run on the second CI architecture of P01.T12,
  where the Kepler propagator and the Eddington inversion are the pieces most exposed to a platform
  difference.
- Benchmarks under `just bench`, targets not promises: feature cell under 10 ms; globular walk under
  0.3 s; `CentreModel` build under 100 ms; 50 ly query in a globular core under 20 ms cold; century
  query at the centre about 10 ms with cached orbits; the Sun-like 50 ly query still under 5 ms cold
  with every source registered.
- By eye in the `GALAXY` display: globulars concentrated to the centre, nurseries tracing the arms
  on the young map, shells larger above the plane, the chart at 0.3 ly from the black hole.

## Generator version

This plan changes generated output four times, each a bump with regenerated goldens: P09.T2.c (the
field gives up φ), P09.T28.b (the Kepler regime and the grid's loss cone), P09.T28.d (the secular
rates, away from the epoch only), P09.T35 (the supernova and luminous-blue-variable carve-outs,
layer D's ancient share and the hypervelocity class's weight). It reserves, so that later plans move
nothing: the `ClassId` values 2, 3, 5, 6 and 8 for plan 11 and the rule that feature-level lists
append classes in the list order of design note 16; event tags 0x0203–0x02FF of this plan's block;
the `10` sub-kinds `01` and `10` for plan 10 (`11` stays rejected);
`FeatureKind::{Stream, DwarfCore}`; `FeatureShares::set_halo_discrete`; the binary classes of the
class table with scratch fractions; member 1–15 of a catalogue system; domain-tag prefixes
`feature.`, `member.`, `centre.`, `snr.`, `class.`. Parameters that belong to the generator version
and are named as such in code: `SHELL_WINDOW_CAP`, the explosion energy's log-normal, the height
proposal's scale rule, β, σ and the smoothing scale of the window; the bound fraction, embedded
duration, dissolution ages and association mass floor; the Type Ia channel shares; the black-hole
loss constants; light-curve templates; cloud statistics; the nuclear cluster's mass scaling.

## Risks and open points

- **Updated for the 2026-09-21 density rulings.** The default mass function is now Chabrier's system
  function, with about 0.87 times Kroupa's systems per solar mass, so the centre's figures are
  scaled: about 7,800 systems per cubic light-year at 3 ly (T24.a), 1,400 candidates in the fullest
  cell and band and eighty in the innermost (T27), and 37,000 systems in r_full at a century (T29).
  The cluster's 4–5 × 10⁷ members and every conclusion about the 8,192 index still hold.
- **Where the shell window lives.** The brainstorm's order of attack puts "the shell test" with
  kicks and displaced objects, which is plan 08, while this plan's scope holds the supernova section
  in full. This plan owns `snr` and `claims`; if plan 08 has already built the window, P09.T15
  adopts it and P09.T35 shrinks to the feature side.
- **Association lifetimes.** The features table says associations are "under about 30 Myr", the φ
  rule says φ falls to the bound fraction "by 30–100 Myr". Resolved as dissolution ages on 30–100
  Myr, which is what four in five core collapses inside features requires; "under 30 Myr" is read as
  the age at which an association still has O stars. The association count then tends to the top of
  "tens of thousands". The two sentences of the brainstorm do not agree as written (an association
  "under about 30 Myr" cannot hold the stars that keep φ above the bound fraction until 100 Myr, nor
  reach 300 ly at a few km/s), and the brainstorm should settle which it means. If it settles on 30
  Myr, only G(a) and the expansion speeds change, by a version bump.
- **The feature index.** With a uniform proposal the 14-bit index would be at risk (about 7,400
  cluster candidates in the densest cell at Milky Way values, before nurseries and a galaxy three
  times as heavy), so design note 21 proposes disc features in height from the start, which leaves a
  margin of about ten. P09.T3.b asserts the headroom for every process together. If it still fails
  for some seed, the next lever is the arm factor in the proposal; the ID layout is the brainstorm's
  and does not change.
- **The centre's feature-level index** is 13 bits. The supernova classes, the variables and the
  disruption victims need tens. Plan 11's accreting white dwarfs, at the galaxy's 6–12 × 10⁶ per
  10¹¹ systems, come to 2,400–6,000 among the centre's 4–5 × 10⁷ members before any dynamical
  enhancement, which is most of 8,192; P09.T22's assertion will catch an overflow, and plan 11
  checks it in P11.T8.e. Plan 11 proposes widening the index into the level and cell bits under the
  spare band value. That contradicts the brainstorm's "under it the level and cell are zero" and
  plan 01's canonical rule, so it needs the brainstorm revised first; the alternative within the
  brainstorm is to list at feature level only the centre's hosts above a brightness floor. Reported
  to the brainstorm's owner.
- **The nuclear cluster's budget** is not in the brainstorm's list of populations (design note 5).
  Plan 02 draws its mass for the potential; this plan treats it as a budget of its own, about 0.05%
  on top of the drawn stellar mass. The brainstorm should say so or name the population it comes out
  of.
- **Globulars and the halo's discrete share** (design note 4): the brainstorm lists only streams and
  dwarf cores under "discrete", and its mixture table has no row for living clusters. Globular
  members are taken from their origin population's budget on top of the discrete share, which for
  the halo is a few per cent. The brainstorm should confirm.
- **Members as `SystemRecord`s** (design note 20). Plan 03's record carries a `SystemOrigin` from
  M1, so a member needs no placeholder component. Every consumer that reads a component's laws (plan
  06's `draw_metallicity`, plan 08's `draw_velocity`) must be bypassed for members, and P09.T10 and
  P09.T37 test that neither is reached with a record whose `component()` is `None`.
- **`SHELL_WINDOW_CAP`** (design note 22). An earlier draft of P09.T38 allowed windows to 4.6 Myr
  while plan 08's prefilter stops at 4 Myr; a window the prefilter cannot see would break
  complementarity silently. The constant is now the single ceiling and P09.T15.b tests it.
- **The luminous blue variables' owner.** Plans 06 and 15 assign the class to this plan and plan 11
  builds only the four binary classes, so P09.T43 was added. The brainstorm gives the class no
  count; the golden summary records what the tracks yield.
- **Neighbouring plans.** Plans 01–08 and 15 already provide what this plan needs: the ID layouts,
  the `FeatureShare` and `catalogue_claims` hooks, `StarDraws::for_attempt`, the helium excess,
  `GasModifierSource`, the explosion-site hook and the zero-weight hypervelocity class. The mass
  formed per system is plan 02's `Galaxy::mean_formed_mass` (P02.T4), which is not
  `Galaxy::mean_system_mass` (the present-day mass) but the same quadrature without deaths.
- **Kepler propagation against the true potential.** The orbit is about the mass inside the epoch
  radius and the precession from the enclosed stars is added on top. P09.T28.a's comparison with a
  direct integration decides whether the second rate needs a correction beyond 0.3 ly.
- **Grid systems inside the influence radius** are not drawn from the distribution function, so
  their density is not exactly stationary. They are about one in a hundred of the systems there and
  the effect is confined to the inner 1.5 ly over the clock window.
- **Scratch constants** (black-hole loss, equipartition, pulsar counts, Type Ia samplers, helium)
  ship as defaults and change with plan 15's tables, each a version bump.
