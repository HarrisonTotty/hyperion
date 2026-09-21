# Galaxy Generation

Brainstorm for the core procedural generation system: how one seed becomes an actual-size galaxy of
star systems and bodies that the simulation can query on demand. This is a design exploration, not a
plan. Decisions are marked **Lean** where there is a recommendation, and the open ones are collected
under [Open questions](#open-questions). What has been settled is under [Decisions](#decisions).

## Goal and scope

Given a `u64` seed, produce a galaxy with the size, structure and statistics of a real barred
spiral: on the order of 10¹¹ star systems across roughly 100,000 ly. Every station on the bridge
must be able to ask questions of it ("what is within 50 ly?", "what orbits this star?", "where is
this moon at T+3 h?") and always get the same answer for the same seed.

In scope:

- The determinism foundation: seeds, random streams, identifiers, coordinates.
- Galaxy structure and where star systems are.
- Stars of every class, multiple-star systems and stellar remnants.
- What lies between the stars: brown dwarfs, rogue planets, isolated remnants, dust and nebulae.
- Planetary systems: planets, moons, belts, rings, and their bulk physical properties and orbits.
- Time: motion, stellar evolution and events, all as functions of one universe clock.
- The query interface the rest of the simulation uses.

Out of scope, each deserving its own brainstorm, but with hooks defined here:

- Planet surfaces below orbital scale: terrain, local biomes, anything a landing party would see.
  What a crew sees from orbit and through sensors is in scope; see [Decisions](#decisions).
- Life, species, civilisations, factions, history and languages.
- LLM enrichment of descriptions.
- The faster-than-light drive itself. Its effect on this design is settled; see
  [Decisions](#decisions).

## What to take from the references

| Reference                      | Take                                                                                                                                                                                                                                                                                                                                         | Leave                                                                                                                                                                                                          |
| ------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| No Man's Sky                   | Everything is a pure function of seed and address, generated on arrival and never stored. Only player changes are saved. A short address (the twelve-glyph portal code: one glyph for the planet, three for the system, then the region's Y, Z and X) identifies any place and can be shared.                                                | A uniform grid of regions with no galactic structure. Stars that are backdrop, not bodies. Planets far smaller than real ones that neither orbit nor rotate. Life on nearly every world. One biome per planet. |
| Elite Dangerous: Stellar Forge | Top-down generation: galaxy-scale mass and age distributions decide how much material each sector holds. An octree of sectors where large cells hold the rare massive stars and small cells hold the dwarfs. A 64-bit ID packing sector, layer, system and body. Real orbits at real scale. Catalogue stars overlaid on the procedural fill. | The hand-painted Milky Way density map; ours must come from the seed. Simulating each system's formation through epochs is attractive but costly; see [Planetary systems](#planetary-systems).                 |
| SpaceEngine                    | Proof that catalogue data and procedural fill can share one hierarchy, from galaxy down to moon, with seamless scale changes.                                                                                                                                                                                                                | Rendering concerns. HYPERION shows the galaxy through instruments, not a free camera.                                                                                                                          |

The common lesson: **the galaxy is a function, not a database.** HYPERION's addition is that the
function must be astrophysically defensible at every level, because the consoles will expose the
numbers.

## Determinism foundation

This part must be right first. Everything else can be refined later without breaking it; this
cannot.

### Random streams, not a random sequence

A single generator advanced through the whole pipeline is fragile: adding one draw for a new
property shifts every later value, and a different universe falls out. Instead, each value comes
from a stream keyed by what it is for:

```text
stream = hash(universe_seed, object_id, domain_tag)
```

`domain_tag` names the property group (`"star.mass"`, `"planet.orbits"`, `"moon.count"`). Adding
ring generation later then cannot change any star's mass. This is the counter-based approach of
Salmon et al. (Random123), and it is what makes lazy, out-of-order and parallel generation safe.

**Lean:** own the generator and the samplers. The `rand` crate allows its distributions to produce
different values after a minor release, and a dependency bump must never be able to move a star.

- **Generator.** One of the Random123 block functions (Threefry2x64-20 or Philox), with key = (seed,
  domain tag) and counter = (object ID, draw number). For a fixed key it is a bijection on the
  counter, so two different IDs can never share a stream, and it was tested on exactly this kind of
  input: structured, sequential counters such as adjacent cells and consecutive candidates. A body's
  ID is wider than 64 bits, so its body index shares the second counter word with the draw number. A
  SplitMix64 construction is simpler but weaker here. Every stream would be a window onto one 2⁶⁴
  cycle, and with some 10¹² streams a 64-bit state makes tens of thousands of them collide outright.
  The cost difference, about 10–20 ns per block, is nothing next to a density evaluation.
- **Domain tags** are strings hashed to a `u64` at compile time, with a test that no two collide. A
  tag is never renamed. Enum discriminants are riskier because they can be renumbered by accident.
- **Samplers** are hand-written for the few distributions needed: uniform, normal, log-normal,
  Poisson, power law, piecewise. Poisson means run from well under 1 in a fine cell at the rim to a
  few hundred thousand in a coarse cell at the galactic centre, so it needs two exact methods:
  inversion of the cumulative distribution for small means, and Hörmann's transformed rejection
  (PTRS) above that. A rounded normal is not good enough, because its error at a mean of 1.2 is over
  5% and would fail our own statistical tests. The normal sampler must avoid ln 0. The power law
  needs the exponent 1 special case and an explicit upper mass limit.

Draws never depend on time. An object's draws are fixed by its ID, and time enters only through
closed forms and thresholds: a star's state is its evolution evaluated at age plus clock time, and a
property that changes with phase is one fixed uniform compared against a threshold that moves. That
is what lets any moment be evaluated directly, forwards or backwards, with no history to replay.
Streams of events in time are keyed in two steps: the object's event key is one block output from
(seed, tag; ID), and the counter under it is (bin or cycle number, draw). See
[Events in time](#events-in-time).

### Floating point

Rust's `+ - * /` and `sqrt` on `f64` are IEEE-exact and identical on every target we care about
(x86-64, AArch64, WebAssembly), and the compiler never fuses a multiply and an add on its own.
`sin`, `cos`, `exp`, `ln`, `powf` and `powi` are not: they call the platform's maths library or have
unspecified precision, and may differ between operating systems and CPU architectures. The server is
authoritative, so clients never regenerate anything, but a saved universe must survive being moved
to another machine.

**Lean:** route every transcendental function in `hyperion-sim` through the pure-Rust `libm` crate,
enforced by a Clippy `disallowed_methods` list. Two cautions:

- `libm` is reproducible across platforms but not across its own versions. Its functions are not
  correctly rounded, and a release may replace an algorithm. That is the same risk as `rand`, so the
  version is pinned exactly, and golden tests cover a few function values.
- The bits of a NaN are unspecified, so float bits are never hashed.

Random decisions compare a hashed integer against an integer threshold. This is a convention, not a
second line of defence: the threshold is itself computed from floats, so reproducibility rests on
`libm`. The convention does remove the edge case of a uniform draw equal to exactly 1.

### Generator version

A universe is identified by `(seed, generator_version)`. Any change that alters output bumps the
version, and a saved game records the version it was created with. Before the first release the
version can be bumped freely; afterwards old versions either remain runnable or saves are declared
incompatible. Golden tests make accidental changes visible: a change that moves a pinned star fails
CI until the version bump is deliberate.

### Time

The galaxy has one clock. `UniverseTime` is an `i64` of seconds plus a `u32` of nanoseconds,
coordinate time in the galaxy's rest frame. An `f64` of seconds would resolve only about a
millisecond at the far end of the span below. Zero is the epoch: the instant the [fields](#fields)
describe. Everything that has a position or a state takes a time, and is symmetric in it.

Two constants belong to the generator version:

- **H = 1,000 years**, the clock window. Within it a visit to any object is accurate to the
  guarantees of [Orbits and time](#orbits-and-time). Beyond it nothing breaks locally, but the
  errors of straight-line drift grow with the square of the time and distant events stop being
  findable.
- **L = 2¹⁸ years** (262,144), the light-crossing bound. The root cube's diagonal is 227,023 ly, so
  no observer inside it can receive light that left a source more than L before.

Together they give the source horizon, from −(H + L) to +H: every emission time that any observer
inside the cube can see or touch during play.

### Coordinates

An `f64` in metres cannot address the galaxy. At 50,000 ly from the origin (4.7 × 10²⁰ m) adjacent
representable values are about 65 km apart. Positions need nested frames:

| Frame    | Representation                                                 | Resolution                       |
| -------- | -------------------------------------------------------------- | -------------------------------- |
| Galactic | Integer cell (1 ly cubes, `i32` per axis) + `f64` metre offset | about 2 m anywhere in the galaxy |
| System   | `f64` metres from the system barycentre                        | about 1 mm at 50 au              |
| Body     | `f64` metres from the body's centre                            | sub-micrometre in low orbit      |

A ship is always in exactly one frame, and changes frame at defined boundaries (entering a system's
sphere of influence, entering a body's). Newtypes keep the frames from being mixed, as `rust-dev.md`
already requires for quantities.

A system's sphere of influence is its tidal (Jacobi) radius, inside which its own gravity beats the
galaxy's: (G m ÷ (4Ω² − κ²))^⅓, where m is the system's mass and Ω and κ are the circular and
epicyclic frequencies at its position, from the potential tables of
[Galaxy parameters](#galaxy-parameters), dark matter included. Around a point mass M at distance R
this reduces to the familiar R × (m ÷ 3M)^⅓. It is about 4 ly for a solar mass at 26,000 ly, and it
takes a floor in the harmonic core of a cluster, where 4Ω² − κ² goes to zero. Inside a
[large feature](#large-features) the same formula is applied about the feature's centre, and the
smaller radius wins: 3 ly from the central black hole it is about 0.01 ly. A system on a Kepler
orbit about the black hole takes its radius at pericentre, which is constant in time and is also the
radius to which its planetary system has been stripped: 2.7 au for a semi-major axis of 0.01 ly. The
radius still depends only on the system and its orbit. Spheres this size overlap their neighbours as
a matter of course, and Poisson placement has no minimum separation either: in the grid alone some
fifteen million pairs of systems lie within 0.1 ly of each other, over half of them in the nuclear
disc, as real stars sometimes do. They are unbound fly-bys, and their separation changes by a few
hundredths of a light-year a century. **Lean:** leave such pairs as two systems, so that generation
stays independent, and settle the frame when it is asked for. A ship inside several spheres belongs
to the system for which distance ÷ radius is smallest, and to the lower ID on an exact tie. It
changes frame only when another system's ratio is smaller by a tenth, so a ship drifting along a
boundary does not flicker between frames. The rule needs only the systems near the ship, which the
range query supplies. It is evaluated at the query's time, because systems move: a system's frame
rides with its barycentre, and a ship leaving a system inherits the system's velocity.

The galactic axes need a definition, because a fictional galaxy has no Sun to measure from. The
origin is the galactic centre. +x runs along the bar's long axis, and +z is galactic north, chosen
so that the galaxy rotates counter-clockwise seen from the north. The spiral arms trail. The named
directions follow: coreward is towards the z axis, spinward is the direction of rotation, north is
+z. They are local directions, so they are undefined at the exact centre and turn noticeably across
a chart close to it. Fixing the bar to the x axis costs nothing, since the bar's angle only ever
meant something relative to the Sun, and it makes the density bound in
[Exact placement by thinning](#exact-placement-by-thinning) exact for the bar.

The 1 ly cells of this frame and the larger generation cells of [Identifiers](#identifiers) are the
same grid at different scales: a generation cell's coordinate is the light-year coordinate divided
by the cell size, rounded down. Positions are drawn as integer light-years plus an offset, never as
one `f64` across a whole 128 ly cell, where the last bit is worth 256 m.

Units inside the sim are SI. Light-years, astronomical units, solar masses and the like are
conversions at the edges and in generation code where the source formulae use them, always through
named newtypes.

### Identifiers

Every generated object needs a stable ID that also says where to regenerate it from. Following
Stellar Forge, a system ID can pack into 64 bits. As a sketch, with a root cube 131,072 ly (2¹⁷)
across and a finest stellar cell of 8 ly:

| Field         | Bits    | Notes                                                    |
| ------------- | ------- | -------------------------------------------------------- |
| Layer         | 3       | 8 values: 5 stellar, 2 substellar, 1 reserved (below)    |
| Cell x, y, z  | 42 − 3k | 14 − k bits per axis at cell size 8 × 2ᵏ ly (k = 0 to 4) |
| Index in cell | 16 + 3k | 65,536 at the finest layer, 2²⁸ at the coarsest          |
| Spare         | 3       | Zero, except in the rogue planet layer (below)           |

Each layer up has cells twice as wide, so it needs one bit fewer per axis, and those three bits go
to the index. This matters because the coarse layers are the ones that fill up: a 128 ly cell has
4,096 times the volume of an 8 ly cell. With a fixed 16-bit index the coarsest layer would overflow
at a total density of only about 5 systems per cubic light-year, which the nuclear disc exceeds
several times over. With the sliding split, the finest layer is the tightest again, at about 170 per
cubic light-year, which only a nuclear or globular cluster core reaches. Those are not placed by the
grid at all; see [Dense features](#dense-features-clusters-and-the-galactic-centre).

The split follows a layer's cell size, not its layer value, because the two substellar layers of
[Between the stars](#between-the-stars) have sizes of their own. The brown dwarfs reuse the 16 ly
size. The rogue planets use 4 ly cells (k = −1), which need 45 cell bits, and take the three spare
bits to keep a 16-bit index: 1,024 per cubic light-year, which their numbers at the galactic centre
require. The two take the last layer values, so the spare bits of the other layers are the only room
left to grow.

The encoding must be canonical so that one system has one ID. Cell coordinates are stored with an
offset so that they are unsigned (the root cube runs from −65,536 to +65,536 ly, and the planes x =
0, y = 0 and z = 0 are cell faces at every layer), unused bits are zero, and anything else is
rejected. One layer value is reserved for objects that are not placed by the grid: members of
[large features](#large-features) and pinned content. The remaining 61 bits are laid out differently
under it; see [Dense features](#dense-features-clusters-and-the-galactic-centre).

A well-formed ID does not always name a system. The index is a candidate number (see
[Exact placement by thinning](#exact-placement-by-thinning)), so resolving an ID means recomputing
the cell's candidate count, checking the index against it, and rerunning that candidate's acceptance
test. All of that is constant-time. An ID that fails resolves to "no such system", and IDs read from
saves or the protocol always go through this check.

Bodies are `(SystemId, body_index: u16)`. An event is the ID of its system or body plus a 64-bit
word: a 16-bit tag from a registry, a signed 40-bit bin or cycle number, and an 8-bit number within
the bin (see [Events in time](#events-in-time)). Generated data is never stored, because it can be
rebuilt from the ID. IDs are what go into saves, the protocol and player logs. A human-readable
designation is derived from the ID (in the manner of Elite's `Sector AB-C d12-3`), which guarantees
every one of 10¹¹ systems has a unique catalogue name before any language generation exists. Proper
names from generated languages are a later overlay.

## Pipeline

Each stage is a pure function of the seed, the object's ID and the output of the stage above it.
Nothing reads sideways from a sibling, which is what allows any object to be generated alone.

```text
seed
 └─ galaxy parameters          morphology, masses, sizes, arms, dark halo, accretion history
     ├─ potential tables       rotation curve, escape speed and dispersions, computed once
     └─ fields                 density, age, metallicity, gas and dust as functions of position
         ├─ large features     clusters, nebulae, streams, the galactic centre, their members
         ├─ catalogue classes  rare systems carved out of the cells so that charts find them
         └─ cells              field systems, brown dwarfs and rogue planets: position, population,
             │                 primary mass, age
             └─ system         metallicity, multiplicity, companion masses
                 └─ stars      phase, luminosity, radius, temperature, class, variability
                 └─ bodies     planets, moons, belts, rings, orbits
                     └─ hooks  surface seed, habitability, resources
```

### Galaxy parameters

The seed chooses the galaxy's gross properties from the observed ranges for large spirals:

- **Stellar mass**, 3–10 × 10¹⁰ M☉, log-uniform. The number of systems is derived from it, not
  drawn: mass divided by the mean present-day mass of a system, which is a once-per-galaxy
  quadrature over the mass function, multiplicity, the age distributions, stellar lifetimes and
  remnant masses. It comes to 0.48 M☉ under Kroupa's function and 0.55–0.60 under Chabrier's, living
  stars, remnants and companions together, and varies by only 3% between the old populations. So a
  galaxy holds 0.5–2 × 10¹¹ systems, and one of the Milky Way's mass about 10¹¹.
- **Shares and sizes** of the [populations](#populations): disc scale length (the Milky Way's is
  about 2.6 kpc), scale heights, bulge, bar and nuclear disc. Sizes are tied to the mass they hold,
  as mass^⅓ with a small scatter. Independent draws were tried first and reached a rotation speed of
  390 km/s at 1 kpc and a centre dense enough to overflow the rogue-planet index.
- **Star formation history**: how fast the thin disc's formation rate has declined.
- **Arms**: their number and pitch angle. The bar has no orientation parameter, because it defines
  the x axis (see [Coordinates](#coordinates)). Its pattern speed follows from its corotation
  radius, 1.0–1.4 times its half-length: 38 km/s per kpc for Milky Way values, against 33–41
  measured.
- **Gas**: the mass of the gas disc, about 15% of the thin disc's.
- **Dark halo**: an NFW profile, which has a closed-form enclosed mass and potential and, unlike a
  logarithmic halo, a finite escape speed. Its mass is the stellar mass divided by 0.157 f★, with f★
  log-uniform over 0.12–0.45 as for massive spirals, and its concentration follows Dutton and Macciò
  (2014) with 0.11 dex of scatter.
- **Accretion history**: the time of the last major merger (6–11 Gyr ago), a short list of accreted
  progenitors with their masses, times and orbits, and the number of globular clusters, which scales
  with the dark halo's mass. See [Streams and accreted structure](#streams-and-accreted-structure).
- **Central black hole**: its mass follows the M–σ relation with scatter, read from the bulge's
  projected velocity dispersion of 105–115 km/s. The relation with bulge mass overpredicts the Milky
  Way's twelvefold, as it does for barred pseudobulges generally.

Once per galaxy the mass model is reduced to **potential tables**: circular speed, the frequencies Ω
and κ, and the potential on a logarithmic grid of about 64 × 64 points in R and z. The potential is
built as a sum of Gaussians whose dimensionless coefficients for each profile are fitted offline, so
each one's force is a one-dimensional quadrature. Velocities, escape speeds, tidal radii, the bar's
pattern speed, the orbits of kicked remnants and the tracks of streams all read these tables. Over
4,000 draws the rotation curves come out flat, with 210–270 km/s at 8 kpc for most seeds (the Milky
Way's is about 230), and Milky Way values give an escape speed of 574 km/s there against 500–580
measured.

A universe holds one galaxy (see [Decisions](#decisions)), so there is no intergalactic scale above
this one. **Lean:** version one generates only barred spirals in the Milky Way's size class. Other
morphologies for other seeds are variety we can add without touching the layers below. Satellites
inside the root cube exist as dwarf cores with their streams; those beyond it, such as a Magellanic
Cloud, remain addable variety.

### Fields

Analytic functions of galactic position, one set per stellar population:

- **Density.** Thin disc, thick disc and nuclear disc as double exponentials, bulge and long bar as
  triaxial profiles, halo as a sum of power-law components. Spiral arms are logarithmic spirals that
  modulate density, weakly for old stars and strongly for young ones, which is why arms are traced
  by blue stars and nebulae and not by mass.
- **Age.** Each population has its own age distribution: halo and bulge old, thick disc old, thin
  disc a broad range, arms weighted young.
- **Metallicity.** Falls with galactic radius (about −0.05 dex per kpc in the Milky Way disc) and
  with age, with scatter. It matters downstream: giant-planet occurrence rises steeply with
  metallicity. Small rocky planets depend on it only weakly and are known around old, metal-poor
  stars, so they should thin out only at the very low metallicities of the halo.
- **Velocity.** Each population has closed-form kinematics in the tabulated potential; see
  [Orbits and time](#orbits-and-time).
- **Dust and gas.** A field, not a set of objects, with density, pressure and extinction; see
  [Between the stars](#between-the-stars).

Because these are closed-form, a galaxy map at any zoom integrates fields and never touches
individual stars. It is not free, though; see [Visualiser](#visualiser).

### Large features

Objects big enough to shape their surroundings, or to be charted from far away. Counts are rough
Milky Way figures:

| Feature                      | Count                                                          | Size                                  | Members                                                                 | Follows                                                                                                                      |
| ---------------------------- | -------------------------------------------------------------- | ------------------------------------- | ----------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| Globular cluster             | 80–800, with the dark halo's mass; the Milky Way has about 160 | up to 500 ly                          | 10⁴–10⁶ systems, coeval and old, split by class and dynamically evolved | 40% in situ (bulge and thick disc), the rest the accreted halo                                                               |
| Open cluster                 | about 10⁵                                                      | 10–100 ly                             | 10²–10⁴ systems, coeval, a few Myr to several Gyr old                   | Young disc under 100 Myr, the old thin disc beyond                                                                           |
| OB association               | tens of thousands                                              | up to 300 ly: expansion speed × age   | Unbound and under about 30 Myr                                          | Young disc, so the arms                                                                                                      |
| Star-forming region          | about 10⁴                                                      | 10–300 ly                             | An embedded cluster still forming, protostars, gas                      | Young disc                                                                                                                   |
| Molecular cloud, dark nebula | thousands                                                      | 50–300 ly                             | None: gas and dust only                                                 | Dust field                                                                                                                   |
| Supernova remnant            | about 10⁴, a thousand or two of them bright                    | 10–800 ly                             | It is a system: the star that died                                      | Four in five core collapses are inside associations, which hold a quarter of the shells; Type Ia shells follow the old stars |
| Stellar stream               | 150–1,500                                                      | 30,000–10⁶ ly long, 100–6,000 ly wide | 10³–10⁸ systems lost by a cluster or dwarf                              | The orbits of dissolving globulars and accreted dwarfs                                                                       |
| Dwarf galaxy core            | 0–3                                                            | up to 20,000 ly                       | 10³–10⁸ systems, not coeval                                             | Accretion history                                                                                                            |
| Galactic centre              | 1                                                              | about 100 ly                          | Central black hole, 4–5 × 10⁷ more                                      | Fixed at the origin                                                                                                          |

Features follow the density of the populations that make them, statistically and never by reading
individual stars. One bookkeeping rule keeps anything from being counted twice: a population's
budget covers everything born in it, and sets its formation rate, its band shares, its
[displaced classes](#displaced-objects-kicks-and-runaways) and its runaways. Its field density is
the budget times (1 − φ), where φ is the expected share of its systems now inside features, computed
once per galaxy from the catalogue's rates and never from individual clusters. For the young disc φ
depends on age: near 0.9 at a few million years, when most stars are still in the association that
bore them, falling to the bound fraction of about 0.1 by 30–100 Myr. It enters as a factor in the
young field's age distribution.

Open clusters are not all young. They dissolve in 1.3 Gyr × (M ÷ 10⁴ M☉)^0.62 (Lamers et al. 2005).
With 240–360 born per million years on a mass function falling as M⁻², about 10⁵ are alive at once,
but only a third of those are under 100 Myr old and a tenth are over a gigayear. So the catalogue
splits by marking: young bound clusters and unbound associations on the arms, and older clusters on
the old thin disc with its weak arms. A cluster's present mass follows from Lamers's closed form. A
feature carries what a console will want from it: a profile, an extent, gas and dust content where
it has any (a nebula adds to the [dust field](#between-the-stars) along a line of sight), and an
emission class (H II region, reflection nebula, dark cloud, remnant shell, pulsar wind nebula).
Planetary nebulae are not features. They are under a light-year or two across and last some 20,000
years, so one is derived from its central star by the stellar stage and found the way stars are.

A first idea was to generate these in the coarse cells and have them add a bump to the density
field. That does not survive contact with the rest of the design. A globular cluster is wider than a
128 ly cell, so its neighbours would have to read sideways. Its core is a density peak inside a
cell, which the thinning bound never sees. And bounding a whole cell by a core density of tens to
hundreds of systems per cubic light-year would mean millions of wasted candidates.

**Lean:** features are objects of their own, not bumps in the field.

- A feature catalogue is generated from the seed on its own coarse grid (cells of 4,096 ly), by the
  same thinning as the systems, with expected counts from the fields. Any query finds the features
  near it by looking at that cell and its neighbours, which is bounded by the largest feature
  radius.
- Features too long to be found from a centre, the streams and dwarf cores, are not in the grid.
  With the galactic centre they form a global list of at most a few thousand entries, built once per
  galaxy in about a second, each with a bounding shell from one integrated orbit. Every query tests
  the list.
- A cluster's members are their own Poisson processes, keyed by the cluster's ID and drawn from
  profiles in the cluster's frame. They share its age and metallicity, so they are coeval as they
  should be. Member IDs sit under the reserved layer value. See
  [What is inside a cluster today](#what-is-inside-a-cluster-today).
- The superposition of independent Poisson processes is still exact, so the field stars need to know
  nothing about the clusters. The range query merges the two.

Features arrive after the first milestone, with a bump of the generator version: the field gives up
the share φ to them, and the [catalogue classes](#events-in-time) are carved out of the cells.
Before the first release a bump costs nothing.

#### What is inside a cluster today

Asking what a cluster holds now, and not what it was born with, brings in its dynamics. All of it
fits one device: a feature's members are split by class, as a layer's are split by population.

- In each feature and mass band the member density is the sum over classes of expected count ×
  profile. The count is a closed form in the feature's parameters, and the profile is normalised and
  never rises with radius, so the cell bound is the sum of the classes' nearest-corner values. One
  uniform draw rejects a candidate or picks its class by the odds at its position, which is how the
  grid picks a population. The class is a derived mark and never appears in the ID. The stellar and
  binary stages draw conditionally on it.
- **Who stays** is decided by kick against escape speed, in closed form from the feature's own
  potential, not by the velocity spread that serves the grid. For an old cluster it is the escape
  speed at birth, about twice today's for a globular that has since lost half its mass and expanded.
  A fit to all 157 clusters of the Baumgardt–Hilker catalogue gives the central value to 2.5%: √(GM
  ÷ r_h) × 10^(0.1055 + 0.2550u − 0.0769u²), with u = log₁₀(r_h ÷ r_c). The median is 20 km/s and
  the largest 88. An open cluster's is 1–6 km/s.
- **Neutron stars.** The ordinary kick law of
  [Displaced objects](#displaced-objects-kicks-and-runaways) alone would leave 47 Tucanae 0.3% of
  its neutron stars, against the 10–20% its pulsars demand (Pfahl et al. 2002; Ivanova et al. 2008).
  Two things close the gap: the birth escape speed, and the low mode, whose neutron stars stay bound
  to a companion and are judged on the pair's velocity. Together they retain 18–26% at 100 km/s,
  13–19% at 50, 8–12% at 20 and under 1% in the most massive open clusters. A cluster like 47
  Tucanae holds a few thousand neutron stars, M4 about a hundred, Palomar 5 none. They are also the
  ones in binaries, ready to be recycled.
- **Black holes.** Those that collapse directly are born without a kick and are kept even by an open
  cluster, so about four fifths are retained at birth. They then sink to the core and eject one
  another. The loss is a closed form in the cluster's relaxation time (Breen and Heggie 2013, as
  parametrised by Antonini and Gieles 2020): gone after four to six relaxation times at constant
  radius. Today that leaves none in dynamically old clusters, tens to a few hundred in a typical
  massive one, and thousands in a giant like ω Centauri. The constants want an offline fit to the
  CMC cluster catalogue (Kremer et al. 2020).
- **Segregation by mass.** A class of mean mass m has the profile (1 + r² ÷ r_c²)^(−3q ÷ 2) times a
  common outer taper, with q = m ÷ turn-off mass (Heinke et al. 2005). Neutron stars, heavy white
  dwarfs and binaries, which are classes of their own by system mass, are concentrated. M dwarfs are
  extended. Black holes form a compact subsystem of their own, and a cluster rich in them has a
  large core.
- **Depletion of dwarfs.** A cluster evaporates from the bottom of the mass function. The slope for
  0.2–0.8 M☉ is −0.46 − 0.79 × (log₁₀ of the relaxation time in years − 9), with 0.54 of scatter
  drawn per cluster, against a canonical −1.5. At 47 Tucanae's slope band A holds a third of what
  the mass function would give it.
- **Core collapse.** A cluster with no black holes left and an age above about 14 relaxation times
  is core-collapsed and takes a cusp of slope −1.6 to −2 in place of a core. That puts a fifth of
  the real catalogue over the line, as observed (Trager et al. 1995).
- **Recycled objects.** Millisecond pulsars, X-ray binaries and blue stragglers are marks within
  their classes, with expected counts from the encounter rate Γ ∝ ρ_c^1.5 r_c²: about 40 pulsars ×
  (Γ ÷ Γ of 47 Tucanae)^0.7, capped for core-collapsed clusters, and about 4,000 over the whole
  system.
- **Runaways.** About 15% of O stars are thrown out of their birth cluster by close encounters, and
  up to 38% from clusters near 10^3.5 M☉, where ejection peaks (Oh et al. 2015). A young cluster's
  living band-E count carries that factor.
- **Tails.** Every cluster, open or globular, carries its near debris as one more class: a straight
  tube along its orbit out to the reach of its grid. Older debris is field, or a
  [stream](#streams-and-accreted-structure).
- **Multiple populations.** Every globular born above about 10⁵ M☉ splits its members by an
  independent mark into a first and a second population (Milone and Marino 2022). The first's share
  is 0.62 − 0.30 × (log₁₀ M − 5), held between 0.1 and 0.7. A second-population member draws an
  enrichment, and its composition is linear in it: nitrogen and sodium up, oxygen and carbon down,
  aluminium up and magnesium down in massive metal-poor clusters, iron unchanged except in about a
  sixth of clusters, and helium up by as much as 0.18 in the most massive. The second population is
  more concentrated and has fewer binaries, so escapers, tails and streams favour the first.
- **Velocities.** The cluster's bulk motion plus an internal dispersion σ(r) ÷ √q.

Escapers need no population of their own, and that is exact and not an economy. The halo's, bulge's
and thick disc's shares are measured totals that already include cluster debris, and escapers start
from the same orbits as the field around them. So the populations are defined to include what was
born in clusters, and the clusters simply lack what they have lost. Evaporated stars come to about
an eighth of the halo, which matches the share of halo stars chemically traceable to globulars (Koch
et al. 2019).

Globular clusters are drawn as they are today, and their history is derived backwards. The
alternative, drawing them at birth and evolving them forwards, was tried against the real catalogue
and fails in outcome: it puts survivors at the wrong radii, with a turnover mass that varies where
the observed one is universal, and a quarter of the observed mass. So:

- **Number**: the dark halo's mass ÷ 6.5 × 10⁹ M☉, with 0.2 dex of scatter (Burkert and Forbes
  2020). About 40% are in situ and the rest belong to the accreted progenitors in proportion to
  their mass (Massari et al. 2019).
- **Mass**: an evolved Schechter function, (M + Δ)⁻² e^(−(M + Δ) ÷ M_c) with Δ = 2.0 × 10⁵ and M_c =
  1.07 × 10⁶ M☉, fitted to the catalogue. It is the same at every radius.
- **Place and size**: a cored r^−3.5 with a median radius of 5 kpc; a half-mass radius of 2.6 pc ×
  (R ÷ kpc)^0.41 with 0.21 dex of scatter.
- **Metallicity and age**: 30% metal-rich, near [Fe/H] = −0.55, in situ, flattened and rotating; 70%
  metal-poor, near −1.55. In-situ clusters are about 12.8 Gyr old, accreted ones 10.5–13.
- **History**: the initial mass by inverting M = 0.70 M₀ (1 − t ÷ t_dis) with Baumgardt and Makino's
  (2003) dissolution time, and the expansion from Gieles, Heggie and Zhao (2011).
- **Destroyed clusters** exist only as orphan streams and as the globular-born mark on field stars.

#### Supernova remnants: one route, not two

A shell is the consequence of a star's death, so a catalogue of shells and a stellar stage that
kills stars would describe the same events twice. A recently dead field star would have no shell,
and a catalogued shell no history. The realistic answer is that the shell belongs to the star, and
the catalogue is only how distant charts find it. The marking theorem for Poisson processes makes
that exact: split a Poisson process by a property of each point, which may depend on the point's
position and on its own independent draws, and the parts are independent Poisson processes.

- **The window is physics, not a number.** A shell is distinct until its shock slows to twice the
  signal speed of the gas around it. That time is a closed form in the explosion energy, the
  metallicity, and the gas density and pressure at the site (Cioffi, McKee and Bertschinger 1988,
  with the hot-gas branch of Tang and Wang 2005). Rarefied gas is hot, so the window is short both
  there and in dense gas, and longest, 0.5–1 Myr, in warm gas near 0.1–0.5 atoms per cubic
  centimetre. A pressure floor from the hot corona keeps the largest possible window finite, at 2–4
  Myr, which the oldest known remnant bears out (4.3 Myr, in the low-pressure outer Galaxy).
- **One shared test.** A layer-E system is a catalogue entry if and only if its primary's death
  falls between the window before now and the end of the clock window; the exact interval is under
  [Events in time](#events-in-time). The catalogue draws a candidate's place, mass and time of death
  under a cap, evaluates the window, and keeps the entry if the test passes. Layer E and the young
  [displaced](#displaced-objects-kicks-and-runaways) bins evaluate the same function on the same
  marks and resolve to "no such system" where the catalogue says yes. Nothing is counted twice and
  no death is nudged. The test reads only fields and the star's own marks, never a sibling feature,
  so the dependency graph stays acyclic. A shell's appearance may read nearby clouds at query time,
  as extinction does.
- **Where they are.** Four in five core collapses happen inside the superbubble of the association
  that bore the star (Higdon and Lingenfelter 2005). Those shells belong to the feature: its
  recently dead members are a class of its band E, listed at feature level so that charts find them
  from the feature catalogue alone. The feature carries the bubble, a Weaver-type radius from its
  count of O and B stars and its age, capped at blow-out, which is a hole in the gas field. A shell
  inside it expands into thin hot gas and is large, faint and gone in about 10⁵ years, often by
  reaching the bubble's wall. The remaining core collapses, from runaways and dissolved
  associations, are catalogue systems in field gas. Globular clusters have no shells. The galactic
  centre owns a handful from its young few per cent, as the real one owns Sgr A East and a magnetar.
- **The count is a result.** Across our ranges there are 3,000 to 30,000 distinct shells, about
  7,000 at the Milky Way's rates of about two core collapses and half a Type Ia per century (Li et
  al. 2011). Of those a thousand or two are still hot and bright in radio and X-rays, which is what
  the real catalogues estimate against the 310 actually found (Green 2025; Ranasinghe and Leahy
  2022). The rest are cool expanding shells of neutral hydrogen. Emission class follows phase: X-ray
  and radio while non-radiative, optical filaments while the shock is above about 70 km/s, the 21 cm
  shell to the end.
- **What is inside.** The entry is a system, and the stellar stage gives the rest: neutron star or
  black hole, pulsar, [kick](#displaced-objects-kicks-and-runaways). The shell's radius and phase
  follow in closed form from its age (free expansion, Sedov–Taylor, pressure-driven and
  momentum-conserving snowplough). The remnant sits at kick × age from the centre, typically 200 ly
  and up to a few thousand in field gas, so about half of field remnants have left their shell and
  trail a bow shock (van der Swaluw et al. 2003). A pulsar wind nebula lasts as long as spin-down
  says, 10⁴–10⁵ years, so most old shells have none.
- **Type Ia shells** are a class of layer D, exactly as recent core collapse is a class of layer E.
  Their rate is observed, not computed: the delay-time distribution, 1.3 × 10⁻³ events per solar
  mass formed, falling as t^−1.1 from 40 Myr (Maoz and Graur 2017), applied to each population's own
  history. That gives 0.4–1 a century, a fifth of all supernovae, but a third or more of the shells,
  because they explode in thin gas, often high above the disc, where a shell lasts longer. The
  binary formulae do not set the rate, which they are known to underpredict several times over
  (Claeys et al. 2014). They supply a pool of candidate events, mergers of white dwarfs and
  accreting white dwarfs that reach ignition, sub-Chandrasekhar ones included, and an independent
  explosion mark thins the pool to the observed rate. The observed merger rate is five to seven
  times the Ia rate (Maoz, Hallakoun and Badenes 2018), so about one pooled event in six explodes,
  and the rest stay what the formulae made them: massive white dwarfs, R Coronae Borealis stars, hot
  subdwarfs.
- **A Type Ia entry draws its delay first**, then the binary that has that delay: population, age,
  time of explosion, channel, the two masses, and for a merger the separation after the common
  envelope, solved from Peters's (1964) inspiral time so that lifetimes and inspiral add up to the
  delay. Every shell has a complete history from single-star lifetimes and one formula. Primaries
  are all of layer D: nothing under about 2.5 M☉ makes a heavy enough white dwarf in time. About 3%
  of layer D has exploded long ago and left nothing, and the grid's binaries, run forward, redraw on
  the same stream if they come out exploded.
- **What a Type Ia leaves.** By channel, as defaults of the generator version, since the science is
  unsettled: both white dwarfs destroyed, about half; a surviving donor flung out at 1,900–2,500
  km/s, about 30% (Shen et al. 2018); a hydrogen donor, puffed up and moving at 100–250 km/s, under
  5%; and the weak Iax events, about 10%, which leave a partly burnt white dwarf (Foley et al.
  2013). A recent survivor is a second member of the entry, at speed × age from the centre. Ancient
  hypervelocity survivors are unbound and cross the cube in about 10⁷ years, so some tens of
  thousands are inside at any time, as one more [displaced](#displaced-objects-kicks-and-runaways)
  class on straight lines.

#### Dense features: clusters and the galactic centre

The nuclear cluster and the cores of globular clusters are too dense for the grid. The fine layer's
index overflows at about 170 systems per cubic light-year, and long before that a cell-wide bound
makes the candidate counts absurd. The field itself stays finite at the centre, at about 0.3 per
cubic light-year for the bulge and about 18 for the nuclear disc of [Populations](#populations), so
it needs no cap and still knows nothing about the features. What was left unsolved was how the range
query finds the members near a ship without generating a million of them. **Lean:** each feature
with members carries a small nested grid of its own, in its own frame:

- Level j is a block of 16 × 16 × 16 cells of width w × 2ʲ centred on the feature. Its inner 8 × 8 ×
  8 cells are exactly the volume of level j − 1, which owns them, so each level is a shell around
  the last. Eight levels span a factor of 128 in cell size: with w = 0.5 ly, half-light-year cells
  in the core and 64 ly cells out to 512 ly. The width w is chosen per feature from its core radius,
  so that no cell expects more than a thousand or two members.
- Every cell is placed by the same thinning as the field, once per mass band, so a search for bright
  members skips the dwarfs here too. Profiles are spherical or flattened along an axis of the grid,
  fall with radius and have a finite core or an integrable cusp (Plummer or King, one per class; a
  cusp for core-collapsed clusters and the nuclear cluster). The feature's centre is a cell corner
  at every level, so the nearest-corner bound is exact.
- A cluster's density falls as r⁻³ or faster outside its core while cell volume grows eightfold per
  level, so the count per cell stays flat or falls going outward. A globular of 10⁶ systems with a
  240 per cubic light-year core peaks at about 600 members in a cell.
- The range query treats a feature as one more stack of layers: it visits the nested cells that
  touch the sphere, band by band. The expected count for the census decision is summed over those
  cells from their bounds. That errs high, which can only drop a layer early, and it is
  deterministic.

A member ID must carry all of that. As a sketch, under the reserved layer value:

| Field             | Bits | Notes                                                     |
| ----------------- | ---- | --------------------------------------------------------- |
| Layer             | 3    | The reserved value                                        |
| Kind              | 1    | 0: member of a catalogue feature. 1: see below            |
| Feature cell      | 15   | 5 bits per axis at 4,096 ly                               |
| Feature index     | 14   | Candidate number: 16,384 per feature cell                 |
| Mass band         | 3    | The same bands as the layers, and one spare value (below) |
| Level             | 3    | Eight nested levels                                       |
| Cell in the level | 12   | 4 bits per axis                                           |
| Index in the cell | 13   | Candidate number: 8,192 per cell and band                 |

That is all 64 bits. The band field's spare value marks members drawn at feature level and not in a
cell: a Poisson count on the feature's stream, with positions by inverse transform of the profile,
which is exact. Under it the level and cell are zero and the index counts the members. Index 0 is
member zero, such as a central black hole, and 1 upward are the feature's members of the
[catalogue classes](#events-in-time), such as its recently dead. A chart then finds every shell in
an association from the feature catalogue alone, where walking band E of each young feature would
cost tens of thousands of cell draws apiece.

The kind field is a prefix:

| Prefix | Meaning                                | Rest of the ID                                                                                                                                                                                                                                                                                                             |
| ------ | -------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `0`    | Member of a catalogue feature          | As above                                                                                                                                                                                                                                                                                                                   |
| `10`   | Member of a feature on the global list | Sub-kind (2): centre, stream or dwarf core. Then for the centre band (3), level (4), cell (15, at 5 bits per axis) and index (13); for a stream its number (12), band (3), cell along (14) and across (6 + 6), and index (13); for a dwarf core its number (2), then band, level, cell and index as in a catalogue feature |
| `110`  | Pinned content                         |                                                                                                                                                                                                                                                                                                                            |
| `111`  | Catalogue system                       | Class (6), cell (24, at 8 bits per axis of 512 ly; coarser classes zero the low bits), index (24), member (4)                                                                                                                                                                                                              |

The galactic centre is the first entry of the global list. Its central black hole is member zero.

Its nuclear cluster was the tight case, so it was worked through with the Milky Way's measured
profile: 2.5 × 10⁷ M☉ (Schödel et al. 2014), which is 4–5 × 10⁷ systems, on a broken power law with
an inner slope of 1.3 (Gallego-Cano et al. 2018), a break near 10 ly and an outer slope of 3.5. That
puts about 9,000 systems per cubic light-year at 3 ly from the black hole, matching the measured 1.5
× 10⁵ M☉ per cubic parsec. On a slope that shallow the count per cell rises with each level as far
as the break, and the 16-cell grid above fails: its fullest cell in the M dwarf band expects 14,000
candidates against an index of 8,192. Halving the cells fixes it. With 32 cells per axis the fullest
cell expects about 1,700 candidates, and the bound wastes only a few per cent of them.

An earlier draft softened the cusp into a core of 0.03 ly. That cannot stand. The cluster's
velocities must come from somewhere, and the only consistent source is a distribution function f(E),
found once per galaxy by Eddington inversion of the profile in the potential of the black hole and
the cluster. For a softened core that inversion goes negative inside 0.035 ly: no isotropic cluster
around a point mass can be shallower than r^−½. So the cusp continues inward to 10⁻³ ly, inside
which fewer than three systems are expected, then falls as r^−½, and ends at the loss cone: a member
whose pericentre would pass within about 2 au of the black hole is thinned out, which removes 4 ×
10⁻⁵ of the cluster. The density is defined as the integral of f, so positions and velocities agree
by construction. The grid becomes twelve levels, from cells of 1 ÷ 256 ly, a binary fraction so that
cell edges are exact, up to 8 ly and a reach of 128 ly, where the cluster has fallen well below the
nuclear disc around it. The innermost cell expects about a hundred candidates.

Three more things are true of the real cluster and copied:

- It is flattened to about 0.7 along z and it rotates. Both come from one mark on a member's orbital
  inclination i: accept with probability exp(−k sin² i), and turn a share of the retrograde orbits
  prograde. Inclination is an integral of the motion, so the cluster stays stationary. The density
  becomes the profile times a closed-form function of polar angle, k ≈ 0.84 gives the observed
  flattening, and the cell bound is the nearest corner's profile times that function's maximum.
- Its members are not coeval. They draw their ages from a distribution of the centre's own, mostly
  old with a young few per cent on an inner disc, as the real one has.
- Its dark remnants have profiles of their own, by the class device of
  [What is inside a cluster today](#what-is-inside-a-cluster-today). Black holes sink: a slope of
  1.75–2 with a break about half the stars' (Bahcall and Wolf 1976), which puts ten to forty
  thousand of them in the central parsec, as the X-ray sources there imply (Hailey et al. 2018).
  Retention is far from total: the escape speed is 1,100 km/s at 0.1 ly but only 210 at 10 ly, so
  the cluster keeps about a fifth of its neutron stars and about nine tenths of its black holes. The
  rest stay bound to the inner galaxy among the bulge's displaced remnants.

Two consequences for play. At 3 ly from the black hole the mean distance between systems is about
0.05 ly, so a chart there works in tenths and hundredths of a light-year and the census rule of
[The range query](#the-range-query) does the rest. And nothing here can be treated as standing
still; see [Orbits and time](#orbits-and-time).

#### Streams and accreted structure

A realistic halo is not a smooth power law. Nearly all of the Milky Way's is the debris of a handful
of accreted galaxies (Naidu et al. 2020). Most of that debris is phase-mixed, smooth in space and
lumpy only in velocity and chemistry, and some of it is still in cold streams.

**The halo is a marked mixture.** Its share stays about 1% and is split by independent marks into
components. Each smooth one is a cored, flattened or triaxial power law that never rises with |x|,
|y| or |z|, and the halo's bound is the sum of their nearest-corner values. The component is a
derived mark, as the population is, and it sets metallicity, α abundance, age and the velocity
ellipsoid:

| Component                          | Share of the halo | Form                                                                                                         |
| ---------------------------------- | ----------------- | ------------------------------------------------------------------------------------------------------------ |
| In situ, the heated early disc     | 15–30%            | Flattened to about 0.5, inside about 50,000 ly, prograde, [Fe/H] near −0.6                                   |
| The dominant ancient merger        | 35–60%            | Strongly radial orbits, no net rotation, [Fe/H] near −1.2, a break where its stars pile up at apocentre      |
| Two to five lesser old progenitors | 10–25% together   | Each with its own net rotation, metallicity and age                                                          |
| Globular-born debris               | 8–15%             | Steeper inward; up to a third carry second-population chemistry, so nitrogen-rich stars are 2–4% of the halo |
| Discrete: streams and dwarf cores  | 2–15%             | Below                                                                                                        |

**A stream is a tube, and its track is not an orbit.** The obvious scheme, a tube around the
progenitor's orbit, was tested with a particle-spray model and works only for short streams in the
outer halo. For a typical inner globular the stars leave the orbit by tens of tidal radii within one
wrap, which is the known misalignment of streams and orbits. So the track is measured. Once per
stream, lazily, as a pure function of seed and stream number, about 2,000 tracers are released from
the progenitor's Lagrange points (Fardal et al. 2015) and integrated in the tabulated potential with
a fixed-step leapfrog and `libm`. They are ordered by the orbital angle each has gained on the
progenitor, which is monotone and survives wraps, and reduced to a tube table of about 256 knots per
arm: centre line, local frame, mean velocity, two widths, three velocity dispersions and the
cumulative line density. The density comes from weighting tracers by the mass-loss rate at release,
which gives leading and trailing arms, the pile-up at apocentre, and a gap where a dissolved
progenitor used to be. A mean-preserving lattice noise along the track supplies the observed gaps.
The table is a cache, about 0.2 s and 80 kB to rebuild, and is never stored.

- Members are a Poisson process in the tube's own coordinates: along, and two across. A cell's count
  has a closed-form mean, and positions come by inverse transform inside the cell, so nothing is
  thinned and no bound is needed. Mapping the points to space keeps the process Poisson whatever the
  curvature. The tube is cut at four widths, and members outside the root cube are dropped.
- Band shares are what the cluster lost: the evolved mass function minus the cluster's present one.
  So streams are bottom-heavy, hold white dwarfs but no neutron stars or black holes, and are mostly
  first-population stars.
- Only cold debris gets a tube. Stripping is counted back to the wrap time or to the last major
  merger, whichever is shorter, and older debris is the smooth field. A progenitor whose pericentre
  lies inside the bar's corotation gets none, because its debris fans out. Only about a fifth of the
  Milky Way's globulars qualify.
- Globular streams number about 1.5 per globular, most of them orphans whose cluster is gone: 10³–
  10⁵ M☉, 30,000–160,000 ly long, 100–400 ly wide, with a dispersion of 0.5–3 km/s. On its axis such
  a stream is 25–45 times the smooth halo around it and a thousandth of the disc's reference
  density. The multiplier is a parameter of the generator version. The known census is over 120
  streams and far from complete (Bonaca and Price-Whelan 2025).
- Dwarf streams come from the handful of progenitors accreted in the last 6 Gyr or so, with stellar
  masses drawn from M^−1.45 over 10⁵–10⁹·⁵ M☉: several wraps, 1,000–6,000 ly wide, 10–25 km/s. About
  one galaxy in five has one as large as Sagittarius.
- A **dwarf core** is what is left of such a progenitor inside the cube: a feature with a nested
  grid of 16–64 ly cells, members that are not coeval, and a metallicity from the dwarfs' mass–
  metallicity relation.
- A query tests the global list's bounding shells, then a stream's small spatial hash of knots, then
  its cells. A 50 ly query on a globular stream generates about 160 members to find two.
- A member moves at the tube's mean velocity plus its dispersion, and drifts like everything else.
  Over a thousand years it travels half a light-year along a tube a hundred wide.

The disc needs no tubes. An open cluster's tails are the tail class in its own grid, older debris is
field at a thousandth of the local density, and the moving groups of the solar neighbourhood are
structure in velocity, which belongs to the velocity draw.

### Placing star systems

The obvious approach, a uniform grid with a Poisson-distributed count per cell, has a flaw: to find
the bright stars within 5,000 ly (for a long-range chart, or the sky as seen from the ship) you
would have to generate every red dwarf in that volume, hundreds of millions of systems.

**Lean:** adopt Stellar Forge's layering. There are several layers of cell size, and each layer owns
a band of primary-star mass. The coarsest layers hold the rare O and B stars and what they leave
behind; the finest holds the M dwarfs. (Stellar Forge calls this an octree and this document follows
it, but nothing is subdivided: it is a stack of independent grids, each twice as coarse as the
last.) For each cell of each layer, in principle:

1. Expected count λ = the density field for that layer's mass band, integrated over the cell.
2. Actual count from a Poisson draw on the cell's stream.
3. Positions drawn inside the cell, following the field where density varies across the cell.

In practice the integral and step 3 are replaced by thinning, which gives the same distribution
without either. That, and how many layers of what size, are worked through in
[Galactic structure in detail](#galactic-structure-in-detail).

Poisson counts in disjoint cells are independent, so no cell needs to know about its neighbours, and
the galaxy-wide totals come out right without a global mass budget. A query for "everything brighter
than magnitude X within R" walks only the layers that can contain such stars. The same structure
serves gameplay: bright stars are charted from afar, dim ones are discovered by going there.

Near the Sun the density is about 0.0023 systems per cubic light-year (0.003 counting individual
stars), so an 8 ly cube holds about one. At the centre of the bulge it is about a hundred times
higher, about 0.3 per cubic light-year, and the nuclear disc adds about 18 more in the innermost few
hundred light-years. The index absorbs that at every layer (see [Identifiers](#identifiers)). The
nuclear cluster does not fit: it averages some 10⁵ stars per cubic parsec over its central few
parsecs and passes 10⁶ in the innermost half parsec. It is a feature with a grid of its own; see
[Dense features](#dense-features-clusters-and-the-galactic-centre).

### Systems and stars

- **Primary mass** from an initial mass function, within the layer's band, up to a limit of 150 M☉.
  About three quarters of stars come out as M dwarfs, which is correct and should not be "fixed" to
  make the galaxy more colourful. Two functions are supported behind one interface: Kroupa's and
  Chabrier's system function. See [Sizing the layers](#sizing-the-layers).
- **Multiplicity** depends on primary mass: roughly a quarter of M dwarfs, nearly half of Sun-like
  stars and most O and B stars have companions. Companion mass ratios and orbital periods are drawn
  from the observed distributions (periods log-normal, peaking near 10⁵ days for Sun-like
  primaries). Triples and higher are hierarchical, which keeps them stable by construction.
- **Stellar state** follows from initial mass, age and metallicity. If the age exceeds the star's
  lifetime it is a remnant: a white dwarf, neutron star or black hole according to initial mass,
  with the remnant's mass from an initial-to-final mass relation. Otherwise its phase (main
  sequence, subgiant, giant and so on), luminosity, radius, temperature and spectral class come from
  stellar evolution fits.

#### Covering every class of star

The stellar stage has to be comprehensive (see [Decisions](#decisions)). Every kind of star an
astronomer would name must be able to fall out of it, at the right frequency, from mass, age,
metallicity and, for some, a companion. Nothing is rolled as "this one is a Wolf-Rayet star".

For the backbone there are two candidates. The analytic formulae of Hurley, Pols and Tout (2000)
give every phase for any mass and metallicity as closed-form code: main sequence, Hertzsprung gap,
red giant branch, core helium burning, both asymptotic giant phases, naked helium stars, and the
three kinds of white dwarf, neutron stars and black holes. Interpolating a bundled grid of modern
tracks (MIST) is more accurate but means shipping and versioning a data table, and it stops short of
the remnants. **Lean:** the analytic fits. Coverage is a matter of phases and classes, not of track
precision, their error is far below what a player could detect, and they are pure code. They are
valid from 0.1 to 100 M☉ and for metallicities Z from 0.0001 to 0.03, and they say nothing about
spectra. So the backbone needs these around it:

| Gap                        | Treatment                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| -------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Below 0.1 M☉               | Cooling fits for the latest M dwarfs and the brown dwarfs, giving luminosity and temperature from mass and age (Burrows et al. 2001, checked against Baraffe et al. 2015). These supply classes L, T and Y.                                                                                                                                                                                                                                                                                                                                                                         |
| 100–150 M☉                 | An extension of the fits checked against published models of very massive stars. Only about a thousand such stars are alive at once.                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Before the main sequence   | Age zero is the onset of collapse, and the first 0.5 Myr is the protostar phase (Class 0 and I), with a closed form for the growth in mass; about a million protostars exist at once. Then contraction tracks ahead of the zero-age main sequence. The young population is under 100 Myr old and a 0.2 M☉ star takes several times that to arrive, so most young dwarfs are still contracting: T Tauri and Herbig Ae/Be stars.                                                                                                                                                      |
| Winds and remnant masses   | The prescriptions current population-synthesis codes use in place of the originals, which are dated at high mass: Vink et al. (2001) for hot-star winds and Mandel and Müller (2020) for neutron star and black hole masses and kicks, with windows for electron capture 0.1 M☉ wide in single stars and about 1 M☉ in stripped ones.                                                                                                                                                                                                                                               |
| White dwarfs               | Temperature and luminosity from cooling age. Spectral type (DA, DB, DC, DO, DQ, DZ) from an atmosphere draw whose odds depend on temperature.                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| Neutron stars              | Birth spin and magnetic field are drawn, and spin-down is a closed form in age. That gives the pulse period, whether the pulsar is still alive, the magnetars, and with a beam direction whether it is seen from a given place.                                                                                                                                                                                                                                                                                                                                                     |
| Black holes                | Mass and spin. Dark unless something feeds them.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| Classification             | Temperature, luminosity, gravity and surface composition map to a spectral type (O to M with subclass, then L, T, Y) and a luminosity class (Ia⁺ to V, subdwarf, white dwarf), calibrated on Pecaut and Mamajek (2013).                                                                                                                                                                                                                                                                                                                                                             |
| Classes beyond the MK grid | Wolf-Rayet types (WN, WC, WO) from how far a helium star is stripped. Luminous blue variables near the Humphreys–Davidson limit. Carbon and S stars on the late asymptotic giant branch over the mass range where dredge-up works.                                                                                                                                                                                                                                                                                                                                                  |
| Variability                | Derived, not rolled. A star inside the instability strip pulsates (classical and type II Cepheids, RR Lyrae, δ Scuti) with a period from its mean density. Late giants are Miras and semiregulars. Cycle-to-cycle irregularity is keyed by cycle number. Flares, outbursts and glitches are [events in time](#events-in-time) on the star's own streams.                                                                                                                                                                                                                            |
| Rotation and magnetism     | One draw each, giving Be stars, the chemically peculiar Ap and Am stars, and activity levels.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| Interacting binaries       | The companion formulae of Hurley, Tout and Pols (2002), run forward once for each binary close enough to interact: blue stragglers, hot subdwarfs, cataclysmic variables, X-ray binaries, millisecond pulsars, symbiotic stars, Type Ia progenitors. The formulae run forward conditional on the system's class: explosion as a Type Ia is a thinning mark calibrated offline to the observed rate (see [Supernova remnants](#supernova-remnants-one-route-not-two)), and the hosts of rare events are [catalogue classes](#events-in-time) with conditional samplers of their own. |
| Helium                     | The fits fix helium by metallicity. Second-population members of globular clusters carry up to 0.18 more, which shortens lifetimes and sets the blue end of the horizontal branch, so it enters as a correction fitted offline.                                                                                                                                                                                                                                                                                                                                                     |

Single stars come first and the interacting binaries last, since they need multiplicity. Mass
transfer has one consequence for the layers: a merger of two layer-B dwarfs can outshine its band,
so "layers A and B are reliably dim" gains a rare exception once binaries evolve.

Statistical tests compare class fractions by population against observed ones, and a
Hertzsprung–Russell diagram of a sample is the quickest check by eye.

### Planetary systems

Three approaches:

| Approach                                                                                                                                       | For                                                                                                       | Against                                                                                                                                                                                                         |
| ---------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **A. Statistical.** Draw planets directly from exoplanet occurrence rates.                                                                     | Fast. Matches what has been observed.                                                                     | Transit surveys are blind beyond about 1 au and below Earth size, and other methods reach only the giants farther out, so the small cold planets would be guesswork. Independent draws give incoherent systems. |
| **B. Formation simulation.** Accrete planets from a disc of dust and gas, in the line of Dole's ACCRETE (1970) and StarGen.                    | Systems are coherent because they grew together. Milliseconds per system.                                 | Reproduces the Solar System and little else: no migration, so no hot Jupiters and no compact chains of super-Earths, which are the commonest kind known.                                                        |
| **C. Hybrid.** Derive a disc from the star, choose an architecture class from observed frequencies, place planets under dynamical constraints. | Coherent and diverse. Every constraint is a testable rule. Extends cleanly as exoplanet science improves. | More design work: the architecture classes and their frequencies are ours to define and defend.                                                                                                                 |

**Lean: C.** In outline:

1. **Disc.** Mass scales with stellar mass, solid content with metallicity. The snow line sits near
   2.7 au × √(L/L☉). In binaries, planets are allowed only in the dynamically stable zones: close
   around one star or wide around both (Holman and Wiegert 1999).
2. **Architecture class.** For example: no planets, compact multi-planet, Solar-like, dominated by
   an eccentric giant, hot Jupiter with few companions. Class frequencies depend on stellar mass and
   metallicity: giant-planet occurrence rises roughly as 10^(2[Fe/H]), and M dwarfs rarely host
   giants but often host compact chains.
3. **Placement.** Spacing between neighbours drawn in mutual Hill radii, centred on the observed
   14–20 and rejected below the long-term stability floor of about 10–12. Adjacent planets have
   correlated sizes (the "peas in a pod" pattern). Rocky inside the snow line, ice and gas giants
   beyond it, except where the class implies migration.
4. **Derivation.** Everything else is computed, not rolled: radius from mass (Chen and Kipping 2017,
   refined by composition with Zeng et al. 2019), equilibrium temperature, whether an atmosphere
   survives thermal escape, greenhouse warming, tidal locking timescale against system age, the
   habitable zone (Kopparapu et al. 2013), Roche limits for rings, Hill spheres bounding moon
   orbits.
5. **Small bodies.** Moons (regular satellites scaled to the planet's mass, captured irregulars, the
   occasional giant-impact moon), asteroid belts at resonances with giants, a Kuiper-like belt, a
   cometary halo as a statistical population.

Free-floating planets and brown dwarfs are "systems" with no star, and reuse this stage for their
moons and bulk properties. See [Between the stars](#between-the-stars).

### Between the stars

With free travel in 3D a ship can stop anywhere, so the space between systems needs content (see
[Decisions](#decisions)). None of it moves an existing star, because every layer and field draws
from its own streams.

- **Isolated remnants** are already there. A grid system whose primary has died is a white dwarf,
  neutron star or black hole, alone or with whatever survived. Layer E alone is born with some 10⁹
  of them, which matches estimates for the Milky Way, and keeps about four fifths once kicks have
  had their say. They are dark, so they are found by going there or by what they do to their
  surroundings. Most neutron stars are not where they were born; see
  [Displaced objects](#displaced-objects-kicks-and-runaways).
- **Brown dwarfs** get a layer of their own: 13–80 Jupiter masses, 16 ly cells, the same populations
  and ages as the stars. At one free-floating brown dwarf for every five or six stars, a cell holds
  two or three at the reference density. Their state comes from the cooling fits of the stellar
  stage.
- **Rogue planets** get the last layer, from a third of an Earth mass up to 13 Jupiter masses. How
  many exist is the least certain number in this document, so it follows the best measurement and
  not convenience: about 21 per star, with an uncertainty of a factor of two either way, on a mass
  function falling nearly as 1 ÷ mass (Sumi et al. 2023), which also keeps Jupiters under one for
  every four stars (Mróz et al. 2017). Nearly all of them are smaller than Neptune. That is about 26
  per system, and they follow the stars, since the measurement is made towards the bulge. In 8 ly
  cells the galactic centre would then hold about 490 per cubic light-year, and up to 700 for the
  densest seed, against an index limit of 128. So the layer uses 4 ly cells and the ID's spare bits
  (see [Identifiers](#identifiers)): five to a cell at the reference density, and a limit of 1,024
  per cubic light-year. The abundance stays a parameter of the generator version, capped by that
  limit at about 38 per star for Milky Way values and 27 for the densest seed, and the range query
  never walks this layer unless asked.
- **Dust and gas** are a field, not objects. Three smooth components, all needed once supernova
  shells read the gas they expand into: a thin neutral disc a few hundred light-years tall with a
  hole inside the bar, a warm ionised layer of about 0.03 atoms per cubic centimetre with a scale
  height near 3,000 ly, and a hot corona of about 10⁻³. With them goes a closed-form pressure,
  falling with height to the corona's floor. The neutral disc has a small dense disc of molecular
  gas at its centre, where the nuclear disc is, and is gathered into lanes along the inner edges of
  the arms by reusing the arm geometry with a phase offset. All of it is broken up by a few octaves
  of lattice noise under a mean-preserving log-normal, wide enough (σ of 2–2.5 in the logarithm)
  that the volume runs from hot rarefied gas, a fifth to two fifths of the plane, to cloud. The
  superbubbles of young [features](#large-features) are holes in it. The noise hashes integer
  lattice points and interpolates with exact arithmetic, so it is reproducible. Its integral along a
  line of sight is extinction, about one magnitude per 3,000 ly in the plane, which sets how far an
  optical sensor sees, dims and reddens stars on the consoles, and puts the dark lanes into the
  [galaxy map](#visualiser). Stars never read it, with one exception: a massive star's supernova
  shell reads the gas at its site, smoothed to the shell's own scale (see
  [Supernova remnants](#supernova-remnants-one-route-not-two)). Stars share the arm parameters with
  it, which is what makes young stars and dust coincide. Molecular clouds and nebulae are
  [large features](#large-features) that add their own dust locally. The cost to watch is the line
  integral, which is numerical.
- **What dust and gas do** is what physics says and nothing invented (see [Decisions](#decisions)):
  - Extinction depends on wavelength (Cardelli, Clayton and Mathis 1989). At 2 µm it is about a
    ninth of its visual value, and radio is untouched. So each sensor band has its own horizon. The
    Milky Way's centre lies behind some thirty magnitudes in visible light and about three in the
    infrared, which is why astronomers study it there.
  - It works both ways. A ship inside or behind a cloud is hidden from optical sensors exactly as
    much as the stars behind it are, and an infrared or radio sensor sees it regardless.
  - The field gives gas as well as dust: a hydrogen column of about 2 × 10²¹ atoms per square
    centimetre for each magnitude of visual extinction (Bohlin, Savage and Drake 1978), with the
    dust-to-gas ratio following metallicity. That is about one atom per cubic centimetre in the disc
    and 10² to 10⁶ inside a molecular cloud. For a ship at a large fraction of light speed that gas
    is a radiation and erosion load proportional to density, so a dense cloud is a real hazard below
    light speed.
  - Gas glows where something excites it: H II regions around hot stars, shells, the 21 cm line
    everywhere. The emission classes of the features carry this.
  - Nothing here says what a faster-than-light drive does in gas, because no physics does. That
    belongs to the drive's brainstorm.
- **Interstellar comets and asteroids** are far too numerous to index, perhaps one for every ten
  cubic astronomical units. They are a statistical density, made real only in a ship's immediate
  surroundings if a feature ever needs them.

The mass floor of the range query gains two steps below 0.08 M☉ for the substellar layers.

### Orbits and time

Every orbit is a set of Keplerian elements, and a body's position is a pure function of time. No
N-body integration: systems are stable by construction and on rails. A system nobody is visiting
therefore costs nothing, and returning after a year shows every body where it should be.

The fields are a snapshot at the epoch, but systems move. An earlier draft froze them, on the ground
that a star covers only 0.0007 ly a year. That does not survive the consoles. A range read to 0.001
ly goes stale in six years, a fast neighbour moves 190 au in a decade, and a galaxy that reports
velocities while nothing moves contradicts itself. Motion takes two closed forms, each a pure
function of seed, ID and time:

- **Every system drifts in a straight line** from its epoch position at its own velocity. Over a
  century the neglected curvature is under 10⁻⁴ of a tidal radius everywhere outside the central few
  light-years. Moving every point of a Poisson process independently leaves it a Poisson process, so
  placement is untouched.
- **Within the central black hole's sphere of influence**, about 10 ly, where it outweighs the stars
  inside, a system follows the Kepler orbit about the black hole and the cluster mass inside its own
  radius, with the relativistic advance of pericentre, and the precession from the enclosed stars,
  added as secular rates. That covers the centre's members and grid systems alike, and it passes
  smoothly into drift further out. Straight lines fail here by more than a tidal radius a century at
  1 ly and would empty the cusp. Velocities come from the cluster's distribution function, which
  Kepler motion preserves, so the density is exactly the profile at every time, past or future, and
  the thinning bound holds at all times. A Monte Carlo confirms it over a thousand years each way.
  Eccentricities come out thermal, as the S-stars' are, and the orbit of S2 is reproduced: a 16-year
  period, 120 au at pericentre, 7,800 km/s.

Velocities are closed-form in the potential tables, per population:

| Population           | Velocity                                                                                                                                                                                                                                                                                                                                                |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Discs                | Vertical dispersion from the vertical Jeans equation and the population's own scale height, so it falls outward with the disc. Radial dispersion is that ÷ 0.5–0.6, and the azimuthal follows from κ² ÷ 4Ω². The mean lags the circular speed by the asymmetric drift, about σ_R² ÷ 80 km/s.                                                            |
| Young disc           | The same, with a floor of 5 km/s from the turbulence of the gas, plus streaming of 5–15 km/s along the arms as a closed form of the arm phase.                                                                                                                                                                                                          |
| Thick disc           | About (65, 40, 35) km/s, lagging by about 50.                                                                                                                                                                                                                                                                                                           |
| Halo                 | Per [component](#streams-and-accreted-structure): radial orbits and no rotation for the dominant merger, mild rotation for the in-situ part. The mixture averages an anisotropy near 0.6 and a radial dispersion of about 145 km/s against 141 measured.                                                                                                |
| Bulge and bar        | Rotation at the bar's pattern speed plus streaming along the density's own ellipses, which satisfies continuity exactly and rotates cylindrically by construction. Dispersions from an axisymmetric Jeans solution tabulated once: 160–215 km/s intrinsic near the centre, which projects to the 116–134 km/s that surveys measure at a latitude of 1°. |
| Nuclear disc         | Rotation of about 100 km/s and a dispersion of about 70, falling outward (Sormani et al. 2022).                                                                                                                                                                                                                                                         |
| Nuclear cluster      | The distribution function above: a dispersion rising as r^−½ inside about 3 ly, to 500 km/s at 0.1 ly.                                                                                                                                                                                                                                                  |
| Features and streams | The feature's bulk motion plus an internal dispersion; a stream's mean velocity along its tube.                                                                                                                                                                                                                                                         |

Draws are cut off at the local escape speed.
[Kicked remnants and runaway stars](#displaced-objects-kicks-and-runaways) draw theirs from the
class that placed them.

The disc's velocities force a change to its structure. One scale height for the whole old thin disc
contradicts the observed heating of stars with age, σ_z = 22 km/s × (age ÷ 10 Gyr)^0.44 (Sharma et
al. 2021). So the old thin disc is a set of about five discs by age, as in the Besançon model, each
with the scale height at which the Jeans equation returns its dispersion: from about 320 ly at half
a gigayear to 1,700 ly at ten. A system's age, height and vertical speed are then correlated, as
they are in reality. See [Populations](#populations).

**What a sensor sees is the past.** Every reading of an object at distance d is its state at the
retarded time t − d ÷ c. In a model where everything is a function of time that costs one
evaluation. A supernova 10,000 ly away is not seen for 10,000 years, an observer at 26,000 ly sees
hundreds of supergiants that are already dead, and a ship that outruns light can watch an event
again from further off. No contradiction can arise in play, because the model is linear: an observed
position and velocity, extrapolated by the navigation computer and jumped to, land exactly on the
star. Charts therefore show present positions, and sensors show retarded state and apparent
position. Spatial searches always use present positions. Retarded evaluation is defined back to the
start of the source horizon, and its error from neglected curvature is stated: under half an
arcsecond for a disc star seen from across the galaxy, and up to 13 for the nuclear disc.

These are continuous functions of time and need no events: orbital motion, rotation and pulse phase,
pulsation, eclipses and transits, precession, spin-down, cooling, the expansion of shells, inspiral
by gravitational radiation, drift, and stellar evolution itself. Microlensing is a ray walked at
query time along one monitored line of sight over real moving objects. A survey of lensing over the
whole sky is not offered, because its lenses could not be real objects.

### Events in time

A galaxy in which nothing happens during a campaign is not realistic. The Milky Way has some 30–50
novae a year, a supernova every few decades, and countless flares, outbursts and glitches. All of it
is generated as a pure function of seed, ID and time, without stored state.

**Evolution is free.** A system's state is its evolution evaluated at age plus clock time, so a star
that leaves the main sequence, sheds a planetary nebula or spins down during play simply does. The
stellar stage must therefore be continuous in age, with no tables binned by age. A death is a clock
time, T = lifetime − age at the epoch.

**Star formation continues.** Every population still forming stars draws ages from −H, so systems
not yet born at the epoch exist in the process: 1,300–8,400 of them, at one to eight births a year.
Such an ID resolves at all times, its state before birth is "no system yet", and the range query
filters on age plus time being positive.

**Rare events are found by carving out their hosts, not the events.** By the marking theorem, the
systems whose marks put them in a class are an independent Poisson process, which can be placed on
its own coarse grid where distant charts and alerts can find it, while the cells draw conditional on
not being in the class. A class is a deterministic test on a candidate's own marks, evaluated
identically on both sides, so nothing is counted twice. Carving events into bins of space and time
would be equivalent, but would give a recurrent host a different identity in each bin, and a nova's
previous eruption can be observed again from 10⁴ ly further away. The classes:

| Class                     | Hosts                            | Note                                                                                |
| ------------------------- | -------------------------------- | ----------------------------------------------------------------------------------- |
| Core collapse and Type Ia | Death within the interval below  | Some 3,000–21,000 more entries than there are shells: progenitors and bare remnants |
| Stellar mergers           | Merger within the source horizon | Luminous red novae, 0.2–0.5 a year (Kochanek et al. 2014)                           |
| Neutron-star mergers      | The same                         | About 10 entries                                                                    |
| Luminous blue variables   | All                              | Giant eruptions                                                                     |
| X-ray binaries            | All, about 10⁴                   | Needed anyway as X-ray beacons; about 1,300 are black-hole transients               |
| Accreting white dwarfs    | All, 6–12 × 10⁶                  | From the measured local density (Pala et al. 2020); novae and dwarf novae           |

Conditional samplers for each class are offline tables, as for Type Ia. Inside a feature the same
classes go on its feature-level list. Magnetars and young glitching pulsars are already inside the
supernova class.

**The supernova test with a clock.** A system is a catalogue entry if and only if its death T lies
between −(W + L + H) and +H, where W is its shell's window. Membership does not depend on the time
of the query, so there is one ID on both sides of the explosion. What the entry is depends on the
time it is evaluated at, which for a sensor is the retarded time:

| Evaluated    | The entry is                                                                                                                                    |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| Before T     | The living progenitor. For a Type Ia by merger, two white dwarfs spiralling together, 80–100 s apart in period a thousand years before the end. |
| From T       | A supernova of that age: neutrino burst, shock breakout, a light curve by type, then the shell at its radius and the remnant at kick × age.     |
| Beyond T + W | The shell has merged with the gas. The entry remains as the bare remnant.                                                                       |

The thinning cap grows by L + 2H, and the entry's lifetime is held to 4,096 ly ÷ its fastest
member's speed, so that lookups stay within a known number of rings of cells. Beyond the clock
window nothing breaks locally: a grid star due to die in 1,500 years still dies in place, with its
shell. Only finding it from afar lapses. Within ±H a galaxy has 20–160 core collapses and 8–20 Type
Ia.

**A system's own events** use two constructions, both exact and open to random access:

- **Poisson bins**, for memoryless events. Time is cut into bins, and the count and times of events
  in bin k are a pure function of (seed, ID, tag, k). A rate that varies is handled by thinning
  within the bin, and durations by looking back a fixed number of bins. Flares on M dwarfs and
  Sun-like stars, glitches of Crab-like pulsars, magnetar bursts and giant flares, FU Orionis
  outbursts (one per 5,000 years for the youngest protostars, one per 112,000 later), giant
  eruptions of luminous blue variables, and the flares of the central black hole, about one a day.
- **A monotone phase**, for cycles of accumulation and release. An event falls where t ÷ P plus a
  bounded-slope noise crosses an integer, which keeps events in order while letting the phase
  wander, with an optional skip mark for strongly irregular sources. A plain jittered lattice is
  wrong, because its phase never diffuses. Classical and recurrent novae, with P from the ignition
  mass and the accretion rate, dwarf novae, X-ray transients and bursts, Vela-like glitches, and
  thermal pulses on the asymptotic giant branch.

No event may leave a mark that needs history replayed: a cumulative effect must be a closed form in
the event number, as a pulsar's spin-down already includes its mean glitch activity, or vanish
within a bin.

**The centre.** Tidal disruptions are a Poisson process on the black hole's own stream, about one
per 10⁴ years for the Milky Way's black hole (Stone and Metzger 2016), separate from the Kepler
members, whose loss cone is empty by construction. The victim is a list member on a near-parabolic
orbit. The black hole's luminosity is quiescence plus flares plus the t^−5/3 tails of recent
disruptions, still 10³⁹ erg/s a thousand years on, which matches the light echoes seen around Sgr
A\* (Ponti et al. 2010).

**Alerts.** "What went off?" walks the catalogue classes within the sensor's horizon and evaluates
each host over the retarded interval, under the census rule: complete per class or nothing. A
galaxy-wide scan of the accreting white dwarfs takes seconds cold, so it runs in the background and
is cached, like the density map. Flares, dwarf novae and glitches are local and come with the range
query. An alert reaches a console through the knowledge overlay, first as a bearing and a flux and
only later as a resolved host.

### Hooks for the layers above

Each body ends with what later systems need and no more: a surface seed, bulk composition, surface
conditions, a habitability assessment and resource abundances tied to composition and metallicity.
The surface, life and civilisation generators consume these and do not reach back into this
pipeline.

Surfaces stop at orbital scale for now (see [Decisions](#decisions)): what a crew sees from orbit
and through sensors. That means global maps at a resolution of kilometres (terrain class, ice, ocean
and cloud cover, the large craters, basins and volcanic provinces), plus composition and atmosphere.
The body's global figures, such as ocean fraction, relief and crater density from surface age, are
derived here, and the map generator turns them and the surface seed into pictures. Anything a
landing party would need waits for a feature that requires it.

## Overlays and persistence

The generated galaxy is immutable. Everything else is a layer on top of it, stored by ID:

- **Pinned content.** Hand-authored or top-down generated objects (a starting system, faction
  homeworlds) that replace or suppress procedural output in their volume, as Elite overlays real
  catalogue stars.
- **Deltas.** Anything play changes: a mined-out asteroid, a destroyed station.
- **Enrichment.** LLM-written descriptions are not deterministic, so they are generated once,
  stored, and never feed back into simulation values. They live in `hyperion-server`, not the sim.
- **Knowledge.** What the crew has actually detected, separate from what is true, and as old as the
  light that brought it. The server holds the truth; consoles receive only what sensors have
  resolved. Generation should make it easy to degrade a body's record to "unresolved contact", "mass
  and orbit only" and so on.

One tension to flag now: civilisations and history want to be generated top-down over the whole
galaxy, but the galaxy is only ever generated lazily and locally. The likely answer is that history
runs over a coarse summary (habitable-world density per large cell, from the fields) and emits
pinned content. That is for the civilisation brainstorm, but it is why overlays are part of the core
design and not an afterthought.

## Runtime and code shape

- Generation is lazy with bounded LRU caches per level (cells, systems). Eviction is always safe.
  The caches are bounded by bytes, not entries, because a bulge cell is ten thousand times heavier
  than a rim cell, and they hold only accepted systems, as state at the epoch and never as positions
  at some time. The potential tables, the streams' tube tables and the alert scans are caches of the
  same kind, rebuilt on demand. The caller owns them: the generators stay pure functions, so the
  server can call them from a thread pool with whatever cache it likes.
- `hyperion-sim` spawns no threads and does no I/O. Pure generators can still be called from any
  thread the server chooses.
- Targets to validate by benchmark, not promises: placing the systems of a sparse fine cell in 1–2
  µs, a full system with its bodies in under a millisecond, and a 50 ly neighbourhood query at
  Sun-like density in under 5 ms cold. The query touches about 1,400 fine cells, so the first and
  last targets stand or fall together.
- **Lean:** begin as modules inside `hyperion-sim` (`rng`, `units`, `galaxy`, `stellar`,
  `planetary`), with an exactly pinned `libm` as the only new dependency. Split into a
  `hyperion-galaxy` crate only if compile times or reuse demand it.
- The protocol gains query and result types as displays need them. The first are those of the
  [Visualiser](#visualiser); system summary and body detail follow with the stellar and planetary
  stages.

### Testing

- **Golden tests.** Pinned seeds and IDs with exact expected output, guarding determinism and the
  generator version.
- **Statistical tests.** Generate a large sample and assert the distributions: spectral class
  fractions, multiplicity by mass, planets per star, density against the field. This is how
  "realistic" becomes a test and not an opinion.
- **Property tests.** For any seed and ID: no overlapping orbits, moons inside Hill spheres, rings
  inside Roche limits, no main-sequence star older than its lifetime, no planet hotter than its
  star.
- **Order independence.** Generating A then B equals generating B then A equals generating B alone.
- **Bound checks.** A debug assertion that the density never exceeds the thinning bound, and a test
  that hunts for violations along arm ridges near the bar, where they are likeliest.
- **Complementarity.** For sampled candidates, exactly one of the cell and the catalogue claims the
  system, on both sides of a death and across the clock window. An assertion that no shell window
  exceeds its cap.
- **Time.** State is continuous in time. Both event constructions return the same events in any
  order of asking. Members of the nuclear cluster keep its profile at ±1,000 years, and stream
  members stay in their tubes.
- **The kick law**: the speeds of young unbound pulsars against the log-normal, with 5 ± 2% below 50
  km/s across the sky; a low-mode share of 20 ± 10%; retention of at least a tenth in a cluster with
  a birth escape speed of 50 km/s; most double neutron stars at low eccentricity; at least half of
  black holes unkicked.
- **Against the Milky Way**, at its parameters: enclosed mass from 1 pc to 2 kpc, the shape of the
  bulge's dispersion profile, the ratio of rotation speed at 1 and 8 kpc, the globular system's mass
  function, sizes and orbits, the counts of supernova shells by phase, and the rates of events.
- **Budgets.** Field plus features plus streams equals each population's budget in expectation.
- **By eye.** Much of the tuning will be visual. The `GALAXY` display under
  [Visualiser](#visualiser) serves for that, so there is no separate developer viewer.

## Galactic structure in detail

The first milestone is the top of the pipeline: parameters, fields, placement and the range query,
with the [Visualiser](#visualiser) to inspect them. Placement also draws each system's primary mass
and age, which the chart's readout needs. This section works the generation side through far enough
to expose the choices.

### Populations

Seven populations, some of them split further, each with a closed-form number density in systems per
cubic light-year, an age range, and a share of the galaxy's systems drawn from the seed:

| Population      | Shape                                                                                                                  | Share                        | Age               |
| --------------- | ---------------------------------------------------------------------------------------------------------------------- | ---------------------------- | ----------------- |
| Young thin disc | Double exponential, scale height 130–200 ly, strongly bound to the arms                                                | 0.3–0.6% of the thin disc    | −H to 100 Myr     |
| Old thin disc   | About five double exponentials by age, scale length 7,000–11,500 ly, heights from 320 to 1,700 ly, averaging 850–1,150 | the remainder, 47–70%        | 0.1–10 Gyr        |
| Thick disc      | Double exponential, shorter and about three times as tall                                                              | 8–14%                        | 10–12 Gyr         |
| Bulge           | Boxy triaxial exponential along x, scale lengths 1,700–3,000 ly by 0.5–0.7 by 0.3–0.4 of that                          | 20–35% with the long bar     | 8–12 Gyr          |
| Long bar        | Along x, half-length 10,000–18,000 ly, about a tenth as wide, 500–700 ly tall                                          | 30–40% of the bulge's figure | 6–10 Gyr          |
| Nuclear disc    | Double exponential, scale length 200–400 ly, height 0.3–0.5 of that                                                    | 1–2.5%                       | mostly over 8 Gyr |
| Halo            | A mixture of cored power laws near r^−3.5, out to 65,000 ly                                                            | about 1%                     | 10–13 Gyr         |

The shares must sum to 100%, so the seed draws the thick disc, the bulge with its bar, the nuclear
disc and the halo, and the thin disc takes what is left. The bulge range follows Bland-Hawthorn and
Gerhard (2016), who put the Milky Way's bulge and bar together at roughly a quarter to 30% of its
stellar mass. The bulge was first a Gaussian. That put four fifths of its mass inside 1 kpc, drove
the rotation curve to 265 km/s there, and let the velocity dispersion fall too fast with latitude.
The measured density falls exponentially along all three axes (Wegg and Gerhard 2013). So the bulge
is exp(−m), where m combines |x| ÷ a and |y| ÷ b with an exponent of 2, and the result with |z| ÷ c
with an exponent of 3–4, which makes it boxy. With a, b and c of 2,280, 1,440 and 820 ly the model
matches the Milky Way's enclosed mass from 1 pc to 2 kpc and the shape of its dispersion profile
(Zoccali et al. 2014). It never rises with |x|, |y| or |z|, so the nearest-corner bound stays exact,
and it varies by only 15% across a 128 ly cell. A true peanut, thicker along the bar than at its
centre, can come later as this envelope times a bounded vertical factor, as the arms are. The long
bar is a separate, thinner structure reaching some 16,000 ly from the centre in the Milky Way (Wegg,
Gerhard and Portail 2015), and it is what makes a face-on map read as a barred spiral and not as a
spiral with an oval middle. It is level along most of its length and falls off at the end, Gaussian
across and exponential in height, so the bound stays exact for it too. The arms start at its ends:
their fade-in radius is the bar's half-length, and with two arms each leaves the x axis there. The
bulge's central density is about 0.3 per cubic light-year (0.15–0.6 over the ranges), a hundred
times the solar neighbourhood's. The nuclear disc is the small, dense, rotating disc at the heart of
a barred galaxy, fed by gas the bar drives inward. The Milky Way's holds about 10⁹ M☉, between 1.2%
and 2.4% of its stars, with a scale length of 290 ly and a height of 93 ly (Launhardt et al. 2002;
Sormani et al. 2022), which comes to about 18 systems per cubic light-year at its centre, and 9–26
over our ranges. Most of it is over 8 Gyr old and a few per cent formed in the last gigayear. It
surrounds the nuclear cluster of [Dense features](#dense-features-clusters-and-the-galactic-centre),
and being a double exponential it keeps the corner bound exact. The halo is a mixture of components
chosen by marking, most of them the debris of accreted galaxies; see
[Streams and accreted structure](#streams-and-accreted-structure). It stops at 65,000 ly so that it
fits inside the root cube, which also drops about 15% of the globular clusters. The discs' tails
beyond the cube are cut off, which loses a negligible share.

A constant star formation rate over a 10 Gyr disc would put 1% of its stars in the latest 100 Myr,
and an earlier draft used that. It is too many. Once
[supernova remnants](#supernova-remnants-one-route-not-two) are counted from the model instead of
being given a number, 1% yields 3 to 14 core collapses a century against the Milky Way's two. Real
discs formed stars faster in the past: the Milky Way makes under 2 M☉ a year now (Licquia and
Newman 2015) against an average of about 5 over its history. **Lean:** the thin disc's formation
rate declines exponentially with a timescale of 5–9 Gyr drawn from the seed. That fixes both the
young share, at 0.3–0.6%, and the shape of the old disc's age distribution, which is no longer flat.
The age distribution also decides each of the old disc's sub-discs' share, so a system's age is
correlated with its height and its vertical speed (see [Orbits and time](#orbits-and-time)). The
young field's ages carry the factor 1 − φ for stars still inside the [features](#large-features)
that bore them, and run from −H so that stars are born during play. Splitting the young population
out is what makes the arms look right. In real spirals the arms are only a 10–30% ripple in the old
stars that carry the mass; they stand out because the short-lived bright stars have not had time to
leave them. So the old disc gets a gentle cosine modulation and the young disc a sharp one, both
normalised to average 1 around any circle so that the totals do not change. Arms are logarithmic
spirals, two or four of them, with a pitch of 10–18°, fading in outside the bar. The sharp profile
needs two properties: an azimuthal mean that is 1 in closed form, and a width set in light-years,
not in phase. A width in phase becomes physically narrow near the bar, which is exactly where a
density bound is hardest to keep.

Each population is normalised by its share of the galaxy's system count, which is cleaner than
normalising by a "solar neighbourhood" density, because a fictional galaxy has no Sun. The count is
the drawn stellar mass divided by the mean mass of a system (see
[Galaxy parameters](#galaxy-parameters)). Over the parameter ranges that gives 0.001–0.009 systems
per cubic light-year in the plane at 26,000 ly from the centre. Milky Way values give 0.0027 against
the measured 0.0023, so the real galaxy sits mid-range. The statistical test compares a sample
against the density computed for that seed, not against a fixed number.

### Sizing the layers

Integrating a mass function from 0.08 to 150 M☉ gives the share of systems in each band, and a
reference density of 0.003 systems per cubic light-year then gives the expected count per cell. The
reference is a round figure near the middle of our range, a little above the Milky Way's 0.0023, and
the sums below that speak of "the reference density" use it too. The first pair of columns is for
Kroupa's function and the second for Chabrier's system function:

| Layer | Cell   | Primary initial mass | Share (Kroupa) | Per cell | Share (Chabrier) | Per cell |
| ----- | ------ | -------------------- | -------------- | -------- | ---------------- | -------- |
| A     | 8 ly   | 0.08–0.5 M☉          | 76%            | 1.2      | 66%              | 1.0      |
| B     | 16 ly  | 0.5–0.75 M☉          | 9.8%           | 1.2      | 12%              | 1.4      |
| C     | 32 ly  | 0.75–2.5 M☉          | 11%            | 11       | 17%              | 17       |
| D     | 64 ly  | 2.5–8 M☉             | 2.3%           | 18       | 3.7%             | 29       |
| E     | 128 ly | 8–150 M☉             | 0.64%          | 40       | 1.0%             | 64       |

Both are supported. The mass function sits behind one interface, the band shares are computed from
it by integration and never written down as constants, and the five layers are comfortable under
either. Which one a universe uses belongs to its generator version. The arbiter between them is the
test that means something: the fractions among all stars, companions included, against the observed
single-star function. An earlier draft leaned to Chabrier's system function on the ground that a
primary is what placement draws, and expected Kroupa's to make dwarfs too common once companions
were added. Worked through, it is the other way round. Kroupa's function used for primaries, with
companions at the observed frequencies, reproduces the single-star function almost exactly: 76.4% of
all stars below 0.5 M☉ against 75.9%. Chabrier's system function used the same way comes out
top-heavy, at 67%. **Lean:** Kroupa's as the default. Chabrier's stays supported, with its branch
above 1 M☉ scaled by a constant of about 0.65–0.7 fitted offline so that the test passes. The worked
figures elsewhere in this document use the Kroupa columns. Under Chabrier the coarse layers are
about half as full again, which changes none of the conclusions.

This is a correction to the first sketch. Each step up multiplies cell volume by eight, but above
0.5 M☉ the mass function only thins out by about 2.5 times per doubling of mass, so eight layers
reaching 1,024 ly would put ten thousand systems or more in each coarse cell. A range query would
then have to generate a whole 1,024 ly cell to find the handful of stars within 50 ly of the ship.
Five layers topping out at 128 ly keep every cell cheap, and also keep cells small against the
scales over which density changes (the young disc's 150 ly height and the nuclear disc's 90–150 ly
are the tightest).

The boundary between B and C sits at 0.75 M☉ for a reason. In the old, metal-poor populations (halo,
thick disc, globular clusters) stars from about 0.8 M☉ up have already left the main sequence, and
their red giants are the brightest stars those populations have. Below 0.75 M☉ nothing has had time
to evolve at any metallicity, so layers A and B are reliably dim, and a search for bright stars can
skip them.

The same mass bands serve every population, and that must stay true: a population with bands of its
own would need layers of its own. The shares need not be the same, though. A layer's density is the
sum over populations of share × density, and nothing in the thinning cares whether the share is one
number per layer or one per layer and population. Making it the second costs nothing and is what
lets [displaced objects](#displaced-objects-kicks-and-runaways) exist, so the share is a matrix from
the start, even though every population's column is identical in the first milestone. It grows
later: layer E alone gains about a hundred displaced classes.

Four honest caveats:

- The bands are of _initial_ mass. A layer E system whose primary has already died is a neutron star
  or black hole, not a beacon. "Coarse layers are the bright ones" is an upper bound, not a
  guarantee, until the stellar stage says what each star is now.
- It is a loose upper bound at the top. Stars above 8 M☉ live under 40 Myr, and only the young
  population, about half a per cent of the disc, is that young. So layer E is almost entirely
  remnants, with well under one living O or B star per cell among forty systems, and the arms will
  not stand out in it until the stellar stage can tell the living from the dead.
- Layer D loses 2–4% of its systems to ancient Type Ia supernovae that left nothing behind, and
  every layer's share in the field is its budget less what sits in features and catalogue classes.
- Brown dwarfs are not in these five layers. They number perhaps one for every four or five stars,
  companions included, and would mostly be noise at this stage. The free-floating ones arrive later
  in a layer of their own, as do rogue planets; see [Between the stars](#between-the-stars).

### Exact placement by thinning

Treating density as constant across a cell is fine at 8 ly and wrong at 128 ly, where the young disc
changes by a factor of two or more from one face to the other. Subdividing coarse cells works but
complicates the index. Thinning (Lewis and Shedler 1979) is simpler and exact:

1. Find an upper bound on the density in the cell.
2. Draw the number of _candidates_ from a Poisson distribution with mean bound × cell volume.
3. Each candidate opens its own stream, keyed by its ID, draws a uniform position in the cell, and
   survives with probability density(position) ÷ bound.

Survivors are distributed exactly as the field demands, with no visible cell boundaries, and cells
stay independent of each other and of the order they are generated in. The index in a system's ID is
its candidate number, so IDs are stable but not contiguous. Evaluating the populations at the
candidate's own position also gives the odds for which population it belongs to, from which its age
follows, at no extra cost.

Everything rests on step 1 being a true bound. If the density exceeds it anywhere, the acceptance
probability saturates at 1 and the galaxy is silently short of stars there, in a cell-shaped patch
that no ordinary test would notice. The first sketch took the maximum over the cell's corners and
centre with a small margin. A numerical check showed that is not a bound:

- The discs, the halo's components, and a bulge and long bar along the x axis never rise with |x|,
  |y| or |z|. Because the planes x = 0, y = 0 and z = 0 are cell faces, no cell straddles them, and
  the maximum of every one of these components is at the cell's corner nearest the origin. For these
  the corner value is exact. A bar rotated off the axes would break this (its maximum can sit inside
  a face), which is the practical reason the bar defines the x axis.
- Arm ridges cross cell interiors. For a sharp young-disc arm in a 128 ly cell near the bar the true
  maximum beat the corner estimate by up to 12%, more than any small margin.

**Lean:** bound each population separately and add the bounds. Disc envelopes, halo, bulge and long
bar take their nearest-corner value. A disc's bound is its envelope's bound times its arm factor's
bound. The arm factor, including its fade-in outside the bar, which rises with radius, is bounded
from its phase: compute the phase at the cell's centre and how far it can change across the cell,
and if that interval reaches a ridge use the ridge's peak value, otherwise the larger end. This
costs about 2.8 candidates per accepted system in the young disc against 2.0 for the unsafe version.
The young disc is under half a per cent of systems, so the cost overall is under 1%. Using the arm's
global peak everywhere would also be safe, at a few per cent.

The arm factor's trick is general, and the rule behind every bound in this document is this: a
density is an envelope that never rises with |x|, |y| or |z|, optionally times a factor that is
unimodal in one scalar (arm phase, radius R or distance r) and is bounded from that scalar's range
across the cell. A cell that straddles no axis plane has its range of R fixed by its nearest and
farthest corners. The flared layers of [displaced remnants](#displaced-objects-kicks-and-runaways)
use it: exp(−(z ÷ h)^β) ÷ h is unimodal in h with its peak at h = z β^(1 ÷ β), so the bound is the
radial envelope at the nearest corner times that peak if it falls inside the cell's range of h, and
the nearer end otherwise. A peanut bulge would use it the same way. The test that hunts for
violations covers flared classes in the inner galaxy as well as arm ridges.

The bound is part of the generated output: change how it is computed and the candidate counts
change, and with them every star. It belongs to the generator version and is covered by the golden
tests.

### Displaced objects: kicks and runaways

A neutron star is born with a kick. The speeds of isolated pulsars under 10 Myr old follow a
log-normal with a median near 270 km/s: μ = 5.60 ± 0.12 and σ = 0.68 ± 0.10 in ln(km/s) (Disberg and
Mandel 2025). The long-used Maxwellian of Hobbs et al. (2005), σ = 265 km/s, came from a fitting
error and runs half as fast again, and the two modes of Verbunt, Igoshev and Cator (2017) are not
statistically significant. Against a rotation speed of 230 km/s and an escape speed near 570, the
log-normal unbinds about an eighth of neutron stars and lifts most of the rest far out of the disc
they were born in. A galaxy whose dead massive stars all sit where they formed is wrong about
several hundred million objects. Kicks are modelled (see [Decisions](#decisions)).

But isolated pulsars are the ones that got away. Be X-ray binaries, double neutron stars and the
pulsars of globular clusters all need a second mode of 10 km/s or less (Valli et al. 2025), and it
belongs to particular progenitors: electron-capture supernovae, accretion-induced collapse, and
stars whose envelope a companion stripped down to a low-mass core. These stay bound to their
companions, which is why the isolated pulsars do not show them. They are a sixth to a quarter of all
neutron stars.

The stellar stage draws the kick from one law behind one interface, which belongs to the generator
version. Remnant type and mass follow Mandel and Müller (2020) from the mass of the carbon–oxygen
core, so the line between neutron stars and black holes is a probability over initial masses of
about 10–25 M☉ and not a threshold. In the ordinary mode the kick's rank follows their scaling,
(M_CO − M_rem) ÷ M_rem with 45% scatter, and the rank is mapped onto the measured log-normal through
a table computed once per generator version. So the distribution is the measurement, and only the
ordering by progenitor is a model. The low mode is a Maxwellian of σ = 5 km/s. It always applies to
electron capture, and to a companion-stripped star with a probability of 1 for a core under 2 M☉,
falling to 0 at 3. Black holes take the same map times 0.75, and none at all after complete
fallback, which is three quarters of them. White dwarfs get about 1 km/s, which matters only against
an open cluster's escape speed. A scratch Monte Carlo of this law reproduces the pulsars' speeds
with 5–7% under 50 km/s across the sky (5 ± 2% observed; Willcox et al. 2021), the low
eccentricities of double neutron stars, and, among the black holes under 12 M☉ that have measured
motions, the mix of three fifths unkicked and a fifth above 100 km/s (Nagarajan and El-Badry 2025).

The difficulty is position. The straight-line drift of [Orbits and time](#orbits-and-time) is good
for a thousand years, not a billion, and a remnant that has wandered for a gigayear cannot be found
from its birth cell. What is needed is where kicked remnants are _now_, as a density. The
displacement theorem for Poisson processes supplies it: move every point of a Poisson process
independently and the result is again a Poisson process, whose density is the original smeared by
the displacements. So kicked remnants are exactly a set of populations of their own, independent of
everything else, and the only approximation is how well a closed form fits the smeared density. With
the marking theorem, used for [supernova remnants](#supernova-remnants-one-route-not-two), it gives
this scheme:

- Each layer-E system falls into a class by what its primary has become and how long ago: alive or
  retained, displaced (by speed and by time since death), or gone. The classes are fed by a
  population's whole budget, stars born in features included, since every neutron star born in an
  open cluster or association leaves it. The probability of each class for each birth population is
  an integral over the mass function, the population's age distribution, stellar lifetimes, the kick
  law, and binarity: the share of stars stripped by a companion, from the period and mass-ratio
  distributions, which the binary stage must later reproduce. It is computed once per galaxy by a
  fixed quadrature, like the band shares.
- **Alive or retained** systems stay in the grid where their population put them, with that
  population's layer-E share reduced to match. In the grid, retained means a kick below a quarter of
  the circular speed, which is most black holes and a sixth to a quarter of neutron stars, nearly
  all of them in binaries. In a feature the test is the feature's escape speed; see
  [What is inside a cluster today](#what-is-inside-a-cluster-today).
- **Recently dead** is not a class but a test, because a shell's window depends on the gas at the
  site. A candidate of any class whose death falls in the [interval](#events-in-time) evaluates the
  shared function and resolves to "no such system" where the catalogue claims it. A displaced
  candidate first backs out its birth site along a straight line, which is accurate for the two
  million years that matter. So the class table stays independent of position.
- **Displaced** remnants are further populations that exist only in layer E. Their forms were found
  by integrating some twenty million orbits in the model's potential and fitting the present-day
  density per class. Made dimensionless, the fits are universal: with lengths in disc scale lengths
  R_d, kick speed u in units of the circular speed and time since death τ in units of R_d ÷ circular
  speed (11 Myr for the Milky Way), five potentials spanning our ranges agree to 4–5%, and the Milky
  Way's table serves the others nearly as well as their own. So one fitted table belongs to the
  generator version, and only a normalisation depends on the dark halo.
  - Disc-born remnants take eight bins of speed (u edges at 0.25, 0.5, 0.85, 1.3, 1.75, 2.2 and 2.8)
    by seven of age (τ edges at 0.1, 0.3, 1, 2, 4 and 8). The measure of fit is the share of objects
    the scheme puts in the wrong place for their speed and age: 25% with one class, 13% with 21,
    6.6% with these 56, after which the forms and not the bins set the limit. The old populations
    need speed bins only, since their massive stars all died long ago.
  - A disc-born class is a flared layer, exp(−R ÷ h_R) × exp(−(|z| ÷ h)^β) ÷ h with h growing
    exponentially with R, plus a flattened cored power law. An old population's class is one
    flattened cored power law. Slow classes keep a layer a few hundred light-years tall, and the
    fastest fill a near-spherical halo on a slope near −2.5.
  - The flare is real and breaks the rule that a density never rises with |x|, |y| or |z|. Kicks are
    the same everywhere while the disc's vertical pull weakens outward, so the layer's height grows
    tenfold from the centre to seven scale lengths, and at a fixed height the density rises with
    radius in the inner galaxy. The bound survives by the arm factor's trick; see
    [Exact placement by thinning](#exact-placement-by-thinning).
  - Remnants under about 3 Myr dead (τ < 0.3) are still in a layer of height 1.1 u τ R_d, with the
    young disc's arms blurred by 0.8 u τ R_d, and the arm factor is dropped once u τ passes about
    0.4. Those 10–90 Myr dead, the age of most pulsars, are on their first excursion and stand
    taller than they will once mixed. Phase mixing is complete by τ of 8–30, a hundred to three
    hundred million years.
  - Layer E's thin-disc systems are one source, whatever age they drew: born in the thin young layer
    of their day, without arms once τ passes 1. The old disc's own height applies to layers A–D,
    whose stars were heated over gigayears. A neutron star is flung in an instant.
  - Remnants born in the bar, the bulge and the nuclear disc were tested in a rotating barred
    potential, for Milky Way values. There is no threshold below which they keep the bar's shape.
    The elongation fades smoothly with kick speed: 99% of the unkicked bar's at u under 0.25, 79% at
    0.25–0.5, half at 0.5–0.85, a quarter at 0.85–1.3 and none beyond 1.75. The length along the bar
    does not change, while the width and above all the height grow. So each speed class is a
    mixture: a share of the population's own form plus a flattened cored power law, both of which
    never rise with |x|, |y| or |z|. The own-form share by speed bin is 0.95, 0.61, 0.20 and 0.05
    for the bar, 0.91, 0.76, 0.55 and 0.25 for the bulge, and 0.88, 0.67 and 0.43 for the nuclear
    disc with u taken against its own circular speed of about 130 km/s. It fits to 3–5% for the
    bulge and 6–12% for the bar. Kicked millisecond pulsars integrated in a barred bulge show the
    same smoothing (Boodram and Heinke 2022). A second pattern speed was not run, so the shares'
    dependence on the bar's parameters is part of the offline fit.
  - A class also gives velocities, as a mean rotation and three dispersions in units of the circular
    speed. The slowest class rotates with the disc. The fastest counter-rotates on average, because
    prograde kicks escape and retrograde ones stay.
- **Gone** covers the unbound, 13–14% of neutron stars at Milky Way values, and the bound that are
  beyond the root cube at the moment, which the fits count: 71–73% of neutron stars, and 99% of
  black holes, are inside it. At 500 km/s a remnant leaves the cube within about 50 Myr, so the
  unbound still inside are a few hundred thousand at any time. They join the fastest displaced
  class, and the rest are dropped, which is what happened to them.
- A displaced system draws its marks conditionally on its class: initial mass, then time since
  death, then birth population and kick speed within the bin. Its velocity follows from the class,
  so a fast pulsar high above the disc is also moving away from it.

Runaway stars come from the same machinery. When a supernova unbinds a binary the companion leaves
at its orbital speed, and young clusters eject stars by close encounters. Between a tenth and a
quarter of O stars and a few per cent of B stars are runaways, at 30–100 km/s or more (Hoogerwerf et
al. 2001). They take the same forms as the remnants at u of 0.13–0.5, in layers D and E, with τ
counted from ejection and capped by the star's remaining life, so they are never phase mixed: after
10 Myr, a layer about 700 ly tall with arms blurred by a thousand light-years or more. A runaway is
single by construction, and the neutron star that ejected it is elsewhere and independent, which is
true to life: their common origin can only be found by tracing both velocities back. Most companions
released by a supernova are not runaways but walkaways, under 30 km/s (Renzo et al. 2019), and those
above 2.5 M☉ join the same population with a lower speed. A released companion above 8 M☉ dies in
its turn, away from its birthplace, so the runaway population has deaths of its own in the
catalogue. Low-mass companions released the same way are left out. They are under 1% of the dwarfs
and nothing distinguishes them but a slightly high velocity.

A kicked remnant usually travels alone, except in the low mode, which keeps its companion and moves
at the pair's recoil of 10–30 km/s. Those that keep a companion through the explosion become the
X-ray binaries and millisecond pulsars of the [interacting binaries](#covering-every-class-of-star),
which draw their class accordingly.

None of this touches the first milestone except the shape of the share table (see
[Sizing the layers](#sizing-the-layers)). The classes need stellar lifetimes, the potential and the
gas, so they arrive after the stellar stage with a bump of the generator version.

### The range query

"Which systems lie within R light-years of this point?" walks the layers from coarsest to finest,
visiting only the cells that intersect the sphere. Five details matter for the drive-range gameplay:

- **Results are a complete census or nothing, per layer.** If adding a layer would exceed the
  caller's limit, that whole layer is left out and the result says so: "complete above 0.5 M☉
  initial mass". Stopping halfway through a layer would return a lopsided patch of sky. Whether a
  layer fits is decided before generating it, from its _expected_ count (the layer's density, which
  is the sum over populations of share × density, integrated over the sphere). That is deterministic
  and does not depend on what happens to be cached. If even layer E exceeds the limit, the query
  reports that nothing fits and the caller must shrink the radius.
- **The census is by initial mass, so it includes remnants.** In the inner bulge, where nearly
  everything is 8–12 Gyr old, a limit of a few thousand returns only the coarsest layer or two, and
  almost every one of those systems is a white dwarf, neutron star or black hole. A chart there is
  better served by a smaller radius than by a mass floor.
- **Cost scales with volume, and volume with R³.** A 50 ly sphere intersects about 1,400 layer-A
  cells and about 300 cells of the other layers together. At the reference density that is some
  2,900 candidates for 1,600 systems returned, a few milliseconds of work. The coarse layers are the
  least efficient, since a 50 ly sphere is small against a 128 ly cell and twenty candidates are
  generated for each one kept, but they are cheap in absolute terms. A 500 ly sphere intersects a
  million cells. Long ranges therefore need a mass floor ("navigation beacons only"). In the bulge a
  50 ly sphere holds 30,000 to a few hundred thousand systems, and about 35 million at the very
  centre, 7 million from the nuclear disc and the rest from the cluster, so the limit and the census
  rule are needed from the start.
- **The query has a time.** Cells are chosen by epoch position, so the sphere is padded by the
  largest speed × |t|: 1,000 km/s, above any escape speed, which is 0.33 ly a century and adds 2% to
  a 50 ly query. Only the unbound class needs more. Distances are then tested at time t, and systems
  not yet born are dropped. Expected counts do not change, because the process is stationary. Around
  the central black hole the pad per level is the distance a radial plunge could cover, and
  everything inside a radius growing as |t|^⅔ is scanned in full: 0.31 ly and 42,000 systems at a
  century, about 10 ms with cached orbits. Indexing by orbital invariants was looked at and does not
  help, since two fifths of the stars with semi-major axes of 1–10 ly dip inside 1 ly.
- **The grid is not the only source.** Members of [large features](#large-features), of the global
  list of streams, dwarf cores and the galactic centre, the [catalogue classes](#events-in-time),
  and pinned content from [Overlays and persistence](#overlays-and-persistence) are merged in, and a
  pinned volume can suppress the procedural systems inside it. Each merged system counts in the
  layer its mass would have put it in, and a feature's expected members within the sphere are added
  to that layer's expected count for the census decision.

The two substellar layers of [Between the stars](#between-the-stars) come after layer A in the walk
and only when the caller asks for them.

A drive can jump to any system within its range, charted or not (see [Decisions](#decisions)). The
query is therefore the whole of reachability: no chart state is consulted, and the drive's own rules
are for a later brainstorm.

## Visualiser

A `GALAXY` display in the bridge client, as a developer instrument first and the seed of the
navigation charts later. It follows `docs/frontend/ux-guidelines.md`: thin marks on `--surface-0`,
scale, orientation and reference frame always shown, shape for type, colour kept free for status.

- **Galaxy map.** Column density (systems per square light-year along the line of sight) from the
  fields alone, so it never generates a star. Face-on and edge-on views, and a choice of all systems
  or the young population, which is where the arms show: the 10–30% ripple in the old disc is only a
  few grey levels on a logarithmic ramp. Density spans about four orders of magnitude face-on and
  six edge-on, so the brightness ramp is logarithmic, in a single hue from `--surface-0` to
  `--text`, with a stated floor and a legend with units. Two points for the style guide: its ban on
  gradients covers console chrome, not data, but a raster field is a departure from "thin vector
  lines", so the guide will allow it explicitly (see [Decisions](#decisions)). Dust lanes join the
  map when the dust field exists.
- **Local chart.** The systems around a chosen point from the range query, in a rotatable 3D view.
  Worked through under [The local chart in 3D](#the-local-chart-in-3d).
- **Parameters.** The seed and the drawn structural parameters with units.

The map is also the quickest way to tune the model by eye, so there is no separate developer viewer.

Technically this needs three additive protocol messages (galaxy parameters, density map, systems
within range, which carries the query's time), a way for displays to make requests over the client's
single socket, and interactive navigation between displays. The protocol is JSON, so system IDs
cross the wire as strings of 16 hexadecimal digits, because a `u64` does not survive JSON in
JavaScript.

The density map needs care on two counts. A 512 × 512 map with a hundred samples along each line of
sight and a dozen or more components is over 10⁸ density evaluations, which is seconds, not
milliseconds. Face-on, the integral through the discs has a closed form, which removes most of it.
The boxy bulge has none and takes a 32-node quadrature per pixel. Edge-on needs numerical
integration, and must integrate across each pixel too, since the young disc is thinner than a pixel.
The result is cached per seed, view and population, and computed off the connection's task so that
it cannot stall the socket. As for size, a map of floats is over 2 MB as JSON. **Lean:** the server
sends the logarithm of the density quantised to 8 or 16 bits and base64-encoded, a few hundred
kilobytes, with the floor and ceiling alongside.

### The local chart in 3D

Travel is truly three-dimensional, so the chart is a 3D view the operator can rotate. The risk with
any rotatable view is that it becomes a toy: pretty in motion and unreadable when still. The rules
below are there to keep it an instrument.

**Projection.** Orthographic, not perspective. The style guide requires spatial displays to be true
to scale and to show their scale, and only an orthographic view has one scale for the whole picture.
It also has a useful property for this game: a drive's range is a sphere, and the outline of a
sphere under orthographic projection is a circle of the same radius from every angle. The range
limit is therefore always a plain circle on the screen. The ring on the reference plane is a
different thing: it projects as an ellipse and measures distance _within the plane_, so a system
high on a stalk can stand inside the ring and still be out of range. The two curves are labelled,
and the range circle, which carries meaning, is drawn at the guide's 6:1 contrast, not as a faint
`--line` aid.

Two radii are in play and are named apart: the **query radius** (how much sky was fetched) and the
**drive range** (how far the ship can go). The edge of the query is drawn as well, because the
guide's rule on honest data forbids letting unfetched space look empty.

A drive can reach any system within its range (see [Decisions](#decisions)), so the chart has to
show which ones those are. The range circle cannot do it alone, because a system in front of the
sphere or behind it also projects inside the circle. The guide gives `--accent` to what is
"interactive, selected or available", so a reachable system is drawn in `--accent` and the rest in
`--text`, and the list repeats it in words so that colour is never the only signal. Selection then
needs a cue of its own: a bracket reticle around the symbol. A commanded destination takes the
reticle in `--target`. The query radius defaults to the drive range, in which case everything shown
is reachable.

**Depth cues.** Orthographic views give up the depth cue that perspective provides, and a cloud of
dots can appear to flip inside out. Three cues restore it without touching colour:

- A **reference plane** through the chart centre, parallel to the galactic plane, carrying the faint
  `--line` grid and the distance rings.
- A **stalk** from each system to its foot on that plane, as in Homeworld and Elite's galaxy map.
  Stalks are what make a position readable in a still image, so the rotatable view keeps them. It
  does not replace them. Above and below the plane must look different. The usual convention is
  solid above and dashed below, but the style guide uses dashes for predicted paths. A muted stalk
  is out too, because `--text-muted` already means a stale value. **Lean:** the symbol is filled
  above the plane and open below it, with the same outline. A cue on the foot mark was the
  alternative, but in the `TOP` view stalks have no length and every foot hides under its symbol,
  while fill reads in every view and adds no marks to a crowded chart. The outline still encodes the
  type and colour still carries status. The price is that fill can mean nothing else on this kind of
  display, and that the smallest symbol must be large enough for an open shape to read as open. The
  readout gives the signed height as well, so the cue is never the only source.
- **Drawing order**, far to near. A stalk spans a range of depths, so the order is: everything on
  the far side of the plane, then the plane with its grid and rings, then the near side, swapping as
  the elevation passes zero.

An earlier draft also dimmed far systems. That is dropped. The guide demands 6:1 for the parts of a
symbol that carry meaning, which leaves almost no room to dim, and a system whose brightness changed
as the view turned would break the rule that a thing looks the same everywhere. Symbol _size_ stays
tied to the mass layer and never to depth. Sizes are therefore not to scale, which the guide
requires the display to say, with a legend.

**Camera.** An orbit camera around the chart centre with two angles: azimuth about the galactic
north axis and elevation clamped to ±90°. There is no roll, so galactic north is up on the screen at
every elevation short of ±90° and the operator cannot get lost. At exactly ±90° north points at the
viewer and the azimuth alone decides which way is up. The `TOP` preset fixes that by putting
coreward at the top of the screen. A view from the south is mirrored, as it must be, and the triad
shows it. Zoom is separate from both radii and defaults to fitting the query sphere.

**Controls.** Every one works from the keyboard, as the guide requires, and none depends on hover or
a right click:

- Drag, or arrow keys while the chart has focus, to rotate. Wheel, pinch, or `+` and `−`, to zoom.
  The canvas is focusable, carries an accessible name and shows the guide's focus ring. The arrow
  keys move through the system list only when the list has focus.
- Preset views as buttons, each with its single-key binding shown on it, and defined by the
  direction of view: `TOP` looks south onto the plane with coreward up, `SIDE` looks coreward along
  the plane, `FRONT` looks spinward along it, and the default is oblique. Presets matter more than
  free rotation in practice, because they give axis-aligned views that can be measured against the
  grid. The grid itself aligns to the local coreward and spinward directions at the chart centre.
- Query-radius and minimum-mass selectors. The mass floor steps in the layer bands, and the chart
  always shows the census line from the range query (`COMPLETE ABOVE 0.5 M☉`). It doubles as the
  declutter control. In the inner bulge a smaller radius works better than a higher floor; see
  [The range query](#the-range-query). Inside the nuclear cluster the useful radius is a few tenths
  of a light-year, and a few hundredths close to the black hole, so the radius steps and the scale
  bar run down that far.

**Motion.** No automatic rotation and no idle drift: the guide allows motion only to show a change
of state. The view moves only while the operator moves it. Redraws happen on demand, coalesced
through `requestAnimationFrame`, and never on a continuous loop. Presets switch within the guide's
80–150 ms transition, or instantly under `prefers-reduced-motion`. The azimuth and elevation
readouts update at the guide's 4 Hz limit and are not announced to assistive technology on every
frame of a drag.

**Orientation and frame.** A fictional galaxy has no "towards Sagittarius", so directions are named
for the galaxy itself: coreward and rimward, spinward and antispinward, north and south, as defined
under [Coordinates](#coordinates). The chart always shows an axis triad with those labels, an arrow
towards the core, the azimuth (`000°` to `359°`) and signed elevation as numbers, a scale bar in
1-2-5 steps, the frame name (`GALACTIC`) and the centre's coordinates, as distance from the axis,
angle around it from +x, and height. Numbers follow the guide's formats, with digit grouping from
five digits. The solar mass and the megayear are not among the units the guide allows, so the guide
gains them (see [Decisions](#decisions)). The `☉` sign is a problem of its own: neither B612 nor
B612 Mono contains U+2609, checked against the bundled font files, and the guide forbids fallback
glyphs. **Lean:** draw it as a small inline SVG sized to the text, a circle with a centre dot, with
"solar masses" as its accessible name. Planets will need `⊕` the same way, which the fonts also
lack.

**Selection and text.** A canvas is opaque to the keyboard and to assistive technology, and the
guide keeps readable text in the DOM. So the chart is paired with a list of every system in the
query, sorted by distance, which is also how a system is selected from the keyboard. It shows its
position and total as the guide requires of scrolling lists, and is windowed once it passes a couple
of thousand rows. Pointer selection picks the nearest projected symbol within a tolerance of at
least `1rem`, to honour the guide's `2rem` targets, and prefers the system nearer the viewer when
two coincide. Systems exactly in line are told apart through the list. The selected system gets a
readout in an `output` element: designator, distance, coordinates, initial mass, age at the chart's
time, and population. The chart shows that time beside the frame name. Mass and age are cheap draws
on their own streams, so the first milestone includes them with placement even though the rest of
the stellar stage comes later. The few labels, for the selected system and the most massive ones,
are DOM elements positioned over the canvas, which keeps them in B612 at a legible size under
interface scaling.

**Rendering technology.** Points, lines and circles, up to a few thousand of each. Canvas 2D with
our own projection (a rotation and a scale, a dozen lines of arithmetic) handles that with ease and
adds no dependency. Three.js would bring a scene graph, materials and lighting for a display that
uses none of them. **Lean:** Canvas 2D now, with the projection, depth sort and picking written as
pure functions so they can be unit-tested without a canvas. Move to WebGL only if a display needs
tens of thousands of marks.

**Reuse.** The style guide already names the tactical plot and the orbit map as spatial displays,
and both need the same camera, reference plane, stalks, rings, triad and picking. The chart should
be built as a general spatial view that is handed marks to draw, with the star chart as its first
user.

**The galaxy map stays 2D.** It shows a field, not points, and a rotatable version would need volume
rendering. Its two views do combine to choose a 3D chart centre: the face-on view sets x and y, and
the edge-on view sets z.

## Decisions

Settled with the project owner on 2026-09-20:

- **The galaxy is fictional and Milky-Way-like.** No real catalogue and no Sol. Its structure comes
  from the seed, with parameters drawn from the ranges observed for large barred spirals.
- **Travel is free movement in 3D space, limited by drive range.** There are no star lanes and no
  fixed graph of connections. A jump or warp drive has a travel radius, so the primitive every
  navigation feature rests on is "which systems lie within R light-years of this point?". That
  sphere query over the octree must be fast, and it is the first query to build. Reachability
  follows from real geometry: dense regions are easy to cross and sparse gaps between arms or above
  the disc are real obstacles.
- **Life, civilisations and campaign set-up are deferred.** This includes how common life is, where
  a campaign starts and whether the crew is human. The pipeline keeps its habitability hook and the
  overlay mechanism, and nothing more for now.
- **First milestone: galactic structure and a visualiser.** Galaxy parameters, fields, system
  placement and the range query, with a display in the bridge client to inspect them. Placement
  includes each system's population, primary initial mass and age, which the chart's readout shows,
  and queries take a time from the start. The rest of the stellar stage (evolution, remnants,
  multiplicity) comes later. Worked through under
  [Galactic structure in detail](#galactic-structure-in-detail) and [Visualiser](#visualiser). No
  code until the design is agreed.
- **The local chart is a rotatable 3D view.** Orthographic, with a reference plane and stalks, an
  orbit camera without roll, and preset views. See [The local chart in 3D](#the-local-chart-in-3d).
- **One galaxy.** A universe is one galaxy. 10¹¹ systems is already unexplorable, and nothing is
  designed for an intergalactic scale.
- **Surfaces stop at orbital scale for now.** Global maps, composition and atmosphere, as seen from
  orbit and through sensors. See [Hooks for the layers above](#hooks-for-the-layers-above).
- **Stellar evolution is comprehensive.** The stellar stage covers the full range of star
  classifications, from brown dwarfs to hypergiants, with every remnant, variable and interacting
  binary. See [Covering every class of star](#covering-every-class-of-star).
- **Interstellar space has content.** Rogue planets, brown dwarfs, isolated black holes and other
  remnants, dust and nebulae are all generated. See [Between the stars](#between-the-stars) and
  [Large features](#large-features).
- **A drive can jump to any system within range**, charted or not. The drive's own rules are left
  for later.
- **The style guide gains what the chart needs.** A raster density field with a logarithmic ramp and
  a legend, the galactic direction names, the units M☉, Myr and Gyr, and a drawn `☉` sign because
  B612 has none. The edits to `docs/frontend/ux-guidelines.md` are made when the display is planned.
- **Natal kicks are modelled.** Neutron stars and black holes are where their kicks would have taken
  them, not where they formed. The law is the measured log-normal of young pulsars, ordered by
  progenitor, plus a low mode tied to the progenitors that make it. See
  [Displaced objects](#displaced-objects-kicks-and-runaways).
- **Where a choice is open, the most realistic answer wins.** The owner's ruling on the second round
  of open questions, and the default for later ones. It produced these:
  - Supernova remnants have one route. The shell belongs to the star that died, and the feature
    catalogue is only how charts find it. See
    [Supernova remnants](#supernova-remnants-one-route-not-two).
  - Rogue planets follow the measured abundance, about 21 per star, and their layer is resized to
    hold them. See [Between the stars](#between-the-stars).
  - The nuclear cluster follows the Milky Way's measured profile. One 16-cell grid cannot hold it,
    so the galactic centre has an ID layout of its own. See
    [Dense features](#dense-features-clusters-and-the-galactic-centre).
  - Dust and gas do what physics says: extinction by wavelength, in both directions, and a hazard at
    high sublight speed. Nothing is invented for the drive. See
    [Between the stars](#between-the-stars).

Left to this document by the project owner on the same day, and open to revision:

- **The galactic centre and cluster cores** are features with nested grids of their own. See
  [Dense features](#dense-features-clusters-and-the-galactic-centre).
- **Below the reference plane a symbol is open, and above it filled.** See
  [The local chart in 3D](#the-local-chart-in-3d).
- **Both mass functions are supported**, with Kroupa's as the default, because used for primaries it
  reproduces the observed mix of all stars and Chabrier's system function does not. See
  [Sizing the layers](#sizing-the-layers).
- **Close pairs stay two systems**, and a ship's frame goes to the smallest distance ÷ radius with
  hysteresis. See [Coordinates](#coordinates).
- **The long bar is a sixth population.** See [Populations](#populations).

Added in the second round because realism required them, and equally open to revision:

- **The nuclear disc is a seventh population.** A realistic galactic centre has one around its
  cluster. See [Populations](#populations).
- **The thin disc's star formation declines with time**, which sets the young share at 0.3–0.6%
  instead of 1%. See [Populations](#populations).
- **A system's sphere of influence is its tidal radius.** See [Coordinates](#coordinates).
- **Systems have velocities**, closed-form per population. The second round froze their positions,
  and the third unfroze them; see below.
- **The galaxy has a dark halo** among its parameters, because velocities, tidal radii and the
  orbits of kicked remnants all need the full rotation curve and not only the stars. See
  [Galaxy parameters](#galaxy-parameters).
- **Layer shares are per population**, which is what makes displaced objects possible. See
  [Sizing the layers](#sizing-the-layers).
- **Runaway stars** are placed like kicked remnants. See
  [Displaced objects](#displaced-objects-kicks-and-runaways).

Added in the third round, when the open questions were worked through by subagents under the same
ruling, until a round raised no new ones. Equally open to revision:

- **The galaxy has a clock, and everything is a function of it.** Systems drift in straight lines,
  and follow Kepler orbits about the central black hole. Stars evolve, are born and die during play.
  The clock window is a thousand years either side of the epoch. See [Time](#time) and
  [Orbits and time](#orbits-and-time).
- **Sensors see the past light cone.** Every reading is at the retarded time, and charts show the
  navigation computer's extrapolation to the present.
- **Events happen**, from a system's own streams, and the hosts of rare events are carved out into
  catalogue classes so that alerts can find them. See [Events in time](#events-in-time).
- **A supernova shell lasts as long as the local gas says**, by a test shared between the catalogue
  and the cells. Four in five core collapses happen inside association features, which own those
  shells.
- **The Type Ia rate is the observed delay-time distribution**, and binary evolution is conditional
  on it.
- **One potential**, with an NFW dark halo, reduced once per galaxy to tables that every velocity,
  escape speed, tidal radius and orbit reads.
- **The seed draws stellar mass, and the number of systems is derived.** Sizes are tied to the
  masses they hold.
- **The bulge is a boxy exponential**, not a Gaussian.
- **The old thin disc is a set of discs by age**, so that age, height and vertical speed agree.
- **The nuclear cusp has no softened core.** Its density is the integral of a distribution function,
  and it is flattened and rotates through a mark on orbital inclination.
- **Feature members are split by class**, each with its own profile and count, which carries mass
  segregation, the retention of remnants, the loss of black holes, core collapse and multiple
  populations. Escapers need no population of their own.
- **Open clusters span ages to gigayears**, and associations are a kind of their own.
- **Globular clusters are drawn as they are now**, with their history derived backwards.
- **The halo is a marked mixture** of accreted and in-situ components. Streams are tube features on
  a global list, with tracks measured from tracer sprays, and dwarf cores exist inside the cube.
- **The gas field has three phases and a pressure.**

## Open questions

Three rounds of questions were answered on 2026-09-20 and are now under [Decisions](#decisions). The
third was worked through by subagents until a round raised no new questions. None is open.

What remains is offline fitting, which is work and not a choice. Each is a table or a constant that
belongs to the generator version and has a named source to fit against:

- The production table of the displaced populations' forms, including the own-form shares of bar,
  bulge and nuclear-disc remnants over the range of bar parameters. The scratch fits behind
  [Displaced objects](#displaced-objects-kicks-and-runaways) show the scheme works; several young
  bins want regularising.
- The Gaussian-sum coefficients of each density profile's potential.
- The Type Ia yield table and the conditional samplers of the other catalogue classes, against the
  binary formulae and the observed rates.
- Black-hole loss from clusters, against the CMC cluster catalogue; the partial-equipartition
  exponent, against multimass King models; the pulsar count against encounter rate.
- The number of orphan streams per globular cluster, 1.5 and uncertain threefold.
- The helium correction to lifetimes and the horizontal branch.
- The scaling of Chabrier's high-mass branch.
- The kick law's rank table, from the generator's own tracks, and its four defaults: the low mode's
  ramp between core masses of 2 and 3 M☉, the black holes' factor of 0.75, the widths of the
  electron-capture windows, and the fate of merged binaries in clusters. Each is pinned by a test
  above.

## Suggested order of attack

Not a plan, only the dependency order a plan would follow:

1. Random streams, units, the clock with H and L, event keys, coordinate frames and IDs, with golden
   and order-independence tests.
2. Galaxy parameters and the potential tables, fields and system placement, with primary mass and
   age (ages from −H), and layer shares held per population.
3. The range query with its time argument, padding and unborn filter, its protocol messages and the
   `GALAXY` display. This completes the first milestone. It reserves what later stages need so that
   they move no star they need not: the time argument, the ID prefixes, the event-key convention,
   and velocity draws on streams of their own.
4. Stars: single-star evolution as a continuous function of age plus time, remnants and
   classification, with statistical tests, and single-star events.
5. The gas field's density and pressure. Extinction and the consoles' use of it can follow.
6. Velocities, kicks, the displaced populations and the class tables, which need lifetimes, the
   potential and, for the shell test, the gas.
7. Large features and the catalogue classes: clusters with their class tables, associations and
   their bubbles, nebulae, supernova remnants with light curves, and the galactic centre with its
   orbits and the black hole's own events.
8. The global list: dwarf cores and streams.
9. Multiplicity and binary orbits, then the interacting binaries, conditional on class, with the
   accreting white dwarfs, X-ray binaries and mergers and their events.
10. Observation at retarded time, and alerts.
11. The substellar layers.
12. Planetary systems, with events on bodies by the same two constructions, then system and body
    queries for the consoles.

## Sources

Figures above are rounded and should be re-checked against these when they become code.

- Sean Murray, _Building Worlds Using Math(s)_, GDC 2017.
  <https://www.gdcvault.com/play/1024514/Building-Worlds-Using>
- Innes McKendrick, _Continuous World Generation in No Man's Sky_, GDC 2017.
  <https://www.gdcvault.com/play/1024265/Continuous_World_Generation_in__No_Man_s_Sky_>
- No Man's Sky portal address layout. <https://nomanssky.miraheze.org/wiki/Portal_address>
- _Generating the Universe in Elite: Dangerous_ (interview with Dr Anthony Ross), 80 Level.
  <https://80.lv/articles/generating-the-universe-in-elite-dangerous>
- Boxels and mass codes in Elite Dangerous.
  <https://forums.frontier.co.uk/threads/marxs-guide-to-boxels-subsectors.618286/>
- Salmon et al. 2011, _Parallel Random Numbers: As Easy as 1, 2, 3_ (counter-based generators).
- Lewis and Shedler 1979, _Simulation of nonhomogeneous Poisson processes by thinning_, Naval
  Research Logistics Quarterly 26.
- Hörmann 1993, _The transformed rejection method for generating Poisson random variables_,
  Insurance: Mathematics and Economics 12.
- The Rust Rand Book, _Reproducibility_. <https://rust-random.github.io/book/crate-reprod.html>
- Bland-Hawthorn and Gerhard 2016, _The Galaxy in Context_, ARA&A 54 (Milky Way structure).
- Reylé et al. 2021, _The 10 parsec sample in the Gaia era_, A&A 650 (local density).
- Kroupa 2001, _On the variation of the initial mass function_, MNRAS 322.
- Chabrier 2003, _Galactic stellar and substellar initial mass function_, PASP 115.
- Duchêne and Kraus 2013, _Stellar Multiplicity_, ARA&A 51; Raghavan et al. 2010, ApJS 190.
- Hurley, Pols and Tout 2000, _Comprehensive analytic formulae for stellar evolution_, MNRAS 315.
- Hurley, Tout and Pols 2002, _Evolution of binary stars and the effect of tides on binary
  populations_, MNRAS 329.
- Vink, de Koter and Lamers 2001, _Mass-loss predictions for O and B stars as a function of
  metallicity_, A&A 369.
- Burrows et al. 2001, _The theory of brown dwarfs and extrasolar giant planets_, Rev. Mod.
  Phys. 73.
- Baraffe et al. 2015, _New evolutionary models for pre-main sequence and main sequence low-mass
  stars_, A&A 577.
- Pecaut and Mamajek 2013, _Intrinsic colors, temperatures and bolometric corrections of
  pre-main-sequence stars_, ApJS 208 (spectral type calibration).
- Freeman 1970, _On the disks of spiral and S0 galaxies_, ApJ 160.
- McMillan 2017, _The mass distribution and gravitational potential of the Milky Way_, MNRAS 465.
- Dutton and Macciò 2014, _Cold dark matter haloes in the Planck era_, MNRAS 441.
- Eilers et al. 2019, _The circular velocity curve of the Milky Way from 5 to 25 kpc_, ApJ 871.
- Binney and Tremaine 2008, _Galactic Dynamics_, Princeton (Jeans equations, Eddington inversion,
  asymmetric drift, Jacobi radius).
- Sharma et al. 2021, _Fundamental relations for the velocity dispersion of stars in the Milky Way_,
  MNRAS 506; Holmberg, Nordström and Andersen 2009, A&A 501; Robin et al. 2003, A&A 409 (the
  Besançon model).
- Bond et al. 2010, _The Milky Way tomography with SDSS III: stellar kinematics_, ApJ 716.
- Portail et al. 2017, _Dynamical modelling of the galactic bulge and bar_, MNRAS 465; Sanders,
  Smith and Evans 2019, MNRAS 488; Clarke and Gerhard 2022, MNRAS 512 (pattern speed).
- Wegg and Gerhard 2013, _Mapping the three-dimensional density of the Galactic bulge_, MNRAS 435.
- Zoccali et al. 2014, _The GIRAFFE Inner Bulge Survey I_, A&A 562; Valenti et al. 2018, A&A 616.
- Sormani et al. 2020, _Jeans modelling of the Milky Way's nuclear stellar disc_, MNRAS 499; Fritz
  et al. 2016, ApJ 821; Feldmeier et al. 2014, A&A 570; Nogueras-Lara et al. 2020, Nature
  Astronomy 4.
- GRAVITY Collaboration 2018, A&A 615, L15; 2020, A&A 636, L5; Gillessen et al. 2017, ApJ 837 (the
  S-stars).
- Bahcall and Wolf 1976, _Star distribution around a massive black hole in a globular cluster_, ApJ
  209; Hailey et al. 2018, Nature 556; Generozov et al. 2018, MNRAS 478.
- McKee, Parravano and Hollenbach 2015, _Stars, gas, and dark matter in the solar neighborhood_,
  ApJ 814.
- Cioffi, McKee and Bertschinger 1988, _Dynamics of radiative supernova remnants_, ApJ 334; Leahy
  and Williams 2017, AJ 153; Tang and Wang 2005, ApJ 628; Truelove and McKee 1999, ApJS 120.
- Higdon and Lingenfelter 2005, _OB associations, supernova-generated superbubbles, and the source
  of cosmic rays_, ApJ 628; Weaver et al. 1977, ApJ 218.
- Green 2025, _A revised catalogue of 310 Galactic supernova remnants_, J. Astrophys. Astron. 46;
  Ranasinghe and Leahy 2022, ApJ 940; Sarbadhicary et al. 2017, MNRAS 464; Stil and Irwin 2001,
  ApJ 563.
- van der Swaluw et al. 2003, _Interaction of high-velocity pulsars with supernova remnant shells_,
  A&A 397.
- de Avillez and Breitschwerdt 2004, A&A 425 (the hot gas's filling factor).
- Maoz and Graur 2017, _Star formation, supernovae, iron, and α_, ApJ 848; Maoz, Mannucci and
  Nelemans 2014, ARA&A 52; Li et al. 2011, MNRAS 412 (supernova rates).
- Claeys et al. 2014, A&A 563; Maoz, Hallakoun and Badenes 2018, MNRAS 476; Peters 1964, Phys.
  Rev. 136.
- Shen et al. 2018, _Three hypervelocity white dwarfs in Gaia DR2_, ApJ 865; El-Badry et al. 2023,
  OJAp 6; Foley et al. 2013, ApJ 767.
- Zapartas et al. 2017, A&A 601 (late core collapse in binaries); Renzo et al. 2019, A&A 624
  (walkaways).
- Baumgardt and Hilker 2018, _A catalogue of masses, structural parameters and velocity dispersion
  profiles of 112 Milky Way globular clusters_, MNRAS 478, with its online catalogue.
- Pfahl, Rappaport and Podsiadlowski 2002, ApJ 573; Ivanova et al. 2008, MNRAS 386 (neutron star
  retention).
- Breen and Heggie 2013, MNRAS 432; Antonini and Gieles 2020, MNRAS 492; Kremer et al. 2020, ApJS
  247 (black holes in clusters).
- Heinke et al. 2005, ApJ 625; Baumgardt and Sollima 2017, MNRAS 472; Trager, King and Djorgovski
  1995, AJ 109; Bahramian et al. 2013, ApJ 766.
- Lamers et al. 2005, _An analytical description of the disruption of star clusters in tidal
  fields_, A&A 441; Oh, Kroupa and Pflamm-Altenburg 2015, ApJ 805.
- Baumgardt and Makino 2003, MNRAS 340; Gieles, Heggie and Zhao 2011, MNRAS 413; Burkert and Forbes
  2020, AJ 159; Massari, Koppelman and Helmi 2019, A&A 630.
- Milone and Marino 2022, _Multiple populations in star clusters_, Universe 8; Koch, Grebel and
  Martell 2019, A&A 625.
- Naidu et al. 2020, _Evidence from the H3 survey that the stellar halo is entirely comprised of
  substructure_, ApJ 901.
- Bonaca and Price-Whelan 2025, _Stellar streams in the Gaia era_, New Astron. Rev. 100; Mateu 2023,
  MNRAS 520; Fardal, Huang and Weinberg 2015, MNRAS 452.
- Kochanek, Adams and Belczynski 2014, MNRAS 443 (stellar mergers); Pala et al. 2020, MNRAS 494
  (cataclysmic variables); Corral-Santana et al. 2016, A&A 587.
- Stone and Metzger 2016, MNRAS 455; Ponti et al. 2010, ApJ 714; Neilsen et al. 2013, ApJ 774.
- Contreras Peña, Naylor and Morrell 2019, MNRAS 486; Melatos, Peralta and Wyithe 2008, ApJ 672;
  Fuentes et al. 2017, A&A 608.
- Boodram and Heinke 2022, _Millisecond pulsar kicks cause difficulties in explaining the Galactic
  Centre gamma-ray excess_, MNRAS 512.
- Disberg and Mandel 2025, _The kick velocity distribution of isolated neutron stars_, ApJL 989, L8;
  Disberg, Gaspari and Levan 2025, A&A 700, A75; Disberg, Mandel and Hirai 2026, arXiv:2608.19690.
- Mandel and Müller 2020, _Simple recipes for compact remnant masses and natal kicks_, MNRAS 499.
- Willcox et al. 2021, ApJL 920, L37; Igoshev et al. 2021, MNRAS 508; Valli et al. 2025,
  arXiv:2505.08857; Gessner and Janka 2018, ApJ 865 (low kicks).
- Nagarajan and El-Badry 2025, PASP (arXiv:2411.16847); Atri et al. 2019, MNRAS 489 (black hole
  kicks).
- Hobbs et al. 2005, _A statistical study of 233 pulsar proper motions_, MNRAS 360.
- Verbunt, Igoshev and Cator 2017, _The observed velocity distribution of young pulsars_, A&A 608.
- Sartore et al. 2010, _Galactic neutron stars I: space and velocity distributions in the disk and
  in the halo_, A&A 510.
- Hoogerwerf, de Bruijne and de Zeeuw 2001, _On the origin of the O and B-type stars with high
  velocities_, A&A 365.
- Kingman 1993, _Poisson Processes_, Oxford (the marking and displacement theorems).
- Licquia and Newman 2015, _Improved estimates of the Milky Way's stellar mass and star formation
  rate_, ApJ 806.
- Schödel et al. 2014, _Surface brightness profile of the Milky Way's nuclear star cluster_,
  A&A 566.
- Gallego-Cano et al. 2018, _The distribution of stars around the Milky Way's central black hole I_,
  A&A 609.
- Launhardt, Zylka and Mezger 2002, _The nuclear bulge of the Galaxy_, A&A 384.
- Sormani et al. 2022, _Self-consistent modelling of the Milky Way's nuclear stellar disc_,
  MNRAS 512.
- Cardelli, Clayton and Mathis 1989, _The relationship between infrared, optical, and ultraviolet
  extinction_, ApJ 345.
- Bohlin, Savage and Drake 1978, _A survey of interstellar H I from Lα absorption measurements II_,
  ApJ 224.
- Wegg, Gerhard and Portail 2015, _The structure of the Milky Way's bar outside the bulge_,
  MNRAS 450.
- Mróz et al. 2017, _No large population of unbound or wide-orbit Jupiter-mass planets_, Nature 548.
- Sumi et al. 2023, _Free-floating planet mass function from MOA-II_, AJ 166.
- Holman and Wiegert 1999, _Long-term stability of planets in binary systems_, AJ 117.
- Fischer and Valenti 2005, _The planet-metallicity correlation_, ApJ 622.
- Weiss et al. 2018, _The California-Kepler Survey V: peas in a pod_, AJ 155.
- Pu and Wu 2015, _Spacing of Kepler planets: sculpting by dynamical instability_, ApJ 807.
- Chen and Kipping 2017, _Probabilistic forecasting of the masses and radii of other worlds_,
  ApJ 834.
- Zeng et al. 2019, _Growth model interpretation of planet size distribution_, PNAS 116.
- Buchhave et al. 2012, _An abundance of small exoplanets around stars with a wide range of
  metallicities_, Nature 486.
- Kopparapu et al. 2013, _Habitable zones around main-sequence stars_, ApJ 765.
- Dole 1970, _Computer simulation of the formation of planetary systems_, Icarus 13.
