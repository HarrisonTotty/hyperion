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
  (`galaxy/catalogue_classes/registry.rs`, re-exported by its `mod.rs`) is the one place a value is
  allocated: values 0–8 are taken, 9 upward are free, and a test asserts that no two names share a
  value. `ClassId::cell_log2_ly() -> Option<u32>` gives the class's catalogue cell size (`None` for
  `TIDAL_DISRUPTION_VICTIM`, which is never placed), which `resolve` checks with plan 01's
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

- `features::shares::FeatureShares`, held by `Galaxy` (`Galaxy::feature_shares()`) and reached by
  plan 02's fields through new variants of its `FeatureShare` enum, which plan 02 put in
  `galaxy::ages` (consumed by `AgeDistribution::young_disc`): `phi(population, band) -> f64`,
  `phi_young(age: Years) -> f64`, `phi_sub_disc(SubDisc) -> f64`,
  `field_factor(population, band) -> f64`, `set_halo_discrete(phi: f64)` for plan 10. `phi` and `field_factor` take plan 02's `MassBand` and
  are defined for every value it will ever have: for a band below the five stellar ones (plan 13
  adds the brown-dwarf and rogue-planet bands) they return band A's value, through one private
  `stellar_band_or_a` mapping, so plan 13 changes nothing here. Plan 11's P11.T7 adds one more
  setter to this type, `set_class_shares(&ClassShareTable)`, folded into `field_factor` beside the
  Type Ia ancient share; it is plan 11's code in this plan's file `galaxy/features/shares.rs`.
  `features::shares::NurseryRates`.
- `features::{FeatureProcess, FeatureKind}` (in `features::ids`, re-exported) and
  `features::catalogue::{FeatureRecord, FeatureMarks, FeatureCatalogue, FeatureCellContents,
FeatureCellCache, NoFeatureCache}` with
  `FeatureCatalogue::cell(&Galaxy, FeatureCell) -> FeatureCellContents`,
  `resolve(&Galaxy, FeatureId) -> Option<FeatureRecord>`,
  `near(&Galaxy, &GalacticPosition, LightYears, &dyn FeatureCellCache) -> impl Iterator<Item =
FeatureRecord>`, `walk_process(&Galaxy, FeatureProcess) -> impl Iterator<Item = FeatureRecord>`
  (plan 10 walks the globulars with it), `density`, `expected_candidates`, `counts`,
  `MAX_FEATURE_REACH: LightYears = 4,096`. `FeatureCellCache::contents(&self, &Galaxy, FeatureCell)
-> Arc<FeatureCellContents>` takes `&self` and keys by galaxy (ruling 25).
  `FeatureRecord::kind_at(t) -> Option<FeatureKind>` and `reach()`;
  `catalogue::bulk_velocity(&Galaxy, &FeatureRecord) -> Option<GalacticVelocity>`.
- `features::kinds::open_cluster::OpenClusterMarks`,
  `features::kinds::nursery::{NurseryMarks, NurseryStage, Superbubble}`,
  `features::kinds::cloud::CloudMarks`; `features::kinds::globular::GlobularMarks` arrives with
  P09.T12, which first draws globulars.
- `features::emission::EmissionClass`, with variants `HiiRegion`, `ReflectionNebula`, `DarkCloud`,
  `RemnantShell` and `PulsarWindNebula`, and `EmissionClass::of(&Galaxy, &FeatureRecord, t)` (the
  galaxy for its mass function).
- `features::gas_overlay::FeatureGas::new(&Galaxy, &dyn FeatureCellCache, t)`, an implementation of
  plan 07's `GasModifierSource` that turns superbubbles into `GasModifier::Hole` and clouds and
  star-forming regions into `GasModifier::Cloud`.

### Interiors, nested grids and members

- `features::cluster::ClusterModel` (`from_record`, and `new(&Galaxy, &ClusterParameters)` for a
  cluster from its parameters), with `escape_speed_central()`,
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
  `features::members::sphere_of_influence(galaxy, &FeatureRecord, &ClusterModel, &MemberRecord) ->
Metres` (as built: the member needs its cluster's model) is the smaller-radius
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

- **Plan 01:** `math`, `rng::{Seed, Stream, DomainTag, TagScope, ObjectKey, tags}` with
  `Stream::open(Seed, DomainTag, ObjectKey)` (the tag by value), the `domain_tags!` registry in `rng/tags.rs` and the
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
- **Plan 02:** `Galaxy` (`Galaxy::from_params` returns a `Result`; the fixture is
  `GalaxyParams::milky_way_like()`), `params::GalaxyParams` (dark halo, `AccretionHistory` with the last major
  merger, the progenitors and the globular count, the black hole's mass, `NuclearClusterParams`),
  `imf::{MassFunction, BandShares}`, the mean mass per system (`Galaxy::mean_system_mass`) and the
  mass formed per system (`Galaxy::mean_formed_mass`), `potential::PotentialTables` (`v_circ`,
  `omega`, `kappa`, `potential(r, z) -> Option`, `escape_speed(r, z) -> Option`,
  `tidal_radius(m, &PointLy) -> Metres`), the nuclear cluster's `BrokenPowerLaw`, `fields` (population
  densities, sub-discs, age distributions, metallicity) with the `FeatureShare` hook of
  `galaxy::ages` held at φ = 0, `bounds`, `ShareMatrix`, `map`. There is no formation-rate
  function: a population's rate is its count times its age distribution's density times the mass
  formed per system, as `NurseryRates` computes it.
- **Plan 03:** `placement::{SystemRecord, resolve, ResolveSystemError}` with
  `SystemRecord::from_parts` for non-grid sources, `placement::SystemOrigin` (`#[non_exhaustive]`,
  extended here) and `KindNotGenerated`, which this plan's dispatch replaces for the `0`,
  `10`-centre and `111` kinds, candidate streams, the private carve-out hook `catalogue_claims`,
  `query::{RangeQuery, RangeResult, SystemHit, Census}`, `QuerySphere`, `LayerCounts`, the merge
  hook `SystemSource` (`expected_in_sphere`, `systems_in_sphere`, `suppresses`; it takes `&self`, so
  a source holds a reference to the caller's caches and the caches use interior mutability), the
  drift hook `epoch_velocity` and `position_at`, the padding rule (`PAD_SPEED`, `pad_for`). The
  record keeps its 80-byte cap (ruling 20): `SystemOrigin::FeatureMember`'s payload packs into at
  most 7 bytes at 4-byte alignment, a `u32` index into the catalogue plus up to three bytes of role
  or member data, since a `FeatureRef` is 16 bytes in memory; only a payload that cannot pack
  raises the cap to 88, with plan 04's cache figures re-measured. `range_query` already sums
  sources' counts in value order and asserts their IDs disjoint (ruling 23). Plan 08's
  `kinematics::draw` reads `let SystemOrigin::Grid(component) = record.origin()` irrefutably;
  the first task that adds a variant must make that a match.
- **Plan 04:** request IDs, the universe registry, the CPU pool, byte-bounded LRU caches, the
  TypeScript request layer. **Plan 05:** the general spatial view, the galaxy map, the `GALAXY`
  display's selectors and readout, the UX guide.
- **Plan 06:** stellar evaluation as a continuous function of age plus time;
  `lifetime(m0, &Composition, &StarDraws)`; `turn_off_mass(age, &Composition)`;
  `StarDraws::for_attempt(seed, star, attempt)` for conditional redraws;
  `Composition::from_fe_h(fe_h, helium_excess)`; the remnant outcome, built as `Death` (with
  `DeathKind`, `SupernovaType` and `ProgenitorAtDeath`) and `CompactRemnant`;
  `stellar::remnant::KickLaw` and `StandardKickLaw`; neutron-star spin-down, built as
  `NeutronStar::state_at` and `PulsarState`; the tracks cover 0.1–100 M☉;
  the event constructions `PoissonBins` and `MonotonePhase`.
- **Plan 07:** `GasField::state(position, SmoothingScale, &mut NoiseCache) -> GasState` (density,
  pressure, temperature, and the sound speed split into `thermal_sound_speed` and
  `isothermal_sound_speed`, the shell window's per ruling 98; the parcel's four phases of ruling
  103), the corona's pressure floor (`GasField::params().pressure_floor()`),
  `GasField::neutral_bound`, `GasModifier`, `GasModifierSource::modifiers_near_segment(a, b, out)`
  and the `NoModifiers` source this plan replaces. Plan 07's integrators take the modifiers as a
  `&[GasModifier]`, which the caller fills from a source; the server calls none of them yet.
- **Plan 08:** velocity laws of each population and halo component behind `epoch_velocity`
  (`KinematicTables::ellipsoid`; `draw_velocity` and `epoch_velocity` take a `SystemRecord`, so a
  feature draws its bulk motion through the crate-private `kinematics::draw_on`, which P09.T4.a
  added); `kick_bins::speed_bin_shares` (the kick distribution by remnant
  kind and mode); `displaced::{explosion_site, ExplosionSite}` and the stub
  `recent_death_claims(galaxy, &site, &record) -> bool`, whose body this plan supplies, with
  `SHELL_WINDOW_CAP` (4 Myr there), which this plan takes over; `displaced::ClassTable` with the
  split between budget-fed `class_weight` and field-fed `stay_share`; the zero-weight hypervelocity
  class (`DisplacedKind::HypervelocitySurvivor`); `runaway::RunawayModel`;
  `SystemRecord::{placement_class, formation_site, kick_constraint, mark_attempt}`;
  `pad_speed(Layer)` with `UNBOUND_PAD_SPEED`. At `d2787a2` only P08.T1–T7 are built: `kick_bins`,
  `site`, `class_table` and `runaway` are stubs, and `recent_death_claims`, `SHELL_WINDOW_CAP` and
  the record's `placement_class`, `formation_site`, `kick_constraint` and `mark_attempt` do not
  exist (the `PlacementClass` type does).
- **Plan 15:** `tables::cluster_dynamics` (`BH_LOSS_BETA`, `BH_LOSS_PSI_SLOPE`, `BH_CLOCK_FACTOR`
  (ruling 126.4), `BH_RELAXATION_PREFACTOR`, `EQUIPARTITION_EXPONENT`, `PULSARS_AT_47_TUC_GAMMA`,
  `PULSAR_GAMMA_EXPONENT`, `PULSAR_CORE_COLLAPSE_CAP`), `tables::type_ia_delay` (`DELAY_EDGES`,
  `YIELD_PER_SOLAR_MASS`, `CHANNEL_SHARE`, the two mass CDFs, `LAYER_SHARE`,
  `ANCIENT_LOSS_PER_SOLAR_MASS`; the explosion mark is plan 11's `tables::binary::IA_YIELD`),
  `tables::lbv::LBV_SAMPLER`, and plan 06's `tables::helium`, which this plan reaches only through
  `Composition`'s helium excess. Plan 15 moves this plan's scratch constants into its tables
  unchanged first (P15.T8, T9.a, T10.a), so no task here blocks on a fit; if this plan runs before
  those tasks, it commits the constants under the same names in the same modules, marked
  provisional. At `d2787a2` none of these tables exists.

## Design notes

Each note is a decision the brainstorm leaves open. None contradicts it.

1. **One young sequence.** Star-forming regions, OB associations and young bound clusters are one
   Poisson process of nurseries on the young disc, born at the young disc's formation rate on a mass
   function falling as M⁻² over 10²–10⁵ M☉. A nursery is a star-forming region while embedded (the
   first 3–5 Myr), then by an independent mark a bound open cluster (the bound fraction, 10–15%) or
   an unbound association that dissolves at an age drawn on 30–100 Myr. The kind is therefore a
   function of the evaluation time, as a star's state is. Reason: the three kinds' counts then agree
   with each other and with φ(age) by construction: about 3,000 nurseries per Myr gives 10⁴
   star-forming regions, 300–480 bound clusters per Myr and, with the association mass floor as a
   parameter of the generator version, 1.2–2.1 × 10⁵ associations (ruling 140.2–140.3, amending
   the earlier 2,100 nurseries, 240–360 clusters and "tens of thousands", derived at plan 02's
   scratch Chabrier scale 0.68: the Galaxy's 1.46–2.3 M☉ a year, Kroupa-normalised from
   massive-star tracers by Licquia and Newman 2015 and Chomiuk and Povich 2011, put in the default
   mass function's own terms at the fitted 0.92, × 1.26–1.29, at the bound fraction 0.125; the
   associations are that rate × (1 − Γ_b) × their mean stage of about 61 Myr). An association's
   size is its expansion speed × age, 300 ly at 100 Myr and 3 km/s. The brainstorm lists the three
   as rows of one table with counts of their own and says only that "the catalogue splits by
   marking"; one process split by marks is that, and three independent processes could not keep their counts,
   φ(age) and the four-in-five rule consistent. The association's age limit is discussed under
   Risks.
2. **φ(age) is derived, not drawn.** φ(a) = f_n × [Γ_b m_b(a) + (1 − Γ_b)(1 − G(a))], where f_n ≈
   0.9 is the share of star formation in nurseries, Γ_b the bound fraction, m_b(a) the mean
   surviving mass fraction of bound clusters from Lamers's closed form, and G the distribution of
   association dissolution ages. G is uniform on 30–100 Myr, so that 80–90% of core collapses fall
   inside features (ruling 118.2; Higdon and Lingenfelter 2005: 80% by time, 90% by space), which a
   test pins.
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
   (4–6 × 10⁷ systems "more", ruling 144.4) are given separately; carving it out of the nuclear disc instead would
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
    bound. _Revised by ruling 144.5a (lane `centre09b`):_ the bound is the nearest corner's
    profile times the angular factor's greatest value, `e^(−k/2) I₀(k/2) ÷ A` (×0.69 for the old
    stars), I₀ once per class. Under it the inclination cannot be a rejection after the velocity,
    so a candidate is thinned by its position's own angular factor (an I₀ per candidate and
    flattened class) and the inclination mark draws the velocity's direction conditioned on it.
14. **Dark remnants at the centre get distribution functions of their own**, one Eddington inversion
    per distinct profile (stars, black holes, the young inner disc), all in the same potential. The
    young disc has no inner hole, because a hole has no isotropic equilibrium. _Revised by ruling
    144.6:_ four inversions, the young split into the clockwise disc and the isotropic young; the
    disc's inner edge at 0.1 ly is its distribution function's energy cut, as the stars' r^−½ core
    is, which leaves an r^−½ tail inside it and is an equilibrium.
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
    1,000 ly of the centre. As built, the process follows `ε n_n + 9 n_mol`: a share ε = 0.15 of the
    smooth neutral layer (provisional; about 10⁹ M☉ of clouds at Milky Way values, the molecular
    mass Miville-Deschênes et al. 2017 find in their clouds) plus nine times the molecular disc, so
    `w = 9 ÷ ε`.
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
    the ratio's greatest value over the cell's heights. For the clouds, which follow the gas, that
    is the ratio at the height nearest the plane, because every gas layer is exponential in height
    (ruling 2 of 2026-09-22) and none is taller than H. The stellar discs are cored in height (the
    density rulings of 2026-09-21), so the ratio first rises with height and the greatest value
    lies above the plane; it is taken exactly from the vertical profile's table, whose exponent is
    linear between knots (`VerticalProfile::max_ratio_to_exponential`). That is thinning with a
    non-uniform proposal, exact by the same theorem. As built, x and y are proposed from 256
    columns of 256 ly, each with its own bound, and the molecular disc's clouds exponentially in
    |x| and |y| as well (P09.T3, as built, in Risks): a whole cell's bound alone wasted nine
    candidates in ten on the young disc's sharp arms. Globulars keep the uniform proposal.
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
`cargo test -p hyperion-sim -- features::ids catalogue_classes::registry` passes.

#### P09.T2 Feature rates and the φ table

- **P09.T2.a Rates and lifetimes.** `NurseryRates::from_galaxy`: nursery birth rate from the young
  disc's formation rate, the mass function, the bound fraction, the embedded duration, G(a).
  `open_cluster::dissolution_time(m0) = 1.3 Gyr × (m0 ÷ 10⁴ M☉)^0.62` and `present_mass(m0, age)`
  from Lamers et al. (2005), and `surviving_mass_fraction(age)` = m_b(a) by a fixed quadrature over
  the mass function. Files: `features/shares.rs`, `features/kinds/open_cluster.rs`. Tests: 300–480
  bound clusters born per Myr (ruling 140.2; 373 at version 15) and about 6 × 10⁴ alive at Milky Way parameters (5–8 × 10⁴), about
  half under 100 Myr (0.45–0.60), a few per cent over 1 Gyr (0.04–0.12), mean life 165–205 Myr
  (ruling 118.1: Lamers et al. 2005's 1.3 Gyr is the total disruption time; the earlier 295 Myr
  was 1.3 Gyr ÷ 0.62, and "about 10⁵ … a third" followed from it; Cantat-Gaudin et al. 2020's local
  catalogue has a median age of 132 Myr and 40% under 100 Myr).
- **P09.T2.b `FeatureShares`.** φ for the young disc as a function of age (design note 2), one
  constant per old sub-disc, the globulars' φ for bulge, thick disc and halo from the expected
  number × mean mass of the evolved Schechter function, per band where the class tables of phase 2
  deplete a band (until then uniform across bands, with a `TODO(P09.T9)` removed in that task).
  Tests: φ_young(3 Myr) within 0.85–0.95, φ_young(100 Myr) within 0.05–0.15, monotone falling.
- **P09.T2.c Apply to the field.** Through new variants of plan 02's `FeatureShare`, `fields`
  multiplies each population's density by `field_factor`, plan 08's `stay_share` takes the same
  factor while its budget-fed class weights do not, the young field's age distribution carries (1 −
  φ(a)) and is renormalised, and the share matrix takes the per-band factor. `map` keeps the budget.
  Bump `GENERATOR_VERSION`, regenerate goldens. Files: `galaxy/ages.rs` (`FeatureShare`'s new
  variant and `AgeDistribution::young_disc`), `galaxy/fields/*`, `galaxy/params/derive.rs` (the
  other caller of `young_disc`), `galaxy/placement/*`, plan 08's `displaced/class_table.rs`,
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
  star-forming regions, associations within 1.2–2.1 × 10⁵ (ruling 140.3; 158,664 at version 15),
  clouds in the thousands, each within
  Poisson error of the process's own expected count. Criterion benches: one inner-disc cell (target
  under 10 ms), the full globular walk (target under 0.3 s).

#### P09.T4 Marks of each kind

Parallel with each other after P09.T3.b.

- **P09.T4.a Open clusters.** Age (young: from the nursery; old: ∝ formation history × survival
  within the sub-disc), initial mass conditional on being alive at that age, present mass, half-
  mass radius (11.08 ly × (M ÷ 10⁴ M☉)^0.242 with 0.25 dex, Brown and Gnedin 2021's LEGUS fit × 4/3;
  ruling 126.6), concentration, metallicity
  from the population's law at the position and age, bulk velocity from the population's velocity
  law on the feature's stream. Tests: present-mass function against the closed form; every cluster's
  age below its dissolution time.
- **P09.T4.b Nurseries: star-forming regions, associations and superbubbles.** `NurseryMarks` (mass,
  bound mark, dissolution age, expansion speed 1–5 km/s, age spread up to 3 Myr, gas mass while
  embedded), `NurseryStage::at(age + t)`, size = expansion speed × age capped at 300 ly.
  `Superbubble`: radius 0.76 × (L_w t³ ÷ ρ)^⅕ (Weaver et al. 1977) with the wind and supernova power
  from the expected count of stars of 8 M☉ or more (winds 10⁵⁰ erg per star over the nursery's
  first 4 Myr, Krause et al. 2013 citing Voss et al. 2009; supernovae 10⁵¹ erg each, ruling
  118.4) and the smoothed gas density at the site, capped at blow-out (2.5 gas scale heights at
  that radius; the cap's source is still owed), interior density log-normal about 0.005 cm⁻³ at
  10⁶·² K. Members of an embedded region draw ages from −H. Tests: sizes 10–300 ly; 80% of bubble
  radii in 100–1,000 ly and every one at or below both the blow-out cap and 3,300 ly
  (McClure-Griffiths et al. 2002: 40 pc to 1 kpc); stage transitions continuous in time.
- **P09.T4.c Molecular clouds and dark nebulae.** `CloudMarks` per design note 19, a Plummer-like
  gas profile, mean density 10²–10⁶ cm⁻³ towards the core, dust by the local dust-to-gas ratio. The
  process's density carries the molecular weight w of design note 19; radii follow Roman-Duval et
  al. 2010's `M = 228 R^2.36` (ruling 118.3). Tests: sizes 30–400 ly (as built 25–400 across,
  median 40–60), count in the thousands; the expected mass of the molecular term's clouds inside
  1,000 ly of the centre is nine times plan 07's `MolecularDisc` mass inside the same sphere
  (ruling 118.5: design note 19's ratio holds point by point), and at Milky Way parameters clouds
  plus smooth disc there hold
  2–5 × 10⁷ M☉; over 200 seeds the realised cloud mass there is Poisson-consistent with the
  expectation; with w forced to 1 in a test build the same region holds under a tenth of that (the
  defect this guards against).
- **P09.T4.d Emission classes.** `EmissionClass::of(record, t)` per design note 17; dark cloud for
  clouds; remnant shell and pulsar wind nebula come from phase 4. Tests: table-driven, and a
  nursery's expected ionising stars at 1 Myr 3.0–4.8 per 10³ M☉ formed (ruling 140.4): about one
  primary above 15 M☉ (O9.5V and earlier, Martins et al. 2005) per 270 M☉ formed with companions,
  against 210 under Kroupa's function counting every star and 330 under Salpeter's.

#### P09.T5 Features in the gas field

`FeatureGas` implements plan 07's `GasModifierSource`: for a segment it looks up the features near
it and yields a `GasModifier::Hole` for each superbubble (radius and interior density from P09.T4.b)
and a `GasModifier::Cloud` for each cloud and embedded region (Plummer core radius, central density,
dust per hydrogen). Plan 07 integrates them; the server passes `FeatureGas` where it passed
`NoModifiers` (at `d2787a2` it has no such call site: plan 07's sight lines are not served, so the
server's part waits for the first handler that serves them). The shell window of phase 4 does not use modifiers: it reads the smooth field only.
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
10⁴ M☉ at P09.T4.a's median radius have central escape speeds of 0.92, 2.2 and 5.3 km/s to 10%
(ruling 126.6).

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
  final mass. Removes the `TODO` in `FeatureShares`. Test: at 47 Tucanae's slope (−0.65) band A,
  0.08–0.5 M☉, holds 0.386 ± 0.01 of the canonical count (ruling 126.8: "a third" was the factor
  below 0.2 M☉).
- **P09.T9.b Neutron-star and white-dwarf retention.** Retained fraction = the kick law's
  distribution below the effective birth escape speed (design note 8), by a fixed quadrature over
  plan 06's `KickLaw` through plan 08's `kick_bins::speed_bin_shares`: ordinary mode on the star's
  own speed, low mode on the pair's systemic speed, a Maxwellian of σ = 12 km/s (a Be/X-ray
  binary's, van den Heuvel et al. 2000; ruling 126.3). White dwarfs' 1 km/s kick applies to open
  clusters. Tests (ruling 126.3, amending 96.3 and 106.4 for clusters): 18–26% at 100 km/s, 15–25%
  at 50, 5–17% at 20, under 1% for a 10⁴ M☉ open cluster; 1,000–5,000 neutron stars in 47 Tucanae,
  170–600 in M4 (ruling 140.5: 126.3's 100–350 at w 0.181 and the scratch scale 0.68, carried to
  w 0.2675 and the fitted 0.9201; 396 at version 15, against Ye et al. 2019's 150–225, a tension
  on w per primary re-checked at P11.T6/T11), under one expected in Palomar 5; no cluster's pulsars outnumber its neutron stars.
- **P09.T9.c Black holes.** Retained at birth from the kick law with complete fallback unkicked
  (about four fifths). Mass fraction today f(t) = [(1 + ψ₁ f₀) e^(−β ψ₁ k t ÷ t★) − 1] ÷ ψ₁, floored
  at zero, with the clock factor k = `BH_CLOCK_FACTOR` = 2.5 for the cluster's denser past (ruling
  126.4; scratch, fitted by P15.T8.a with the rest), f₀ = 0.06 × retention (Breen and Heggie 2013;
  Antonini and Gieles 2020). It is the solution, at constant mass and radius, of df ÷ dt = −β (1 + ψ₁ f) ÷ t★: black-hole mass is lost at
  β cluster masses per relaxation time, and the relaxation time shortens by 1 + ψ₁ f while black
  holes remain. β = 2.8 × 10⁻³ and ψ₁ = 147 are `BH_LOSS_BETA` and `BH_LOSS_PSI_SLOPE` of
  `tables::cluster_dynamics`, read from there and never written as literals, and with
  `BH_RELAXATION_PREFACTOR` in t★ they are the three constants plan 15's P15.T8.a fits against the
  CMC catalogue (Kremer and others 2020); these are the scratch values until then. The decay rate β
  ψ₁ = 0.41 is derived, not a constant of its own, and with f₀ near 0.05 the black holes are gone
  after about five relaxation times, inside the brainstorm's four to six (on the clock k t ÷ t★, so
  1.6–2.4 of today's t★ at k = 2.5; ruling 126.4). Black holes are a compact
  Plummer class of scale 0.1–0.3 r_h, and the drawn core radius is correlated with f. Tests: none in
  dynamically old models, 20–400 in 47 Tucanae, 3,000–20,000 in ω Centauri, 10⁴–10⁵ over
  `milky_way_globulars` (ruling 126.4); f reaches zero between four and six t★ for retention between
  0.6 and 1.
- **P09.T9.d Core collapse.** f = 0 and age above 14 t★ sets `is_core_collapsed`, and every class
  takes a cusp of slope drawn on −1.6 to −2. Test: about a fifth (0.12–0.28) of
  `milky_way_globulars` over the line (Trager et al. 1995).
- **P09.T9.e Binaries and recycled objects.** Binaries are classes by system mass with a fraction
  per kind and population (scratch: 5% first and 1% second population in globulars, 30% in open
  clusters; plan 11 replaces the numbers, not the device). Millisecond pulsars, X-ray binaries and
  blue stragglers are marks inside the neutron-star and binary classes with expected counts from Γ:
  40 × (Γ ÷ Γ_47Tuc)^0.7 pulsars, capped for core-collapsed clusters, with the three numbers read
  from `PULSARS_AT_47_TUC_GAMMA`, `PULSAR_GAMMA_EXPONENT` and `PULSAR_CORE_COLLAPSE_CAP`. As built,
  the class table caps a cluster's pulsars at its expected neutron stars, the class they are marks
  in (`MemberClassTable::millisecond_pulsars`; `ClusterModel::millisecond_pulsars` stays the
  uncapped law), so no cluster's pulsars outnumber its neutron stars (ruling 126.3). Test: about
  4,000 (2,000–8,000) pulsars over `milky_way_globulars`, and the cap binding in at most 8 of them.
- **P09.T9.f Runaway factor.** A young cluster's living band-E count × (1 − f_ej(M)), with f_ej 15%
  rising to 38% at 10^3.5 M☉ (Oh et al. 2015), and a few per cent in band D. The ejected are plan
  08's runaways, already fed by the budget. Test: table-driven.
- **P09.T9.g Tails.** One class per band: a straight tube along the bulk velocity through the
  cluster, from the tidal radius to the grid's reach, Gaussian across with a width of one tidal
  radius, line density = mass-loss rate ÷ drift speed along the tail, mass-loss rate from Lamers or
  from P09.T13. Bottom-heavy band shares (what the cluster lost) and first population only. Bound
  per design note 10. Tests: bound hunt; the tail's expected count equals the mass lost in reach ÷
  drift speed. _Superseded by rulings 139.4 and 142.3:_ the tail runs as far as its oldest escaper
  has drifted, and each band's count is the interior's lost stars of that band (born less the
  K-scaled living classes, clamped at 0) times `w`, the window's share of all the mass lost.
  _Amended by ruling 145:_ the births are `N₀ = (M − M_BH) ÷ min(s p_can, p_dep)` in the classes'
  own currency with `K = min(s p_can ÷ p_dep, 1)`, and a globular's `w` is the time share
  `min(τ, age) ÷ age` (0 if `Ṁ ≤ 0`).
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
feature's (with the age spread for nurseries). Velocity = bulk + isotropic normal of σ(r) g(m) ÷
g(m_TO), Bianchini et al. 2016's partial equipartition with g = e^(−m ÷ 2m_eq) up to m_eq = 1.5 M☉ and
e^(−½)(m ÷ m_eq)^(−½) above (ruling 126.7; the profiles keep η = 1 for P15.T8.b), cut at the local
escape speed. Sphere of influence: the smaller of the galactic tidal radius and the
same formula about the feature's centre, with the floor in a harmonic core (`sphere_of_influence`).
Plan 06's `SystemStars::generate(galaxy, record)` draws metallicity from the record's component,
which a member must not do, so this task adds
`SystemStars::generate_with(galaxy, record, &Composition, MultiplicityContext)` (as built: a binary
class forces a companion) beside it in `stellar/system.rs`, with
`generate` delegating to it, no output change. Files: `features/members.rs`, `stellar/system.rs`.
Tests: order independence; a retained neutron star's kick is always below the escape speed; velocity
dispersion by class against σ(r) g(m) ÷ g(m_TO); band A's factor is 1.18.

#### P09.T11 Interior checks

Slow tests over `named_cluster` and `milky_way_globulars`: the figures of P09.T9 together, the
retention test of the brainstorm's kick law ("at least a tenth in a cluster with a birth escape
speed of 50 km/s"), and total black holes over the system of order 10⁴–10⁵.

### Phase 3: the globular system

#### P09.T12 Globulars as they are today

- **P09.T12.a Number, origin and place.** Expected number = dark halo mass ÷ 6.5 × 10⁹ M☉ with 0.2
  dex scatter (Burkert and Forbes 2020), read from plan 02's `AccretionHistory`, which draws it. An
  origin mark: 40% in situ, the rest among the accreted progenitors in proportion to mass, sharing
  the progenitor's halo component (Massari et al. 2019). Density a cored r^−3.5 with a core of
  6,800 ly, flattened to 0.5 and inside 26,000 ly for the metal-rich in-situ clusters, cut at 65,000
  ly, normalised at its cut (ruling 126.5): the metal-poor part places 0.82 of its share inside
  65,000 ly (Harris 2010: 87 of 106 inside 20 kpc), the metal-rich part is normalised to its
  truncated law. Replaces the stub of P09.T3.a. Tests: 80–800 over seeds, about 160 (the untruncated
  count) at Milky Way parameters, 0.874 of the count placed, inside-cut medians of 5.0–5.6 kpc
  (metal-poor) and 2.8–3.3 kpc (metal-rich).
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
M₀ (1 − t ÷ t_dis(M₀)) with 40 bisection steps, `R_G` in kiloparsecs as BM03's eq. 10 reads (ruling
126.1). Birth half-mass radius today's (ruling 126.2: Gieles, Heggie and Zhao's 2011 expansion
erases the birth radius and cannot be inverted once a cluster evaporates), so the birth escape speed
is today's × √(M₀ ÷ M). Outputs feed `ClusterModel` (birth escape speed, mass-loss rate for tails,
the first population's share). Destroyed clusters are not generated here: a test asserts that no
code path creates a globular with zero present mass. Tests: BM03's Table 1 (71,236 M☉ at 8.5 kpc:
23,769 Myr ±2%, 11,675 Myr ±3% at ε = 0.5); the initial masses of `milky_way_globulars` at a median offset within ±0.25
dex of the Baumgardt–Hilker catalogue's and at least 70% within 0.25 dex (the catalogue integrates
orbits with dynamical friction); median birth escape speed 1.5–2.3 times today's (ruling 126.1–2).

#### P09.T14 Milky Way checks: the globular system

Slow test at Milky Way parameters over 50 seeds: mass function (turnover near 2 × 10⁵ M☉, the same
inside and outside 5 kpc), sizes against radius, median eccentricity 0.5–0.75 and median pericentre
1–2.5 kpc, central escape speeds with a median of 17–22 km/s, under 1% above 100 and a 99th
percentile under 100 (ruling 126.8: a continuous law expects about 0.5 per galaxy above 100).

### Phase 4: supernova remnants and the clocked test

Pure functions. The processes that use them are in phase 7.

#### P09.T15 The shell window

- **P09.T15.a Closed form.** `shell_window(site: &SiteGas, energy, metallicity) -> ShellWindow`. The
  shell is distinct until its shock slows to β c_net, β = 2, c_net² = C₀² + σ², σ = 8 km/s, with
  C₀² = P ÷ ρ the ambient's isothermal sound speed from the site's pressure and density
  (`GasState::isothermal_sound_speed`; Cioffi, McKee and Bertschinger 1988, p. 264, "the ambient
  isothermal sound speed"; ruling 98 of 2026-09-22 replaced the adiabatic γP ÷ ρ this text first
  had). Radiative branch (Cioffi, McKee and Bertschinger 1988): W = t_PDS × [¾ (v_PDS ÷ β
  c_net)^(10⁄7) + ¼], the exact inverse of their eq. 3.32b (their eq. 4.4a drops the ¼), with
  t_PDS = 1.33 × 10⁴ yr E₅₁^(3⁄14)
  ζ^(−5⁄14) n^(−4⁄7) and v_PDS = 413 km/s n^(1⁄7) ζ^(3⁄14) E₅₁^(1⁄14). Hot branch (Tang and Wang
  2005), taken when the blast turns sonic before t_PDS: W = 0.41 t_c, with t_c their characteristic
  time from energy, pressure and sound speed. `SiteGas` comes from `GasField::state` with
  `SmoothingScale::AtLeast(250 ly)`, floored at the corona's pressure. β, σ and the smoothing scale
  belong to the generator version. Files: `galaxy/snr.rs`. Tests: `window_table` at P ÷ k = 3,800 K
  cm⁻³ reproduces, to 25%, the radiative branch's 4.8, 8.4, 8.4, 4.4, 1.9, 0.82 and 0.35 × 10⁵ yr at
  n = 10⁻², 10⁻¹, 1, 10, 10², 10³ and 10⁴ cm⁻³, and the hot branch's 2.0 × 10⁵ yr at 10⁻³ (ruling
  98 re-pinned the radiative entries with the isothermal C₀; the table first read 3.5, 6.2, 7.5,
  4.3, 1.9 and 0.35, the adiabatic form's, with no 10³ entry; the hot branch's 2.0 is Tang and
  Wang's and was not re-derived; with this form plan 07's floor of 300–450 K cm⁻³ gives a largest
  window of 2.06–2.39 Myr at E₅₁ = 1, which rises as E₅₁^0.32, so P09.T15.b's supremum over the drawn
  energies is larger);
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
  (version bump with P09.T35). Tests: 0.4–1 Type Ia a century over seeds, about 0.46 at Milky
  Way parameters, within 0.42–0.53 (ruling 141: the earlier 0.40 ± 15% was the model's own
  estimate at the scratch Chabrier scale 0.68, and the fitted scale raised the formed mass 7.9%;
  Li et al. 2011's 0.54 ± 0.12 gives the bottom); a fifth of delays under 0.1 Gyr and 62% under 1 Gyr.
- **P09.T18.b The delay-first draw.** `IaProgenitor::draw(stream, population)`: age ∝ formation
  history × ψ, time of explosion within the interval, channel given the delay, the two masses given
  channel and delay (scratch closed forms until plan 15's samplers; the primary always at least 2.5
  M☉ so that it is layer D, and both lifetimes within the delay), and for a merger the separation
  after the common envelope from Peters (1964): a⁴ = (256 ⁄ 5) G³ m₁ m₂ (m₁ + m₂) t_insp ÷ c⁵ with
  t_insp = delay − lifetimes. `IaProgenitor::period_at(t)`. Tests: lifetimes plus inspiral equal the
  delay to a second; the orbital period a thousand years before a merger is 80–100 s.
- **P09.T18.c What a Type Ia leaves.** `IaLeftover` by channel, as defaults of the generator
  version: both destroyed 50%; a surviving donor 30%, split as plan 08's hypervelocity class is
  (ruling 128.1): 26% at 1,000–1,500 km/s and 4% at 2,000–2,500 km/s, uniform in each (El-Badry et
  al. 2023 §8.2; the D6 mechanism of Shen et al. 2018); a
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

_Status (lane `centre09a`, 2026-09-29): T24.a–c, T25, T26 and T27 are built; see Risks, "Phase 6,
T24–T27 as built", and the provisional findings after it. `resolve` now answers a centre ID;
nothing else reads the centre until T28.b. Lane `centre09b` (2026-09-29) applied ruling 144's
points 1, 2, 4, 5a and 6–10 to the centre's own goldens; see Risks, "Ruling 144 as built". The
windows below are ruling 144's._

#### P09.T24 Profile and distribution function

- **P09.T24.a Profile and potential.** `CentreProfile`: the broken power law with inner slope 1.3, a
  break near 10 ly and outer slope 3.5 (Schödel et al. 2014; Gallego-Cano et al. 2018), continued
  inward to 10⁻³ ly, then r^−½, truncated at the grid's reach of 128 ly; mass and break from
  `NuclearClusterParams`; enclosed mass in closed form by pieces; potential of the black hole plus
  the cluster. Tests at Milky Way values (ruling 144.2 and 144.4): ρ(1 pc) 1.2–1.8 × 10⁵ M☉ pc⁻³,
  M(<1 pc) 0.8–1.2 × 10⁶ M☉, M(<3 pc) 6–10 × 10⁶ and M(<3.9 pc) 7–11 × 10⁶ (Schödel et al. 2018;
  Chatzopoulos et al. 2015), 4–6 × 10⁷ systems at T27's own mean system mass, fewer than three
  inside 10⁻³ ly, the stars outweigh the black hole near 10 ly; systems per cubic light-year at 3
  ly printed.
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
acceptance (design note 13), computed by a fixed quadrature. Tests (ruling 144.7–8): isodensity
axis ratio 0.65–0.75 inside 5 ly and projected isophote ratio 0.68–0.76 over 1–8 ly (the observed
0.71 and 0.73 are both isodensity ratios; a sample's second moments inside a sphere read 0.91);
net rotation of the sign of the galaxy's and 30–50 km/s in a slit along the plane (Feldmeier et
al. 2014), at a reversal share of 0.8 (Chatzopoulos et al. 2015's F = 0.85 ± 0.15); removed share
within a factor of two of 4 × 10⁻⁵.

#### P09.T26 The centre's classes

By the class device of P09.T8, with distribution functions per design note 14: old stars by band
with ages from the centre's own distribution (Schödel et al. 2020: 80 / 15 / 3 / 1%, ruling
144.10); a burst of 2.5 × 10⁴ M☉ 3–8 Myr ago, a third on the clockwise disc (n ∝ r⁻³ from an inner
edge at 0.1 ly, k = 25 about the Milky Way's (i, Ω) = (130°, 96°)) and the rest isotropic, n ∝
r^−2.1 (ruling 144.6); white dwarfs; neutron stars on the
stellar profile, widened, with retention from the kick law against the local escape speed (1,100
km/s at 0.1 ly, 210 at 10 ly) averaged over the profile, about a third (ruling 144.9: 0.30–0.40
under Disberg and Mandel's log-normal kicks); black holes on a slope of
1.75–2 with a break at half the stars', about nine tenths retained (Bahcall and Wolf 1976). Tests:
10⁴–4 × 10⁴ black holes inside the central parsec (Hailey et al. 2018); retention figures to a
third; the unretained are not generated here (they are in the bulge's displaced classes).

#### P09.T27 The twelve-level grid and the centre's members

`NestedGrid::new(1 ⁄ 256 ly, 32, 12)`: cells from 1 ⁄ 256 ly to 8 ly, reach 128 ly. Placement as
P09.T21 with the marks of P09.T25 after the class pick. `CentreMemberId`; the central black hole is
member zero of the feature-level list, a `MemberRecord` with its mass from `GalaxyParams`.
`CentreModel::as_global_entry()` gives plan 10 the first entry of its list. Plan 03's `resolve`
dispatches `SystemIdKind::Centre` here. Tests: the fullest cell and band, under the flattened bound
`e^(−k/2) I₀(k/2) ÷ A` (ruling 144.5a), expects the computed figure at the centre's own mean mass
(Risks, "Ruling 144 as built"), and a typed headroom check says whether a seed's centre can pass
8,192; the innermost cell against the model's own expectation of what it holds; every member's ID
round-trips through `SystemId::from_raw` (levels
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
  passed down from the parent feature. Tests: 80–90% (ruling 118.2; 0.7–0.9 sampled) of core collapses of a sampled
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
  The cluster's 4–5 × 10⁷ members and every conclusion about the 8,192 index still hold. _(T24.a's
  and T27's figures are superseded by ruling 144; see "Ruling 144 as built".)_
- **Where the shell window lives.** The brainstorm's order of attack puts "the shell test" with
  kicks and displaced objects, which is plan 08, while this plan's scope holds the supernova section
  in full. This plan owns `snr` and `claims`; if plan 08 has already built the window, P09.T15
  adopts it and P09.T35 shrinks to the feature side.
- **Association lifetimes.** The features table says associations are "under about 30 Myr", the φ
  rule says φ falls to the bound fraction "by 30–100 Myr". Resolved as dissolution ages on 30–100
  Myr, which puts 80–90% of core collapses inside features (ruling 118.2); "under 30 Myr" is read as
  the age at which an association still has O stars. The association count then tends to the top of
  "tens of thousands", and at the fitted Chabrier scale passes it: 1.2–2.1 × 10⁵ above the 100 M☉
  floor, some 10⁴ of them above 10³ M☉, the rich OB associations "tens of thousands" fits (ruling
  140.3). The two sentences of the brainstorm do not agree as written (an association
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
- **Retention at 50 km/s (ruling 106.4, plan text only).** P09.T9.b's window at 50 km/s was
  13–19%, but plan 06's kick law as built (ruling 96: Disberg and Mandel's log-normal truncated at
  1,000 km/s, with a low mode of σ 5 km/s and a share of about 19%) retains 19.6% there, nearly
  all of it the low mode's. The window is 15–25%, the same as ruling 96.3's at 20 km/s, which this
  plan's T9.b still gave as 8–12% and now gives as 15–25% too. The 18–26% at 100 km/s is not
  re-checked here. Nothing is built yet; no output moves. _Superseded for cluster retention by
  ruling 126.3:_ a low-mode neutron star is judged on its pair's systemic speed (σ 12 km/s), so the
  window at 20 km/s is 5–17%; 15–25% stays a test of the kick law alone.
- **Re-validated at `d2787a2` (lane `feat09a`, 2026-09-26, `GENERATOR_VERSION` 12).** Consumes
  reconciled with plans 01–08 and 15 as built: `TagScope` and the tag by value in `Stream::open`;
  plan 02's `FeatureShare` lives in `galaxy::ages` and has no formation-rate function; the
  potential's method names; plan 06's remnant outcome as `Death` and `CompactRemnant`, spin-down as
  `NeutronStar::state_at`, tracks to 100 M☉; plan 07's split sound speeds, pressure floor through
  `params()` and modifier slices; plan 08 built only to P08.T7; none of plan 15's tables. Rulings
  folded into the text: 2 (Design note 21's bound for the cored stellar discs), 20 (the record's
  cap and the member payload, in Consumes), 23 and 25 (built; the feature cache keys by galaxy), 98
  (already in P09.T15.a), 103 (the four phases, in Consumes). Ruling 106.4's retention window for
  P09.T9.b arrives with the orchestrator's `r9-amr` patch and is not edited here. T1's acceptance
  command is fixed; T2.c's files are the real ones; T5's server call site does not exist yet.
  **Pending re-validation:** P09.T9.b waits on P08.T8.b (`kick_bins::speed_bin_shares`); P09.T2.c
  on P08.T12's `stay_share`; P09.T15.b, T17 and T33–T36 on P08.T9, T12 and T13 (`ClassTable`,
  `explosion_site`, `SHELL_WINDOW_CAP`, `recent_death_claims`); P09.T10's member velocities on a
  record-free draw such as `kinematics::draw_on`; P09.T24 on research finding R2 (the M–σ offset).
  Phases 2–8 otherwise consume only what phase 1 now provides.
- **Ruling 2's gas decision is provisional** (copied here at re-validation, as the ruling asks). The
  gas stays exponential in height, so the clouds' proposal bound is the ratio at the height nearest
  the plane. If a later ruling cores the gas as the stellar discs are, the clouds take the stellar
  discs' route (the greatest ratio over the profile's table) and nothing else here changes.
- **T1–T6, as built (lane `feat09a`, 2026-09-26, at `GENERATOR_VERSION` 12).** No output moved:
  nothing reads φ until T2.c. New goldens `catalogue_classes/registry.golden` and
  `galaxy/features/cells.golden`; `rng/tags.golden` gains the fourteen `feature.*` tags. Names that
  differ from the sketches: `ClassId::cell_log2_ly()` returns `Option<u32>`; `NurseryRates` is
  built `from_fields`, since `Galaxy::from_params` builds `FeatureShares` (Design note 15; some
  milliseconds per galaxy); `FeatureShares` adds `phi_sub_disc`, `nurseries()`,
  `clusters_alive_per_system`, `supernova_clock()` and `cloud_weights()`; `EmissionClass::of` and
  `FeatureGas::new` take the galaxy; `FeatureCellCache::contents` returns an `Arc`. Helpers added
  outside `features/`: `VerticalProfile::max_ratio_to_exponential`, `GasField::mean_cloud_gas`
  and `GasField::cloud_gas_bound`, and `kinematics::draw_on`, the velocity draw on a caller's
  stream, which `draw` now calls (bit-identical). Deviations:
  - _Proposals._ Design note 21's single cell bound wasted nine nursery candidates in ten on the
    young disc's sharp arms, and put the fullest Milky Way cell near 46,000 candidates with the
    molecular disc's clouds bounded at the centre's corner. As built the disc processes and the
    neutral clouds propose x and y from 256 columns of 256 ly, each with its own bound, and the
    molecular clouds exponentially in |x| and |y| with scale √2 L (`R ≥ (|x| + |y|) ÷ √2`), their
    count drawn after the neutral part's on the same cell stream. Waste is now 1.4–1.6 per
    accepted feature and the fullest fixture cell expects about 2,700 candidates; the heaviest
    galaxy the builder allows and 200 seeds stay under 16,384 with eight standard deviations.
  - _Conditional draws._ Rather than rejecting a candidate whose age or life has ended, an old
    cluster draws its age given that it is alive, and a nursery its age, mass, bound mark,
    embedded duration and dissolution age given that it is alive at the epoch, by rejection on
    their own streams (at most 256 attempts; a candidate that exhausts them, with probability
    under 10⁻¹⁴, is rejected). The processes' densities are the living features'.
  - _T2.a._ Lamers et al. 2005's 1.3 Gyr is their _total_ disruption time of a 10⁴ M☉ cluster
    (their eq. 11 and abstract), so `dissolution_time` is the age at which a cluster is gone.
    `present_mass` is `m₀ μ_ev(t) (1 − t ÷ t_dis)^(1 ÷ γ)`, the product of L05's stellar-evolution
    fit (Table 1's solar row, bridged linearly to 0 below 10 Myr) and the disruption term, not
    eq. 6's difference of powers, so that the life is `t_dis` exactly. φ counts the disruption
    term alone, since a dead star is still a system of its budget.
  - _T2.b._ φ adds the embedded share `e(a)` and the association floor's share to Design note
    2's form (both 1 at the plan's figures). G is uniform on 30–100 Myr and not tuned (finding
    below). Globulars' φ is 0 until P09.T12 draws them. φ is uniform across bands with
    `TODO(P09.T9)`.
  - _T4.a._ The present-mass test is the initial mass's conditional distribution given the age,
    uniform after the closed-form transform; the present mass is a monotone function of it at a
    given age, so the two tests are the same test. Young bound clusters carry the same half-mass
    radius and concentration marks, and stop expanding when they emerge: their size is then four
    half-mass radii within 10–100 ly (the one jump in a nursery's size, where its gas disperses).
  - _T4.b._ The bubble's power is the supernovae's, `N_SN × 10⁵¹ erg` spread over `t_first` to
    `t_last` (Mac Low and McCray 1988's approximation), and, since ruling 118.4, the winds', 10⁵⁰
    erg per star of 8 M☉ or more over the first 4 Myr (Krause et al. 2013). With a varying power the
    radius reads Weaver's form through the injected energy, `0.76 (E(t) t² ÷ ρ)^⅕` with `t` the
    nursery's age (ours; it is Weaver's exactly while one constant power acts), so a nursery has a
    bubble from birth.
    An embedded region's gas is `M_* (1 − ε) ÷ ε` with ε uniform on 0.1–0.3 (Lada and Lada 2003),
    falling linearly to none when it emerges; its size is `max(D₀, v × age)` within 10–300 ly, D₀
    the clump's diameter by the clouds' mass–radius relation. Members' ages from −H belong to phase 5.
  - _T4.c._ Clouds: `M^−1.7` over 10⁴–10^6.5 M☉, a Plummer ball whose half-mass radius is
    Roman-Duval et al. 2010's equivalent radius, `R = (M ÷ 228)^(1 ÷ 2.36)` pc (ruling 118.3;
    first built with Σ = 170 M☉ pc⁻²), ε = 0.15 of the neutral layer (Design note 19).
  - _T5._ `FeatureGas` puts a segment's ends in a canonical order, so its list is a function of
    the unordered pair; a feature is listed when the segment passes within its reach (ten core
    radii for a cloud). Features sit at their epoch positions.
  - _Plan 10_ must reach `set_halo_discrete` through a `FeatureShares` it builds or owns, since
    `Galaxy` lends its own immutably.
- **Findings of T2–T4, ruled (ruling 118, built by `feat09a` at `GENERATOR_VERSION` 13).** Only
  phase 1's own goldens could move (none did: the clouds' radii and the bubbles' power are not
  pinned). Measured at Milky Way values:
  - _Cluster lives (T2.a)._ 1.3 Gyr is L05's total disruption time: mean life 183 Myr over `M⁻²` on
    10²–10⁵ M☉, about 6.6 × 10⁴ bound clusters alive at 345 born per Myr, about half under 100 Myr
    and 8% over 1 Gyr, inside ruling 118.1's windows.
  - _Core collapses inside features (T2.b)._ 0.876 (8–100 M☉, plan 06's solar lifetimes), in the
    80–90% of ruling 118.2.
  - _Clouds (T4.c)._ Roman-Duval et al. 2010's `M = 228 R^2.36` gives diameters of 32–370 ly
    (median about 49), in ruling 118.3's 25–400 and 40–60. Miville-Deschênes et al. 2017's CO
    clouds, which include faint envelopes, are larger and thinner (median R 25 pc, Σ 16.5 M☉ pc⁻²,
    their Table 2); the dense clouds are chosen because the plan's clouds are where stars form and
    what dims a line of sight. About 9,800 clouds ("thousands"). The fixture's molecular disc has
    14% of its mass beyond 1,000 ly, so T4.c compares the molecular term's clouds inside the sphere
    with the disc's mass inside it (ruling 118.5); the ratio is 9 to 10⁻⁶.
  - _Bubbles (T4.b)._ With winds, a bubble exists from birth. At least 80% of radii lie in 100–1,000
    ly, and every radius is at or below the blow-out cap (2.5 × the fixture's 700 ly neutral
    height, 1,750 ly) and 3,300 ly. The cap's source is still owed.
  - _R5 (warm ionised filling)._ T5 adds hot holes and neutral clouds, no ionised gas, so the
    features leave `gas14`'s 0.21 against Gaensler's ~0.3 where it was.
- **Phases 2 and 3 as built (lane `feat09b`, 2026-09-27, at `GENERATOR_VERSION` 13): T7, T8.a–b,
  T9.a–h, T10, T11, T12.a–b, T13, T14.** Nothing any consumer reads moves: the catalogue is
  unwired until T2.c. Its own golden, `galaxy/features/cells.golden`, is re-blessed at 13: the
  globulars now take the first indices of every cell, so every other feature is renumbered and,
  its streams keyed by its ID, redrawn. `rng/tags.golden` gains five tags (`feature.globular`,
  `feature.cluster`, `member.mass`, `member.velocity`, `member.abundance`).
  Names and shapes that differ from the sketches:
  - `tables::cluster_dynamics` is hand-entered with the scratch values (2.8 × 10⁻³, 147, 0.138; η
    = 1; 40, 0.7, cap 40) and no fit header, for P15.T8 to take over. The Baumgardt–Hilker copy is
    `features::testing::{milky_way_globulars, named_cluster, catalogue_parameters}` (165 clusters
    with orbits, fetched 2026-09-27, behind the crate's `testing` feature).
  - `SystemOrigin::FeatureMember { feature: PackedFeature, attempt: u16 }`: the feature packs into
    29 bits (ruling 20), and the attempt is the primary's redraw a remnant class chose, which
    `stellar::system`'s primary draws now read (a grid record's is 0, bit for bit).
    `SystemRecord::layer` reads a member's slot band. `kinematics::draw` panics for a member,
    whose velocity is its cluster's.
  - `ClusterModel::new` takes `ClusterParameters`; the model holds T9.b–e's closed forms
    (`retention`, `black_hole_fraction`, `black_hole_count`, `is_core_collapsed`, `cusp_slope`,
    `millisecond_pulsars`, `xray_binaries`, `blue_stragglers`). Its own marks are
    `ClusterMarks` on `feature.cluster`. An open cluster's core is `r_t ÷ 10^c` held below `r_h ÷
10^0.1`; young bound nurseries now carry T4.a's radius and concentration marks.
  - `features::interior::{MemberClass, ClassKind, Generation, Multiplicity, ClassProfile,
ProfileShape, MemberClassTable, TailClass, LocalCell, MemberAbundances}` and the modules
    `counts` (T9's closed forms), `retention` (T9.b's quadrature), `abundances` (T9.h).
    `MemberClassTable::new(galaxy, model, reach, tail_axis)`; `bound(band, corner_radius)` is the
    profiled classes' and `cell_bound(band, &LocalCell)` adds the tail's (Design note 10), since the
    tail needs the cell's geometry, and `pick(band, p, mark, bound)` takes the caller's bound for the
    same reason; `density` and `mean_member_mass` take a local `PointLy` and return `f64` (per
    cubic light-year, M☉). Binaries are a `Multiplicity` of the living and neutron-star classes,
    not a kind. `members::draw_member(galaxy, feature, model, class, position, id, bulk) ->
Option<MemberRecord>`: the caller holds the model and the bulk velocity; `None` only after 4,096
    attempts, with a debug assertion.
  - `kinds::globular::{GlobularMarks, GlobularOrigin, GlobularSystem, GlobularMassFunction,
GlobularHistory, history, history_on_orbit, orbit, dissolution_time}`; `FeatureShares` holds
    the `GlobularSystem` and its φ (`phi_globular`, band-weighted, which removes T2.b's `TODO`).
    Recent progenitors' globulars are left to plan 10's streams and dwarf cores.
  - Choices of ours, each provisional: the effective escape factor 0.7826 (the Plummer mean of
    `(1 + x²)^(−3/4)`, cubed-root); a mean retained black
    hole of 15 M☉; a system's binary mass 1.5 times its primary's; neutron stars of 1.35 M☉;
    Kalirai et al. 2008's initial–final relation for the white dwarfs' profiles; the depleted slope
    read at `t_rh × 12 Gyr ÷ age`, so young clusters are not depleted; the black holes' scale
    `(0.1 + 0.2 f ÷ 0.06) r_h`, which the profiles' core takes while it is the larger; tails
    drifting at the central dispersion and reaching four tidal radii until P09.T21's grid; the runaway share falling back to 15% at 10^4.5 M☉; carbon, aluminium and
    magnesium spans and a 0.1 dex iron spread; X-ray binaries (5 at 47 Tucanae's Γ, `Γ^0.74`) and
    blue stragglers (`M_core^0.38`); a globular's `log₁₀(r_h ÷ r_c)` normal about 0.72 ± 0.37 (the
    catalogue's); the in-situ metal-rich globulars in the bulge's figure by its scale lengths.
- **Findings of phases 2 and 3, ruled (ruling 126, built by `feat09b` on 21a82d0 and rebased onto
  9148413, at `GENERATOR_VERSION` 13).** The code was fixed where the ruling found a bug (T13's kiloparsec) or
  a wrong model (T9.b's pair speed, T10's equipartition), and the tests follow the ruling's
  windows. Measured on 9148413:
  - _T13:_ BM03's Table 1 to ±2% and ±3%; the initial masses' median offset −0.168 dex, 82% within
    0.25 dex; the median birth escape speed 1.77 times today's.
  - _T9.b:_ retention 22.9% at 100 km/s, 19.7% at 50, 11.0% at 20, 0.29% at 4.7; 1,647 neutron
    stars in 47 Tucanae, 220 in M4, 0 in Palomar 5.
  - _T9.c–e:_ 142 black holes in 47 Tucanae, 8,314 in ω Centauri, 12,616 over the catalogue;
    core-collapsed 0.218; 4,930 pulsars, the cap binding in 5 clusters.
  - _T12.a:_ inside-cut medians 5.14 kpc (metal-poor) and 2.97 kpc (metal-rich), over 32 galaxies.
  - _T7:_ central escape speeds of 0.906, 2.07 and 4.80 km/s at 10², 10³ and 10⁴ M☉ (−1.6%, −6.0%
    and −9.4% of the targets).
  - _T14:_ median escape speed 18.0 km/s, 0.36% above 100 km/s, p99 82.3, fastest 164; median
    eccentricity 0.67, pericentre 1.30 kpc, turnover 10^5.1 and 10^5.3 M☉.

  Recorded as risks:
  - _The black holes' clock_ (`BH_CLOCK_FACTOR` = 2.5) is a knife edge: the closed form's hard
    floor at f = 0 drops 47 Tucanae from about 400 at 2.0 to about 170 at 2.5 and to none at 3.0.
    The closed form cannot give the small non-zero fractions Dickson et al. 2023 measure, nor
    Palomar 5's 20% (Gieles et al. 2021), which lie outside the model. The ruling's two optional
    items were not built: the mean black hole from plan 06's remnants at the cluster's metallicity
    (Dickson et al. 2023: 6.7 M☉ at 47 Tucanae, 13–16 M☉ when metal-poor) and `r_h × max(1, M ÷
10⁶ M☉)^0.24` for the heaviest globulars.
  - _Pulsars are capped at the neutron stars they are marks in_ (`MemberClassTable::
millisecond_pulsars`; a deviation in T9.e): the encounter-rate law alone gives more pulsars
    than neutron stars in 5 of the catalogue's 165 clusters, each with well under one of either.
  - _A retained neutron star's pair_ can be unbound by its companion's own supernova, ejecting the
    neutron star later (Pfahl et al. 2002's "several times smaller"): not modelled.
  - _Open clusters' radii_: Hunt and Reffert 2024 find them only lightly correlated with mass,
    against Brown and Gnedin 2021's M^0.242 that T4.a now reads.
  - _T13's inversion_ stays about 0.15 dex from the catalogue's initial masses, which come from
    backward orbit integration with dynamical friction.
  - _The globular law_ is `r^−3.5` where Harris 2010 measures it (3–40 kpc) and steepens beyond;
    the normalisation at the cut stands for that.
  - _Still provisional_ (ruling 126.9): the 0.7826 factor, neutron stars of 1.35 M☉, Kalirai 2008,
    the depletion read at `t_rh × 12 Gyr ÷ age`, `u ~ N(0.72, 0.37)`, the 15 M☉ mean black hole,
    the X-ray binary and blue-straggler laws.

- **Phase 4 bar T17 as built (lane `feat09c`, 2026-09-28, at `GENERATOR_VERSION` 14): T15.a–b,
  T16.a–b, T18.a–c, T19.a–b.** Pure functions; nothing any consumer reads moves. A new golden,
  `catalogue_classes/supernova.golden`, pins the window table, shell states, the fixture's caps,
  Type Ia rate and ancient shares, twelve progenitors and leftovers on their own tags and
  light-curve points; the only moved golden is `rng/tags.golden`, which gains `class.type_ia.progenitor`, `class.type_ia.leftover`
  (scope `System`) and `class.ev.supernova` (scope `Event`, behind event tag `0x0202 SUPERNOVA`,
  registered in `event_tags!`). Files: `galaxy/snr/{mod,window,caps,shell,remnant,testing}.rs`
  (a directory for the plan's `galaxy/snr.rs`), `galaxy/catalogue_classes/{type_ia,supernova}.rs`.
  Names and shapes that differ from the sketches:
  - _T15.a._ `SiteGas::{at, of, uniform, hot_interior}`; `SiteGas::at` reads `GasField::state` at
    `SiteGas::SMOOTHING` (250 ly) and takes the drawn phase's `local_density()` and
    `isothermal_sound_speed()` (plan 07's Risks left the choice to this plan; research NOTES §1.4's
    recommendation, CMB88's `n₀` being the local ambient). `shell_window(site, ExplosionEnergy,
Dex)`; `ExplosionEnergy` is in units of 10⁵¹ erg, a log-normal of 0.2 dex truncated at ±2σ
    (0.40–2.51, ours; `ExplosionEnergy::from_uniform` is the one law, which T17's `DeathMarks::of`
    feeds one uniform of `snr.energy`). The metallicity is `[M/H]` held to −1…+0.5 (ours), read as
    CMB88's ζ, which is the swept-up gas's: T17 should pass the site gas's metallicity
    (`GasField::dust_per_hydrogen`'s `[M/H]`), not the star's, a finding for T17. `ShellWindow`
    carries what T16 needs (`pds_time`, `pds_speed`, `merge_speed`, density, energy, ζ) and
    `ended_at(Years)`.
  - _The hot branch._ Built as `W = t_PDS (v_PDS ÷ β c_net)^(5⁄3)`, taken when `v_PDS ≤ β c_net`:
    the Sedov–Taylor blast reaching `β c_net` before `t_PDS`, continuous with the radiative branch
    by construction (CMB88's `v_PDS` is the Sedov speed at `t_PDS`). In Tang and Wang's (2005, eq. 3) terms it is `t_c (c_s ÷ β c_net)^(5⁄3)`, 0.48 `t_c` in hot gas, not the plan's 0.41 `t_c`,
    which is continuous with nothing (finding below). At 10⁻³ cm⁻³ and 3,800 K cm⁻³ it gives
    2.27 × 10⁵ yr against the plan's 2.0 (+13%, inside the test's 25%).
  - _T15.b._ `WindowCaps::{from_galaxy, from_gas_params, at_floor, cap, cut}`: the field's and
    the Type Ia's caps are one scan at the floor (4,096 points in `log n` over 10⁻⁸–10⁸ cm⁻³ and
    80 golden-section steps, raised by 10⁻⁶) at the greatest energy and least metallicity, since
    the window never rises with `C₀` and no site's `C₀² = s P ÷ ρ` is below the floor's; the
    bubble's is the interior's least density, the Box–Muller cut at 8.58σ. `cut` is the debug
    assertion. `SHELL_WINDOW_CAP` (4 Myr) is defined here: plan 08's `displaced` has none at
    `782c708` (P08.T13 is `disp08d`'s), so it is not moved but created where Design note 22 puts
    it; plan 08's prefilter should import it from `snr`.
  - _T16.a._ `shell_state_at(&ShellWindow, Years) -> Option<ShellState>` (`None` before T and from
    the window's end), `ShellPhase`, `ShellEmission`, `ShellWindow::ended_at_wall(LightYears)`.
    Truelove and McKee's uniform-ejecta (`n = 0`) forms with `R*_ST` taken from their first form
    at 0.495 so the join is exact; CMB88's offset power law with the offset `c` set so the speed is
    continuous too (their eq. 3.30's rule, ¼ for a Sedov blast); a momentum-conserving
    continuation from CMB88's `t_MCS` (their eq. 4.2 with Spitzer conduction). The ejecta mass is
    a constant 3 M☉ (`EJECTA_MASS`, ours); it moves only the first centuries and `t_MCS`.
  - _T16.b._ `remnant_offset(KilometresPerSecond, Years) -> LightYears`, `has_bow_shock(offset,
&ShellState)` at `BOW_SHOCK_SHARE` 0.68, `PulsarWindNebula::of(&PulsarState)` above
    `NEBULA_THRESHOLD` = 10³⁵ erg/s (ours): plan 06's own flag, `has_wind_nebula` at 10³⁶, ends a
    nebula after a median of 2.2 × 10³ yr, outside the plan's 10⁴–10⁵ (finding below).
  - _T18.a._ `DelayTimeDistribution::{amplitude_per_year_per_solar_mass,
rate_per_year_per_solar_mass, cumulative_per_solar_mass, share_below, from_galaxy,
rate_per_year, population_rate_per_year, ancient_share, drawable_rate_per_year, draw_floor,
lifetime_of, mass_with_lifetime}` and `type_ia::ancient_share(&Galaxy, Population)` (the galaxy added to
    the sketch). The Hubble time of Maoz and Graur's integral is 13.7 Gyr. A component's rate is
    `count_with_unborn × mean_formed_mass × E[ψ(age)]`, the expectation a 64-panel quadrature in
    the age distribution's rank. The ancient share counts every channel (every exploded system
    leaves its cell) per layer-D system of the population.
  - _T18.b._ `IaProgenitor::draw(&DelayTimeDistribution, &mut Stream, Population, after, until)
-> Option<IaProgenitor>`: the distribution and the explosion interval `(after, until]` added
    (T17's `SupernovaInterval` is not built). Fixed word offsets: the component mark, 4,096
    two-word proposals of the age (from the component's ages above the draw floor, accepted on
    `ψ ÷ ψ(floor)`), then channel, primary, secondary and the time of explosion in whole seconds.
    Delays, lifetimes and the inspiral are `Span`s, so the secondary's lifetime plus the inspiral
    is the delay exactly. Lifetimes are plan 06's track at solar composition and median draws,
    tabulated at 48 masses over 0.8–8 M☉ (scratch). The draw floor is τ(8 M☉) = 42.6 Myr plus
    the interval's 4.26 Myr before the epoch (finding below); `drawable_rate_per_year` is the intensity a
    class thins against. Masses (scratch): primary `m^−2.35` over the layer-D masses that fit; a
    double white dwarf's secondary uniform from max(0.1 m₁, M(delay)) to m₁; a living donor
    uniform on ½–1 of min(3 M☉, M(delay)); white dwarfs by Kalirai et al. 2008.
  - _T18.c._ `IaChannel { Merger, DoubleDetonation, HydrogenDonor, Iax }` (with
    `is_double_degenerate` and `leaves_a_survivor`) and `CHANNEL_SHARES` 0.56, 0.30, 0.04, 0.10:
    the merger's 0.56 against the plan's "both destroyed 50%", because the four keep their sum
    (ruling 128.1) and "under 5%" and "about 10%" leave 0.56; independent of the delay (scratch); `IaLeftover { Nothing, SurvivingDonor, HydrogenDonor, PartlyBurntDwarf }`,
    `IaLeftover::draw(channel, stream)` (two words; the donor's split reads plan 08's
    `SURVIVOR_POPULATIONS`, ruling 128.1), `offset_at(age)`.
  - _Cut-off._ The ancient share and the draw floor are taken before −(`SHELL_WINDOW_CAP` + L +
    H), the constant, not the Type Ia cap, so neither depends on a site; the explosions between
    the two fall to the grid's redraw rule (P09.T35).
  - _Test brackets beside the plan's words._ The bubble cap is tested only as under the field's
    (its 1.35 Myr is a finding); T16.b's offsets hold the median to 100–400 ly and the share
    outside the shell to 0.3–0.7 for "near 200" and "about half"; T16.a's 21 cm flag is set in the
    radiative phases only, so a shell that ends on the hot branch never shows it ("the 21 cm shell
    to the end" read as from cooling to the end).
  - _T19.a._ `SupernovaKind`, `SupernovaEntry::{new, with_remnant, with_leftover,
with_progenitor, state_at, explosion_event}`, `SupernovaState::{Progenitor, Supernova,
BareRemnant}`, `RemnantState`. `Progenitor` carries the time left and a double white dwarf's
    period, not plan 06's star: the caller evaluates the star at the same time. `supernova.rs` is also where T17 puts `DeathMarks`,
    `SupernovaInterval` and `claims`.
  - _T19.b._ `LightCurve::{of, at, luminosity, neutrino_luminosity}`: templates as sums of smooth
    terms (breakout, a logistic plateau, Nadyozhin's ⁵⁶Ni/⁵⁶Co heating with a diffusion rise and
    γ-ray escape), so there is no join. A Type Ia peaks at 1.07 × 10⁴³ erg/s on day 15; the
    neutrino burst is its own function.
- **Findings of phase 4, for a ruling (lane `feat09c`).** Measured at Milky Way values unless
  said:
  - _Which gas the window reads (T15.a; plan 07's Risks, ruling 103)._ As built, the drawn
    phase's `local_density()` and `isothermal_sound_speed()` (research NOTES §1.4); the
    alternative is `√(P ÷ ρ̄)` of the parcel's mean. The caps do not depend on it.
  - _ζ for T17._ CMB88's ζ is the metallicity of the swept-up gas, so T17 should pass the site
    gas's `[M/H]`, against T17's text, "the metallicity from the system's `Composition`". The
    window moves by at most 15% between −1 and 0 dex.
  - _The hot branch (T15.a)._ Built as the pure Sedov blast's time to `β c_net`, 0.48 `t_c` in hot
    gas, continuous with the radiative branch. Tang and Wang's own fit to their simulations (eq.
    2, `V_s = c_s (t_c ÷ t + 1)^(3⁄5)`; they say eq. 1, Sedov, "is not valid even before t = t_c")
    reaches `β c_net` at 0.93 `t_c`, about twice as late: 4.29 × 10⁵ yr at 10⁻³ cm⁻³ and 3,800 K
    cm⁻³ against the built 2.27 and the plan's 2.0. The plan's 0.41 `t_c` derives from nothing
    found (Tang and Wang's Mach 2 is 0.46 `t_c`, Sedov's 0.32; the plan's 2.0 × 10⁵ yr is 0.43)
    and is discontinuous with the radiative branch. Adopting eq. 2 needs a rule for the branch
    switch; it would lengthen hot-gas windows and the bubble cap (science check,
    `research/snr-verify/`).
  - _The Iax share (T18.c)._ The brainstorm's "about 10% (Foley et al. 2013)" is below the paper it
    cites: Foley et al. 2013, §7, give 31 (+17/−13) Iax per 100 Type Ia, about 24% of thermonuclear
    events; 10 per 100 is Foley et al. 2009's and 5.7 Li et al. 2011's. Built at 0.10.
  - _Citations corrected in the code._ The 0.677 bow-shock fraction is van der Swaluw, Downes and
    Keegan 2004 (A&A 420, 937), not the brainstorm's van der Swaluw et al. 2003; ⁵⁶Co's positron
    share is Nadyozhin 1994's 3.2%.
  - _Caps (T15.b)._ The field's cap is 3.26 Myr for the fixture, 3.13–3.63 Myr over 200 seeds'
    floors, 3.63 at the lowest floor (300 K cm⁻³): inside 2–4 Myr and under `SHELL_WINDOW_CAP`.
    The 10⁶-site hunt's longest window was 3.40 Myr (0.98 of its cap). The bubble's cap is 1.35
    Myr against the plan's "about 0.5 Myr": T4.b's interior log-normal (0.3 dex) is untruncated,
    and the window rises as `n^(−1⁄3)` towards the sampler's 8.6σ tail, 1.3 × 10⁻⁵ cm⁻³ (at the
    median interior, 0.005 cm⁻³, a shell lasts 1.37 × 10⁵ yr).
  - _Radii (T16.a)._ At the end of each window at 3,800 K cm⁻³: 565, 464, 311, 173, 78, 34, 15 and
    6 ly from 10⁻³ to 10⁴ cm⁻³. The densest is under the plan's 10–800 ly; the test holds 10–800
    to 10³ cm⁻³ and 3–10 ly at 10⁴.
  - _Offsets (T16.b)._ Median 234 ly and 0.56 outside the shell for field sites (young disc plane,
    ages uniform in the window, the ordinary kick mode at uniform ranks): "near 200", "about half".
  - _Nebulae (T16.b)._ Under plan 06's spin-down, a nebula at 10³⁶ erg/s lasts a median 2.2 × 10³
    yr among the half of pulsars born with one; 10³⁵ gives 1.45 × 10⁴ yr (77% born with one) and
    0.5% of 10⁵–10⁶ yr pulsars keep one. Plan 06's `has_wind_nebula` and this plan's nebula now
    use different thresholds.
  - _Rates (T18.a)._ 0.425 Type Ia a century at the fixture (0.40 ± 15%); 0.25–0.87 over 24
    seeds against the plan's 0.4–1 (the test holds 0.2–1).
  - _Ancient share (T18.a)._ 4.1–4.6% of each old population's layer D (young disc 0.3%) against
    the plan's 2–4% and the brainstorm's "about 3%": 1.3 × 10⁻³ per M☉ × 0.957 M☉ per system ÷
    layer D's 2.6% of systems. Counting only the "both destroyed" channel gives 2.3–2.6%; all but
    Iax 3.7–4.1%. The test holds 3.5–5%.
  - _The shortest delay (T18.b)._ Plan 06's 8 M☉ track lives 42.6 Myr at solar composition, above
    Maoz and Graur's 40 Myr, so no layer-D primary fits a delay of 40–42.6 Myr. The draw starts
    at 42.6 Myr plus the interval; it loses 18% of the young disc's Type Ia and 4% of the nuclear
    disc's, about 2.5% of the galaxy's, which `drawable_rate_per_year` states.
  - _Periods (T18.b)._ A thousand years before a merger the period is a median 89.6 s, 5–95%
    77.9–118.4 s: the plan's 80–100 s holds at the median (the test), heavy pairs run to 120.

- **Phase 5 as built (lane `feat09d`, 2026-09-28, at `GENERATOR_VERSION` 14): T20–T23.** No
  existing output moves: nothing reads members yet; the change to phase 1's catalogue (an old open
  cluster's reach, below) is pinned by no golden and changes no gas modifier, and the one to phase
  2's tails (zero beyond the reach) reaches only members. New golden
  `galaxy/features/members.golden`; `rng/tags.golden` gains five tags (`member.cell`,
  `member.position`, `member.accept`, `member.list`, `member.list_position`). Names and shapes
  that differ from the sketches:
  - `features::nested::{NestedGrid, NestedCell, NestedLevel, BuildNestedGridError}`.
    `NestedGrid::new` returns a `Result` (the width must be exactly a power of two, the cells a
    multiple of four); `cells_touching(&PointLy, radius)` and `owner_of(&PointLy) -> Option` take
    local light-years; `owns`, `cell`, `owned_cells`, `local_cell` (a phase-2 `LocalCell`),
    `level`, `all_levels`, `reach`, `width_log2` besides.
  - `MemberClassTable::{grid_width, grid, extent, with_tails_to, proposal, cell_candidates,
peak_candidates}` and `interior::CellProposal`, with `FEATURE_GRID_CELLS`,
    `FEATURE_GRID_LEVELS`, `GRID_WIDTH_MIN_LOG2`, `CELL_CANDIDATE_TARGET` (2,000).
  - `members::placement::FeatureInterior` (a feature's record, `ClusterModel`, class table, grid
    and bulk velocity: `of`, `member_id`, `candidate_count`, `candidate`, `members_in_cell`,
    `expected_candidates`, `galactic`) and `resolve_member(galaxy, FeatureMemberId) ->
Result<MemberRecord, ResolveSystemError>`; `placement::resolve` dispatches the `0` kind to it.
  - `members::level_list::{FeatureLevelList, ListClass, ListEntry, registered_classes}`:
    `FeatureLevelList::of(galaxy, feature)`, and `with_classes(galaxy, word, centre, MemberZero,
classes)` for the centre (P09.T27) and the tests. A class's count sits at word `c × 2³²` of
    `member.list` and member `k`'s position at `c × 2³² + 3k` of `member.list_position` (`c` its
    `ClassId` value), so a class registered in front moves the indices after it and never another
    class's members. `member(index)` is `None` until P09.T30.b, T36 and T43 register their draws.
  - `members::source::{FeatureMemberSource, FeatureInteriorCache, NoInteriorCache}`:
    `FeatureMemberSource::new(&dyn FeatureCellCache, &dyn FeatureInteriorCache)`. The interior
    cache is an addition for P09.T40's `ClusterModelCache`, since an interior costs seconds
    (findings). The census sums the cells touching the **unpadded** radius (plan 03's contract);
    the hits walk the padded one. `resolve_member` takes no cache and rebuilds the interior.
  - Members exist only in a galaxy with its kinematic tables (`Galaxy::with_full_potential`):
    the bulk velocity sets the tails' axis and so which candidates are accepted, so a zero
    stand-in would give one ID two members. Without them `FeatureInterior::of` is `None` and a
    member ID resolves to "no such system".
  - _Grid width (T21)._ A cell's expected count grows with w (the core's `ρ_c w³`), so "the
    smallest w from 1 ⁄ 64 ly for which no cell expects more than 2,000" is always 1 ⁄ 64 ly, whose
    16 ly reach misses the cluster. As built: the least power of two from 1 ⁄ 64 ly whose reach
    holds the tidal radius, where every profile ends, doubled while the doubled grid keeps every
    cell and band at 2,000 or under, until it holds the tails' extent; the tails are
    then cut at the grid's reach, their count in proportion to the length kept (P09.T9.g's "to
    the grid's reach"). The 2,000 is a target: a core too dense for the finest grid that reaches
    its tidal radius keeps that grid (seed `0x0921_0000`'s globular at feature cell (−2, −4,
    2), index 0, expects 4,191 in its fullest cell, under the index with eight standard
    deviations to spare). On the fixture (31 globulars, 43 open clusters and nurseries) the
    fullest cell and band expects at most 1,991 candidates (tails along x). The peak is searched on
    the cells beside the three axes, where the profiles' bound peaks, and within one cell of the
    tails' line (`peak_candidates`); a test asserts it equals the peak over every owned cell for
    three clusters. A globular of 10⁶ systems with a 240 ly⁻³ core takes w = 2 ly
    and peaks at 723 (band D, level 0; "near 600").
  - _Cusps (T21)._ A collapsed cluster's cusp, softened only at 10⁻³ ly, puts the nearest-corner
    bound of the eight cells at the centre at its peak: 5.8 × 10⁴ expected candidates in a 1 ⁄ 8
    ly cell of the fixture's collapsed globulars and 3 × 10⁷ in a 1 ly cell, against the 8,192
    index. There the candidates are proposed radially under `B max(ε, r)^−γ` over the octant ball
    of radius `√3 e` and dropped outside the cell (`CellProposal::Cusp`): thinning with a
    non-uniform proposal, exact by the same theorem as Design note 21's. The same three words of
    `member.position` are read; the profile test passes on a collapsed globular.
  - _Every member within the feature's reach (T23)._ `FeatureCatalogue::near` finds a feature by
    its reach, so every member must lie inside it. Ten half-mass radii fell short of the tidal
    radius for the densest old open clusters (r_t 90 against 22 ly, 114 against 80, 129 against
    71): an old open cluster's reach is now the greater of the two, the tidal radius computed as
    `ClusterModel` computes it (pinned by no golden). A tail's Gaussian spilled past its reach
    across the axis: a tail's density is now zero beyond the reach of the centre (phase 2's
    `TailClass::density`), and its expected count, the tube's to the reach along the axis, errs
    high by the Gaussian's share outside the sphere. Where an old cluster's reach is its tidal
    radius it has no tail at all (P09.T9.g's table keeps a tail only for a reach past r_t).
  - _Tests (T21)._ "Near 600" is asserted as 300–1,200 (built: 723). "Flat or falling outward"
    is asserted for bands D and E only, with 5% slack a level (finding below). The profile test
    takes every class of bands A–C with 300 members or more in one open-core and one collapsed
    globular. The headroom is asserted over every owned cell for three clusters (fast) and on the
    axes, the fullest count plus eight standard deviations under 8,192, for every cluster of three
    feature cells and fifty globulars (slow: 686 clusters, 23 minutes; 2 over the target, the
    fullest 4,191). T23's 50 ly brute-force comparison in a globular is slow; a 12 ly one in an
    open cluster is fast. Every cluster model costs seconds (below), so the tests share their
    interiors.
  - _Plan 10 (P10.T7.a)_ reads `grid_width` "with that floor" of 16–64 ly for dwarf cores; as
    built the floor is the constant `GRID_WIDTH_MIN_LOG2` and the interior is tied to
    `FeatureRecord` and `ClusterModel`, so P10.T7 needs a floor argument.
- **Findings of phase 5 (for a ruling; nothing built on them).**
  - _Cost of a cluster model._ `ClusterModel::from_record` takes 3.3–4.5 s per open cluster at the
    dev profile's opt-level 2, nearly all of it T9.b's sixteen `speed_bin_shares_against` kick
    quadratures; the class table takes 9–12 ms, the grid's width 2–4 ms, every expected count of
    a grid 40–100 ms and every member 0.1–0.65 s. T23's 50 ly cold query in the core of a
    1.4 × 10⁴ M☉ globular (target 20 ms) takes 12.7 s for its 7,846 members, 3.8 s of it the
    census, which builds the interior once and the hits a second time; a query in the disc visits
    every cluster whose reach touches it, a nursery's 1,750 ly included. A cached model (P09.T40) or a cheaper
    retention quadrature is needed before the server merges this source.
  - _Counts rising outward below the turn-off._ With η = 1 (Design note 9), a class of `q < 1`
    falls as `r^(−3q)` outside its core, band A's (0.3 against 0.8 M☉) as `r^(−1.1)`, against the
    brainstorm's "falls as r⁻³ or faster outside its core". The 10⁶-system globular's fullest cell
    per level in band A is 7, 9, 38, 154, 499, 358, 4, 32 (levels 0–7); bands B and C rise too
    (26 → 91 and 30 → 135 over levels 1–4). The index stays safe (the width rule sees it), but
    the brainstorm's argument does not hold for the low-mass classes.
    The science check (source not re-checked here): Heinke et al. 2005's `(1 + x²)^(−3q/2)` (their
    eq. 2) is fitted near the core at q ≈ 1.6; in multi-mass King models (Gieles and Zocchi 2015,
    MNRAS 454, 576, eqs. 20 and 29) a light class leaves the isothermal regime early and falls as
    about `r^(−2.5)` (isotropic) to `r^(−3.5)` (Michie) outside `r_h`, so the `r^(−1.1)` is an
    extrapolation of the fit. Suggested for the ruling: an outer exponent of at least 5 ⁄ 4 below
    the turn-off, or larger cores for the light classes with an outer slope of at least 3.
  - _Young clusters' tails (P09.T9.g)._ A nursery of 60 M☉ (241 M☉ at birth) expects 1.87 × 10⁴
    members, nearly all in its tails out to its 1,750 ly reach (5,000 or so after the cut at its
    grid's 512 ly): more members than it ever had, since the tail's line density is the
    mass-loss rate over a drift at the central dispersion, a fraction of a km/s. At most
    (241 − 60) ÷ 0.5 ≈ 360 stars can have left it, some 50 times fewer. The science check suggests
    capping a tail at `r_t + v_drift × age` and its total at the mass lost less the remnants and
    runaways already counted (Küpper et al. 2010, MNRAS 401, 105, not re-checked), and notes that
    a nursery's loss by gas expulsion leaves at about 1 km/s in every direction, not as a tail.

- **P09.T9.b's retention after P11.T1.d (round 9b, `fates`; a finding for the orchestrator).**
  Plan 06's companion-stripped mark is now read against plan 11's stripped share re-derived from
  ruling 81's direct construction (0.433 over neutron-star progenitors, against ruling 123.3's
  0.25–0.33), so more neutron stars take the low kick mode: retention is 31.4% at 100 km/s and
  28.6% at 50 km/s, above ruling 126.3's 18–26% and 15–25% (20 km/s and the open cluster still
  pass). `neutron_stars_are_retained_as_the_brainstorm_says` holds the two measured values
  provisionally, and the doctest reads "a fifth to a third", until plan 11's finding is ruled on.
- **Ruling 137.3, as applied (round 9b, `fates`; tests only).** Ruling 126.3's retention windows
  were stated at w = 0.181 and now shift with the measured w by (w − 0.181)(P_low(< v) −
  P_DM25(< v)). At w = 0.2675 they are 26–34% at 100 km/s, 24–34% at 50 and 10–22% at 20, and
  under 1% in the open cluster: measured 31.4%, 28.6%, 16.2% and 0.43%, all asserted, the
  provisional holds removed. The doctest's "a fifth to a third" stays.

- **Ruling 136 as built (lane `feat09c`, 2026-09-28, at `GENERATOR_VERSION` 14, in the version-15
  batch).** It answers the findings above; where an entry above says otherwise, this one stands.
  - _T15.a (136.1)._ The hot branch is Tang and Wang's eq. 2 on CMB88's Sedov clock: `c_s =
√(5⁄3) C₀`, `v* = (v_PDS^(5⁄3) + c_s^(5⁄3))^(3⁄5)`, radiative if `v* > β c_net` with `v*` in the
    radiative form, otherwise `W = t_PDS v_PDS^(5⁄3) ÷ ((β c_net)^(5⁄3) − c_s^(5⁄3))`, 0.931 `t_c` in
    hot gas. The plan's 0.41 `t_c` is dropped. `ShellWindow` gains `sound_speed()` (`c_s`) and
    `characteristic_time()` (`t_c`); `snr::{standard_normal_cdf, truncated_standard_normal}` are
    `ExplosionEnergy`'s law, now shared. The window table at 3,800 K cm⁻³ is 4.37, 5.28, 8.45, 8.41,
    4.35, 1.91, 0.82 and 0.35 × 10⁵ yr, tested against ruling 136.1's figures to 25%, the switch at
    1.5 × 10⁻³ cm⁻³. The largest window at 10⁵¹ erg is 2.40 Myr at a floor of 300 K cm⁻³ and 2.07 at
    450 (the test's 2.0–2.45).
  - _T16.a (136.2)._ The non-radiative phase adds Tang and Wang's excess over Sedov to Truelove and
    McKee's, `₂F₁(−3⁄5, 2⁄5; 7⁄5; −x)` by Pfaff's transform and 64 terms; `2.5 ₂F₁(1) = 2.8901`
    (tested against their 2.89). Radii at the end of each window: 843, 493, 313, 173, 78, 34, 15
    and 6 ly from 10⁻³ to 10⁴ cm⁻³ (tests: 3–1,000 ly, 800–900 at 10⁻³, 3–10 at 10⁴). A shell in
    the median interior lasts 2.65 × 10⁵ yr without a wall. Over 6,979 of the fixture's bubbles
    within 12,000 ly of the Sun-like point, each shell at a point uniform in its bubble, the median
    shell ends at 9.1 × 10³ yr, 95% of them at the wall (test: median under 3 × 10⁵ yr).
  - _T15.b and T4.b (136.3)._ A nursery's interior density is `truncated_standard_normal(Φ(z), 2)`
    of its drawn normal `z` on the same words (`BUBBLE_INTERIOR_TRUNCATION` = 2), 1.26 × 10⁻³ to
    0.020 cm⁻³. This moves every nursery's `bubble_interior`, which no golden pins; it joins the
    version-15 batch. Caps: bubble 0.570 Myr (test 0.5–0.65), field 3.295 Myr for the fixture,
    3.67 at 300 K cm⁻³ and 3.15 at 450 (test 3.15–3.67).
  - _T16.b and P06.T21.e (136.4)._ `stellar::remnant::neutron_star::WIND_NEBULA_THRESHOLD` is 10²⁸
    W (10³⁵ erg/s) and `PulsarWindNebula::of` reads `has_wind_nebula`; `snr::NEBULA_THRESHOLD` is
    gone. Median nebula 1.45 × 10⁴ yr; plan 06's 10³–10^5.5 yr test passes.
  - _T18.a (136.5–7)._ The seed window is 0.2–1 Type Ia a century (measured 0.25–0.87 over 24
    seeds; the fixture 0.427). The ancient share is 4.1–4.6% of the old populations' layer D and
    0.26% of the young disc's, "no longer a layer-D system" whatever the channel (test 3.5–5%;
    at version 15, 3.6–4.1% and test 3.3–4.5%, ruling 141.6: 1.3 × 10⁻³ × 1.096 M☉ per system ÷
    layer D's 3.4%, 4.17% over a Hubble time; the fixture's rate 0.462, test 0.42–0.53).
    `MIN_DELAY` is plan 06's τ(8 M☉) at solar composition and median draws, 42.55 Myr (asserted to
    0.1%); `A` = 2.162 × 10⁻¹³; 18.6% of delays under 0.1 Gyr, 61.7% under 1 Gyr.
    `draw_floor` and `drawable_rate_per_year` are removed: the draw proposes the delay itself
    above `MIN_DELAY` (the formation history read at the delay) and sets the age at the epoch to
    the delay less the explosion's clock time, so nothing is lost to the draw.
  - _T18.b (136.8)._ The period a thousand years before a merger: median 89.6 s, 5–95% 77.9–118.9
    s (tests: median 80–100, 5–95% inside 70–130).
  - _T18.c (136.9)._ `CHANNEL_SHARES` = 0.53, 0.30, 0.04, 0.13 (Iax from Srivastav et al. 2022).
  - _For T17 (136.9–10)._ Pass the site gas's `[M/H]` to `shell_window`, not the star's: CMB88's
    ζ is the cooling gas's. Iax energies (0.05 × 10⁵¹ erg for SN 2020kyg) lie below
    `ExplosionEnergy`'s 0.40–2.51 and need their own law. `EJECTA_MASS`'s 3 M☉ is CMB88's own
    simulation value. The window reads the drawn phase's density and sound speed (confirmed:
    Kim and Ostriker 2015 find a blast follows the uniform solution in the volume-filling phase).
  - _Goldens._ `catalogue_classes/supernova.golden` re-blessed at 14: the window table, shell
    radii and speeds (136.1–2), the caps (136.1, 136.3), the Type Ia rate and ancient shares
    (136.7's `MIN_DELAY`) and the twelve progenitors (the delay-first draw, `MIN_DELAY`, 136.9's
    shares) moved; the light curves did not.
- **Ruling 139 as built (lane `feat09d`, 2026-09-28, rebased onto b2ceb62 at `GENERATOR_VERSION`
  15).** No committed output a consumer reads moves; `galaxy/features/members.golden` is re-blessed
  at 15 (items 1, 4 and 5 move its members, tails and cells), `cells.golden` does not move, and
  `galaxy/features/retention.golden` is new.
  - _Design note 9 (139.1)._ `ProfileShape::Core` gains `halo` (the cluster's `r_h`) and `outer`
    = `max(0, 1 − 3q′ ÷ 2)`; `ProfileShape::cored(core, q, half_mass)`. Classes with `3q′ ≥ 2`
    skip the factor and are bit-identical. η's label is δ = ½ mass segregation (139.2); plan 15's
    P15.T8.b window is 0.8–1.0.
  - _Finding (139.1, 139.3)._ The ruled factor reaches only `y² ÷ (1 + y²)` of `r⁻²` at `y = r ÷
r_h`: a light class's slope is about −1.8 at 2 r_h (0.8 of −2 plus the core term), and "−2 or
    steeper between 2 r_h and r_t ÷ 2" holds only in the limit. The profile test asserts slope ≤
    `−2 y² ÷ (1 + y²)` and ≤ −1.6 there. For the 10⁶-system globular band A's fullest cell per
    level is 45, 45, 126, 273, 432, 147, 4, 34: the rise from the level at 16 ly (about r_h) to
    the next is ×2.164, over the ruled 2^1.1 = 2.144, so T21's rise test runs from 2 r_h (×1.58,
    ×1.30 and ×1.43 for bands A–C), the range of 139.1's slope test.
  - _Tails (139.4)._ `interior::counts::{tail_window, TailWindow}` (drift speed, length ℓ, τ,
    mass), `TailCount::length`, `TailClass::reach()`. v_drift reads `omega` and `kappa` at the
    cluster's spherical radius, as `tidal_radius` does. μ_ev for a globular's cap is L05's solar
    fit, the only one built. Measured: the 60 M☉ nursery (241 M☉, 69 Myr) at 26,700 ly holds
    146.6 M☉ (the research's 147) over **87.5 ly as 623 members**, against the ruling's "about 70
    ly" and "300–420": the fixture's Ω and κ give `4Ω² ÷ κ² − 1` = 1.27, not the flat curve's 1
    (v_drift 0.38 km/s against 0.30), and the tail's bottom-heavy band shares as built (the
    canonical less the depleted count) a mean lost mass of 0.235 M☉, not 0.35–0.5 (586 in band
    A at 0.213 M☉, 37 in B). The test asserts the ruled formula (mass 140–155 M☉, length = v_drift
    × age, count × mean = mass, 250–1,000 members). 47 Tucanae: v_drift 9.2 km/s, a 1,213 ly tail
    of 3.9 × 10⁷ yr, 5,311 members. Gas expulsion's isotropic loss at about 1 km/s is outside the
    model (Design note 1's single bound mark).
  - _Retention table (139.5)._ `tables::cluster_retention::{SHARES, ORDINARY_BELOW}` (`static`,
    352 rows: 16 masses × 11 [Fe/H] × 2 kinds) from the hyperion-fit task `cluster_retention`
    (slow class, 67 s on three threads), with `kick_bins::{retention_shares, RetentionShares}` and
    `interior::retention::{node_shares, retention_tabulated, retention_from_rows, mass_nodes,
fe_h_nodes, speed_nodes, table_row}`. `ClusterModel::new` reads the table; `retention` stays
    as the reference. Acceptance over 64 off-node points (Chabrier and Kroupa; refitted at 15): within 2 × 10⁻⁵
    absolute where retention is 0.05 or less and 1.95% relative above it, both inside the
    ruling's 0.005 and 3%; the largest absolute difference is 0.013, a black-hole retention of
    0.667 at [Fe/H] +0.05 between the nodes at −0.07 and +0.18, a fate step the linear
    interpolation smooths (ruling 137's trade). A cluster model now builds in 0.2 ms (from 3.3–4.5
    s) and an interior in 14–17 ms.
  - _T23 (139.6)._ `members::KeepInteriors::new(&Galaxy)`, a keeping cache bound to one galaxy
    (it panics on another), so one query's census and hits build each interior once;
    `resolve_member(galaxy, &dyn FeatureInteriorCache, id)`. The bench measures the 50 ly
    globular-core query cold (a fresh `KeepInteriors`) and warm (kept). Measured (release, 8,168
    hits): **cold 4.9 s, warm 4.8 s**, against the warm 20 ms target. The interior is no longer
    the cost: the members are. Band C's 4,831 white dwarfs take 6.5 s of a whole-cluster pass
    and band D's 982 take 1.6 s (1.35–1.6 ms each), against 10 µs a living star:
    `draw_member` builds a `StarModel` per attempt to confirm a remnant class's kind. That
    check, not the model, now bounds a query (for the owner; P09.T10's draw).
  - _Headroom after 139._ The slow headroom test now runs in under a minute (from 23 minutes: the
    model is 0.2 ms). At 941d80b it held 686 clusters, the fullest cell 3,475 (from 4,191) with
    the profiles and tails of 139.1 and 139.4; rebased onto b2ceb62's catalogue it holds 734, two
    over the 2,000 target, the fullest 4,237, under the 8,192 index with eight standard
    deviations to spare.
- **Phase 6, T24–T27 as built (lane `centre09a`, 2026-09-29, on 6cf4b5a at `GENERATOR_VERSION`
  15).** No existing output moves: only `resolve` reads the centre, for IDs it refused before
  (`KindNotGenerated`), and `Galaxy` does not build the centre. New goldens
  `galaxy/features/centre.golden` (profile, potential, the three distribution functions at six
  energies, densities, acceptances, four drawn velocities with their marks, the class counts) and
  `galaxy/features/centre_members.golden` (the hundred innermost members); `rng/tags.golden` gains
  `centre.velocity`, `centre.marks`, `centre.age` and `centre.mass` (scope `System`). New bench
  `benches/centre.rs`: the profile and the three inversions together 40–44 ms (target 50),
  `CentreModel::new` 91 ms (Design note 15's 100), release on a shared machine; one `EnergyGrid`
  serves the three inversions. Files: `galaxy/features/centre/{mod,profile,df,marks,classes,
members,testing}.rs`; `placement/{record,resolve}.rs` (the `CentreMember` origin, the dispatch);
  `features/members.rs` and `stellar/system.rs` (the new origin's attempt; `remnant_fits` and
  `systemic_speed` now `pub(crate)`); `planetary/context.rs` and the server's
  `compute/systems.rs` (a centre member has no system stage yet); `features/interior/retention.rs` (`monotone_cubic` now
  `pub(crate)`). Names and shapes that differ from the sketches:
  - `features::centre::{CentreModel, CentreProfile, TracerProfile, TracerShape, SlopeBreak,
DistributionFunction, EnergyGrid, OrbitMarks, LossCone, CentreClasses, CentreClass,
CentreClassKind, CentreTracer, BuildCentreError}`. `CentreModel::new(&Galaxy) -> Result` holds
    the profile, the three tracers' profiles and distribution functions (`CentreModel::invert`,
    in `CentreTracer::ALL`'s order), the loss cone, each tracer's mean acceptance and the classes;
    `enclosed_mass`, `influence_radius` (bisection where the stars weigh the black hole's mass),
    `loss_cone_radius`, `tracer_profile`, `distribution`, `acceptance`. `as_global_entry()` is not
    built: plan 10's list type does not exist yet. `Galaxy::centre()` is not built either
    (Design note 15): 91 ms in release is some seconds in the debug profile, for every galaxy
    every test builds; `resolve` builds the model per call and T28.b decides.
  - _T24.a._ `CentreProfile::{new(&NuclearClusterParams, black_hole), from_params, density,
systems_per_cubic_ly, stellar_mass_within, enclosed_mass, psi, psi_slope, psi_curvature,
escape_speed, influence_radius, reach}`, radii as `f64` light-years from the black hole and
    `Ψ = −Φ` in (km/s)², zero at infinity, of the black hole plus the whole, uncut law (its total
    is `NuclearClusterParams::mass`, as the potential tables hold it). A tracer's shape is `r^−γ₀
Π (1 + (r ÷ r_k)^α)^(−Δ_k ÷ α)`, normalised to one; the stars' is 1.3 inside 10 ly, 3.5
    outside, α = `BREAK_SHARPNESS` = 4. The enclosed mass and the potential's outer term are
    tabulated at 32 knots a decade over 10⁻⁷–10⁶ ly by 16-node panels, their logarithms cubic
    Hermite in `ln r` between knots through the exact derivatives (within 10⁻⁷ of a direct
    quadrature; a panel from the knot below instead made the three inversions take a second),
    with power-law tails in closed form: not in closed form by pieces, which a smooth break has
    not. The cut at 128 ly is where members end (`REACH`); the potential and the inversion use
    the uncut law, because a sharply cut density has no non-negative distribution function
    either.
  - _T24.b._ `DistributionFunction::invert(&TracerProfile, &CentreProfile)`, and `invert_on`
    with a shared `EnergyGrid`: Eddington's second-derivative form at the energies of 256 radii
    even in `ln r` from 10⁻³ to 10⁶ ly, integrated over radius in 16-node panels to 10⁷ ly with
    `ln r = ln r_j + t²` on the singular panel and the boundary term at 10⁷ ly. `d²n ÷ dΨ²` is in
    closed form from the shape's slope and curvature (`TracerShape::derivatives`). f is cut to
    zero above `E₀ = Ψ(10⁻³ ly)`, which makes the brainstorm's r^−½ core (below). f not positive
    at any node is `BuildCentreError::NegativeDistribution(r)`. The realised density `n_f` is
    tabulated at the grid's radii and 64 inside the core, monotone cubic Hermite (Fritsch and
    Butland) in `ln r` and `ln f`; `density`, `density_direct`, `fraction_within`, `dispersion`,
    `speed_moment`, `mean_over_speeds`, `speed_cdf`, `value`, `cut_energy`.
  - _T24.c._ `DistributionFunction::draw_velocity(&CentreProfile, [f64; 3], &mut Stream) ->
Option<[f64; 3]>`: `1 − w` from `Beta(1, γ₀ − ½)` by inverse transform restricted to energies
    under the cut, accepted on `√w f(E) ÷ (B E^{b−1})`: the plan's `Beta(3⁄2, γ − ½)` with its
    `w^½` moved into the acceptance, exact by the thinning theorem and with no Beta variates.
    `B` is the tabulated running maximum of `f E^{1−b}` times the most the interpolant can
    exceed its nodes. Words on `centre.velocity`: the direction's two, then two per attempt, at
    most 4,096 attempts (`None` after, with a debug assertion).
  - _T25._ `marks::{loss_cone_radius, kepler_pericentre, LossCone, OrbitMarks, mean_acceptance}`.
    `OrbitMarks::apply(&LossCone, position, velocity, &mut Stream) -> Option<velocity>` draws its
    two marks on `centre.marks` first, then rejects inside the loss cone (the Kepler pericentre
    about the black hole alone), then by `exp(−k sin² i)`, then reverses a retrograde survivor's
    velocity on the second mark. `OrbitMarks::OLD_STARS` is k = 0.84 and a reversal share of 0.8
    (provisional, below); `OrbitMarks::YOUNG_DISC` k = 16 and 1 (ours). `LossCone::share` is a
    fixed quadrature, one 16-node panel a decade in radius and the distribution function's panels
    in speed; `mean_acceptance` = the inclination's `∫₀¹ e^{−k(1−μ²)} dμ` × (1 − that share).
  - _T26._ `CentreClasses::new(&CentreProfile, &dyn MassFunction)`: `classes`, `retention`,
    `remnants_per_primary`, `primaries_formed`, `systems`, `mean_system_mass`, `count(kind)`,
    `on_tracer(tracer)`. Classes by band × kind (`Living`, `WhiteDwarf`, `NeutronStar`,
    `BlackHole`) × age component (`classes::age_components()`: Schödel et al. 2020's 80% at 10–13
    Gyr, 15% at 2.5–3.5 Gyr and 5% since 300 Myr, its part under 10 Myr, from −H, on the young
    disc; the spans are ours), four Gauss–Legendre ages each, scaled to the cluster's mass.
    Turn-off and kick law at `CENTRE_FE_H` = +0.3 dex (ours; Z clamps at 0.03). Retention reads
    ruling 139.5's `tables::cluster_retention` per progenitor node, the kept share at the local
    `√(2Ψ)` averaged over the stars' profile inside the reach, the table's 500 km/s held above
    it (0.349 against plan 08's quadrature's 0.359 for the neutron stars; a test holds the two to
    0.02); the low mode on its pair's σ 12 km/s (ruling 126.3). Black holes on 7⁄4 inside 5 ly and
    3.5 outside (`black_hole_shape`), 15 M☉ each; the young disc on r⁻² inside 1.5 ly and r⁻⁵
    outside (`young_disc_shape`, ours), no inner hole. Neutron stars lie on the stars' profile,
    not widened (below). Binaries are not classes here (plan 11).
  - _T27._ `centre::members::{centre_grid, CentrePlacement, CentreProposal, CentreMemberRecord,
resolve_centre_member, resolve_centre_member_with}` and `CENTRE_GRID_{WIDTH,CELLS,LEVELS}`.
    `CentrePlacement::new(&CentreModel)`: `proposal`, `expected_candidates`, `candidate_count`,
    `member_id`, `candidate`, `members_in_cell`, `peak_candidates` (the cells beside the axes; a
    test checks a whole level), `black_hole`. A band's density is `Σ N_c n_t(r) ÷ A_t` over its
    classes; the bound is the nearest corner's, and the eight cells with a corner at the black
    hole propose radially under `B r^−γ` with no softening (`n_f` rises as r^−½ to the centre),
    γ the band's steepest tracer slope and `B` from each tracer's cusp continued inward, which
    bounds its realised density. A candidate reads `member.position` and `member.accept` (as a
    catalogue feature's), `member.cell` for the count, then `centre.velocity`, `centre.marks`,
    `centre.age` (uniform in its component's span) and `centre.mass` (six words an attempt, as
    `member.mass`; a living star below the turn-off at its age, a white dwarf above it, a remnant
    redrawn until its kind fits and its kick is below the local `√(2Ψ)`). `SystemOrigin::CentreMember
{ attempt: u16 }`; population `NuclearDisc` (Design note 20); a centre member's layer is its
    slot's band, the black hole's layer E. The black hole is `CentrePlacement::black_hole()` at
    the origin, at rest, its record's primary mass the black hole's. The centre's feature-level
    list registers no class (T30.b's victims will). `SystemContext::for_system` and the server's system cache
    (`compute/systems.rs`) answer a centre member `KindNotGenerated`: its stars would need the
    centre's composition, and the black hole is no star, so the centre's system stage waits.
  - _Measured at Milky Way values (default mass function unless said), at 15._ T24.a: 6,595
    systems per cubic light-year at 3 ly (3,859 M☉ ly⁻³ over the nuclear disc's 0.585 M☉; 8,116
    under Kroupa's), 4.23 × 10⁷ systems (3.26 × 10⁷ inside 128 ly), 1.6 inside 10⁻³ ly, 5.29 ×
    10⁶ M☉ of stars inside 10 ly against the black hole's 4.30 × 10⁶, influence radius 8.60 ly,
    escape speeds 1,122 km/s at 0.1 ly and 210 at 10 ly. T24.b: the integral of f returns the
    profile to 6.6 × 10⁻⁵ from 10⁻³ to 128 ly; slope −0.509 over 10⁻⁵–10⁻⁴ ly; σ 512.4 km/s at 0.1
    ly (the Kepler cusp's `√(GM ÷ 2.3r)` is 511.9), log slope −0.498 over 0.01–0.3 ly and −0.445
    over 0.3–3 ly; a 0.03 ly core is refused; 64 drawn seeds invert. T24.c: Kolmogorov–Smirnov p
    of 0.047–0.79 at 3 × 10⁻⁴, 0.01, 0.3, 3 and 60 ly. T25: loss-cone share 2.97 × 10⁻⁵, mean
    acceptance 0.5903 (k = 0.84), isodensity axis ratio 0.720, slit rotation 39.2 km/s. T26:
    neutron stars 0.349 retained, black holes 0.847; 1.37 × 10⁵ black holes, 2.37 × 10⁴ inside
    the central parsec; 1.20 × 10⁵ neutron stars, 6.8 × 10⁶ white dwarfs, 5.82 × 10⁷ systems at a
    mean 0.425 M☉; the young disc 9.7 × 10⁴ systems, 6.9 × 10⁴ M☉; 77% of the present mass is the
    old component. T27: the fullest cell and band expects 3,108 candidates (band A, level 9, the
    cell beside the x axis 16 ly out; 4,255 under Kroupa's), the innermost 885 (band A, most of
    them the young disc's); over 16 drawn seeds the fullest is 818–7,790, all under 8,192; of the
    hundred innermost members 86 are the young disc's.
- **Findings of T24–T27 (provisional, for a ruling; nothing built on them moves output).**
  - _Sharp breaks have no isotropic distribution function (T24.a–b)._ Where a density turns
    shallower inward at a sharp break, `dn ÷ dΨ` drops there and Eddington's integral takes
    `−J ÷ √(E − Ψ_b)`, minus infinity just above the break's energy. The 10 ly break as specified
    gives f < 0 near 9.7 ly; so does a sharp turn to r^−½ at 10⁻³ ly, and so would any sharply
    cut density (the 128 ly truncation). As built: the 10 ly break is smooth with α = 4 (ours;
    positive up to about α = 25; it raises the density at 3 ly by 6% over the sharp law, and plan
    02's potential tables keep their sharp law), and the r^−½ core is f's cut at `Ψ(10⁻³ ly)`,
    which keeps the cusp exactly outside 10⁻³ ly and turns it to r^−½ inside. A smooth r^−½ core
    in the profile would stay positive only with a sharpness of 1 or less, spread over more than a
    decade. The brainstorm and T24's text should say which.
  - _7,800 at 3 ly (T24.a)._ 6,595 at 15 (−15%; Kroupa's 8,116, −10%; 6,984 at 14). The
    brainstorm's figure is the measured 1.5 × 10⁵ M☉ pc⁻³ (4,322 M☉ ly⁻³) over the mean system
    mass; 2.5 × 10⁷ M☉ on this law gives 3,859. Held at ±25% of the brainstorm's figures,
    provisional.
  - _Systems inside the reach (T24.a, for T38)._ 22% of the law's mass lies beyond 128 ly, so
    3.26 × 10⁷ systems are inside the grid against the cluster's 4.23 × 10⁷. The budget test
    (Design note 5) must count only what the grid holds, or the reach or the normalisation
    changes.
  - _The centre's own mean system mass (T26–T27)._ The class device counts primaries at their
    own present mass, with no companions (as the clusters' device does, plan 11 adding
    binaries): 0.425 M☉ and 5.82 × 10⁷ systems, against the nuclear disc's 0.585 M☉ (plan 02's,
    companions included) and 4.23 × 10⁷. Every T27 count is 1.38 times what the galaxy's mean
    would give.
  - _The fullest cell (T27)._ 3,108 against the brainstorm's "about 1,400": the brainstorm's
    figure is under the spherical bound, and Design note 13 divides that by the marks' mean
    acceptance, 0.59 (×1.69), and the class device's mean mass adds ×1.38 (above). The test holds
    1,400 ÷ 0.59 to a factor of 1.5, provisional. Over 16 seeds the fullest is 7,790 for a cluster
    of 6.3 × 10⁷ M☉ (the draw's +0.2 dex scatter at 2.3σ): under 8,192, but 8σ over it, which
    the plan asks for. The test asserts under 8,192 and prints the margin. Remedies for the
    ruling: count companions in the class device (×0.73); a flattened bound, the angular factor's
    greatest value `e^(−k/2) I₀(k/2) ÷ A` = 1.16 in place of `1 ÷ A` = 1.69 (×0.69, at the cost of
    a Bessel function Design note 13 avoids); or, as Design note 5 says, narrow the cluster's
    mass range in plan 02.
  - _The innermost cell (T27)._ 885 candidates against "about eighty"; the stars' share is some
    14 (the cusp holds some five systems in the eight cells' octant ball). Nearly all are the
    young disc's: an r⁻² cusp (ours) under k = 16, whose inclination mark accepts 3.2%. The
    hundred innermost members are mostly the young disc's (86). The disc's shape and k are for a
    ruling.
  - _Axis ratio (T25)._ With k = 0.84 the pole's density over the equator's at one radius is
    `e^(−k/2) ÷ I₀(k/2)` = 0.63, an isodensity axis ratio of 0.70 in the 1.3 cusp (the
    brainstorm's derivation; the sample gives 0.720). The plan's "from a sample's second moments"
    gives 0.91 at the same k, because the flattening is the same at every radius and not a
    homoeoid's. The test uses the isodensity ratio.
  - _Rotation (T25)._ The brainstorm gives no reversal share. 0.8 (ours) gives 39.2 km/s in a slit
    of ±1.4 ly over 1–4 pc, against Feldmeier et al. 2014's "amplitude of ∼40 km/s" (§4.3; their
    faint stars reach about 50). The slit's rotation is linear in the share, 24 km/s at 0.5.
  - _Neutron-star retention (T26)._ 0.349 at 15 (0.288 at 14) against the brainstorm's "about a
    fifth" (to a third, 0.13–0.27): plan 06's kick law keeps every low-mode pair at these escape
    speeds. Held at 0.30–0.40, provisional, as ruling 106.4 found for the clusters.
  - _"Widened" neutron stars (T26)._ Not built: Design note 14 has three inversions, and a
    retained neutron star (1.35 M☉) outweighs the mean star, so segregation would narrow it while
    kicks widen it. They lie on the stars' profile.
  - _The young disc (T26)._ "A young few per cent on an inner disc" is read as Schödel et al.
    2020's few per cent formed in the last few 100 Myr, whose part under 10 Myr is the disc:
    0.17% of the mass formed, 6.9 × 10⁴ M☉, against Lu et al. 2013's 1.4–3.7 × 10⁴ M☉ (not
    re-checked). Its shape, k = 16 and reversal share 1 are ours; it lies in the galactic plane
    and turns prograde, while the Milky Way's clockwise disc is steeply inclined to it.
  - _Retention above 500 km/s (T26)._ Ruling 139.5's table ends at 500 km/s; the centre's escape
    speed passes it inside about 1 ly, and the table's value is held there (−0.01 on the neutron
    stars' share). A table to 1,500 km/s would remove it.
  - _For T28 and later:_ `Galaxy` building `CentreModel` (91 ms, Design note 15), or a cache that
    `resolve` and T29's source share; `as_global_entry()` once plan 10's list exists; T29's
    `CentreMemberSource` reads `CentrePlacement`; T30.b registers the victims on the centre's
    list; young-disc members of negative age are placed and must be dropped at the query.
- **The tails' mass against L05 (ruling 142.3; closed by ruling 145).** A tail's members are the
  interior's lost stars times the window's share `w`, so their mass is `w` times the interior's
  lost living mass, not L05's `M_esc`. The gap (70 M☉ of the nursery's 146.6 as built by r142) was
  the currency of `N₀`, not the physics: counted in the classes' own mass per system formed, with
  the history's dynamical survival carried over (ruling 145.1), the interior's lost living mass is
  L05's `M_esc` for a young cluster with no remnants kept, and the nursery's tail carries it within
  3% (below, "Ruling 145 as built").
- **`BOUND_SHARE` against the galaxy's own μ (ruling 145.5; queued, wired, its own bump).**
  `kinds::globular`'s history `M = 0.70 M₀ (1 − t ÷ t_dis)` takes BM03's 0.70, a Kroupa function
  to 15 M☉, while the galaxy's function and fates leave about 0.46 of the mass formed at 12 Gyr.
  Generated globulars' birth masses are then about 1.5 times low, which feeds
  `first_population_share`, the black holes and the retention. A history revision should derive the
  share from the galaxy's function and fates; it moves wired output and needs a `GENERATOR_VERSION`
  bump, so it is not in lane `r145`. Also noted, not ruled: the model reads every depleted slope as
  loss from the canonical −1.5, while Baumgardt et al. 2019 read α ≈ −0.6 in dynamically unevolved
  globulars as a bottom-light mass function, which makes a cluster like 47 Tucanae born heavier
  than they say.
- **Ruling 142 as built (lane `r142`, 2026-09-29, on 6cf4b5a at `GENERATOR_VERSION` 15).** No
  wired output moves: `galaxy/features/members.golden` is re-blessed at 15 (its open cluster's
  band-A cell was a tail cell and is now a living one), and no other golden moves.
  - _Slope (142.1)._ The profile test asserts `s ≤ −2 y² ÷ (1 + y²)` from 0.05 r_h to r_t ÷ 2 at
    r_c = 1.5 ly and at the tight case r_c = r_h, where every light class lies on the bound within
    10⁻⁶, and asserts r_c ≤ r_h first; the ≤ −1.6 from 2 r_h stays. The precondition is also
    asserted over the 165 catalogued globulars (the widest profile core is 0.624 r_h) and over the
    headroom test's clusters. T21's rise test adds the level from r_h to 2 r_h at ≤ 3.2: ×2.167,
    ×1.604 and ×1.730 for bands A–C.
  - _Length (142.2)._ The nursery's v_drift 0.380 km/s and ℓ 87.5 ly, asserted in 0.34–0.42 km/s
    and 80–95 ly with ℓ = v_drift × age.
  - _Count (142.3)._ `ClassCounts` gains `born` and `lost` (living systems per band, net of the
    runaways, with the binary mix in their mass), `TailWindow` gains `lost` (`M_esc(age)`, or
    `m₀ μ_ev(age) − M`) and `share()`. Each band's tail is `w × lost_b` at the lost stars' mean
    mass; a band whose count or mass complement is not positive has no tail, and the fallback
    for a cluster losing no dwarfs is gone. The runaways are not counted as lost (they are plan 08's).
    Measured for the nursery: window 146.6 M☉, w = 1 (τ = age), **155 members of 76.8 M☉, mean
    0.495 M☉** (75/10/11/2% over A–D: 121.0, 15.0, 16.4, 2.8); 216.6 living systems born, 61.5
    kept. Each band's tail equals its lost stars, so tail plus kept is the 216.6 born.
    M4: 2,055 members of 591 M☉ (w 0.0032); Palomar 5: 183 of 49 M☉ (w 0.0096).
  - _Finding (142.3, for a ruling): the nursery's 155 members miss the ruled 180–350._ The
    research's 310 born came from a single-star mean of 0.77 M☉; `N₀ = M₀ ÷ mean_formed_mass`
    takes the galaxy's 1.096 M☉ with its companions, while the classes carry only the scratch
    binary mix (30% at 1.5 times the primary), so `N₀` is about 30% low against the classes'
    masses and the scale K rises to compensate. The test pins 150–160 provisionally.
  - _Finding (142.3, for a ruling): 23 of the 165 globulars have no tail._ Where K lifts a band's
    kept stars above its births, the clamp empties it, and in 26 globulars the living stars kept
    outnumber those born. 47 Tucanae keeps 1.24 × 10⁶ band-A systems of 1.06 × 10⁶ born (K about 3
    against band A's depletion ratio 0.39), and ω Centauri 1.48 times its births; both lose their
    tails (47 Tucanae's was 5,311 members). The cause is the same `N₀` against the classes'
    masses, with a globular's M ÷ M₀ near μ_ev (0.49 for 47 Tucanae) leaving no room for the
    depletion. The tests pin 47 Tucanae's empty tail and at most 23 tail-less globulars.
  - _Retention (142.4)._ `retention.rs`'s module documentation now reads "within 0.005 at or
    below 0.05 and 3% relative above it".
  - _Headroom._ Unchanged: 734 clusters, two over the 2,000 target, the fullest cell 4,237 (a
    core, not a tail); the widest profile core among them is 0.794 r_h.
- **Ruling 145 as built (lane `r145`, 2026-09-29, on 92d663d at `GENERATOR_VERSION` 15).** No
  wired output moves: `galaxy/features/members.golden` is re-blessed at 15, and its 12 changed
  lines are the globular's cell expectations moving in the last bit (the kept classes still hold
  the present mass; only `K N₀` rounds differently). No other golden moves.
  - _Births (145.1)._ `class_counts` builds every class per system formed, then `p_dep` (their
    present mass: living at the depleted slope with the binary mix and runaways, plus the retained
    white dwarfs and neutron stars) and `p_can` (the same with the living stars canonical). The
    new `dynamical_survival(model)` is `s`: L05's `disruption_survival(m₀, age)` for an open
    cluster, `M ÷ (M + Ṁ age)` for a globular. `N₀ = (M − M_BH) ÷ min(s p_can, p_dep)`,
    `K = min(s p_can ÷ p_dep, 1)`, and the classes are `K N₀` times their per-system counts.
    `mean_formed_mass` no longer enters the interior; the classes keep the scratch binary mix.
    Where `s p_can ≤ 0` (a dissolved cluster) `N₀` and `K` are 0. `ClassCounts` gains
    `canonical_per_system`, `depleted_per_system`, `survival`, `scale` and `systems_formed`,
    printed by the tail tests.
  - _Tail share (145.2)._ For a globular `TailWindow::mass` is `Ṁ min(τ, age)` and `lost` is
    `Ṁ age` (both 0 if `Ṁ ≤ 0`), so `share()` is the time share; the test asserts `w = τ ÷ age`
    for the named globulars. Open clusters are unchanged.
  - _Catalogue helper (145.3)._ `testing::CATALOGUE_EVOLUTION_SHARE` = 0.50:
    `t_dis = age ÷ (1 − M ÷ (0.50 M₀))`, `Ṁ = 0.50 M₀ ÷ t_dis`, and no loss at or above 0.50 M₀.
  - _Windows (145.4)._ Measured, all inside:
    - nursery: p_can 0.6236, p_dep 0.4931 M☉, s 0.2912, K 0.368, N₀ 331.7; **265.4 members of
      146.4 M☉ (mean 0.552 M☉)** against the window's 146.6 M☉, by band 196.0/28.3/35.0/6.1;
      326.8 living born, 61.5 kept.
    - 47 Tucanae: p_can 0.3562, p_dep 0.3001, s 0.982, **K = 1**, N₀ 2.83 × 10⁶; w 0.0033
      (τ 3.95 × 10⁷ yr); **2,301 members of 523 M☉, mean 0.227 M☉**; lost by band 6.66 × 10⁵,
      3.29 × 10⁴, 558, 0, 0.
    - ω Centauri: K 0.992, s 0.992, 214 members of 65 M☉ (mean 0.306), w 0.0030: it has a tail.
    - M4: K 0.261, s 0.177, 3,172 members of 936 M☉; Palomar 5: K 0.521, s 0.445, 377 members of
      110 M☉.
    - Catalogue: K ≤ 1 and tail + kept ≤ born in every band of all 165; 31 have K held at 1; the
      8 tail-less are exactly those at or above 0.50 M₀ (AM 1, NGC 2419, Pal 4, Crater, NGC 5024,
      NGC 5824, NGC 6715, Sagittarius II), the research's list; every one with Ṁ > 0 has a tail.
  - _Finding (145.4, for a ruling): 47 Tucanae's band C loses 558 stars._ The ruling says bands
    A–B only. Band C's living stars run from 0.75 M☉ to the 0.85 M☉ turn-off, and the slice under
    0.8 M☉ is depleted, so at K = 1 it loses 558 of its 73,499 born: 0.08% of the lost, about two
    tail members (the research's replica has 749, so the ruling's "A–B only" read the depletion
    as ending at the band edge). The test pins band C at under 1% of its births and 0.2% of the
    lost, and none above band C.
  - _Tests (145.6)._ The catalogue test asserts "Ṁ > 0 ⇒ tail" and M ≥ 0.50 M₀ for every
    tail-less globular (at most 8); the 142.3 pins (155 members, 23 tail-less, 47 Tucanae's empty
    tail) are gone. The "L05 gap" Risk is closed; ruling 145.5 is queued in the Risks.
  - _Headroom (145.6)._ Re-run: unchanged, 734 clusters, two over the 2,000 target, the fullest
    cell 4,237 (a core, not a tail), the widest profile core 0.794 r_h. The remnants test at the
    0.50 helper: 5,397 pulsars (3 capped), 12,631 black holes, 0.218 core-collapsed.
- **Ruling 144 as built (lane `centre09b`, 2026-09-29, on d5330c7 at `GENERATOR_VERSION` 15;
  points 1, 2, 4, 5a and 6–10).** Only the centre's two unwired goldens move
  (`galaxy/features/centre.golden`, `centre_members.golden`), re-blessed at 15, and `resolve` of a
  centre ID; no tag is added and nothing wired moves (ruling 142.3's precedent). Files:
  `features/centre/{mod,profile,df,marks,classes,members}.rs` and `benches/centre.rs`.
  - _144.1, α = 10._ `BREAK_SHARPNESS` = 10 for the stars and the black holes' copy of the break;
    `TracerShape::nuclear_cluster_with(params, α)` and
    `CentreProfile::from_shape(shape, mass, black_hole)` keep α a parameter for the joint
    revision. At α = 10 f dips near 8 ly, where Fritsch and Butland's monotone slopes for `ln f`
    go flat and left 4.2 × 10⁻⁴ in the density's integral against T24.b's 10⁻⁴ (parabolic slopes
    1.9 × 10⁻⁴): `ln f` now takes five-point slopes (the derivative of the Lagrange quartic through
    the nearest nodes), and the sampler's bound is each energy panel's exact maximum of its cubic
    (the extrema are a quadratic's roots) in place of the monotone interpolant's slack; `ln n_f`
    takes the profile's exact slopes from the cut radius out. The integral now returns the
    profile to 4.7 × 10⁻⁵ integrated and 4.5 × 10⁻⁵ tabulated.
    `no_drawn_centre_fails_its_inversion` also inverts clusters of 10⁶, 2.5 × 10⁷ and 6 × 10⁷
    M☉ (144.5b's cap) about black holes 0.76 and 1.5 dex either side of 4.3 × 10⁶ M☉ (±2σ and
    ±4σ of the M–σ scatter): all positive.
  - _144.2 and 144.4, T24.a's test._ `the_milky_way_s_cluster_has_the_observed_masses`: ρ(1 pc)
    1.146 × 10⁵ M☉ pc⁻³, M(<1 pc) 8.47 × 10⁵, M(<3 pc) 5.37 × 10⁶, M(<3.9 pc) 7.60 × 10⁶, 78%
    of the law inside 128 ly; 5.85 × 10⁷ systems at the classes' own mean of 0.423 M☉ (7.04 ×
    10⁷ under Kroupa's at 0.351), 8,709 systems per cubic light-year at 3 ly (10,486 under
    Kroupa's; printed, not tested), 2.13 inside 10⁻³ ly. **Two misses, pinned provisionally:**
    M(<3 pc) is 31% under the ruled 6–10 × 10⁶ (held at 4.5–6 × 10⁶; the recorded miss, from the
    22% of the mass beyond the reach), and ρ(1 pc) is 4.5% under the ruled 1.2 × 10⁵ floor (held
    at 1.1–1.8 × 10⁵): α = 10 moves density from about 3 ly outward, as `prof.py` said, and
    the research's 1.21 × 10⁵ was α = 4's. The joint revision's normalisation inside the reach
    (×1.28) lifts both. The 5.85 × 10⁷ systems sit near the top of 4–6 × 10⁷.
  - _144.4, plan 11._ The note on re-deriving the count with companions is in plan 11's Risks.
  - _144.5a, the flattened bound._ `OrbitMarks` now holds an axis, k, the reversal share, the
    inclination acceptance A (sixteen 32-node panels, which k = 25 needs) and the peak
    `e^(−k/2) I₀(k/2)` (`galaxy::special::bessel_i0e`, once per class; ×(1 + 10⁻⁹)); the bound is
    the nearest corner's density times the peak, ×0.686 for the old stars (1.163 ÷ A against
    1.694) and ×0.114 for the disc (5.58 against 49). **A deviation the ruling did not foresee:**
    under a bound below `1 ÷ A` the inclination cannot stay a rejection after the velocity,
    because `exp(−k sin² i) ÷ g_max` exceeds one near the axis. So the placement's pick now
    weighs each class by its position's own angular factor `g(θ) = e^(−k cos²θ) e^(−a) I₀(a)`
    (an I₀ per candidate and flattened tracer), and the inclination mark draws the velocity's
    direction conditioned on it: the speed is kept and the direction redrawn on `centre.marks`
    (two words each) until the mark accepts, at most 4,096 times, then "no such system". The kept
    direction of `L` about the position is the velocity's azimuth about the radius, independent
    of the speed and of the angle on which the loss cone depends, so the three marks stay
    independent (tested: the factor is the marks' mean over directions at four angles and both
    k; the conditioned `cos² i` is the analytic one). `centre.marks` words: the mark, then
    (direction, mark) per rejection, then the reversal's. The fullest cell and band now expects
    2,199 candidates under the default mass function (3,008 under Kroupa's), band A at level 9
    beside the x axis 16 ly out, at the classes' mean mass: pinned to 2% in
    `the_fullest_cell_stays_under_the_index`.
  - _144.5a, the overflow made visible._
    `CentrePlacement::check_index_headroom() -> Result<(), ExceedCentreIndexError>`, plan 03's
    rule for its own cells (the fullest cell's expectation
    plus eight standard deviations under 8,192), and the clamp in `count_from_mean` now has a
    debug assertion, as `candidate_count_from_bound` has. Why this form: the clamp keeps
    resolving and generating in agreement and belongs to the generator version, so it stays; the
    overflow is a property of the galaxy's drawn cluster, known before any member is generated,
    so it is a typed error at that level rather than a `Result` on every cell's hot path; and a
    debug assertion alone would be a reachable panic for the seeds that fail today. Whoever plays
    a centre calls the check, as plan 04 calls plan 03's (T28.b decides where); nothing calls it
    yet. `CentreModel::from_params` builds the centre without the galaxy, so the seed sweeps
    build no potential. Over 16 seeds none fails (the heaviest, 6.3 × 10⁷ M☉, expects 5,512);
    the slow `few_seeds_centres_overflow_their_index` finds **8 of 512 (1.6%) failing** (clusters
    of 8.6 × 10⁷–1.5 × 10⁸ M☉), inside the research's 2%, and **4 of 512 (0.8%) whose fullest
    cell expects 8,192 or more itself** (up to 13,298 for 1.5 × 10⁸ M☉), which clamp: a real
    truncation for those galaxies until the joint revision's cap (144.5b). Pinned provisionally at
    2% and 1%. Nothing wired moves meanwhile, since nothing plays a centre yet.
  - _144.6, the young population._ Four tracers now (`CentreTracer::YoungIsotropic` added, and
    `CentreTracer::index`, `marks`, `profile`). The burst is `BURST_SHARE` = 6 × 10⁻⁴ of the mass
    formed, uniform over 3–8 Myr, a third on the disc: 2.75 × 10⁴ M☉ formed at Milky Way values
    (1.73 × 10⁴ above 1 M☉), 2.52 × 10⁴ present, 1.19 × 10⁴ disc systems and 2.37 × 10⁴
    isotropic. The disc: `young_disc_shape` r⁻³ (from r⁻² about 0.01 ly, only so its mass
    converges) to a break at 0.5 ly to r⁻⁵, sharpness 4 (ours), cut at 0.1 ly by
    `TracerProfile::with_cut`: the inversion takes the first grid node at or outside the cut,
    0.1027 ly, finds f only from there down (only those energies must be positive), and scales f
    so the realised density holds one. Its marks `OrbitMarks::young_disc()`: k = 25 about
    `milky_way_disc_normal()`, Yelda's (130°, 96°) by Lu et al.'s eq. 8 (checked against
    Paumard et al.'s own vector) through the galactic plane's position angle 31.40° at Sgr A* (the
    IAU J2000 frame) and Wegg and Gerhard's bar angle of 27° for plan 01's +x: (−0.887, −0.325,
    0.329), 70.8° from the galaxy's axis. **Findings:** the model has no Sun, so the bar angle
    places the normal's azimuth, and every galaxy's disc takes the Milky Way's orientation, since
    a per-galaxy draw with the fixture pinned needs a parameter in plan 02 (the joint revision's,
    or later); and the transform puts the disc's `L` 71° from the galaxy's, partly co-rotating,
    where the research note paraphrased Feldmeier et al. as "roughly opposite" (their §5 compares
    line-of-sight patterns of old and young stars). The isotropic young: `young_isotropic_shape`
    r^−2.1 to a break at 1.6 ly to r⁻⁵ (ours), no flattening. The cells beside the black hole
    propose under `DistributionFunction::inner_envelope`: the cusp continued inward for a tracer
    cut at the core radius, or `4π √2 √(R Ψ(R)) ∫ f dE r^−½` (as `n_f ≤ 4π √(2Ψ) ∫ f dE` and
    `rΨ` does not fall outward), whichever holds fewer over the ball. The innermost cell proposes
    42.5 candidates over all bands against the model's own expectation of 15.4
    (`the_innermost_cell_holds_what_the_model_expects`, a quadrature over directions; held to
    1–4 times), not the brainstorm's eighty. **Finding:** the isotropic young now dominate the
    black hole's surroundings, since r^−2.1 is steeper than the old stars' 1.3: 73 of the hundred
    innermost members (inside 2 ⁄ 256 ly) are theirs, 21 the old component's and 3 the disc's
    (its r^−½ tail inside the edge). By an estimate from the shape (not a test), the isotropic
    young put some 1,700 systems inside 0.13 ly (1″), a few per cent of them B stars, which is of
    the order of the few dozen S-stars seen there; Do et al.'s slope, not re-checked, is the
    input to confirm.
  - _144.7, axis ratio._ The stated reason is corrected (the observations are isodensity and
    isophote ratios; 0.91 is the sphere's sampling of a flattened cusp). Isodensity 0.700–0.7002
    at 0.01–4.9 ly. **Projected: a miss beyond 6 ly, pinned provisionally:** 0.720, 0.721, 0.724,
    0.729, 0.736, 0.746, 0.761 and 0.782 at 1–8 ly, against the ruled 0.68–0.76 over 1–8 ly; held
    to 0.76 to 5 ly and 0.80 beyond, until the joint revision's optional k(E).
  - _144.8, rotation._ 40.2 km/s in the slit, window 30–50.
  - _144.9, neutron stars._ 0.354 retained (the quadrature's 0.364), window 0.30–0.40; black holes
    0.851, 2.34 × 10⁴ inside 1 pc.
  - _144.10, history._ `age_components()` is six: 80% at 10–13 Gyr, 15% at 2.5–3.5 Gyr, 3% at
    150–500 Myr and 1% from −H to 150 Myr on the stars' profile, and the burst's two. Their shares
    sum to 0.9906 and are divided by it, which the neutron-star count's check against the formed
    remnants needed. Old stars are 78.5% of the present mass. The retention table's nodes above
    500 km/s wait for its next regeneration, as ruled.
  - _Cost._ Four inversions, not Design note 14's three; the bench is renamed
    `profile_and_four_inversions` and was not run (the plan's 50 ms was for three, at 40–44 ms).
    The centre's fast tests take 31–41 s at three threads on a shared machine (51 s before).
