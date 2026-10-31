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
  `FeatureMemberSource` implements plan 03's `SystemSource`, and pads each feature's cells by its
  members' own speed bound, `FeatureInterior::member_speed_bound() -> KilometresPerSecond`
  (P09.T23.b), which R06.T16.a's census relies on. A `MemberRecord` holds a plan 03
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
  `tidal_radius(m, &PointLy) -> Metres`), the nuclear cluster's `NuclearCluster`, `fields` (population
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
  kind and mode); `displaced::{explosion_site, ExplosionSite}` and the gate
  `RECENT_DEATH_CLAIM: Option<fn(&Galaxy, &ExplosionSite, &SystemRecord) -> bool>`, `None` until
  P09.T35 writes `recent_death_claims` and sets it to `Some` (main's ruling on P08.T13's cost,
  2026-10-15), with `SHELL_WINDOW_CAP` (4 Myr there), which this plan takes over;
  `displaced::ClassTable` with the split between budget-fed `class_weight` and field-fed
  `stay_share`; the zero-weight hypervelocity
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
   range is narrowed in plan 02, not the grid here. _Revised by ruling 144's joint revision (lane
   `pot02`):_ plan 02's mass is the mass inside the reach, which the grid then holds, and its draw
   is capped where the fullest cell expects 7,468 (Risks, "The joint revision as built").
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
   `tables::cluster_dynamics::EQUIPARTITION_EXPONENT`, 1, which is δ = ½ mass segregation and not
   velocity equipartition (ruling 139.2), kept as production by plan 15's ruling
   (decision-p15-t8-blocks). Core-collapsed clusters replace the core by a cusp of drawn slope,
   softened at 10⁻³ ly so it stays integrable and bounded. Internal dispersions use the Plummer
   form with a = r_h ÷ 1.305.
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
    is, which leaves an r^−½ tail inside it and is an equilibrium. _Revised by ruling 144's joint
    revision (lane `pot02`):_ the potential is the black hole's, the cluster's and the galaxy's
    spherical average, not the black hole's and the cluster's alone, and retention keeps the
    cluster's own escape speed.
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
  (candidates keep their streams). **It lands with rendering plan R06's R06.T16.a**, in one
  integration at this task's version (decided 2026-10-05, `decision-r06-t16a-scope.md`). R06's sky
  census, caps and band read the field alone. So the field gives up φ only when the sky takes the
  members in. Otherwise the sky loses φ of each population: about nine in ten of the young disc's
  O and early-B stars, and at least a quarter of its B stars.

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
  126.4; kept by P15.T8.a's ruling), f₀ = 0.06 × retention (Breen and Heggie 2013;
  Antonini and Gieles 2020). It is the solution, at constant mass and radius, of df ÷ dt = −β (1 + ψ₁ f) ÷ t★: black-hole mass is lost at
  β cluster masses per relaxation time, and the relaxation time shortens by 1 + ψ₁ f while black
  holes remain. β = 2.8 × 10⁻³ and ψ₁ = 147 are `BH_LOSS_BETA` and `BH_LOSS_PSI_SLOPE` of
  `tables::cluster_dynamics`, read from there and never written as literals, and with
  `BH_RELAXATION_PREFACTOR` in t★ they are the three constants plan 15's P15.T8.a fits against the
  CMC catalogue (Kremer and others 2020); plan 15 kept them as production (decision-p15-t8-blocks):
  they are Antonini and Gieles's (2020) own fit, β 2.80 × 10⁻³ and a₁ 1.47 (ψ₁ = a₁ ÷ 0.01). The
  decay rate β ψ₁ = 0.41 is derived, not a constant of its own, and with f₀ near 0.05 the black
  holes are gone
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

**Built** (2026-10-27; no output moves): see "P09.T17 as built" under Risks.

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
eight standard deviations must stay under its list's limit, 8,192, or 65,536 at the centre
(P11.T8.e.b), asserted when the list is built, and the count is clamped. Tests: resolution,
canonical IDs (the bits between the list's index and the band zero), and that registering a second
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

#### P09.T23.b Each feature padded by its own speed bound (new; no output moves; before R06.T16.a)

Decided 2026-10-05 (`decision-r06-pad-speed.md`). A member moves at its feature's bulk velocity
plus its own internal velocity (P09.T10).

- The bulk is below min(v_esc, 1,000 km/s). The internal part can add up to the cluster's
  central escape speed for a core member (164 km/s at most over P09.T14's 50 seeds), and
  12.12 times the tidal radius's σ for an uncut tail member.
- So a member can outrun plan 03's `PAD_SPEED` only where its feature's bulk lies within that of
  1,000 km/s. That needs the galaxy's escape speed near 1,000 km/s, which only the central few
  hundred light-years allow.
- No member is known to outrun it, but nothing proves none can.

1. `FeatureInterior::member_speed_bound() -> KilometresPerSecond` is |bulk| plus the largest
   internal speed any member can take, which is the larger of:
   - the cluster's central escape speed: a core member is cut at its local escape speed, which is
     greatest at the centre;
   - for tail members, √(212 ln 2) = 12.1222 times the largest σ a tail class takes at the tidal
     radius, its equipartition factor included. Box–Muller's largest normal is √(106 ln 2) =
     8.5717 (`rng::sample::normal`), an internal velocity is one pair of normals and one more,
     and the maximum can be reached. Compute the factor in code, or round it up (8.58 √2), as
     `galaxy/params/draws.rs` rounds its cap.
2. `FeatureMemberSource::systems_in_sphere` pads each feature, band by band, by `pad_for(t,
max(pad_speed(layer), member_speed_bound))`. Where the bound is under `pad_speed`, the cells
   are today's.
3. `interiors_near` is asked with the sphere padded at `UNBOUND_PAD_SPEED`. A debug assertion
   holds every bound below that.

Tests:

- over the members of 200 features of the fixture, including every feature within 2,000 ly of the
  centre, every member's speed is below its feature's bound;
- P09.T23's brute-force test at t = ±H, for 20 centres, one of them the fixture's feature with
  the largest bound;
- the fixture's features with a bound at or above `PAD_SPEED` are counted and recorded (none
  expected).

Files: `galaxy/features/members/source.rs`, `galaxy/features/members/placement.rs`. Acceptance:
`cargo test -p hyperion-sim features::members`, `just ci`. No GENERATOR_VERSION bump: padding
chooses cells, not output.

_Done (issue #15, 2026-10-14, at `GENERATOR_VERSION` 22): see Risks, "P09.T23.b as built"._

### Phase 6: the galactic centre

_Status (lane `centre09a`, 2026-09-29): T24.a–c, T25, T26 and T27 are built; see Risks, "Phase 6,
T24–T27 as built", and the provisional findings after it. `resolve` now answers a centre ID;
nothing else reads the centre until T28.b. Lane `centre09b` (2026-09-29) applied ruling 144's
points 1, 2, 4, 5a and 6–10 to the centre's own goldens; see Risks, "Ruling 144 as built". The
windows below are ruling 144's. Lane `pot02` (2026-09-29, for version 16) built the joint revision
with plan 02: the law normalised inside the reach and tapered, the full potential's monopole, the
cap; see Risks, "The joint revision as built"._

#### P09.T24 Profile and distribution function

- **P09.T24.a Profile and potential.** `CentreProfile`: the broken power law with inner slope 1.3, a
  break near 10 ly and outer slope 3.5 (Schödel et al. 2014; Gallego-Cano et al. 2018), continued
  inward to 10⁻³ ly, then r^−½, truncated at the grid's reach of 128 ly; mass and break from
  `NuclearClusterParams`; enclosed mass in closed form by pieces; potential of the black hole plus
  the cluster. _As built with the joint revision:_ the law tapered at 100 ly and its mass the mass
  inside the reach; the potential the black hole's, the cluster's and the galaxy's monopole. Tests
  at Milky Way values (ruling 144.2 and 144.4): ρ(1 pc) 1.2–1.8 × 10⁵ M☉ pc⁻³, M(<1 pc) 0.8–1.2 ×
  10⁶ M☉, M(<3 pc) 6–10 × 10⁶ and M(<3.9 pc) 7–11 × 10⁶ (Schödel et al. 2018; Chatzopoulos et al.
  2015), 4–6 × 10⁷ systems at T27's own mean system mass, fewer than three inside 10⁻³ ly, the stars
  outweigh the black hole near 10 ly; systems per cubic light-year at 3 ly printed.
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
  _As built with the joint revision:_ under a piecewise-constant envelope of v f(E) over the energy
  grid's panels instead, which the galaxy's potential needs (Risks, "The joint revision as
  built").
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
- **P09.T28.b Regime and the drift hook** (split in two by decision-galaxy-25-train §2,
  2026-10-27: T28.b.a, the new functions, lands pure; T28.b.b, the two swaps that move output, lands
  behind a galaxy hold for version 25). `regime_of(record)` per design note 12, with the
  influence radius where the enclosed stars equal the black hole's mass. `position_velocity_at`
  replaces the straight line for Kepler-regime systems in plan 08's drift hook, for members and grid
  systems alike, and the grid's loss-cone carve-out. Bump `GENERATOR_VERSION`. Tests: positions at t
  = 0 unchanged for every golden system; at 9.9 and 10.1 ly the two regimes differ by less than 0.01
  ly after 1,000 yr.
  - **P09.T28.b.a Regime and the Kepler state, pure** (no output moves, no bump; after T28.a;
    before T28.b.b, T28.c and T29; decision-galaxy-25-train §2). `motion::Regime` and
    `regime_of(..)` per design note 12: a system is in the Kepler regime if its epoch distance from
    the black hole is inside the influence radius (`CentreProfile::influence_radius`, where the
    enclosed stars weigh the black hole's mass), whatever the query's time.
    `position_velocity_at(galaxy, record, t)`: the `KeplerOrbit` from the record's epoch state (a
    member's own; for a grid system, plan 08's epoch velocity as the initial condition), μ = G
    (M_bh + M★(< r₀)), propagated to t. A pericentre inside the loss cone gives the "no such
    system" verdict as a value, as a member's would. A galaxy without kinematic tables, or a
    `Radial` orbit, keeps the epoch position.
    - **Design note 15's choice is this task's.** The regime needs only the influence radius. The
      task picks how a galaxy carries it: a scalar computed at build from the profile, or a memo
      left out of the derived equality. Either way, no output and no `Debug` digest moves, and the
      fast suite's added time is recorded.
    - **Nothing on the record calls these:** `position_at`, `hit_at` and `resolve` are unchanged.
    - **Tests:** the two regimes at 9.9 and 10.1 ly differ by under 0.01 ly after 1,000 years
      (continuity at the regime's edge); t = 0 returns the epoch state to the bit; an S2-like
      member agrees with T28.a's `KeplerOrbit`; a galaxy without kinematic tables keeps the epoch
      position; the loss-cone verdict on a pinned plunging orbit.
    - **New golden `galaxy/motion/regime.golden`** (the pre-pin of decision-galaxy-25-train §1.5,
      D1). For three seeds, each grid record within 1.5 influence radii: its regime, and today's
      `position_at` at t = ±100, ±1,000 yr and ±H. An extension at the version in force.
    - **Files:** `galaxy/motion.rs`, `galaxy/features/centre/` as the carrier needs,
      `galaxy/mod.rs`, `tests/galaxy_motion.rs`, the golden.
  - **P09.T28.b.b The drift hook and the loss cone** (behind `KEPLER_REGIME_VERSION` = 25 and
    `GalaxyRules::kepler_regime`; rides version 25, else 26; decision-galaxy-25-train §1).
    - **The swap.** `position_at` (through `epoch_velocity`) takes `position_velocity_at` for
      Kepler-regime systems, members and grid systems alike. `resolve` answers "no such system" for
      a grid record whose pericentre falls inside the loss cone.
    - **Under the held arm**, today's code runs.
    - **The bump** is the 25 wiring lane's, which deletes the hold.
    - **Tests:** the tripwire; `regime.golden` unchanged; under `GalaxyRules::every_25_hold()`:
      positions at t = 0 unchanged for every golden system (the plan's test), records inside the
      radius move and records outside it don't, and the loss cone's carve-outs counted and listed;
      the same seed twice.
    - **Census C1:** over the three seeds, at t = ±100, ±1,000 yr and ±H. The movers, the largest
      displacement, every mover finite, and the carve-outs by ID.
    - _Built behind its hold (lane `p09-centre`, 2026-10-29, at `GENERATOR_VERSION` 23, no output
      moved): see Risks, "P09.T28.b.b as built"._
- **P09.T28.c Sphere of influence at pericentre.** A Kepler-regime system's tidal radius is taken at
  its pericentre and is constant in time: 2.7 au for a semi-major axis of 0.01 ly. Test: that
  figure, and about 0.01 ly for a near-circular orbit at 3 ly.
  _Built (lane `p09-secular`, 2026-10-29, at `GENERATOR_VERSION` 23, no output moved): see Risks,
  "P09.T28.c as built"._
- **P09.T28.d The two secular rates.** `propagate` turns the orbit in its plane by two rates × dt:
  the Schwarzschild advance 6πGM ÷ (c² a (1 − e²)) per orbit (`schwarzschild_rate`) and the
  retrograde precession from the enclosed stars (`mass_precession_rate`), from the orbit-averaged
  force of the r^−γ cusp tabulated in eccentricity once per galaxy (Binney and Tremaine 2008, §3.2;
  Merritt 2013, ch. 4, to re-check). Unbound orbits take neither. Bump `GENERATOR_VERSION`:
  positions away from the epoch move, those at the epoch do not. Tests: 12′ of advance per orbit for
  the S2-like orbit; against a direct integration in the smooth potential of black hole plus
  cluster, position error under a tenth of the tidal radius after 1,000 yr at 0.1, 1 and 5 ly; the
  profile test of P09.T31 is the acceptance of the pair.
  - **P09.T28.d.a The rates, pure** (no output moves, no bump; decision-galaxy-25-train §2.2).
    `schwarzschild_rate`, `mass_precession_rate` and the eccentricity table (built once per galaxy)
    land pure. `KeplerOrbit::propagate` keeps the two-body law, and T28.a's `kepler.golden` stays.
    The secular step is a method of its own. _Built (lane `p09-secular`, 2026-10-29, at
    `GENERATOR_VERSION` 23, no output moved): see Risks, "P09.T28.d.a as built"._
  - **P09.T28.d.b The term** (behind `KEPLER_SECULAR_VERSION` = 25; after P09.T28.b.b; restated
    2026-10-30 on research advice). A Kepler-regime state with a bound orbit in the centre's Ψ
    follows that orbit exactly, with the Schwarzschild advance (`schwarzschild_rate`) as the one
    secular rate. T28.d.a's secular step and its averages stay pure and unwired.
    - _The orbit._ From the epoch state, E = v² ÷ 2 − Ψ(r₀) and L, with
      `v_r² = v_r0² + 2(Ψ(r) − Ψ(r₀)) + L²(1 ÷ r₀² − 1 ÷ r²)`, which avoids E's rounding.
      - The turning points r_p and r_a come from a safeguarded Newton solve in a bracket, a fixed
        step count, started from the Kepler orbit's.
      - Hénon's anomaly is `r = r̄ − Δ cos η`. dt/dη and dψ/dη are taken at N = 64 fixed nodes
        η_j = 2π(j + ½) ÷ N and kept as K = 32 cosine terms each, in the form regular at the
        turning points, `Δ ÷ √H(cos η)` with `H = v_r² ÷ (1 − cos² η)`.
      - At t, η solves t(η) = t(η₀) + t by a safeguarded Newton solve with a fixed step count.
        The position is r(η), turned in the plane by ψ(η) − ψ(η₀) + ω̇_GR t; the velocity is from
        the same series. t = 0 returns the epoch state to the bit.
    - _Edges._
      - A near-circular orbit, with Δ ÷ r̄ under a threshold the task measures, takes the
        epicyclic limit, since v_r² cancels there.
      - A state with no apocentre inside Ψ's table keeps the two-body law, which is T28.b.b's.
      - The task measures e ≥ 0.97, where the Fourier series converges slowest.
    - _The carrier._ A Ψ table on `SecularTable`'s pattern: the black hole's 1/r exact, the rest at
      32 knots a decade. It is built once with the galaxy's `CentreMotion`, behind `GalaxyRules`,
      under T28.d.a's lazy-value notes.
    - _What follows._
      - At every radius, the true orbit is no faster than the Kepler orbit about μ = G M(<r₀), and
        its radial range lies inside that orbit's. Both follow from M(<r) ≶ M(<r₀) on either side
        of r₀, to the dark halo's share of about 10⁻³.
      - So P09.T29's shells and pads, T28.b.b's loss-cone verdict and T28.c's radius, all taken on
        the Kepler orbit, stay valid bounds and stay as built. The verdict then errs towards
        carving out, and the radius towards the smaller. The task re-checks this.
    - _Tests._
      - Against a direct integration in Ψ over ±1,000 yr from 0.1, 1 and 5 ly, on a circular orbit
        and four eccentric ones: under a tenth of T28.c's radius, the plan's bound. This replaces
        T28.d.a's pinned misses.
      - A harmonic core about a point mass: the radial period and the apsidal angle against their
        closed forms.
      - S2's 12′.
      - t = 0 to the bit, and the same bits twice.
      - T28.b's continuity at the regime's edge.
      - A bench of `Trajectory::of` and `state_at` against the two-body law's, in
        `benches/centre.rs`.
    - _The hold._ The term in `position_velocity_at` sits behind `KEPLER_SECULAR_VERSION` = 25,
      apart from T28.b.b's hold for attribution, and may re-pin T28.b.b's 25-arm figures by name
      (decision-galaxy-25-train §1.3). The plan's "Bump `GENERATOR_VERSION`" becomes the 25
      wiring's. Its D2 and C1 run with `kepler_regime` on, under `every_25_hold()`; its term
      reaches generated output only through T28.b.b's swap.
    - _Built behind its hold (lane `p09-t28db`, 2026-10-30, at `GENERATOR_VERSION` 23, no output
      moved): see Risks, "P09.T28.d.b as built"._

#### P09.T29 The range query at the centre

`CentreMemberSource`: per level the sphere is padded by the radial plunge Δ(r₀, t) = r₀ − (r₀^(3⁄2)
− 1.5 √(2GM) |t|)^(2⁄3), and everything inside r_full = (1.5 √(2GM) |t|)^(2⁄3) is scanned in full.
Orbits are propagated to the query's time and tested there. The caller may supply a cache of orbital
elements per cell (`CentreOrbitCache`). The same pad applies to grid systems in the Kepler regime.
Tests: r_full of 0.31 ly and about 37,000 systems at a century (42,000 under Kroupa's); a query at t
= ±100 yr equals a brute-force scan of the inner 2 ly; bench: the century query with cached orbits,
target about 10 ms.

_Done (lane `p09-centre`, 2026-10-28, at `GENERATOR_VERSION` 23, no output moved): see Risks, "P09.T29
as built"._

#### P09.T30 The black hole's own events

- **P09.T30.a Flares and disruptions.** On the black hole's event key (`EventKey::derive` with the
  black hole's ID as subject), with plan 06's `PoissonBins`: flares at 1.1 a day above a mean
  unabsorbed 2–10 keV luminosity of 10³⁴ erg/s, their mean luminosity (dN/dL ∝ L^−1.9 on
  10³⁴–1.925 × 10³⁵ erg/s) and duration (dN/dT ∝ T^−0.9 on 400–8,175 s) drawn independently and
  their energy the product (Neilsen et al. 2013, §5 and Table 2), tag `0x0200 CENTRE_FLARE`, in bins
  of one day; tidal disruptions at 2.9 × 10⁻⁵ per year × (M_bh ÷ 10⁸ M☉)^−0.404 (Stone and Metzger
  2016, equation 27 as printed; 1.03 × 10⁻⁴ a year at the Milky Way's mass), tag
  `0x0201 TIDAL_DISRUPTION`, in bins of 1,024 years so that the source horizon is a few hundred
  bins. Both tags are registered in `event_tags!` here, inside plan 06's block for this plan. Tests:
  the same events in any order of asking; 0.06–0.21 disruptions expected in ±H and about 27 (27.31)
  in the horizon at Milky Way values; 401.775 flares a Julian year (1.1 a day) (corrected 2026-10-15
  to the as-built law: Risks, "P09.T30.a as built").

  _Done (issue #20, 2026-10-15, at `GENERATOR_VERSION` 22): see Risks, "P09.T30.a as built"._

- **P09.T30.b Victims and luminosity.** Each disruption inside the source horizon has a victim, a
  feature-level member of class `TIDAL_DISRUPTION_VICTIM`, first in the centre's list order, on a
  near-parabolic orbit whose pericentre passage is the event time; its mass is drawn from the
  centre's living classes and its state after the event is "no system". Its draws start at word 0 of
  its disruption's event stream, where T30.a draws none, so no T30.a ID or time moves. Each victim
  has an outcome, disrupted or swallowed whole: swallowed when the black hole's mass is at or above
  the victim's Hills mass, where r_t = 2 r_g (Stone and Metzger 2016, §2 and near equation 33, §4.2;
  1.14 × 10⁸ M☉ for a 1 M☉, 1 R☉ star). The outcome is recorded on the event, never dropped, and
  only a disrupted victim adds a tail (updated 2026-10-15). `luminosity_at(t)` = quiescence +
  flares + the t^(−5⁄3) tails of the disruptions in the horizon, by looking back over their bins
  with this task's own look-back (T30.a's disruptions look back 0 bins). Tests: a victim resolves
  from its ID and is at pericentre at the event time; a Sun-like tail follows its closed forms; at
  Milky Way values the centre is at or below Ponti et al.'s 10³⁶ erg/s at the epoch in most seeds;
  the Kepler members' loss cone stays empty; at Milky Way values every main-sequence victim is
  disrupted (updated 2026-10-30 by P09.T30.c, from "0.3–3 × 10³⁹ erg/s a thousand years after a
  disruption (Ponti et al. 2010)": the light echoes measure short outbursts, not a tail).

  _Done (lane `p09-victims`, 2026-10-29, at `GENERATOR_VERSION` 23, no output moved): see Risks,
  "P09.T30.b as built", and the findings after it._

- **P09.T30.c The centre's quiescence** (added 2026-10-30: the research lane `research-p09-t30b`'s
  advice on P09.T30.b's findings 1 and 2, §1 and §2, as main ruled it on 2026-10-29). A disrupted
  victim's tail radiates `ε Ṁ c²`, at most L_Edd, with an efficiency that falls with its fallback
  rate `ṁ = Ṁ ÷ Ṁ_Edd`, `Ṁ_Edd = 10 L_Edd ÷ c²`: Xie and Yuan's (2012, MNRAS 427, 1580, Table 1,
  equation 11) hot-flow fit at δ = 0.5, α = 0.1 and s = 0.4, `ε = ε₀ (ṁ ÷ 10⁻²)^a` with
  `(ε₀, a)` = (1.58, 0.65), (0.055, 0.076) and (0.17, 1.12), each range between where it meets
  its neighbours (2.8802 × 10⁻⁵ and 3.3929 × 10⁻³), and η = 0.1 from where the top range reaches
  it (6.2265 × 10⁻³), so that ε is continuous and non-decreasing. The tails do not end, and
  `peak_luminosity_watts` takes the same efficiency at the peak rate. The law moves
  `galaxy/features/centre_luminosity`, so it is built behind a hold of its own in `GalaxyRules`
  (decision-galaxy-25-train §1): `CENTRE_QUIESCENCE_VERSION` = 25 with its tripwire, and a field
  whose `of_version(25)` arm is the new law, which `every_25_hold()` includes. Tests: at 23 no
  golden moves; under every 25 hold only `centre_luminosity` moves, its moved figures recorded; a
  Sun-like tail follows its closed forms, without Ponti's bracket; over T30.b's 200 seeds at Milky
  Way values the centre is at most 10³⁶ erg/s at the epoch (Ponti et al. 2010) in at least half
  the seeds under 25's rules, and under 23's law in 1 of 200, pinned. Recorded as gaps, not
  built: the echoes' short outbursts; partial disruptions' steeper t^(−9⁄4) tails; a 2–10 keV
  share that should fall at very low rates (Sgr A*'s own C_X is about 190–380, not 15.8).

  _Done (lane `p09-t30c`, 2026-10-30, behind its hold at `GENERATOR_VERSION` 23, no output moved):
  see Risks, "P09.T30.c as built", and its gaps._

#### P09.T31 Centre tests

Slow tests. Members sampled in six radial shells from 0.01 to 100 ly keep the profile at t = −1,000,
−100, +100 and +1,000 yr (Kolmogorov–Smirnov per shell, and the flattening unchanged);
eccentricities thermal (mean 0.6–0.72); the brightest-band members inside 0.04 ly include S2-like
orbits (periods of 10–20 yr occur); mean spacing about 0.05 ly at 3 ly.

### Phase 7: catalogue classes and the merge

#### P09.T32 The catalogue-class grid

**Built** (2026-10-27; no output moves): see "P09.T32 as built" under Risks.

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

**P09.T33.a built** (2026-10-29; no output moves): see "P09.T33.a as built" under Risks.

**P09.T33.b not built** (2026-10-29): it waits on P08.T12's displaced fields; see "P09.T33.b not
built" under Risks.

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
  travels more than 4,096 ly from its site within its lifetime. (Bound by P08.T13's interval
  predicate: see "From P08.T13: the interval predicate is part of the one shared test", after
  P09.T35; main's ruling, 2026-10-15.)
- **P09.T33.b Runaway and walkaway deaths.** A second site density in the same class: plan 08's
  runaway and walkaway classes of band E, whose members die away from their birthplace, with the
  death rate conditional on the class's ejection-age marks. The grid side of these classes is carved
  in P09.T35. Tests: their share of field core collapses against the closed form from
  `RunawayModel`; complementarity for sampled runaway candidates. (Bound by P08.T13's interval
  predicate: see "From P08.T13: the interval predicate is part of the one shared test", after
  P09.T35; main's ruling, 2026-10-15.)

#### P09.T34 The Type Ia class and the ancient survivors

**P09.T34.a built** (2026-10-30; no output moves): see "P09.T34.a as built" under Risks.

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
  visited at |t| = H rises by under a third. Through P08.T17's `escape_cut_holds`, the raise needs
  no change of its own anywhere else:
  - plan 12's lensing walk moves layer D off the escape envelope, to `UNBOUND_PAD_SPEED`;
  - R06's census pads layer D's cells, and its bound before the drift, at 3,000 km/s.

  Tests, added: `escape_cut_holds(Layer::D)` is false; P08.T12.d's `every_exempt_class_is_padded`
  passes with the class's weight; and P08.T17's debug assertion holds over the 100 query centres'
  survivors (decided 2026-10-05, `decision-r06-pad-speed.md`).

#### P09.T43 The luminous blue variables

**Field half built** (2026-10-31; no output moves): see "P09.T43 as built" under Risks. The band-E
runaways' sites (after P08.T12, with P09.T33.b), the feature-level list entry (after P09.T36.a) and
`lbv_claims`'s wiring (P09.T35) wait.

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

Plan 03's `catalogue_claims` computes `displaced::explosion_site` for every layer-E record,
field or displaced, whose death time falls within (−(`SHELL_WINDOW_CAP` + L + H), +H], backing a
displaced candidate out to its birth site along a straight line, once a claim is set. This task
writes `recent_death_claims(galaxy, &site, &record)` (the record's `SystemStars`, `DeathMarks::of`,
`claims(galaxy, &site, &marks, ShellEnvironment::Field)`) and sets `RECENT_DEATH_CLAIM` to
`Some(recent_death_claims)` in its bump commit (main's ruling on P08.T13's cost, 2026-10-15). Only
records that pass plan 08's prefilter pay for a stellar evaluation. A claimed candidate resolves to
"no such system". Runaway classes do the same, and the luminous blue variables' test of P09.T43 is
asked beside it. Layer D's share loses `ancient_share`; recent Type Ia need no carve-out here
because plan 11's binaries redraw any explosion before +H, which this task records as a requirement
in plan 11's consumed interface and guards with a `debug_assert` hook. Bump `GENERATOR_VERSION`,
regenerate goldens. It requires `LifetimeBracket` baked as a `hyperion-fit` table, P08.T13's second
follow-up, which lands before P08.T12.c or this task, whichever is first. It measures the sim's fast
suite before and after setting the claim, and benches the hook at the Sun-like point, on a young arm
ridge and at the centre (with P08.T16, which should bench an arm ridge too). Tests: the
complementarity test of P09.T38 for sampled candidates; a grid star due to die at +1,500 yr is still
a grid star, dies in place and has a shell when evaluated then; and a placement golden that pins a
layer-E cell holding a death the claim takes, so that setting the claim cannot land without moving
a golden: check that one does, or add one, such as the cell (200, 203, 0) that
`resolve_refuses_a_record_whose_recent_death_is_claimed` (`galaxy/placement/resolve.rs`) finds
(galaxy plan 08's Risks, "Deferred findings (2026-10-13 rule) (P08.T13.b)").

> **From P08.T13: the interval predicate is part of the one shared test** (main's ruling,
> 2026-10-15).
>
> Plan 08's `displaced::death_in_interval(m0, &composition, eta, age_at_epoch, draws)` decides
> whether a primary's death falls in (−(`SHELL_WINDOW_CAP` + L + H), +H]. It decides in three
> tiers: plan 08's `LifetimeBracket`, then the fate table at the range brief's margin, then the
> exact `lifetime`. Its `None`s rest on validated bounds, not proved ones. So:
>
> 1. **Every side calls it.** That means:
>    - the grid's `catalogue_claims`, through `explosion_site`;
>    - P09.T33.a's field candidates;
>    - T33.b's runaways and walkaways;
>    - T36.a–b's members and the centre's.
>
>    Each calls `death_in_interval` on its own candidate's primary marks. It keeps an entry only if
>    that is `Some` and `claims` is true. No side tests T against the interval by any other route.
>    A catalogue candidate whose T was drawn inside the interval, but which the tiers put outside,
>    is dropped, because the grid keeps that system.
>
> 2. **The inputs belong to the version.** From T35's bump on, the predicate's inputs are
>    generator-version items: the fate tables, `LifetimeBracket` and its widening, and
>    `STATED_FACTOR` and `DEAD_MARGIN_FLOOR`. A change to any of them is a bump, and re-runs
>    P08.T13's soundness tests.
> 3. **A failed bound costs a shell, never a double count:** the system stays on the grid without a
>    shell. P08.T13's slow test bounds the rate below 3 × 10⁻⁴ per death in the interval: at 95%,
>    about two of the Milky Way's roughly 7,000 shells. P09.T38 records the measured figure beside
>    the shell counts.
> 4. **P09.T38 tests the reject path.** Its complementarity test includes the predicate's reject
>    path, forced through a test-only margin, and checks that both sides agree there.
> 5. **The same rule binds P09.T43.** Its prefilter on the record alone binds its catalogue side in
>    the same way.

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
  members. (Bound by P08.T13's interval predicate: see "From P08.T13: the interval predicate is part
  of the one shared test", after P09.T35; main's ruling, 2026-10-15.)
- **P09.T36.b Globulars and the centre.** Globular clusters have no core-collapse entries and no
  shells; their Type Ia hosts are drawn from the delay-time distribution on their mass (0.1–0.5
  expected over the whole system) and are listed as hosts whose window is zero. The centre's list
  takes a handful of core collapses from its young class and its Type Ia from its old classes, and
  the centre's band-D and band-E cells carve them by the same test, with the smooth gas at the
  origin as the site. Tests: a handful (1–20) of entries at the centre at Milky Way values; no
  globular entry ever reports a shell. (Bound by P08.T13's interval predicate: see "From P08.T13:
  the interval predicate is part of the one shared test", after P09.T35; main's ruling, 2026-10-15.)

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
recorded against the brainstorm's 2–4 Myr and the 4.3 Myr of the oldest known remnant. (Bound by
P08.T13's interval predicate: see "From P08.T13: the interval predicate is part of the one shared
test", after P09.T35; main's ruling, 2026-10-15.)

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
  query over the WebSocket inside a globular returns members with their origin. Every row carries
  `origin` from the build that registers the features' source, for the variants that the
  registered sources place (plan 04's reserved list names `SystemOriginWire`, `CatalogueClassWire`
  and `MemberClassWire` growing). `CatalogueClassWire` comes from `ClassId` by a match on the
  registry's constants, and a test walks `ALL_CLASSES` and finds a distinct wire string for each,
  in value order. `PROTOCOL_VERSION`'s documentation and `features.rs` name the three as growing
  (`just gen-protocol`). Rendering plan R06's R06.T16.a reads only the features' part: `FeatureCellCache`, `ClusterModelCache`,
  `FeatureMemberSource` registered, and `FeatureGas` passed. That part may be built ahead of the
  centre's and the catalogue classes' sources, which wait on P09.T29 and T37 (decided 2026-10-05,
  `decision-r06-t16a-scope.md`).
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
  state. Component tests for the lists and readouts. An origin, a catalogue class or a member
  class this build does not know reads Missing, and the readout's other rows read as before (plan
  04's reserved list): `toChartSystem` maps the wire's origin to the model's with an explicit
  unknown arm, so the readout's switches stay exhaustive. Tests: a row whose origin is
  `{ "type": "pinned" }`, one whose `catalogue_system` class is `"magnetar"` and a `listed` member
  class of `"magnetar"`, each cast at the test's boundary, read Missing where they are unknown,
  keep the row, and change no other reading.

#### P09.T42 Client: the centre's fine radius steps

The query-radius selector and the scale bar run down in 1-2-5 steps to 0.01 ly and 0.001 ly, the
readout gives distances in light-years to four decimals or in AU below 0.01 ly, and the census line
is shown at every step. A `BLACK HOLE` readout shows mass and luminosity at the chart's time. Tests:
selector steps, unit switching, formatting under the guide's number rules.

_Done (lane `p09-t42`, 2026-10-31, at `GENERATOR_VERSION` 24, client only, no output moved): see
Risks, "P09.T42, records"._

#### P09.T44 Client: a core-collapse remnant's nebula rows

Added 2026-10-21 by main's decision on a young neutron star's `NEBULA RADIUS NONE`, from P12.T11's
landing look, where next to a Vela-age pulsar it read as no nebula at all, though such a star
almost always has a supernova shell. The rows are the planetary nebula's, which comes only from a
star that leaves a white dwarf, so both system readouts, `GALAXY`'s `STAR A` and the `SYSTEM`
display's host readout, leave out `NEBULA RADIUS` and the four rows after it (`NEBULA AGE`,
`NEBULA EXPANSION`, `IONISED MASS`, `EXCITATION CLASS`) for a `neutron_star` or `black_hole` host,
as they are left out for `no_remnant`: the guide's Not applicable state. This amends the
orchestrator's ruling 149.4's "`NEBULA RADIUS` always" for those two kinds only; `EVENTS` stays, and
a white dwarf and a living star keep the rows, with `—` or `NONE`. P09.T41.c's remnant rows (the
shell's radius, phase and emission, and the pulsar wind nebula) are to take the place. Files: under
`apps/hyperion/src/renderer/src/`: `displays/galaxy/PrimaryReadings.tsx`,
`displays/system/HostReadings.tsx`, tests; the guide's `NEBULA RADIUS` row. Tests: Vitest for a
neutron-star and a black-hole host in both readouts, and a white dwarf and a Sun-like star keeping
the row. Acceptance: `pnpm test`, `just ci`. No `GENERATOR_VERSION` change.

_Done (issue #74, 2026-10-22, at `GENERATOR_VERSION` 22): see Risks, "P09.T44 as built"._

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

This plan changes generated output five times, each a bump with regenerated goldens: P09.T2.c (the
field gives up φ), P09.T28.b (the Kepler regime and the grid's loss cone), P09.T28.d (the secular
rates, away from the epoch only), P09.T30.c (the disruption tails' falling efficiency, ruled
2026-10-29), P09.T35 (the supernova and luminous-blue-variable carve-outs, layer D's ancient share
and the hypervelocity class's weight). T28.b, T28.d and T30.c ride version 25 (S1), each behind a
`GalaxyRules` hold until the bump, and T35 and T2.c (with rendering plan R06's R06.T16.a) ride 26
(S2); each boards a later train if not ready (decision-bump-plan-after-22, 2026-10-14; updated
2026-10-15). It reserves, so that later
rates, away from the epoch only), P09.T35 (the supernova and luminous-blue-variable carve-outs,
layer D's ancient share and the hypervelocity class's weight), and P09.T13's bound share (ruling
145.5). T28.b and T28.d ride version 25 (S1), and T35, T2.c (with rendering plan R06's R06.T16.a)
and the bound share ride 26 (S2); each boards a later train if
not ready (decision-bump-plan-after-22, 2026-10-14; updated 2026-10-15). It reserves, so that later
plans move nothing: the `ClassId` values 2, 3, 5, 6 and 8 for plan 11 and the rule that
feature-level lists append classes in the list order of design note 16; event tags 0x0203–0x02FF of
this plan's block; the `10` sub-kinds `01` and `10` for plan 10 (`11` stays rejected);
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
  to the brainstorm's owner. **Ruled 2026-10-26:** the centre's list index alone is widened to 16
  bits (`[15:0]` under band 7, `[31:16]` zero), an extension that moves no existing ID, built by
  plan 11's P11.T8.e.b (plan 11's Risks, "The centre's feature-level index, ruled").
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
  direct integration decides whether the second rate needs a correction beyond 0.3 ly. _(As built,
  T28.a compares with a two-body integration only; the comparison in the smooth potential of black
  hole plus cluster is T28.d's test, and waits for lane `pot02`'s centre potential. See "P09.T28.a
  as built".)_
- **Grid systems inside the influence radius** are not drawn from the distribution function, so
  their density is not exactly stationary. They are about one in a hundred of the systems there and
  the effect is confined to the inner 1.5 ly over the clock window. _(As built in P09.T28.b.b,
  under its hold's new arm: 78.6% of the sampled Kepler-regime grid records are unbound about the
  mass inside their epoch radius, and they move up to 4.63 ly from their straight lines over ±H
  across the whole regime, not the inner 1.5 ly alone; see "P09.T28.b.b as built", "Unbound
  orbits".)_
- **Scratch constants** (helium) ship as defaults and change with plan 15's tables, each a version
  bump. The cluster-dynamics three (black-hole loss, equipartition, pulsar counts) are production
  at their scratch values (decision-p15-t8-blocks, no bump), with plan 15's findings on the closed
  form, the profile's form and Γ's scale recorded there. The Type Ia samplers are P15.T9.a's
  fitted `tables::type_ia_delay` since version 15.
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
  _(T17's names reconciled at `e8b5c6f3`: see "P09.T17 as built".)_ Phases 2–8 otherwise consume
  only what phase 1 now provides.
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
- **`BOUND_SHARE` against the galaxy's own μ (ruling 145.5; built behind `BOUND_SHARE_VERSION` = 26, below).**
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
- **P09.T28.a as built (lane `centre28a`, 2026-09-30, on d9a24a6 at `GENERATOR_VERSION` 15).** No
  output moves: nothing reads the propagator until T28.b wires it into the drift hook, and it reads
  no potential, so none of its figures waits on `pot02` (μ is the caller's). New golden
  `galaxy/motion/kepler.golden` (twelve states from circular to e = 5.8, 0.001–9.5 ly, at six times
  over ±1,000 yr), unwired. Files: `galaxy/motion.rs` (new), `galaxy/mod.rs`,
  `tests/galaxy_motion.rs`.
  - _Signatures._
    `KeplerOrbit::from_state(PointLy, GalacticVelocity, GravitationalParameter) -> Result<KeplerOrbit, BuildKeplerOrbitError>`:
    position relative to the black hole in ly (what `CentreMemberRecord::local_position` gives), velocity and μ in SI; refuses non-finite values,
    μ ≤ 0, a position at the black hole (`AtCentre`) and exactly zero angular momentum (`Radial`,
    which includes a galaxy without kinematic tables: T28.b must keep such a system on its epoch
    position). `propagate(Seconds) -> KeplerOrbit`, the state `dt` on as an orbit in its own
    right. Getters `position`, `velocity`, `distance`, `gravitational_parameter`,
    `specific_energy_j_per_kg`, `angular_momentum_m2_per_s`, `eccentricity`,
    `pericentre() -> LightYears`, `semi_major_axis`, `period` (the last two `None` unless bound). `schwarzschild_rate`
    and `mass_precession_rate` are T28.d's.
  - _Method._ Universal variables with the Lagrange coefficients (Battin 1999, §4.5; Vallado 2013,
    algorithm 8), solved by Laguerre–Conway (n = 5) for a fixed `UNIVERSAL_ITERATIONS` = 10
    (at most seven needed to 10⁻¹⁴ over the stress set); a bound orbit's `dt` is first reduced to
    within half a period. An unbound or parabolic orbit starts from the least of the parabolic
    cubic's root, (for e > 1.01) the value its hyperbolic anomaly gives, and √μ τ ÷ q: the line
    √μ τ ÷ r₀ alone started beyond the root of a nearly parabolic flyby, where each step gains
    only 1 ÷ √−α, and diverged.
  - _S2's elements._ The test takes S2's measured a = 0.0163 ly (125.058 mas at 8,246.7 pc) and
    e = 0.8846 (GRAVITY Collaboration 2020, table E.1), which give 15.97 yr, 119.0 au and 7,771
    km/s at the Milky Way fixture's 4.297 × 10⁶ M☉, held to 16 ± 0.1 yr, 120 ± 2 au and 7,800 km/s
    ± 1%. The plan's a = 0.0158 ly (1,000 au) and e = 0.88 give 15.24 yr, 5% short of 16; the test
    pins that too. The plan's text should read 0.0163 ly and 0.8846.
  - _Tolerances._ The S2-like orbit is held to the plan's 10⁻¹² relative for energy, angular
    momentum and the round trip at every time over ±1,000 yr. The stress set (e from 0 to 100,
    pericentres 10⁻³–10 ly) holds the energy to 10⁻¹² of the larger of its kinetic and potential
    terms (a nearly parabolic orbit's energy is near zero) and the angular momentum to 10⁻¹² of
    |r| |v| (far out on a hyperbola r × v is a difference of products a million times |h|); worst
    measured 1.3 × 10⁻¹⁴ and 4.6 × 10⁻¹⁴. Its round trips are bounded by the problem's
    conditioning: 10⁻¹² × (1 + orbits completed) ÷ (1 − e) for bound orbits (worst some forty times
    inside), and 10⁻¹² × (r ÷ q)² for unbound and nearly parabolic ones (worst 1.5 × 10⁻¹⁴).
  - _Domain._ The stress set's hyperbolae stop at e = 10. At e = 100 and a pericentre of 10⁻³ ly
    (v∞ of 16% of c, far outside anything the centre places) the solve back from 160 ly out does
    not converge in ten steps, and the universal equation's root is defined only to about 10⁻⁷
    there; not pursued (deferred).
  - _Status._ T28.a done (its tests and golden pass; `just ci` in the lane's report). Not started:
    T28.b (`Regime`, `regime_of`, `position_velocity_at`, the drift hook and loss-cone carve-out,
    bump), T28.c (tidal radius at pericentre), T28.d (`schwarzschild_rate`,
    `mass_precession_rate`, the direct integration in the true potential, bump), T29–T31. All wait
    for `pot02`'s centre potential (v16).
- **The joint revision as built (lane `pot02`, 2026-09-29, on 0500394 at `GENERATOR_VERSION` 15,
  for version 16's batch; ruling 143.1 with ruling 144's "Joint revision").** Plan 02's side is its
  R26 (the cluster's law and mass in the potential, the nuclear disc's inner part, the cap); this
  plan's side is below. Files: `features/centre/{mod,profile,df,marks,classes,members,testing}.rs`,
  `benches/centre.rs`, and plan 03's `placement/headroom.rs` with plan 13's
  `substellar/abundance.rs` (the partition below).
  - _The law and its mass (144.1, 144.3)._ `TracerShape::nuclear_cluster()` (no parameters now:
    plan 02's constants) is the α = 10 break at 10 ly to 3.5 and the taper at 100 ly, sharpness 4,
    to 5.5; `nuclear_cluster_with` is gone. `CentreProfile::mass()` is the whole law's, the
    parameter over its 0.9680 inside the reach (2.556 × 10⁷ M☉ at the fixture), and
    `mass_within_reach()` the parameter's; `from_shape`'s mass is the mass inside the reach too.
    The black holes' shape takes the same taper, so that the grid holds the same share of them.
    **T24.a at the ruled windows, all met:** ρ(1 pc) 1.515 × 10⁵ M☉ pc⁻³, M(<1 pc) 1.120 × 10⁶,
    M(<3 pc) 7.097 × 10⁶, M(<3.9 pc) 1.004 × 10⁷; the provisional pins of "Ruling 144 as built"
    (1.1 × 10⁵ and 4.5–6 × 10⁶) are gone. 5.83 × 10⁷ systems inside the reach at the classes'
    mean of 0.424 M☉ (6.03 × 10⁷ in the whole law; 7.03 × 10⁷ inside under Kroupa's at 0.352),
    tested inside the reach; 11,471 systems per ly³ at 3 ly (13,820 under Kroupa's), printed.
    **A miss, pinned provisionally:** under Kroupa's function 3.39 systems lie inside 10⁻³ ly,
    over the brainstorm's "fewer than three" (2.81 under the default), since the normalisation
    raised the cusp by 28%; held under 3.5 for Kroupa's.
  - _The potential (the joint revision's last item)._ `CentreProfile` holds the galaxy's monopole,
    plan 02's `SphericalAverage` of the discs, bar, bulge and dark halo, when built for a galaxy
    (`from_params`, which builds the mass model, or `new(..).in_galaxy(model)`, which
    `CentreModel::new` uses with the galaxy's own); `from_shape` and `new` alone stay the black
    hole and the cluster, for tests of other laws. `psi`, `psi_slope` and `psi_curvature` are the
    whole potential's; `cluster_psi` and `cluster_escape_speed` the black hole's and the cluster's,
    which the retention quadrature and the members' remnant draws read (144's "retention keeps
    cluster-only escape"). The escape speed from the cluster is 1,136 km/s at 0.1 ly and 236 at 10
    ly at the fixture (the brainstorm's 1,100 and 210, on the heavier cusp), and from the galaxy
    1,378 and 815. Members at 30 and 100 ly now move with σ 57.5 and 48.5 km/s, where the cluster's
    potential alone gave 49.3 and 26.4.
  - _Two devices the full potential needed (engineering)._ **An outer energy cut:** in the
    galaxy's well the members' energies are a thin slice just under Ψ at the centre, so f is also
    zero below `Ψ(r_out)`, `r_out` the last grid radius inside `df::OUTER_CUT` = 8,192 ly (64
    reaches); f is found and must be positive from the core cut down to it only, and the realised
    density is tabulated to it and zero beyond. At 1,024 ly the cut's renormalisation moved the
    density inside the reach by 5 × 10⁻⁴, over T24.b's 10⁻⁴; at 8,192 ly the integral of f returns
    the profile to 4.8 × 10⁻⁵ (4.2 × 10⁻⁵ in the cluster's potential alone). **A new velocity
    sampler:** the Beta proposal in `v² ÷ v_esc²` failed 4,096 times at 30 ly in the galaxy's
    potential and took 15.8 µs a draw at 3 ly; it is now a piecewise-constant envelope of `v f(E)`
    over the energy grid's panels (each panel's greatest f, its cubic's extrema, times the greatest
    v on it), picked and placed by one uniform and accepted by one mark: 1.2–1.9 µs a draw at
    10⁻²–100 ly in either potential, two words an attempt as before. The plan's Beta proposal is
    retired (T24.c's text).
  - _Headroom (144.5b)._ The fullest cell expects 2,896 candidates under the default mass function
    and 3,964 under Kroupa's at Milky Way values (band A, level 9, 16 ly out; ×1.32 from 2,199 and
    3,008, the normalisation); `the_fullest_cell_stays_under_the_index` pins them to 2%. Plan 02's
    cap puts the capped cluster's fullest cell at 7,418 under the default and 7,458 under Kroupa's
    (`the_cap_s_centre_fills_its_fullest_cell_to_the_limit`, new, 97–100% of 7,468, headroom
    passed). The fast sweep of 16 seeds passes; the slow sweep, renamed
    `no_seed_of_512_overflows_its_centre_s_index`, finds no seed failing and none clamping, where 8
    failed and 4 clamped at version 15; 21 of the 512 (4.1%) are at the cap, whose fullest cell
    expects 7,417.
  - _Re-measured (144.6–9)._ The projected isophote ratio is 0.720, 0.721, 0.724, 0.729, 0.736,
    0.746, 0.761 and 0.782 at 1–8 ly, as before (the taper at 100 ly moves nothing inside 8 ly):
    **still a miss beyond 6 ly, pinned provisionally** as "Ruling 144 as built" held it (0.76 to 5
    ly, 0.80 beyond), since 144.7's optional k(E) is not built. The slit's rotation is 44.0 km/s
    (40.2 before; the members move faster in the galaxy's potential), in 30–50. Neutron-star
    retention is 0.380 (0.354 before; the heavier cusp raises the cluster's escape speed), in
    0.30–0.40; black holes 0.880, 2.91 × 10⁴ inside 1 pc (2.34 × 10⁴), in 1–4 × 10⁴. The innermost
    cell proposes 48.7 candidates against the model's expected 17.1 (42.5 and 15.4 before), within
    the test's four times.
  - _Plan 03's headroom check, and the rogue planets' threshold (a consequence)._ The nuclear disc's
    inner part is holed (plan 02, R26), and a holed disc's bound over plan 03's single root octant
    is its amplitude at the centre, four and a half times its densest ring's: the fixture's layer A
    then read 80,896 candidates under Kroupa's function against 65,536 for cells that expect far
    fewer. `check_index_headroom` now takes the largest bound over a partition of the root cube, in
    each octant the layer's 8 × 8 × 8 cells about the origin and then shells of seven boxes of
    doubling edge (`placement::partition`, 4,656 boxes for layer A), which bounds every cell just as
    surely (every cell lies in one box, and a bound grows with its box) and is the densest cell's
    own bound where it matters: the fixture's fullest layer-A cell expects 20,608 (about 6,000
    before the inner part; Design note 6 of plan 03), in its ring, and the check passes under both
    mass functions and for 48 drawn seeds under each (a lane sweep, not a committed test; the
    densest ring 108 systems per ly³, under Kroupa's function at a 238 ly nuclear disc). Plan 13's
    saturation threshold is C over the same partition's greatest total bound. **A finding for the
    owner:** the inner part's ring holds some 50 systems per ly³ near 30 ly (the ACES's measured
    total there, with the cluster, is about 66 M☉ ly⁻³), so the fixture's 30 rogue planets per
    system pass the rogue layer's 992.5 per ly³ there: the partition's greatest bound on the systems
    is 68.5 per ly³ (84.2 under Kroupa's), so the saturation threshold falls from 37.5 to 10.09 per
    star (31.8 to 8.46 under Kroupa's), under the default 21, and the fixture's ring saturates; the
    brown dwarfs' fullest cell rises from some 20,000 to 74,036 of 2¹⁹. By ruling 125 those cells
    saturate in place and nothing else moves, but the brainstorm's "the Milky Way's does not reach
    it" no longer holds; the tests are pinned provisionally to the measured state.
- **The monopole's density check beyond 10⁵ ly (P11.T4.h, 2026-10-04).**
  `the_table_holds_the_gaussians_mass_and_density` takes its finite difference beyond 10⁵ ly
  Richardson-extrapolated at h = 4 × 10⁻³, with the 10⁻⁵ tolerance kept: at h = 10⁻⁴ it sat at its
  rounding floor and failed on P11.T4.h's last-bit change to the galaxy's parameters. See plan 11's
  Risks, "P11.T4.h as built".
- **The envelope bounds' tightness check at its rounding floor (P11.T4.h, 2026-10-04).**
  `envelopes_never_exceed_their_bounds` (`tests/galaxy_bounds.rs`, plan 02's P02.T8.a) allows the
  exponent's rounding, 4 |ln corner| ε, beyond its 10⁻¹² in the normal range, and a disc's
  2⌈n0⌉ subnormal units beyond its 64 in the subnormal range, in the tightness half only (safety
  stays exact; `bounds.rs` is untouched). The same change to the galaxy's parameters took it to
  1.08 × 10⁻¹² at ln corner = −634 and to 81 units for the nuclear disc at 3.5 × 10⁻³¹¹. See plan
  11's Risks, "P11.T4.h as built". Since 2026-10-12 a disc's 2⌈n0⌉ applies in both halves (plan
  02's Risks, "P02.T8.a's subnormal check, either way").
- **P09.T6's budget test takes P11.T7's carved classes (lane slow-fix-tests, 2026-10-12, at version
  21; the slow-pass triage's T2).** Test-only: no generated output moves.
  - `field_and_features_add_up_to_every_budget` (`tests/galaxy_features.rs`) failed in slow passes
    1A and 2A, as it had since ad16be12 (on `origin/galaxy-generation`: bin5b, P11.T7).
    `FeatureShares::field_factor` became 1 − φ − `class_share`, the carved binary classes that plan
    11's catalogue will hold (P11.T8), while the test still summed only the field, the members and
    the globulars. It fell short by exactly the class share, the same in every population:
    1.6 × 10⁻⁷ in band A, 1.8 × 10⁻⁶ in band B (the first to fail, the young disc at seed 0), 4.8
    and 7.7 × 10⁻⁴ in bands C and D, and 1.8 × 10⁻⁵ in band E.
  - The sum now adds `budget × share × class_share(population, band)`, and the test has a doc
    comment. The tolerance stays P09.T6's 10⁻⁶ relative. The carved share is the value the field
    factor subtracts, so it adds only rounding. The error left is the members' quadratures': at
    worst 5.1 × 10⁻¹², the young disc's, in every band at all three seeds; the other populations
    reach 4.7 × 10⁻¹³ at most. The young disc's ten age bins read 1 − φ(age) and are unchanged;
    they reach 2.9 × 10⁻¹¹ (bins 7–9, the triage's run). The carved share itself is checked by
    `shares.rs`'s `the_field_features_and_carved_classes_add_up_to_the_budget`.
  - It passes by name (`--cargo-profile slow-test --run-ignored only`).
- **P09.T23.b as built (issue #15, lane `lane/15-p09-t23b-feature-pad`, 2026-10-14, at
  `GENERATOR_VERSION` 22).** No output moves and there is no bump: the padding chooses which cells
  are visited, not what they hold. No golden is added, and every golden passes unchanged,
  `galaxy/features/members` among them. Files: `members/placement.rs`, `members/source.rs`, and
  one `pub(crate)` constant in `members.rs`.
  - _The bound._ `FeatureInterior::member_speed_bound() -> KilometresPerSecond` is computed once
    in `FeatureInterior::of` and kept in a field. It is (|bulk| + max(core, tail)) × (1 + 10⁻⁹).
    - "The cluster's central escape speed" is read as the value of `member_velocity`'s cut at the
      centre, `ClusterModel::escape_speed_at(0)` = √(2GM ÷ a) of the cluster's Plummer sphere,
      the most the cut takes anywhere. It is not `ClusterModel::escape_speed_central`, the fitted
      `central_escape_speed` that retention reads: on the sample's fastest feature the fit is the
      larger, 0.334 against 0.268 km/s.
    - The tail term is 8.58 √2 = 12.134 (`members::INTERNAL_SPEED_SIGMAS`, rounded up from
      √(212 ln 2) = 12.1222) times σ at the tidal radius times the largest `equipartition_factor`
      over the bands' tails, in the table cut at the grid's reach. It is 0 without a tail.
    - Its doc comment states the limit: the feature-level list's classes (P09.T30.b, T36 and T43)
      are not covered, and a task that registers one with its own velocity law extends the bound.
  - _Rounding._ Both: the 8.58 factor for the tail term, and a relative margin of 10⁻⁹ on the sum
    (`placement::SPEED_BOUND_MARGIN`). The margin was applied from the start, not after a test
    tripped: the core term is the cut's own value, unrounded, while a member's speed in m/s is the
    norm of a component-wise sum.
  - _The reach._ `source::band_reach(radius, t, layer, bound)`, a private pure function, is the
    radius plus `pad_for(t, max(pad_speed(layer), bound))`: today's reach, bit for bit, where the
    bound is the lower. `interiors_near` is asked with the sphere padded at `UNBOUND_PAD_SPEED`
    whatever the layers asked (it was the fastest asked layer's pad). The debug assertion that
    every bound lies below `UNBOUND_PAD_SPEED` sits in `FeatureInterior::of`, so that it checks
    every interior any caller builds, not only those the source looks up. The census is unchanged.
  - _Fast tests,_ added to `the_member_source_finds_and_counts_what_a_scan_finds` on its one
    fixture build. The bound of the tests' open cluster and globular equals |bulk| plus the larger
    term recomputed from the model's getters (to the margin), is above every member the fast tests
    enumerate (the open cluster's 1,647; the globular's busiest band-C cell, 7) and is below
    `UNBOUND_PAD_SPEED`. Open cluster: 236.359 km/s = 230.260 + max(core 1.445, tail 6.099);
    globular: 170.816 = 165.392 + max(1.773, 5.424). The tail term is the larger in both.
    The open cluster's query asked for band C alone now equals the scan's band-C hits too, which
    checks the lookup without layer E, the one whose radius grew.
    `a_feature_s_reach_is_its_layer_s_unless_its_bound_is_faster` needs no fixture. The scan now
    runs over a member list enumerated once (`every_member`), so the open cluster's comparison and
    the slow globular-core test each enumerate their cluster once for both times.
    `cargo test -p hyperion-sim features::members`: 15 passed, 5 ignored, in 18.1 s.
  - _The sample_ (`source::tests::sample`, for the slow tests). It takes every feature with an
    interior within 2,000 ly of the galactic centre, from `FeatureCatalogue::near`, then features
    from `walk_process` up to 200: a third each from the globulars, the old open clusters (the
    sub-discs in turn) and the nurseries. The inner set alone is 225 features: 7 globulars, 104
    old open clusters (89 of the first sub-disc, 10 of the second, 4 of the third, 1 of the fourth)
    and 114 nurseries. So the sample is the inner set, and the walk adds none.
  - _"The fixture's features"_ (the count, and "the fixture's feature with the largest bound" among
    the ±H centres) are read as the sample's, not the whole catalogue's. A feature farther out has
    its bulk below the local escape speed (the pad-speed ruling of 2026-10-05). That is at most
    715.9 km/s beyond 2,000 ly, the value in the plane at 2,000 ly, where the flattened potential
    is deepest. The sample's largest internal term is 69.7 km/s, and the ruling's largest central
    escape speed over P09.T14's 50 seeds is 164 km/s, so no feature outside the sample is expected
    near `PAD_SPEED` either.
  - _The run-time rule_ is issue #15's brief's: time the member test once, alone and capped; at 20
    minutes or less keep every member; above that keep every member of the inner set, the open
    clusters, the nurseries and the feature with the largest bound, and take the other globulars'
    bands A and B whole and bands C–E in every eighth owned cell, recording the cut.
  - _Slow tests,_ each run once on the slow-test profile, alone under a 6 GB memory cap, on the
    shared 8-thread laptop at a 1-minute load of 1–4:
    - `every_member_of_the_sampled_features_moves_below_its_feature_s_bound`: all 2,731,509
      members of the 225 features, every band and owned cell, move below their feature's bound.
      The largest ratio of a member's speed to its bound is 0.999089, in FFG F201, an old open
      cluster of 228 members bound at 314.209 km/s. 1,179 s (19.6 minutes), within the run-time
      rule's 20, so no member is cut; the cut would have thinned only globulars outside the inner
      set, and the sample has none, so the test has no thinning. The seven globulars hold
      2.04 × 10⁶ of the members, FGF F2's 768,514 the most, and most of the time.
    - `the_sampled_features_bounds_stay_below_the_unbound_pad_and_the_fast_ones_are_counted`: none
      of the 225 at or above `PAD_SPEED`. The largest bound is 688.505 km/s, of GGF F43, an old
      open cluster of the first sub-disc with 136 members: bulk 685.996, core 0.268, tail 2.509
      km/s. 7.0 s.
    - `queries_at_plus_and_minus_h_find_what_a_scan_finds_in_the_20_fastest_sampled_features`: at
      t = ±1,000 years the source's hits of each of the sample's 20 fastest features equal the
      scan's. Their bounds run from 464.6 to 688.5 km/s; they are 18 old open clusters and the
      globulars GFG F3 and FGF F4. The sphere is the half-mass radius, centred a quarter of it off
      the feature's centre, not P09.T23's 50 ly: its edge cuts the cluster, so that members cross
      it over ±H, where the bulk alone carries the fastest some 2.3 ly. It holds 5 to 40,774
      members a query; the test asks that 18 of the 20 centres find members, and all 20 do. 581 s,
      nearly all of it the two globulars' 237,000 and 321,000 members. The 20 centres are a slow
      test of their own; the fast comparison at 0 and −600 years is unchanged.
    - The existing `a_50_ly_query_in_a_globular_s_core_is_a_brute_force_enumeration` passes in
      18.8 s, and placement's `no_cell_and_band_of_the_fixture_s_clusters_nears_the_index` in
      14.1 s with its recorded figures (734 clusters, 2 over the target, the fullest 4,237).
- **Deferred findings (2026-10-13 rule)**, from P09.T23.b's review:
  - _No golden pins the member source's hits._ That a padding change moves no hit is held only by
    the scan comparisons, so a later edit to the padding or the lookup could change which members
    a query returns without any golden failing. A small golden of the shared open cluster's query
    hits, all layers and band C alone at −600 years, would pin them: a new label at the version in
    force, with no bump. This task adds no golden, by its brief.
- **P09.T30.a as built (issue #20, lane `lane/20-p09-t30a-bh-events`, 2026-10-15, at
  `GENERATOR_VERSION` 22).** No output moves and there is no bump (the bump plan after 22, ruled
  2026-10-14: "P09.T30.a | No (new event tags) | No bump"): the task adds two event tags and the
  kinds on them and changes nothing that existed. `rng/tags` gains exactly two lines at its end, and
  `galaxy/features/centre_events` is a new golden at header 22. Files:
  `galaxy/features/centre/events/{mod,flares,disruptions}.rs`, `centre/mod.rs` (the module
  declaration and one line of its doc), `rng/tags.rs`, `id/event_tags.rs`. Nothing reads the events
  yet; P09.T30.b and plan 12 will.
  - _API._ `features::centre::events::subject() -> EventSubject`, the black hole's own ID (Design
    note 23).
    - Flares: `CentreFlareModel::{new(seed), of(&Galaxy), series(), listed_rate()}`, a `RateModel`
      whose bound and rate are both 1.1 a day; `CentreFlare` (`Copy`) with `id`, `onset`, `end`,
      `duration` (a `Span`), `duration_seconds`, `mean_luminosity_watts`, `energy_joules`,
      `peak_luminosity_watts`, `is_active_at(t)` and `luminosity_watts_at(t)`;
      `flares(&model, window, out)`, `flares_active_at(&model, t, out)` (the look-back's onsets
      filtered by duration) and `flare(&model, id) -> Option<CentreFlare>`;
      `flares::peak_over_mean()`; and the constants of `flares`: `BIN_SECONDS`, `LOOK_BACK_BINS`,
      `RATE_PER_DAY`, `MIN_MEAN_LUMINOSITY_WATTS`, `MAX_MEAN_LUMINOSITY_WATTS`,
      `LUMINOSITY_SLOPE`, `MIN_DURATION_SECONDS`, `MAX_DURATION_SECONDS`, `DURATION_SLOPE`,
      `MAX_DURATION` and `MAX_PEAK_LUMINOSITY_WATTS`.
    - Disruptions: `TidalDisruptionModel`, a `RateModel`, with `new(seed, black_hole_mass)`,
      `of(&Galaxy)`, `series()`, `black_hole_mass()`, `listed_rate()` and
      `expected_count(window)` (the rate times the span, in 128-bit nanoseconds). Its `new`
      returns `BuildTidalDisruptionModelError::InvalidMass` for a mass that is not finite and
      positive, and `BuildTidalDisruptionModelError::RateAboveBinLimit` for a mass below about
      0.56 M☉, where a bin's mean would exceed `PoissonBins::MAX_MEAN_PER_BIN` (see the disruption
      law below); `TidalDisruption` (`Copy`, `Eq`, `Ord`, `Hash`) with `id` and `time`;
      `tidal_disruptions(&model, window, out)` and `tidal_disruption(&model, id)`;
      `disruption_rate(mass) -> EventsPerSecond`, equation 27; the constants
      `disruptions::{BIN_SECONDS, LOOK_BACK_BINS, RATE_AT_PIVOT_PER_YEAR, PIVOT_MASS, MASS_INDEX}`.
    - The plan's `flares` and `tidal_disruptions` keep their names; `luminosity_at` is T30.b's.
      Each model is built from its parts (the seed; the seed and the mass), with `of(&Galaxy)`
      beside it, so that the tests sweep seeds and masses without solving a galaxy's black hole
      (the brief's lean). `of` reads `galaxy.params().black_hole().mass()`, as `black_hole_of`
      does. Luminosities are watts and energies joules, the unit in the getter's name; no unit
      newtype is added.
    - For plan 12's registry rows (P12.T5.a): the flares' `BIN_SECONDS`, `LOOK_BACK_BINS`,
      `MAX_DURATION` and `MAX_PEAK_LUMINOSITY_WATTS`, and `CentreFlare::luminosity_watts_at`; the
      disruptions' bound, duration and light curve are T30.b's.
  - _The tags_ (the brief's ruling of 2026-10-15, item 4).
    `CENTRE_EV_FLARE: Event = "centre.ev.flare"` (hash `0xe60651f1fe53aed5`) and
    `CENTRE_EV_TIDAL_DISRUPTION: Event = "centre.ev.tidal_disruption"` (`0xa1782ed5a9226363`),
    appended after `SYSTEM_ABUNDANCES` under a "Plan 09, phase 6 (P09.T30.a)" heading; `0x0200 => CENTRE_FLARE` and
    `0x0201 => TIDAL_DISRUPTION` after `0x0202 => SUPERNOVA` in plan 09's block, so that the
    registry's order holds. The module doc's parenthetical now reads "(plan 09 registered `0x0202`
    first, in P09.T19.a, and `0x0200` and `0x0201` after it, in P09.T30.a)". A test pins the block
    to exactly `0x0200`–`0x0202` with their names, and
    `plan_09_registers_its_tags_with_their_scopes` covers the two new tags. No `System` tag: a
    kind's marks come from `BinEvent::marks()`.
  - _The flare rate_ (ruling item 2). A constant 1.1 a day above a mean unabsorbed 2–10 keV
    luminosity of 10³⁴ erg/s (Neilsen et al. 2013, ApJ 774, 42, §5: "1.1 (+0.2 −0.1) flares per day
    above a luminosity of 10³⁴ erg s⁻¹"), 401.775 a Julian year, the same at every time and for
    every black hole. The threshold is the luminosity law's floor. The plan's "about 365 flares a
    year" is read as the brainstorm's "about one a day", at the bottom of Neilsen's interval
    (1.0–1.3 a day): the test asserts 1.1 × 365.25 = 401.775 to 10⁻¹² relative. Bins of 86,400 s,
    looking back one; a bin's mean is 1.1 and nothing is thinned.
  - _The flare marks_ (ruling item 3; a deviation from the plan's "power-law energy
    distribution"). Word 0 is the mean luminosity, dN/dL ∝ L^−1.9 on [10³⁴, 1.925 × 10³⁵] erg/s
    (Table 2: Γ = −1.9 +0.3 −0.4), and word 1 the duration, dN/dT ∝ T^−0.9 on [400, 8,175] s
    (Table 2: −0.9 ± 0.2), each by inverse transform of its truncated law, independently. The
    energy is their product, E = L̄ × T, since the mean luminosity is the fluence over the duration
    (§3), so the energy distribution is a product of two power laws. The reasons, as ruled: §5's
    "independence of the flare intensity on duration"; §4.2's correlation coefficients (the fluence
    with the peak rate ρ ∼ 0.89 and with the duration ∼ 0.54, the peak rate with the duration
    ∼ 0.27), which independent L̄ and T reproduce; and a rate stated above a luminosity, which an
    energy-first law cannot hold. The energy-first law is not built. The tops of the two ranges are
    where §4.3's test of the fits' extrapolation starts, "at our highest bins" (the paper does not
    call them bin edges), just above the brightest flare's 1.92 × 10³⁵ erg/s (Table 1) and the
    longest's about 7,800 s (§3); 400 s is a selection limit, "our shortest allowed time" (§3).
    Scatter added later takes words 2, 3, … in a fixed order.
    - _Checks of the built law_ against the ruling's figures.
      `flares::tests::the_law_reproduces_neilsen_et_al_s_figures` computes them from the built
      constants and asserts the share above 10³⁵ erg/s and its interval, the duty cycle, the energy
      range, the share above 10³⁸ erg, the peak ratio and the bound; it prints the mean duration
      and the daily rate above 10³⁸ erg. The Poisson interval and the dN/dE index are the
      science-checker's, recomputed by hand for this record.
      - above 10³⁵ erg/s, a share of 0.0603, one flare in 15.08 days, against Neilsen's "every
        ∼11.5 days" (§5), three flares in 34.7 days, whose 90% Poisson interval is one in 4.5–42.5
        days;
      - a mean duration of 2,750.0 s and a duty cycle of 3.501%, against §4's 3.5% (the ruling's
        2,752 s is the same law, rounded differently);
      - energies from 4.0 × 10³⁶ to 1.574 × 10³⁹ erg, which hold Table 1's L × T, from
        1.15 × 10³⁷ (ObsID 13853) to 1.05 × 10³⁹ erg (ObsID 14392);
      - above 10³⁸ erg, a share of 0.2530, 0.278 a day (the ruling's 0.254 and 0.279), against
        Ponti et al.'s (2015, MNRAS 454, 1525) bright-or-very-bright 0.26 ± 0.05 a day in 2012
        (Table 3) and 0.27 ± 0.01 over 1999–2014 (§5.4);
      - a least-squares index of dN/dE over 10³⁷–10³⁹ erg of −1.53 on a log-uniform grid (the
        ruling's −1.52; Table 2's fluence fit: −1.5 ± 0.2).

      The science-checker's closed forms reproduce every figure independently; the small
      differences from the ruling's are rounding and fitting method, not the law.

  - _The light curve_ (lean, the brief's §2.2 item 2). Neilsen §3's flare shape, a Gaussian in
    time whose start and stop are its 2σ limits. It is centred at the onset plus T ÷ 2 with
    σ = T ÷ 4; its value at ±2σ, e^−2 of its amplitude A, is subtracted, so that it is continuous
    and 0 outside the flare; and it is scaled so that ∫ ΔL dt = E. Its integral is 1.851235 σA and
    its peak A (1 − e^−2), so the peak is 1.868298 times the mean luminosity (`peak_over_mean`) and
    the bound is 3.596474 × 10²⁸ W, which `MAX_PEAK_LUMINOSITY_WATTS` holds rounded up at the fifth
    figure, 3.5965 × 10²⁸ W (3.5965 × 10³⁵ erg/s). It is ΔL in the 2–10 keV band, in watts, not a
    factor: the black hole has no quiescent luminosity until T30.b.
  - _The duration's cap._ `MAX_DURATION` is the law's top, 8,175 s. A `const` assertion holds it
    under the day's look-back, and `flare_of` asserts each flare's duration under it in debug
    builds, so `flares_active_at` finds every flare still running.
  - _The disruption law_ (ruling item 1; a technical correction). Stone and Metzger's (2016, MNRAS
    455, 859, §4.1) equation 27 as printed, Ṅ = 2.9 × 10⁻⁵ (M• ÷ 10⁸ M☉)^−0.404 per Julian year,
    the whole sample's fit ("our fiducial model uses a power law fit to the entire galaxy sample (B
    = −0.404)"), at the black hole's mass, constant in time, for any finite positive mass and with
    no clamp. The plan's 10⁻⁴ × (M ÷ 4.3 × 10⁶ M☉)^−0.4 is this equation rounded at the Milky Way's
    mass, and is withdrawn.
    - _Technical correction._ The ±H window of the plan's test moves from 0.06–0.2 to 0.06–0.21,
      since its top was the rounding; its bottom, 0.06, is Yao et al.'s observed rate over 2,000
      years and stays. The horizon's "about 26" becomes "about 27 (27.31)".
    - At the fixture's 4.300220 × 10⁶ M☉ the equation gives 1.033874 × 10⁻⁴ a year: 0.20677 in ±H,
      27.309 in the source horizon's 264,144 years and 0.10587 a bin. Per bin it is 0.427 at 1.5
      dex below the Milky Way's mass, 0.484 at 10⁵ M☉ and 0.0187 at 10^8.5 M☉, under 64
      throughout. The 1.5 dex either side is the ±4σ of the M–σ relation's 0.38 dex scatter that
      the draws reach in practice, as `centre`'s `no_drawn_centre_fails_its_inversion` takes it;
      the builder itself validates the scatter to ±9σ, and a draw reaches 8.58σ at most, about
      4 × 10³ M☉ at the Milky Way's dispersion, 1.8 a bin.
    - _Masses below 0.56 M☉ are refused_ (a decision of this build, from the review). There a
      bin's mean would exceed `PoissonBins::MAX_MEAN_PER_BIN`, which the bins cannot list, so
      `TidalDisruptionModel::new` returns `RateAboveBinLimit` rather than clamp the rate. No galaxy
      the builder allows has such a black hole, and the rate at every mass accepted is equation 27
      unclamped, as ruled.
    - The module doc states that an event is the disruption of a star that entered the loss cone,
      at the equation's rate of loss-cone entries (in a steady state the two rates are equal), its
      time the disruption, which T30.b makes the victim's pericentre passage; that the equation is
      extrapolated below SM16's sample, about 10⁶ M☉ (§6), and has no support below Yao et al.'s
      10^5.3 M☉; and that above about 10^7.5 M☉ some would be swallowed whole, which T30.b decides
      per victim (SM16's rule near equation 33, §4.2; the brief's ruling item 1).
    - _Sanity checks._ Van Velzen (2018, ApJ 852, 72) finds ≈ 1 × 10⁻⁴ per galaxy per year
      (abstract; §3), the equation's value at the fixture's mass, and slopes from +0.3 to −0.5
      "broadly consistent" (§4.3), a range that holds −0.404. Yao et al. (2023, arXiv:2303.06523v2)
      find 3.2 (+0.8 −0.6) × 10⁻⁵ at a host of 10¹⁰ M☉ (abstract; §6.5), where their equation 5
      gives a black hole of 2.66 × 10⁶ M☉ and equation 27 gives 1.26 × 10⁻⁴, 3.9 times more: the
      tension of SM16 §5.1 and Yao et al. §6.6, a deferred finding.
  - _The disruption bins and look-back_ (lean, the brief's §2.2 item 5). Bins of 1,024 Julian
    years, 32,314,982,400 s, so that the source horizon is bins −257 to 0, 258 bins; a look-back of
    0, since a disruption is an instant. T30.b sets its own look-back for the tails, which moves no
    event.
  - _The word layout._ Flares: word 0 the mean luminosity, word 1 the duration. Disruptions: no
    word is drawn; the words of a disruption's event stream from word 0 are T30.b's, for its
    victim, so that T30.b moves no time and no ID.
  - _Sources_, each read in full text for this task: Neilsen et al. 2013 (ApJ 774, 42; §3, §4,
    §4.2, §4.3, §5, Tables 1 and 2); Stone and Metzger 2016 (MNRAS 455, 859; §2.2, §4.1 and
    equation 27, §4.2 near equation 33, §5.1, §6); van Velzen 2018 (ApJ 852, 72; abstract, §3,
    §4.3 and equation 10, §5.4); Yao et al. 2023 (arXiv:2303.06523v2; abstract, equation 5, §6.4,
    §6.5, §6.6); Ponti et al. 2015 (MNRAS 454, 1525; §1, §2.4, Table 3, §5.4); GRAVITY
    Collaboration 2020 (A&A 638, A2; §4, Table 2); Witzel et al. 2018 (ApJ 863, 15; abstract);
    Witzel et al. 2021 (ApJ 917, 73; abstract).
  - _Tests_, 26 in the module, none marked slow.
    - Flares (`flares::tests`, 12): the year's 401.775 to 10⁻¹² relative; the bound across the
      source horizon; the law's figures above; the marks' ranges, the cap and the two
      Kolmogorov–Smirnov tests over a year; the light curve (0 outside, continuous to 10⁻⁶ of the
      peak over a microsecond, peak at the middle, the trapezoid integral the energy to 10⁻⁵);
      partitions in any order and on threads (compiled out on wasm32-wasip1); regeneration and
      `None` for the other tag, another subject and numbers beyond the counts; `flares_active_at`
      against the listing at 20,000 instants; no history at +900 years; the same seed twice.
    - Disruptions (`disruptions::tests`, 11): 258 bins; the fixture's 0.20677 and 27.309, each
      equation 27 times the span to 10⁻¹² relative, and 0.06 ≤ 0.20677 ≤ 0.21; the bound over 71
      masses from 10⁵ to 10^8.5 M☉; the two refusals; partitions in any order and on threads;
      regeneration and `None` for foreign IDs; no history at +900 years; the same seed twice.
    - `events::tests` (3): the subject, `of(&Galaxy)`, and the golden.
    - _The rate integrals._ Flares over one Julian year from the epoch, summed over 10³ seeds
      (family `0x0930_a000_0000_0000`): expected 401,775.000, counted 402,103, interval
      (399,691, 403,862) at α = 10⁻³. Disruptions over the source horizon, summed over 10⁴ seeds
      each: at the fixture's mass expected 273,091.72, counted 273,537, interval (271,374, 274,813);
      at 10⁵ M☉ expected 1,248,069.44, counted 1,247,877, interval (1,244,395, 1,251,747); at
      10^8.5 M☉ expected 48,110.38, counted 48,058, interval (47,390, 48,834). They take 0.27 s and
      1.09 s on the slow-test profile and 1.4 s together in a debug build, so neither is marked
      slow: about 0.27 ms for a year of flares at one seed and 36 µs for a source horizon of
      disruptions at one seed.
    - `cargo test -p hyperion-sim features::centre::events`: 26 passed, in 2.2 s of tests (67 s
      with the build); its five doctests pass under `--doc`. The registry run,
      `cargo test -p hyperion-sim -- events event_tags domain_tags_are_pinned`: 93 passed and 3
      ignored (other tasks' slow tests), 1 passed, and 15 doctests passed. Measured on the shared 8-thread laptop at a 1-minute load of
      3–8, with another lane's tests running beside them at times.
- **Deferred findings (2026-10-13 rule) (P09.T30.a).** Recorded and deferred: each leaves the
  algorithm and every contract as they are.
  - _One flare law for every black hole._ Neilsen et al. measured Sgr A* alone; the rate, the laws
    and the template are the same at every black-hole mass. A dependence on the mass or the
    accretion rate needs a source that this task did not read.
  - _The near-infrared and submillimetre counterparts._ Every X-ray flare has a near-infrared one,
    and near-infrared flares are about four times as frequent (Ponti et al. 2015, §1; GRAVITY
    Collaboration 2020, §4: "X-ray flares and NIR flares are coupled. The converse is not true").
    The near infrared is a flux distribution: a log-normal with a power-law tail above about 2 mJy
    (GRAVITY 2020, §§4–5; median 1.1 ± 0.3 mJy), the log-normal flux densities of a red-noise
    process with τ_b = 243 (+82 −57) minutes (Witzel et al. 2018). The X-ray process is the
    low-pass-filtered near-infrared one, and the submillimetre lags the near infrared by about 30
    minutes (Witzel et al. 2021). Not modelled.
  - _The bright-flare rate's changes._ Ponti et al.'s (2015, §5.4) bright-flare rate rises from
    0.27 ± 0.01 a day to 2.52 ± 0.98 after 2014 August 31, and GRAVITY (2020, §4) report six bright
    near-infrared flares in 2019. The rate here is constant in time.
  - _The efficiency-corrected integral._ Neilsen's Table 2 fit to the efficiency-corrected
    luminosity distribution (N = 50 ± 20, Γ = −1.9) integrates to about 1.5 a day on
    [10³⁴, 1.925 × 10³⁵] erg/s over the campaign's 34.5 days (104.4 ks of flaring at a 3.5% duty
    cycle, §4), against the 1.1 built (ruling item 2); without the top it is 1.6. Its errors
    involve "extrapolating outside the domain of the data" (Table 2's note), and Ponti et al.'s
    bright-flare rate matches 1.1 a day under this law (0.278) and not 1.5 (0.379).
  - _Flare substructure._ The brightest flare peaked near 5 × 10³⁵ erg/s with four subflares
    (Neilsen §5, Table 1), above the single template's bound of 3.5965 × 10³⁵ erg/s. Neilsen fit
    each subflare with σ of 100–1,600 s; a flare here longer than 6,400 s has σ up to 2,044 s and
    stands for several subflares in one Gaussian.
  - _Faint and short flares._ Flares below 10³⁴ erg/s and shorter than the 400 s selection limit
    (Neilsen §3, §5) are not modelled.
  - _Equation 27 below 10⁶ M☉_ is an extrapolation (SM16 §6), in the direction that Yao et al.'s
    TDE mass function, ∝ M_BH^−0.25 down to 10^5.3 M☉ (§6.4), supports; below 10^5.3 M☉, which a
    draw reaches only beyond about 4σ of the M–σ scatter, nothing supports it.
  - _The Hills-mass cut._ SM16 (near equation 33, §4.2: "Eq. 27 if M• < M_Hills(M★), but is 0
    otherwise"), van Velzen 2018 (§3) and Yao et al. 2023 (§6.4) cut the rate where stars are
    swallowed whole, from about 10^7.5 M☉. A 1 M☉, 1 R☉ star's Hills mass is 1.14 × 10⁸ M☉ by SM16
    §2.2's definition, below 1.36 × 10⁸ M☉, 1.5 dex (±4σ) above the Milky Way's black hole, and
    the builder allows more. Every star entering the loss cone is an event here; T30.b gives each victim its outcome, disrupted or swallowed whole, so that no
    T30.a ID or time moves.
  - _SM16's tension with observed rates._ Their few × 10⁻⁴ per galaxy per year exceeds the
    observed rates "by at least an order of magnitude" (§5.1); Yao et al.'s optical rate at the
    Milky Way's mass is 3.9 times below equation 27 (§6.6).
  - _A rate of this centre's own._ The rate could come from this centre's loss cone and
    distribution function (P09.T24.b's `DistributionFunction`, P09.T25's `LossCone`) instead of
    SM16's fit to other galaxies' Nuker profiles, which ignore nuclear-cluster light (SM16 §6).
    This centre's inner slope sits at their boundary between core and intermediate galaxies.
  - _A shared power-law sampler._ `flares::power_law_quantile` is this module's own, beside
    P06.T28.a's private inverse transform; one helper in `crate::events` would change T28.a's file.
    Lean: share it when a third kind needs one.
- **P09.T44 as built** (issue #74, lane `lane/74-p12-t11c-compact-refusals`, 2026-10-22, at
  `GENERATOR_VERSION` 22). Client and guide only: no Rust, protocol, binding, server, sim or golden
  change, nothing generated moves, and no version changes.
  - _Names._ Under `apps/hyperion/src/renderer/src/`: `displays/system/HostReadings.tsx`'s private
    `radiusInKm` becomes `isCoreCollapseRemnant(host)`, a neutron star or a black hole, read at
    both its uses, the radius's unit and the nebula's rows; its private `nebulaReadings` takes the
    host rather than its nebula and gives nothing for such a host. `PrimaryReadings` names its
    inline test of the same two kinds `collapsed` and reads it for the radius and for
    `NEBULA RADIUS`. No helper is shared between the two files: no file of this task's scope is
    one both import, and each tests the kind as it already did for the radius in `km`.
  - _As briefed._ The rows are left out by the host's kind, whatever nebula the answer carries, so
    a star read as observed keeps them by the kind its light shows. `EVENTS` stays. The docs of
    `PrimaryReadings`, `HostReadings` and `nebulaReadings` say so; `PrimaryReadings`' and
    `isCoreCollapseRemnant`'s cite Kwitter and Henry (2022, PASP 134, 022001, §1) for a planetary
    nebula's coming only from a star of 0.8–8 M☉ that leaves a white dwarf, and Smartt (2009, ARA&A
    47, 63) for core collapse above about 8 M☉ (science check: both hold; Kwitter and Henry imply
    the exclusion rather than state it). The guide's `NEBULA RADIUS` row reads as the brief gives
    it. P09.T41.c's entry is not edited to point back here: a task's text changes only through a
    re-validation, and this record and T44's entry say what T41.c's rows take.
  - _Tests (11 new, as run)._ `displays/system/HostReadings.test.tsx` (6: a neutron-star and a
    black-hole host read none of the five rows, and each still reads `EVENTS NONE`; a neutron-star
    host reads none of them even where the answer carries a nebula; a white dwarf that lights none
    still reads `NEBULA RADIUS NONE`); `displays/galaxy/SystemReadout.summary.test.tsx` (5: no
    `NEBULA RADIUS` under `STAR A` for `aNeutronStar` and `aBlackHole`, each still reading `EVENTS`,
    and `aWhiteDwarf`'s `—`). The Sun-like star's `—` the brief names is the existing test of what
    this version does not model, so the new one is the white dwarf's, which separates the remnants
    (TypeScript review). Every other existing test passes unchanged, the white dwarf's nebula and
    the living star's `NONE` and `—` among them.
  - _Deferred findings (2026-10-13 rule)._ (1) § Data states has the server set a section's state
    and the console never infer it; here the console leaves the rows out from `host.kind`, which is
    the server's, so a contradictory answer (a neutron star carrying a nebula) is hidden without a
    sign. Whether a row left out by a server-sent kind counts as set by the server, or Not
    applicable should come on the wire with P09.T41.c's rows, is the owner's (UX review). (2) The
    one kind test now sets the radius's unit and the nebula's rows in two files (`collapsed`,
    `isCoreCollapseRemnant`); one predicate in `lib/system/`, outside this lane, would keep the two
    displays from drifting apart (UX review).
- **P09.T17 as built (issue #122, lane `lane/122-p09-t17-supernova-clock`, 2026-10-27, at
  `GENERATOR_VERSION` 23, no output moves).** Pure functions; nothing calls them until P09.T35, so
  no generated output moves and there is no bump (decision-bump-plan-after-22, §3). Files:
  `galaxy/catalogue_classes/supernova.rs` (the items) with `supernova/clock_tests.rs` (their
  tests), `snr/window.rs` (the electron-capture and Iax laws), `snr/caps.rs` and `galaxy/mod.rs`
  (the held caps), `gas/field.rs` (`GasField::metallicity_at`), `rng/tags.rs` (`snr.energy`, scope
  `System`, keyed by `ObjectKey::from(SystemId)`: word 0 one uniform, the energy's rank; words 1–7
  reserved). Also the brief's three doc comments from the docs queue: `features/cluster.rs`'s
  `MEAN_BLACK_HOLE_MASS` (F1), `Galaxy::mean_formed_mass` (F2: 1.0961 M☉ under the default and
  0.8215 under Kroupa's, measured at this base) and `sse/track/build.rs`'s
  `stripped_by_companion` (F3). The desktop accepted leans (a)–(g) as built on the issue
  (2026-10-27); the signatures below are the Provides that P09.T33–T36 consume.
  - _Consumes, re-validated against P08.T9 and P08.T13 as built._
    `SystemStars::death_time()`'s T is `ClockDeath::At(UniverseTime, DeathKind)`, with
    `BeyondClockRange` and `AlreadyRemnantAtBirth` beside it. `natal_kick()` is `Option<NatalKick>`
    (`speed`, `direction`, `velocity` in m/s). The system's `Composition` is
    `SystemStars::primary().composition()` and is not read (ruling 136.9).
    `displaced::ExplosionSite` is `new(position, death)`, `position()`, `death()`.
    `SHELL_WINDOW_CAP` is `snr::caps::SHELL_WINDOW_CAP`, already this plan's. `explosion_site` and
    `death_in_interval` are as built, the latter's interval (−(`SHELL_WINDOW_CAP` + L + H), +H].
    `recent_death_claims` is no function: `RECENT_DEATH_CLAIM: Option<RecentDeathClaim> = None`,
    for P09.T35, untouched here. `ClassTable` is read only through `IaLeftover::speed`, whose
    speeds `IaLeftover::draw` takes from `SURVIVOR_POPULATIONS`. `GasField::dust_per_hydrogen`
    returns ζ = 10^[M/H], not [M/H]. `ShellEnvironment` carries no data. `snr.energy` was not
    registered, and is added here.
  - _Signatures, against the Provides sketch._
    `DeathMarks::of(&Galaxy, &SystemStars, &ExplosionSite, GalacticVelocity) -> Option<DeathMarks>`
    (the velocity added, and `Option`; points (a) and (e)).
    `DeathMarks::of_type_ia` (added; point (b)) takes `&Galaxy`, `SystemId`, `&IaProgenitor`,
    `&IaLeftover`, `&ExplosionSite` and `GalacticVelocity`, and gives `DeathMarks`.
    `claims(&Galaxy, &ExplosionSite, &DeathMarks, Surroundings) -> bool` (`Surroundings` for
    `ShellEnvironment`; point (g)). Added beside them: `claimed_window` (the cut window where
    `claims` holds, the one route by which a class tests a death and gets its entry's window),
    `effective_window` (W_eff's window), `lifetime_cap`, `LIFETIME_REACH` (4,096 ly, a parameter
    of the generator version), `Surroundings::environment`, `BubbleInterior`,
    `SupernovaKind::of_death`, `SupernovaInterval`'s `of_window`, `of_cap`, `thinning_cap`,
    `window`, `start`, `end`, `length` and `contains`, and on `ExplosionEnergy` the two new laws
    and their constants.
    `DeathMarks` holds the system, T, the kind, the energy and v_fast; the sketch's metallicity is
    dropped (point (c)).
  - _The interval on the clock._ W_eff = min(W, the environment's cap, 4,096 ly ÷ v_fast − (L +
    2H)), floored at zero, in Julian years, then floored to the nanosecond
    (`Span::from_seconds_f64`) and held to 0…`SHELL_WINDOW_CAP`. `claims` is start < T ≤ +H with
    start = EPOCH − (W_eff + L + H), so T = +H is in and T = start is out, exactly. At
    `SHELL_WINDOW_CAP` the start is `death_in_interval`'s, 134,534,593,094,400 s before the epoch,
    and no interval opens earlier.
  - _Design points._
    - (a) v_fast for a core collapse is |v + kick|, or |v| with no kick, with v the caller's
      galactic velocity: the grid's `epoch_velocity`, a member's bulk plus its own, a candidate's
      draw. `epoch_velocity` serves grid records only and a Type Ia candidate has no
      `SystemStars`, so the constructor cannot find v itself. The system's companions are not
      members of the entry and are not read.
    - (b) A second constructor for Type Ia candidates, since they have no `SystemStars`; a recent
      Type Ia has no grid side to agree with (P09.T35: plan 11's binaries redraw any explosion
      before +H). v_fast is the bound |v| + `IaLeftover::speed`, which has no direction; |v| for
      a merger and for a Type Iax's partly burnt dwarf.
    - (c) `claims` reads the site gas's [M/H], `GasField::metallicity_at(p) -> Dex`, which
      `zeta` now calls, bit for bit (`dust_per_hydrogen` is 10^ it, tested); `shell_window` holds
      it to −1…+0.5 dex. Cited to CMB88, p. 259 ("`ζ_m` = 1 for solar abundances").
    - (d) The Iax law, `ExplosionEnergy::iax_from_uniform`, and, by the desktop's answer on the
      issue (2026-10-27), an electron-capture law, `ExplosionEnergy::electron_capture_from_uniform`:
      below. A private `EnergyLaw` picks the law from the death (`ElectronCapture`) or the channel
      (`Iax`), so `SupernovaKind` is unchanged and an electron-capture supernova keeps the light
      curve of its type. `ExplosionEnergy::from_foes` now accepts `IAX_MIN`–`MAX`; `MIN`, `MAX`,
      `from_uniform` and the caps' scan, at `MAX`, are unchanged, so no cap moves.
    - (e) `SupernovaKind::of_death(&Death) -> Option<SupernovaKind>`. `CoreCollapse` keeps its
      type. `ElectronCapture` is typed by plan 06's envelope rule (`SupernovaType::of_envelopes`
      on the hydrogen envelope and the helium outside the carbon–oxygen core), so a super-AGB
      progenitor is a IIP, as SN 2018zd is (Hiramatsu et al. 2021). `EnvelopeLoss`,
      `DirectCollapse` and a `CoreCollapse` of `SupernovaType::None` make no shell and give no
      marks; so do `PairInstability` (5–94 × 10⁵¹ erg, Heger and Woosley 2002, far above the caps)
      and `ThermonuclearDisruption` (no Type Ia channel describes a single star's carbon
      ignition), both stated as not modelled and left on the grid. No marks rather than a `claims`
      of false: the marks would have no kind to carry, and the caller learns before it reads the
      gas.
    - (f) `WindowCaps::from_galaxy` costs 3.7 ms in a test build against 352 ms for
      `Galaxy::new`, about 1%, so `Galaxy::from_params` computes the caps once
      (`Galaxy::window_caps`) and `from_galaxy` now copies them; a test holds them to
      `WindowCaps::from_gas_params` of the gas's parameters, bit for bit. A scan on every claim
      would cost that for every recent death. `Galaxy::from_params` documents the caps' assertion
      under Panics; it never fires at the drawn floors, 300–450 K cm⁻³.
    - (g) `ShellEnvironment` stays fieldless. `Surroundings`, of `Field`, `TypeIa` and
      `Bubble(BubbleInterior)`, carries the interior the caller passes from the member's parent
      feature, `BubbleInterior::new(SiteGas, wall)`, with the distance from the site to the
      bubble's wall, so the wall cut (`ShellWindow::ended_at_wall`) is part of the shared test and
      every side agrees on T36.a's window "ended at the wall". The window is cut at the cap with
      `WindowCaps::cut`, whose debug assertion catches an interior the bubble's cap was not
      scanned over.
    - (h) `claims` takes no cache, so no `FeatureCellCache` can be passed to panic on use, and the
      plan's test cannot be written as sketched. `the_test_reaches_no_feature` pins the signatures
      of `claims`, `claimed_window`, `effective_window` and both constructors, and checks that the
      non-test source of `supernova.rs`, `snr/{mod,window,shell,caps}.rs` and
      `gas/{field,noise,phase,pressure,smooth}.rs` names neither `FeatureCatalogue`,
      `FeatureCellCache`, `NoFeatureCache` nor `FeatureRef`, and that `supernova.rs` and
      `snr/window.rs` name no `features::` path, no `GasModifier` and no `modifiers_near`. Our
      reading: with no cache argument the remaining route to a feature is generating its cell,
      `FeatureCatalogue::cell`, and nothing the test calls names it.
  - _The electron-capture law (the desktop's answer, 2026-10-27)._ Log-normal in E, median
    10^−0.85 = 0.141 × 10⁵¹ erg (√2 × 10⁵⁰, the geometric middle of the simulations' 1–2 × 10⁵⁰),
    0.2 dex as the core-collapse law, truncated at ±2σ: 0.056–0.35 × 10⁵¹ erg (ours, provisional;
    `ELECTRON_CAPTURE_LOG_MEDIAN`, `_SIGMA_DEX`, `_TRUNCATION`, `_MIN`, `_MAX`). Sources: an
    oxygen–neon core explodes "with an energy of about or slightly more than `10**50` erg" (Kitaura,
    Janka and Hillebrandt 2006, the arXiv abstract; the manuscript's reads "≳ 0.1 × 10⁵¹ erg");
    "simulations consistently predict explosion energy (∼ 2 × 10⁵⁰ erg)" (Hiramatsu et al. 2021,
    on SN 2018zd); self-consistent models give 0.12–0.18 × 10⁵¹ (Radice et al. 2017, model n8.8)
    and 1.36–1.48 × 10⁵⁰ (Zha et al. 2022). The Crab's filaments carry about 7 × 10⁴⁹ erg (Smith
    2013, §2), inside the range near its floor. The whole law lies below the core-collapse law's
    0.40 and above the Iax floor. Science check: the quotes and constants hold; it offered
    10^−0.9 (0.126) as an alternative median, between the Garching runs' ~0.10 and the others'
    0.14–0.18, and found −0.85 defensible, which is kept.
  - _The Iax law (ruling 136.10)._ Log-normal in E, median 10^−1.3 = 0.050 × 10⁵¹ erg, 0.65 dex,
    truncated at ±2σ: 0.0025–1.0 × 10⁵¹ erg (ours, provisional; `IAX_LOG_MEDIAN`,
    `IAX_SIGMA_DEX`, `IAX_TRUNCATION`, `IAX_MIN`, `IAX_MAX`). Median: SN 2020kyg's E_k = 4.4
    (+2.2/−1.5) × 10⁴⁹ erg (Srivastav et al. 2022, §3), a faint Iax; their volume-limited rates
    put faint Iax at 12 and luminous at 0.9 per 100 Type Ia, of 15 for all. Floor: SN 2008ha's
    ~2 × 10⁴⁸ erg (Foley et al. 2009); Valenti et al. (2009) find 1–5 × 10⁴⁹ for the same event,
    which the sources leave discrepant. Top: SN 2012Z's ~10⁵¹ erg (Stritzinger et al. 2015, who
    give the class ~10⁴⁹–10⁵¹). 6.3% of Iax lie above 0.4 × 10⁵¹ erg, the luminous share of 0.9 in
    15 if the luminous are the energetic (our reading). A normal Type Ia's 1.2 ± 0.2 × 10⁵¹ erg
    (Woosley et al. 2007, §8) lies inside the core-collapse law's 0.40–2.51. The window at each
    law's least energy is finite and positive over the window table's densities.
  - _Figures._ The lifetime cap at 2,500 km/s is 4,096 × 299,792.458 ÷ 2,500 − 264,144 =
    227,035.963 yr, under 0.23 Myr; at a thin site, (0, 30,000, 900) ly, it is what ends the
    window. Seed 19's caps, as `claims` reads them: field and Type Ia 3.357 Myr, bubble 0.570
    Myr; thinning caps 3.622 and 0.834 Myr. At the solar site (0, 26,000, 20) ly, 10⁵¹ erg, W_eff
    is 3.181 × 10⁵ yr, so the interval opens 18,343,657,851,196.6 s before the epoch.
  - _Tests_ (`supernova/clock_tests.rs`, fifteen, each on its own build of seed 19's galaxy;
    `snr::caps`'s held-caps test; the plan-09 tag test; five doctests, on `lifetime_cap`, `claims`,
    `claimed_window`, `DeathMarks::of_type_ia` and `SupernovaInterval`): membership by
    `SupernovaEntry::state_at` before, at and after T and after W_eff, and a death 500 years
    after the epoch claimed as a progenitor; the marks against the stellar stage, the kick and
    the stream (the death's law bit for bit, the same twice, two systems two streams); each death
    kind; Type Ia marks of all four channels (the Iax law, |v| + survivor); a 240-case sweep for
    purity and `assert_order_independent`, both answers in every environment, `claimed_window`
    agreeing with `claims`; the four boundary ticks; the lifetime cap, and W_eff never negative
    nor above its cap or `SHELL_WINDOW_CAP` over 400 sites and speeds to 6,000 km/s; every
    interval inside `death_in_interval`'s, the ends and thinning caps exact; ζ from the site gas;
    the bubble's wall and cap; the Iax and electron-capture laws' ranges and medians, and the Iax
    share above 0.4 × 10⁵¹ erg; each death on its law; no feature reached; and the three laws at
    three ranks, two systems' draws and the solar site's W_eff and start pinned bit for bit, so
    that a change before P09.T35 fails a test rather than riding T35's bump (determinism review).
  - _Goldens._ golden_diff against `origin/rendering-and-planets`: `rng/tags.golden` extended only
    (+1 −0, `snr.energy (System) = 0xc77e73f30ba691e8`), no changed value, no other golden,
    `GENERATOR_VERSION` 23 → 23.
  - _Not here (P09.T32–T36's)._ `ClassProcess` and the class grid, the processes and lists of
    T33–T36, `recent_death_claims` and `RECENT_DEATH_CLAIM`, any change in `displaced/` or
    `placement/`, plan 08's text.
- **Deferred findings (2026-10-13 rule)**, from P09.T17:
  - _One velocity on every side._ v_fast reads the velocity the caller passes, and decides
    W_eff's lifetime cap, so near the interval's start two sides that pass the same system
    different velocities could disagree. T35 and T33–T36 should take the velocity from one
    function per kind of candidate (the grid's `epoch_velocity`), or test that the sides pass the
    same bits (determinism review).
  - _CMB88 at low energy._ The Iax and electron-capture laws run CMB88's window down to 0.0025 ×
    10⁵¹ erg, far below the energies its scalings were fitted at, where the ejecta-dominated phase
    lasts longer. The caps are unaffected (the window is largest at `MAX`); the window at the
    least energies is an extrapolation, noted in `ExplosionEnergy`'s doc (science review).
  - _Partly burnt Iax remnants move._ LP 40-365 moves at 852 ± 10 km/s in the Galaxy's rest frame
    (Raddi et al. 2018) and Jordan et al. (2012) expect a kick of a few hundred km/s, but
    P09.T18.c's `IaLeftover::PartlyBurntDwarf` stays at the site with no speed, so an Iax entry's
    v_fast is |v|. If T18.c gives it a speed, `of_type_ia` reads it through `IaLeftover::speed`.
  - _Companions._ v_fast reads only the remnant: a bound companion's recoil
    (`SystemStars::recoil`) and an unbound one's speed are plan 11's, and no member of the entry
    yet.
  - _F2's "alone"._ `Galaxy::mean_formed_mass`'s doc, as queued, says the figure "depends on the
    mass function alone", but 1.0961 and 0.8215 M☉ count the companions the mass function implies
    (`fates.rs`); "on the mass function and the companions it implies" would be exact (science
    review). Applied as queued.
  - _A stale Consumes line._ This plan's Consumes bullet for plan 08 still says "only P08.T1–T7
    are built" at `d2787a2`; P08.T9, T12 and T13 are built. Left for the plan's next
    re-validation.
- **P09.T28.a's propagator: a solve the iteration leaves unsolved is bisected (found and fixed in
  P09.T28.b.a, lane `p09-centre`, 2026-10-27, at `GENERATOR_VERSION` 23; main's ruling of
  2026-10-27).** No output moves: nothing generated reads the propagator yet, and every state
  that the ten Laguerre–Conway steps solve keeps its bits (`kepler.golden` and T28.a's tests
  unchanged).
  - _The defect._ The fixed `UNIVERSAL_ITERATIONS` = 10 do not solve every state the centre
    holds. P09.T28.b.a's pre-pin found a rogue planet of seed 0x5eed0000c0ffee00
    (`0xcfffdfffc0005f87`: 1.50 ly out, leaving at 958 km/s on a nearly radial hyperbola, e =
    1.449, q = 0.0875 ly) whose state 1,000 yr back came out 26,480 ly away (3,953 ly at −999 yr),
    where the orbit is some 2 ly out; its energy was conserved, since any χ gives a point of the
    conic, but at the wrong time. Run backward, the flyby falls in through its pericentre: the
    starter (the parabolic cubic's, below the root as designed) lies near the pericentre, where
    `F′ = r` is least, the first step overshoots into the hyperbolic branch, and from there each
    step gains only about `1 ÷ √−α`. A scale-free scan (e from 1 to 2, the start's true anomaly,
    τ in units of `√(q³ ÷ μ)`) finds two such regions, neither in T28.a's stress set: a solve
    towards the pericentre from 20–220 pericentres out at e of 1.05–2 and τ′ of 60–3,000, and a
    nearly parabolic flyby (e ≈ 1.01) over τ′ of 10⁶–10⁷, which within the clock window means
    pericentres inside about 10⁻⁴ ly.
  - _The fix._ `propagate` solves through `universal_anomaly`: the ten steps as before, then
    `solves` checks Kepler's universal equation to `UNSOLVED_RESIDUAL` = 10⁻⁸ of its largest term
    (a solved χ leaves 10⁻¹³ even far out on a hyperbola; the unsolved ones leave 10³–10⁴ times
    √μτ; an overflowed one is never solved), and an unsolved χ is found again by `bisect_anomaly`,
    `BISECTION_STEPS` = 128 halvings of `[0, √μτ ÷ q]`. The bracket holds the one root:
    `dF ÷ dχ = r ≥ q > 0` (Battin 1999, §4.5), so F is increasing, `F(0) = 0` and
    `F(√μτ ÷ q) ≥ √μτ`. Far out on a hyperbola the bracket's end can
    lie where the Stumpff terms overflow and their sum is not a number; such a χ is beyond the root
    (the terms grow with |χ|, and every root the clock window reaches is finite), so the far end
    moves in to it. Only IEEE operations and `math` functions, fixed counts; the residual test is a
    comparison of deterministic values, so native and wasip1 agree bit for bit (the tests run in
    the wasip1 suite).
  - _Tests (new, in `galaxy/motion.rs`)._
    `an_inbound_flyby_through_its_pericentre_is_solved_pinned` (the rogue planet: the ten steps
    leave it unsolved, the fallback solves it, 1.5–2.5 ly at −1,000 yr, within 10⁻⁹ of the
    hyperbolic anomaly's position, energy to 10⁻¹²);
    `nearly_radial_flybys_match_independent_references_pinned` (e = 1, 1 + 10⁻⁶, 1 + 10⁻⁴, 1.01,
    1.05, 1.2, 1.45, 2; pericentres 10⁻⁴–1 ly; starts 1–300 pericentres out, inbound and outbound;
    ±1 to ±10⁴ yr; against Barker's equation for the parabolas and the hyperbolic anomaly for the
    rest, both by bisection and sharing no algebra with the universal variables (Vallado 2013,
    §2.2); 2,016 propagations, 14 by the fallback, worst 3.5 × 10⁻¹⁰ of the farther radius,
    held to 10⁻⁹; it fails if the fallback is never reached); and T28.a's
    `the_fixed_iteration_count_converges` now asserts that its stress set never takes the
    fallback, so those states keep the iterated χ. Both constants belong to the generator version
    once P09.T28.b.b wires the propagator in. This supersedes "P09.T28.a as built"'s _Method_ (the
    ten steps alone); its _Domain_ case (e = 100, q = 10⁻³ ly, solved back from 160 ly) was not
    re-run.
  - _In P09.T28.b.a's pre-pin sample_ (three galaxies, 453,819 Kepler-regime grid records at four
    times each), 5 records take the fallback, each at one of its times; with it, the Kepler arm
    moves no record more than 4.6 ly from its straight line over 1,000 yr (fast flybys deflected
    by the black hole), where it had moved one 26,480 ly.
- **P09.T28.b.a as built (lane `p09-centre`, 2026-10-27, on e8b5c6f3 at `GENERATOR_VERSION` 23;
  decision-galaxy-25-train §2, with main's rulings of 2026-10-27 on the influence radius and the
  pre-pin).** No output moves: nothing on the record calls the new functions (`position_at`,
  `hit_at` and `resolve` are unchanged; plan 03's `drifted` is only made `pub(crate)`), no golden
  moves, and `golden_diff.py` finds only new labels. New goldens `galaxy/motion/regime.golden` (the
  pre-pin, slow) and `galaxy/motion/centre_motion.golden` (fast), extensions at 23. `Galaxy`'s
  `Debug` text gains a fixed `centre_motion: CentreMotionCell` field, which no persisted digest
  reads. Files: `galaxy/motion.rs`, `galaxy/mod.rs`, `galaxy/features/centre/{profile,mod}.rs`
  (`BuildCentreError::NoInfluenceRadius`, a crate-internal `heap_bytes`, doc pointers),
  `galaxy/potential/monopole.rs` (a crate-internal `heap_bytes`), `galaxy/query/{mod,motion}.rs`,
  `tests/galaxy_motion.rs`, `tests/galaxy_handle.rs` (a comment), the goldens.
  - _Signatures._ In `motion`: `Regime { Straight, Kepler }`,
    `regime_of(&Galaxy, &SystemRecord) -> Regime`,
    `position_velocity_at(&Galaxy, &EpochState, UniverseTime) -> StateAt`,
    `loss_cone_verdict(&Galaxy, &EpochState) -> LossConeVerdict`, `CentreMotion`, `EpochState`,
    `BuildEpochStateError`, `MotionState`, `StateAt { Exists(MotionState), NoSuchSystem }` and
    `LossConeVerdict { Clear, Inside }`; and `Galaxy::centre_motion() -> &CentreMotion`.
    `position_velocity_at` takes an `EpochState`, the record with its epoch velocity, not the
    record alone, so that one function serves T28.b.b's grid records, T29's members and plan 12:
    `EpochState::of_grid_record` (plan 08's `epoch_velocity`, zero without kinematic tables),
    `of_centre_member` (the member's own) and `new` (any source with its own velocity; refuses a
    non-finite velocity or one not below light's, so no law can leave the addressable range).
    `CentreMotion`: `new(&GalaxyParams, &MassModel)`, `profile`, `influence_radius`,
    `stars_within`, `enclosed_mass`, `loss_cone`, `regime_at` and `kepler_orbit`, the orbit
    `position_velocity_at` propagates, bit for bit, for T28.c's pericentre and T29's cache. A
    crate-internal `KeplerOrbit::displacement` (its state in metres); `Regime::name`.
  - _Design note 15's choice: a memo in the galaxy._ `Galaxy` holds `CentreMotion` in a
    `OnceLock`, built on first read from the parameters and the mass model; it compares equal
    always and prints a fixed `Debug` text (the sim-determinism skill's lazy exception, as
    `BlackHoleCell`), and `the_motion_is_order_independent_pinned` holds its three readers to the
    same answers in any order and cold or warm. Built eagerly it added 64 ms to every galaxy build
    (test profile, the sim at opt-level 2; `Galaxy::new` 328 → 392 ms on a loaded machine), nearly
    all `SphericalAverage::of_extended`, against Design note 15's 100 ms; `CentreProfile::new` is
    under a microsecond once the cluster's shape is built. The galaxy holds this part, not the
    whole `CentreModel` (91 ms): `resolve` still builds the model per call, and T29 decides its
    cache. Once read, the memo adds about 20 KiB to `heap_bytes`, which then depends on whether it
    has been read; the server's `GalaxyCache` sums it at insertion, and `galaxy_handle.rs`'s exact
    byte difference holds while neither galaxy has read it. The fast suite gains the nine new fast tests of `galaxy_motion`, 9.7 s of test time (each its own
    nextest process, which builds its galaxy in about 0.6 s; the order test 3.0 s), the motion
    module's new unit tests (its twelve take 0.02 s) and two doctests; a galaxy build pays nothing
    until the memo is read.
  - _The influence radius (main's ruling, a blocker as found)._ The ruling's
    `CentreProfile::influence_radius` counts the nuclear cluster alone, and for 60 of seeds 1–300
    (default mass function) the black hole outweighs the whole cluster (seed 1: 4.16 × 10⁷ against
    8.3 × 10⁶ M☉), so it has no root and returns its bracket's end, 10⁴ ly: every system within
    10⁴ ly would have been in the regime. The edge is instead where all the mass inside weighs the
    black hole, the brainstorm's "where it outweighs the stars inside": the cluster plus the
    centre's spherical average of the discs, bar and bulge (`SphericalAverage::extended_mass`), gas
    included, dark halo not. That is M(< r_h) = `M_bh`, the convention of Baumgardt, Amaro-Seoane
    and Schödel (2018, A&A 609, A28, §3), not Merritt's r_m, M★(< r_m) = 2 `M_bh`, nor G `M_bh` ÷ σ²
    (Merritt 2004; Merritt 2013, CQG 30, 244005), which agree in a singular isothermal sphere,
    where this radius is half of them (science check). μ takes the same mass. At Milky Way values
    r_h = 7.19 ly (2.20 pc, against the 2.18 pc of Schödel et al. 2018's masses; the cluster-only
    radius 7.20 ly); seeds 1–300 give 1.9–177 ly, median 11.8 ly. The brainstorm's "about 10 ly" is
    the literature's ~3 pc, which is r_m, some 11 ly here. Gas is 2.2 × 10⁻⁶ of the mass inside the
    fixture's edge and 1.0 × 10⁻⁴ inside seed 1's (78.3 ly, where the rest of the galaxy is 81% of
    the mass inside); an NFW halo of Milky Way normalisation holds about 0.1% of `M_bh` inside the
    fixture's edge (science check, estimate). A bracket with no root is
    `BuildCentreError::NoInfluenceRadius`, which no seed of the sweep gives.
    - _Finding, for its owner (P09.T24.a's `CentreProfile::influence_radius` and
      `CentreModel::influence_radius`, which Provides lists for plan 10):_ unchanged, it has no
      root for a fifth of drawn galaxies and returns 10⁴ ly (seed 1's is pinned by
      `the_centre_motion_weighs_every_star_inside`); its docs now say so and point at the
      regime's edge. Its only reader is the centre's golden.
  - _The law._ The Kepler arm applies strictly inside r_h. At the epoch either arm gives the epoch
    state to the bit; outside, the straight arm is plan 03's `position_at`, bit for bit (held for
    every pre-pin record at all four times). A state with no orbit (the black hole itself, or one
    with no angular momentum) keeps its epoch position, which is how a galaxy without kinematic
    tables keeps its grid systems still. A centre member's `EpochState` carries its
    distribution-function velocity, which exists without those tables, so in such a galaxy the
    member follows its orbit: the plan's "a galaxy without kinematic tables keeps the epoch
    position" is built as the grid systems'. T28.b.b decides what `position_at` gives a member
    there.
  - _The loss cone._ The verdict is the members' own test (`LossCone::contains`: the Kepler
    pericentre about the black hole alone, about 2 au), not the pericentre of the orbit followed
    about μ: a nearly radial orbit's pericentre `J² ÷ 2G M_bh` depends only on the black hole,
    all the mass within a few au (Alexander 2017, ARA&A 55, 17, §3.1; science check). It applies
    to Kepler-regime grid records and feature members that have an orbit; a centre member is
    always `Clear` (its placement passed the same test), and so is a state with no orbit.
  - _The edge (main's ruling: a figure, not a blocker)._ Across the edge the two laws part by
    `½ μ t² ÷ r²` to leading order, G `M_bh` t² ÷ r_h² at the edge: 0.0130 ly over 1,000 yr at the
    fixture's 7.19 ly, and 0.0136 ly at worst over four velocities to 440 km/s, against the plan's
    0.01 ly, which was written for the brainstorm's "about 10 ly". The test holds 0.014 ly there and
    the closed form to 2% for a slow state 0.1 ly inside; at the plan's 9.9 and 10.1 ly the two
    laws part by under 0.01 ly (four velocities; M(< 10 ly) = 7.38 × 10⁶ M☉ against the black hole's
    4.30 × 10⁶, not the plan's version-15 5.29 × 10⁶). The gap scales as `M_bh ÷ r_h²`: about
    0.020 ly at seed 0x2's 4.09 ly edge (closed form, ours).
  - _Tests (as run)._ `tests/galaxy_motion.rs`: `the_two_regimes_meet_at_the_influence_radius`;
    `the_epoch_state_comes_back_to_the_bit` (members and grid systems on both sides, the black
    hole, a grid system with no velocity); `an_s2_like_member_follows_its_kepler_orbit` (GRAVITY
    2020's a and e about the model's μ: 16 ± 0.1 yr, the position to 10⁻¹² of the apocentre and the
    velocity to the bit over ±1,000 yr, the same state twice);
    `a_galaxy_without_kinematic_tables_keeps_its_grid_systems_still`;
    `the_loss_cone_carves_out_a_plunging_grid_orbit` (1 ly out, 200 km/s in and 1.38 km/s across:
    about 1 au against the 2 au cone, no such system at any time; a grazing one, a centre member
    and the same state outside the regime clear);
    `the_centre_motion_weighs_every_star_inside` (the model's profile and loss cone bit for bit,
    the memo equal to a direct build, equality and `Debug` text unchanged by the read, the edge
    weighing the black hole to 10⁻⁹, seed 1's cluster-only radius at its bracket's end);
    `the_centre_motion_s_tables_count_once_read`; `the_motion_is_order_independent_pinned`;
    `centre_motion_figures_are_pinned` (the fast golden: the fixture's edge, the mass inside eight
    radii, one orbit at ±1,000 yr); and the slow `regime_records_near_the_centre_are_pinned`.
  - _The pre-pin (decision-galaxy-25-train §1.5, D1; main's ruling)._ `regime.golden`: the Milky
    Way fixture (seed 0x0928b0a000000001) and seeds 0x5eed0000c0ffee00 and 0x2, each with its full
    potential, edges 7.19, 10.14 and 4.09 ly. The stellar layers A–E within 1.5 r_h; the brown
    dwarfs and rogue planets within min(1.5 r_h, 4 ly), the reach of the rogue planets' eight
    central cells, since 1.5 r_h holds some 10⁵ brown dwarfs and 10⁷ rogue planets in a drawn
    galaxy, so the substellar records between 4 ly and 1.5 r_h of the fixture and of 0x5eed are not
    pinned. Per layer: the record count, the Kepler count, digests (`f64_digest`) of the IDs, the
    regimes and `position_at` at −1,000, −100, +100 and +1,000 yr (±H is ±1,000 yr, so the ruling's
    "±1,000 yr and ±H" are one pair), and the four records nearest the black hole in full: 790,511
    records, 453,819 in the regime, 1,818 lines, 131 kB. `#[ignore = "slow: …"]`, about 112 s,
    blessed by name; T28.b.b runs it by name at its hold, and the slow pass's wasip1 part runs it.
    A drawn galaxy with an edge near 177 ly would mean some 10⁸ records and was not used.
  - _For P09.T28.b.b._ Over the pre-pin, the Kepler arm (with P09.T28.a's fix) moves records at
    most 4.04, 4.63 and 3.94 ly from their straight lines over ±1,000 yr (fast flybys deflected by
    the black hole) and carves out one record, in 0x5eed (its C1 census lists the ID).
    `EpochState::of_grid_record` reads `epoch_velocity`, which panics for a non-grid record in a
    galaxy with tables, so the swap dispatches on the record's origin (plan 03's Risks). The memo
    makes `heap_bytes` depend on what has been read.
  - _For P09.T29 (may start)._ It reads `regime_of`, `position_velocity_at` with
    `EpochState::of_centre_member`, `loss_cone_verdict`, `CentreMotion::kepler_orbit` (the same
    orbit, for its cache) and `galaxy.centre_motion().{influence_radius, enclosed_mass}` (not
    `CentreModel::influence_radius`), with T28.a's `KeplerOrbit` and T27's `CentrePlacement`.
    Kepler-regime grid records move up to 4.6 ly from their straight lines over ±H, so they need
    the grid-side pad, whose reach `GalaxyRules` (T28.b.b) sets. The regime's edge can lie beyond
    the centre's 128 ly reach (up to 177 ly at the sweep's seeds).
  - _Deferred findings (2026-10-13 rule)_, from P09.T28.b.a's science check: (1) T25's
    `LOSS_CONE_AU` doc calls 2 au "the tidal radius of a Sun-like star", which is about 0.76 au at
    4.3 × 10⁶ M☉ (r_t ≃ (M_bh ÷ M★)^⅓ R★; Alexander 2017, §1.2): 2 au is about 2.6 r_t, a sound
    loss-cone radius described wrongly (`marks.rs`, T25's; the figure is the brainstorm's).
    (2) The brainstorm's "Orbits and time" says the edge is "about 10 ly", μ "the cluster mass
    inside its own radius" and the regime "passes smoothly into drift"; as built under main's
    ruling the edge is M(< r_h) = `M_bh` over all the mass inside (7.19 ly at Milky Way values),
    μ uses all the mass inside, and at ±H the two laws part by 0.013 ly at the fixture's edge
    (0.02 ly at seed 0x2's). Drafted for the brainstorm's sign-off: which convention it means, and
    that this jump is the regime's continuity.
- **P09.T32 as built (issue #127, `lane/127-p09-t32-class-grid`, 2026-10-27, at
  `GENERATOR_VERSION` 23, no output moves).** The class grid, tested on toy classes; no
  production class is registered, so nothing generated moves and there is no bump
  (decision-bump-plan-after-22, §3). Files: `galaxy/catalogue_classes/grid.rs` (the grid) with
  `grid/tests.rs` (its tests and the toys), `galaxy/catalogue_classes/ids.rs` (`CatalogueCellKey`,
  moved from `mod.rs`, and the `111` checks), `mod.rs` (declarations and re-exports, every old
  path kept), `rng/tags.rs` (`class.cell`, scope `Cell`, keyed by
  `ObjectKey::cell(CatalogueCellKey::word())`: one Poisson draw from word 0, of mean the class's
  bound × the cell's volume). `registry.rs` is unchanged. The desktop accepted design points
  (a)–(e), the lookup, the headroom check and the one-velocity rule as built, on the issue
  (2026-10-27); P09.T33.a's brief inherits (a).
  - _Signatures, the Provides that P09.T33–T38, T43 and plan 11 consume_ (all re-exported from
    `catalogue_classes`):
    - `trait ClassProcess`, with `type Candidate` and `type Entry`, and five methods:
      - `class(&self) -> ClassId`;
      - `bound(&self, &Galaxy, &CatalogueCellKey) -> PerCubicLightYear`;
      - `candidate(&self, &Galaxy, &CatalogueCellKey, CatalogueSystemId, PerCubicLightYear)`,
        which returns `Option<Self::Candidate>`;
      - `test(&self, &Galaxy, Self::Candidate) -> Option<Self::Entry>`;
      - `members(&self, &Self::Entry) -> u8`.

      The candidate's ID is its member 0, which keys its streams. `test` both tests and builds the
      entry, because P09.T33.a's test, `claimed_window`, returns the window its `SupernovaEntry` is
      built with. `members` is 1–16, numbered without gaps (debug-asserted, then clamped). The
      trait's doc states the contract in eight rules: purity, streams, the bound, the test, the
      reach, one velocity, headroom and a placed class.

    - `CatalogueClassCell::<E>::generate(&Galaxy, &P, CatalogueCellKey)`, which panics on a key of
      another class, with `key`, `candidates` and `entries` (or `into_entries`) in index order.
      `CatalogueEntry<E>` has `id` (member 0), `members`, `value` (or `into_value`) and
      `member_id(u8) -> Option<CatalogueSystemId>`. `CatalogueMember<E>` has `id`, `member` and
      `entry` (or `into_entry`).
    - `resolve_entry(&Galaxy, &P, CatalogueSystemId)`, one process's resolve, returns
      `Result<CatalogueMember<P::Entry>, ResolveSystemError>`.
    - The `111` prefix's `resolve(&Galaxy, &L, CatalogueSystemId, V)`, with `L: ClassLookup` and
      `V: MemberVisitor`, returns `Result<V::Output, ResolveSystemError>`.
    - `ClassLookup::visit_class(&Galaxy, ClassId, V)`, with `V: ClassVisitor`, returns
      `Option<V::Output>`. `ClassVisitor::visit<P: ClassProcess>(self, &P)` and
      `MemberVisitor::visit<P: ClassProcess>(self, &P, CatalogueMember<P::Entry>)` each return
      their `Output`. `ProductionClasses` is the production lookup, with no process.
    - `cells_in_sphere(ClassId, &QuerySphere) -> impl Iterator<Item = CatalogueCellKey>`.
    - `check_class_headroom(ClassId, PerCubicLightYear)`, the class and its largest bound, returns
      `Result<(), CheckClassHeadroomError>`: `NotPlaced(ClassId)`, or `ClassTooDense` with the
      class, the largest mean and the capacity.
    - On `CatalogueCellKey`: `of(CatalogueSystemId)` returns
      `Result<Self, BuildCatalogueCellKeyError>`, whose error gains `FreeClass(u8)` and
      `Misaligned { class, cell }`; `candidate_id(index, member)` returns
      `Result<CatalogueSystemId, BuildSystemIdError>`; `origin_ly() -> [i32; 3]`;
      `volume_ly3() -> f64`.
  - _Design points._
    - (a) `placement::resolve`'s `Catalogue(_)` arm is unchanged and still answers
      `KindNotGenerated`, which a test pins. The class grid's resolve is its own. Wiring the arm now
      would turn every `111` answer into `NoSuchSystem` with no class to resolve, and a resolved
      entry needs a `SystemRecord` whose `SystemOrigin::CatalogueSystem` does not exist yet, which
      means deciding what a catalogue system is in every match on `SystemOrigin`. That is P09.T33.a's:
      it registers the first class, wires the dispatch, adds the origin variant and says what the
      record's stars and motion are.
    - (b) Associated types with static dispatch, no `dyn`. An entry's type differs by class
      (T33.a's carries `SystemStars`, T34.a's an `IaProgenitor` and a survivor), and T32 has no
      common entry type to put behind an object-safe trait: the record with a catalogue origin is
      T33.a's. A dispatch on `ClassId` is one `match` in a `ClassLookup` that hands the class's
      process to a generic `ClassVisitor`, as `count_tag` matches on `FeatureProcess`; `resolve`
      hands its `CatalogueMember` to a second generic visitor, `MemberVisitor`, so the caller
      decides what a member becomes (T33.a's a `SystemRecord`). T37's merge and plan 12's walk
      visit each class in turn and turn entries into records there; if T37's merge needs a common
      entry type, T37 introduces it (the desktop, 2026-10-27). The bound, the candidate and the
      test are separate methods, as P11.T8 expects.
    - (c) One `Cell` tag, `class.cell`, for every class's count: design note 23 gives each feature
      process a tag because processes share a feature cell, but class cells share nothing and the
      word carries the class. Each class adds its candidates' `System` tags under `class.` in its
      own task.
    - (d) `ids.rs` holds `CatalogueCellKey` (moved from `mod.rs`; its path and doctest kept), the
      canonical check `CatalogueCellKey::of` and the conversions between a key and its IDs.
      `BuildCatalogueCellKeyError` gains two variants rather than a second error type, since
      `of` builds a key; nothing outside the module named it.
    - (e) Two toy processes in `grid/tests.rs`, not physics: `CORE_COLLAPSE` at 512 ly (12
      candidates expected per cell, entries of two members, as T34.a's survivor) and
      `STELLAR_MERGER` at 4,096 ly (40, one member). A candidate draws its position uniformly in
      the cell, is accepted with probability its x fraction across the cell, and is kept by the
      test if it moves upwards (v_z > 0), so a cell's entries are Poisson of mean `mean` ÷ 4,
      checked by hand; every toy draw is on `selftest.stream`. The golden adds two more, at means
      of 0.7 (`XRAY_BINARY`) and 0 (`ACCRETING_WHITE_DWARF`).
  - _The lookup._ `cells_in_sphere` yields every cell of the class whose closed box meets the
    sphere of `sphere.radius()` + `LIFETIME_REACH` about its centre, clipped to the root cube, in
    ascending x, then y, then z (the key's order). It reads the sphere's radius, not its
    `padded_radius`, the grid layers' pad for drift: the reach is the class's pad (a test). The gap
    test is plan 03's walk's, whole light-years and the centre's fraction, written again here
    because `query::Walk` is private and keyed by `Layer`; each column's run grows outwards from
    its nearest cell. 4,096 ly ÷ 512 ly is eight rings of 512 ly cells and one of 4,096 ly cells,
    which a test checks from a cell's middle. A 50 ly sphere visits 2,902 cells of 512 ly and 20 of
    4,096 ly at (0, 26,000, 20) ly, and 2,890 and 20 at (1,234.4, 26,000.6, 17.2) ly, as a Minkowski
    sum of the cell and the 4,146 ly sphere estimates (1 + 6r + 3πr² + 4πr³ ÷ 3 cells for r = R ÷
    a: 2,892 and 21).
  - _Headroom._ The count debug-asserts its draw against `CatalogueSystemId::INDEX_LIMIT` and
    clamps it, as `candidate_count_from_bound` does. `check_class_headroom(class, largest_bound)`
    asks the fullest cell's mean, the bound × the class's cell volume, plus eight standard
    deviations to stay within 2²⁴, plan 03's eight (repeated as a private constant, since
    `placement`'s is private): the largest mean that passes is (√(16 + 2²⁴) − 4)² = 16,744,480.
    Each class's task finds its largest bound and calls it in its tests.
  - _One velocity on every side (T17's deferred finding)._ Rule 6 of `ClassProcess`'s contract:
    each kind of candidate takes its velocity from one function, a class candidate's drawn once
    and carried into its test and its entry, the grid side's plan 08's `epoch_velocity`, naming
    `DeathMarks::of`'s `velocity` argument; or a test shows that two routes give the same bits.
    The toy carries a velocity drawn by one function (`Toy::velocity`) and its test reads it;
    `the_velocity_has_one_route` checks that the cell, `resolve_entry` and that function see the
    same bits.
  - _Against the task text._ The trait has no cell-size method: the size is
    `ClassId::cell_log2_ly()`, read wherever a key is built, so no second copy can disagree, and a
    process for `TIDAL_DISRUPTION_VICTIM` has no `CatalogueCellKey`, so it generates and resolves
    nothing. "A candidate draw under a cap" is `candidate(.., bound)`, the cell's bound handed to
    the count and to every candidate. `members` is added, since P09.T34.a's survivor is member 1.
    `CatalogueClassCell` is `CatalogueClassCell<E>`, generic in the class's entry type, the price of
    static dispatch (point (b)); plan 11's Consumes names it without the parameter. `resolve`
    takes a `MemberVisitor` beside the lookup, for the same reason. Its checks run in this order:
    `CatalogueCellKey::of` (a free value, the unplaced class, a misaligned cell), the lookup's
    process, the index against the count, the thinning, the test, the member. Every one answers
    `NoSuchSystem`.
  - _Registration._ A class registers by one arm in `ProductionClasses::visit_class`,
    `ClassId::X => Some(visitor.visit(&XProcess::new(galaxy)))`, and not with
    `CatalogueClassSource`, which is P09.T37's and wraps the lookup: P11.T8's "registers its
    `ClassId` with plan 09's `CatalogueClassSource`" means that arm. At T32 the lookup has no arm,
    and every `111` ID answers `NoSuchSystem` from the grid; `placement::resolve` keeps
    `KindNotGenerated` for `111` (point (a)) until P09.T33.a points its arm at
    `resolve(.., &ProductionClasses, ..)` and adds the record's origin.
  - _Tests_ (`grid/tests.rs`, 26, each on seed `0x0932_0000_6772_6964`'s galaxy, shared through a
    `OnceLock`; `ids.rs`, 7; the plan-09 tag test; eight doctests, on `ClassProcess`,
    `CatalogueClassCell::generate`, `resolve_entry`, `ClassLookup` with a `ClassVisitor`,
    `resolve`, `check_class_headroom`, `cells_in_sphere` and `CatalogueCellKey::of`):
    - order independence: the cells forwards against backwards with every entry of the cell before
      resolved and a stray ID of the other class asked between, bit for bit;
      `assert_order_independent` over the cells and over 16 resolves per cell; the same seed on a
      fresh galaxy twice, and another seed's counts differ;
    - the toys' counts and entries over 576 cells each, Poisson of mean `mean` and `mean` ÷ 4 at α
      = 10⁻³;
    - resolution: every entry and member resolves to itself bit for bit, and through the toys'
      lookup; each `NoSuchSystem` case in a test of its own (a free class value, the
      tidal-disruption victims with a toy registered for them, a class with no process, a
      misaligned cell, an index at the count and at 2²⁴ − 1, a thinned candidate, one failing the
      test, a missing member);
    - one velocity: the cell's, `resolve_entry`'s and `Toy::velocity`'s bits agree, and the test
      kept only upward movers;
    - canonical IDs: aligned, member 0, round-trips through `SystemId::from_raw`, `cell_word` the
      key's word, entries in index order below the count; every low cell bit of a 4,096 ly entry
      names nothing while its aligned twin resolves;
    - headroom: the clamp, a debug assertion past 2²⁴ (`should_panic`), a cell of another class
      refused (`should_panic`), the check passes the toys and refuses a mean at 2²⁴, either side of
      16,744,480, and a NaN, infinite or negative bound; `NotPlaced` for the victims;
    - the lookup against a brute-force scan and against a plain float distance, over 47 centres
      (the Sun-like point, the origin, the root cube's faces and corners, 3,000 ly outside a face,
      40 sampled) at radii of 0.001–2,000 ly, at 512 and 4,096 ly, ascending; eight rings and one;
      the sphere's pad not read; the 50 ly counts pinned;
    - the production lookup: no class visited, and 2,000 sampled `111` IDs of every class value
      answer `NoSuchSystem`, while `placement::resolve` still answers `KindNotGenerated`;
    - `ids.rs`: `of` inverts `candidate_id` over 1,800 sampled IDs and refuses each non-canonical
      kind with its own variant; the cell field's figures; the index and member limits.
  - _Goldens._ golden_diff against `origin/rendering-and-planets` (`ae64fa92`): `rng/tags.golden`
    extended only (+1 −0, `class.cell (Cell) = 0xd675c50fb0714e5f`), no changed value;
    `catalogue_classes/grid.golden` new, at header 23, pinning four toys' cells (the key words,
    the counts on `class.cell`, the entries' IDs; means of 12, 40, 0.7 and 0, so that the Poisson
    draw's two-word, one-word and no-word paths are all pinned) and the 50 ly lookup's 20 cells
    of 4,096 ly and the count, first and last three of its 2,902 of 512 ly; no other golden;
    `GENERATOR_VERSION` 23 → 23.
  - _Not here (P09.T33–T38's)._ Every class's process, list and tags (T33–T36, T43, P11.T8);
    `recent_death_claims` and `RECENT_DEATH_CLAIM` (T35); `CatalogueClassSource`,
    `expected_hosts_in_sphere` and the merge (T37), T38, and T40's `CatalogueClassCellCache`; the
    wiring of `placement::resolve` and `SystemOrigin::CatalogueSystem` (T33.a); plan 12's
    source-horizon sweep, whose row the task that places the first production class adds
    (T33.a).
- **Deferred findings (2026-10-13 rule)**, from P09.T32:
  - _Two copies of plan 03's code._ `cells_in_sphere`'s gap test repeats `query::walk`'s (private
    and keyed by `Layer`), and `check_class_headroom` repeats `placement::headroom`'s private eight
    sigmas and its fit test, since `query/` and `placement/` were out of scope. Each copy has its
    own tests; a later task in those modules could give both a `pub(crate)` home (Rust review).
  - _The 50 ly query's cells._ A 50 ly sphere visits 2,902 cells of each 512 ly class and 20 of
    each 4,096 ly class, so with the five 512 ly classes and the three 4,096 ly ones registered a
    cold 50 ly query evaluates some 14,600 class bounds and counts. P09.T37's 5 ms budget leaves
    about 0.34 µs a cell, so T37 or T40's `CatalogueClassCellCache` will need the cache warm or a
    cheaper bound for empty cells.
  - _A contract check for every class._ Each class's task re-proves the same three properties on
    its own process: order independence, every entry resolving to its cell's bits, and the same
    seed twice. A `pub(crate)` test helper generic over `ClassProcess`, run over a list of keys,
    would let T33.a, T34.a, T43 and P11.T8 call one check (determinism review).
  - _A checked member count._ `members` returns a bare `u8` that the grid debug-asserts to 1–16
    and clamps; a `MemberCount` newtype with a checked constructor would carry the proof, and is
    cheapest before any class implements the trait (Rust review).
  - _No shared position draw for a class cell._ No helper draws a uniform position in a class cell
    (plan 01's `GenCell::position_from_words` stops at 128 ly), so each class lays out its own site
    draw; T33.a's is the first.
- **P09.T29 as built (lane `p09-centre`, 2026-10-28, on 820e5431 at `GENERATOR_VERSION` 23;
  decision-galaxy-25-train §2.2).** No output moves: nothing registers `CentreMemberSource`
  (P09.T37 does) and no walk reads `kepler_pad_for`; `position_velocity_at` is
  `Trajectory::of(..).state_at(t)`, the same operations in the same order; `resolve` of a centre
  member reads the galaxy's held model, equal to `CentreModel::new`'s (`the_galaxy_holds_its_centre`);
  and `NestedGrid::cells_touching` yields the same cells in the same order through its new per-level
  walk. No golden moves (`golden_diff.py`: "No golden files changed"), no tag is added, and
  T28.b.a's pre-pin passes by name. `Galaxy`'s `Debug` text gains a fixed `centre: CentreModelCell`
  field, which no persisted digest reads. Files: in `galaxy/features/centre/`, `source.rs` (new),
  `mod.rs`, `profile.rs`, `df.rs`, `classes.rs` and `members.rs`; `galaxy/features/nested.rs`,
  `galaxy/features/interior/table.rs`, `galaxy/motion.rs`, `galaxy/mod.rs`, `galaxy/query/mod.rs`,
  `galaxy/query/motion.rs`, `tests/galaxy_centre_query.rs` (new) and `benches/centre.rs`.
  - _Signatures._ In `features::centre`: `CentreMemberSource`, `CentreOrbitCache`,
    `CentreCellOrbits`, `NoCentreOrbitCache` and `KeepCentreOrbits`.
    - `CentreMemberSource::new(&Galaxy, &dyn CentreOrbitCache)` returns a `Result` over
      `BuildCentreError`, with `placement()`; its `SystemSource` methods panic if asked about
      another galaxy, as `KeepInteriors` does, and it suppresses nothing.
    - `CentreOrbitCache::orbits(&self, &Galaxy, &CentrePlacement, MassBand, NestedCell)` lends an
      `Arc<CentreCellOrbits>`: the sim's caller-supplied trait, as `FeatureCellCache` is; the
      server's byte-bounded cache is P09.T40.a's.
    - `KeepCentreOrbits` keeps one galaxy's cells and asserts that the placement is the galaxy's
      own. Its key is one integer: 46 ms against the tuple key's 61 ms over a century's 480,000
      band-cells (opt-level 2).
    - `CentreCellOrbits` (`new`, `trajectories`, `heap_bytes`) holds each member's `Trajectory`
      and its shell.
    - `Galaxy::centre()` returns `Result<&CentreModel, BuildCentreError>` (Provides: a bare
      `&CentreModel`): the inversions can fail.
    - `motion::plunge_pad(mu, r0, dt)` returns an `Option<LightYears>` (`None` inside the full
      scan), and it and `full_scan_radius(mu, dt)` take μ (Provides: `(r0, dt)` and `(dt)`), since
      the source pads about a μ of its own per cell.
    - Added: `motion::Trajectory` (`new`, `of`, `start`, `regime`, `orbit`, `state_at`: the law
      prepared once, for the cache), `KeplerOrbit::apocentre` and `query::kepler_pad_for`; and,
      crate-internal, `motion::SpeedBound`, `CentreProfile::outer_psi`,
      `LocalCell::farthest_radius`, `NestedGrid::level_cells_touching` and `level_touches`,
      `CentreModel::from_parts` and the model's `heap_bytes`.
    - Not built: `as_global_entry()` (P10.T4 adds it, decision §2.2). Nothing calls
      `CentrePlacement::check_index_headroom` (P09.T40.a, below).
  - _Design note 15's choice for the model: a memo in the galaxy_, built on first read from
    `centre_motion()`'s profile and the galaxy's mass function, boxed, equal always, with a fixed
    `Debug` text, as T28.b.a's motion; `with_full_potential` and `with_substellar_params` carry it,
    since neither changes what it reads. Once read with the motion it adds 470 kB to `heap_bytes`.
    `resolve` of a centre member now rests on `centre_motion()`'s `expect` (the mass inside
    10⁻³–10⁴ ly brackets the black hole's at every seed of T28.b.a's sweep), where it mapped every
    `BuildCentreError` to "no such system".
  - _The plunge pad, per cell, about an effective μ (a deviation from "per level ... √(2GM)")._
    Members are drawn in the galaxy's whole potential (`v < √(2(Ψ − E_out))`, the distribution
    function's outer cut) and orbit the mass inside their epoch radius, so they can be unbound
    about it and the pad about the black hole's √(2GM) does not hold them. A cell's members have
    μ ≤ G M(< r_far) (`CentreMotion::enclosed_mass`, r_far its farthest corner) and a Kepler energy
    under ε = Ψ_out(r_near) − E_out (`outer_psi`, the potential of the mass outside the cell's
    nearest radius and of the rest of the galaxy, non-increasing; E_out the four floors' least),
    so inside r_far they move below the escape speed of μ′ = μ + ε r_far, and their path is under
    Δ about μ′ from r_near; members beyond r_h move in straight lines under the same bound. A
    bound per level, over its nearest radius and far corner, chooses the walk's radius; each cell
    is then tested at its own pad. Margins: 10⁻⁹ on the closed forms, 10⁻⁶ on the tables' inputs
    and on the shells. The science check confirmed each step (the comparison argument, μ′, the
    outward fixed point, the energy bound, the grid pad's three cases).
  - _The full scan._ A cell whose nearest radius is inside the full scan about its own μ′ is
    scanned in full whenever the sphere comes within its members' outward reach of the black hole
    (a fixed-point bound on r^(3⁄2) over their Kepler orbits, or the straight lines beyond r_h),
    and not otherwise, so a sphere away from the black hole scans none of it. At a century it
    reaches a nearest radius of 0.389 ly over 95,696 cells (`each_level_s_reach_holds_its_cells`),
    against the plan's 0.31 ly, which is `full_scan_radius` about 4.30 × 10⁶ M☉ (0.3114 ly); at
    ±1,000 yr it reaches 2.88 ly over 179,208 cells. A member whose shell (pericentre to
    apocentre; its epoch distance with no orbit; none for a straight line) misses the sphere's is
    not propagated.
  - _The grid's Kepler-regime pad: built, not wired (decision §2.2 said it reads
    `galaxy.rules().kepler_regime`)._ `kepler_pad_for` (μ = G M(< r_h), ε = V²⁄2 at the layer's pad
    speed) is read by no walk: under the law in force every grid record moves in a straight line,
    which `pad_for` covers; `GalaxyRules` does not exist yet; and `decide_census` counts the padded
    sphere's cells against the cell budget (`query/census.rs`), so wiring it can move the census
    near the centre, against §2.2's "padding moves no output either way". It pads a 1 ly sphere at
    the black hole 1.23 ly at a century and 5.71 ly at ±1,000 yr (layers A–D), against the
    straight lines' 0.33 and 3.34. Its test holds every Kepler-regime record of layers A and D of
    the fixture, under the Kepler arm called directly, at ±100 and ±1,000 yr, each in a 0.01 ly
    sphere about where it ends and in five near the black hole; in this fixture none moved farther
    than the straight pad (the farthest 2.95 ly), so the test does not show the pad's need, which
    T28.b.a's seeds do (4.6 ly from the straight line); layer E (1.7 million records in its central
    cells) is not exercised.
  - _"The brute-force test runs under both rules" (decision §2.2)._ The members have one law, the
    Kepler orbit inside r_h, under either rule; the grid's Kepler arm is held by the pad's test;
    the range query's brute force under both arms is T28.b.b's (below).
  - _Figures (Milky Way fixture)._ Systems inside 0.31 ly from the model's classes: 62,443 (Kroupa's
    75,285) against "about 37,000" (brainstorm) and "42,000 under Kroupa's" (this plan), pinned at
    1%; realised members inside 0.312 ly: 62,534. Split: old stars 48,462, black holes 1,563, the
    young disc 8,074, the isotropic young 4,345 (Kroupa's: 58,971, 1,277, 9,773, 5,264). The old
    stars' own share is the brainstorm's 37,000 raised by the joint revision's normalisation (×1.32
    on the cusp: M(<1 pc) 8.47 × 10⁵ → 1.120 × 10⁶ M☉, not the 28% "The joint revision as built"
    quotes) at the class device's mean mass; ruling 144's young population and the black holes add
    the rest (science check). The observed cusp allows 27,000–66,000 inside that radius (Schödel et
    al. 2018, A&A 609, A27, §4.5 and Table 3; science check, rough).
  - _Tests (as run)._ `tests/galaxy_centre_query.rs`:
    `a_query_a_decade_either_way_finds_what_a_scan_of_the_inner_fifth_of_a_light_year_finds` (fast:
    ±10 yr, the inner 0.2 ly, cached and uncached, a second build of the galaxy, and
    `assert_order_independent` warm and cold); `a_query_a_century_either_way_finds_what_a_scan_of_the_inner_2_ly_finds`
    (the plan's test, `#[ignore = "slow: …"]`: five spheres at ±100 yr, 1,190,151 members, 65,178
    hits that moved in from outside their sphere, 303 s; its peak memory is not measured, some
    300 MB by estimate); `the_census_counts_the_cells_touching_the_sphere_and_the_black_hole`;
    `the_full_scan_radius_and_the_systems_inside_it_at_a_century`; `the_galaxy_holds_its_centre`;
    `a_centre_source_merges_into_the_range_query` (registered in the test only);
    `the_kepler_pad_holds_every_kepler_regime_grid_record`. Unit tests:
    `each_level_s_reach_holds_its_cells` (every owned cell at six times),
    `a_cell_s_orbits_are_its_members_on_their_shells`, `the_excess_holds_the_fastest_member_s_orbit`,
    `the_pads_bound_every_orbit_and_straight_line` (T28.a's stress set), `the_plunge_pad_is_its_closed_form`,
    `the_outer_potential_is_the_potential_less_the_pull_from_inside`,
    `a_cell_s_farthest_radius_is_its_farthest_corner_s`, and the nested walk level by level. The
    integration file's fast tests take some 14 s of test time natively.
  - _The bench_ `centre_query/century_query_cached` (0.1 ly at the black hole, +100 yr, a warm
    `KeepCentreOrbits`): **not run** with the task, pending main's quiet window. A probe at
    opt-level 2 on a shared machine measured 102 ms warm, after the shell test, the integer key and
    one classification per cell (160 ms before), against "about 10 ms": the 480,000 band-cell
    lookups of the full scan some 45 ms, the cell choice and the propagation of some 110,000
    members the rest. Cold it took 17.6 s, nearly all member placement (about 160 µs a member,
    P09.T27's draws). `CentreModel::new` is not re-benched either.
    - _Run (lane `p09-centre`, 2026-10-29, at `GENERATOR_VERSION` 23; pod, 4 vCPU, not the
      target desktop)._ `cargo bench -p hyperion-sim --bench centre`, release (thin LTO), with
      nothing else running, Criterion's 20 samples:
      - `centre_query/century_query_cached`: **159.5 ms** (159.1–159.9 ms; a second run 159.5 ms),
        against the old machine's probe of 102 ms at opt-level 2 and the plan's "about 10 ms". The
        cold query before it, 11,793 hits with 109,544 members placed and propagated, took about
        29 s from the process's start (the galaxy's and the centre's builds included, under a
        second of it; the bench times only the warm query), against the probe's 17.6 s: some
        265 µs a member.
      - `centre/model` (`CentreModel::new`, the four inversions, the marks' acceptances and the
        classes): **233.7 ms** (233.5–234.0 ms), against Design note 15's 100 ms and P09.T27's
        91 ms with three inversions.
      - `centre/profile_and_four_inversions`: **185.6 ms** (185.4–185.8 ms), against the 50 ms set
        for three inversions.
      - _Findings (2026-10-13 rule; recorded, not blockers: cost alone is not one)._ The warm
        query misses its 10 ms about sixteen times over, and `CentreModel::new` its 100 ms more
        than twice over; the galaxy pays the model once, when its centre is first read
        (`Galaxy::centre`), and a galaxy that never asks pays nothing. The pod's figures are not
        the target desktop's; the owner's machine should re-run the bench before either is tuned.
  - _For P09.T28.b.b._ Wire `kepler_pad_for` into `sphere_for_layer` behind `kepler_regime`, with
    its census moves in C1; run a range-query brute force near the centre under both arms; if a
    centre member's `position_at` is gated on the rule, gate the source's `Trajectory` with it, so
    the two agree under the held arm once P09.T37 registers the source.
  - _For P09.T40.a._ Call `CentrePlacement::check_index_headroom` beside the grid's (an
    overflowing cell clamps at 8,192 in release, a debug assertion otherwise); the server's
    `GalaxyCache` sums `heap_bytes` at insertion and misses the model read later (470 kB with the
    motion); at ±1,000 yr the full scan reaches every member within about 3 ly, some two million
    members, some 400 MB at 200 bytes each and minutes cold.
  - _For plan 10._ P10.T4 can read `Galaxy::centre()` rather than build a second model; P10.T9
    wraps `CentreMemberSource::new`, a `Result`; R06.T16.a's census reads the source through
    `SystemSource` with a shared `CentreOrbitCache`, and pays the cold placement above near the
    centre.
- **Deferred findings (2026-10-13 rule)**, from P09.T29: (1) 62,443 systems inside r_full at a
  century against the brainstorm's 37,000 (75,285 against 42,000 under Kroupa's), split above:
  drafted for the brainstorm's sign-off. (2) The full scan reaches 0.389 ly at a century against
  0.31, and the warm century query took 102 ms (opt-level 2) against "about 10 ms": the
  brainstorm's figures assume the black hole's escape speed bounds the members. (3) Members are
  drawn in the galaxy's whole potential but follow Kepler orbits about the mass inside their epoch
  radius (T28.b.a's law), so many are unbound about it: in single cells along the axis, 31% at
  0.6 ly, 29% at 1.2 ly, 39% at 2.4 ly and 27% at 4.7 ly of the Kepler-regime members (samples of
  13–557). Over the clock window they leave on hyperbolae, so the density is not stationary, where
  the brainstorm says "Kepler motion preserves [the distribution function], so the density is
  exactly the profile at every time"; P09.T31's profile test at ±1,000 yr will meet it, and it
  bears on T28.b.b's wiring: for main and the brainstorm's owner. (4) Decision §2.2's "padding
  moves no output either way" does not hold through the census's cell budget. (5) "The joint
  revision as built" says the normalisation raised the cusp by 28%; its own figures give ×1.32.
- **P09.T30.b as built (lane `p09-victims`, 2026-10-29, on 3161136a at `GENERATOR_VERSION` 23;
  the brief's "ext").** No output moves and there is no bump: the victims take the centre's
  feature-level IDs from index 1, which `resolve` answered "no such system" before; no T30.a ID
  or time, no cell member and no existing golden moves (`golden_diff.py`: two new goldens, no
  changed value), and no tag is added. New goldens `galaxy/features/centre_victims` and
  `galaxy/features/centre_luminosity` at 23. Files: `galaxy/features/centre/events/victims.rs`
  and `events/luminosity.rs` (new), `events/mod.rs`, `centre/{members,mod,source}.rs`,
  `features/members/headroom.rs`, `features/members/{level_list,placement}.rs` (docs),
  `galaxy/motion.rs` (a crate-private `MotionState::new`), `tests/galaxy_sweeps.rs` (a doc figure).
  - _API_, in `features::centre::events` (re-exported there):
    - In `victims`: `CentreVictims`, `TidalDisruptionVictim`, `DisruptionOutcome` (`Disrupted`,
      `SwallowedWhole`), `BuildCentreVictimsError`, `tidal_radius`, `schwarzschild_radius`,
      `hills_mass`, `outcome_of`, `expected_victims` and the word constants `ORBIT_WORDS`,
      `STAR_ATTEMPT_WORDS`, `STAR_ATTEMPTS`, `MIN_STAR_AGE`. `CentreVictims`:
      `new(&Galaxy, &CentreModel)`, `of(&Galaxy)`, `from_parts(seed, mass, centre, mass function)`,
      `one(galaxy, centre, index)`, `victims`, `victim(index)`, `of_disruption(EventId)`, `count`,
      `at(t)`, `model`; a victim's `id`, `index`,
      `disruption`, `time`, `member`, `record`, `class`, `star_mass`, `star_radius`, `star_phase`,
      `outcome`, `tidal_radius`, `hills_mass`, `pericentre`, `orbit_at_disruption`, `tail`,
      `state_at` and `reach_at`.
    - In `luminosity`: `luminosity_at(&CentreLuminosityModel, t) -> CentreLuminosity`,
      `CentreLuminosityModel::{new, of}`, `DisruptionTail`, `fallback_time`,
      `eddington_luminosity_watts` and the constants `QUIESCENT_LUMINOSITY_WATTS`,
      `RADIATIVE_EFFICIENCY`, `BOLOMETRIC_CORRECTION`, `XRAY_SHARE` and `TAIL_LOOK_BACK_BINS`.
      Provides' `luminosity_at(t)` takes the model, which lists the
      horizon's victims once; the result is by part (quiescence, flares, tails), in watts.
    - Crate-private: `CentreMemberRecord::from_parts`, `members::centre_composition`,
      `MotionState::new`. `resolve_centre_member_with` answers a feature-level index from 1
      through `CentreVictims::one`, which draws that victim alone.
  - _Not a `ListClass` (a deviation)._ The victims' count is the horizon's disruptions, not a
    `member.list` draw, and their places are their parabolas, not a profile's, so the centre lists
    them itself at indices 1 to N after its black hole (N = `CentreVictims::count()`, by time, then
    event ID); `registered_classes` stays empty. _For P11.T8.f, T36 and T43:_ a class listed on
    the centre after the victims starts at N + 1. `FeatureLevelList::with_classes` numbers from 1
    and its clamp does not count them, so it needs a first-index offset at the centre.
  - _Draws_, the disruption's event stream from word 0. Words 0–2: the pericentre's direction
    (isotropic) and the velocity's angle about it. From word 3, attempt k at 3 + 3k:
    - a `Mark` picking a living class (kind `Living`, expected above 0, in the classes' order) by
      its expected count over the whole cluster (lean, below);
    - the age at the disruption, uniform in the class's component from `MIN_STAR_AGE` (10⁴ yr);
    - the initial mass on the galaxy's mass function in the class's range below the turn-off at
      that age, as P09.T27's members draw theirs. An empty range passes the attempt; at most 64,
      never reached in practice (a debug assertion, then the first class).

    The star is plan 06's at its age with `StarDraws::median()` (lean: its own draws are keyed by
    its ID). The record's epoch position and velocity are its parabola's at the epoch, formal for
    a victim gone by then (59 ly out for the oldest); its origin `CentreMember { attempt }`, its
    population `NuclearDisc`, its age the drawn one plus the time since the disruption, its layer
    its initial mass's.

  - _The outcome._ SM16 §2.2's Hills mass with the IAU 2015 nominal R☉ and GM☉, 1.1434 × 10⁸ M☉
    for 1 M☉ and 1 R☉; swallowed whole at M• ≥ M_H. It is the victim's (`of_disruption` finds it
    from the event's ID); T30.a's `TidalDisruption` is unchanged.
  - _The orbit (lean)._ An exact parabola about G M• alone, not T28.b.a's μ from the epoch
    radius: the "near-parabolic" orbit as the theory of disruption takes it. Its pericentre is r_t
    when disrupted (β = 1, not SM16 equation 28's pinhole share ∝ β⁻² with f_pinhole ≈ 0.3), and
    2 r_g when swallowed, so the parabola is slower than light before the event. The state at the
    disruption is held exactly, at pericentre; `state_at(t)` propagates it up to t_d and is
    `StateAt::NoSuchSystem` after. _For P09.T28.b.b and T37:_ a victim's record says `Exists`
    after its disruption (plan 03's `Existence` has no end), and `position_velocity_at` on its
    `EpochState` would follow the Kepler regime's law, so a victim is asked through its own
    `state_at`.
  - _The tails (lean on the efficiency and the band)._
    `t_fb = (π ÷ √2) R★^{3⁄2} M• ÷ (M★ √(G M•))`, 3.54 × 10⁶ s M₆^½ m★⁻¹ r★^{3⁄2} (SM16
    equation 23; Stone, Sari and Loeb 2013,
    equation 49), then `Ṁ = (M★ ÷ 3t_fb)(Δt ÷ t_fb)^(−5⁄3)` (SSL13 equation 50), radiating
    `min(η Ṁ c², L_Edd)` with η = 0.1 (SM16 §3) and L_Edd for ionised hydrogen, 1.257 × 10³⁸
    erg/s per M☉. A share 1 ÷ 15.8 (Ho 2009's median C_X) is in 2–10 keV, so that a tail adds to
    T30.a's flares in their band, above a quiescence of 3.6 × 10³³ erg/s (Nowak et al. 2012,
    Table 1, as Neilsen et al. 2013 §4 take it), the same for every black hole. The look-back is
    the horizon, `TAIL_LOOK_BACK_BINS` = 257 (bin −257 from +H's bin 0, not T30.a's 258 bins
    counted from its edge); `luminosity_at` sums the held victims, which a test holds equal to
    `active_at` with that look-back, cut to the horizon, from −H, 0 and +H. A disruption before
    the horizon has no victim and adds no tail.
  - _For P12.T5.a._ The tails' bound is `DisruptionTail::peak_luminosity_watts`,
    `XRAY_SHARE × L_Edd(M•)`, 3.42 × 10⁴³ erg/s at the fixture; their duration is unbounded
    within the horizon (`TAIL_LOOK_BACK_BINS`); their light curve is
    `DisruptionTail::luminosity_watts_at`.
  - _The speed bound (the brief)._ `FeatureInterior::member_speed_bound` covers catalogue
    features, whose lists hold no victim (the class is the centre's alone), and its doc says so.
    `CentreMemberSource` (still unregistered) lists the victims that exist at the sphere's time,
    on their own parabolas, and passes over one whose reach misses the sphere: its pericentre plus
    `full_scan_radius(G M•, t_d − t)`, widened by 10⁻⁹, since a victim moves below the black
    hole's own escape speed. The reach changes no hit (`a_victim_s_reach_changes_no_hit`: 312
    hits, 156 passed over); the census adds one in its layer for each victim inbound at the epoch
    inside the radius. `CentreMemberSource::new` `expect`s the victims (refused only for a black
    hole below 0.56 M☉).
  - _Headroom_ (plan 11's P11.T8.e record: "each must join the candidates when it registers").
    `CentreListForecast` gains `victims()` (`expected_victims`: SM16 equation 27 over the
    horizon's 264,144 years, 27.31 at the fixture, 125 at 10⁵ M☉) and `entries()`, the victims
    then the candidates; `fits`, `margin` and the check's error read the entries, and
    `candidates()` is unchanged. `CentreListTooFull { candidates }` keeps its field's name, holds
    the entries and says "entries". Re-pinned margins at eight standard deviations: 57,685.18 and
    58,394.90 at the fixture under Chabrier's and Kroupa's functions, 46,112.75 and 52,449.08 at
    the caps (from 57,713.79, 58,423.57, 46,140.86 and 52,477.38); the 10³-seed sweep passes, its
    worst margin 46,074.8 (from 46,130.9; 407 s in the slow-test profile, pod, 4 vCPU). The
    twice-the-cap tripwire still doubles the candidates alone.
  - _Figures_, at the fixture (seed `0x0926_0000`): 26 victims (27.31 expected), all disrupted,
    of bands A–C and components 0–2, the latest 1,294 yr before the epoch (a 1.16 M☉ star), so
    the black hole is at 3.47 × 10³⁸ erg/s now, nearly all that tail. Over 200 seeds, 5,524
    victims: 4,962 on the main sequence, 89 before it, 473 below 0.1 M☉ on plan 06's cooling fits
    (phase `Substellar`); the least Hills mass 2.87 M•. At 1.36 × 10⁸ M☉, 276 of 286 are
    swallowed whole. A Sun-like tail at 4.30 × 10⁶ M☉: t_fb 84.9 d; 2.10 × 10⁴⁰ erg/s at 100 yr,
    4.52 × 10³⁸ at 1,000 yr (inside 0.3–3 × 10³⁹), 9.73 × 10³⁶ at 10⁴ yr.
  - _Tests._ `victims::tests` (8): the Hills mass; the plan's resolve-and-pericentre test (with
    `one`, `of_disruption` and the epoch state on its parabola); the words from 0; every
    main-sequence victim disrupted at Milky Way values (200 seeds, and a main-sequence star of
    0.1–150 M☉ at five ages); swallowed whole above the Hills mass; the Kepler members' loss cone
    stays empty (3,268 members of the cells about the black hole, levels 0–5, none inside it, and
    no victim on a cell); order independence; the golden (with a seed of both outcomes at 1.36 ×
    10⁸ M☉, a victim inbound at the epoch and states before a disruption). `luminosity::tests`
    (6): the closed forms; the Ponti bracket on a Sun-like tail, with the drawn tails pinned as
    they stand; the sum by part; the look-back; order independence; the golden. `source::tests`
    (2): the reach changes no hit; an inbound victim counted and found, the same in any order and
    with either cache. `cargo test -p hyperion-sim --lib -- features::centre features::members`:
    112 passed, 7 ignored, 36.8 s of tests (pod, 4 vCPU).
- **Findings of P09.T30.b (for the owner's ruling; nothing built on them moves output).**
  1. _The Ponti bracket's subject._ The plan's 0.3–3 × 10³⁹ erg/s a thousand years on holds for
     a Sun-like victim (4.52 × 10³⁸) and is asserted there. The drawn victims are mostly M dwarfs,
     and a tail scales as M★^⅓ R★: their median is 8.2 × 10³⁷ erg/s over 200 seeds (7.5 × 10³⁷ at
     the fixture), 9.7% inside (5 of the fixture's 26), pinned as they stand. Ponti et al. 2010
     state no band (their figure is Terrier et al. 2010's 1–100 keV) and no disruption; Ponti et
     al.'s 2013 review (§4.4) has flares "of the order of 10³⁹ erg s⁻¹ or higher, between ∼100
     and ∼1000 years ago". Lean: the bracket stays on the Sun-like star. _Ruled 2026-10-29 and
     built in P09.T30.c: the bracket applies to neither; see "P09.T30.c as built"._
  2. _The tails at late times._ η = 0.1 is SM16's figure for super-Eddington accretion. A tail is
     below a tenth of Eddington after about five years and at 1.3 × 10⁻⁵ of it by 1,000, where
     low-luminosity nuclei radiate "much less than the canonical value of 0.1" (Ho 2009). With
     t^(−5⁄3) for ever, the horizon's tails keep a Milky Way centre at 10³⁵–10³⁸ erg/s, the
     fixture at 3.47 × 10³⁸ now, 10⁵ times Sgr A*'s 3.6 × 10³³; Ponti et al. 2010 put Sgr A*
     below 8 × 10³⁵ erg/s until 60–90 years ago. Lean: the plan's law stands now; a ruling may let
     η fall with Ṁ below about 10⁻² Ṁ_Edd (an advection-dominated law, not checked) or end the
     tails, a version bump. _Ruled 2026-10-29 and built in P09.T30.c, behind its 25 hold: Xie
     and Yuan's falling efficiency, the tails not ended._
  3. _Weights and β (leans)._ A victim's class is drawn by its count over the whole cluster, not
     by the loss-cone flux, which favours stars near the influence radius; β is 1, not SM16's
     pinhole tail. Neither moves a figure the plan names.
  4. _Minor._ L_Edd is for ionised hydrogen; SM16 equation 24 takes 0.34 cm² g⁻¹, 17% higher.
     Ho's C_X applies below a tenth of Eddington, so not to a tail's first five years.
- **P09.T28.b.b as built (lane `p09-centre`, 2026-10-29, on 3161136a at `GENERATOR_VERSION` 23;
  decision-galaxy-25-train §1 and §2).** Built behind its hold, `KEPLER_REGIME_VERSION` = 25, so
  no output moves at 23: `golden_diff.py` finds no golden changed, T28.b.a's pre-pin
  (`regime_records_near_the_centre_are_pinned`) passes by name, and the gate's `fit-check` holds
  every table. `Galaxy`'s `Debug` text gains `rules: GalaxyRules { kepler_regime: Straight }`,
  which no persisted digest reads, and its equality compares the rules. Files: `galaxy/rules.rs`
  (new), `galaxy/motion.rs`, `galaxy/motion/kepler_regime_hold.rs` (new, the hold's tests),
  `galaxy/mod.rs` (with P09.T29's bench figure in `Galaxy::centre`'s doc),
  `galaxy/placement/{candidate,generate,resolve}.rs`, `galaxy/query/{mod,motion,result}.rs` and
  `galaxy/frame.rs`.
  - _The hold (decision §1.2–§1.3, E1–E6)._ The crate-internal `galaxy::rules::GalaxyRules` with
    one field, `kepler_regime`, and `of_version` and `#[cfg(test)] every_25_hold()`, as
    `bump-25-list.md` names them; `galaxy::motion::{KEPLER_REGIME_VERSION, KeplerRegimeRules}`
    (`Straight`, held; `Kepler`). `Galaxy` carries `rules`, which `from_params` sets from
    `of_version(GENERATOR_VERSION)` through a private `build`; `rules()` reads it, and the tests'
    doors are `#[cfg(test)] from_params_with_rules` and `#[cfg(test)] with_rules`. Three call sites, each one `match` whose held arm is the old code
    verbatim: plan 03's `position_at`; the candidate evaluation that `generate_cell` and `resolve`
    share (`in_loss_cone`); and the crate-internal `query::walk_pad`, which the range query's
    `sphere_for_layer` (now taking the galaxy) and the frame search's `search_sphere` read. Nothing
    gated feeds a static or a table (E2, E3), and no payload or protocol type moves (E5).
  - _`with_rules` (a test door the decision does not name)._ It swaps the rules of a built galaxy,
    so that D2, C1 and the frame test compare both arms on one galaxy without building its
    potential's grid twice (8 s a build in the test profile on the pod); D4 holds it equal to
    `from_params_with_rules`, with the same cells, positions and memo. It keeps the centre's motion
    and model if read, which holds while neither memo reads the rules: a hold whose arm reaches one
    (T28.d's secular term stays at its call site) must reset them there.
  - _The swap._ Under `Kepler`, `position_at` of a grid record away from the epoch is
    `position_velocity_at` from `EpochState::of_grid_record` (`motion::kepler_regime_position_at`),
    which outside the regime is the straight line to the bit.
  - _Members' `position_at`, against the task's "members and grid systems alike" (lean,
    2026-10-29; for main)._ Under either arm a feature or centre member's `position_at` is the held
    arm's: in a galaxy with kinematic tables it panics (`draw_velocity` on a record with no
    component; plan 03's Risks, "`position_at` and feature members"), and without them it is the
    epoch position, while `CentreMemberSource` moves the member on its orbit. A `SystemRecord`
    does not carry a member's velocity, and resolving the member inside `position_at` would be a
    fourth call site and a change outside the hold. The members move by their own sources:
    the centre's on its `Trajectory`, the features' on their lines (`observe::Drift::of_record`
    resolves a feature member). _Finding, a crash at version 23 already:_ the server's extinction
    request (`requests/galaxy.rs`, `LineEnd::System`) resolves any ID and calls `position_at`, so a
    feature or centre member's ID away from the epoch panics in its job; `resolve_system` already
    dispatches on the origin. Lean: route it through the origin as `resolve_system` does, a fix of
    its own outside this hold. _Closed by P07.T10.c (lane `fix-server-positions`, 2026-10-29;
    plan 07's Risks, "Fixed in P07.T10.c"):_ the crash had three paths, `extinction`,
    `scene_ship` and the scene's core, and each now places a client's ID through one function,
    `convert::system_motion`, by its origin, as `resolve_system` now does too: the centre's
    members are refused at every time until P09.T37 and P09.T40 place them. Deferred finding (2)
    below then reads, for a client's ID, that function's grid arm alone (`requests/observe.rs` and
    `scene/core.rs` call it), with `convert.rs`'s `system_place` and range rows for the grid.
  - _The carve-out, where `resolve` and the walk both see it (lean, 2026-10-29)._ The task text
    names `resolve` alone. The carve-out sits in the candidate evaluation that `generate_cell` and
    `resolve` share, after the catalogue hook, as a new `CandidateOutcome::InLossCone`, so that a
    range query never returns a system `resolve` refuses, which plan 03's placement promises. It
    applies to the grid records of every layer, the substellar two included, by the members' own
    test (`LossCone::contains`, the pericentre about the black hole alone), and only to a
    Kepler-regime record: a record outside the regime is answered by its distance, without drawing
    its velocity. A galaxy without kinematic tables carves out nothing (no velocity, no orbit) and
    is answered without building `centre_motion`, so under the new arm such a galaxy and its
    `with_full_potential` differ near the black hole by the carve-outs. A candidate a class claims
    is `ClaimedByCatalogue` first; no class claims any yet. _Finding (science check):_ the cone is
    the stars' (about 2 au at Sgr A*'s mass, some 2.6 times a Sun-like star's tidal radius, r_t ≃
    R (M_bh ÷ m)^⅓; Rees 1988, Nature 333, 523; Alexander 2017, ARA&A 55, 17, §1.2); brown dwarfs
    and rogue planets are destroyed or swallowed only inside about 0.34–0.79 au (the larger of
    their tidal radius and direct capture at 8GM ÷ c²), so for them it is 2.5–6 times too wide and
    removes a few times more than it need: here one record in 4.5 × 10⁵. As Design note 12's "as
    a member would" sets the members' test, it is recorded, not changed (with T28.b.a's deferred
    finding (1) on `LOSS_CONE_AU`'s doc).
  - _A carved-out record made by hand (lean, 2026-10-29)._ `position_at` gives it its straight
    line, the held arm's answer, rather than a panic or a position on an orbit it does not have;
    neither `generate_cell` nor `resolve` gives one.
  - _Members, and the source's `Trajectory`._ No member's `position_at` is gated: the centre's
    members move by `CentreMemberSource` on their `Trajectory`, one law under either rule (P09.T29
    as built), so the source needs no gate, and it and the grid agree under both arms once P09.T37
    registers it.
  - _The walk's pad._ Under `Kepler`, `walk_pad` is `kepler_pad_for`, and the census counts the
    same sphere's cells. The range query's `widest_sphere`, what its sources are asked about, and
    `QueryStats::padded_radius` stay the straight lines' (the sources pad for their own members;
    `padded_radius`'s doc says so). The frame search picks the sphere its sources are asked about
    as the widest of its layers' search spheres, which under the new arm are Kepler-padded near
    the black hole (its comment says so); a source reads only the sphere's centre, radius and
    time.
  - _Tests (as run)._ In `galaxy/rules.rs`: `a_galaxy_carries_the_version_s_rules` (D3: the
    version's rules, equal to a galaxy built under them explicitly, `Debug` text and all, and
    unequal to one under every 25 hold; `with_substellar_params` keeps them) and
    `each_hold_turns_on_at_its_version`. In `galaxy/frame.rs`:
    `the_search_pads_as_the_walk_does_under_either_arm` (the frame search's spheres near the black
    hole, every stellar layer at ±100 and ±1,000 yr: the straight pad under the held arm, the
    Kepler pad under the new one, wider in 18 of 20 layer-times; layer E's 10 ly at ±1,000 yr
    already holds the orbits). In `galaxy/motion/kepler_regime_hold.rs`, fast:
    `the_kepler_regime_is_held_only_until_the_25_bump` (the tripwire, E6);
    `the_kepler_arm_moves_records_inside_the_radius_and_no_others_pinned` (D2, in the wasip1
    suite: the fixture's 12,671 layer-B records within 1.5 r_h, 3,286 in the regime and 9,385
    outside, at ±100 and ±1,000 yr; at the epoch both arms give the epoch position; the held arm is
    the straight line; the new arm moves every Kepler-regime record onto `position_velocity_at`'s
    state, 13,144 record-times, the farthest 1.40 ly, and no other; the cells are the held arm's
    less the loss cone's, none here; the walks' pad is `pad_for` under the held arm and
    `kepler_pad_for` under the new one, to the bit; the counts and a digest of the new arm's
    positions, `0xdc2a4657d88c161e` on x86-64, are pinned, which T28.d's secular term may re-pin by
    name, §1.3); and `the_kepler_arm_gives_the_same_seed_the_same_answers_twice_and_in_any_order`
    (D4 on seed 0x5eed0000c0ffee00: built under the rules, it equals the galaxy given them by
    `with_rules`, with the same cell, positions and centre motion to the bit; the census's
    carved-out rogue planet is resolved under the held arm, and under the new one is
    `InLossCone`, "no such system" and missing from its cell, which loses nothing else; and
    `assert_order_independent` over its cell, a central layer-B cell, two resolutions, three
    positions and a pad, once on one galaxy and once on a fresh copy each time, so that the
    centre's motion is built from inside cell generation, first or last). Slow, by name:
    `the_kepler_arm_s_census_near_the_black_hole` (C1, below, 426 s on the pod in the test
    profile) and `a_range_query_near_the_black_hole_finds_what_a_scan_finds_under_either_arm`
    (four spheres of 0.5–2 ly at and near the black hole at ±100 and ±1,000 yr, grid only, against
    a scan of every Kepler-regime record and the straight-regime records within the straight pad:
    3,674 hits under the held arm and 3,817 under the new one, 369 s; no stellar hit came from
    beyond the straight pad, which C1 shows for the rogue planets). The fast tests take 37 s of
    test time on the pod, each in its own nextest process (D4 22 s, D2 9.7 s). T28.b.a's pre-pin
    passes by name (223 s).
  - _"Positions at t = 0 unchanged for every golden system", as tested._ No integration golden can
    run under every 25 hold (`from_params_with_rules` is `#[cfg(test)]`): the tests hold it for
    the pre-pin's records (C1) and D2's sample, and everywhere else `position_at` returns the epoch
    position before it reads the rules. Under the new arm `0xcfffe0003fff0b41` no longer exists, so
    `regime.golden` loses it at the bump.
  - _Census C1 (decision §1.6), under every 25 hold against the held arm._ Over T28.b.a's pre-pin
    (the Milky Way fixture, seeds 0x5eed0000c0ffee00 and 0x2: 790,511 grid records, 453,819 in the
    regime; ±100 yr and ±1,000 yr, which is ±H): every Kepler-regime record moves at every time,
    and none outside the regime; every mover is finite; the largest moves are 4.04, 4.63 and
    3.94 ly at −1,000 yr (rogue planets; the stellar layers at most 2.47, 3.70 and 2.30 ly), as
    T28.b.a's tally forecast, and at ±100 yr 0.34, 0.43 and 0.31 ly; every record lies within the
    walk's Kepler pad of where it ends, and some rogue planets passing near the black hole faster
    than 1,000 km/s end farther from their epoch positions than the straight lines' pad (5
    record-times in the fixture, at ±100 yr; 55 in seed 0x5eed, 10–16 at each time; none in seed
    0x2 or in a stellar layer), which only the Kepler pad holds. The one carve-out in the sample is
    rogue planet `0xcfffe0003fff0b41` of seed 0x5eed0000c0ffee00, 2.54 ly from the black hole, the
    one T28.b.a forecast: "no such system" under the new arm. The substellar layers are sampled
    within 4 ly, as in the pre-pin, so their records between 4 ly and r_h, most of the regime's by
    number, are not counted. The range query's census (48 queries a galaxy:
    spheres of 0.5–50 ly at and near the black hole, stellar and with the substellar layers, at
    ±100 and ±1,000 yr) moves for none of the 144: near the black hole the census limit stops
    first and the 2²⁰-cell budget never binds. The cells the census counts rise: a 3 ly sphere
    12.2 ly out at ±1,000 yr from 38 to 158–174, and a 1 ly sphere 3.2 ly out from 40 to 46–66
    (stellar) and from 74 to 124–208 (with the substellar layers). So P09.T29's finding (4) stands
    for a caller's smaller budget, and at the default budgets the pad moved no census here.
  - _Unbound orbits (P09.T29's deferred finding (3))._ The held arm does not change the centre's
    members, which have one law under either rule. Under the new arm the grid's Kepler-regime
    records take the same law with plan 08's velocities, and 78.6% of them are unbound about the
    mass inside their epoch radius (the fixture 79.9%, 0x5eed 73.2%, 0x2 87.3%; the stellar layers
    alone 80.6%, 74.1% and 87.6%). The science check traces it to the tracers, not to the draw's
    cut: plan 08's populations there (the nuclear disc, the bulge, the bar) are cored over tens to
    hundreds of light-years, and their Jeans dispersions at a few light-years, about 170–290 km/s
    per axis at 1–5 ly in the black hole's and cluster's well alone, carry the whole well's depth,
    against a Kepler escape speed of 222 km/s about M(< 3 ly) = 5.28 × 10⁶ M☉ (circular 157 km/s);
    they pass through on orbits that reach far beyond r₀, hyperbolae about that point mass, and the
    cut, 1,000 km/s there, removes under 10⁻⁴ of them (its estimate, 68–79% unbound, matches; not
    re-checked at source). The centre's members, at about 100 km/s, are 27–39% unbound by the same
    measure (P09.T29's (3)). They move no farther than the figures above over the clock window
    (a crossing at 3 ly takes some 2,600 yr), but the density inside r_h is not stationary under
    the new arm, where the brainstorm's "Orbits and time" says "the density is exactly the profile
    at every time". Recorded for main and the
    brainstorm's owner with P09.T29's (3), not as a blocker: no state, class or value is wrong or
    non-finite, and the walks hold every move.
  - _Deferred findings (2026-10-13 rule), for the 25 wiring and the plans named._ (1)
    `observe::Drift::of_record`, which the sky census's retarded times and plan 12 read, still
    draws a Kepler-regime grid record's straight line under the new arm, so it and `position_at`
    part inside r_h at 25 (the sky census's pads stay the straight lines', consistent with it);
    (2) the server reports `epoch_velocity` beside `position_at` (`convert.rs`,
    `requests/observe.rs`, `scene/core.rs`), which inside r_h is not the orbit's velocity at t
    (`position_velocity_at` gives both); (3) feature members inside r_h keep their source's
    straight lines under either arm, though the task text says "members and grid systems alike";
    (4) under the new arm a galaxy with its kinematic tables builds `centre_motion` (64 ms) with
    the first record it places, and every galaxy with its first range query or frame search
    (`kepler_pad_for` reads it); (5) `displaced::site::explosion_site` drifts a grid record in a
    straight line to its death, and plan 08's test holds the site equal to `position_at` inside the
    clock window, so under the new arm a Kepler-regime layer-E record's site parts from its orbit
    (nothing reads it until P09.T35 sets `RECENT_DEATH_CLAIM`, train 26); (6) from 25 a cell
    depends on whether its galaxy has its kinematic tables (the carve-out), which the server's
    `(GalaxyKey, CellKey)` cell key does not say: every production galaxy has them
    (`compute/galaxies.rs`), and a debug assertion in the shared cell cache, or the tables' state
    in the key, would keep it so (determinism audit). None moves output at 23.
  - _D5._ The new arm propagates on `position_at`'s path, so the D2 test, named `pinned`, runs in
    the wasip1 suite (wasmtime 49.0.1). Run by name on the pod before the gate, it passes in
    12.1 s with the digest blessed natively on x86-64; the gate's wasip1 suite runs it again with
    the module's other fast tests.
- **P09.T39 as built (lane `p09-wire`, 2026-10-29, on 7a674ea at `GENERATOR_VERSION` 23 and
  `PROTOCOL_VERSION` 3; no output moves).** Types only. `hyperion-protocol` gains plan 09's four
  kinds and their payloads (`features.rs`, new; `FeatureIdHex` in `primitives.rs`; the kinds in
  `envelope.rs`, appended to `REQUEST_KINDS` as `features_in_range`, `galaxy_features`,
  `feature_detail` and `black_hole_state`) and the range row's optional `origin`, `member_class`
  and `supernova` (`galaxy.rs`). The server answers each kind `unsupported` through
  `not_served_yet` until P09.T40.b, as plan 04's design note 15 has a server answer a kind it does
  not serve, classes `features_in_range` and `galaxy_features` large in `is_large`, and writes the
  three row fields `None` (`convert.rs`, and the range-query bench's row), so no wire form, server
  golden, protocol fixture or client fixture moves. `just gen-protocol` adds 40 binding files and
  changes three, every new type exported from `packages/protocol/src/index.ts`; the client's fake
  server in `apps/hyperion/src/tools/placeShip.test.ts` lists the four kinds. Every shape is free to
  change until the first build that sends a value of it (P09.T40). Choices the task leaves open
  (lean, 2026-10-29), as built:
  - _IDs._ `FeatureIdHex` is the 64-bit word a feature's members share with their member bits
    zeroed: `FeatureRef::object_word` for a catalogue feature, `f000000000000000` for the galactic
    centre (a wire convention the sim does not name, which P09.T40 resolves). The word is also a
    well-formed member `SystemId` (a zero slot is band A's member 0 of nested cell 0), so T40
    refuses a word that does not round-trip.
  - _Requests._ `FeatureDetailRequest` is `{ universe, feature, time }`, not `{ feature }`: an ID
    names no universe, and a feature's kind and structure depend on the time. Each `kinds` list is
    non-empty with no repeat, which T40.b validates.
  - _`features_in_range`._ `FeatureSummary` adds `designation`, for P09.T41.c, beside the plan's
    fields; `age_myr` (a cloud has none), `emission` and `bubble_radius_ly` are nullable, and
    `extent_ly` is the radius at the time, not `reach()`. A feature is within range when its sphere
    of `extent_ly` meets the query's. The census has a line per requested kind,
    `{ kind, returned, status }`, with `status` one of `included`, `over_cell_budget` and
    `not_generated` (streams and dwarf cores, plan 10's global list); the plan names no limit, so
    there is no `expected` and no `over_limit`.
  - _`galaxy_features`._ Its kinds are `MapFeatureKindWire` (`globular`, `galactic_centre`,
    `bright_shell`), not `FeatureKindWire`, because a bright shell is no `FeatureKind`. A bright
    shell is a remnant whose shell is non-radiative (free expansion or Sedov–Taylor), sent as
    `BrightShell { system, designation, site, age_yr, shell }`. The answer's lists are tagged by
    kind, so that a kind fixes its entries' type (TypeScript review), each `complete` or
    `not_generated`. There is no `pending` on the wire: the request waits, as the density map's
    does, and P09.T41.b's "pending" is the client's own request state.
  - _`feature_detail`._ The reply echoes `universe` and `time` and carries the `FeatureSummary`
    beside a structure tagged `cluster` (radii, central escape speed, relaxation time, the
    core-collapse flag, the black-hole count and the classes), `centre` (the influence and
    loss-cone radii and the classes) or `cloud` (core radius, central density). `ClassCountWire` is
    `{ class, initial_mass_min_msun, initial_mass_max_msun, mean_mass_msun, expected }`; a profile's
    shape and a tail's length are not sent. `MemberClassWire` is tagged
    `cluster { kind, band, generation, multiplicity }`, `centre { kind, band, episode, tracer }` or
    `listed { class }` (an entry of a feature-level list), with `band` as `MassLayer`. The episode
    is the index into the centre's `age_components`, without which its four `Stars` episodes would
    share one class (plan-conformance review).
  - _The shell._ `ShellSummary` is the shell alone (radius, shock speed, phase, emission flags). The
    remnant's offset, its bow shock, its nebula and a Type Ia survivor's offset are a
    `RemnantSummary` beside it, as the sim splits `ShellState` and `RemnantState`, so that a bare
    remnant keeps them; the plan's text puts the offset and the nebula in `ShellSummary`.
    `SupernovaStateWire` is `progenitor { until_explosion_yr, pair_period_s }`,
    `supernova { age_yr, shell, remnant }` or `bare_remnant { age_yr, remnant }`.
  - _`black_hole_state`._ It adds the black hole's `id` (`f000000700000000`). `luminosity` is
    optional on the wire, as the lane's brief set it while P09.T30.b was building `luminosity_at`;
    that is in the tree now, and T40 may send it on every answer. It comes by part (quiescent,
    flares, tails, total; unabsorbed 2–10 keV). The plan's "recent disruptions" are every
    disruption at or before the time, newest first, back to the start of the source horizon
    (`TidalDisruptionWire { victim, time, star_mass_msun, outcome, peak_luminosity_w }`).
  - _Origins._ `SystemOriginWire` is `grid` (with no component: the row names its population),
    `feature_member { feature }`, `centre_member` and `catalogue_system { class }`, and
    `CatalogueClassWire` holds the registry's nine values in value order. The sim's
    `SystemOrigin::CatalogueSystem(ClassId)` is P09.T33.a's and was not in this lane's tree: nothing
    here changes when it lands, and T40 converts it by `ClassId::value()`. The row's `supernova` is
    boxed, since few rows carry one.
  - _Not taken (reviews)._ Nesting `member_class` and `supernova` under `origin` (TypeScript
    review): the plan and the lane's brief name three row fields, a supernova host can be a feature
    member as well as a catalogue system, and a member class spans two origins; a narrowing helper
    is P09.T41.c's if it wants one. Tagging `RemnantSummary` by whether a compact remnant exists: it
    mirrors the sim's `RemnantState`, whose offset is absent for a remnant without a kick too.
  - _Open, for the owner._ (1) `SystemOriginWire` is a closed enum, and plan 10's P10.T10.a adds
    `stream` and `dwarf_core`; pinned content (`110`) would add another. Lean: name it growing in
    plan 04's reserved list, as `Population` is, with P09.T41.c reading an unknown origin as
    Missing; until that is ruled, P09.T40 sends no `origin` before P10.T10.a's variants exist, or
    P10.T10.a bumps. (2) `CatalogueClassWire` likewise: the registry leaves values 9–63 free and
    design note 16 appends later classes. Lean: growing too.
  - _Ruled (2026-10-30, main)._ `SystemOriginWire`, `CatalogueClassWire` and `MemberClassWire` join
    plan 04's growing list (its reserved list), before any build sent a value of any of them, so
    no bump. Every row carries `origin` from the build that registers the features' source, for the
    variants the registered sources place; P09.T33.a's `catalogue_system` and P10.T10.a's `stream`
    and `dwarf_core` join without a bump, as would a member class of plan 10's own. Registering
    `FeatureMemberSource` waits for P09.T2.c, at version 26, since members registered before it are
    counted twice (`decision-r06-t16a-scope.md`): P09.T40.a builds the registration behind a switch
    that is off at 23, so no row carries an `origin` before then and the range query's replies do
    not change. P09.T41.c reads an origin, a catalogue class or a member class it does not know as
    Missing.
  - _Tests._ Wire forms with JSON round trips for every request and reply (features in range, also
    with nothing in range; the map's lists of each kind and status; a cluster's, the centre's and a
    cloud's structure; the black hole with and without a luminosity), every `SystemOriginWire`,
    `SupernovaStateWire` and `MemberClassWire` variant and every string enum, and the row with each
    new field absent (the existing rows) and present; `FeatureIdHex`'s form and its refusals;
    `request_kinds_are_pinned` and the envelope's walk over every kind; the server's
    `kinds_without_a_handler_are_answered_unsupported`, now over nine kinds, and over the WebSocket
    `a_feature_kind_is_answered_unsupported_until_it_is_served`.
- **P09.T30.c as built (lane `p09-t30c`, 2026-10-30, at `GENERATOR_VERSION` 23; the research lane
  `research-p09-t30b`'s advice, §1 and §2, as main ruled it on 2026-10-29; decision-galaxy-25-train
  §1).** Built behind its hold, `CENTRE_QUIESCENCE_VERSION` = 25, so no output moves at 23:
  `golden_diff.py --base` the lane's base finds no golden changed. Files:
  `galaxy/features/centre/events/luminosity.rs`, `events/luminosity/centre_quiescence_hold.rs`
  (new, the hold's tests), `events/victims.rs` and `galaxy/rules.rs`; the brainstorm's centre
  paragraph (its own commit, as ruled) and this plan's T30.b test sentence and Generator version.
  Finding 2 of P09.T30.b is closed by it, and finding 1 as its record says.
  - _The hold (E1–E6)._ `luminosity::{CENTRE_QUIESCENCE_VERSION, CentreQuiescenceRules}`
    (`ConstantEfficiency`, held; `HotFlow`), crate-private, and `GalaxyRules::centre_quiescence`,
    which `every_25_hold()` turns on with nothing to edit. It is read through the `Galaxy` (E1)
    where its victims are drawn, `CentreVictims::new` and `one`, so `CentreLuminosityModel::new`
    and `of` and `CentreMemberSource::new` take it; each `DisruptionTail` carries its galaxy's arm.
    It reaches no memo of the galaxy (the centre's model holds no victim), so `Galaxy::with_rules`
    needs no reset for it, and it feeds no static (E2), table (E3) or wire type (E5). _One call
    site, not the advice's two (lean):_ the private `DisruptionTail::accreted_watts(rate)`, a
    `match` whose held arm is the old expression verbatim, `RADIATIVE_EFFICIENCY * rate * c * c`
    in the same order, which `bolometric_watts_at` and `peak_luminosity_watts` share, so the bump
    deletes one `match`. `Galaxy`'s `Debug` text gains `centre_quiescence: ConstantEfficiency`,
    and a tail's `Debug` and equality its arm; no persisted digest reads either.
  - _Doors (lean)._ The crate-private `DisruptionTail::new_with_rules` and
    `CentreVictims::from_parts_with_rules` take the arm; the public `DisruptionTail::new` and
    `CentreVictims::from_parts`, which take no galaxy, take the generator version's
    (`GalaxyRules::of_version(GENERATOR_VERSION)`), the held arm until the bump and the one law
    after. `radiative_efficiency(eddington_ratio)`, `HOT_FLOW_EFFICIENCY` and
    `HOT_FLOW_RATE_SCALE` are crate-private until the bump, and `EDDINGTON_RATE_PER_WATT`
    (`10 ÷ c²`, Xie and Yuan's `Ṁ_Edd = 10 L_Edd ÷ c²`) private.
  - _The law._ `ε(ṁ) = min(η, max(min(ε₁, ε₂), ε₃))`, `εᵢ = ε₀ (ṁ ÷ 10⁻²)^a`: the advice's
    piecewise form exactly (each range between where it meets its neighbours, 2.8802 × 10⁻⁵ and
    3.3929 × 10⁻³, η from 6.2265 × 10⁻³), continuous and non-decreasing by construction, so a
    tail's light falls monotonically from t_fb. Three `powf` a call, where the piecewise form
    would take one; nothing reads it on a hot path.
  - _Census C1, what moves under every 25 hold_ (the fixture `0x0926_0000`, against the held
    arm). The victims do not move: records, stars, outcomes, orbits, and the tails' timing and
    peaks, since every peak at the Milky Way's black hole is above the switch (15 Eddington rates
    for a Sun-like star). Of the `centre_luminosity` golden's 52 values 33 move, every one
    finite and fainter, and no other: a Sun-like tail at 100, 1,000 and 10⁵ yr, 2.0968 × 10⁴⁰ →
    9.3276 × 10³⁹, 4.5175 × 10³⁸ → 9.5944 × 10³⁷ and 2.0968 × 10³⁵ → 3.0340 × 10³² erg/s (its
    1-yr and 10-yr values do not: it is η's for its first 24.9 yr); and the tails and totals at
    all fifteen instants, by 0.028–0.39, the fixture's at −1,000, 0 and +1,000 yr 4.0174 ×
    10³⁹ → 1.5726 × 10³⁹, 3.4676 × 10³⁸ → 6.0033 × 10³⁷ and 1.3674 × 10³⁸ → 1.2468 × 10³⁷
    erg/s (totals). The flares, the Eddington luminosity and the fallback time do not move. The
    new arm's values have the digest `0xacdcf23d013fd38e` (x86-64). A Sun-like tail falls to the
    quiescence 4.0677 × 10⁴ yr on, where η takes 1.1459 × 10⁶.
  - _Ponti et al.'s limit_ (2010: at most 10³⁶ erg/s, the mean over the last 60–90 years), at the
    epoch over P09.T30.b's 200 seeds at Milky Way values, quiescence, flares and tails: under every
    25 hold 149 seeds are within it (the least 4.544 × 10³³, median 9.574 × 10³⁴, 90th percentile
    1.719 × 10³⁷ erg/s; 35 at or below 10³⁴ and 134 at or below 3.5 × 10³⁵, the limit in 2–10 keV
    if it is Terrier et al.'s 1–100 keV); under the held law 1 is (the least 9.211 × 10³⁵, median
    9.170 × 10³⁶, 90th percentile 1.686 × 10³⁸). The greatest, 3.421 × 10⁴³ under either, is a
    seed disrupting a star in the year before the epoch. The counts are the research probe's,
    with the flares in.
  - _The drawn tails a thousand years on_, re-pinned as they stand: over the 200 seeds' 5,524,
    a median of 8.199 × 10³⁷ erg/s and 538 in the old 0.3–3 × 10³⁹ under η, 5.742 × 10³⁶ and 35
    under the hot flow; the fixture's 26, a median of 7.472 × 10³⁷ under η.
  - _Checks of the law._ At EHT 2022 Paper V's rates for Sgr A*, 5.2 and 9.5 × 10⁻⁹ M☉/yr, it
    gives 1.7629 and 4.7651 × 10³⁵ erg/s bolometric, 3.86 and 1.93 times below EHT's 6.8 and
    9.2 × 10³⁵. Xie and Yuan's Eddington rate at the fixture's black hole is 6.0146 × 10²¹ kg/s
    (0.095456 M☉/yr). A peak below the switch is lowered: a 1 M☉, 10 R☉ star at 10⁸ M☉ peaks at
    4.2 × 10⁻³ Eddington rates, at 0.6494 of η's light; `peak_luminosity_watts` is a tail's light
    at t_fb under either arm.
  - _Deviations (leans, 2026-10-30)._ (1) The Ponti test is in the fast tier, not the slow one
    the lane's brief named: its 200-seed sweep takes about 2 s in the test profile (pod, 8 vCPU),
    as T30.b's own sweep does, so it runs in every gate and in the wasip1 suite. (2) It reads
    `luminosity_at`'s total, the flares in, where the probe left them out; the counts are the
    same. (3) It tests an instant, the epoch, against a mean over 60–90 years, as ruled. (4) _D1._
    No extension pre-pin: `centre_luminosity`, blessed at 23, already pins the region the hold
    moves (a Sun-like tail at five times, the fixture's tails and totals at fifteen instants), and
    the hold leaves it byte-identical. (5) The hold's constant is `CENTRE_QUIESCENCE_VERSION`, as
    the lane's brief named it, not the advice's `TAIL_EFFICIENCY_VERSION`. (6) The EHT check's
    bracket, 1.9–4.0, holds the measured 1.93 and 3.86, which the advice rounds to "2–4 times";
    EHT's §6 gives 6.8–9.8 × 10³⁵ erg/s where its abstract gives 6.8–9.2, and unpaired the ratio
    spans 1.4–5.2 (science check). Against EHT's own efficiency, (1.3–3.0) × 10⁻³, the law's
    6.0–8.9 × 10⁻⁴ is 1.5–5 times lower.
  - _The fast suite._ The hold's six tests and the module's three new ones add 10.7 s of test time
    (pod, 8 vCPU; the renamed closed-form test went from 1.8 s to 1.1 s), most of it the fixture
    galaxy that each of nextest's processes builds; the slowest, the drawn tails' and Ponti's
    sweeps, take 2.4 s each. The centre's, the members' and the rules' tests, under
    `cargo nextest run -p hyperion-sim --lib` over `features::centre`, `features::members` and
    `galaxy::rules`: 122 passed, 45.6 s.
  - _Tests (as run)._ In `luminosity::tests`: `a_sun_like_tail_follows_its_closed_forms` (from
    T30.b's `a_thousand_years_after_a_disruption_its_tail_gives_ponti_s_luminosity`, without the
    bracket, under both arms: dark before t_fb, Eddington's share at it, half the star back, the
    rate's t^(−5⁄3), the light's t^(−5⁄3) under η and `Ṁ^1.65` on the lowest range under the hot
    flow, the research's six figures a tail),
    `the_drawn_tails_a_thousand_years_on_are_pinned_as_they_stand`,
    `the_hot_flow_s_efficiency_is_xie_and_yuan_s_fit_made_continuous` (the switches, the published
    bounds within 1% and 3%, the 2.9% dip the printed bounds would leave, each range between its
    switches, η above, continuity, non-decreasing over 10⁻¹²–10² Eddington rates on 140,001
    points, EHT's check) and `a_tail_s_light_falls_from_its_peak_under_either_efficiency`;
    `centre_luminosity_is_pinned` writes `centre_luminosity_entries`, the same bytes. In
    `luminosity/centre_quiescence_hold.rs`: the tripwire,
    `the_centre_s_quiescence_is_held_only_until_the_25_bump` (E6, with the doors that take no
    galaxy on the version's arm); `the_hot_flow_moves_only_the_centre_s_luminosity_pinned` (D2
    and C1 above; `CentreVictims::one`, `resolve`'s door, equal to the list's victim at every
    index under the new arm); `the_hot_flow_s_law_beyond_the_golden_is_pinned` (a second digest,
    `0x7fe1828ba0fad5a7`, of `radiative_efficiency` inside each range and above the switch and of
    the 10 R☉ tail's peak and its light ten fallback times on, where the golden does not reach;
    both pinned tests run in the wasip1 suite by their names);
    `the_hot_flow_gives_the_same_seed_the_same_light_twice_and_in_any_order` (D4: a galaxy built
    under every 25 hold equals the fixture given them, its victims and light to the bit, and
    `assert_order_independent` over 119 instants); and
    `at_milky_way_values_the_centre_is_within_ponti_s_limit_at_the_epoch_in_most_seeds`. In
    `galaxy/rules.rs`, `each_hold_turns_on_at_its_version` covers the field.
  - _D5._ The new arm adds three `powf` a tail on its path. Run by name on the pod before the
    gate, the module's 14 tests and the rules' 2 pass on wasip1 under wasmtime 49.0.1 (the two
    pinned tests in 2.2 s and 0.4 s), with both digests blessed natively on x86-64; the gate's
    wasip1 suite runs them again.
  - _Reviews._ The Rust, determinism, plan-conformance and science reviews' fixes are in: plain
    code, not links, for the crate-private names in public docs (rustdoc's private links); the
    units in the rates' names; `one` and the law beyond the golden pinned under the new arm; and
    the science check's corrections, some of them to the advice (the outbursts' energy, the
    debris's apocentres, where a peak falls below the switch, Ho's quote, the switch's place
    against Maccarone's range and Xie and Yuan's two-phase flow). It could not reach Nowak et al.
    2012's 3.6 × 10³³ erg/s (P09.T30.b's quiescence, unchanged here) at source. _For main (lean:
    keep the ruled text):_ the brainstorm's "radiatively inefficient once the fallback falls below
    a few thousandths of Eddington's rate" is the label of the regime, but at δ = 0.5 the law is
    only 2–3 times below η at 10⁻³–3 × 10⁻⁵ Eddington rates ("the efficiency of ADAFs is not as
    low as people sometimes imagine", Xie and Yuan 2012, §4.1), and 10 times below near 4 × 10⁻⁶.
  - _For P12.T5.a._ The tails' bound is still `DisruptionTail::peak_luminosity_watts`, exact
    under either arm; their duration still the horizon (`TAIL_LOOK_BACK_BINS`), though from 25 a
    Sun-like tail is below the quiescence some 4 × 10⁴ yr on; their light curve
    `DisruptionTail::luminosity_watts_at`. Plan 12's text is not changed here.
  - _Gaps (recorded, not built; the advice's, as main ruled)._ (1) _The echoes' outbursts_: about
    10³⁹ erg/s for a few to ten years, 10⁴⁷ erg or so each (the Bridge's fluence is about
    10⁴⁸ n₃⁻¹ erg, Churazov et al. 2017, MNRAS 465, 45), a 5–10% duty cycle over the past
    millennium (Ponti et al. 2015, MNRAS 453, 172, §8.4.3, after Ponti et al. 2013; Clavel et al.
    2013; Terrier et al. 2018, A&A 612, A102), of unsettled cause: neither T30.a's
    flares (at most 3.6 × 10³⁵ erg/s and 8,175 s) nor T30.b's tails; a candidate for a later centre
    task. (2) _Partial disruptions' steeper tails_: t^(−9⁄4) when a core survives (Coughlin and
    Nixon 2019, ApJL 883, L17), n ≃ −2.2 for about half of all disruptions (Guillochon and
    Ramirez-Ruiz 2013, ApJ 767, 25); at β = 1 a γ = 4⁄3 Sun-like star is only partly disrupted
    (Stone and Metzger 2016, §3), and 24% of the drawn victims are above 0.5 M☉. It needs a drawn
    β, new draw words and a bump. (3) _An X-ray share that falls at very low rates_: Ho's
    C_X = 15.8 is extrapolated below about 10⁻⁶ L_Edd and errs bright there; Sgr A*'s own is about
    190–380 (EHT 2022's L_bol over Baganoff et al. 2003's and Nowak et al. 2012's L_X), and with
    300 below 10⁻⁷ L_Edd the research probe's median at the epoch falls about elevenfold. A share
    that falls with ṁ needs a sourced law and a ruling. Two assumptions are also unchecked at
    source: that accretion follows the fallback in the thin-disc phase (≲25 yr), where the viscous
    time at 2 r_t is of order 10 yr; and that debris returning after 10³–10⁵ yr, from apocentres of
    0.16–3.4 pc (semi-major axes of 0.08–1.7 pc, where the advice took them for apocentres), does
    so ballistically through Sgr A*'s gas.
  - _For the 25 wiring (C2)._ Delete in its own checkpoint, blessed at 24:
    `CENTRE_QUIESCENCE_VERSION`, `CentreQuiescenceRules` with `ConstantEfficiency`,
    `GalaxyRules::centre_quiescence`, the `match` in `accreted_watts`, the tail's `rules` field,
    the doors `new_with_rules` and `from_parts_with_rules` (folded into `new` and `from_parts`)
    and the tripwire. `centre_luminosity` moves, its 33 values as above and no other golden; the
    hold's tests become the version's (the digest, the 149 and the 33 stay; the held arm's pins,
    the 1 of 200 among them, go).
- **Deferred findings (2026-10-13 rule)**, from P12.T5.a.2 (main's ruling of 2026-10-30; plan 12's
  Risks, "P12.T5.a.2's citations, checked at source"). Recorded and deferred, not built; nothing in
  this plan changes with them.
  - _The Type Ia template rises too fast_ (P09.T19.b; `catalogue_classes::supernova`, τ_m = 13 d).
    - It peaks at 15.1 days. That is earlier than all 18 of Firth et al.'s (2015, MNRAS 446, 3895)
      supernovae, whose rise is 18.98 ± 0.54 days (15.98–24.7). It is 1.4 days under Scalzo et
      al.'s (2014, MNRAS 440, 1498, equation 6) bolometric rise of 16.5 ± 2 days. Contardo et al.
      (2000) give 16–20 days, and Stritzinger et al. (2006) 19 days.
    - Its peak is 0.74 of the heating rate at that time, where Arnett's rule takes α = 1
      (Stritzinger et al.) or 1.2 (Scalzo et al.).
    - Retuned to a 17–19 day rise with α ≈ 1, the peak becomes 1.18–1.31 × 10⁴³ erg/s, and plan
      12's `TYPE_IA_PEAK` must follow it: its test holds the bound to the template.
    - Design note 18 names no source for the templates, so this is a choice of this plan, not a
      contradiction of the brainstorm. `LightCurve::of`'s "about 15 days in" is the template's
      figure, not an observed one.
    - If the templates feed recorded output by then, the retune rides a bump train.
  - _Ho's 15.8 is applied outside its range_ (P09.T30.b's tails;
    `features::centre::events::luminosity::BOLOMETRIC_CORRECTION`).
    - Ho (2009, ApJ 699, 626, §3) gives 15.8 as the median for nuclei below a tenth of
      Eddington, but the tails apply it up to the Eddington cap.
    - As plan 12's X-ray bound it errs high, which is safe for reach. The tails' X-ray light near
      the cap is overstated, though, since a more luminous nucleus puts a smaller share in X-rays
      (Vasudevan and Fabian 2007; recalled, not read at source).
    - For P09.T30.c, whose law replaces T30.b's. The three corrections to that file's docs that
      plan 12's record lists, (c) to (e), ride with it.
- **P09.T28.c as built (lane `p09-secular`, 2026-10-29, on 5bec9f1 at `GENERATOR_VERSION` 23;
  decision-galaxy-25-train §1.7 and §2.2: pure, no hold).** No output moves: nothing calls the new
  functions (P14.T29 wires them in plan 14's train), no golden is added or changed, and no tag or
  stream is touched. Files: `galaxy/motion/tidal.rs` (new) and one `pub mod tidal;` line in
  `galaxy/motion.rs`.
  - _Signatures._ An inherent method in the child module,
    `CentreMotion::tidal_radius_at_pericentre(&self, &KeplerOrbit, SolarMasses) -> Metres`, and
    the one call for P14.T29, `motion::tidal::tidal_radius(&Galaxy, &EpochState, SolarMasses) -> Metres`:
    in the Kepler regime with an orbit (`CentreMotion::kepler_orbit`), the radius at that orbit's
    pericentre; otherwise (the straight regime, or a Kepler-regime state with no orbit, which is
    every grid system of a galaxy without kinematic tables) plan 02's
    `galaxy.potential().tidal_radius(m, &PointLy::from(record.epoch_position()))`, bit for bit.
    It takes an `EpochState` (Provides names no signature): a grid record's is
    `EpochState::of_grid_record`, a centre member's `of_centre_member`.
  - _The form (lean, 2026-10-29; science check in this record)._ `r_t = (G m ÷ (ω_p² − Φ″(r_p)))^⅓`
    at the Kepler pericentre `r_p` (`KeplerOrbit::pericentre`), with `ω_p = h ÷ r_p²` the orbit's
    own angular speed there and Φ″ the centre's spherical potential's
    (`CentreProfile::psi_curvature`: the black hole, the cluster and the galaxy's monopole, dark
    halo included), which carries King's d ln M ÷ d ln r term. It is King's (1962) radius at
    perigalacticon in its general form: about a point mass it is `r_p (m ÷ ((3 + e) M))^⅓`, and for
    a circular orbit `ω² − Φ″ = 4Ω² − κ²`, the brainstorm's formula, so the rule and plan 02's
    radius agree where the orbit is circular. The denominator is floored at 0.05 ω_p², plan 02's own
    floor, which no orbit about the black hole reaches. μ excludes the dark halo (P09.T28.b.a) and
    Φ″ includes it, some 10⁻³ of the mass inside the fixture's edge.
  - _Figures (Milky Way fixture, a solar mass)._ A circular orbit at 0.01 ly keeps **2.6965 au**
    (the plan's 2.7 au; M(<0.01 ly) = 4.300 × 10⁶ M☉, so King's term is under 10⁻⁴), and one at
    3 ly **0.01240 ly**, where plan 02's radius at that position is also 0.01240 ly (about 0.01 ly,
    the plan's); at 0.99 and 1.01 times the circular speed (e = 0.02) 0.01189 and 0.01237 ly. S2's
    orbit keeps about 0.47 au (King's point-mass form, e = 0.8846).
  - _Leans (2026-10-29)._ (1) An unbound Kepler-regime orbit takes the rule too: a flyby is stripped
    at its one pericentre passage, before or after the epoch. (2) The call reads neither the
    galaxy's rules nor the loss cone: the radius is the planets' whatever the drift hook draws
    (`KEPLER_REGIME_VERSION`), and a system the loss cone carves out is not asked about. (3) The
    plan's 2.7 au "for a semi-major axis of 0.01 ly" is the circular orbit's; an eccentric orbit of
    that a keeps less, `a (1 − e) (m ÷ ((3 + e) M))^⅓`, so P14.T29's test ("nothing beyond 2.7 au")
    holds for a circular one. (4) King's turning term makes the radius at pericentre up to 9%
    smaller, `(3 ÷ (3 + e))^⅓`, than the brainstorm's point-mass radius `r_p (m ÷ 3M)^⅓` taken at
    the same pericentre: for main and the brainstorm's owner, with (3).
  - _Tolerances._ 2.7 ± 0.05 au and the point-mass radius to 10⁻⁴ at 0.01 ly; 0.008–0.015 ly and
    within 10% of plan 02's radius at 3 ly; King's point-mass form to 10⁻¹²; the radius from the
    orbit propagated ±1,000 yr to 10⁻⁹.
  - _Science check (2026-10-29)._ The form, its two reductions and the figures re-derived: ok
    (2.6965 au; S2's 0.4655 au, its pericentre 118.96 au). The citations were not re-checked at
    source (no access to the journals from the pod): King 1962 for the `(3 + e)` form, and Webb et
    al. 2013, §1, for the general form, which may credit Bertin and Varri (2008, ApJ 689, 1005); to
    re-check before P14.T29 wires the rule. The floor's doc had its inequality's sign wrong
    (`−Φ″ ≥ −G M(<r) ÷ r³`), fixed in the review's commit.
  - _Tests (in `motion/tidal.rs`, fast, 1.2 s together):_
    `a_circular_orbit_a_hundredth_of_a_light_year_out_keeps_2_7_au`,
    `a_near_circular_orbit_at_3_ly_keeps_about_a_hundredth_of_a_light_year` (against plan 02's
    radius to 10%), `about_a_point_mass_the_radius_is_king_s` (to 10⁻¹², bound and unbound),
    `the_radius_is_the_same_at_every_time` (from the orbit propagated ±1,000 yr, to 10⁻⁹), and
    `the_rule_follows_the_regime` (plan 02's radius to the bit outside the regime and with no
    orbit, the pericentre rule inside, the same bits twice); and a doctest on each function.
  - _For P14.T29._ Replace `SystemContext::from_stars`' `galaxy.potential().tidal_radius(..)` with
    `motion::tidal::tidal_radius(galaxy, &EpochState::of_grid_record(galaxy, record), mass)` for a
    grid record (`of_grid_record` panics for any other record in a galaxy with kinematic tables),
    and take the smaller of it and the feature's radius for a feature member. It reads
    `Galaxy::centre_motion`, which builds on first read (about 64 ms, once a galaxy). It moves
    output only within the influence radius (7.19 ly at the fixture), which removes the README's
    relaxation "no pericentre stripping within about 10 ly of the centre".
- **P09.T28.d.a as built (lane `p09-secular`, 2026-10-29, on 5bec9f1 at `GENERATOR_VERSION` 23;
  decision-galaxy-25-train §2.2: the rates pure, the term held).** No output moves: nothing calls
  the rates or the step (the term is P09.T28.d.b's), `propagate` is unchanged and `kepler.golden`
  with it, and no golden is added. Files: `galaxy/motion/secular.rs` (new) and one
  `pub mod secular;` line in `galaxy/motion.rs`.
  - _Signatures._ In `motion::secular`: `SecularTable` (`new(&CentreMotion)`), the eccentricity
    table, a value built from the centre's motion and held by no `Galaxy` field (the held half adds
    the carrier with `GalaxyRules`); `SecularRates` (`schwarzschild`, `mass_precession`, `apsidal`,
    `mean_motion_correction`, each a `PerYear`); and `SecularOrbit` (`orbit`, `rates`, and the
    secular step `propagate(Seconds) -> KeplerOrbit`), the orbit with its own rates, so that the
    step cannot be given another orbit's (review). On `KeplerOrbit`, in the child module:
    `schwarzschild_rate(GravitationalParameter) -> Option<PerYear>` (the black hole's μ, since the
    orbit's own holds the stars inside r₀), `mass_precession_rate(&SecularTable) -> Option<PerYear>`
    (the table, since no galaxy holds one until T28.d.b's carrier),
    `secular_rates(&SecularTable) -> Option<SecularRates>` and
    `secular(&SecularTable) -> Option<SecularOrbit>`; Provides named no arguments. Parabolic and
    unbound orbits take `None`, and keep `propagate`.
  - _The rates._ Schwarzschild: `3 G M_bh n ÷ (c² a (1 − e²))`, `6π G M_bh ÷ (c² a (1 − e²))` a
    radial period, about the black hole alone. The stars: Lagrange's equations for the
    orbit-averaged `ΔΦ = Φ + μ ÷ r` about T28.b.a's μ = G M(<r₀), whose partial derivatives reduce
    to orbit averages of the stars' pull `g★ = G M★(<r) ÷ r²`, so that `ω̇★ = −n √(1 − e²) β★ ÷ μ`,
    `β★ = (a² ÷ πe) ∫₀^π g★ (e − cos E) dE`, retrograde.
  - _Deviation: the mean motion (lean, 2026-10-29)._ The plan turns the orbit "by two rates × dt".
    That alone moves a circular orbit along its track by ω̇★ t r, though about μ = G M(<r₀) the
    circular orbit is exact: 36–68 tenths of its tidal radius in 1,000 yr at 0.1–5 ly. So the step
    also advances the mean anomaly at the rate Lagrange's equations give,
    `Ṁ − n = n (2 (G M_bh − μ) + 2 α★ + (1 − e²) β★) ÷ μ` with
    `α★ = (a² ÷ π) ∫₀^π g★ (1 − e cos E)² dE`, which cancels the precession on a circular orbit, and
    takes the mean orbit's mean motion from the energy integral,
    `1 ÷ ā = 1 ÷ a − 2 (ΔΦ(r₀) − ⟨ΔΦ⟩) ÷ μ` (the epoch's osculating a is not the mean one). The step
    is the two-body orbit propagated by dt (1 + δn ÷ n), then turned about its angular momentum by
    the two precession rates × dt. With the GR advance it changes no radial period, which is
    gauge-dependent at order G M ÷ (c² a). The deviation has a cost the direct integration shows
    (below): on the eccentric orbits at 1 and 5 ly the step as built is the worst of the four
    variants (3,181 and 1,953, against 323 and 276 for the precession alone), the mean-motion
    correction that keeps circular orbits exact costing a factor of 6–10 there; at 0.1 ly it is the
    best by far (65 against 1,838).
  - _The table's design point (lean, 2026-10-29)._ On (a, e), of the profile itself: `α★`, `β★` and
    `⟨Γ★⟩ − Γ★(a)` (Γ★ the stars' potential), each over its scale G M★(<a) (or ÷ a), at 16 knots a
    decade from 10⁻⁴ to 10⁴ ly and 32 eccentricities `(j + ½) ÷ 32` (mirrored across e = 0), read by
    cubic Lagrange interpolation; each entry an 80-node quadrature in E over five panels graded
    towards pericentre, of `M★(<r)` and Γ★ tabulated at 32 knots a decade from 10⁻⁷ to 10⁶ ly.
    Values of a and r outside are clamped, and e above 63 ÷ 64 takes the last node's averages.
    Measured: the orbit averages to 3.6 × 10⁻⁴ of a 2,048-node quadrature on the exact mass for a
    under 100 ly, and 2.3 × 10⁻³ beyond (orbits of e above 0.93 from inside the 10 ly break to
    beyond the 100 ly taper); a harmonic core's closed forms to 6 × 10⁻⁵ (4 × 10⁻⁴ for the
    potential's average). At 8 knots a decade the break's orbits were read 4 × 10⁻³ off. The
    alternative, the power law's averages at the slope read at a, agrees to 10⁻⁴ inside 3 ly at
    Milky Way values (the cusp is a clean 1.3 there) and is 0.3–0.7% off at 5 ly, 3–14% at 8 ly and
    up to 21% at 12 to 30 ly, across the break, which seeds with influence radii of 10–177 ly hold.
    The table builds in about 54 ms (release, pod, 4 vCPU) and holds 106 kB: T28.d.b's carrier pays
    it once.
  - _Figures._ S2 (a = 0.0163 ly, e = 0.8846) advances **12.260′** an orbit about 4.297 × 10⁶ M☉
    (GRAVITY 2020's 12.1′ is about their 4.261 × 10⁶ M☉), the plan's a = 0.0158 ly, e = 0.88
    12.192′. A circular orbit's precession and mean-motion shift cancel to 10⁻⁴: at 0.1, 1 and 5 ly
    ω̇★ is −1.53, −2.39 and −2.71 × 10⁻⁵ rad yr⁻¹.
  - _The direct integration (the plan's test; a finding, pinned as it stands)._ The step, with the
    stars' rates alone, and the two-body law, against a fourth-order Runge–Kutta integration in
    `CentreMotion`'s M(<r) (steps of 2 × 10⁻³ local dynamical times, agreeing with steps of 10⁻³ to
    10⁻³ of the bound), after ±1,000 yr from 0.1, 1 and 5 ly, on a circular orbit and four eccentric
    ones (e 0.51–0.85), in tenths of the orbit's T28.c tidal radius for a solar mass (the bound is
    1):

    | From   | Step as built | No mean orbit | Precession alone | Two-body |
    | ------ | ------------- | ------------- | ---------------- | -------- |
    | 0.1 ly | 65.4          | 875           | 1,838            | 1,805    |
    | 1 ly   | 3,181         | 1,990         | 323              | 380      |
    | 5 ly   | 1,953         | 962           | 276              | 1.09     |

    Circular orbits meet the bound under the step and the two-body law (under 0.001). No eccentric
    orbit does under any of them. At 0.1 ly the span holds one to eight radial periods (P 131–838
    yr) and the step beats the two-body law on every orbit, some thirtyfold at worst; at 1 and 5 ly
    it holds a fraction of one (P 4,000–240,000 yr), the averages are not what a short arc feels,
    and the two-body law is the closer on every eccentric orbit, the step putting a star 0.1–2.5 ly
    from where it is. The step is first-order secular theory: its error is the short-period terms,
    of order the variation of M(<r) over the orbit times a, which only an orbit inside some 0.03 ly
    keeps under a tenth of its tidal radius. The two-body law's own miss (380 at 1 ly) is T28.b.a's
    fixed μ = G M(<r₀) on an orbit whose enclosed mass changes by percents. The test
    (`the_secular_step_beats_the_two_body_law_only_where_the_span_holds_whole_periods`, fast, 1.5 s
    with the others) pins each worst to 10% and holds the order above.

  - _For P09.T28.d.b (lean, 2026-10-29; QUESTION for main)._ The term as the plan has it would move
    Kepler-regime records at 1–7 ly by up to a few light-years more than the two-body law, a move
    larger than any T28.b.b's census found. Lean: apply the term only to a bound orbit whose radial
    period is under the clock window's half-width H (1,000 yr; a ≲ 0.25 ly at Milky Way values), a
    property of the system and so continuous in time, and keep the two-body law elsewhere; the
    plan's bound itself needs the short-period terms (first-order variation of parameters along the
    arc) or a propagator in the spherical potential, a task of its own.
  - _Tests (in `motion/secular.rs`, fast):_ `an_s2_like_orbit_advances_12_arcminutes_an_orbit`,
    `the_table_holds_a_harmonic_core_s_closed_forms`,
    `the_table_holds_the_profile_s_orbit_averages`, `a_circular_orbit_keeps_its_two_body_motion`,
    `unbound_orbits_take_neither_rate_and_the_step_repeats_to_the_bit` (the table's entries the same
    bits twice, dt = 0 the orbit itself, the energy and the angular momentum's length kept) and
    `the_secular_step_beats_the_two_body_law_only_where_the_span_holds_whole_periods`; doctests on
    `SecularTable` and `SecularOrbit::propagate` (S2 back at apocentre a period on, turned some
    12′). T28.a's `kepler.golden` is unchanged (`golden_diff.py`: nothing to reconcile).
  - _Tolerances._ 11.8–12.6′ for both S2-like orbits and the closed form to 10⁻⁹; the harmonic
    core's averages to 10⁻⁴ (10⁻³ for the potential's) and its rate to 10⁻⁴; the profile's averages
    to 5 × 10⁻⁴ inside 100 ly and 3 × 10⁻³ beyond; a circular orbit's precession and mean-motion
    shift cancelling to 10⁻⁴ and its step within 10⁻⁶ r of the two-body orbit; the integration's
    worsts pinned to 10%.
  - _Science check (2026-10-29)._ Re-derived and ok: the Schwarzschild rate and S2's 12.260′ and
    12.192′; Lagrange's equations from Murray and Dermott's ϖ and ε forms (`ω̇ = ϖ̇`,
    `Ṁ − n = ε̇ − ϖ̇` in the plane); the two partial derivatives, the point masses adding nothing to
    the e-derivative; the small-e limit `Ω − κ` (a numerical ratio of 1.00004); the harmonic core's
    closed forms; and the mean orbit's 1 ÷ ā, which is Lagrange's first-order short-period δa.
    Against the exact radial period and apsidal angle of a black hole and a harmonic core, ω̇★ errs
    at second order (7.8 × 10⁻⁴ at a stellar mass of 10⁻³ of the black hole's) and the mean motion
    with the mean orbit to 2 × 10⁻⁷, against 5 × 10⁻⁴ to 10⁻³ without it. The short-arc conclusion
    holds: about μ = G M(<r₀) the two-body law feels no perturbing force at the epoch, so its error
    grows as t³ and the step's as t. Fixed in the review's commit: the stars' relativistic term is
    about M★(<a) ÷ M_bh of the black hole's, not "under 10⁻¹⁰". Not re-checked at source (no access
    from the pod): Weinberg 1972 §8.6, Murray and Dermott §6.8, Merritt 2013 §4.4.1 (which the plan
    marks "to re-check"), Binney and Tremaine §3.2.3, and GRAVITY 2020's printed 12.1′ (their
    elements give 12.156′).
- **P09.T28.d.b as built (lane `p09-t28db`, 2026-10-30, on 90e53c0 at `GENERATOR_VERSION` 23).**
  No output moves at 23: nothing below 25 builds the centre's potential, and no golden changes.
  - _Names._ `motion::spherical::{CentrePotential, SphericalOrbit}`;
    `Trajectory::in_potential(&CentreMotion, &CentrePotential, EpochState)`,
    `Trajectory::exact_orbit()` and `Trajectory::heap_bytes()`; `Trajectory` is no longer `Copy`
    (its exact orbit is boxed, some 640 bytes). `Galaxy::centre_potential()` (crate-internal,
    lazy, outside equality and `Debug`).
  - _The hold._ `GalaxyRules::kepler_secular` (`KeplerSecularRules::{TwoBody, Spherical}`),
    `KEPLER_SECULAR_VERSION` = 25; one call site, `Trajectory::new`, which `position_velocity_at`
    and now `CentreCellOrbits::new` read. The bump deletes the field, its `match` and the tripwire
    `the_kepler_secular_term_is_held_only_until_the_25_bump`.
  - _Deviations (lean, 2026-10-30)._
    - The anomaly is logarithmic, `ln r = ū − Δ cos η`, not Hénon's: Hénon's missed the bound by
      16 and 10⁵ tenths on wide orbits (unbound about μ, as most of the grid's are).
    - The Schwarzschild advance is `6π (G M_bh ÷ (c L))²` a radial period, on the exact orbit's L,
      turned in proportion to the Newtonian azimuth, not `schwarzschild_rate × t` (none for an
      orbit unbound about μ, too large by up to μ ÷ G M_bh for a bound one, and uniform in time).
    - The velocity is the Newtonian one turned with the position, as T28.d.a's step, without `ω̇ r`.
    - The near-circular threshold is Δ < 10⁻⁵, measured; the harmonic core's closed forms are
      tested in its two limits, with the first-order advance between them.
  - _Re-pinned by name (decision §1.3)._ T28.b.b's D2 digest, `0xce83_b24f_e65c_7554`. T28.d.a's
    pinned-miss test is replaced by `the_exact_orbit_meets_the_plan_s_bound_against_a_direct_integration`.
  - _For P09.T29 and P14.T29._ Under the exact law the source's shells are the exact orbit's
    turning points; the pads, the loss-cone verdict and T28.c's radius stay on the Kepler orbit,
    which bounds the exact one but for the dark halo's share inward (re-checked by the source
    against a scan and by C1).
- **P09.T40.a, the features' part, as built (lane `p09-t40`, 2026-10-30, on 1e55cff at
  `GENERATOR_VERSION` 23 and `PROTOCOL_VERSION` 3; no output moves).** Main's rulings of
  2026-10-30 narrow it to the features' part (`decision-r06-t16a-scope.md`): `CentreMemberSource`,
  `CatalogueClassSource`, `CentreOrbitCache`, `NestedCellCache` and `CatalogueClassCellCache` wait
  on P09.T37, and with them "P09.T29 as built"'s _For P09.T40.a_ (`check_index_headroom`, the
  centre model's bytes in `GalaxyCache`). No server golden, wire form or fixture moves;
  `golden_diff.py` reports nothing to reconcile, and `just gen-protocol` changes three bindings'
  comments.
  - _Names._ `compute::SharedFeatureCellCache` is a `SharedByteLru` over `(GalaxyKey, FeatureCell)`
    of `FeatureCellContents`, and `compute::SharedClusterModelCache` one over
    `(GalaxyKey, FeatureId)` of `FeatureInterior` (the cluster model with its class table and nested
    grid). They stand for Provides' `FeatureCellCache` and `ClusterModelCache`: the sim's caller-supplied trait
    is already `FeatureCellCache`, and the server's caches are `Shared*`. Each lends the sim's trait
    through a handle for one galaxy (`FeatureCellHandle`, `ClusterModelHandle`) that asserts the
    seed; `SharedClusterModelCache::kept` gives one job a `KeptInteriors` over it, so a range
    query's census, hits and rows build each interior at most once whatever the budget. A feature
    without members is not stored, and a galaxy without its kinematic tables never reads the cache
    (its key is its full-potential twin's). Added: `--feature-cache` (`HYPERION_FEATURE_CACHE_MB`,
    16 MiB) and `--cluster-model-cache` (`HYPERION_CLUSTER_MODEL_CACHE_MB`, 64 MiB);
    `ServerStats::{feature_cells, cluster_models}`; in the sim, `FeatureCellContents::heap_bytes`,
    `MemberClassTable::heap_bytes` and `FeatureInterior::heap_bytes`, which the caches charge (a
    record is 168 bytes, a cell near the Sun about 38 kB; an interior 27 kB on average over the 477
    within 3,000 ly of the Sun-like point; 2026-10-30). The system and brief caches resolve a
    feature member through the interior cache (`with_interiors`), plan 07's deferred finding (1).
  - _The switch (lean, 2026-10-30)._ A version hold, `config::FEATURE_MEMBERS_FROM` = 26, with
    `FeatureMembers::{Held, Served}` (default `of_version(GENERATOR_VERSION)`), in the pattern of
    the surface's serve gate `GENERATED_SURFACES_FROM`, rather than a flag: it opens with the bump
    that carries P09.T2.c without a lane having to remember it, and its tripwire,
    `the_feature_members_gate_holds_until_its_version`, fails at 26, so that bump opens it with
    P09.T2.c and R06.T16.a or raises it to P09.T2.c's train. No option or variable serves members,
    so no running server counts them twice; tests serve them through
    `ServerConfigBuilder::feature_members`. Held, the handler registers no source, no row carries
    `origin` or `member_class`, and neither the sky nor `extinction` takes any modifier: the replies
    are as before, bit for bit (`behind_the_gate_a_query_inside_a_globular_answers_as_the_grid`
    reads the frame for the keys).
  - _Served._ The `SystemsInRange` handler registers `FeatureMemberSource` over both caches, in
    both modes. Every row carries `origin`, grid rows included (`grid`,
    `feature_member { feature }`); a centre member or a later origin panics, since no registered source places one. A
    member's row carries `member_class` (`cluster { … }`; `listed` is never placed, as
    `FeatureLevelList::member` is `None`) and its own velocity, its cluster's and its own: both its
    member record's, resolved again through the job's kept interiors (`convert::RowOrigins::Sent`),
    since a hit carries its record alone. `supernova` stays absent.
  - _Feature gas._ The server passed `NoModifiers` in the sky's job context, and an empty list in
    the `extinction` request's lines, which plan 07's P07.T10.c names as a `NoModifiers` site.
    Served, `SkyTablesService::with_features` lends every table the shared feature cells and each
    job marches `FeatureGas` at its observer's time (`SkyTables::gas`, `SkyGas`; `march_context`
    takes the modifiers); `extinction`'s lines take `FeatureGas` at the request's time
    (`compute::LineGas`), keyed by that time, a smooth line keeping its key. The extinction maps
    take none: `render_extinction_rows` integrates the smooth field. The caps' rays read no
    modifiers (before this task too), so served, the census's sightlines see the features' gas and
    the caps' do not: R06.T16.a's to reconcile. Each served segment looks its feature cells up
    through the shared cache's one lock from every worker; unmeasured.
  - _Catalogue classes._ `convert::features::catalogue_class(ClassId) -> Option<CatalogueClassWire>`
    matches the registry's constants and is `None` past plan 09's nine, since `ClassId` is a struct;
    `every_registered_class_has_a_distinct_wire_string` walks `ALL_CLASSES` in value order. Its
    first caller is P09.T33.a's origin. "P09.T39 as built"'s "by `ClassId::value()`" gives way to
    the match. The docs of `PROTOCOL_VERSION`, `features.rs` and the three enums name them growing;
    `TargetExtinction::NoSuchSystem` and `ErrorCode::UnknownSystem` say a system that resolves but
    cannot be placed is answered so too (plan 07's deferred finding (2)).
  - _Costs (served; test profile, the sim at opt-level 2; pod; 2026-10-30)._ A cold 50 ly query at
    the Sun-like point builds 115 interiors, one for every feature whose reach (a nursery's up to
    1,750 ly) touches the padded sphere, in 10.1 s, about 88 ms each, against 22 ms for the grid
    alone; warm, 28 ms. A 1 ly query in the core of the globular nearest the Sun (43 members, 68
    interiors) costs some 12 s cold, nearly all P09.T23's member draws. A finding against the
    brainstorm's 5 ms cold target, not a blocker (2026-10-13 rule). Levers for the lane that opens
    the gate: test a feature's nested grid against the sphere before building its interior, or warm
    the interiors at open.
  - _Not tested._ A served answer of more than `BRIEF_CHUNK_ROWS` rows (the chunked conversion
    takes the same `RowOrigins`), too dear in a globular's core in a test build; and observed
    members' lines, which the sim's `Drift::of_record` builds through `NoInteriorCache`
    (`of_record_in` is crate-private), so each observed member row builds its interior again.
  - _Tests._ `compute::features`: `a_feature_cell_is_the_catalogue_s_and_the_second_ask_is_a_hit`,
    `an_interior_is_the_sim_s_charged_and_lent_again`, `eviction_never_changes_a_range_query_s_members`
    (hits and census, cold and warm, keeping nothing, evicting and keeping all, and kept for a job);
    `convert::features`: the class strings, the origins, a member class; `compute::sky`'s and
    `compute::sky_tables`' feature gas at a nursery's two times, through the tables and through the
    service, and `the_sky_takes_the_features_gas_only_once_the_gate_serves_them`;
    `compute::sightlines::a_line_through_the_features_gas_is_the_sim_s_at_its_own_time`; the gate's
    tripwire and `the_feature_members_gate_opens_at_its_version`; over the WebSocket,
    `tests/feature_members.rs`: served, a query inside a globular returns its members with their
    origin, class and velocity, the sim's own rows at 0 and 1,000 yr, the same warm, and with
    briefs and observed; near the Sun every row says `grid`; held, no member and no key.
- **P09.T40.b, records (lane `p09-t40`, 2026-10-30, at 23 and 3; no existing output moves).**
  - Handlers: `requests/features.rs`; checks and answers in `convert/feature_answers.rs`. The four
    kinds are served at 23, whatever P09.T40.a's members gate holds.
  - New wire: `ErrorCode::UnknownFeature` (growing, no bump), naming `feature`, for a feature word
    naming nothing at the time; a word that is no feature's is `bad_request` naming `feature`.
    `FeatureStructure::Association(AssociationStructure)`, a nursery with no model and no gas left;
    one with gas is `cloud`, its gas Plummer ball.
  - New limits: `MAX_FEATURE_CELLS` = 512 gives `over_cell_budget` to the catalogue kinds (the
    centre still answered); `GALAXY_FEATURES_CACHE_BYTES` = 1 MiB.
  - New server items: `compute::{GalaxyFeaturesService, MapGlobulars, MapGlobular}` (the map's
    globulars per galaxy, a bulk job), `expected_members_of`, `counted_members`,
    `ServerStats::galaxy_features`.
  - The centre: ID `f000000000000000`, designation `CENTRE`, extent 128 ly, null age.
    `black_hole_state` is refused outside the clock window and is `internal` if the centre's
    events fail to build.
  - Left undone: a detail of a feature that is none at the time is untested; no job checks its
    cancel token between features. Bright shells stay `not_generated` until P09.T37.
  - Golden: `tests/golden/features.golden` pins the four answers' frames.
- **P09.T40.a, the served query's speed (lane `perf-range-query`, 2026-10-30, at 23 and 3; no
  output moves).**
  - `FeatureInteriorCache::grid_reach` (provided; never below the lent interior's grid reach,
    `None` only where it lends none): `FeatureMemberSource` builds an interior only where that
    bound meets the sphere, and draws only the candidates within the radius padded at
    `member_speed_bound` (`FeatureInterior::members_in_cell_near`). New:
    `FeatureInterior::grid_reach_bound`, `ClusterParameters::of_record`.
  - `SharedClusterModelCache` holds a feature by its bound until its interior is built; its
    counters count both. `FeatureInterior::of` asserts the speed bound in release too.
    `ClassProfile` tabulates its distribution when first read.
  - Bench: `hyperion-server/benches/feature_range_query.rs`. Left: the grid and the feature cells
    keep the Sun-like query over 5 ms cold, and remnant draws a globular core's (the lane's
    findings).
- **P09.T40.a, the served query's speed, second pass (lane `perf-range-query-2`, 2026-10-30; no
  output moves).**
  - New: `FeatureCatalogue::cell_through(galaxy, cell, last)`, `FeatureCellContents::holds`, and
    `FeatureCellCache::contents_through` (provided; lends the whole cell by default).
    `FeatureCatalogue::near` reads a cell only through the last process whose farthest reach
    (crate-private `farthest_reach`) can touch the sphere. The server's cell cache holds such parts.
    `counts()` is 0 for a process a part does not hold.
  - A cell's disc proposals read shared column bounds (`Fields::component_bounds_of`).
    `MemberClassTable::grid_width` reads only each doubling's new cells, and skips a level whose
    ceiling is under the target; `peak_candidates` is unchanged in value. A change to the width
    rule or the profiles keeps `level_ceiling` a ceiling
    (`the_width_search_is_the_whole_grids_search_bit_for_bit` and its slow twin).
- **P09.T40.a, the served query's cells in parallel (lane `perf-range-cells`, 2026-10-31; no
  output moves).**
  - Sim: `range_query` is `RangePlan::new(galaxy, sources, query).run(galaxy, cache, sources)`;
    `RangePlan::{cells, with_sources, census, query}`. `FeatureCatalogue::cells_near` lists the
    cells `near` reads, each with its `last`. `FeatureMemberSource::{feature_cells, read_ahead}`.
  - Server: `systems_in_range` runs through `compute::served_range_query` (`compute/range.rs`, not
    `compute/systems.rs`), which takes a `RangeCaches` and a `ServedQuery`; `AppState.cells` is an
    `Arc`. Its cell jobs are queued by the request's task, never by a job (the pool's FIFO would
    stall).
  - Second pass: `RangePlan::{walk_cells, source_hits, finish}` with `WalkedCells` and
    `SourceHits` (`query/parts.rs`); `run` is `finish` of no part. `finish` takes a part's cell
    only if the part walked its layer with the plan's own sphere. The grid's jobs walk as they
    generate (`SharedCellCache::walk_generated`, in place of `generate_all`); a sources job
    (census and source hits) follows the feature jobs. A cold finish asks the cell cache for no
    cell, so its counters read no lookup.
- **P09.T41.a, records (lane `p09-t41`, 2026-10-30, at 23 and 3; client only, no output moves).**
  - The guide keeps its one Greek cross for every kind, the kind named by the label (`OC GPF F207`),
    in place of the task text's "a symbol per kind"; it gains the cross's proportions, the label's
    and a superbubble's form, and the legend's `FEATURE` and `TRUE RADIUS`.
  - Spatial view: `SymbolShape` gains `greek-cross`; `FeatureMark` (a `PointMark` with `radius`) and
    `ShellMark` (centre, radius, label; its own type, since a `SphereMark` is about the view
    centre) ride `SpatialScene.features` and `.shells`. A feature is placed, picked and labelled
    with the points; circles at true radius are drawn first.
  - Chart: `useFeaturesInRange` asks every kind beside the range query; `LocalChartState` gains
    `features` (only an answer for the chart's own centre, radius and time), `featureState`,
    `featureFault`, `selectedFeature`, `selectFeature`, `selectMark` and `retryFeatures`. A mark's ID
    is `feature:<id>` (`featureMarkId`, `selectionOfMark`), since a feature's word is also a member's.
  - Model and adapter: `ChartFeature` and `ChartFeatures` in `lib/galaxy/model.ts`, built by
    `lib/galaxy/featureWire.ts`; words in `lib/galaxy/featureWords.ts`; fixtures cut from the
    server's `features.golden` in `test/featureFixtures.ts`.
  - Not drawn: a remnant's shell on the chart, since a range row carries no explosion site; the
    map's bright shells carry one (P09.T41.b).
- **P09.T41.b, records (lane `p09-t41`, 2026-10-30, at 23 and 3; client only).**
  - `useGalaxyFeatures` (the page's one `galaxy_features` request, at the epoch) and
    `toMapFeatureLayers` give `MapFeatureLayer`s (`MapFeature`, `MapShell` in `lib/galaxy/model.ts`);
    `placeMapFeatures` (`lib/galaxy/mapFeatures.ts`) places them per view, pure; `MapFeatureOverlay`
    draws them over each picture; `MapFeatureToggles` (`GC`, `CENTRE`, `SNR`) stands in the face-on
    view's words, through `GalaxyMapView`'s new `remPx`, `featureLayers`, `featureKinds` and
    `featureControls` props; the toggles, each beside its sample, are the overlay's legend.
  - "Pending" is the request's own state, said on the page's line for the views' wait after the
    link's state and a stale view's `PENDING`, with a failure's `RETRY`; a shown kind the server
    does not generate reads `SNR: NOT YET MODELLED` there.
  - `mapLayout`'s face-on words gain `FEATURES_ROW_REM` = 2.25 rem, estimated, not measured: plan 07's
    hidden runs should re-measure it (the stacked overlay's room at 1920 × 1080 now has 20 px).
  - Not drawn: a globular beyond the edge-on picture's ±32,768 ly; nothing is pegged.
- **P09.T41.c, records (lane `p09-t41`, 2026-10-30, at 23 and 3; client only).**
  - `ChartSystem` gains `origin: SystemOrigin` (`not_sent`, `unknown` and the four wire arms),
    `memberClass: MemberClass | null` and `supernova: SupernovaState | null`, built by
    `toSystemOrigin`, `toMemberClass` and `toSupernovaState` (`lib/galaxy/featureWire.ts`) inside
    `toChartSystem`; unknown values are found by `Object.hasOwn` against the known tags and classes.
  - The panel shows one of two lists, `SYSTEMS` or `FEATURES` (`FeatureList`), one tab stop; a feature
    selected shows `FeatureReadout`, with `useFeatureDetail` (`STRUCTURE`) and, for the centre,
    `useBlackHoleState` (`BLACK HOLE`: mass, total 2–10 keV luminosity, disruptions). That is
    P09.T42's `BLACK HOLE` readout's half, mass and luminosity at the chart's time; T42 keeps the
    fine radius steps and the distance units.
  - `OriginReadings` adds `ORIGIN`, `MEMBER CLASS` and the supernova rows to the system readout,
    `SystemReadout` taking the chart's `features`. They are in the `GALAXY` readout only: the
    `SYSTEM` display's host readout, which T44 also names, gets none, since `system_summary` carries
    no supernova state.
  - The selection is the operator's (`LocalChartState.selection`): while a new chart's features are
    awaited the selected feature is read, stale, from the answer before (`selectedFeatureStale`).
  - Leans (2026-10-30): a grid system's `ORIGIN` is its population alone until P08.T15.a sends a
    placement; an absent origin reads `NOT YET MODELLED`; a member of a feature the chart lacks reads
    `FEATURE` and its ID; the server's zero `expected_members` (no cluster model) reads Missing; the
    slow hosts' accreting white dwarfs read `SLOW ACCRETING WD`; supernova ages are in `kyr`.
  - Merged over the client's growing enums (2026-10-30): a member class's `band` is a `ChartLayer`
    through `wire.ts`'s `toChartLayer`, and a grid origin's unknown population reads Missing.
    A class whose band's layer is unknown reads that letter Missing: `LIVING · — · FIRST · SINGLE`.
- **P09.T33.a as built (lane `p09-classes`, 2026-10-29, at `GENERATOR_VERSION` 23, no output
  moves).** The first production class, registered: `ProductionClasses` has the
  `CORE_COLLAPSE` arm and plan 03's `placement::resolve` hands every `111` ID to the class grid
  (P09.T32's point (a)), so a core-collapse entry's ID resolves to its record. A placed class whose process is not
  built yet still answers `KindNotGenerated` through plan 03 (`system_of`; the grid's own
  `resolve` answers `NoSuchSystem`, as P09.T32 built it), since its IDs will name systems once
  its task registers it with no bump; a free class value, the tidal-disruption victims, a
  misaligned cell and every candidate the class does not keep answer `NoSuchSystem`
  (determinism review). Nothing the grid places
  moves (`RECENT_DEATH_CLAIM` stays `None`, P09.T35) and no existing golden pinned a `111` answer,
  so there is no bump and no `GalaxyRules` hold (decision-bump-plan-after-22, §3). Files:
  `catalogue_classes/core_collapse.rs` (the process, its candidate and entry) with
  `core_collapse/tests.rs`; `catalogue_classes/system.rs` (`CatalogueSystem`, `CatalogueMotion`,
  `system_of`, `system_of_record`); `grid.rs` (the trait's `system`, the arm, `candidate_count`
  `pub(crate)` for the tests' census); `ids.rs` (`CatalogueCellKey::position_from_words`, T32's
  deferred "no shared position draw"); `ages.rs` (`AgeDistribution::max_pdf_between`);
  `placement/record.rs` (`SystemOrigin::CatalogueSystem(ClassId)`) and every match on the origin;
  `supernova.rs` (v_fast, below); `rng/tags.rs` (five `System` tags under `class.core_collapse.`:
  `site`, `mass`, `metallicity`, `death`, `velocity`, appended after `class.cell` at the end of
  `domain_tags!`, and the registry's lint attribute, below); the
  server's system cache; `tests/source_horizon.rs` (the class's row, P12.T0's rule).
  - _Signatures_ (what P09.T43, T36.a, T37 and P11.T8 consume). In `catalogue_classes::core_collapse`:
    `CoreCollapseProcess::new(&Galaxy)`, `components() -> &[YoungComponent]` (`id`, `share`,
    `deaths_per_system`), `interval() -> &SupernovaInterval`; `CoreCollapseEntry` (`record`,
    `component`, `composition`, `site`, `motion`, `marks`, `supernova`, `stars(&Galaxy)`);
    `RECENT_DEATH_AGES: Years` (`YOUNG_AGE_LIMIT`, 100 Myr). Re-exported from `catalogue_classes`:
    `CatalogueMotion` (`broken`, `straight`, `point`, `at`, `velocity_at`, `position_at`),
    `CatalogueSystem` (`new`, `with_stars`, `record`, `motion`, `position_at`, `star_model`,
    `stars`), `system_of(&Galaxy, CatalogueSystemId)`, which returns
    `Result<CatalogueSystem, ResolveSystemError>`, and
    `system_of_record(&Galaxy, &SystemRecord) -> CatalogueSystem`. Also
    `ClassProcess::system(&self, &Galaxy, &CatalogueMember<Self::Entry>) -> CatalogueSystem`;
    `CatalogueCellKey::position_from_words([u64; 3]) -> GalacticPosition`;
    `AgeDistribution::max_pdf_between(Years, Years) -> f64`;
    `TraceMotionError::CatalogueMotionNotTraced(SystemId)`.
  - _The process_ (module documentation). Sites follow the young field's layer-E density: every
    component with layer-E systems younger than `RECENT_DEATH_AGES` (100 Myr), at 23 the young thin
    disc and the nuclear disc's two components; the old thin disc's youngest sub-disc starts at
    100 Myr. A candidate draws its site in its cell, thinned on the envelope there with one mark
    that also picks its component (plan 03's pick); its band-E mass; its [Fe/H] from its
    component's law at the site, read at age zero (both young laws are flat in age); its primary's
    draws at attempt 0 and their exact `lifetime`; T uniform over the field's thinning interval
    (word 0 of `class.core_collapse.death`); its age at the epoch, lifetime − T; then it is thinned
    on its component's age density at that age against the component's largest (word 1); then its
    velocity, plan 08's `draw_on` of its component's law at the site (rule 6; zero without
    kinematic tables, as `epoch_velocity`). The test is `death_in_interval` on its primary's marks,
    `DeathMarks::of` its single-star `SystemStars` (companions are no members) at the site with that
    velocity, and `claimed_window` in `Surroundings::Field`. The entry has one member, moving at the
    velocity until T and at it plus the natal kick from T (`CatalogueMotion::broken`); its record's
    epoch position is that line's at the epoch, its stars `generate_with` at its composition,
    `MultiplicityContext::Free`.
  - _Deviations from the task text, each a lean (2026-10-29)._ (1) **No redraw until a core
    collapse.** "Builds its `SystemStars` … with `StarDraws::for_attempt` until the death kind is a
    core collapse (bounded at 64 attempts)" would keep a candidate whose first draws die another
    way and so over-produce entries wherever another death is likely. Under MM20 (their Table 1)
    complete fallback is certain from an 8 M☉ carbon–oxygen core, some 27 M☉ initial at constant
    mass (Hurley, Pols and Tout 2000, eqs. 66 and 75); near solar metallicity plan 06's tracks hold
    the core at 6.0–8.1 M☉ over 23–56 M☉ (P06.T18's Wolf–Rayet plateau), where it is likely but
    not certain. So redrawing would turn a chance of a shell of perhaps a sixth to a third into
    near-certainty, and where fallback is certain all 64 attempts would fail; the mass's acceptance
    would have to carry P(core collapse | m), which no table holds. The class thins instead: a death without a shell has no marks and is dropped, exactly as the grid
    side's claim reads the same record (in the sample cells, 73 of 356 kept candidates had none).
    The origin carries no attempt. (2) **A death rate per component.** The envelope's "largest
    death rate per system" is each component's own: the nuclear disc's (a twentieth of its stars
    over its last gigayear) is 1/201 of the young disc's, and against the young disc's the four
    cells at the centre drew 30 million candidates (245 s) for 97 entries; with each component's
    own, 150,549 (4.5 s). The envelope is Σ share × the component's `bound` over the cell × its
    deaths per system over the interval, and the site's mark picks against those terms. (3) **The
    rate is read at the drawn age**, lifetime − T, after T is drawn, not "at that mass" before it,
    so the thinning is exact where the age density or a later (1 − φ(a)) varies with age. (4)
    **The young field includes the nuclear disc's young tail**: the grid places it and P09.T35's
    claim takes every layer-E record, so the catalogue must hold its deaths too (66 of 102 entries
    of four central cells in the probe were its). The field holds no nuclear-cluster system: plan
    02's two nuclear-disc components are the nuclear stellar disc's main and inner parts (Sormani
    et al. 2022; ruling 143.1), and the nuclear cluster is a budget of its own (Design note 5), so
    T36.b's centre entries add no double count and T36.b subtracts nothing (decision-galaxy-25-train
    §3's consumes check). (5) **v_fast counts the progenitor** (below).
  - _`ClassProcess::system` (rule 9, added)._ "Deciding what a catalogue system is" needs every
    class's member as a record, a motion and its stars, and `resolve`'s `MemberVisitor` is generic
    over any class, so the trait gains a required method,
    `system(&Galaxy, &CatalogueMember<Entry>) -> CatalogueSystem` (P09.T32's toys implement it).
    `CatalogueSystem` holds the record, a
    `CatalogueMotion` (two lines meeting at a break; `straight` for one) and the composition and
    multiplicity context of its stars, `None` for a class whose members the stellar stage does not
    model (`star_model()`). `system_of(&Galaxy, CatalogueSystemId)` resolves one through
    `ProductionClasses`; `CatalogueMotion::position_at` takes the time from its break as one exact
    clock span, rounded once (determinism review). A catalogue system's existence and epoch
    position read the galaxy's kinematic tables (a progenitor rests without them, as every grid
    record does), which the server always builds.
  - _What a catalogue system is, match by match._ `SystemOrigin::CatalogueSystem(ClassId)`
    (8 bytes still): `from_parts` asserts an ID of its class; `layer()` the band of its primary's
    initial mass (P09.T37's "counts in the layer its initial mass gives it"); `component()` `None`;
    the primary read at attempt 0 (`record_attempt`, `MemberRecord::attempt`); the loss cone
    `Clear` (it follows its class's line); `query::position_at` its class's motion;
    `query::epoch_velocity` its line's velocity at the epoch (the server's scene and place read
    it); `observe::stars_of`, `BriefModel::of_record`, the planetary context and the server's
    system cache its class's stars (`KindNotGenerated` for a class without), the last two through
    one exhaustive match on the ID's kind; lensing's mass its class's stars; `from_parts` also
    asserts its primary is of 0.08–150 M☉, so that `layer()` cannot fail; `Drift::of_record` refuses it with a new `TraceMotionError::CatalogueMotionNotTraced`,
    since its line breaks at the explosion (the server maps it to `unknown_system`, as the centre's
    members); the sky census skips it (no class is a census source before P09.T37).
  - _v_fast counts the progenitor._ `DeathMarks::of` took v_fast as the remnant's speed after T,
    |v + kick|. A kick against the motion leaves the remnant slower than its progenitor, which
    then outruns `LIFETIME_REACH` before T: over the field's thinning interval (3.43 Myr at the
    fixture, its cap plus L + 2H) a progenitor above about 360 km/s does, as the nuclear disc's
    can. v_fast is now max(|v|, |v + kick|), the
    brainstorm's "fastest member's speed" over the entry's lifetime; nothing calls it in production
    before P09.T35, so no output moves. T17's marks test asserts the new rule and a regression case
    (v = −0.9 × kick). This supersedes "P09.T17 as built"'s design point (a), |v + kick|;
    `of_type_ia`'s bound, |v| + the survivor's speed, is unchanged.
  - _The registry's const assertion._ With the five tags the sim's registry passes rustc's
    deny-by-default `long_running_const_eval` (the macro's name check is quadratic), as P10.T7.a's
    one tag did on integration. `rng/tags.rs` carries P10.T7.a's module-level
    `#![expect(long_running_const_eval, …)]`, with its reason string, as main asked (2026-10-29),
    so the merge is trivial; the real fix in `hyperion-base`'s `assert_tag_names`, comparing stored
    hashes, is queued separately. The lane's first commit changed `assert_tag_names` itself, and
    its fix commit takes that back.
  - _Figures (pod, 4 vCPU, opt-level 2)._ Fixture (seed `0x0933_0000_6363_6f6c`): layer-E share
    0.00946; deaths per system over the 3.43 Myr interval 0.0345 (young disc) and 1.71 × 10⁻⁴
    (nuclear disc); 54 entries in the 21 sample cells (73 deaths without a shell, 229 before their
    window's interval); the farthest any of them travels 1,693 ly; the fullest cell expects at
    most 38,302 candidates (the octants' bound), far inside 2²⁴. The census (`core_collapses_over_seeds`,
    slow, 358 s on four threads): the fixture 1.56 core collapses a century, 36 in ±H, 15,558
    entries, 11,383 shells, 4,175 beyond them, longest window 2.62 Myr, farthest 4,088 ly; three
    drawn galaxies 1.66, 2.42 and 1.44 a century, 33, 44 and 28 in ±H, 4,444, 6,455 and 3,726
    beyond the shells, farthest 4,093–4,095 ly. "About 40 at two a century" holds as 36 at 1.56
    (31 expected); the Milky Way's measured rate is 1.63 ± 0.46 a century (Rozwadowska, Vissani and
    Cappellaro 2021, New Astronomy 83, 101498).
  - _Cost (finding)._ Some 10 ms a cell at the solar circle and 1.1 s a nuclear-disc cell at the
    centre, nearly all of it three track builds per thinned candidate (the rate's `lifetime`, the
    predicate's exact tier, `SystemStars`); a cold 50 ly query's 2,902 cells would take some 30 s,
    against P09.T37's 5 ms. T37/T40's cache is needed. A prefilter that `claims` implies (T before
    the interval's start at the window of the site's gas at `ExplosionEnergy::MAX`, with a margin
    for the rounding of T) would skip the tracks for the 65–85% of candidates the window rejects,
    changing nothing generated; and P08.T13's seam `decide(.., exact_death_age)` could take the
    lifetime already computed.
  - _Tests_ (`core_collapse/tests.rs`, 13, the census natively over four galaxies and on
    wasm32-wasip1, which has no threads, over the fixture alone; the server's system cache, 1; the plan-09 tag test; `record.rs`, 3; `ages.rs`, 1;
    `grid/tests.rs`'s production test rewritten; `clock_tests.rs`'s marks test; doctests on
    `CoreCollapseProcess`, `CatalogueMotion`, `system_of`, `position_from_words`,
    `max_pdf_between`): the young field's components and rates, every age density under its
    component's bound; the envelope over 24,000 random points and the centre's corners; headroom by
    the octants; the kept candidates against the field's own intensity by quadrature (young disc
    138 against 139.1, nuclear disc 875 against 869.6; α = 10⁻³); every entry the shared test's
    (predicate, marks with the one velocity, `claims`, the window, the caps, the kick at T);
    no entry past the reach; resolution through the process, `system_of` and plan 03, a second
    member and an index past the count naming nothing; the rest of the sim reading an entry through
    its class (`stars_of`, the range brief on every route, the planetary context, the lensing mass,
    plan 03's position and epoch velocity, the loss cone, the drift's refusal); deaths without a
    shell and too early
    thinned and unresolvable; order independence with stray resolves, the same seed twice, another
    seed's counts differ; a galaxy without kinematics; the golden
    `catalogue_classes/core_collapse.golden` (new, at 23: rates, interval, 21 cells' bounds, counts
    and every entry); the slow census over four galaxies (above), with every window under its caps
    and every system inside the reach at its interval's ends.
  - _Goldens._ `rng/tags.golden` extended only (+5); `catalogue_classes/core_collapse.golden` new;
    no other golden; `GENERATOR_VERSION` 23 → 23.
- **Deferred findings (2026-10-13 rule)**, from P09.T33.a: (1) _Sites at the explosion, not at the
  epoch._ The class's sites follow the young field's density, as the task says, so its systems'
  epoch positions are those moved on by v (t − T), up to some 3,400 ly along the orbit over the
  interval, while the grid's carved systems' epoch positions follow the density and their sites
  are moved back. The two agree off the arms; on the young disc's sharp arms they differ by a
  pattern shift, which P09.T38's budgets in twelve volumes will measure. The class also reads its
  component's [Fe/H] law and plan 08's velocity law at the site, where the grid reads them at the
  epoch position (`draw_metallicity`, `epoch_velocity`), so the two differ by the same shift. The
  alternative, to draw the epoch position from the density and back the site out along x + v T as
  `explosion_site` does, is listed for main (lean: the task's sites, 2026-10-29). (2) _No Kepler
  regime._
  A catalogue system follows straight lines everywhere; a nuclear-disc entry inside the black
  hole's influence radius would follow its orbit on the grid side (T36.b, T38's centre volumes).
  (3) _Companions are not carved._ `SystemStars::generate_with` redraws carved binaries for grid
  records only (`binary::carve::grid_redraws`), so a catalogue system's companions may fall in a
  plan 11 class (P11.T8). (4) _At version 26._ The class reads each component's age distribution
  and `ShareMatrix::component_share`, so P09.T2.c's (1 − φ) and P08.T12.a's per-component
  `stay_share` factor reach it unchanged; P09.T33.b adds the displaced classes' deaths beside it.
  (5) _Plan 03's `position_at` and feature members._ It still panics for a feature member away
  from the epoch in a galaxy with kinematic tables (P12.T0's finding), which the server's
  extinction request can reach with a member's ID; a catalogue system is now routed through its
  class. (6) _The census's brackets are galaxy-wide._ 20–160 core collapses in ±H, about 40 at two
  a century and 3,000–21,000 entries beyond the shells are the brainstorm's figures for a whole
  galaxy, which the field class holds alone at 23 only because φ = 0. From P09.T2.c (version 26)
  the young field keeps 1 − φ(a), so the class holds a tenth to a fifth of core collapses (ruling
  118.2) until P09.T36.a lists the rest, and `core_collapses_over_seeds` fails as written: P09.T2.c
  restates it on the field's share, or the galaxy-wide counts move to T36.a's or T38's census.
  (7) _A class that redraws._ `SystemOrigin::CatalogueSystem(ClassId)` carries no attempt, and
  `record_attempt` reads every catalogue system's primary at attempt 0. P09.T43's catalogue side
  redraws its star "with `StarDraws::for_attempt` until the window overlaps", exact there because
  its sampler is conditional on the overlap, so T43 adds the attempt to the variant
  (`CatalogueSystem { class, attempt: u16 }`, still eight bytes; every core collapse keeps 0, so
  nothing moves). (8) _The fixture's rate._ 1.56 core collapses a century against the brainstorm's
  "about two" (Li et al. 2011); the measured 1.63 ± 0.46 (Rozwadowska et al. 2021) sits between,
  so the census checks the ±H count against the galaxy's own rate, and the figure is listed for
  main. (9) _Fast-suite cost._ The sample cells take some 10 s to generate in each test process
  that reads them (the centre's cell 1.1 s; opt-level 2, pod), and the golden reads them on
  wasm32-wasip1 too; the module's fast tests take 13–30 s of test time natively.
- **P09.T33.b not built (lane `p09-classes`, 2026-10-29, at `GENERATOR_VERSION` 23): it waits on
  P08.T12.** Its second site density is "plan 08's runaway and walkaway classes of band E", and
  those classes have no density until P08.T12.a builds the displaced fields: each class's weight
  × its `FormTable` form, with the form's in-cube, unbound and own-form shares (the nuclear disc's
  own-form share stays in the field) and a bound over a cell. Plan 09's Consumes already puts
  T33–T36 on P08.T12. Three things stand in the way, and building around them would be a stand-in
  for P08.T12.a:
  - _No carrier._ `ClassTable::build` takes 0.53 s for the fixture (pod, opt-level 2), and the
    class's process is built from the galaxy alone at each visit of the production lookup
    (P09.T32's registration), so it cannot build the table; nothing carries it per galaxy before
    P08.T12.
  - _No densities or bounds._ `DiscBornForm` and `CoredPowerLaw` have densities but no bound over
    a cell yet; `FlareFactor` and the arms' `sup` are the pieces P08.T12.a's `layer_bound` will
    assemble.
  - _No physical forms._ `FormTable::committed()` is provisional, "fitted to the orbit run's smoke
    histograms and … not physical", until P15.T6's production table.
  - _The closed form, measured_ (a probe, not committed): at the fixture, W = 0.340 from the class
    table; over band E's 33 nodes at the thin disc's reference lifetimes, solar composition and
    the median draws (whose deaths make a shell up to 18.2 M☉ and 65.9% of band E's deaths by
    rate), runaways are 15.4% and walkaways 34.0% of the young field's core collapses: 49.4% of
    them die after being ejected. These are lifetime shares. A runaway's encounter, at 0–3 Myr,
    comes before every band-E death after 3 Myr; a walkaway's release, at its heavier
    companion's death, always comes first. At 23 the grid places every young star in the field,
    so T33.a's field-only class agrees with the grid.
  - _Design for when P08.T12 lands (lean, 2026-10-29)._ (1) T33.a's field term keeps a death only
    if its ejection marks say it was not ejected before it died. The marks go on a new tag,
    `class.core_collapse.ejection`: the kind by `runaway_share` and `walkaway_share`, the channel,
    and the ejection age as the class table's `ejection_ages` gives it. They are judged at the
    reference lifetime, so the closed form is exact for the share. (2) A second set of terms in
    the same class, one per band-E runaway and walkaway class of the thin disc and the nuclear
    disc (speed bin by the bin of the time from ejection to death). Each term is its class's
    form × an envelope of its recent deaths, and the site's mark picks a term as T33.a's picks a
    component, so both densities share the class's cells and index space. (3) A term's candidate:
    - draws its mass by thinning on its class's conditional weight against its largest;
    - draws its ejection age inside the age bin and its speed inside the speed bin;
    - takes as its velocity the source component's law at the site plus the ejection velocity
      (the one velocity);
    - then goes through T33.a's death-rate thinning and the shared test.

    (4) Tests: the class's ejected share against the closed form above (α = 10⁻³); and
    complementarity for sampled runaway candidates, where a grid runaway record and a catalogue
    candidate with the same marks and velocity give the same `death_in_interval` and `claims`.
    The production lookup needs the galaxy to carry P08.T12's displaced fields, or a
    `GalaxyRules` hold at 26.

  - _For P09.T43._ Its "sites follow … the band-E runaways" waits on the same carrier.
- **Deferred findings (2026-10-13 rule)**, from P09.T33.a's review: _Every read of a catalogue
  record resolves its entry again._ `query::position_at`, `epoch_velocity`, `stars_of`,
  `BriefModel::of_record` and lensing's mass each call `system_of_record`. That rebuilds the
  process and reruns the candidate and its test, some milliseconds each time. Before P09.T37 puts
  catalogue systems into the range query, callers that hold the `CatalogueSystem` should pass it,
  or the process should be cached per galaxy (Rust review).
- **P09.T33.a merged over P09.T28.b.b, T30.b and P10.T7.a (lane `gate-p09-t33a`, 2026-10-29, at
  `GENERATOR_VERSION` 23, no output moves).** T33.a was built on an older base; its three commits
  were applied over the integration tip with their textual conflicts resolved (both docs of
  `placement::resolve`; the union of `query/motion.rs`'s imports, `position_at` giving a catalogue
  system its class's motion before T28.b.b's `match` on the rules; P10.T7.a's lint attribute and
  tag, then T33.a's five; both lanes' records). One semantic gap remained:
  - _The Kepler arm's `match` on the origin._ T28.b.b's `motion::kepler_regime_position_at` did
    not cover `SystemOrigin::CatalogueSystem` (E0004). It gains that arm, its class's broken line
    (`system_of_record(..).position_at(t)`), the answer `position_at` gives before it reads the
    rules, rather than a drift at the epoch velocity, which is the remnant's after the explosion. So
    a catalogue system moves by its class under either arm, as T28.b.b's "its own source moves it"
    has a member do; `regime_of`'s doc says the regime does not choose its law. Test, in
    `galaxy/motion/kepler_regime_hold.rs`: `a_catalogue_system_moves_by_its_class_under_either_arm`
    (the core collapses of the class cell at the black hole, on T28.b.b's fixture: the same entries
    and systems under both rules; `position_at` under both arms and `kepler_regime_position_at` are
    the class's line, bit for bit, at ±100 and ±1,000 yr and either side of an explosion inside the
    clock window; a millennium on the far side of each explosion from the epoch, the arm's answer is
    the line and a drift at the epoch velocity is not; every entry is of layer E, its v_fast under
    layer E's pad speed and its moves within either arm's walk pad; 20 entries, none inside the
    7.19 ly influence radius, pinned; 11.6 s on the pod). The tag registry's comment now says
    T33.a's tags follow P10.T7.a's.
  - _Joins checked, no gap._ (1) The loss cone: `CandidateOutcome::InLossCone` is the grid
    candidates' outcome, after the claim hook; a catalogue candidate never reaches it, `verdict_on`
    answers `Clear` for a catalogue system, and the class reads no rule, so a core collapse is kept
    or thinned alike under either arm (T33.a's deferred finding (2), no Kepler regime). A placed
    class without a process still answers `KindNotGenerated`, and a carved-out grid candidate
    `NoSuchSystem`. (2) The pads: no walk reaches a catalogue system before P09.T37 (the range
    query, the frame search, the sky census and the lensing walk read grid cells and feature
    sources), so `walk_pad` pads grid records alone. Every core collapse's primary is of band E, so
    its layer is E, and its v_fast, at most |v| + 990 km/s with plan 08's cut below 1,000 km/s, is
    under layer E's 3,000 km/s, which the test holds; T37's source pads by the class's own v_fast.
    (3) T30.b's victims are the centre's feature-level IDs (`SystemIdKind::Centre`, indices 1 to N),
    which `resolve_centre_member` answers; T33.a's dispatch takes `SystemIdKind::Catalogue` alone,
    where class 7, `TIDAL_DISRUPTION_VICTIM`, is never placed and answers `NoSuchSystem`, and the
    server's system cache and the planetary context still send a centre ID through `resolve` and
    refuse its stars. (4) The headroom: the core collapses are a field class in the class grid's
    index space, not on the centre's list, so `CentreListForecast`'s entries (the victims and the
    carved classes' candidates) are unchanged, and its doc's other classes there are still T36's
    and T43's. (5) Census C1 counts the grid records of P09.T28.b.a's pre-pin, which no class claims
    at 23 (`RECENT_DEATH_CLAIM` is `None`), so no catalogue system is among them; under every 25
    hold the catalogue systems move as they do at 23.
  - _Met P07.T10.c's server at 24 (2026-10-30)._ `extinction`'s `match` on `system_motion` lacked
    `CatalogueMotionNotTraced` (E0004). A catalogue system is now refused at every time, as a
    centre member is: `no_such_system` to `extinction`, `unknown_system` to `resolve_system` and
    `scene_ship`. The task that gives its class's broken line a tracer lifts the refusal there.
- **P09.T33.a's leans, kept (main's research, 2026-10-29).** Its four leans stand and nothing
  blocks landing. Two follow-ups, each its own commit, no golden moved:
  - _[Fe/H] at age zero, pinned (determinism audit)._
    `a_candidate_s_metallicity_is_read_at_age_zero` (`core_collapse/tests.rs`): every candidate of
    a young-disc cell and of the cell at the centre (5, 197 and 21 by component) has the
    composition of the one normal of `class.core_collapse.metallicity` from its component's law at
    its site read at age zero, to the bit, and both young laws give the same bits at its age at the
    epoch, the age plan 06's `draw_metallicity` reads a grid record's law at, and at
    `RECENT_DEATH_AGES`; 8.3 s on the pod. A slope in either law fails it, and the class must then
    read its candidate's age.
  - _`v_fast`'s reason, corrected._ `v_fast` = max(|v|, |v + kick|) stays, but "P09.T33.a as
    built"'s reason, that over the field's thinning interval a progenitor above about 360 km/s
    outruns `LIFETIME_REACH` before T, is wrong: nothing reads an entry before −(H + L), the source
    horizon's start, so a progenitor's line is read for at most L + 2H before T, and only one above
    4,096 ly ÷ (L + 2H), about 4,649 km/s (`lifetime_cap`'s edge), could outrun the reach.
    `DeathMarks::of`'s doc now says so, and keeps the max as the fastest member's speed over the
    entry's lifetime, the brainstorm's v_fast. A debug guard at that speed is P09.T35's.
- **P09.T34.a as built (lane `p09-t34a`, 2026-10-30, at `GENERATOR_VERSION` 23, no output
  moves).** `type_ia::TypeIaProcess` (`type_ia/process.rs`), in `ProductionClasses`; golden
  `catalogue_classes/type_ia_class` new; `rng/tags.golden` +3, `class.type_ia.` `site`,
  `velocity` and `survivor`.
  - _Sites._ Σ_c density × `DelayTimeDistribution::rate_per_system_per_year` (new) × the interval,
    no `component_share`: the layer-D budget × its rate per layer-D system at 23. P09.T2.c brings
    its (1 − φ) to this rate; T36's feature and globular hosts come out of it then.
  - _Progenitors._ `IaProgenitor::draw_in(.., ComponentId, ..)` (new): `draw` after its pick, bit
    for bit, with the site's mark choosing the component (lean, 2026-10-30). T36 can call it.
  - _Member 0_ moves at its drawn velocity and carries the shell. P12.T5.b reads T and the site
    from `TypeIaEntry::site()`, and the event from `supernova().explosion_event()`.
  - _Member 1_ exists where `IaLeftover::has_survivor()`: a donor flung out isotropically from T, or
    a Type Iax's unkicked dwarf. Its record's mass is its own star's (layer A–D). Before T it sits
    at member 0's place, so T37 lists it from T only (lean).
  - _No stars._ `observe::stars_of` and `BriefModel::of_record` now return `Result`,
    `Err(KindNotGenerated)` for both members; R06's and P12's callers take the new signatures.
  - _Pads._ A survivor outruns its layer's walk pad in ±H (3 of 7 at the centre). T34.b's layer-D
    raise covers member 0 only; T37 pads this class by `v_fast`.
  - _T34.b's tests._ P09.T34's "Tests, added" paragraph continues T34.b's bullet (the pad raise,
    the hypervelocity weight), so T34.a does not build it.
  - _Merge gap._ The extinction handler's match lacked T33.a's `CatalogueMotionNotTraced`; such a
    target is `no_such_system` (own commit).
  - _At 26._ The census's Type Ia share of shells (0.19–0.21) is against field core collapses
    alone; from P09.T2.c it moves to T36.a's or T38's census.
- **P12.T5.b's addition to `supernova.rs` (lane `p12-t5b`, 2026-10-30, at 23, no output moves).**
  `LightCurve::terms_at(Years) -> SupernovaTerms` (`breakout`, `plateau`, `radioactive`, `total`,
  `weighted([f64; 3])`, `ZERO`). The terms stay in erg/s and are converted once, so `total` and
  `weighted([1.0; 3])` are `at`'s bits; `at` is now `terms_at(age).total()`, and
  `phase_four_golden` is unchanged. A retune of a template (the Type Ia's rise among them) moves
  plan 12's band tables and bounds in `alerts::light` and `alerts::registry`; their recompute tests
  and `alerts/registry.golden` catch it.
- **Ruling 145.5 as built (P09.T13; lane `p09-bound-share`, 2026-10-31, at `GENERATOR_VERSION` 24,
  behind `BOUND_SHARE_VERSION` = 26).**
  - `kinds::globular::history` and `history_on_orbit` take the cluster's `population` before its
    mass, and `bound_share(galaxy, population, age)` is the μ of the history's law and of its mass
    loss `μ M₀ ÷ t_dis`. `GalaxyRules::bound_share` is `BoundShareRules::{Bm03, OwnMu}`; this is
    the first galaxy 26 hold, so it adds `GalaxyRules::every_26_hold()` (test-only).
  - `OwnMu`: `fates::present_share_of_formed_with` under the galaxy's function and
    `fates_for(population)`, counting the stars and white dwarfs (`CensusFates`,
    `StarsAndWhiteDwarfs`), on `Galaxy::companions()`, a lazy cell built once a galaxy. (Lean,
    2026-10-31: neutron stars and black holes are not counted. That gives the ruling's "about
    0.46", 0.47 at 12 Gyr; with them the halo's would be 0.552.)
  - The 26 bump deletes `BOUND_SHARE_VERSION`, `BoundShareRules` with its `Bm03` arm, the field,
    the `match` in `bound_share` and the tripwire `the_bound_share_is_held_only_until_the_26_bump`.
- **P09.T43 as built, the field half (lane `p09-t43`, 2026-10-31, at 24, no output moves).**
  `catalogue_classes::lbv::{LbvProcess, LbvEntry, may_host, horizon_window}` and a `pub(crate)`
  `lbv_claims`; the process in `ProductionClasses`; scratch `tables::lbv::LBV_SAMPLER`; five tags.
  - _The sampler (for P15.T10.a)._ Nodes 32, 36, 43, 55 and 97 M☉ (to 150), each the first and
    last age of every window of its masses over [Fe/H] −2.4 to +0.3, widened 2% and rounded out to
    the kyr; a slow test rebuilds it. `LbvProcess::new` weights the nodes by the mass function.
  - _No redraw (lean, 2026-10-31)._ No draw moves a window (`no_draw_moves_a_window`), so the class
    thins on attempt 0 as the grid reads it; the origin gains no attempt (T33.a's finding (7) moot).
  - _Prefilter (lean, 2026-10-31)._ `may_host`: the mass in a node and the age within its
    [first − H, last + H + L], tighter than the lowest node and longest end; both sides read it.
  - _Sites at the epoch (lean, 2026-10-31)._ A host's site is its epoch position on the young
    field's density, its [Fe/H] and velocity read there, as the grid's; entries move straight.
  - _Resolution._ A class-4 `111` ID resolves through the class where it answered
    `KindNotGenerated`; the server's serving test pins one host.
  - _For P09.T35:_ ask `lbv_claims(galaxy, record)` of every layer-E record under `Claimed`, after
    `recent_death_claims`, with or without an explosion site (`claims_with`'s site gate must not
    hold it). The class's core-collapse claim is the grid's (`the_class_s_core_collapse_claim_…`).
  - _Waiting:_ the band-E runaways' sites (after P08.T12, with P09.T33.b); the nurseries' and young
    open clusters' lists after `TYPE_IA` (after P09.T36.a), and the centre's from N + 1 (lean,
    2026-10-31: P09.T30.b's record). Companions are not carved (T33.a's finding (3)). A track change
    reruns the scan.
  - _Goldens._ `catalogue_classes/lbv` new (hosts per galaxy at its end); `rng/tags.golden` +5.
- **P09.T42, records (lane `p09-t42`, 2026-10-31, at 24 and 3; client only).**
  - `RADIUS_STEPS_LY` runs 0.001–500 ly (18 steps). The selector and `QUERY EDGE` write a radius
    through `formatQueryRadius` (`chartModel.ts`): `0.01 ly` and up as before, the fine steps in AU
    to three figures (`316 AU`, `126 AU`, `63.2 AU`). The scale bar's ladder was already ly then AU.
  - `LocalChartState.distanceDecimals` is now `distanceUnit: ChartDistanceUnit` (`symbol`, `perLy`,
    `decimals`) from `distanceUnitFor(radius)`; `SystemList`, `FeatureList`, `SystemReadout` and
    `FeatureReadout` take `distanceUnit` and write through `formatChartDistance` and
    `formatChartOffset`. Below 0.01 ly every distance and offset is in AU to 0.1 AU (lean,
    2026-10-31), the lists' headers `DIST AU`; other charts read as D21 had them.
  - `BLACK HOLE`: T41.c's readout already asks at the chart's time and reads mass and the 2–10 keV
    total; T42 adds only tests (asked again at a new chart time; an absent luminosity Missing).
    Nothing is missing from the wire for it.
- **P09.T42 fixes, records (lane `p09-t42`, step 2, 2026-10-31, at 24 and 3; client only).**
  - _`DIST` column._ Neither switched to ly nor widened: `formatFeatureDistance` (`chartModel.ts`)
    keeps the chart's unit and writes a feature's distance at the chart's precision while it fits
    `DISTANCE_COLUMN_CH` (7), and past it to three figures (`formatSignificant`: `126,000`, `300`,
    `6.32E8`), in `FeatureList` and `FeatureReadout` alike. Systems always fit.
  - _`CentreEntry`._ `MAX_DECIMALS` 5: 0.00001 ly (0.63 AU), a hundredth of the finest chart's
    radius, the finest chart's furniture step; the field is 13ch, for `-65,535.99999`.
  - _Furniture._ `placeDecimalsFor(radius)` (`chartReadings.ts`): 1 decimal from 10 ly, one more
    per decade below, 5 below 0.01 ly; `radiusReading`, `placeReadings` and `modeReading` take it,
    the slots widening with it. `HrDiagram` gains `placeDecimals`. Built to a guide draft (lean,
    2026-10-31): the guide states no furniture precision but `POSITION`'s "to one decimal".
  - _`DRIVE RANGE`._ Accepts 0.001–500 ly at four decimals (the query radii's span, D9's default
    radius following it); `RANGE` and `PLANE` labels go through `formatQueryRadius`, so a range
    below 0.01 ly reads in AU (`RANGE 94.9 AU SET`).
- **P09.T40.b fix, the centre's served sphere of influence (lane `galaxy-centre-core`, 2026-10-31,
  at 24 and 3; served value only, no switch).** `CentreStructure::influence_radius_ly` is now
  `galaxy.centre_motion().influence_radius()`, every mass inside, not the cluster-only
  `CentreProfile::influence_radius` (10⁴ ly where the hole outweighs the cluster). The server
  tests' seed serves 111.09 ly; `features.golden` and the client's `aCentreDetail` moved with it.
- **P09.T10 perf, the remnant attempts bit for bit (lane `galaxy-centre-core`, 2026-10-31, at 24;
  no output moves).** `remnant_fits` reads `stellar::system::RemnantFate` (`of`, `remnant`,
  `kick_is_none_or`), from `sse::fate_to_age` and `fate_of`, not a `StarModel`. The track builder
  holds the companion-stripped mark unread (`Builder::with_companion_mark`) and reads it only where
  the capture window's two widths disagree; the remnant stage, only where the kick's verdict
  depends on it. Dwarf-core, centre and global-list members share `remnant_fits`.
- **P09.T10 perf, the tabulated remnants at 26 (lane `galaxy-centre-core`, 2026-10-31, at 24,
  behind `TABULATED_REMNANTS_VERSION` = 26).** `GalaxyRules::member_remnants`
  (`MemberRemnantRules::{EveryAttempt, Tabulated}`, `features/members/remnants.rs`). Under
  `Tabulated`, `draw_member` asks `tabulated_fit` before `remnant_fits`: a white dwarf below
  `ClusterModel::white_dwarf_ceiling` (lazy, per stellar rules) is kept on its draws-only kick
  (`StandardKickLaw::white_dwarf_kick`); a neutron star whose systemic speed and the ordinary
  kicks' floor both reach `v_eff` is refused. Only `draw_member` reads it; dwarf-core, centre and
  global-list members keep `remnant_fits`. The bump deletes the hold as its tripwire says.
- **P12.T5.c's addition to `supernova.rs` (lane `p12-t5b`, 2026-10-31, at 23, no output moves).**
  `SupernovaEntry::pulsar() -> Option<NeutronStar>`, the neutron star the class's test put in the
  entry, which plan 12's magnetar walk reads to drop entries whose field is born at or below the
  critical field before it builds their stars.
