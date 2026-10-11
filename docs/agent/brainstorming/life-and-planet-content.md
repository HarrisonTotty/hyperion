# Life and Planet Content

Brainstorm for what the galaxy's worlds hold: whether a world has life and of what kind, what that
life has made of its planet, how it shows from light-years away and from orbit, the biomes and
organisms a landing party meets, and the resources and places worth finding. This is a design
exploration, not a plan. Decisions are marked **Lean** where there is a recommendation, the open
ones are collected under [Open questions](#open-questions), and what has been settled is under
[Decisions](#decisions).

It is the document the earlier brainstorms left owed. The [galaxy brainstorm](galaxy-generation.md)
ends each body with hooks — a surface seed, bulk composition, surface conditions, a habitability
assessment and resource abundances — and defers life, and how common it is, to a later ruling. The
[rendering brainstorm](rendering-and-planets.md) builds the surface generator and a coarse field of
plates, climate and drainage, but it "generates a _surface_; it does not populate it", and its
climate classes leave bare ground wherever no biosphere says otherwise. The
[single-player brainstorm](single-player-experience.md)'s open question 7 asks for a planets
document covering content, and this one, with the rendering brainstorm, answers it. The brainstorm
after this one, of species, civilisations, history, factions and languages, builds on it, because a
species arises on a world and has to be conditioned on that world.

## Goal and scope

Given a body the galaxy generates, decide what lives there and what is there to find, as a pure
function of the universe seed, the body's ID and the time, and make all of it defensible in the way
the astrophysics is: what a console states is what the numbers say, and where the science is
unconstrained the generator says so and follows a stated parameter.

In scope:

- Whether life arose on a body, when, and how far it has come: its origin, its major transitions and
  its end.
- What life is made of and what it lives on: chemistry, energy, and the productivity a world can
  carry.
- Where life lives: land and ocean, and the habitats beyond the surface.
- What a biosphere does to its planet's atmosphere, albedo and sediments, and how that is applied
  without breaking the astrophysics that plan 14 owns.
- Biosignatures and their false positives, and what the crew can measure at each range.
- Biomes as places on the coarse field, and vegetation as the renderer will draw it.
- Organisms: non-sapient flora and fauna, their descent, form and ecology, and how they are made
  real near the crew.
- Resources as deposits, and geology as places a landing party seeks.
- Knowledge: surveys, samples, and the landing party's conditions and procedures.
- How all of it fits the consoles.
- Where it sits in the generation pipeline, what play stores over it, how it runs and is tested,
  and what it asks of the earlier layers and hands to the next.

Out of scope, each owed its own document, with hooks defined here:

- Sapience, species with culture, civilisations, history, factions and languages: the next
  brainstorm. [Hooks for the layers above](#hooks-for-the-layers-above) says what it receives.
- Technosignatures, ruins and artefacts, which belong to civilisations.
- Trade, markets and mining as gameplay. This document places deposits, not economies.
- The art of creatures and plants. What a body plan looks like as a mesh is a later question, as
  ship models are for the rendering brainstorm.
- The text of LLM enrichment, which reads this layer's facts and never feeds a value back.

## What to take from the references

Every game that populates planets answers three questions: how common life is, how its organisms
relate to their world and to one another, and how a player comes to know them. The
speculative-evolution literature answers the second better than any game.

| Reference                                     | Take                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | Leave                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| --------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| No Man's Sky                                  | One loop from a world's summary to the thing at one's feet: flora and fauna summarised in a word on the world's discovery page, then every creature, plant and mineral scanned into a discovery list, named by its discoverer and uploaded. Resources keyed to what the world is, so that where to look follows from its type. Creatures as artists' templates over few skeletons, with gait following proportion: "if you make a creature really fat, we can automatically make it start to lumber" (Duncan 2015).                                                                                                                                                                                                                                                                                                                                                               | Life on nearly every world, by players' account, and one biome per world, which is deliberate ("it wouldn't make you want to go and visit other planets"; Murray 2016, in _PC Gamer_) and which the _Worlds_ updates of 2024 and 2025 kept. Creatures assembled from parts by chance, with no descent, so that nothing on a world is related to anything else: Compton's "10,000 Bowls of Oatmeal problem", variety that is combinatorial but not perceived (Maiberg 2016). Diet and temperament as labels, with predators that attack prey but no food chain behind them. Resources as tokens of a biome type. A world's fauna counted against their true total before they are found, which no survey could know (community wiki).                                                                                               |
| Elite Dangerous (_Beyond_ and _Odyssey_)      | Detection as a ladder of instruments: the system scan reports a body's biological and geological signals, probe mapping marks their probable habitats, and the search on the ground finds them. Occurrence as rules on atmosphere, temperature, gravity, volcanism, star class and distance from the star (many genera only at or below 0.27 g, by players' measurement on the community wiki). Three samples to a species, each beyond the genus's clonal colony range of 100–1,000 m (community wiki), so that each is a separate individual: a real sampling rule in game form. The Codex, by galactic region, with `Rumoured`, `Reported` and `Discovered` states, one commander's report updating every player's Codex; and the name of a body's first discoverer and first mapper (Frontier Developments 2018). Ring hotspots found by mapping, then an assay of each rock. | A fixed, hand-written list of some twenty genera (fifteen of them added with _Odyssey_; community wiki) whose rules hold galaxy-wide, so that the same species recur on thousands of worlds. Organisms only on thin-atmosphere and airless bodies, the ones the game lands on; elsewhere life is a word in a planet class, Earth-like, water and ammonia worlds and gas giants with water- or ammonia-based life, with nothing behind it. A signal count that is the number of genera present, and a probe map that names them, which no remote instrument could give. Colour variants assigned by table, to the star's class or to a rare element on the ground, rather than by any pigment's physics. Organisms with no ecology and no effect on their world; hotspots with no geology behind them; sampling as a credit-earner. |
| Starfield                                     | A survey over four kinds of finding, resources, flora, fauna and "traits", whose data is itself the product. Traits as named features in categories ("Active Faulting", geophysical; "Boiled Seas", geological; "Aeriform Life", biological) that give a world an identity beyond its class, found at the places they produce. Mineral resources by element from an orbital scan, before any landing; biomes as shares of the surface (Jemison's "60% Ocean, 14% Deciduous Forest").                                                                                                                                                                                                                                                                                                                                                                                              | Twenty-two biome kinds drawn into some 430 variants (community wiki), and flora and fauna as a count per world to be completed, a checklist rather than a biosphere. Worlds with little on them to do but "scan rocks or zap them with a mining laser" (IGN 2023). The same review found them "fairly barren and lifeless", as "planets that're mostly uninhabitable" would be, and that is not the fault: most of HYPERION's worlds are sterile ([Expected statistics at the default](#expected-statistics-at-the-default)), so what is worth finding on them comes from their geology and resources ([Geology as places](#geology-as-places); [Resources and deposits](#resources-and-deposits)).                                                                                                                                |
| SpaceEngine                                   | One or two compact life records per world, each organic or exotic, unicellular or multicellular, and subglacial, marine, terrestrial or aerial, with a flag for panspermia (its manual). And the order of operations this document needs: life seeded after the planetary system, from surface volatiles rather than planet class; allowed to change the atmosphere ("older planets have multicellular oxygen-producing life"); wiped out, or moved under the ice or into the air, if the climate then collapses (SpaceEngine 2018).                                                                                                                                                                                                                                                                                                                                              | Earth's timetable applied to every world, with no oxygen before 2.5 Gyr and the Great Oxygenation Event after it (SpaceEngine 2017). Exotic life admitted without an argument for it. A camera that sees what is true rather than what has been measured.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Star Control II                               | Three scans from orbit, mineral, energy and biological, each drawn as dots on the world's map before any landing. Hazards read from orbit, weather and tectonics as classes 1 to 8 and temperature in °C, which set the chances of lightning, quakes and fire storms on the ground (its source code): a landing decided from instruments. A lander with finite cargo, crew and room for biological data (community wiki).                                                                                                                                                                                                                                                                                                                                                                                                                                                         | Hazards as dice rolled each tick. Minerals as eight tiers of value, `Common` to `Exotic`, rather than ores. Life as targets to stun and collect, and biological data as currency.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| Subnautica                                    | Biomes as named places with edges and their own residents; solar power falling with depth, at a rate set by the biome; a scanner that adds each find to a databank of written entries, the way stored enrichment text can sit beside the measured record. Predation that can be watched.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | One authored map, its biomes placed by hand with overlapping depth ranges (the Crash Zone runs from the surface to about 550 m, by the community wiki) rather than following from physics. Fauna that respawn when eaten or killed, leviathans and a few others apart, so that predation never changes a population.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| Kerbal Space Program                          | Knowledge keyed by how and where it was gained: each experiment's result depends on its situation, landed, splashed, flying low or high, in space low or high, and for many on the biome, with diminishing returns on repetition (its official wiki). That is the natural key of a survey record: instrument, situation and region.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               | Hand-drawn biome maps whose "biomes" are landforms rather than living communities. Science as a currency for unlocking parts.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Dwarf Fortress                                | Biomes as thresholds on generated fields, temperature, rainfall, elevation, drainage and salinity: the structure of R09's coarse field feeding a biome class.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | Plants and animals placed by biome alone, which its author wanted to replace with geographic ranges ("Right now, it goes strictly by biome type"; Adams 2008), as [Organisms](#organisms) does with realms. Its savagery and good-and-evil fields, fantasy axes that decide which creatures appear.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Spore                                         | Hecker et al.'s (2008) animation system, which applies motion authored once to skeletons "that did not exist when the animation was authored": proof that generated body plans can move without animating each one by hand.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       | The creature editor: parts that add speed, strength or diet, or merely decorate, at no physical cost, and with descent only along the player's own line. A review in _Science_ failed its mutation, selection, genetics and drift (Bohannon 2008).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Speculative evolution                         | Every organism explained by its niche and its ancestry, as in Dixon's _After Man_ (1981), whose animals, 50 Myr from now, descend from the survivors of a mass extinction just after our own time. A biosphere with one inherited ground plan: no animal of Barlowe's Darwin IV has eyes, hair or external ears, by the author's own ground rule (Barlowe 1990), and Kösemen's Snaiad began from "a common body plan" "evolved … in successive variations". Bajda's Serina, a terraformed moon seeded with a handful of Earth species (among them grasses, sunflowers, ants, crickets, guppies and the canary, its only land vertebrate), radiates along a dated timeline with cladograms. Van Dijk's Furaha works out limbs under gravity. A field guide as the presentation.                                                                                                    | Single hand-made worlds, whose ground plans an artist chose rather than a generator drew. The art of creatures, which is out of scope.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| Mineral exploration reporting (the JORC Code) | Confidence categories for deposits that an industry already reports in: Inferred, Indicated and Measured resources, "in order of increasing geological confidence", each defined by the sampling behind it (JORC 2012), and below them an Exploration Target, quoted as ranges of tonnage and grade where exploration is not yet enough to estimate a resource (clause 17).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       | Its reserve categories, Probable and Proved, and the economic test every Mineral Resource must pass ("reasonable prospects for eventual economic extraction", clause 20), which need the mining and market factors this document leaves out; only the categories' geological confidence and sampling are borrowed.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |

The games divide on whether life follows from its world. In No Man's Sky and Spore a creature's form
does not: it is assembled from a list of parts, by chance in one and by a player in the other, so
its variety is combinatorial and reads as sameness; Spore lets a world's atmosphere and temperature
set only how many species it holds. In Elite and SpaceEngine life follows by rules on the
environment, and Elite's ladder of scan, map and sample is also the best model of how a crew comes
to know a world. None of the games has common descent, on which the speculative-evolution works are
built: a world's organisms should resemble one another more than anything elsewhere, because they
are related. The common lesson, beside the galaxy and rendering brainstorms': **life is a
consequence of its world, not a decoration on it**, and it reaches the crew only through
measurement.

## What the earlier layers owe us

Like the surface generator, this layer is written against what the earlier layers promise and
invents no planetary statistics of its own ([What the generator already owes
us](rendering-and-planets.md#what-the-generator-already-owes-us)). Status is from the plans'
as-built notes and the code at commit `0c7b257d` (2026-10-10), after plan 14's composition phase
(decision-composition, 2026-10-09) and its moons ruling (decision-p14-moons, 2026-10-10); much is
specified and not built.

**Plan 14.**

| Input                                                                                                                                                                                                                                                               | Status                                                                                                                                                                                                                                                                                                                                        | Read here for                                                                                          |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| Mass, radius, gravity, class, mass fractions, equilibrium temperature (P14.T11–T12, T16)                                                                                                                                                                            | Built; the record's bulk section                                                                                                                                                                                                                                                                                                              | Whether there is a surface or an ocean; gravity; the energy budget                                     |
| `Atmosphere` (P14.T13): `SurfaceState`, surface temperature and pressure, partial pressures of H₂O, CO₂, N₂ and Ar, Bond albedo, cloud fraction                                                                                                                     | Built inside `derive_body`'s three albedo passes; in no record field until P14.T48.e and T35.e. The temperate, snowball and runaway states come from the host's flux alone. T13.c's findings (2026-10-10): no methane greenhouse, and dry worlds beyond the maximum-greenhouse limit at 490–575 K labelled `Snowball` until T51.a's cold trap | The abiotic climate an origin is judged on, and the gases, temperature and albedo a biosphere perturbs |
| Carbon speciation and O₂ (P14.T24.f): methane on cold bodies, abiotic O₂ from a runaway world's lost water                                                                                                                                                          | Built 2026-10-10, held until version 22 (`CARBON_SPECIATION_VERSION`); interim, since T53.a replaces its O₂ route and T55.a its carrier                                                                                                                                                                                                       | False positives; Titan-like methane                                                                    |
| Rotation, obliquity, tidal locking, the `BodyFixedFrame`, figure and photometry (T14, T46, T47)                                                                                                                                                                     | Built; on the wire. A temperate world with thick air takes Earth's phase curve and albedos                                                                                                                                                                                                                                                    | Seasons, day length, locked worlds; the disc a biosphere recolours                                     |
| Surface seed and `BulkComposition` (T23): fractions, volatile inventory, [Fe/H], [α/Fe]                                                                                                                                                                             | Built; server-only, outside the record                                                                                                                                                                                                                                                                                                        | The elemental budget; the seed of per-cell draws                                                       |
| `SurfaceConditions`, `GlobalFigures`, habitability, resources and body events (T24.a–b, T25, T26, T31): liquids and ices by substance, heat flow, regime, present outgassing, magnetic and radiation classes, surface age                                           | Specified; habitability "says nothing about life"                                                                                                                                                                                                                                                                                             | Ocean and land, reductant supply, shielding; the origin's prefilter; nutrients; local hazards          |
| σ_h and continental fraction f_c, `WetEpoch`, the climate regime, signed contrasts (T48.a–e, R09's asks)                                                                                                                                                            | Specified                                                                                                                                                                                                                                                                                                                                     | Land area, the wet history, eyeball and ice-belt worlds                                                |
| Phase L: elements and mantle redox ΔIW (T50.a), condensates (T51.a), area-mix albedo (T51.b), crust (T51.c), escape (T53.a); aerosols with the ozone rule and the vertical structure (T24.c, T24.e); the greenhouse's refits at measured states (T57.a, then T53.a) | The registry built (T49.a); the rest specified, T24.c and T24.e drafted by R08.T1 and not accepted                                                                                                                                                                                                                                            | Which metabolisms pay; a cover term in the albedo; ozone from O₂, haze from CH₄                        |
| Moons' own volatile ranks, the ammonia line and tides by state (T56.c–d)                                                                                                                                                                                            | Specified for the 22 → 23 batch, held behind `MOONS_VERSION`; cold moons' tidal heat 0.4–9% of version 22's, no ocean flag moved; no generated Europa analogue has an ocean until a ruling on the moons' nursery (decision-p14-moons, F2)                                                                                                     | The heat flow a subsurface origin is gated on; Titan-like inventories                                  |
| The biosphere hook (T53.d): `Biosphere { fluxes }` of O₂, CH₄ and N₂O in units of Earth's present biotic flux, which request 2 asks to make the pre-industrial one                                                                                                  | Drafted for the 23 → 24 batch; absent on every body                                                                                                                                                                                                                                                                                           | The slot this layer fills                                                                              |
| Fate transform (T28), `XuvHistory` (P14.T1.a) and the habitable zone at a time (T12.b)                                                                                                                                                                              | Built                                                                                                                                                                                                                                                                                                                                         | The ends of every habitable window; the early ultraviolet load                                         |

**The stellar stage and the galaxy.**

| Input                                                                                                | Status                                                                                         | Read here for                                             |
| ---------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- | --------------------------------------------------------- |
| `StarModel::state_at` (luminosity, T_eff and radius at any age) and `max_luminosity_until` (plan 06) | Built, continuous in age                                                                       | The faint young star; light for photosynthesis            |
| Rotation and `ActivityLevel` (P06.T25); flares and magnetar giant flares (P06.T28.a, c)              | Activity built, a closed form of age; flares specified, on the built event machinery (P06.T27) | Ozone transients; the M-dwarf hazard                      |
| The host's ultraviolet spectrum                                                                      | None: plan 06 gives V and B − V, plan 14 an X-ray and EUV history                              | Surface UV, photochemistry, photosynthesis                |
| Supernova entries by evaluation time, shells and light curves (P09.T15–T19); alerts (plan 12)        | Built; the class grid and processes (P09.T32–T35) not; plan 12 has only `AlertBand`            | Nearby explosions in play                                 |
| Gas and dust (plan 07): density, phase and pressure at a point; features and the centre (plan 09)    | Built at present positions; the black hole's flares and disruptions (P09.T30) not              | Astrosphere compression (speculative); dense environments |
| Gamma-ray bursts                                                                                     | Modelled nowhere                                                                               | Sterilisation                                             |

**R09's coarse field.** Its types, codec, coverage and the pass's frame are built (R09.T1.a, T2–T5,
T6.a, T10, T18; T1.b, the detail seed, lands in plan 14's 21 → 22 batch); plates, elevation,
climate, erosion and classes (T11–T15) are not. Each cell at the body's level will carry elevation,
crust, plate, boundary kind, distance and obliquity, flow direction, drainage area, steepness, water
surface, ice share, a substance byte naming its ice and liquid in the body's palette, a
`SurfaceClass` (Köppen–Geiger codes 1–63, surface-state forms 64–255, and no vegetation class),
crater state, a sea-floor relief byte and, by T2's follow-up C, two runoff bytes. One `ClimateCell`
per four cells carries the sea-level temperature, twelve monthly anomalies, monthly precipitation,
labelled a heuristic, and twelve monthly winds. The header holds the crater contract and a palette
of at most fifteen substances; the coarse craters ride beside the cells. The palette's `Cover` role
is reserved in the code for a life plan's biological cover, and P14.T53.d drafts a `biological`
flag on it; this layer leaves the role to organic covers and puts biological ones in [Biomes as
places](#biomes-as-places)' `LifeTable` instead, since the palette rides in every payload's first
block.

### What is missing, and who should own it

1. **The abiotic state as a named intermediate** (plan 14): once a biosphere changes a world's
   atmosphere, the lifeless twin must stay evaluable on the server, for an origin's prerequisites
   and for false positives, and, with a callable O₂ steady state under a given forcing (request 7),
   for the gates of oxygen.
2. **The history of liquid water** (plan 14). `WetEpoch` bounds one span of surface liquid for R09's
   erosion; an origin needs every interval of age with surface or subsurface liquid, as crossing
   times of closed forms in age: the magma ocean's end (T28), the runaway limit sweeping outward
   and, about a dimming host (a white dwarf, a pre-main-sequence M dwarf), the maximum-greenhouse
   limit sweeping inward (T12.b on plan 06's track), escape (T13.b, T53.a), a subsurface ocean
   freezing. Since plan 14 judges the runaway limit at the worst flux, a dimming host's late window
   opens only for worlds that were never inside it. T53.a now integrates a runaway's loss with the
   greenhouse's melt timed, a part of this history; nothing dates a subsurface ocean's freezing, and
   no generated Europa analogue has an ocean yet (decision-p14-moons, F2).
3. **The reductant supply** (plan 14, by scheduling P14.T55.a's outgassing speciation before life):
   the fluxes of H₂, CO, H₂S and hydrothermal Fe²⁺ from T24.b's outgassing and T50.a's ΔIW, which
   set what a world without oxygenic photosynthesis can carry. T55.a, still unscheduled ("Later"),
   now also replaces T24.f's insolation rule for the carbon carrier, so scheduling it before life
   retires that stand-in as well.
4. **A weathering steady state** (plan 14). A temperate world's airborne carbon is a fixed share,
   `TEMPERATE_AIRBORNE_CARBON`, fitted on vegetated Earth. It needs to be outgassing balanced
   against weathering, with a biological factor; ocean pH, the carbonate saturation microbial reefs
   read and a phosphorus-supply proxy for [productivity](#what-life-is-made-of-and-what-it-lives-on)
   follow from the same balance. The balance also needs a greenhouse that saturates at the
   maximum-greenhouse limit, which T13.c's findings record as missing ([Land](#land)).
5. **Greenhouse, haze and lifetimes for biotic gases** (plan 14): the grey τ has no temperate CH₄ or
   N₂O term, T24.c's haze is on the Surface path, which never feeds back, and CH₄'s and N₂O's
   photochemical lifetimes and ozone's stellar factor need fitted tables by host. Plan 14 records
   the methane gap itself (T13.c's finding "No methane greenhouse", left to the greenhouse's next
   refit with R08's absorbers), and T53.a adds methane's escape, which the lifetime tables must not
   count twice.
6. **Ultraviolet at the surface** (plan 06 and plan 14): the host's 200–400 nm spectrum by T_eff and
   activity, and the column of O₃, CO₂ and haze above the ground, whose product photochemistry,
   organisms and landing parties read. For a prebiotic Earth it runs from 3,520 times modern Earth's
   effective dose about an F0V star to about twice it about an M3.5V (Rugheimer et al. 2015). R08's
   absorbers are client-side and bake only 380–760 nm.
7. **A hazard history** (plan 09, from plan 02's fields): a closed-form rate of core collapses about
   a system's position over its age, since the catalogue covers only the source horizon; the class
   grid (P09.T32–T35) for explosions in play; and gamma-ray bursts, which no plan generates.
   [Whether life arises](#whether-life-arises-and-how-far-it-gets) owns their lethality.

Plan 14's refits now follow P14.T57.a's rule, each greenhouse constant fitted at a measured state in
the order N₂ on Titan, CO₂ on Venus, water on Earth (22 → 23), and P14.T53.a's new CO₂ form refits
them all again (23 → 24). The partial-pressure finding lands before T57.a's fits ([What plan 14 must
take](#what-plan-14-must-take)); this layer's terms (requests 8–9) belong in T53.a's refit, at
Earth's measured gases, rather than a third. And T53.a already takes up Wordsworth and
Pierrehumbert's (2014) oxygen, part of request 6.

## Where life sits in the pipeline

Plan 14 derives the atmosphere, the Bond albedo and the surface temperature from physics, and the
galaxy brainstorm says the surface, life and civilisation generators "consume these and do not reach
back into this pipeline" ([Hooks for the layers
above](galaxy-generation.md#hooks-for-the-layers-above)). Yet Earth's O₂ is biological, as is most
of its CH₄ and N₂O and part of its CO₂ drawdown, and a living world's albedo and climate differ from
its lifeless twin's. The composition ruling of 2026-10-09 narrowed the question twice. The galaxy
brainstorm now says, signed off, that "the pipeline reserves an empty biosphere slot for the life
generator to fill, and makes no life itself", which P14.T53.d drafts as surface fluxes of O₂, CH₄
and N₂O that plan 14's chemistry treats as sources. And its rule of the two paths puts everything
that feeds the surface pressure, temperature, albedo or state, as a biosphere's gases do, on the
snapshot path, for every body, by closed forms and fitted tables. Left to decide are the order, the
state life is judged on, and what the slot holds.

| Option                                                 | For                                                                              | Against                                                                                                                                                                                                                                                                             |
| ------------------------------------------------------ | -------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **A. Inside plan 14, before the atmosphere**           | One pass                                                                         | Biology in the astrophysics stage, against the ruling; life judged on an atmosphere already holding it, a cycle; no abiotic twin                                                                                                                                                    |
| **B. A later layer overriding the atmosphere**         | Plan 14 untouched                                                                | Two authorities for one number. O₂ moves pressure, molar mass, scale height and Rayleigh scattering, so R08, R09, the photometry and the flight model's drag read the wrong air, or each re-derives it                                                                              |
| **C. Two passes through the slot**                     | Plan 14 owns all chemistry and climate, life only biological rates; a fixed cost | The origin is judged on the abiotic state and each later gate on plan 14's steady state under the forcing of the stages already reached, so feedbacks that biology creates are applied stage by stage, never iterated; a biosphere's own warming or cooling is recorded, not judged |
| **D. Coupled to a fixed point, or integrated in time** | Coevolution, the most realistic in principle                                     | Gigayears of history are the replay the galaxy forbids; O₂'s steady states are bistable (Goldblatt, Lenton and Watson 2006), so a fixed point is not unique                                                                                                                         |
| **E. A statistical table per large cell**              | Cheap                                                                            | A summary, not a biosphere; it complements C for [the layers above](#hooks-for-the-layers-above)                                                                                                                                                                                    |

**Lean: C, two passes through one slot.** For a body at time t:

```text
wet     = system.habitability_timelines(ctx)             every body's abiotic history, as crossings
life    = life::system(seed, ctx, &system, &wet, &pins)  the system's records, a BiosphereSource
  o2    = system.oxygen_steady_state(ctx, i, age, &f)    inside it, at each gate of oxygen: plan
                                                         14's steady state on the abiotic snapshot,
                                                         under the forcing f of the stages reached
abiotic = system.body_at(ctx, i, t, &NoLife)             the twin, for false positives and goldens
record  = system.body_at(ctx, i, t, &life)               plan 14's same passes, the slot filled
```

The timelines are each body's liquid-water history above. The system's records are computed whole,
since panspermia links its bodies ([Panspermia within a system](#panspermia-within-a-system)), and
cached beside the system under a `ContentKey`, the galaxy's key with a digest of the overrides in
the system, zero for none, since pinned content changes them and systems no override touches should
share their entries. Plan 14 defines the forcing's type and every equation that turns it into gases,
albedo and temperature; `hyperion_sim::life`, a module beside `planetary` that depends on it,
defines only how the slot is filled, and `body_at` takes the source as an argument with no default,
so plan 14 holds no life code and the module graph no cycle. The server's system cache holds the
system with its life records, and only `life`, tests and goldens pass `NoLife`, so that no reader
takes the twin by accident.

Every reader reads the second record. The first, the abiotic twin, is server-only, for
[Biosignatures](#biosignatures)' false positives and goldens, and its timeline is what the origin is
judged on, which is right, since life arises from conditions without it. The gates of oxygen
(`Oxygenation`, p_cx and `Land`) read plan 14's callable O₂ steady state (request 7), evaluated on
the abiotic snapshot at the gate's age under the forcing of the stages already reached, never the
second record; since a stage's flux depends only on earlier stages, the order stays acyclic and
biology's feedback on its own gates is applied stage by stage, never iterated. Plan 14's
`Temperate`, `Snowball` and `RunawayGreenhouse` states are set by the host's flux alone, so a
biosphere's gases can never put its world into another state: the second pass changes gases,
temperature and albedo only. **Lean:** a biosphere's own climate is recorded, not judged. Its
warming or cooling, a methane collapse at oxygenation or a haze's cooling, shows in the second
record, in what reads its surface temperature (T24.a's liquid-water flag, T24.b's ice fraction,
T48.d's regime), and on consoles, and sets no stage back; the alternative, one lagged check at stage
times, is [open question 56](#open-questions). Life writes into the one slot plan 14 declares and
recomputes nothing it owns, which is "do not reach back" as the brainstorm now reads.

The cost is a second derivation of the atmosphere and of what reads it, plan 14's three albedo
passes again with the slot's chemistry inside, and the O₂ steady state at each gate. Plan 14's rule
holds each snapshot-path term to under a microsecond a body, but the passes are many terms: T46.b
measured `generate` with `snapshot_at` at 1,098 µs a field system and `generate` alone at 724 µs,
against a 1 ms target it already exceeds (provisional, under load), so `snapshot_at` is some 374 µs
a system before life adds its pass. The prefilter keeps sterilised worlds: the second pass runs on
every body whose life record holds an epoch at t, or one ended within the gases' look-back of some
ten O₂ residence times (17 Myr), whatever its present habitability, so that a world sterilised a
million years ago keeps its oxygen. R09's coarse pass, which may take tens of seconds at level 8
with its climate model, runs once, on the second record.

### Streams and tags

Each draw comes from a stream keyed by what it is for ([Random
streams](galaxy-generation.md#random-streams-not-a-random-sequence)), so a property of life added
later moves no planet. **Lean:** one tag per property group of life (the transition clocks,
panspermia, the lineage seed, the biochemistry, the metabolic menu, the growth strategies, the
forcing's gases and the Galaxy's pulses), appended at the end of the sim's `domain_tags!`, whose
order fixes `tags::ALL`; [Tags and scopes, gathered](#tags-and-scopes-gathered) holds the one list
of names, scopes and keys. Two choices are argued here. The extinction pulses are an event series
keyed by the system, so one explosion strikes all its worlds, and neighbouring systems draw theirs
independently, which nothing can contradict before the source horizon, since past galactic orbits
are not modelled. From −(H + L) on, the catalogue's supernovae are the only explosions: the drawn
series stops there, so that none is counted twice and every recent one has its remnant. The pulses
open the first free block of `id/event_tags.rs`, 0x0500–0x05FF, once plan 06's table of blocks sets
it aside (request 22), and the geysers of [Geology as places](#geology-as-places) may share it. And
the lineage seed is the receiver's own, so a world seeded by panspermia shares only its deep
ancestry with its donor ([Descent, not assembly](#descent-not-assembly)). Draws never depend on
time: a stage is the count of transition times that age + t has passed, each a fixed draw made a
waiting time by the body's history, through closed forms and bounded searches of fixed length, so
any moment is evaluated directly, as a star's phase is.

### Time

A biosphere's stages move over 10⁸–10⁹ years, so it is constant over a campaign, but changes inside
the clock window happen: at 0.41 changes per `Temperate` body per gigayear, about 0.25 fall within
1,000 ly during the 2,000-year clock window (3 × 10⁵ bodies × 0.41 Gyr⁻¹ × 2 × 10⁻⁶ Gyr) and some
2,100–7,000 in the Galaxy, at one `Temperate` body per 20 of its 0.5–1.7 × 10¹¹ systems (computed
here from the rates of [Whether life arises](#whether-life-arises-and-how-far-it-gets)), so stages
evaluate continuously and are never asserted absent. Events that end or set back a biosphere in play
reduce to a time. The body's own (the runaway limit crossing it, engulfment, a scorching peak) are
times found on plan 06's track and plan 14's fate transform by a bounded search, as plan 14 finds
engulfment (a fixed scan, then a bisection on the clock's nanoseconds), and cached with the
system's life records. Another object's, a supernova of the catalogue classes, reaches it twice: its
photons at its explosion time plus its distance over c, from both systems' positions and plan 12's
retarded time, and its cosmic rays from some centuries later, for 10³–10⁴ years (Thomas et al.
2016; Melott et al. 2017).

After a transition at T_k each gas relaxes to its new steady state as exp(−(t − T_k) ÷ τ), with τ
its own timescale. O₂'s is long: Earth's atmospheric O₂, about 3.7 × 10¹⁹ mol, over plan 14's own
sink of 2.21 × 10¹³ mol yr⁻¹ (Catling 2014, as P14.T24.f cites it) gives a residence of 1.7 million
years (computed here), so a world sterilised a million years ago still holds about half its oxygen,
a false positive the same physics makes. Flares and the ozone loss after a nearby explosion are
transients composed at query time over a fixed look-back, a flare's as plan 06's are and an
explosion's some 2 × 10⁴ years, inside the catalogue's source horizon, leaving no mark that needs
history replayed ([Events in time](galaxy-generation.md#events-in-time)).

### Generator version

All of it is generated output ([Generator version](galaxy-generation.md#generator-version)): the
batch that first fills the slot bumps the version, code landing earlier behind a hold as P14.T24.f's
does. Since life is judged on plan 14's outputs, a refit of plan 14 can turn a living world dead
while its `body.life.*` draws never move; that is correct, and the bless note names every world
whose stage moved.

### What the coarse field reads

R09's `CoarseInputs::for_body` reads the record's surface section, which after the second pass is
the living world's: mean temperature, contrasts, pressure and gases. Step 5 is unchanged ("It
classifies climate, not life"). **Lean:** the per-cell life state is a step after the coarse pass
(R09's steps 1–6), on its quantised field, computed on both sides from the surveyed field and the
`LifeTable` ([The per-cell state, the material and the
instances](#the-per-cell-state-the-material-and-the-instances)); the server runs the same sweep in
its `SurfaceService` bulk job for R07's disc map and organisms' pools. The cells must realise the
snapshot's cover fraction and productivity, so [What life is made of, and what it lives
on](#what-life-is-made-of-and-what-it-lives-on) owes a body-level closed form beside its per-cell
one, and one unconstrained sweep, then a cover threshold on the cells' area-weighted cumulative
cover and a productivity scale carried in the `LifeTable`, meets both with no iteration: the
threshold first and the scale over the covered cells after it, summed in cell-index order, the cover
to one cell as R09 places ice to plan 14's fraction. Where the snapshot's cover exceeds the area
with any potential, the threshold stops at zero and the shortfall is recorded. The snapshot's cover
albedo is a body-level form too, never the sweep's: like the body-level productivity it reads the
stage and the abiotic record's light at the ground, with the stage's ozone column ([Biomes as
places](#biomes-as-places)), so nothing the second pass or the Surface path makes feeds it; which
climate the body-level forms read is [open question 57](#open-questions). R10's albedo scale is
solved on the substrate against the abiotic twin's geometric albedo, and the biological covers are
laid over it unscaled, so that block 0 carries nothing of life (request 38). Erosion does not read
vegetation, though plants reshaped Palaeozoic landscapes (Gibling and Davies 2012): R09's
calibration is vegetated Earth's and no closed form for the difference was found, so the omission is
labelled.

The Knowledge principle constrains the wire, since T35.e would mirror P14.T48.e's `biosphere` member
to the client and tell it which worlds live. **Lean:** the forcing, the stage, the abiotic twin, the
palette's drafted `biological` flag and any per-cell flag meaning "biological" are server-only, as
the surface seed is; the rest of the rule is [Detail levels for life, and how they
degrade](#detail-levels-for-life-and-how-they-degrade)'s.

### What plan 14 must take

Of T24 only T24.f is built, and T25, T26 and most of Phase L are not, so nine amendments can land
before the code, requests 1–9 of [Requests to the earlier layers](#requests-to-the-earlier-layers).
Three are argued above: the `BiosphereSource` argument to `body_at` and `snapshot_at`, with no
default (request 1); T53.d's slot widened to the whole forcing and moved to the snapshot path (2);
and T48.e's `biosphere` member kept off T35.e's wire (3). Six follow from [A biosphere changes its
planet](#a-biosphere-changes-its-planet), [False positives, made by the same
physics](#false-positives-made-by-the-same-physics) and the list of what is missing: the
liquid-water history (4); the reductant supply, with T55.a scheduled before life (5); abiotic
sources in the same chemistry, part of which P14.T53.a already takes up (6); the ozone and
methane-lifetime tables, with a callable O₂ steady state for the stage gates (7); the weathering
steady state (8); and the grey τ's CH₄, N₂O and haze terms with the area mix's cover term (9).

One finding bears on this layer without being its amendment. T13.c sets each gas's partial pressure
to its weight over the surface, M_i g ÷ 4πR² (`planetary/derive/atmosphere.rs`, `greenhouse`'s
`partials_at`, lines 1242–1271, and the weight at line 1439), where Dalton's law for a well-mixed
air gives its mole fraction times the surface pressure. The error is μ_i ÷ μ̄: Earth's 2.2 × 10¹⁵ kg
of CO₂, 282 ppmv, weighs 42.3 Pa, where its share of the same 98.7 kPa column is 27.8 Pa, so it is
52% too much, the ratio of molar masses (44.01 ÷ 28.96), and 49% above the 28.4 Pa that 280 ppmv of
sea level's 101.3 kPa gives (computed here); biotic CH₄ would come out 45% low. T24.a's mole
fractions, taken as p_i ÷ p_s, inherit it as mass fractions, and T51.a's drafted cap, min(M_s g ÷
4πR², p_sat), repeats it for a minor condensable. The fitted constants absorb part of it on the
calibration worlds. The fix is Dalton's law, p_i = x_i p_s with x_i = (M_i ÷ μ_i) ÷ Σ(M_j ÷ μ_j)
over the airborne masses, where a capped gas's airborne mass is solved from its vapour pressure (a
quadratic for one condensable over a non-condensing background). It lands before P14.T57.a's fits
at the measured inventories, so that no fit is redone for it: T57.a fits CO₂ on Venus, where
the two fractions agree to 1.3%, and water on Earth, where CO₂'s weight is 52% high (P14.T13.c,
T24.a, T51.a, T24.f, T57.a).

## Whether life arises, and how far it gets

This section decides, for every body, whether life began, when, how far it has gone at age + t, and
when it ends, and turns the owner's reserved question, how common life is, into a number. Everything
later — the atmosphere's oxygen, a biome's vegetation, an organism's body plan, a civilisation's
homeworld — reads the stage given here and never decides it again.

### The shape of the model

Each transition k has a rate λ_k per gigayear and a gate G_k, a condition read from plan 14's
outputs and this layer's earlier transitions. Its time T_k is the first at which the gate has been
open for a total of E_k ÷ λ_k since the previous transition, E_k a standard exponential draw. A
clock that pauses while its gate is shut separates the two causes of a long wait the literature
argues over, a step intrinsically improbable (λ small) and an environment not yet ready (gate shut).
The waits are memoryless, as in the hard-steps model (Carter 1983; Watson 2008). Earth's timeline is
one sample of both causes: its dates calibrate defaults and pin nothing.

### How common life is: the rate of origin

**What Earth says.** Earth had liquid water by 4.40 Ga, from a zircon of 4,404 ± 8 Myr whose oxygen
isotopes record crust that had met a hydrosphere (Wilde et al. 2001). Life was present by 3.43 Ga in
widely accepted stromatolites (Allwood et al. 2006), probably before 3.95 Ga (Tashiro et al. 2017)
and possibly by 4.10 Ga (Bell et al. 2015), and a molecular clock puts the last universal common
ancestor (LUCA) at about 4.2 Ga (4.09–4.33 Ga), already a prokaryote-grade anaerobic acetogen in an
ecosystem (Moody et al. 2024).

**What that constrains** is less than it seems, because observers need an early start to have had
time to evolve. Spiegel and Turner (2012) showed the datum "consistent with an arbitrarily low
intrinsic probability of abiogenesis" under uninformative priors: with a prior uniform in log λ over
10⁻³–10³ per Gyr, about 12% of their Optimistic model's posterior lies below 1 per Gyr. Kipping
(2020), inferring the time intelligence takes jointly rather than fixing it, found that under any
prior a rapid origin (λ well above the reciprocal of the time to first evidence) beats a slow one by
a Bayes factor of at least 2.8 on the 3.465 Ga microfossils and 8.7 on the 4.1 Ga zircon, a
preference that would reverse had the first evidence been younger than 680 Ma. So the data mildly
favour a fast origin and rule out no slow one; a single independent origin anywhere would be much
stronger evidence (Spiegel and Turner 2012).

**Lean:** the owner's prior, that microbial life is common where conditions are appropriate, is
supported weakly and contradicted by nothing. λ₀ is a generator parameter per gigayear of open gate,
bounded to 10⁻³–10², default 2 ([open questions 1 and 73](#open-questions)): a mean wait of 0.5 Gyr,
under which Earth's 0.2–0.5 Gyr is typical rather than lucky (a chance of 0.33–0.63). It sits above
the geometric middle of the range because the only evidence, Kipping's, leans that way. Counts
across the range, on the placeholder 3 × 10⁵ `Temperate` bodies within 1,000 ly of [the statistics
below](#expected-statistics-at-the-default), other rates at their defaults:

| λ₀ (per Gyr) | With life | Complex cells | Macroscopic life | Land life |
| ------------ | --------- | ------------- | ---------------- | --------- |
| 10⁻³         | 1,200     | 130           | 90               | 80        |
| 10⁻²         | 12,000    | 1,300         | 910              | 820       |
| 0.3          | 1.7 × 10⁵ | 23,000        | 18,000           | 16,000    |
| 2 (default)  | 2.6 × 10⁵ | 44,000        | 35,000           | 30,000    |
| 10²          | 3.0 × 10⁵ | 56,000        | 45,000           | 34,000    |

Above one per gigayear the count saturates; the upper end changes the marginal worlds, since at 10²
almost any world with land, once wet for 50 Myr, has had life. At the lower end, life within 100 ly
is one or two worlds: the most generous of Spiegel and Turner's (2012) lower limits, 10⁻³ per Gyr,
"once per 200 stars"; their "once in our galaxy", 10⁻¹¹, lies far below the bound.

### What "appropriate conditions" means

In plan 14's terms, a surface origin's gate is open while the body has surface liquid water
(P14.T24.a's flag, not the `Temperate` class, which a world leaves at the conservative zone's inner
edge long before its oceans go), a solid crust (P14.T28.a's `molten_until`), the last giant impact
plan 14 records behind it, and no more ocean-boiling impacts. That last is a short delay read from
plan 14's crater chronology, not a lottery: ecosystems independent of photosynthesis could have
persisted on Earth from about 4.44 Ga (Sleep et al. 1989), and no plausible bombardment fully
sterilised its habitable zone after the Moon-forming impact (Abramov and Mojzsis 2009).

Which worlds can begin life depends on how it began, and the field is split. The surface scenarios
need ultraviolet and land: cyanosulfidic chemistry makes precursors of nucleotides, amino acids and
lipids in ultraviolet-driven steps (Patel et al. 2015), with enough near-ultraviolet from hosts
hotter than K5 (4,400 K) and from cooler ones only if they flare often enough (Rimmer et al. 2018),
and wet–dry cycling in hot-spring pools on land assembles protocells (Damer and Deamer 2020). The
hydrothermal scenario needs only water on warm rock: LUCA was anaerobic, H₂-dependent and
thermophilic, supporting "an autotrophic origin of life … in a hydrothermal setting" (Weiss et al.
2016). But LUCA's habitat says where the ancestor lived, not where chemistry became biology.

**Lean:** both, weighted. A world with surface water originates at λ₀ × (w_v + (1 − w_v) g), g being
one where it has land (plan 14's ocean fraction below one) and its near-ultraviolet at the surface,
200–280 nm, reaches (6.8 ± 3.6) × 10⁹ photons cm⁻² s⁻¹ Å⁻¹, quiescent or by flares (Rimmer et al.
2018), zero otherwise; an ice-covered ocean (`SubsurfaceOcean`) originates at λ₀ w_v, gated on plan
14's heat flow driving water–rock reactions. The hydrothermal weight w_v is a parameter on 0–1,
default 0.1, so a `SubsurfaceOcean` world can originate life ten times more slowly than a temperate
world with land, or be [seeded](#panspermia-within-a-system). After λ₀ it is the least constrained
number here: at 0.1 some 57% of long-lived subsurface oceans carry life, at 0.03 a quarter. Neither
term scales with the habitat's size ([open question 61](#open-questions)). Energy and productivity
are [What life is made of, and what it lives on](#what-life-is-made-of-and-what-it-lives-on)'s.

### The transitions, and whether each is hard

Carter (1983) argued from the near-coincidence of the time intelligence took on Earth and the Sun's
lifetime that at least one but probably not more than two intrinsically improbable steps were
needed; Watson (2008) put their number below 10, best guess 4, and showed that past hard steps fall
evenly through Earth's history whatever their rates, while a step with λT ≳ 1 follows its
predecessor closely. If step i has rate λ_i with λ_i T ≪ 1 over a window T, the chance of completing
n steps in order is about (λ₁T)(λ₂T)⋯(λ_nT) ÷ n!, a product of per-step factors: the owner's second
prior exactly, each stage exponentially rarer than the last. With log-uniform priors, expected
transition times "likely exceed the lifetime of Earth, perhaps by many orders of magnitude"
(Snyder-Beattie et al. 2021).

Two strands of evidence say the steps are not equally hard. The first is repetition: a step that
happened many times is not improbable. Simple multicellularity "has evolved numerous times within
the Eukarya", complex multicellularity in six clades, animals, land plants, florideophyte red algae,
laminarialean brown algae and two groups of fungi (Knoll 2011), and increased intelligence many
times; oxygenic photosynthesis and the eukaryotic cell arose once, and even that is weak evidence,
since the first inventor may have filled the niche and "pull[ed] up the ladder after themselves"
(Bains and Schulze-Makuch 2016). The second is gating. Mills et al. (2025) argue for no hard steps,
the pace set by "the sequential opening of new global environmental windows of habitability", above
all oxygen: eukaryogenesis needed 0.001–0.4% of the present atmospheric level (PAL), met since the
Great Oxidation Event, and animals about 0.1–1% PAL; oxygen after that event is bounded below,
usually at about 1–10% PAL, though 0.05–1% PAL is possible, and settled near present levels by
420–400 Ma. Since Watson's even spacing holds whatever the hard steps' rates, Earth's timing cannot
tell a hard step from a gated one; repetition can.

**Lean:** a rate per step, not a uniform factor. Steps that happened many times are fast once their
gates open; singular steps get free parameters set at the geometric middle of the range the
arguments span, since no evidence prefers either end. The final stage list:

| Stage                          | On Earth                                                                                   | Origins                                                                                                              | Gate, beyond the previous stage                                                                                                         | Per Gyr, default [bounds]  | Factor |
| ------------------------------ | ------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- | -------------------------- | ------ |
| `Origin`                       | Before 3.95 Ga; LUCA about 4.2 Ga                                                          | One lineage survives                                                                                                 | As above                                                                                                                                | λ₀ = 2 [10⁻³–10²]          | 0.87   |
| `OxygenicPhotosynthesis`       | O₂ of at least 3 × 10⁻⁴ PAL by about 3 Ga (Crowe et al. 2013; disputed: Albut et al. 2018) | Once                                                                                                                 | Surface water; light above the chemistry section's threshold; photic-zone impacts over, after about 3.8 Ga on Earth (Sleep et al. 1989) | 0.3 [10⁻²–10]              | 0.66   |
| `Oxygenation` (above 10⁻⁵ PAL) | Onset 2,460–2,426 Ma (Gumsley et al. 2017)                                                 | Planetary                                                                                                            | Biotic O₂ outruns its sinks in plan 14's steady state (P14.T53.d)                                                                       | Mean delay 0.5 Gyr [0.1–2] | 0.92   |
| `ComplexCells`                 | Eukaryotes by 1.9–1.7 Ga (Bains and Schulze-Makuch 2016)                                   | Once                                                                                                                 | Oxygenated                                                                                                                              | 0.1 [10⁻³–10]              | 0.28   |
| `SimpleMulticellularity`       | Filaments at 1,635 Ma (Miao et al. 2024)                                                   | Numerous                                                                                                             | None further                                                                                                                            | 5                          | 0.95   |
| `ComplexMulticellularity`      | Ediacara biota, 571–541 Ma, _Dickinsonia_ among its animals (Bobrovskiy et al. 2018)       | Six clades                                                                                                           | O₂ partial pressure of at least p_cx                                                                                                    | 2 once the gate opens      | 0.83   |
| `Land`                         | Land-plant spores by 475 Ma (Wellman, Osterloff and Mohiuddin 2003)                        | Plants once (Delwiche and Cooper 2015); animals several, arthropods alone three times (Lozano-Fernandez et al. 2016) | Exposed land; ozone, O₂ above about 10⁻² PAL (Segura et al. 2003); a host not stripping it                                              | 5                          | 0.86   |

The factor is the share of `Temperate` bodies at the previous stage that reach this one over the
[population below](#expected-statistics-at-the-default), with exposed land on 80% of them and the
rest originating at λ₀ w_v and then passing through every stage but `Land`, since the light gate
needs no land. On the owner's ladder, microbial life is `Origin` to `ComplexCells`, viruses, mats
and prokaryotic filaments included; "simple multicellular" is `SimpleMulticellularity`; and
"macroscopic plants and animals" are `ComplexMulticellularity` in water and `Land` ashore. The
threshold p_cx is 10³ Pa, above which organisms relying on diffusion reach about a millimetre, with
10³–10⁴ Pa needed beyond a centimetre with circulation (Catling et al. 2005); bounds 2 × 10²–10⁴ Pa.
It sits at the bottom of Catling et al.'s range, whose "oxygenation time" is defined at 10⁴ Pa, and
above the animals' minimum of about 21–210 Pa (0.1–1% PAL; Mills et al. 2025). When O₂ crosses 10⁻⁵
PAL and p_cx is, in the end, plan 14's answer: P14.T53.d's steady state against its sinks, made
callable ([request 7](#requests-to-the-earlier-layers)) and evaluated on the abiotic snapshot under
the fluxes [A biosphere changes its planet](#a-biosphere-changes-its-planet) sets for the stages
already reached, never on the second record. The gates read that O₂, and since a stage's flux
depends only on stages before it, the order stays acyclic. Until plan 14's reductant supply lands,
the crossings are drawn as stand-ins on reserved words of `body.life.steps`: a delay after oxygenic
photosynthesis with a mean of 0.5 Gyr, and a further one to p_cx with a mean of 1 Gyr (Earth's lies
anywhere from nothing to 1.8 Gyr). The computed crossing then replaces them under a version bump,
the words staying reserved.

**How far this departs from the owner's ladder.** The drop is concentrated in the two singular
steps: from microbial to simple multicellular life the share falls to 0.16, from simple to
macroscopic only to 0.83, from macroscopic to land to 0.86. The prior holds for the first rung,
where the literature is silent and the default honours it, and fails for the later ones, where
repetition says the steps are easy once oxygen allows. A strict hard-steps setting (oxygenic
photosynthesis 0.05, complex cells 0.02 and complex multicellularity 0.05 per Gyr) reproduces the
ladder, each rung ten to a hundred times rarer: of `Temperate` bodies, 0.87 living, 0.009
multicellular, 0.0008 macroscopic. On the placeholder count of [the statistics
below](#expected-statistics-at-the-default), that is 2.6 × 10⁵ worlds with life within 1,000 ly,
2,800 with complex cells, 2,600 simple multicellular, 250 macroscopic and 210 with land life,
against the default's 44,000, 42,000, 35,000 and 30,000. At the default a world with land reaches
`Land` within Earth's 4.4 Gyr with a chance of about 0.06, the lucky tail the selection effect
expects.

### When life ends, and when it is set back

**Sterilisation** ends an epoch of life; a new origin may follow if the gates reopen, and the ended
epoch stays in the record as a fossil biosphere for [Resources and
deposits](#resources-and-deposits) and [Biosignatures](#biosignatures). A **setback** follows a
gate's lasting closure ([open question 58](#open-questions)): the stages above it are lost, their
clocks restarting with fresh draws if it reopens. **Mass extinctions** that leave the stage standing
are a dated series of events and change no stage, as Earth's were followed by recovery within
5–20 Myr with no lasting loss of complexity (Bains and Schulze-Makuch 2016).

| Cause                                                          | Evidence                                                                                                                                                                                                                                 | In the model                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| -------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Brightening: plan 14's state becomes `RunawayGreenhouse`       | The oceans are lost                                                                                                                                                                                                                      | Sterilisation                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Before that, CO₂ starvation and deoxygenation                  | A C4-plant biosphere lasts 0.9–1.5 Gyr more (Caldeira and Kasting 1992); O₂ above 1% PAL lasts 1.08 ± 0.14 Gyr, falling before the moist greenhouse (Ozaki and Reinhard 2021); microbial refuges to 2.8 Gyr (O'Malley-James et al. 2013) | Setback to `OxygenicPhotosynthesis` past a fixed effective flux S_deox, what a body at Earth's present flux receives 1.08 Gyr later on plan 06's 1 M☉ track (about 1.11 S⊕, computed here), scaled by host temperature as plan 14's limits are and read against the summed flux, for dimming hosts and multiple stars alike; CO₂ starvation acts only through productivity's C3- and C4-like ramps ([The energy budget](#the-energy-budget-in-closed-form)), never as a setback |
| A lasting snowball (plan 14's state, read on the abiotic twin) | Oxygenic phototrophs survived Earth's Palaeoproterozoic snowball (Bains and Schulze-Makuch 2016), but plan 14's is a climate state, not an episode                                                                                       | Setback to `Origin` if an ocean persists under the ice, sterilisation if not, judged on the twin's water rather than the label, since until P14.T51.a some rocky worlds plan 14 labels `Snowball` are at 490–575 K; a biosphere's own cooling, as a methane collapse at oxygenation would bring, shows in the second record and sets no stage back ([open question 56](#open-questions))                                                                                        |
| Host death, orbital fate, impacts (P14.T28, T24)               | Engulfment, unbinding at a supernova, a white dwarf's dimming, a giant impact                                                                                                                                                            | `Destroyed` and a giant impact sterilise; an unbound or dimmed body follows its new state                                                                                                                                                                                                                                                                                                                                                                                       |
| Supernovae and gamma-ray bursts; flares                        | Below                                                                                                                                                                                                                                    | Extinction pulses and a recovery gate; the land gate shut by flares                                                                                                                                                                                                                                                                                                                                                                                                             |

**A request to plan 14.** Plan 14 judges the runaway against S_eff = 1.0385 for a Sun-like host
(P14.T12.b's limit, which T13.c applies at the worst flux), the one-dimensional, cloud-free limit of
Kopparapu et al.'s (2013) erratum, which puts Earth right on the limit because, its authors later
wrote, the model cannot account for humidity and clouds (Kopparapu et al. 2014). A body at Earth's
present flux reaches it about 0.40 Gyr later on plan 06's track, ahead of deoxygenation, so its
whole biosphere would end at once. Three-dimensional work puts the threshold near 1.10 S⊕, about
375 W/m² against today's 341 (Leconte et al. 2013), and a three-dimensional Earth avoids a thermal
runaway to at least 21% above today's flux, losing water from 19% and with its mean surface above
360 K by then (Wolf and Toon 2015); the literature gives the biosphere about 0.9–1.9 Gyr more
(Caldeira and Kasting 1992; Ozaki and Reinhard 2021; Mills et al. 2025). The 0.4 Gyr belongs to the
one-dimensional limit, not the stellar track: the track's Sun at 4.57 Gyr has 0.953 L☉, within plan
06's tolerance, which only lengthens a Sun-age analogue's window. The request, for plan 14 to rule
on: judge the runaway against Kopparapu et al.'s (2014) Leconte-scaled coefficients, S_eff☉ = 1.107
for 1 M⊕ (0.99 at 0.1 M⊕ and 1.188 at 5 M⊕), keeping 1.0385 as the recorded one-dimensional
alternative and Wolf and Toon's (2015) stability to at least 1.21 S⊕ as the less conservative
bound; and say whether P14.T25's `Temperate` class should still end at the conservative zone's
moist-greenhouse edge, S_eff☉ = 1.0146, which Kopparapu et al. (2014) replace by the runaway limit.
Even 1.107 arrives within about 10 Myr of S_deox on plan 06's track, about 1.09 Gyr after Earth's
present flux (computed here) and inside Ozaki and Reinhard's ± 0.14 Gyr, so a deoxygenated
biosphere would last at most some 10 Myr and never reach O'Malley-James et al.'s (2013) microbial
refuges.
Most of the literature's 0.9–1.9 Gyr, and any swansong biosphere, need the later bound or an
ocean-loss time between the threshold and sterilisation. This layer takes whatever runaway time
plan 14 gives, and calibrates its own timings by flux, never by years from now.

**Radiation from the Galaxy.** A supernova within about 8 pc roughly doubles biologically active
ultraviolet at the surface, some 1.5 times per Gyr at the Sun (Gehrels et al. 2003). Melott and
Thomas (2011) give the rate within distance D below 100 pc as about 2 per Myr × (D ÷ 100 pc)³, an
extinction-level distance of 8–10 pc (one or two events per Gyr), and a long gamma-ray burst's of
about 2 kpc, where 100 kJ/m² removes some 30% of the ozone; interacting Type IIn supernovae are
lethal by X-rays to about 50 pc (Brunton et al. 2023). The sources disagree on where it is safest:
Piran and Jimenez (2014) give a 95% chance of a lethal burst within 4 kpc of the centre, while
Spinelli et al. (2021), weighting by metallicity, find 2–8 kpc safest in the last 4 Gyr, and
Lineweaver, Fenner and Gibson's (2004) galactic habitable zone, 7–9 kpc out, assumes such events
reset complex life. Yet "lethal" here means an extinction like the late Ordovician's, probably a
burst (Spinelli et al. 2021), or the end-Devonian's, perhaps a supernova at about 20 pc (Fields et
al. 2020), both of which complex life survived.

**Lean:** radiation events are extinction pulses, dated and graded by fluence at the body, a
supernova's cosmic rays included, which arrive some centuries after its light and act for 10³–10⁴
years (Thomas et al. 2016; Melott et al. 2017). They lie on an event series whose rate is a closed
form of the local densities of core collapses, with the young population's metallicity, and of
thermonuclear supernovae, which follow the old population (a delay-time distribution falling as t⁻¹,
from memory; not re-checked); its supernovae are drawn only before the catalogue's source horizon,
−(H + L), and after it are the catalogue's own ([Streams and tags](#streams-and-tags)).
[Organisms](#organisms) and history read them. They change no stage except through a recovery gate:
where extinction-level events come faster than one per recovery time (default 10 Myr, the geometric
middle of the 5–20 Myr above), fifty to a hundred times the Sun's rate, complex multicellularity and
land cannot be held. That keeps a galactic habitable zone, in the inner bulge and dense star-forming
regions, milder than Lineweaver, Fenner and Gibson's and built from measured rates and Earth's
record rather than an assumed reset.

**Flares.** A great flare's ultraviolet barely changes the ozone, but its protons can remove 94% of
it, while the damaging ultraviolet at the surface exceeds Earth's for under 100 s (Segura et al.
2010); repeated flares with proton events deplete it by 94% in ten years with recovery unlikely, an
ocean remaining a refuge (Tilley et al. 2019). **Lean:** the land gate is shut while the host's
activity (P06.T25) is high and plan 14 gives the body no magnetic field; the ocean stages proceed.
How the host terms combine about more than one star is [open question 62](#open-questions).
Atmospheric loss is plan 14's (TRAPPIST-1 b and c show no thick atmospheres; Greene et al. 2023;
Zieba et al. 2023): a world it strips loses its gates.

### Host type

- **M dwarfs.** Habitable-zone planets may spend several hundred million years in a runaway
  greenhouse during the long pre-main sequence and lose several oceans (Luger and Barnes 2015).
  Their quiescent near-ultraviolet is too weak for the surface origin's chemistry, and only about a
  fifth of early M dwarfs flare often enough to make up for it (Rimmer et al. 2018), so most M-dwarf
  worlds originate at λ₀ w_v. Plan 14 takes Luger and Barnes's water loss and abiotic O₂ for worlds
  in the runaway state (P14.T24.f, whose O₂ route T53.a replaces, and T53.b), but not the
  pre-main-sequence runaway itself, because `max_luminosity_until` counts from arrival on the main
  sequence, though plan 06's tracks hold the luminosity before it (P06.T15). Flares shut the land
  gate of a body without a magnetic field; tidal locking is plan 14's limit, not a gate; and
  cyanobacteria photosynthesise similarly under simulated M-dwarf and solar light (Battistuzzi et
  al. 2023). The critical-step model suppresses M-dwarf habitability (Lingam and Loeb 2019b); here
  suppression emerges from the gates, not a penalty.
- **K dwarfs** hotter than K5 meet none of these limits; cooler ones fail g unless active enough,
  as some 30% of stars cooler than K5 are (Rimmer et al. 2018). Both outlive the Galaxy, and they
  are where Lingam and Loeb (2019b) expect most microbial biosignatures.
- **F stars.** Short lives truncate the clocks, as Catling et al. (2005) expect the oxygenation time
  to preclude complex life about short-lived stars. Their ultraviolet does 2.5–7.1 times Earth's DNA
  damage (Sato et al. 2014), but high O₂ shields an F2V planet better than Earth (Segura et al.
  2003), so the land gate's ozone condition carries it.
- **After the main sequence** a planet can stay in the outward-moving habitable zone for 200 Myr to
  9 Gyr (Ramirez and Kaltenegger 2016), and a white dwarf's habitable zone at 0.005–0.02 au lasts at
  least 3 Gyr (Agol 2011), though its planets risk never becoming habitable (Barnes and Heller
  2013). The gates read plan 14's state, so a moon thawed by a red giant simply runs its origin
  clock.
- A brown dwarf's planets can hold surface water while its cooling habitable zone passes them, at
  times for gigayears (Bolmont, Raymond and Leconte 2011), if they were not desiccated before it
  arrived (Barnes and Heller 2013); rogue planets offer only buried habitats, [Habitats beyond the
  surface](#habitats-beyond-the-surface)'s. The same gates apply.

### Panspermia within a system

Microbes on Mars would reach Earth: transfer is "not only possible but also highly probable"
(Mileikowsky et al. 2000). Transfer to the moons of Jupiter and Saturn is possible but rare, and a
planet's own returning ejecta could re-seed it after sterilisation (Worth, Sigurdsson and House
2013); in a compact system such as TRAPPIST-1 it is potentially orders of magnitude more likely than
from Earth to Mars (Lingam and Loeb 2017). **Lean:** in, within a system. A body whose gate is open
gains an origin rate κ from each inhabited body of its system, and life arriving that way records
its donor and the time of transfer: its biochemistry is the donor's, copied whole, and its tree in
[Organisms](#organisms) follows the donor's lineage seed for ranks older than the transfer, in
practice the domains, and its own below. No measurement fixes κ, so it is chosen: 0.3 per Gyr
between neighbouring rocky planets, a hundredth of that to a giant's moon, 1 between moons of one
giant. A sterilised body's own returning ejecta are left out as a donor, a simplification. At the
default λ₀ the counts barely move; at 10⁻² or below, life clusters in systems, pairs of worlds
sharing one biochemistry and one deep tree. Transfer between systems is out, since a system's life
would then depend on its neighbours' and on past galactic orbits, which are not modelled.

### The output

**Lean:** a module `hyperion_sim::life` with a record per body, `BiosphereHistory`, of up to four
epochs (named so as not to clash with P14.T53.d's `Biosphere`, the flux hook it feeds), each holding
its origin time, its source (a surface or subsurface habitat, or a donor body), a lineage seed, its
biochemistry, the time each later stage was reached, a dated `phototrophy` field, setbacks as (time,
stage) and an end with its cause. The stage at t, complex life's age, the remaining lifespan (the
record covers the future) and fossil epochs are lookups; bodies whose origin gate never opens, most
of them, have no record. It is a pure function of the universe seed, the `BodyId` and earlier
layers: plan 14's habitability over time (surface water, state, class, ocean fraction, heat flow,
magnetic field, fate), the host's light and activity (plan 06) and the radiation rate at its
position (the fields), computed per system in time order for panspermia. Gates change at a handful
of times, so each is a short list of open intervals whose ends plan 14 and plan 06 find by
bisection, the O₂ gates re-found after each stage from the fluxes reached so far, and each
transition time an exact inversion over them: no time step, nothing replayed.

The `phototrophy` field dates the first phototrophs, on which [Biomes as places](#biomes-as-places)
and [What life is made of, and what it lives on](#what-life-is-made-of-and-what-it-lives-on) gate by
that name. It is not a `Stage`, so stage counts and goldens do not move: a fast wait after `Origin`,
at 5 per Gyr while surface water and light are present, held no later than `OxygenicPhotosynthesis`,
whose light gate it shares. Anoxygenic phototrophs came first on Earth (Hohmann-Marriott and
Blankenship 2011), with photosynthetic mats by 3,416 Ma (Tice and Lowe 2004); the rate is chosen,
not measured.

Draws, under the tags of [Runtime and code shape](#runtime-and-code-shape), key each epoch by
(window, attempt), never by its ordinal: the window is the index of the open interval of the
abiotic habitability timeline in which the epoch's origin falls, and the attempt its number within
it, should a window hold more than one, so that a changed rate, which moves no window, never
reshuffles organisms. On `body.life.steps`, a standard exponential per (window,
attempt, transition, redraw), counter-based so a setback's fresh draw shifts no other, and the two
stand-in O₂ delays per (window, attempt) on reserved words; on `body.life.transfer`, keyed by the
receiver, one exponential per (window, attempt, donor) of the epoch it would found, counter-based
as the steps are, so a donor can seed the epoch after a sterilisation; on `body.life.lineage`, the
lineage seed, a block per (window, attempt) keyed by the body alone, as the surface seed is; on
`body.life.metabolism`, the phototrophy wait per (window, attempt); and on `system.life.pulse`, the
extinction pulses, Poisson bins of 2⁴⁴ s, about 0.56 Myr, the longest `PoissonBins` allows, listed
only when a reader asks for dated pulses, since the recovery gate reads their rate in closed form;
those of supernovae end at the catalogue's source horizon, −(H + L), the bin across it thinned to
the part before it. Deoxygenation and the recovery gate draw nothing.

### Expected statistics at the default

Shares of `Temperate` bodies by how long their surface has been habitable, and counts within 1,000
ly:

| Stage reached             | 1 Gyr   | 2 Gyr | 4.4 Gyr (Earth) | 8 Gyr | Population | Within 1,000 ly |
| ------------------------- | ------- | ----- | --------------- | ----- | ---------- | --------------- |
| `Origin`                  | 0.73    | 0.85  | 0.92            | 0.96  | 0.87       | 2.6 × 10⁵       |
| `OxygenicPhotosynthesis`  | 0.13    | 0.30  | 0.61            | 0.83  | 0.58       | 1.7 × 10⁵       |
| `Oxygenation`             | 0.062   | 0.22  | 0.55            | 0.81  | 0.53       | 1.6 × 10⁵       |
| `ComplexCells`            | 0.002   | 0.015 | 0.098           | 0.28  | 0.15       | 44,000          |
| `SimpleMulticellularity`  | 0.001   | 0.011 | 0.090           | 0.27  | 0.14       | 42,000          |
| `ComplexMulticellularity` | 0.0001  | 0.003 | 0.060           | 0.24  | 0.12       | 35,000          |
| `Land`                    | 0.00005 | 0.002 | 0.050           | 0.21  | 0.10       | 30,000          |

These shares come from a Monte Carlo of the model: ages uniform over 0–10 Gyr, as in the thin disc;
exposed land on 80% of bodies, with g = 1 on those (enough near-UV assumed), and the other 20%
originating at λ₀ w_v and then passing through every stage but `Land`, since the light gate needs
no land; photic-zone impacts over from the start (Earth's 0.6 Gyr delay would lower the population
shares from `OxygenicPhotosynthesis` on by 3–8%, and `Land` at 4.4 Gyr to 0.041); and no host
penalties, which would lower every row, the first included where a host cooler than K5 fails g.
**The counts rest on a placeholder.** The 3 × 10⁵ `Temperate` bodies within 1,000 ly are an
estimate here, to be re-derived from plan 14's own count of `Temperate` bodies per system, a slow
statistic P14.T25's test reports only as the share of FGK hosts with a `Temperate` body. It takes
the galaxy brainstorm's local 0.0019 systems per cubic light-year, about 0.75 of it on average over
the sphere for the old thin disc's cored cohorts, and one `Temperate` body per 20 systems, with
exposed land on 80% of them, a stand-in for P14.T24.b's ocean fraction. Neither share is derived
from plan 14, whose η⊕ as built, 0.173 per FGK host (P14.T10.b, rulings 116.2–3 and 117.1), counts
planets in the zone, not `Temperate` bodies, and leaves out the M dwarfs. Every count scales
linearly with the placeholder. Within 100 ly, divide by about 750: some 400 `Temperate` bodies, 350
with life, 60 with complex cells, 45 with macroscopic life and 40 with land life.
`SubsurfaceOcean` bodies stop at `Origin`, their oceans dark: at w_v = 0.1, 57% of a population as
old as the thin disc's carry life, before the heat-flow gate and seeding. Such worlds may outnumber
rocky habitable-zone planets by a few orders of magnitude (Lingam and Loeb 2019c), so w_v decides
whether most of the Galaxy's life is under ice.

## What life is made of, and what it lives on

Later sections need three things from this one: what a biosphere is built from, where it can be
active, and how much it can make of what its world offers. The third is a closed form that [Biomes
as places](#biomes-as-places) turns into cover and biomass, [Organisms](#organisms) into food webs
and [Biosignatures](#biosignatures) into gas fluxes.

### Solvents

A solvent must stay liquid, carry the building blocks and let something like a membrane assemble.
The candidates, with liquid ranges from the NIST WebBook's phase data at 1 bar unless stated:

| Solvent                    | Liquid range                                                                                                                                    | Where                                                      | What chemistry is plausible                                                                                                                       | Evidence                                                                                                                                                 |
| -------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Water                      | 273–373 K; critical point 647 K and 221 bar                                                                                                     | Surface and subsurface oceans, the crust                   | All known life                                                                                                                                    | The only demonstrated case                                                                                                                               |
| Ammonia, and with water    | NH₃ 195 K to about 240 K, critical at 405 K and 113 bar; with water, liquid from the eutectic at 176 K, near 33% NH₃ by mass (Chua et al. 2023) | Icy-moon oceans, cold super-Earths                         | With water, water chemistry with an antifreeze; in pure ammonia an "ammonochemistry" is proposed (Bains 2004)                                     | No experimental biochemistry in pure ammonia                                                                                                             |
| Methane and ethane         | CH₄ 91–111 K, C₂H₆ 90–185 K                                                                                                                     | Titan's seas, which plan 14 gains with P14.T24.f and T51.a | Membranes of small nitriles, "azotosomes"                                                                                                         | For, by molecular dynamics (Stevenson, Lunine and Clancy 2015); against, by thermodynamics (Sandström and Rahm 2020) and experiment (Vu and Hodyss 2026) |
| Concentrated sulfuric acid | Droplets of 81–98% H₂SO₄ by mass (Seager S. et al. 2024)                                                                                        | Venus's clouds at 48–60 km                                 | Nucleobases survive a year in it (Seager S. et al. 2024), and 19 of 20 amino acids keep their backbones for four weeks (Seager M. D. et al. 2024) | Water activity ≤ 0.004, "two orders of magnitude below the 0.585 limit for known extremophiles" (Hallsworth et al. 2021)                                 |
| Supercritical CO₂          | Above 304 K and 74 bar                                                                                                                          | Venus's surface, deep crust, some super-Earths             | Enzymes are stable and selective in it (Budisa and Schulze-Makuch 2014)                                                                           | No proposed metabolism                                                                                                                                   |
| Hydrogen-rich atmospheres  | Not a solvent; supercritical H₂–He mixtures are named as one (Benner, Ricardo and Carrigan 2004)                                                | Sub-Neptunes, rogue planets                                | Water life under H₂: _E. coli_ and yeast "survive and grow in a 100% H₂ atmosphere" (Seager et al. 2020)                                          | Tolerance demonstrated; as a solvent, nothing                                                                                                            |

Titan, the cleanest test of an exotic solvent, has gone against it: azotosomes are "not viable
candidates for self-assembly" (Sandström and Rahm 2020), and their acrylonitrile co-crystallises
with ethane in the laboratory (Vu and Hodyss 2026). Venus's acid has proved kinder to biomolecules
than was believed, but its water activity rules out life as known, so a cloud biosphere would need a
genuinely new chemistry.

Which liquids a body holds is plan 14's to say, not this layer's: its condensate partition
(P14.T51.a) lists each body's surface liquids and ices by registry substance and phase, Titan's
methane sea and a cold CO₂ world's liquid CO₂ among them, with brines, `nh3_h2o` and `ch4_c2h6_n2`
as registry solutions (P14.T49.b), and P14.T24.g and T52.c put the acid deck aloft. **Lean:**
water–carbon is the generator's only default chemistry, wherever those lists, the subsurface-ocean
flag or crustal water hold liquid water or a water solution. Ammonia–water is water with an
antifreeze, liquid to 176 K, and the [limits of known life](#the-limits-of-known-life) still decide
where life is active in it. Methane–ethane and sulfuric-acid life are one speculative branch behind
one generator parameter, `exotic_solvents`, a multiplier on the origin rate of [Whether life
arises](#whether-life-arises-and-how-far-it-gets) in those two habitats, 0 by default ([open
question 60](#open-questions) asks how such a clock shares a body with water's). Like every
speculative parameter it is a universe setting, recorded in `universe.json` and shown where a
universe is created, never on a console, since the crew measures its world as it is ([open question
73](#open-questions)). Supercritical CO₂ and H₂–He are excluded outright: with no proposed
chemistry, any non-zero rate would be invention.

### Scaffolds

Carbon's case is its bonds: stable chains and rings with hydrogen, oxygen and nitrogen, and a gas,
CO₂, that keeps its cycle open. Silicon is the only serious rival, and the one systematic review
finds that "in no environment is a life based primarily around silicon chemistry a plausible
option": water locks it into silica, cryosolvents dissolve almost nothing, and sulfuric acid
appears to support a far broader organosilicon chemistry than water (Petkowski, Bains and Seager
2020). **Lean:** carbon only, with no silicon branch even when `exotic_solvents` is on.

### The limits of known life

| Limit               | Value                                                                                                                                                                                                                                                                                 | Source                                                                                                                 |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| Highest temperature | Growth at 122 °C under 20 MPa, 116 °C at 0.4 MPa (_Methanopyrus kandleri_ 116)                                                                                                                                                                                                        | Takai et al. 2008                                                                                                      |
| Lowest temperature  | Growth at −15 °C in 18% salt and activity to −25 °C (_Planococcus halocryophilus_); continuous growth on frozen media to −17 °C; activity at −20 °C in permafrost; free cells vitrify at −10 to −26 °C, and "few multicellular organisms" complete their life cycle below about −2 °C | Mykytczuk et al. 2013; Panikov and Sizova 2007; Rivkina et al. 2000; Clarke et al. 2013                                |
| Water activity      | Germination and cell division of _Aspergillus penicillioides_ at 0.585; halophilic archaea grow at 0.635, 0.611 the extrapolated prokaryotic floor; saturated NaCl is 0.755                                                                                                           | Stevenson et al. 2017; Stevenson et al. 2015                                                                           |
| Chaotropic salts    | No active cells above about 2.3 M MgCl₂ "in the absence of compensating (e.g. kosmotropic) solutes"; sulfate reduction reported at a water activity near 0.4 in a deep-sea MgCl₂ brine, contested and not adopted                                                                     | Hallsworth et al. 2007; Steinle et al. 2018                                                                            |
| pH                  | About 0 (_Picrophilus_, "around pH 0"; −0.06 in Merino et al.'s table) to 12.5 (_Serpentinomonas_)                                                                                                                                                                                    | Schleper et al. 1995; Merino et al. 2019                                                                               |
| Pressure            | Growth to 125 MPa (_Thermococcus piezophilus_); formate oxidation reported to 1,060 MPa and viability in ice-VI inclusions at 1,200–1,600 MPa, disputed; survival of gigapascal pressures after directed evolution                                                                    | Dalmasso et al. 2016, as Merino et al. 2019 give it; Sharma et al. 2002, disputed by Yayanos 2002; Vanlint et al. 2011 |
| Ionising radiation  | Survival of 30 kGy of γ rays (_Thermococcus gammatolerans_)                                                                                                                                                                                                                           | Jolivet et al. 2003                                                                                                    |

These are properties of water and macromolecules more than of a lineage: "there is a common
water-activity limit that is determined by physicochemical constraints for the three domains of
life" (Stevenson et al. 2015), and the cold limit is the cytoplasm's glass transition (Clarke et al.
2013). Growth is demonstrated to 125 MPa, while liquid water at 0 °C persists to about 630 MPa
before ice V forms, and only above the ice V–VI–liquid triple point, at 273.31 K and 632.4 MPa, does
it border ice VI (IAPWS R14-08(2011)), so seafloors between about 125 and 630 MPa are liquid but
beyond any demonstrated growth.

**Lean:** one growth envelope for every water–carbon biosphere:

- liquid water from T_min to 122 °C, T_min a parameter over −15 to −20 °C defaulting to −15 °C, the
  pure-culture growth record;
- water activity at least 0.611, the prokaryotic floor, before `ComplexCells`, and 0.585 once
  eukaryote-like cells exist, since the lower figure is a fungus's;
- MgCl₂ brines under about 2.3 M without kosmotropic solutes, other chaotropic salts needing their
  own limits;
- pH about 0 to 12.5, where a habitat model gives a pH;
- pressure up to `max_growth_pressure`, a parameter defaulting to 125 MPa, because every other limit
  here is one of demonstrated growth and the reports above it are of activity or survival, one of
  them disputed. It bounds the inhabited volume, not the habitat: at a Europa's 1.31 m s⁻², 125 MPa
  lies some 95 km down (computed), near the seafloor, so it trims the deepest water only;
- ionising radiation only on exposed surfaces, through a shielding depth.

The test runs per coarse cell and month on R09's climate, the share of months passing being the
growing season. Brine against ice has the water activity of ice's vapour pressure over supercooled
water's, 0.86 at −15 °C and 0.585 only near −59 °C (computed from Murphy and Koop 2005's fits;
Hallsworth et al. 2021 put ice's water activity below 0.585 under −59 °C), so temperature binds
first in cold brines. Passing is not P14.T25's `Temperate`, which says nothing about life; dormancy
is [open question 58](#open-questions)'s concern and transit [Panspermia within a
system](#panspermia-within-a-system)'s.

### Chirality and alien biochemistry

Earth's life builds proteins from L-amino acids and nucleic acids from D-sugars, from one small set
of building blocks, and an independent origin need share neither the set nor the hand. Abiotic
synthesis is racemic, yet a bias can arise before life: aqueous alteration on meteorite parent
bodies left L-isovaline in excess by 18.5 ± 2.6% in Murchison and 15.2 ± 4.0% in Orgueil (Glavin and
Dworkin 2009). Whether such a bias spans a galaxy or one forming system is unknown.

The crew is human (the owner's ruling of 2026-10-10), so the question is what alien biochemistry
does to a human body:

- **Nutrition.** Human enzymes are stereospecific, and the mirror sugar L-glucose "is an
  unmetabolizable sugar except in some bacteria" (Ono, Takigawa and Yamada 2020). Opposite-handed
  biomass is no food, at most its achiral small molecules yielding energy, and same-handed alien
  biomass lacks the amino acids, vitamins and fatty acids a human must eat unless the repertoires
  happen to coincide, a chance the ladder below does not model.
- **Toxicity.** Molecules shaped for alien receptors are mostly inert at human ones, while generic
  chemistry acts whatever the hand ([Hazards and use](#hazards-and-use)).
- **Infection.** Unlikely, but not impossible; what that means for a landing party is under
  [Biohazard realism](#biohazard-realism).

**Lean:** each independent origin carries a `Biochemistry` record: solvent, scaffold, a handedness
bit drawn with probability `p_left` on `body.life.biochemistry`, a word per epoch keyed by its
window and attempt, and a lineage key, the origin's identity. `p_left` is a parameter defaulting to
0.5, since the meteoritic bias may be regional. Panspermia draws nothing: the seeded epoch copies
its donor's `Biochemistry` whole. The record's compatibility with human biochemistry is a ladder:
shared lineage, the only case that could feed a human; same or opposite hand, inedible; another
solvent or scaffold, inert but for generic chemistry. [Organisms](#organisms) judges toxins by it,
and [Knowledge, surveys and the landing party](#knowledge-surveys-and-the-landing-party) what the
crew can learn and must do.

### Energy: light

Oxygenic photosynthesis on Earth "is limited to photosystems that operate at 400–730 nm", while
anoxygenic photosynthesis reaches 1,015–1,020 nm with bacteriochlorophyll _b_ (Kiang et al. 2007a),
and oxygenic charge separation has been shown with chlorophyll _f_ at 745 nm in photosystem I and
chlorophyll _f_ or _d_ at 727 nm in photosystem II (Nürnberg et al. 2018). What a host's spectrum
means for those bands, computed from blackbodies at Earth's 1,361 W m⁻²:

| Host T_eff | 400–700 nm, µmol m⁻² s⁻¹ | 400–750 nm, µmol m⁻² s⁻¹ | 700–1,100 nm, µmol m⁻² s⁻¹ | 400–750 nm, Sun = 1 |
| ---------- | ------------------------ | ------------------------ | -------------------------- | ------------------- |
| 2,600 K    | 285                      | 429                      | 2,087                      | 0.16                |
| 3,000 K    | 548                      | 773                      | 2,651                      | 0.29                |
| 3,500 K    | 947                      | 1,260                    | 3,080                      | 0.47                |
| 5,772 K    | 2,277                    | 2,683                    | 2,747                      | 1.00                |

A cool star's habitable zone also receives less than Earth's insolation, and the factors multiply:
"the flux of 400–750 nm photons may be just a few percent that of Earth's", making the outer zone of
a TRAPPIST-1-like star light-limited where Earth is nutrient-limited, and longer wavelengths would
not close the gap (Lehmer et al. 2018); below about 0.2 M☉ Earth-like productivity may be out of
reach (Lingam and Loeb 2019a). Light-limited is not lifeless: cyanobacteria under a simulated
M-dwarf spectrum grew and photosynthesised much as under sunlight (Battistuzzi et al. 2023).

**Lean:** the photon yield per joule in a band, q(T_eff, log g; band), is an offline table from the
PHOENIX library, which spans 2,300–12,000 K and log g 0–6 (Husser et al. 2013), since M-dwarf
spectra are far from blackbodies, which serve as its test, with a cooler grid for brown dwarfs,
read at plan 06's state at age + t and summed over a body's hosts. Hosts it does not span take a
blackbody at their T_eff, its own test: white dwarfs, whose log g near 8 lies beyond PHOENIX's 0–6,
hosts above 12,000 K, and a giant parent's glow, which plan 14 counts as a moon's host (P14.T17.b).
The table gives each host's light above the air, and productivity reads that light as it reaches
the ground ([The energy budget, in closed form](#the-energy-budget-in-closed-form)). The oxygenic
band is 400–750 nm, as Lehmer et al. and Lingam and Loeb (2019a) take it, and the anoxygenic
400–1,100 nm, the long limit Kiang et al. (2007a) consider possible. The light gate on
`OxygenicPhotosynthesis` in [Whether life arises](#the-transitions-and-whether-each-is-hard) is the
growth floor: ice algae began growing below 0.17 µmol photons m⁻² s⁻¹ (Hancke et al. 2018), orders
below any habitable-zone surface, so light gates the stage only on dark worlds and under thick ice.
The pigment within the band is [Biomes as places](#biomes-as-places)'.

### Energy: chemistry and radiolysis

Where light does not reach, life runs on redox disequilibrium: H₂ from serpentinisation, radiolysis
and volcanism, CH₄, reduced sulfur and ferrous iron, against O₂, sulfate, nitrate, ferric iron or
CO₂. Earth's marine lithosphere yields about 10¹¹ mol of H₂ a year, and the Precambrian continents
0.36–2.27 × 10¹¹ more (Sherwood Lollar et al. 2014); 2.8 km down in South African basalt one sulfate
reducer lives on geological H₂ and sulfate with "no apparent reliance on photosynthetically derived
substrates" (Lin et al. 2006). A reaction must yield some 20–25 kJ per mole of reaction to be
harnessed (Schink 1997). Fixing a mole of carbon costs about 1,060 kJ from H₂, which needs no
reverse electron transport, and 3,500 kJ from sulfide or ferrous iron (Heijnen and van Dijken 1992,
as Böhnke et al. 2019 give them, the original not re-checked), in line with "only about 10% or
less" of a chemolithoautotroph's energy reaching biomass (McCollom and Amend 2005). A starved
sediment's cells need about 190 zW each, with a minimum near 1 zW (LaRowe and Amend 2015), and turn
over "on timescales of centuries to millennia" (Hoehler and Jørgensen 2013).

### The energy budget, in closed form

Photosynthetic productivity per coarse cell and month takes CASA's light-use-efficiency form, in
which temperature scales the efficiency of absorbed light and water limits it, and which reproduced
Earth's land NPP, 48 Pg C a year at a maximum of 0.39 g C per MJ of absorbed light (Potter et al.
1993). It is counted in photons, so that it holds for any host, and keeps the structure of Lieth's
Miami model, a temperature-limited rate against a water-limited one (Lieth 1975, as Bertsch-Hoermann
et al. 2021 give it), with nutrients as a third limit. This is the form's one statement, which
[Biomes as places](#biomes-as-places) uses by reference:

**P = min(f_T × P_L, P_W, P_N) × f_C**, in mol C m⁻² s⁻¹, and zero in a month outside the envelope.

- P_L = ε_q × a × Φ, where Φ = Σᵢ Fᵢ × q_s,i is the photon flux in the band at the surface, Fᵢ
  each host's surface shortwave flux and q_s,i the band's yield per joule of host i's light at the
  ground of the abiotic record, so that the cover the second pass reads stays acyclic ([request
  31](#requests-to-the-earlier-layers)). The host's own yield q(T_eff,i; band) would not do: the
  air takes more near-infrared than visible light, so on Earth it counts about a fifth too few
  photons (computed, from an assumed 185 W m⁻² of mean surface shortwave, 46% of it at 400–700 nm,
  both from memory). a is the share the cover absorbs (1 for the potential), and ε_q = 0.0060 mol C
  per mole of absorbed 400–750 nm photons: CASA's 0.39 g C MJ⁻¹ at the Sun's 4.57 mol of 400–700 nm
  photons per MJ, divided by its 1.18 ratio of 400–750 to 400–700 nm photons (both computed for a
  5,772 K blackbody). The ceiling is 4.6% of sunlight into biomass for C3 plants and 6% for C4 (Zhu,
  Long and Ort 2008). For cool hosts P_L is an upper bound: it values a far-red photon as a red one,
  while chlorophyll's absorption out to 730 nm is "just the tail end … and presumed not to
  contribute a large amount" (Kiang et al. 2007a), so it holds only for a biosphere with far-red
  pigments, such as Earth's chlorophyll _f_ cyanobacteria.
- f_T = 1 ÷ (1 + e^(1.315 − 0.119 T)) for T in °C, and P_W = 3,000 × (1 − e^(−0.000664 W)) g dry
  matter m⁻² yr⁻¹ for a water supply W in mm per 365.25 d, taken as 0.5 g C per g of dry matter and
  converted to mol C m⁻² s⁻¹ at 12.011 g mol⁻¹ and 365.25 d: Lieth's two terms, Earth-fitted and
  labelled heuristic, applied to monthly rates as R09 applies Köppen's thresholds. f_T scales the
  light-limited rate only, as CASA's temperature scalar does. Since f_T rises to 1 and the
  envelope's 122 °C is a chemotroph's, P_L is zero above a phototroph ceiling, a parameter
  defaulting to 73 °C, the upper limit of photosynthesis in hot springs world-wide (Castenholz 2015;
  Kees et al. 2022).
- P_N = k_P × S ÷ S⊕ mol C m⁻² s⁻¹, S the cell's phosphorus supply (the world's, in the body-level
  form), S⊕ Earth's mean and k_P the one constant the reference Earth fits. S is, on land,
  weathering of exposed land, from each cell's `steepness`, `drainage` and `runoff` in the quantised
  field in closed form, or failing one a server-resolved factor in the `LifeTable`; at sea, the
  upwelling and river-outlet supply of [Oceans, ice and shallow
  water](#oceans-ice-and-shallow-water); and everywhere a hydrothermal floor. Without land,
  phosphorus holds a water world's O₂ to levels indistinguishable from abiotic ones (Glaser et al.
  2020); a subsurface ocean may hold P "a few orders of magnitude" below Earth's (Lingam and Loeb
  2018), yet Enceladus's plume grains show "at least 100-fold" more (Postberg et al. 2023), so the
  floor is a habitat input. Plan 14's element inventory (P14.T50.a) carries no phosphorus for a
  rocky body yet.
- f_C is a ramp in CO₂ partial pressure, as at 1 bar, set by each growth strategy's carbon pathway
  ([Vegetation as structure](#vegetation-as-structure)): for a strategy without a
  carbon-concentrating pathway the C3-like ramp r₃, from 0 at 150 ppm, which Caldeira and Kasting
  (1992) call "the critical level for C3 photosynthesis", to 1 at 300 ppm, within 280–400 ppm; for
  one with it the C4-like ramp r₄, from 0 at their C4 floor of 10 ppm to 1 at 150 ppm, this
  section's choice of upper end. The sim's body-level form takes their mixture, (1 − s₄) r₃ + s₄ r₄,
  s₄ the C4 share of oxygenic productivity, a parameter: C4 plants carry about 23% of Earth's land
  gross productivity (Still et al. 2003), and the ocean's share, where most phytoplankton
  concentrate carbon (from memory), is not researched. f_C applies to any oxygenic biosphere.

The stage ([Whether life arises](#whether-life-arises-and-how-far-it-gets)) decides which terms
exist. Before `OxygenicPhotosynthesis` the band is the anoxygenic one and the world's total is
capped at Σ_d F_d × n_d ÷ 4, F_d the supply of each electron donor, n_d its electrons a mole and
four to a fixed carbon: Earth's early marine NPP was "lower than in the modern oceans by a factor of
at least 40" (Kharecha, Kasting and Siefert 2005). Water and wet ground carry microbial productivity
from the first phototrophs, dated by the epoch's `phototrophy` field, and land carries plants only
from `Land`.

Chemotrophs are counted per habitat: P_chem = Σ_r R_r × |ΔG_r| ÷ G_r mol C s⁻¹ over reactions
yielding over 20 kJ per mole of reaction as written (Schink's basis; per CH₄ formed, for
methanogenesis), R_r the lesser of donor and oxidant supply in stoichiometry, scaled from plan 14's
present outgassing relative to Earth's (P14.T24.b), crust and redox class, and G_r 1,060 kJ mol⁻¹
where no reverse electron transport is needed (H₂) and 3,500 where it is (sulfide, Fe²⁺ and, until
researched, any other donor), and the population that supply carries at maintenance is
N = Σ_r R_r × |ΔG_r| ÷ 190 zW. These drive plan 14's biosphere hook (P14.T53.d), whose net surface
fluxes of O₂, CH₄ and N₂O are in units of Earth's biotic flux, the present one as drafted and the
pre-industrial one once [request 2](#requests-to-the-earlier-layers) is taken: the body's
integrated P over Earth's 104.9 Pg C a year is their scale, with a mole of CH₄ for every four of H₂
a methanogen burns and an O₂ for each carbon fixed by oxygenic photosynthesis, and [A biosphere
changes its planet](#a-biosphere-changes-its-planet) nets them against burial.

| Earth, the calibration point | Value                                                                                                     | Source                                                  |
| ---------------------------- | --------------------------------------------------------------------------------------------------------- | ------------------------------------------------------- |
| Global NPP                   | 104.9 Pg C yr⁻¹, "roughly equal contributions from land and oceans", about 56.4 and 48.5                  | Field et al. 1998                                       |
| Land NPP, other methods      | 48 Pg C yr⁻¹ (CASA); 36–74 Pg C yr⁻¹ across statistical climate–NPP models                                | Potter et al. 1993; Zaks et al. 2007                    |
| Standing biomass             | About 550 Gt C: plants 450, bacteria 70, archaea 7, mostly deep; prokaryotes revised to 23–31 Pg C in all | Bar-On, Phillips and Milo 2018; Magnabosco et al. 2018  |
| Photosynthetic power         | About 130 TW: 8.7 × 10¹⁵ mol C yr⁻¹ at about 478 kJ per mole fixed into sugar                             | Computed, from glucose's 2,870 kJ mol⁻¹, not re-checked |
| Geological H₂                | About 10¹¹ mol yr⁻¹ from the marine lithosphere: under 1 GW even if all burnt with O₂ at 237 kJ mol⁻¹     | Sherwood Lollar et al. 2014; the power computed         |

The last two rows are the design's central contrast: photosynthesis gives Earth's biosphere some 10⁵
times what geology could, so a biosphere that cannot photosynthesise stays small however long it
lives.

**Lean:** the per-strategy kernel of P is `hyperion_surface::life`'s, evaluated on both sides at the
coarse level L − 1, and [Biomes as places](#biomes-as-places) turns it into cover, canopy and
biomass without applying water again. The sim's `hyperion_sim::life::energy` keeps the body-level
form, which the snapshot path needs without a field, with P_chem and N once per habitat, and
resolves the photon yields at the ground, from PHOENIX's spectra, into the `LifeTable`. The
reference Earth is the acceptance test, stated here once for the whole layer: on R09's reference
climate, land within Zaks et al.'s 36–74 Pg C a year and the ocean within a factor of two of Field
et al.'s 48.5, with k_P the one constant fitted to it. Checked against this form with assumed
annual-mean climates (computed: 35, 23 and 18 mol m⁻² d⁻¹ of 400–700 nm light at the surface, 26,
8 and 0 °C, 2,500, 800 and 500 mm of water, a = 1, P_N and f_C not binding), a wet tropical cell
gives about 930 g C m⁻² yr⁻¹, light-limited, a cool temperate one 290 and a boreal one 120, where
Miami's lesser term gives 1,215, 615 and 317. Evaluated by month, as the form is, a seasonal cycle
raises the cool cells to some 330–370 and 190–240 (computed, with sinusoidal seasons of ±10 °C for
the cool temperate cell and ±17.5 °C for the boreal, light ±15 mol m⁻² d⁻¹ in phase, and months
below the envelope's −15 °C excluded; the lower figures spread the year's water evenly over its
months, so that it binds in summer), so where the reference Earth lands in the range is not
established. The test stands; if it fails low, the remedy is f_T's shape, not ε_q, which is CASA's.
Productivity is quoted in g C m⁻² yr⁻¹ and biomass in g C m⁻², and consoles show both in the units
[What the guide must gain](#what-the-guide-must-gain) proposes, with carbon named by the label.

## Habitats beyond the surface

Most of the Solar System's habitable volume is not a surface, so on the owner's prior most of a
galaxy's biospheres may be here, each tested on the [envelope](#the-limits-of-known-life), with
detection left to [Biosignatures](#biosignatures).

### Subsurface oceans under ice

Enceladus's plume carries 0.4–1.4% H₂ beside 0.3–0.8% CO₂ (Waite et al. 2017, as Taubner et al. 2018
tabulate it), a disequilibrium that "favors the formation of methane from CO₂" (Waite et al. 2017);
serpentinisation may supply 0.6–34 mol of H₂ a second, enough for 4 × 10⁴–2 × 10⁶ kg of biomass a
year and 80–4,250 cells per cm³ of ocean (Steel, Davila and McKay 2017). The closed form agrees in
order: at most 33 kJ per mole of H₂, methanogenesis's standard-state yield, over 1,060 kJ mol⁻¹
gives 7 × 10³–4 × 10⁵ kg C a year (computed). Europa's irradiated ice could bring its ocean to "O₂
concentrations comparable to those found in terrestrial surface waters" even with some 10⁹ mol a
year of hydrothermal reductants consuming most of it (Hand, Carlson and Chyba 2007), and its
water–rock H₂ is about a tenth of Earth's, with an oxidant flux of the same order (Vance, Hand and
Pappalardo 2016). Burning 10⁹–10¹⁰ mol of H₂ a year with oxygen gives 2 × 10⁸–2 × 10⁹ mol C, some
3 × 10⁻⁸ to 3 × 10⁻⁷ of Earth's NPP (computed).

Plan 14 has the start: a regular moon's tidal heat sets a conducting shell D = 651 ln(273 K ÷ T_s) ÷
F thick, the moon has an ocean when that is thinner than its water layer
(`moons::regular::has_subsurface_ocean`), and P14.T25 classes it `SubsurfaceOcean`; P14.T56.c
(planned) gives each moon its own water, carbon and nitrogen ranks, the nitrogen boosted only where
it formed below ammonia hydrate's 131 K, and P14.T56.d a Love number by state, whose heated branch
the flag selects and which leaves the flag unchanged. Missing: shell thickness and ocean depth as
outputs; the test reading T24.b's radiogenic heat beside the tidal, and the test for planets and
rogues; T49.b's salt and ammonia eutectics in the test; whether high-pressure ice parts ocean from
rock, ending serpentinisation and phosphorus; seafloor heat flux; and surface radiation dose, which
makes oxidants. Until a later ruling on P14.T17.a's finding F2, a generated Europa analogue forms
dry and has no ocean. **Lean:** populated by default, chemotrophic, with productivity from its
water–rock supply; any cap on stage is [Whether life
arises](#whether-life-arises-and-how-far-it-gets)'s ([open question 59](#open-questions)). It is
detectable through a plume or surface deposits, otherwise only by drilling.

### The deep crust

The continental subsurface holds 2–6 × 10²⁹ cells (Magnabosco et al. 2018) and subseafloor sediment
2.9 × 10²⁹, about 4.1 Pg C (Kallmeyer et al. 2012), on geological H₂ and, under a surface biosphere,
buried organic carbon. Magnabosco et al. predict cell numbers from heat flow, surface temperature,
depth and lithology, and the 122 °C isotherm is the floor, at z = k × (122 °C − T_s) ÷ q. Continents
lose 65 mW m⁻² and oceans 101, 87 on average (Pollack, Hurter and Johnson 1993), which with T_s =
15 °C and an assumed k of 2.5 W m⁻¹ K⁻¹ puts the floor about 4 km under a continent. Mars loses
14–25 mW m⁻², 19 on average (Parro et al. 2017): under a −60 °C surface it has liquid only below
some 8 km and life to about 24 km (computed, same k), the pressure gate reading pore water's
hydrostatic pressure, some 89 MPa there, so a dry surface is not a dead crust. Plan 14 lacks the
crust's water, and porosity is its readers' to state (P14.T49.b). **Lean:** populated by default
wherever crustal water lies in the envelope, its H₂ scaled by plan 14's heat flow, radioactives
(P14.T26) and crust (P14.T51.c, whose ultramafic and komatiitic rock is what serpentinises), and
the one habitat a rocky world keeps where its surface fails; whether it begins life or only
shelters it is [open question 59](#open-questions)'s. From orbit it shows, if at all, as seeping
gas.

### Ice shells and brines

Liquid veins between ice grains can keep at least a cell per cm³ in Antarctic glacial ice (Price
2000), and Europa's chaos terrain suggests water lenses as shallow as 3 km (Schmidt et al. 2011).
Cold brines mostly fail: Mars's surface brines form only as the lowest eutectics, under 225 K and
outside known tolerances (Rivera-Valentín et al. 2020). **Lean:** not a habitat of its own: lenses
and the ice–ocean interface belong to the ocean whose energy they draw, and surface brines to the
monthly envelope test.

### Cloud decks

Venus's clouds at 48–60 km have Earth-surface temperatures (Seager M. D. et al. 2024), sunlight and
a proposed life cycle that outlasts droplet settling as desiccated spores in the lower haze at 33–48
km (Seager et al. 2021), but fail on water activity by two orders of magnitude (Hallsworth et al.
2021). Plan 14's drafted P14.T24.g gives a runaway world its H₂SO₄ deck on T24.e's profile, which
locates the habitat. Earth's water clouds keep transported cells unfrozen (Clarke et al. 2013), but
only while a droplet lasts. **Lean:** off by default; an acid cloud biosphere exists only under
`exotic_solvents`, a speculative universe setting ([Solvents](#solvents)). Phosphine and the
clouds' unidentified ultraviolet absorber are [Biosignatures](#biosignatures)' question.

### Giant-planet atmospheres

Sagan and Salpeter (1976) worked out a Jovian ecology of photosynthetic "sinkers", "floaters" that
hold their pressure level, "hunters" and "scavengers" at almost pyrolytic depths, and concluded that
niches for the first three "appear to exist"; Jupiter's clouds pass on water activity (Hallsworth et
al. 2021). Against them: convection carries particles down to destruction, rock-forming elements
have condensed far below, and sunlight at 5.2 au is about 50 W m⁻² above the clouds (computed). Plan
14's condensation loop (P14.T52.c, with T24.d's decks) will place a giant's water deck, which
locates the layer. **Lean:** excluded by default; floaters exist only under a second speculative
universe setting, `giant_atmosphere_life`, off. Nothing reliable would detect them.

### Rogue planets and tidally heated moons

An ejected Earth-mass body can keep 10²–10⁴ bar of hydrogen whose collision-induced opacity holds
its surface above water's melting point on radiogenic heat, at an effective temperature near 30 K
(Stevenson 1999); without one, a subglacial ocean survives in a rogue of about 3.5 M⊕, or 0.3 M⊕
with ten times the water or a thick frozen atmosphere (Abbot and Switzer 2011), and around a Sun,
40 bar of H₂ keeps a 3 M⊕ planet at 280 K out to 10 au (Pierrehumbert and Gaidos 2011). A rogue
giant's moon can hold surface water under CO₂, tides its main heat (Ávila et al. 2021). All are
dark, so chemotrophic, with supply scaling as the crust's with heat flow, and burn their H₂ with
CO₂ and radiolytic oxidants rather than photosynthetic O₂.

Plan 14 gives a rogue internal heat and "a hydrogen envelope's insulation where one survives"
(P14.T27.b), and P14.T53.a will keep H₂ and He below `THIN_ENVELOPE_FRACTION` as air with an H₂
collision-induced greenhouse calibrated on Pierrehumbert and Gaidos. Above that fraction, 0.1% of
the mass or about 1,100 bar on an Earth (computed from P = g × column mass), a body is still
`GasEnvelope`, with no surface, so Stevenson's thicker cases stay hidden. Pressure matters to life
too: 10⁴ bar is 1 GPa, eight times `max_growth_pressure`'s default. **Lean:** populated by default
where plan 14's surface conditions pass the envelope, as any water habitat is; plan 14 records no
history before ejection (P14.T27.b), so as specified a rogue's life begins in its buried habitats;
both points are [open question 59](#open-questions)'s. They show only in the far infrared at close
range (Abbot and Switzer 2011).

### What is populated by default

| Habitat                   | Energy                          | Bound, against Earth's 105 Pg C a year                                                                | Plan 14 today                         | Default                      |
| ------------------------- | ------------------------------- | ----------------------------------------------------------------------------------------------------- | ------------------------------------- | ---------------------------- |
| Surface land and ocean    | Light, then nutrients and water | The closed form                                                                                       | Surface conditions, R09's climate     | On                           |
| Subsurface ocean          | Water–rock H₂, surface oxidants | 10⁻¹⁰ (Enceladus) to 3 × 10⁻⁷ (Europa), computed                                                      | A moon flag only                      | On, chemotrophic             |
| Deep crust                | Radiolytic and hydration H₂     | 10⁻⁷ to 10⁻⁵ on Earth's H₂, computed                                                                  | Heat flow; no porosity or groundwater | On, chemotrophic             |
| Ice veins and lenses      | Their host habitat's            | Part of the host                                                                                      | Nothing                               | Within the host              |
| Cloud decks               | Light, but water activity fails | Unknown                                                                                               | Acid deck drafted (P14.T24.g)         | Off: `exotic_solvents`       |
| Giant atmospheres         | Weak light, no nutrients        | Unknown                                                                                               | Water deck from P14.T52.c             | Off: `giant_atmosphere_life` |
| Rogue planets, dark moons | Radiogenic and tidal heat       | As the deep crust by heat flow, at methanogenesis's yield without O₂: some 10⁻⁶ at Earth's crustal H₂ | Thin H₂ air from P14.T53.a            | On, where a surface exists   |

## A biosphere changes its planet

This section says what each stage does to its world; when a world reaches each stage, the time O₂
first reaches p_cx (10³ Pa) and the dated first phototrophy are [Whether life arises, and how far it
gets](#the-transitions-and-whether-each-is-hard)'s. PAL here is the present atmospheric level,
Earth's 21 kPa of O₂. Under the owner's prior the commonest living world is microbial, so the
anoxic, Archean-like world is the main case here, the oxygenated microbial world the next, and
complex land life the rare one.

**What the biosphere hands to plan 14** is biological rates alone. **Lean:** a CH₄ surface flux, an
O₂ net source (the organic carbon buried, whose O₂ is left behind) and an N₂O flux, with a CO sink
and optionally CH₃Cl and the sulphur gases for [Biosignatures](#biosignatures), each in units of
Earth's pre-industrial biotic flux, which request 2 asks P14.T53.d, drafted in present units, to
adopt; the weathering factor f_bio, 1 on a lifeless world; a cover term, whose area and albedo
[Biomes as places](#biomes-as-places) sets as body-level forms of the stage under the abiotic
record's light, so that nothing the second pass makes feeds them; and the branch, anoxic or oxic.
Each is drawn on `body.life.gases` within the bounds below and held for the stage, and plan 14
turns them into gases, greenhouse, albedo and temperature inside the three passes that iterate its
Bond albedo, never into another `SurfaceState`.

### Before oxygen: the commonest living world

**Chemotrophs and anoxygenic phototrophs.** Without oxygenic photosynthesis a biosphere lives on
what geology supplies. Kharecha, Kasting and Siefert (2005) found its CH₄ production and net primary
productivity "limited by the downward flux of H₂, CO, S₈, and H₂S through the atmosphere–ocean
interface and by the upwelling rate of Fe²⁺", the productivity at least 40 times below the modern
ocean's and the CH₄ flux about a third to 2.5 times the modern one. In anoxic air methane is
photolysed high in the atmosphere: its hydrogen escapes, and its carbon returns as CO₂ or, above the
haze threshold, as hydrocarbons, the sink P14.T24.c now states, with the reservoir resupplied, here
by the biosphere. **Lean:** the CH₄ flux is a drawn share of plan 14's reductant supply, normalised
so that an Archean Earth's supply spans Kharecha's range; until P14.T55.a, still unscheduled,
supplies it, the share is of a stand-in supply drawn in the same range ([open question
74](#open-questions)). This branch releases no O₂ and no N₂O, whose denitrification needs the
nitrate that oxidants make. Methane's abundance follows from P14.T53.a's diffusion-limited hydrogen
escape, with methane's hydrogen counted in the total mixing ratio, f(H₂) + 2f(CH₄) (Catling, Zahnle
and McKay 2001), not from T53.a's new term for methane escaping whole through nitrogen, which is for
Titan-like airs. Kharecha's fluxes give 100 to 35,000 ppmv "depending on the rate at which hydrogen
escaped to space", the diffusion limit giving the lower values, though they note that calculations
of slow escape "favour the higher CH₄ concentrations", while the xenon record favours rapid escape
(Catling and Zahnle 2020); Catling, Zahnle and McKay (2001) give 10²–10³ ppmv.

**Oxygenic photosynthesis before oxygenation.** With water as the electron donor productivity is
limited by nutrients instead, but volcanic and hydrothermal reductants consume the O₂, and the air
stays anoxic: Archean O₂ was under 10⁻⁶ PAL, CO₂ some 10 to 2,500 times modern and CH₄ 10² to 10⁴
times, enough to offset the fainter Sun at 0–40 °C (Catling and Zahnle 2020). The CH₄ flux continues
from fermented organic matter, drawn in the same range, and its photolysis lets hydrogen escape and
leaves oxygen behind, some 10¹²–10¹³ mol yr⁻¹ (Catling, Zahnle and McKay 2001), slowly oxidising the
crust.

**The greenhouse and its limit.** Methane is the main case's biological greenhouse gas, and its
warming saturates: above a mixing ratio of 10⁻³ its shortwave absorption diminishes it (Byrne and
Goldblatt 2015), and Haqq-Misra et al. (2008), correcting earlier absorption coefficients, found the
late Archean needed about 0.03 bar of CO₂ besides. Above CH₄/CO₂ of about 0.1–0.2, the threshold
P14.T24.c drafts, an organic haze forms that "can cool the planet's surface by about 20 K", limits
itself and cuts surface ultraviolet by about 97% (Arney et al. 2016). **Lean:** plan 14's grey τ
gains a temperate CH₄ term fitted offline to published radiative grids, capped above 10⁻³ and zero
at Titan's temperatures, so that the nitrogen term fitted on Titan keeps Titan's methane
(P14.T24.f), while the collision-induced term P14.T13.c's findings ask for methane-dominated cold
airs stays separate; and the haze's cooling enters the snapshot path as a closed form of CH₄/CO₂
and, on N₂–CH₄ airs, of CH₄, as T24.c's two thresholds have it. Both are stated exceptions to
P14.T57.a's rule that every greenhouse constant is fitted on a measured state, since no measured
world holds a temperate methane greenhouse, and both join the refit of every constant that
P14.T53.a's new CO₂ form makes in the 23 → 24 batch, with T53.d. Left on the Surface path, which
never feeds back, the haze would misstate the commonest living world's climate by up to 20 K.

**The ground.** From the first phototrophy, microbial crusts darken bare ground: on the Colorado
Plateau the death of moss and lichen crusts raised the albedo by over 30% (Rutherford et al. 2017),
which over half an Earth's land under 0.67 cloud is a planetary change of a few thousandths
(computed here). f_bio stays 1, since no global estimate of microbial weathering was found: a
parameter and [open question 32](#open-questions).

### The rise of oxygen

**The branch.** Oxygen rises when the flux of reducing gases "falls below the flux of organic carbon
burial that produces O₂" (Claire, Catling and Zahnle 2006). The switch is fast because ozone's
ultraviolet shield lengthens O₂'s lifetime nonlinearly, which makes a low state below 10⁻⁵ PAL and a
high one above 5 × 10⁻³ PAL both stable (Goldblatt, Lenton and Watson 2006). **Lean:** the branch is
plan 14's comparison of the O₂ sources, biological and abiotic, with its reductant sink, a closed
form at any age, evaluated on the abiotic snapshot under the forcing of the stages already reached
(request 7), so oxygenation falls at the first time after oxygenic photosynthesis that the sink
drops below the source, found as a bracketed root in age; until plan 14's reductant supply lands,
the stand-in delays of [The transitions, and whether each is
hard](#the-transitions-and-whether-each-is-hard) time it. Krissansen-Totton, Kipp and Catling (2021)
allow a rising source as well as a falling sink: fractional organic burial may have risen two to
five times since the Archean, whose absolute burial was comparable to modern.

**The level.** Earth's record is an initial rise, a delayed build-up in the ocean and near-modern
levels two billion years later (Lyons, Reinhard and Planavsky 2014), and sources disagree about the
plateau between. Planavsky et al. (2014) put the mid-Proterozoic at most 0.1% PAL from chromium
isotopes; Daines, Mills and Lenton (2017) find a strong regulating feedback near 0.1 PAL that loses
stability below 0.01; Gregory, Claire and Rugheimer (2021), as Cooke et al. (2022) report them, find
levels between 3 × 10⁻⁸ and 10⁻² PAL unstable; and Cooke et al. take 10⁻³ to 1.5 PAL as the range
since the Great Oxidation. **Lean:** three oxic regimes, each drawing its source so that the
regulated level falls in a band and each beginning at a time the stage record dates, so that oxygen
and stage cannot disagree. From oxygenation to the p_cx crossing the band runs from 10⁻² PAL to
p_cx, about 0.05 PAL, siding with the two stability analyses against the one proxy that would lie in
their unstable band, with Planavsky's 0.1% the recorded alternative; from the crossing to land it is
p_cx to 1 PAL; with land, below. Plan 14 owns the regulated steady state, a fitted table from a
published box model, callable at any age under any forcing for the stage gates (request 7), which
T53.d's own test already constrains: Earth's fluxes give 0.21 bar within a factor of two. Plan 14's
own oxygen, from a temperate world whose cold trap fails for want of nitrogen (P14.T53.a, drafted;
Wordsworth and Pierrehumbert 2014), is added to the regulated level, not drawn into it: the bands
bound the biological source, a world whose abiotic O₂ already exceeds its band keeps it, and the
stage stands, a false positive the same physics makes.

**Ozone and methane.** **Lean:** T24.c's ozone factor F is each of the three-dimensional model's
global mean columns over its own pre-industrial 279 DU: 169 DU at 0.1 PAL, 136 at 0.05, 66 at 0.01,
45 at 0.005 and 18 at 0.001 (Cooke et al. 2022). They are taken over the one-dimensional ones, such
as Segura et al.'s (2003) 266 DU at 0.1 PAL and about 124 at 0.01, since Cooke's lower columns come
from transport the one-dimensional models lack; the stellar factor stays Segura's. Thinner ozone
means more OH, so oxygen shortens methane's life: its surface lifetime is 1.061 of pre-industrial at
1.5 PAL, 0.903 at 0.5, 0.70 at 0.1, 0.52 at 0.05, 0.067 at 0.01, 0.033 at 0.005 and 0.010 at 0.001
(Cooke et al. 2022, Table 2), ratios taken at a fixed 0.8 ppmv of surface CH₄ and so valid only near
it, never at Archean abundances, where the lifetime grows with the gas; and anaerobic oxidation with
sulphate may hold CH₄ under 10 ppmv for the second half of Earth's history "regardless of
atmospheric pO₂" (Olson, Reinhard and Lyons 2016). Oxygenation therefore collapses the methane
greenhouse, which Pavlov et al. (2000) proposed triggered the first widespread glaciation. Here that
cooling is the second record's: it shows in its surface temperature and on consoles and sets no
stage back ([Where life sits in the pipeline](#where-life-sits-in-the-pipeline); [open question
56](#open-questions)), while a snowball that ends a biosphere is read on the abiotic twin ([When
life ends, and when it is set back](#when-life-ends-and-when-it-is-set-back)). N₂O arrives with
nitrate: of fluxes of 0.01–100 Tmol yr⁻¹, compatible with Earth's history, 10 Tmol yr⁻¹ gives at
most about 5 ppm about the Sun but 30 ppm on an Earth-like TRAPPIST-1e (Schwieterman et al. 2022),
so its lifetime too is a table by host.

### Land

**Weathering.** Vegetation enhances chemical weathering about four to seven times, and small
watersheds measure two to sixteen (both as Porada et al. 2016 cite them); Ordovician mosses and
lichens alone may have weathered three times today's global flux (Porada et al. 2016). Continental
weathering goes as F = f_bio f_land F_mod (pCO₂ ÷ pCO₂,mod)^α exp(ΔT ÷ T_e) (Krissansen-Totton,
Arney and Catling 2018), with α 0.24–0.44 (median 0.33) and an e-folding T_e of 22–45 K (median 34),
both 1σ (Krissansen-Totton and Catling 2017), against the 13.7 K of Walker, Hays and Kasting's
(1981) model as Krissansen-Totton and Catling give it. Their f_bio is 1 for modern Earth;
renormalised here so that a lifeless world has 1 and a vegetated one f_bio, a climate warming S per
e-fold of CO₂ against fixed outgassing gives the lifeless twin
ln(pCO₂ ÷ pCO₂,bio) = ln f_bio ÷ (α + S ÷ T_e). At α = 0.33 and f_bio = 4, for T_e from 13.7 to
34 K, the twin holds 8.6–20.7 times the CO₂ and is 9.3–13.1 K warmer at 3 K per doubling, and holds
4.5–11.5 times and is 12.2–19.7 K warmer at Krissansen-Totton and Catling's own 5.6 K (computed
here, before seafloor weathering shares the load). The generator's own climate sets the real figure:
plan 14's grey τ as built warms Earth about 9 K for a doubling of its 42 Pa of CO₂, and on it the
same balance gives the twin some 2.8–5.9 times the CO₂ and 14–28 K (computed here, approximately, on
a Bolton saturation curve rather than the registry's), until P14.T53.a's new CO₂ form changes it.
Its size is the point: `TEMPERATE_AIRBORNE_CARBON` is a vegetated Earth's number given to every
temperate world. The balance also needs a greenhouse that saturates at the maximum-greenhouse limit
(Kopparapu et al. 2013), which plan 14's √p term lacks (P14.T13.c's findings of 2026-10-10): as
built it warms a world at that limit's flux to 288 K with about 0.06 bar of CO₂ (computed here), so
the steady state follows P14.T53.a's new CO₂ form and T51.a's cold trap.

**Oxygen.** Lenton et al. (2016) model the earliest land plants raising organic burial enough to
bring O₂ to present levels by 420–400 Ma, where "fossil charcoal indicates O₂ >15–17%", and
"fire-mediated negative feedbacks stabilize high O₂ levels" after. **Lean:** the land regime's band
is 0.7–1.5 PAL at Earth's pressure, the upper end Cooke et al.'s (2022) bound since the Great
Oxidation and the lower end that charcoal floor of 15% O₂. That floor is a mole fraction, so on a
world of surface pressure p_s it is 0.15 p_s, or about 0.72 (p_s ÷ 101 kPa) PAL; how fire's limits
move with total pressure was not researched here, and wants research before the band is built. A
setback, deoxygenation among them ([When life ends](#when-life-ends-and-when-it-is-set-back)),
returns a world to an earlier regime, and its gases relax as above.

**Albedo and cloud.** In T51.b's area mix the cover term is (1 − cloud) × f_land × c × Δa, with c
the covered share of land and Δa the cover's albedo less bare ground's. Half an Earth's land under
forest at about 0.15 instead of bare ground at about 0.30 darkens the Bond albedo by about 0.007 and
warms T_eq by about 0.65 K (computed here, the albedos [Biomes as places](#biomes-as-places)' to
set). It is chiefly a colour, for [Biosignatures](#biosignatures). No biological cloud effect is
modelled, since the evidence says "it is time to retire the CLAW hypothesis" (Quinn and Bates 2011);
moisture recycling by transpiration is absent from R09's precipitation heuristic, and labelled so.

### Soils and sediments

A biosphere leaves rock behind, which [Resources and deposits](#resources-and-deposits) places.
Organic carbon burial is the net source of O₂ and makes kerogen, coal and oil. Banded iron
formations precipitate where an anoxic ocean's Fe²⁺ meets O₂ or anoxygenic phototrophs that oxidise
it (Kappler et al. 2005; Konhauser et al. 2017), and end when the deep ocean oxygenates. Carbonates
precipitate with or without life, which changes where and how. Biologically active soils make the
clay minerals whose surfaces preserve buried organic matter, a feedback Kennedy et al. (2006) tie to
the late Precambrian rise of oxygen.

### Keeping one climate

The second pass changes O₂, CH₄, N₂O and CO₂, and with them the surface pressure, the mixture's heat
capacity, which T24.e's lapse rate reads, the molar mass, which the scale height reads, the grey τ
and the area-mix albedo, and re-runs the albedo–temperature iteration; it changes gases, temperature
and albedo only, never `SurfaceState`, which the host's flux sets. Ozone and haze optics stay on the
Surface path, the haze's cooling excepted. Plan 14's temperate constants were fitted on living Earth
(its water term on 288 K, its albedo of 0.294, its cloud of 0.67, its airborne carbon), while its
Earth holds 0.78 bar of N₂ and no O₂, and P14.T57.a refits the water term on Earth's measured state
in the 22 → 23 batch, still without O₂ or biotic methane. Refitted on Earth with Earth's forcing, in
P14.T53.a's refit of every constant, they make a lifeless Earth twin without 0.21 bar of O₂ or its
methane, with several times the CO₂ and brighter land, which leaves it roughly 10–30 K warmer, by
climate sensitivity.

False positives come from the same code, and a console reads plan 14's gases whatever made them, so
detection is ambiguous by construction; what plan 14 makes and still lacks is listed in [False
positives, made by the same physics](#false-positives-made-by-the-same-physics).

## Biomes as places

A biome is a class of place on the map that life has made distinctive. On a world with a surface
biosphere it is a closed-form function of R09's [coarse
field](rendering-and-planets.md#the-coarse-global-pass-once-per-planet), evaluated per cell from
what the pass has settled and the biosphere's table of growth strategies and covers: what grows
there, how densely, how tall, how productively, in what colour and on what seasonal cycle. Under the
owner's prior the common case is crusted land, mats on wet ground, reefs in shallow seas, coloured
brines and snow, and plankton-coloured seas; vegetated land is rare, but the renderer most needs it
argued. Both are one problem: structure and pigment from the world's physics, with no Earth plant
assumed.

### What a cell's biome reads

The function reads only the quantised field and a table the server resolves for each biosphere:

- **Climate**, from the `ClimateCell` at level L − 1: each month's temperature, the lapse term
  re-applied from the cell's elevation, its precipitation, R09's labelled heuristic, whose label
  every product inherits, and its 10 m wind, on R09's months of equal eccentric anomaly.
- **Terrain and liquid**, from the `SynthesisCell`: elevation (and slope from its neighbours, inside
  the margin), flow, drainage area, `runoff` and `catchment_runoff`, the liquid's surface and depth,
  ice, the substance byte and `SurfaceClass`. Water, meaning the solvent where the palette's liquid
  is another, is supplied by precipitation concentrated by run-on through the wetness index
  ln(a ÷ tan β) (Beven and Kirkby 1979).
- **Substrate**, R10's form and palette entry before life: soil, sediment and sand of a crust entry
  carry plants, bedrock crusts, a `Deposit` evaporite brine communities, an ice entry's frost and
  glacier their algae, and melt, an airless regolith and a volatile ice nothing.
- **The biosphere**, through a `LifeTable` the server resolves once per living world: the growth
  strategies' traits, light use by month and latitude from the photon spectrum at the ground of the
  first, abiotic record ([Where life sits in the pipeline](#where-life-sits-in-the-pipeline)), as
  the pigments read it ([Pigment, colour and the seasons](#pigment-colour-and-the-seasons)), and
  phenology thresholds, the biological covers, and two constants that make the sweep realise the
  body's snapshot. The record behind it, its stage, `phototrophy` date, solvent and productivity
  form ([What life is made of, and what it lives on](#what-life-is-made-of-and-what-it-lives-on)),
  stays on the server; only the seasons are evaluated in time.

### Choosing a classification

| Scheme                                                                                                     | Basis                                                                                                                                           | Verdict                                                                                                                                                                                                        |
| ---------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Whittaker's biome diagram (Whittaker 1975)                                                                 | Mean annual temperature and precipitation; nine Earth formations                                                                                | A display and a test target. No seasonality or structure, and its boundaries are Earth plants' tolerances.                                                                                                     |
| Holdridge life zones (Holdridge 1947)                                                                      | Biotemperature, annual precipitation, the ratio of potential evapotranspiration to it                                                           | Physical axes, the last carrying the water balance, but Earth formations and no seasonality.                                                                                                                   |
| Köppen–Geiger read as biomes                                                                               | R09's class, mapped to formations                                                                                                               | Cheapest, since Köppen drew his boundaries along vegetation, "crystallized, visible climate" (Beck et al. 2018), but it makes climate and biome one byte and fixes Earth's thresholds on alien life. Rejected. |
| Plant functional types: BIOME (Prentice et al. 1992), BIOME4 (Kaplan et al. 2003), LPJ (Sitch et al. 2003) | A few Earth types with bioclimatic limits, ten in LPJ; dominance among them, by productivity in BIOME4 and LPJ                                  | The right mechanism, climate filtering types and productivity deciding dominance, with Earth's types hard-coded.                                                                                               |
| Trait sampling: Kleidon and Mooney 2000; JeDi-DGVM (Pavlick et al. 2013)                                   | Many random growth strategies, each a vector of traits in trade-offs; the climate decides which survive                                         | The mechanism with no Earth types in it: biomes emerge. Too slow on arrival, JeDi spinning up some 3,500 years; a closed-form equilibrium of its logic is this section's approximation, not JeDi's.            |
| IGBP land cover (Sulla-Menashe and Friedl 2018)                                                            | Structure: forest a canopy above 2 m over 60% of the ground, savanna 10–30%, shrubs of 1–2 m, grassland herbs under 2 m, barren under 10% cover | The label, defined by what a sensor sees, not by taxon. It stops at "barren", with no microbial classes.                                                                                                       |
| Longhurst's ocean biomes (Longhurst 1998)                                                                  | Mixing and light: Polar, Westerlies, Trade-wind, Coastal                                                                                        | The ocean's equivalent, from physics R09's field carries.                                                                                                                                                      |

The physics wants a mechanism, the climate filtering what can grow and productivity deciding what
dominates; the map wants labels that mean the same on any world. Trait sampling gives the first
without Earth's types: Kleidon and Mooney's (2000) random parameter sets of a generic plant model,
tested for survival on a global grid, grouped into grass-like to tree-like prototypes and into
biomes like the observed, and JeDi-DGVM's strategies of 15 traits in trade-offs compare favourably
with fixed-type models. The count matters: with 10 random strategies JeDi's global NPP ran from 29
to 57 Pg C a year across seeds, against a standard 2,000 (Pavlick et al. 2013). IGBP gives the
second, a label a sensor can check.

**Lean:** a mechanism and a label, kept apart. Each biosphere draws at most sixteen growth
strategies on `body.life.strategies` by a stratified design spanning the trade-offs, since sixteen
random draws would not calibrate; the reference Earth runs on a fixed Earth-like set. A strategy's
traits are its thermal window and freezing tolerance, its water use (evergreen, drought-dormant,
cold-dormant or ephemeral), its carbon pathway, with or without a carbon-concentrating mechanism,
which picks f_C's C4-like or C3-like ramp ([The energy budget, in closed
form](#the-energy-budget-in-closed-form)), whether its tissue is rigid, with density and stiffness,
and its allocation to rigid tissue and that tissue's residence time, the stage bounding the ranges;
[Organisms](#organisms) maps plant-like clades onto them, and pigments are computed, never drawn.
Each cell scores every strategy in closed form against its monthly climate. Survival is a carbon
balance net of costs: a strategy survives where its annual P, less the share it builds into rigid
tissue and a maintenance cost on its standing rigid biomass, stays positive. That biomass is what
the strategy's stature needs, its stems sized by elastic similarity under the world's gravity, so
that the cost does not shrink with the cell's P, and the maintenance rate is fitted from the start
so that the reference Earth's treeline and dry woodland edge fall at a seasonal mean root-zone
temperature near 6.7 °C (Körner and Paulsen 2004) and near 100 mm of rain a year (Sankaran et al.
2005). The survivors share the cell's light by height, tallest first ([Vegetation as
structure](#vegetation-as-structure)), so a rigid strategy pays carbon for height and height earns
light; this replaces JeDi's weighting by living biomass. The result — cover, its rigid share,
height, productivity, habit and cover entry — is the authoritative state, and the biome a structural
label read off it by IGBP's thresholds, extended below "barren" into crust, mat, reef and algal
forms and offshore into Longhurst's biomes. Earth names are display labels at most ([Fit with the
consoles](#fit-with-the-consoles)), and Köppen–Geiger stays a climate class that a lifeless twin
keeps.

### Microbial biomes, the common case

Microbial life reaches land long before anything macroscopic. Fossil mats drape river gravels of the
Moodies Group 3,220 Myr old, isotopically distinct from marine mats nearby (Homann et al. 2018), and
biocrusts still cover about 12% of Earth's land (Rodriguez-Caballero et al. 2018), in cryptogamic
covers taking up about 3.9 Pg C a year, some 7% of terrestrial net primary production (Elbert et al.
2012). So visible land cover waits not for the `Land` stage but for phototrophy and a surface
liquid: a chemotrophic biosphere shows nothing beyond its hydrothermal fields, while a phototrophic
one mats its wet and periodically exposed ground early, as at Moodies, and by inference crusts dry
ground from the same date. **Lean:** visible land cover from the epoch's `phototrophy` date ([The
output](#the-output)), which is not a stage.

| Class              | Where on the coarse field                                                                                                               | Structure                                                                                             | What sets its colour                                                                                                                                                                                                                                                       |
| ------------------ | --------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Crust              | Dry land with occasional wetting (R09's B classes, cold deserts); its cover from P, rising with W, gentle slopes favoured (a heuristic) | Millimetres, flat, coherent                                                                           | A UV-screening pigment over the phototroph, scytonemin on Earth (Soule et al. 2009); darker where no ozone shields the ground                                                                                                                                              |
| Mat                | Wet ground: a high wetness index, shores, deltas, wetlands, lake margins, hydrothermal fields                                           | Millimetres to centimetres, laminated, a complete ecosystem at the millimetre scale (Des Marais 2003) | Layers set by light and chemistry, on Earth a green cyanobacterial layer over a pink purple-sulfur one (Rothermich et al. 2000), under a screening skin in strong light                                                                                                    |
| Brine bloom        | Evaporite basins and terminal lakes                                                                                                     | Coloured water and salt crust                                                                         | Red-orange carotenoids and purple retinal proteins, as in haloarchaeal blooms visible from space (DasSarma et al. 2020)                                                                                                                                                    |
| Microbial reef     | Lit, shallow floor of carbonate-saturated shelves                                                                                       | Lithified domes and columns, decimetres to metres (Dill et al. 1986; Suosaari et al. 2016)            | The mineral, under a living mat skin                                                                                                                                                                                                                                       |
| Snow and ice algae | Frost and glacier of an ice entry that pass its `transition` in some month                                                              | The top centimetres                                                                                   | Screening pigment: red snow can lower snow albedo by 13% over a melt season (Lutz et al. 2016); glacier algae carry 11 times their chlorophyll in phenolic pigment and explain up to 75% of the albedo variability across south-western Greenland (Williamson et al. 2020) |
| Plankton province  | Open water, by Longhurst biome                                                                                                          | Suspended                                                                                             | Pigment concentration, which sets the ocean's colour                                                                                                                                                                                                                       |

Crusts are not cosmetic: biocrusts make up as much as 70% of dryland cover (Rutherford et al. 2017),
and their albedo is a planetary term ([Before oxygen](#before-oxygen-the-commonest-living-world)).
Reefs are the shallow-sea biome of a world without grazers: a stromatolite reef of seven morphotypes
spans a 3,430 Myr old peritidal carbonate platform (Allwood et al. 2006), and columnar stromatolites
lost diversity 600–700 Myr ago as metazoans appeared (Awramik 1971). **Lean:** microbial reefs on
every lit shelf of a biosphere without macroscopic grazers whose ocean is carbonate-saturated by
request 8's ocean chemistry ([Land](#land)), every temperate water ocean counted saturated until
that lands, warm or not, since microbial cones grow under perennial lake ice (Greco et al. 2020);
their lithified heads 0.2 m tall or more are rigid, so authoritative instances placed as rocks are.
Hydrothermal fields, mat sites zoned in colour by temperature, at one Oregon spring a green film
from 73–74 °C giving way near 55 °C to a brown cover over a salmon undermat (Castenholz 2015), are
places for [Geology as places](#geology-as-places).

### Vegetation as structure

The step evaluates the productivity of [The energy budget, in closed
form](#the-energy-budget-in-closed-form) exactly, P = min(f_T × P_L, P_W, P_N) × f_C, per strategy
and month. A strategy sets only its envelope, outside which P is zero, its dormant months, the share
a of light its cover absorbs, its pathway, which picks f_C's ramp, and its stature and rigid
allocation, which set its place in the light and its carbon balance; the cell supplies W, its run-on
precipitation, and water enters there alone, through P_W. P_N reads, on land, the cell's steepness,
drainage and runoff in closed form, or a factor the server puts in the `LifeTable`, since no erosion
rate is a field of the cell, and at sea the supply of [Oceans, ice and shallow
water](#oceans-ice-and-shallow-water). On Earth it must pass the reference Earth's acceptance test
that [The energy budget, in closed form](#the-energy-budget-in-closed-form) sets, land within Zaks
et al.'s 36–74 Pg C a year and the ocean within a factor of two of 48.5, against Field et al.'s
(1998) 104.9 in all and 56.4 on land. P_W and f_T being Lieth's own terms, the Miami model checks
nothing, so a further check of the land, outside that test, is Field et al.'s biome totals as shares
of their natural vegetation, their cultivated row reassigned, tropical rainforests 17.8 and savannas
16.8 Pg C a year among them. One unconstrained sweep is then scaled to the snapshot's cover and
productivity ([Where life sits in the pipeline](#where-life-sits-in-the-pipeline)) by a cover
threshold, the point at which the area-weighted cumulative cover of the cells, sorted by their
unconstrained cover and then by cell index, meets the snapshot's, and then a productivity factor,
the snapshot's total over the surviving cells' sum, taken in cell-index order, both computed once
and carried in the `LifeTable`, as R09 places ice to plan 14's fraction. Where the snapshot's cover
exceeds the area with any potential, the threshold stops at zero and the shortfall is recorded.

Light is shared by height, and cover follows from P without applying water again. In a cell the
surviving strategies are taken tallest first, by the height each reaches there. Each sees the light
the taller ones pass, and its cover is the limited rate over the light-limited rate at full cover,
min(f_T × P_L, P_W, P_N) ÷ (f_T × P_L), P_L at its own share a of that light and P_W and P_N less
what the taller ones used; its covered ground absorbs the share a and passes the rest down. So a
canopy closes only where neither water nor nutrients bind, a strategy's productivity, cover × f_T ×
P_L × f_C, is its P under the light it gets, the cell's is their sum, and the whole is one sort by
stature and one pass a month, with no iteration. Sankaran et al. 2005 is its test: over 854 African
savanna sites the 99th-quantile bound on woody cover rises at 0.14% per millimetre of mean annual
precipitation, from zero near 100 mm to about 77% at a break of 650 ± 134 mm, above which only fire
and herbivory hold a canopy open; without them the bound reaches IGBP's 60% forest cover at about
530 mm (computed here; [open question 42](#open-questions)). **Lean:** the reference Earth's woody
cover stays under that bound, and below it where disturbance acts. Combustion needs about 15% O₂ at
Earth's pressure (Belcher and McElwain 2008), and landscape fire is off below about 16% and greatly
suppressed below 18.5% (Belcher et al. 2010), so a world below about 16% O₂ at Earth's pressure has
no fire-held savanna; herbivory is for [Organisms](#organisms). Biomass is productivity times rigid
allocation times residence time: Earth's plants hold about 450 Gt C (Bar-On, Phillips and Milo 2018)
against 56.4 Pg C a year, a residence near eight years, and whole ecosystems about 23 years against
gross, not net, production (Carvalhais et al. 2014).

Canopy height is the least of three limits: productivity, weakly, since the best single predictor of
plant height is wettest-month precipitation (R² 0.256), the mean maximum height of species within
15° of the equator being 7.8 m against 27 cm at 60–75° N, though cold, dry, unproductive systems
hold a wide range (Moles et al. 2009); the strategy's stature, low for anything not rigid; and
hydraulics. The redwoods' leaf traits extrapolate to 122–130 m, set by leaf water stress from
gravity and path-length resistance, their midday water potential falling at 0.00973 MPa m⁻¹, within
1% of the hydrostatic ρ_w g, from −0.71 MPa at the ground to a limit of −1.9 MPa at 122 m (Koch et
al. 2004, Table 1). Mechanics sets girth, not height: trees follow elastic similarity, length as the
⅔ power of diameter (McMahon 1973), the same scaling as Greenhill's buckling height, 0.79 (E ÷ ρg)^⅓
D^⅔ (Greenhill 1881, the coefficient computed here), so the safety factor holds at every size and
buckling alone caps nothing (Niklas 2007).

So the hydraulic limit scales with gravity to first order: H_hyd = (Ψ_0 − Ψ_lim) ÷ (ρ_w g), Ψ_0 the
midday offset, set by soil water and transpiration and not by gravity, and Ψ_lim a property of cells
and xylem, a strategy trait. Koch's values give 122 m at 1 g, about 320 m at Mars's 0.38 g and 61 m
at 2 g, a frictional gradient too small to see in their data making it an upper bound at low
gravity. Pressure acts on gas exchange: diffusive fluxes go as partial-pressure differences over
total pressure, so CO₂ uptake at a fixed mixing ratio holds while thin air loses more water per
carbon (Gale 1972a; 1972b, the second not re-checked), and dense air loads crowns with wind as ρ_a
U². **Lean:** H_max = min(H_productivity, H_strategy, H_hyd), the hydraulic term argued to first
order and untested beyond 1 g, girth from elastic similarity and gravity, and wind loading left out
until R09's winds are trusted at canopy height; at low gravity a 300 m stem's carbon and time, and
the wind, should bind first.

Without vascular tissue, stature stays low, since water moves only by diffusion and capillarity:
non-vascular ground cover rarely exceeds a few tens of centimetres, the tallest mosses' stems
reaching about 80 cm (Hyvönen et al. 1998), the fungus-like Devonian Prototaxites of several metres
a rare exception (Boyce et al. 2007, height not re-checked). Earth is one sample: red-edged
vegetation has been widespread for only about 460 Myr (Hegde et al. 2015), and forests stood by the
Mid Devonian, 393–383 Ma (Stein et al. 2020). **Lean:** within the `Land` stage the strategies'
stature bound grows with the time since that transition, over some 82–92 Myr, Earth's from the stage
table's date for `Land`, land-plant spores by 475 Ma, to forests at 393–383 Ma (67–77 Myr if counted
from the red edge's spread instead), and kept a parameter, an ask of [Whether life arises, and how
far it gets](#whether-life-arises-and-how-far-it-gets).

### Pigment, colour and the seasons

Kiang et al. 2007a propose that pigments peak in absorbance at the wavelength of peak incident
photon flux at the surface, at the longest available wavelength for core antenna or reaction-centre
pigments, and at the shortest wavelengths within an atmospheric window for accessory pigments;
Earth's P680 sits at the surface photon-flux peak near 685 nm, the red edge's near-infrared end at
water bands and the oxygen A band at 761 nm. Around other stars the first rule gives about 451 nm
for an F2V host, 667 nm for K2V, 754 nm for M1V and 1,045 nm for M5V, with M stars' bands at
0.93–1.1, 1.1–1.4, 1.5–1.8 and 1.8–2.5 µm and underwater life below 1.4 µm, likely 1.1 µm (Kiang et
al. 2007b). An antenna tuned to cancel excitation noise (Arp et al. 2020) disagrees: applied
systematically from 2,300 to 5,800 K, its optimum under the Sun is the blue Soret pair, 385 and 415
nm above the air and about 443 nm through it, chlorophyll's red bands only weak local maxima, and
red chlorophyll-like bands appear near 3,300 K, bacteriochlorophyll a's 800–850 nm at 2,800 K and
about 1,000 nm below 2,600 K (Duffy et al. 2023). The rules part by some 200 nm for K and early-M
hosts and meet near 1 µm for late M dwarfs. Around the coolest M dwarfs the 400–750 nm photon flux
may be a few per cent of Earth's (Lehmer et al. 2018); cyanobacteria under a simulated M-dwarf
spectrum, its shape rather than that flux, made oxygen much as under solar light at the intensities
tested (Claudi et al. 2021; Battistuzzi et al. 2023).

**Lean:** the pigment is computed, never chosen, for each stage a biosphere reaches, from the photon
spectrum at the ground of the first, abiotic record, as the body-level productivity is, with the
stage's ozone column from the oxic regimes' table ([The rise of oxygen](#the-rise-of-oxygen); Cooke
et al. 2022) applied to screening pigments' ultraviolet: a function of the stage, never of the
second pass or the Surface path, so that the cover albedo plan 14's second pass reads depends on
nothing that pass makes. The biosphere's own ozone and haze change the light the covers are drawn
under, not where their absorbers lie. The primary, reaction-centre and core-antenna absorber goes
where Kiang's first two rules put it, at the peak photon flux and the longest wavelength in its
window; accessories go at the window's short edge by the third, where the noise-cancelling optimum
also falls; above the longest absorber the tissue's scattering raises an edge, with the solvent's
bands beyond. The noise-cancelling model is the cross-check, its red solutions local maxima rather
than its optimum. Sunlight alone is in-sample for Kiang's rules, so the test is a battery of Earth's
light niches, with the absorber width, smoothing and choice of maximum fixed first: full sun
(chlorophylls a and b, and an edge at 700–750 nm, where leaves brighten tenfold; Seager et al.
2005), canopy shade (purple bacteria's 801 and 857 nm) and 2 m of seawater (bacteriochlorophylls c
and e; both Arp et al. 2020), and far-red shade (chlorophylls d and f, to 750 nm; Claudi et al.
2021). For K and early-M hosts each cover carries the rules' 200 nm spread as its stated
uncertainty. The results are the `LifeTable`'s covers, each with B, V and R normal albedos, a phase
row and dormant albedos, the sensor bands for [Biosignatures](#biosignatures) staying on the server;
the renderer shows them under the host's light through R07's exposure, which scales luminance alone
and applies no white balance, so that an M-dwarf world is drawn red-lit; labelled derived.

Microbial worlds widen the palette: retinal phototrophy, as in bacteriorhodopsin, absorbs at 568 nm,
in the gap chlorophyll leaves, so its holders look purple (DasSarma and Schwieterman 2021, whose
"Purple Earth" is speculative); purple bacteria colour anoxic and oxic habitats alike (Coelho et al.
2024); and 137 microorganisms' spectra (Hegde et al. 2015) and non-photosynthetic pigments
(Schwieterman, Cockell and Meadows 2015) show how often a surface's colour is not its light
harvester's. **Lean:** where a surface faces strong light or ultraviolet — crusts, snow and ice
algae, brines, a mat's top — its colour is a screening pigment's, absorbing at the spectrum's short
end and denser where no ozone shields the ground; elsewhere the phototroph's. Both are covers in the
`LifeTable`'s own list, never the field palette's, a handful a biosphere.

Phenology is computed, not stored. Jolly, Nemani and Running 2005's growing-season index is the
product of indices of minimum temperature, vapour-pressure deficit and day length, each ramping
between thresholds; it follows satellite greenness at r > 0.8 across nine ecosystems and dated leaf
flush to about 3 days at one temperate deciduous forest. **Lean:** the same product, its thresholds
a strategy's traits and its day length the cell's daily insolation, which a locked or slow world
leaves to [open question 63](#open-questions), evaluated on both sides from the `ClimateCell`'s
months blended at the scene's time by R09's `month_blend`, at no cost in bytes. Colour cycles
between dormant and active reflectances through green-up and senescence, when the primary pigment is
withdrawn and the others show, as carotenoids do in autumn (Lee et al. 2003). Algae bloom with the
melt, plankton with the season's light and mixing. A one-month world, which R09 gives only an orbit
of e = 0 with no obliquity, has no cycle; a world locked 1:1 to its star keeps twelve months
wherever its orbit is eccentric or it has obliquity (R09 Design note 8), and its phenology follows
them.

### Oceans, ice and shallow water

Ocean productivity is limited by light, nutrients and temperature (Field et al. 1998), and
Longhurst's four principal biomes, recognised in every major basin (Longhurst 1998, as the Flanders
Marine Institute's province dataset summarises), follow physics R09's field carries. **Lean:** each
open-water cell takes its biome from the field — Polar where seasonal ice or dark months limit
light, Westerlies in the storm-track band, Trade-wind in the Hadley band's stratified gyres, Coastal
within a shelf band — and a nutrient supply from an upwelling index (offshore Ekman transport under
the month's wind on eastern boundaries, and the equatorial divergence), river outlets' drainage and
runoff, and P_N's hydrothermal floor ([The energy budget, in closed
form](#the-energy-budget-in-closed-form)); a world locked to its star or turning slowly lacks those
bands ([open question 63](#open-questions)). The reference Earth's check is Field et al. 1998:
48.5 Pg C a year from the ocean, a mean of 140 g C m⁻² yr⁻¹, eutrophic waters (chlorophyll above 1
mg m⁻³) giving under 18%; Ryther 1969's pre-satellite ordering, open ocean below coast below
upwelling, is a test of order alone.

Ocean biomass is small and fast, about 6 Gt C of marine biota against some 450 Gt C of plants
(Bar-On, Phillips and Milo 2018), so its colour follows the season. R11 draws it from the liquid
entry's absorption, pure water's on an Earth (Pope and Fry 1997). **Lean:** it adds the pigment's
absorption and the particles' backscatter by Morel and Maritorena 2001's case 1 relations of
reflectance to chlorophyll, the concentration from productivity by a closed form fitted to Earth;
their coefficients, fitted to Earth's phytoplankton and its dissolved matter, are rebuilt from the
pigment's own specific absorption, and the colour labelled derived. A floor within the photic depth
carries mats or microbial reefs, or the beds [Organisms](#organisms) places; ice carries algae as
the microbial table has it.

### The per-cell state, the material and the instances

**Lean:** the life state is computed, not sent. R09's field is already about 14.7 MB for an Earth at
level 8, within 1 MB of R03's 15 MiB check, so a 6-byte layer at level L, 2.4 MB, does not fit. Its
inputs are all in the quantised field or the `LifeTable`, so `hyperion_surface::life` runs it where
R10's `material_at` runs, on both sides, under the same discipline and with goldens on all three
targets: a `LifeState` per cell, scored at level L − 1 and corrected for each child's lapse and
wetness, its reads inside R09's five-cell margin. It draws nothing per cell (`surface.life`, in
`SurfaceDetail`, is reserved should it need to) and stays out of `SurfaceClass`. The sim keeps the
body-level form, the chemotrophs and the photon yields ([The energy budget, in closed
form](#the-energy-budget-in-closed-form)).

| Field                  | On land                                 | Under water                                        | Fallback coding                                                           |
| ---------------------- | --------------------------------------- | -------------------------------------------------- | ------------------------------------------------------------------------- |
| `biome`                | Structural class                        | Longhurst biome, or a benthic class on a lit floor | `u8`, a `BiomeClass`                                                      |
| `cover`                | Ground under living cover               | Lit floor under mats or reef                       | `u8`, a fraction in 255ths                                                |
| `rigid`                | Share of the cover that is rigid canopy | Share that is lithified reef                       | `u8`, a fraction in 255ths                                                |
| `height`               | Canopy height, or a mat's thickness     | Reef height                                        | `u8`, 1 mm × 2^(c ÷ 13), 5.5% steps to about 800 m                        |
| `productivity`         | Net primary productivity                | The same, from which pigment concentration follows | `u8`, 0.01 × 10^(c ÷ 45) g C m⁻² yr⁻¹, 5% steps to about 4,600; 0 is none |
| `cover_entry`, `habit` | The `LifeTable`'s cover, and leaf habit | The plankton's cover, and bloom habit              | `u8`: the cover index in 4 bits (`0xF` none), the habit in 2              |

The `LifeTable`, about 1–2 kB, travels with the first block holding a surveyed living cell under
imaging coverage, and life is drawn only where imaging coverage exists; it carries no stage,
`phototrophy` date or phylogeny, so a barren or radar-only survey shows nothing, and block 0's
albedo scale, solved against the abiotic twin's geometric albedo (below), carries nothing of life. A
client holding every cell's class could still solve the scale against the living disc's geometric
albedo, which R07's uniform disc reads, and see the two differ: a leak of the kind exact gases are,
closed only for an honest client. What a readout may quote, and what a console calls a cell before
analysis supports a biological name, are for [Knowledge, surveys and the landing
party](#knowledge-surveys-and-the-landing-party). Should the function prove too slow on the client,
or need reads beyond the margin, the server sends the state at level L − 1 instead, 6 bytes per four
cells: about 0.59 MB on an Earth, 15.3 MB in all, inside 15 MiB and R09's 0.98 MB margin but over
the brainstorm's roughly 15 MB, and R09 asks that any further per-cell byte wait for R03's check to
be re-run.

**Lean:** the material learns life through R10's cover term. R10's re-validation makes `Material`
`{ class, entry }`, a form and a palette entry (decision-composition), and adds a T1.c that
modulates regolith's albedo by T51.d's tholin cover, zero cover giving the substance's own. Life
takes that path: form and entry stay the substrate's, and the cell's cover and fraction modulate the
albedo in each band and the phase row, through a cover fraction per texel in the bake and in R07's
disc map rather than new (form, entry) pairs. The covers are the `LifeTable`'s, not the palette's
`Cover` role, which the tholin keeps; P14.T53.d's `biological` term stays on the server, and a
console names either cover by what was observed ([Detail levels for
life](#detail-levels-for-life-and-how-they-degrade)). Bearing capacity stays the substrate's; a
canopy's backscatter hotspot, mostly 1–2° in half-width (Bréon et al. 2002), is one more row of
R10's phase table. R10.T10.a's albedo scale is solved on the substrate against the abiotic twin's
geometric albedo ([Where life sits in the pipeline](#where-life-sits-in-the-pipeline)), and the
`LifeTable`'s biological covers are laid over it unscaled, so that block 0 carries nothing of life.

**Lean:** R11's instancing takes flora as it takes rocks: `ExtraLayer` gains `Flora(Vec<Plant>)`,
through the same `BakeHook` and "could touch" rule. Plants come in height octaves from 0.2–0.4 m to
the body's H_max, each on a quadtree level whose cells are about the octave's spacing; each cell
holds one jittered candidate, so crowns are spaced as competing trees are rather than
Poisson-clumped. The candidate on the octave holding the cell's canopy height is kept with
probability min(1, cover × rigid × cell area ÷ crown area), so that its crowns fill the rigid cover,
and octaves above it hold none; each octave below holds 2.8 times the stems of the one above it,
stems falling as r⁻² in linear bins of radius (West, Enquist and Brown 2009) under elastic
similarity, its candidate kept with that density times its cell's area, at most 1. The draws are on
`surface.flora` and `surface.flora.shape` (`SurfaceDetail`, keyed by `ObjectKey::surface_cell`);
density rises by the synthesis's channels and falls to zero above R10's cliff slope. A plant's
contact shape is a rigid tapered trunk, its girth from elastic similarity and gravity, and a
compliant crown envelope widened by its sway; its taxon is from [Organisms](#organisms). An Earth's
17.7 m finest patch under 10 m crowns holds a few canopy stems and tens of smaller plants.

| What                                                                                     | Status                                                                                                      |
| ---------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| A cell's `LifeState`                                                                     | Authoritative, a function of the quantised field on both sides                                              |
| Cover entry and fraction at a point                                                      | Authoritative, baked with the material                                                                      |
| Phenology phase                                                                          | Authoritative, a function of the cell and the time on both sides                                            |
| Rigid plants and reef heads 0.2 m tall or more, by height rather than by rocks' diameter | Authoritative instances with contact shapes                                                                 |
| Herbaceous cover of any height, moss, litter, seedlings, the texture of mats and crusts  | Decoration, culled where it meets a body: not rigid, it holds up no hull, though the state gives its height |
| Sway, colour variation within a crown, plankton patchiness below a cell                  | Decoration                                                                                                  |

### The cost

The state costs no bytes a cell and 1–2 kB of table a living world; the bake's cover channel is a
byte a texel, about 4.2 kB a 65 × 65 patch, for R10's cache budget. Scored at level L − 1, a whole
Earth is about 98,000 cells, times 16 strategies and 12 months, or 1.9 × 10⁷ closed-form
evaluations, some 0.2–1 s, which the server spends once for R07's disc map and the organisms'
pools; a client spends microseconds on each cell its patches read. All of it is estimated and
unmeasured, like every timing R09 quotes.

## Organisms

Under the owner's prior most biospheres a crew meets are microbial, so this section serves that case
first; complex fauna is rare, and so precious. Both rest on one structure, a generated phylogeny,
and on the rule the galaxy brainstorm applies to interstellar comets, "a statistical density, made
real only in a ship's immediate surroundings" ([Between the
stars](galaxy-generation.md#between-the-stars)).

### Descent, not assembly

No Man's Sky's creatures are criticised less for being strange than for being unrelated: built from
a parts library, two animals on one world share no more than two on different worlds ([What to take
from the references](#what-to-take-from-the-references)). Real organisms share traits because they
share ancestors, so a naturalist who has seen one beetle can place the next, which is what makes a
biosphere learnable and a catalogue a game. Where unrelated organisms resemble each other, physics
forced it: one thunniform body in whales, ichthyosaurs, lamnid sharks and tunas (Donley et al.
2004), and flight in only four taxa (Maina 2000).

**The tree.** **Lean:** each biosphere has a phylogeny of eight ranks below its root, named for
Earth's principal ranks (domain to species) as rank terms, not claims of kinship. A clade is
identified by its path, the child index at each rank, and its traits are a function of its parent's
and its own stream, so any clade is generated by walking its path from the root and nothing is
stored. Eight, because a crew must perceive each rank: more adds nodes that change nothing visible,
and fewer makes one rank carry two levels of trait, so that siblings differ too much or too little.

- **Conserved traits** are inherited, each changing along a branch with a probability that falls
  towards the root, so that body plans stay as stable as Earth's since the Cambrian:

| Rank    | Decided here (animals; plants and microbes have their own columns of the same table)     |
| ------- | ---------------------------------------------------------------------------------------- |
| Domain  | Cell architecture and informational machinery; simple or complex cells                   |
| Kingdom | Mode of nutrition; which origin of multicellularity, if any                              |
| Phylum  | Symmetry, segmentation, skeleton type, tissue organisation                               |
| Class   | Appendages and their number, respiratory organ, eye type, thermoregulation, reproduction |
| Order   | Feeding apparatus and diet guild, locomotor mode, sensory emphasis                       |
| Family  | Body-size decade, activity cycle, sociality                                              |
| Genus   | Climatic niche, proportions                                                              |
| Species | Size within the decade, colouration, range, abundance                                    |

- **Convergent traits** are computed from the conserved ones, the niche and the world's physics:
  streamlining for a fast swimmer, a wing wherever a clade's appendages can bear one under the
  [flight limit](#body-plans-under-physical-constraint), an image-forming eye wherever there is
  light and the clade moves. The same solution recurs in unrelated lineages, from their own parts.

Continuous traits (log body mass, thermal and moisture optima) drift as Brownian motion, with
variance in proportion to the time a rank spans, reflected at physical limits; discrete ones are
Markov chains. Each clade's origin time is a `clade.traits` draw within its parent's span and after
any stage that gates it, the ranks spaced by default evenly in log age from the epoch's origin to a
species age of about 1 Myr (both parameters). The inherited niche is phylogenetic niche
conservatism (Wiens and Graham 2005).

**Branching.** A clade's number of children is geometric given its age, on `clade.children`: under a
constant-rate birth–death process one lineage's descendants after a time are a zero term plus a
geometric (Kendall 1948), and over clades of random age that mixture is Yule's (1925) hollow curve,
whose tail is a power law. Earth's higher classification "follows a consistent and predictable
pattern" (Mora et al. 2011): taxa multiply faster than exponentially down a kingdom's ranks, and the
numbers of subordinate taxa per taxon follow power laws. Each clade's age follows from its origin
time (above), the means per rank follow Mora et al.'s proportions, and the expected leaf count is
the biosphere's richness. The means are set once per epoch, at the end of the clock window, never at
t: the prokaryotic domains' from the microbial default, the eukaryotic domain's from the sum over
realms of S at each realm's whole area and mean temperature. A clade exists at t only once t passes
its origin time, so a stage reached inside the window adds its clades at its time and moves no
other. The stage gates the tree: complex-cell clades exist only from the `ComplexCells` stage;
complex multicellular kingdoms are independent origins within them, six on Earth ([The transitions,
and whether each is hard](#the-transitions-and-whether-each-is-hard)), while simple multicellularity
"has evolved numerous times" (Knoll 2011) and is a trait any clade may carry; land clades are
independent colonisations from the `Land` stage. How many origins a world gets is for [Whether life
arises](#whether-life-arises-and-how-far-it-gets) to model ([open question 36](#open-questions)).

**Microbes and lateral transfer.** Below complex life a tree is an approximation: prokaryotic
genomes "contain genes from multiple sources", and "the history of life cannot properly be
represented as a tree" (Doolittle 1999). **Lean:** the tree stays the frame, and transfer-prone
traits (metabolic modules, tolerances) may be copied from a donor clade drawn on the recipient's
`clade.transfer` stream by a random descent from the root to the donor's rank, O(depth), never by
enumerating a rank, donors restricted to shallower ranks so that the recursion ends at the root.

**Keys.** **Lean:** the tree is server-only, drawn from the epoch's `LineageSeed`, a block on
`body.life.lineage` per habitable window and attempt, never per epoch ordinal ([Where life sits in
the pipeline](#where-life-sits-in-the-pipeline)), so a re-origin after sterilisation has a new tree
and a changed rate that adds or removes an earlier epoch does not renumber the surviving one. The
seed opens only tags of a new `Clade` scope (`clade.traits`, `.children`, `.transfer`, `.taxon`, and
`.sample`, keyed by site rather than by clade), as R09's `SurfaceSeed` opens only `SurfaceCoarse`
ones. `ObjectKey::clade(path, depth)` packs the path eight bits per rank into the word and the depth
into the sub-object number: 256 children a rank, which a heavy-tailed draw of mean ten exceeds with
probability about 2 × 10⁻³ (computed), so the ranks need unequal widths (a few bits for domains and
kingdoms, eleven or more for species) or the species index the twelve bits of `sub` that the depth
leaves free. Each further three bits cut a power-law tail only about tenfold (computed), so a draw
past its rank's width is truncated to it. Under panspermia the receiving epoch records its donor and
the transfer time, and its tree walks the donor's lineage seed for ranks older than the transfer
(the domain, in practice) and its own below, so two seeded worlds share their deep ancestry and not
their species. Above the transfer the receiver walks the donor's lineage seed with its own means, so
the two worlds share deep clades by path and traits, need not share their counts, and need nothing
of each other's fields. Some worlds change stage within the clock window, so nothing asserts
otherwise: a clade gated by a stage is a time predicate on the lazy tree, and appears at the stage's
time.

### How many kinds, and of what

Earth's counts span orders of magnitude. Eukaryotes number about 8.7 ± 1.3 million species, 86% of
them undescribed (Mora et al. 2011). For prokaryotes a census of 1.7 × 10⁹ 16S sequences gives
0.8–1.6 million taxa at 97% similarity over the gene's V4 region, about 2.2–4.3 million over its
full length, and refutes "trillions" (Louca et al. 2019), against an extrapolation of upward of 10¹²
(Locey and Lennon 2016). **Lean:** the census as the default, being a census, in its full-length
count, since a leaf is a species in the 95% ANI sense of [Microbial
biospheres](#microbial-biospheres-the-common-case) and a short region's 97% cluster lumps more of
them; and the extrapolation as the parameter's upper bound.

**Lean:** a pool's richness, before pruning by productivity, is
S = S_ref·(A/A_ref)^z·exp(−a·(E/k)·(1/T − 1/T_ref))·(1 − exp(−τ/τ_div)):

- **Area.** Species–area slopes are steeper at low latitudes and for larger organisms and depend on
  scale and sampling (Drakare, Lennon and Hillebrand 2006, 794 relations), so z is a parameter by
  organism size and scale. Island slopes run 0.20–0.35 (MacArthur and Wilson 1967), narrowed to
  0.25–0.33 (Rosenzweig 1995), and steepen with scale, about 0.10–0.20 on small islands, 0.25–0.45
  on intermediate ones and above 0.6 where speciation fills large ones (Lomolino and Weiser 2001;
  Rosenzweig 2004), all as Gao and Perry (2016) give them, so z ≈ 0.25 within a realm and a steeper
  z, a parameter, across realms of different area.
- **Temperature.** Ectotherm richness rising as e^(−E/kT) predicts the latitudinal gradient from
  metabolic kinetics (Allen, Brown and Gillooly 2002), but only one of 46 terrestrial data sets fits
  it, and the theory "is a poor predictor of observed diversity gradients" (Hawkins et al. 2007). So
  the exponent carries a factor a below 1: the gradient is real, its kinetic law is not.
- **Time.** τ is the time since the stage that opens the habitat began, and τ_div a parameter of
  tens of millions of years. A microbial world has microbial pools only; one with multicellular life
  only in the sea has no land pools.
- **Productivity.** A taxon whose [density](#ecology-and-densities) times its range falls short of a
  minimum viable population, by default the median 1,377 of Brook, Traill and Bradshaw (2006), is
  dropped in a single pass, the survivors' shares renormalised and never pruned again, so there is
  no iteration. Large animals and top predators are then few where land is small or poor, the
  pattern of Earth's largest species (Burness, Diamond and Flannery 2001).
- **Size.** Most species are small, which the lazy tree makes free. **Lean:** macroscopic pools hold
  taxa above a body-length floor of 1 cm (a parameter); smaller ones are reached through samples.

Geography differs by kind. Earth's vertebrates fall into 20 regions in 11 realms (Holt et al. 2013),
the work of barriers and history, while most microbial taxa "are globally distributed" (Louca et al.
2019). **Lean:** macroscopic pools are per realm and biome, the realms being the coarse field's
landmasses and basins split at barriers, each clade carrying by descent a mask of realms it may
extend to a neighbour or lose; microbial pools are per habitat and environmental class, worldwide. A
pool is composed by descending from the root, sharing its richness among children by expected leaf
count and fit to the realm and biome, at a cost of its size times the depth. A taxon occupies the
cells where a smooth noise field of its own exceeds a threshold set by its range size, so ranges are
patchy and coherent across pool borders. Abundances are log-normal, as Earth's microbes' are (Louca
et al. 2019), and steeply uneven: in Amazonia 227 of some 16,000 tree species are half of all trees,
a ranked abundance fitted by Fisher's log-series (ter Steege et al. 2013).

### Microbial biospheres, the common case

A microbial world shows a camera little. Earth's bacteria and archaea number about 1.2 × 10³⁰ cells,
mostly in the deep oceanic subsurface (4 × 10²⁹), the deep continental subsurface and soil (3 × 10²⁹
each), the ocean (1 × 10²⁹) and the upper oceanic sediment (5 × 10²⁸) (Flemming and Wuertz 2019).
What reaches the surface is a short list:

| Expression                      | Where                                                 | How a crew meets it                                                              | Drawn as                                                                                                     |
| ------------------------------- | ----------------------------------------------------- | -------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| Mats                            | Shallows, tidal flats, hot springs, pans              | Colour bands by eye; from orbit if wide                                          | Material ([Biomes](#biomes-as-places))                                                                       |
| Stromatolites, microbialites    | Carbonate shallows                                    | By eye; a reef 3.43 Ga old of "probable biological origin" (Allwood et al. 2006) | Reef heads of [Microbial biomes, the common case](#microbial-biomes-the-common-case), authoritative at 0.2 m |
| Biofilms and streamers          | Streams, vent outflows, cave walls                    | Close range                                                                      | Decoration                                                                                                   |
| Endoliths                       | Porous rock in deserts                                | Breaking rock: "a narrow subsurface zone" (Friedmann 1982)                       | Nothing until sampled                                                                                        |
| Blooms                          | Oceans and lakes, seasonally                          | From orbit ([Biosignatures](#biosignatures))                                     | Ocean colour                                                                                                 |
| Vent, crust and ice communities | Subsurface ([Habitats](#habitats-beyond-the-surface)) | Plumes, drilling, samples                                                        | Nothing                                                                                                      |

What makes one microbial world unlike another is metabolism, not shape. **Lean:** a microbial
clade's energy metabolism is a `clade.traits` draw from the biosphere's menu, which
`body.life.metabolism` draws from the redox pairs and light that [What life is made
of](#what-life-is-made-of-and-what-it-lives-on) offers, gated by stage: chemotrophs and anoxygenic
phototrophs before `OxygenicPhotosynthesis`, mats of oxygen producers after it, aerobes after
`Oxygenation`, and from `ComplexCells` grazers and algae, the first predators. Envelope, cell
architecture, shape, size and motility are conserved; metabolism, pigments, carbon fixation and
tolerances of temperature, salinity, pH and oxygen are transferable. A hot spring's mats are then
zoned by their taxa's temperature ranges, so a crew reads their colours as a thermometer. Simple
multicellularity (filaments, colonies, fronds) needs no new machinery.

So the crew's instrument is the sample, and a sample must be a function too. **Lean:** a sample
draws on `clade.sample`, keyed by its site, a finest-level `surface_cell`, and an instance. The
site's community, its habitat pool's taxa at log-normal abundances about the pool's, read from each
taxon's smooth field so that neighbouring sites resemble each other, is drawn with instance 0; each
scoop's rarefaction is drawn with one more than the number of samples already taken there, a number
folded from the samples log, so a second scoop of the same mat returns the same community and a
little more of its rare tail. The number of taxa a scoop reveals is that distribution's rarefaction
at the analysis's effective depth, an instrument parameter; its cell density follows the site's
productivity, calibrated to Flemming and Wuertz's totals. Each analysis places a taxon only as deep
as it resolves:

- **Counts and microscopy**: density, size and shape, motility, colonies; a rank of form only.
- **Culture and assays**: growth range, energy source, gas exchange; placement by metabolism, which
  lateral transfer makes a poor guide to ancestry, and the console says so.
- **Molecular analysis**: placement to species, if the crew can read the biosphere's informational
  polymer at all, a question for [Chirality and alien
  biochemistry](#chirality-and-alien-biochemistry). Earth's prokaryotic species are a real unit:
  99.8% of 8 × 10⁹ genome pairs lie above 95% average nucleotide identity within a species or below
  83% between species (Jain et al. 2018).

### Body plans under physical constraint

Each scaling is Earth's, mostly measured over less than an order of magnitude of what a generated
world varies, so the table says where the generator extrapolates.

| Constraint      | Earth's evidence                                                                                                                                                                                                                                                                                                                                                                                           | Rule in the generator                                                                                                                                                                                                                                                                                                                                                                                                  | Beyond Earth's range                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| --------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Support on land | Under geometric similarity stress grows with size, yet peak bone stress stays nearly constant from 0.04 to 300 kg because larger mammals stand more upright, effective mechanical advantage ∝ W^0.25; above that, more skeletal allometry and less agility (Biewener 1989, 2005)                                                                                                                           | Stress ∝ g·M^(1/3) at fixed shape, so a land plan's mass limit ∝ g⁻³, calibrated at 1 g to a stated maximum (a parameter; the largest sauropods are a floor), assuming fixed tissue strength, a fixed safety factor and near-isometric skeletons; approached through posture and then skeletal fraction                                                                                                                | A derivation, untested away from 1 g⊕; actual maxima on Earth are set by land area and diet, and the largest dinosaurs exceed even that (Burness, Diamond and Flannery 2001), so the support limit is a ceiling                                                                                                                                                                                                                                                                                                                                      |
| Flapping flight | The margin between power available and required shrinks with size: a soaring limit near 41 kg and 5.1 m span (Sato et al. 2009); _Argentavis_ at about 70 kg could not sustain flapping (Chatterjee, Templin and Campbell 2007); pterosaurs of 200–250 kg flew (Witton and Habib 2010)                                                                                                                     | The limit ∝ g⁻³·ρⁿ, calibrated to 200–250 kg. Where muscle power binds, required power at the minimum-power speed ∝ g^(3/2)·ρ^(−1/2)·M^(7/6) for geometrically similar wings (momentum theory) against available power ∝ M^(2/3) (frequency ∝ M^(−1/3), Sato et al.'s model), so n = 1, the default; n is a parameter in 0–3, since Sato et al.'s frequency criterion gives n ≈ 3 and a wing-muscle stress limit n = 0 | Extrapolated. At Mars's 0.38 g⊕ and about 0.015 kg/m³ the limit runs from about 18 times Earth's at n = 0, the land limit's own rise, through about a quarter at n = 1 to about 3 × 10⁻⁵ at n = 3 (computed); at Titan's 0.138 g⊕ and about 5.3 kg/m³ it rises about 380 times at n = 0 and 31,000 times at n = 3, against the land limit's 380, so it stays below the land limit for any n in 0–3 (computed); far from Earth the support limit, wing loads and Reynolds number bind first, and the flight limit is never allowed above the land one |
| Oxygen          | Diffusion-reliant organisms need P_O₂ above about 10³ Pa to reach a millimetre, and 10³–10⁴ Pa for centimetre-scale life with circulation (Catling et al. 2005); beetle tracheae scale as M^1.29 (Kaiser et al. 2007); insect size tracked O₂ for 150 Myr then decoupled as birds radiated (Clapham and Karr 2012); the link can be neither accepted nor rejected (Harrison, Kaiser and VandenBrooks 2010) | The thresholds gate macroscopic plans (with [the stage gates](#the-transitions-and-whether-each-is-hard)); a tracheate-type plan's size limit rises with P_O₂ by a parameter exponent, as a ceiling and never an expectation                                                                                                                                                                                           | Contested on Earth, so labelled so                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Light and eyes  | Deep-sea fish pigments cluster at 450–500 nm, where the light is (Douglas, Mullineaux and Partridge 2000); a pigment's thermal noise rises with its peak wavelength, which "presumably explains" why infrared ones do not exist (Luo et al. 2011); a human's long-wave cones peak at 563 nm (Bowmaker and Dartnall 1980), below the Sun's surface photon peak near 685 nm                                  | A clade's visual pigments span the band where the surface photon flux is high, the longest at the lesser of that band's red end and a thermal-noise cap that falls with body temperature, the cap fitted so that Earth's land vertebrates (longest cone near 560 nm) and deep-sea fish (450–500 nm) are reproduced                                                                                                     | Under the Sun the cap already binds for warm endotherms, so under redder hosts their eyes stay; cold ectotherms reach further red, eyes under water follow its light, and none reaches the infrared                                                                                                                                                                                                                                                                                                                                                  |
| Temperature     | Metabolism ∝ M^(3/4)·e^(−E/kT), E about 0.62 eV (Gillooly et al. 2001); top ectotherms outweigh top endotherms on a given land area (Burness, Diamond and Flannery 2001)                                                                                                                                                                                                                                   | Thermoregulation a conserved trait; ectotherm activity follows the monthly temperature of R09's climate field                                                                                                                                                                                                                                                                                                          | Within the limits of [What life is made of](#what-life-is-made-of-and-what-it-lives-on)                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| Water           | Buoyancy lifts the support limit; thunniform bodies converge (Donley et al. 2004); below a Reynolds number near 1, swimming is by cilia and flagella (Purcell 1977)                                                                                                                                                                                                                                        | Aquatic plans have no support limit; streamlining and propulsion computed from Reynolds number                                                                                                                                                                                                                                                                                                                         | Liquid density and viscosity from plan 14 and [Solvents](#solvents)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| Gait            | Walk and run divide near a Froude number v²/(gL) of 0.5 for bipeds, and at the same Froude number in reduced gravity (Kram, Domingo and Ferris 1997)                                                                                                                                                                                                                                                       | Walking speed ∝ √(gL); gait from Froude number                                                                                                                                                                                                                                                                                                                                                                         | At low g walking is slow and bounding natural, as on the Moon                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |

**Lean:** every body plan carries its caps as functions of g, air density, P_O₂, spectrum and
temperature, from plan 14's surface conditions and the planet's atmosphere, including what its
biosphere has done to it; size evolves under them as reflecting limits; and every fact resting on an
extrapolated row is marked internally, so that a correction is a generator-version change.

### Ecology and densities

How many animals of each size stand near a point follows from what the cell produces, as a budget,
not from a density per taxon chosen by hand. Earth supplies the pieces:

- **Metabolism.** Rate ∝ M^(3/4), about 72 kcal a day per kg^(3/4) in Kleiber's (1932) groups,
  though mammalian basal rate goes nearer M^(2/3) (White and Seymour 2003) and most exponents lie
  between 2/3 and 1 (Glazier 2005); with temperature, B ∝ M^(3/4)·e^(−E/kT), E averaging 0.62 eV
  (Gillooly et al. 2001). Endotherms carry a larger normalisation.
- **Density.** For mammalian primary consumers log D = 4.23 − 0.75·log M (D per km², M in grams),
  within about an order of magnitude, so a population's energy use is independent of body size
  (Damuth 1981, 1987); carnivores follow Carbone and Gittleman's rule below instead.
- **Herbivory.** On land consumption grows as aboveground productivity to the 1.38 power (McNaughton
  et al. 1989); aquatic herbivores take on average 51% of primary production, three times
  terrestrial ones' share (Cyr and Pace 1993).
- **Transfer.** A level passes on a mean of 10% in aquatic systems (Pauly and Christensen 1995),
  from below 1% to 52% (Eddy et al. 2021). Of 2,052 estimates, the energy transfers average 5.9%,
  those at sea 8.1% and those on land 1.5%, a mean dominated by plant-to-herbivore links on data
  "substantially underrepresented"; consumers of heterotrophs average 7.2% against 3.5% for
  consumers of autotrophs, and on land endotherms 0.24% and invertebrates 3.8% (Wu et al. 2026). A
  top herbivore eats about eight times what a top carnivore does (Burness, Diamond and Flannery
  2001). Predator biomass goes as prey biomass to near the ¾ power (Hatton et al. 2015); 10,000 kg
  of prey supports about 90 kg of one carnivore species (Carbone and Gittleman 2002); and carnivores
  switch to prey near their own mass at 21.5–25 kg (Carbone et al. 1999).
- **Spectra.** In the sea biomass is roughly constant per logarithmic size class from bacteria to
  whales (Sheldon, Prakash and Sutcliffe 1972), near 1 Gt wet weight in most classes, bacteria
  above it and large marine mammals below (Hatton et al. 2021).

**Lean:** a budget from the cell's productivity, `LifeState.productivity`, which `life_at` computes
on both sides from the surveyed field ([Biomes as places](#biomes-as-places)). Herbivores consume
McNaughton's share on land, read from the cell's productivity as aboveground production in
kJ m⁻² yr⁻¹ (an aboveground fraction by growth strategy and about 40 kJ per g C, both parameters),
its exponent above 1 capped so that the share never passes a ceiling (a parameter, by default one
half, near the aquatic mean), and Cyr and Pace's in water. Above them ε, the share of a level's
metabolised energy that reaches the next, depends on the link (Wu et al. 2026): 8% at sea; on land
3.8% for an invertebrate consumer and 7.2%, the mean for consumers of heterotrophs, for a vertebrate
ectotherm; a land endotherm's ε is fixed by the calibration tests, since Wu et al.'s 0.24% reflects
endotherms' low production efficiency and, through this budget, would give carnivores about a
quarter of Carbone and Gittleman's 90 kg per 10,000 kg (computed). The land mean is never applied
above herbivores, and consumption is converted to metabolised energy by an assimilation efficiency
per diet (a parameter). A taxon takes a log-normal share of its level, independent of mass, and its
density is that share over its metabolic rate averaged over the cell's twelve monthly temperatures,
so that a population does not change with the month. Energy equivalence then yields Damuth's slope
without imposing it, warm ectotherms are fewer than cool ones on the same supply, as carrying
capacity falls with temperature (Savage et al. 2004), and the size spectrum follows from the pool's
sizes. The relations are the tests: herbivorous mammals within an order of magnitude of Damuth's
line, carnivores at Carbone and Gittleman's ratio, and a productive savanna's large herbivores near
the more than 7,500–9,000 kg/km² of Ngorongoro, the Serengeti and pre-war Gorongosa (Stalmans et al.
2019, the first two after Coe, Cumming and Phillipson 1976). A fixed ε makes predator biomass
proportional to prey biomass, not Hatton et al.'s ¾ power, a departure left to
[open question 47](#open-questions). The expected count within r of a point is πr² times the taxa's
densities and occupancies: Damuth's 95 per km² of a 1 kg herbivore and 3 of a 100 kg one put about
75 and 2 within 500 m.

### Instantiation near the observer

An individual exists only where someone could see it. **Lean:** individuals are a Poisson process of
the cell's densities, placed as R11 places rocks: each body-size octave has its own cube-sphere
quadtree level, with cells at least as wide as its home ranges, so a query searches a 3 × 3
neighbourhood across face edges; a cell's count for a taxon is a Poisson draw of density times true
area, read at the cell's canonical point and thinned where the material, its form and substance, is
not the taxon's habitat, which keeps the process Poisson. A large animal's octave cell is tens of
kilometres wide, so density and habitat are read from the surveyed coarse field and `LifeState`,
which a client holds wherever surveyed, not from R11's streamed cells, and no authoritative animal
approaches a craft undrawn. Social taxa are a Neyman–Scott cluster process: group centres at the
density over the mean group size, members about the group's path. Lifespan scales as M^0.15 to M^0.3
(Speakman 2005), so an identity carries a generation, ⌊(t + phase) ÷ lifespan⌋ with a phase per
individual, and a herd turns over with no stored births.

The client must place the same animals without holding the tree. **Lean:** each taxon has a
`TaxonSeed`, a block output on its clade's `clade.taxon` stream, which opens only tags of a new
`Biota` scope, `surface.biota.place`, `.range`, `.path` and `.shape` in the surface registry (R09's
Design note 2, the surface registry's text and its key-form test amended so that `surface.biota.*`
is `Biota`, with R04's Design note 4 listing them among the surface registry's tags), keyed by R09's
`ObjectKey::surface_cell`, as the body's `DetailSeed` alone opens `SurfaceDetail` ones. It reaches
the client in a _drawing record_ for each taxon of its surveyed cells' pools: size, proportions,
locomotion, activity, sociality, habitat, range, the coefficients of its density law (its share of
its level, its level's link and its metabolic normalisation) and an opaque handle for the taxon's
ID. Each side computes the taxon's density per cell from those coefficients,
`LifeState.productivity` and the cell's twelve monthly temperatures. The same animal then stands at
the same place at the same time on both sides, and a console learns what a handle is only through
Knowledge.

**Movement is a pure function of seed and time.** Each individual has a home-range centre and a
radius from the home-range allometry, whose exponent is near 1.0 for herbivores and carnivores alike
(Lindstedt, Miller and Buskirk 1986), steeper than energetics predicts (Kelt and Van Vuren 2001), so
neighbours overlap more as size grows (Jetz et al. 2004). Its path is, per axis, a sum of a few
sinusoids of drawn frequency and phase, bounded by the radius and advanced in _activity time_, the
closed-form integral of a daily schedule: diurnal, nocturnal or crepuscular by local solar time from
plan 14's rotation (on a world without a solar day, [open question 63](#open-questions)), and for an
ectotherm by the month's temperature. Its mean speed makes a day's path follow the day-range
allometry, near M^(1/4) and steepest in faunivores (Carbone et al. 2005), within the gait's Froude
limits. Where the path leaves the habitat, as a land animal's over water, the point is pulled back
towards the centre in a bounded number of fixed steps, sliding it along the shore, the last step
landing on the home-range centre, which thinning keeps in habitat. Migrants shift between two
centres by season ([open question 64](#open-questions)). All of it is `f64` in `hyperion-surface`,
through `hyperion_base::math`, so a replay matches bit for bit.

This loses behaviour: no predator meets its prey, no herd answers another, nothing forages or
remembers the ship. For a crew that mostly watches from orbit or a hovering craft that is the right
price, with one exception.

**Flight, held as session state.** A reaction purely to the ship's present position would push
animals aside and let them spring back the moment it passed. **Lean:** when the ship, a craft or a
landing party comes within an authoritative individual's flight-initiation distance, the server
records a _flight_ (the individual, the time, a bearing away, a distance and a recovery time), one
per Neyman–Scott group for a social taxon so that a herd flees together, and each animal it covers
stands at its path plus a smooth excursion along the bearing, out quickly and back over the recovery
time, when the record expires. The distance grows with the prey's mass and with the threat's size,
speed and directness, which consistently amplify flight, and with group size, except in fish schools
(Stankowich and Blumstein 2005), so a ship scatters a herd from far off. The trigger is evaluated at
tick times on the authoritative ship state; the record is session state, kept in `session.json` and
pushed on the scene topic, not a delta of the overlay, and a replay regenerates it from the same
inputs, so a session holds a handful of live records. Escape from looming objects is widespread
among sighted animals (Fotowat and Gabbiani 2011), so it assumes nothing of alien minds.

### Truth and decoration

The rendering brainstorm's rule holds: every rigid instance 0.2 m tall or more is authoritative
([The line between truth and
decoration](rendering-and-planets.md#the-line-between-truth-and-decoration)). Organisms fall into
three kinds:

- **Sessile organisms** are placed by the flora layer of [Biomes as
  places](#the-per-cell-state-the-material-and-the-instances) from vegetation structure: rigid
  plants and reef heads 0.2 m tall or more as authoritative instances with contact shapes,
  herbaceous cover, mats and crusts as material and decoration. This section maps each plant-like
  clade onto one of the growth strategies by a `clade.traits` draw, and gives each placed instance
  its taxon, drawn by abundance from the pool's taxa of that strategy so that a forest has a real
  one's dominance, with the shape parameters the taxon implies; sessile animals go the same way.
- **Animals 0.2 m tall or more**, and fliers 0.2 m across or more, are authoritative movers: the
  pose above plus any flight record, evaluated by the same code on both sides, with a capsule proxy
  the contact query answers for, always drawn inside R11's could-touch region. A ship that hits one
  kills it, a delta of the individual and the time, so a bird strike on a descending craft is real.
  At the densities above, a could-touch region a few hundred metres across holds some seven of each
  1 kg taxon, tens to hundreds in all, most of them small fliers; each is a closed-form pose.
- **Everything smaller** is decoration: drawn by the GPU from the same densities, never collided
  with, culled against bodies near the ground as pebbles are, and under the view's `DECORATION ON`.

### Hazards and use

Every hazard is judged against human physiology. The realistic ones are chemical, physical and,
rarely, opportunistic infection, never an adapted disease ([Biohazard realism](#biohazard-realism)).

- **Infection.** An organism adapted to another world must also grow at 37 °C in isotonic saline at
  pH 7.4, which it rarely can. **Lean:** infectivity is a rare generated property of a microbial
  taxon, possible only where its growth range covers those conditions; where present it is a severe
  hazard, handled as contamination under the landing party's procedures, with no epidemiology
  simulated.
- **Toxins and venoms.** Defence chemistry is a conserved trait, and its mode decides what it does
  to a human. Specific toxins, tuned to their own world's nerve and receptor proteins, fit human
  ones only by chance. Enzymes digest human tissue only if the biochemistry shares its hand, since
  enantiomeric enzymes "cut only the corresponding substrate enantiomer" (Milton, Milton and Kent
  1992). Generic chemistry acts on anyone: strong acids and bases, oxidisers, membrane-dissolving
  amphiphiles, small molecules that bind metal centres as cyanide binds respiratory iron, and
  accumulated heavy metals; and spines, stings and urticating hairs injure mechanically.
- **Allergens.** Small reactive molecules sensitise by bonding covalently to human proteins, with
  potency set largely by how fast they bind (Chipinda, Hettick and Siegel 2011), which is chemistry
  and not ancestry, so an alien electrophile sensitises as an Earth one does. **Lean:** each defence
  and each inhaled or handled material carries a mode (mechanical, generic, enzymatic, specific,
  hapten), and the hazard table evaluates the modes against human physiology and the `Biochemistry`
  record's hand and scaffold ([Chirality and alien
  biochemistry](#chirality-and-alien-biochemistry)).
- **Predators.** A predator is a hazard when a person falls within its prey sizes, and carnivores
  above 21.5–25 kg take prey near their own mass, so any carnivore of a person's size or more, or a
  social one whose group's mass reaches it, rates as dangerous. With no behaviour simulated that is
  a rating a survey states, not an attack the game stages; attacks wait for whatever design puts a
  landing party on foot.
- **Warning colours.** Conspicuousness evolves with toxicity, as in poison frogs (Summers and Clough
  2001): an input to appearance, and a heuristic a crew can learn and sometimes be fooled by.

Uses (food, materials, pharmacology) are hooks needing the compatibility ladder of [Chirality and
alien biochemistry](#chirality-and-alien-biochemistry); their value is the markets', out of scope.

### Naming, cataloguing and enrichment

**Designations.** A taxon's true ID is its body's `BodyId` and its clade path, which stays on the
server, since a designation that encoded the path would show the crew the phylogeny at first
sighting. **Lean:** a designation is the body's designation followed by a keyed permutation of the
epoch, by window and attempt, and the clade key, a small Feistel network over base's Threefry keyed
once per body on `body.life.designation`: unique across the body's epochs, stable and derived from
the ID, but its digits say nothing of rank or kin, and only the server can invert it. An observation
not yet placed carries an opaque catalogue number, as plan 12's contacts do ([Detail levels for
life](#detail-levels-for-life-and-how-they-degrade)); a taxon designation is attached when analysis
resolves an entry to it, and relationships appear only as the crew's analyses place entries. Proper
names from generated languages, and names a crew gives, are overlays ([open question
67](#open-questions)).

**The catalogue as a taxonomy.** The crew browses the tree as far as it knows it: a node appears
when a member has been placed there, its ancestors inferred, never with a count of unseen siblings
([open question 66](#open-questions)). A sighting places an animal only to the rank its visible
traits decide ([open question 65](#open-questions)), a phylum from its body plan, perhaps a class
from its appendages; a specimen or sample analysed on board places it deeper ([Knowledge, surveys
and the landing party](#knowledge-surveys-and-the-landing-party)). One taxon seen twice may stand as
two provisional entries until analysis merges them, as in a field notebook.

**Enrichment.** The LLM receives a fact sheet, never a seed: each rank's conserved traits, the
convergent ones, size, diet, trophic level, activity, sociality, habitats, the world's gravity,
pressure, temperature and light, and the hazards, filtered to what the crew has established.
**Lean:** text is keyed by (subject, rung, fact-sheet digest), the fact sheet being a fold of
Knowledge: generated at first sighting and again after analysis, regenerated only when the sheet
changes, and stored with its key, so that no description is written from what the ship does not
know; where text and a readout still disagree, the readout is the ship's (item 10 of [What the guide
must gain](#what-the-guide-must-gain)).

**Appearance.** The same traits are the hook for art: symmetry, segmentation, appendages,
proportions from the allometries, integument, eyes, and a palette from the pigment chemistry for
camouflage or warning. A generator from body plan to mesh is out of scope; its input record is not.

## Biosignatures

A biosignature is only as strong as the case against its abiotic explanations, and the design
follows from that. Under the owner's prior most inhabited worlds are microbial, so the commonest
detection a crew makes is an anoxic biosphere seen as methane beside carbon dioxide, perhaps under
an organic haze, with weak oxygen or none: the Archean Earth's signature, and the one with the most
abiotic mimics. This section makes that detection genuinely ambiguous, by generating the false
positives from the same physics as the true signals, and gives the crew a way out through
measurements that become possible as the ship comes closer. The control experiment exists: in its
1990 fly-by Galileo found "abundant gaseous oxygen, a widely distributed surface pigment with a
sharp absorption edge in the red part of the visible spectrum, and atmospheric methane in extreme
thermodynamic disequilibrium" (Sagan et al. 1993). Each is a separate signature below.

### Atmospheric signatures

A short-lived gas is a signature because a large abundance needs a large flux to sustain it, and
the question is always whether something other than life could supply it (Schwieterman et al.
2018). Methane lives under about 1 Myr on a habitable rocky planet about a Sun-like star, and
Earth's methane emissions of about 30 Tmol/yr, "predominantly produced directly by life (including
anthropogenic sources)", dwarf abiotic emissions that are "comparatively tiny" (Thompson et al.
2022).

| Signature                   | Biological source                             | Abiotic sources                                                                                                                                                                                                  | What argues for life                                                                                                                                                                                                                                                                                                                                                                                                |
| --------------------------- | --------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| O₂ (0.76 µm A band)         | Oxygenic photosynthesis                       | Water loss to space; CO₂ photolysis                                                                                                                                                                              | Context, not abundance: water present, no O₄, no CO, a modest O₂ to N₂ ratio (Meadows et al. 2018; Schwieterman et al. 2016)                                                                                                                                                                                                                                                                                        |
| O₃ (0.2–0.35 µm)            | Photochemistry of biogenic O₂                 | As for O₂                                                                                                                                                                                                        | Responds to O₂ far below today's level, so it shows weak oxygenation that the A band cannot (Olson et al. 2018)                                                                                                                                                                                                                                                                                                     |
| CH₄ with O₂                 | Methanogens beside photosynthesis             | None known at Earth's levels                                                                                                                                                                                     | The pair destroys itself and must both be resupplied: Galileo's "extreme" disequilibrium (Sagan et al. 1993)                                                                                                                                                                                                                                                                                                        |
| CH₄ with CO₂ and little CO  | Methanogenesis in an anoxic biosphere         | Serpentinisation, magmatic outgassing, impacts                                                                                                                                                                   | CH₄ above 10⁻³ is "potentially biogenic" and above 10⁻² "likely biogenic", stronger without CO (Krissansen-Totton, Olson and Catling 2018); CO/CH₄ < 1 and a flux above about 10 Tmol/yr (Thompson et al. 2022), though new water cross-sections may suppress abiotic CO too (Ranjan et al. 2020); abundant CO or H₂ beside water is an anti-biosignature, food that a biosphere would eat (Wogan and Catling 2020) |
| N₂O, CH₃Cl                  | Denitrification; marine and fungal metabolism | N₂O: chemodenitrification and lightning (small), and energetic-particle chemistry about young or active stars (Airapetian et al. 2016, via Schwieterman et al. 2018); CH₃Cl: biomass burning, possibly volcanoes | Abundance beyond abiotic supply; both live longer and accumulate further under an M dwarf's ultraviolet (Segura et al. 2005)                                                                                                                                                                                                                                                                                        |
| DMS and other sulphur gases | Marine phytoplankton                          | Evidence of abiotic DMS in comet 67P (Hänni et al. 2024)                                                                                                                                                         | Weak alone; K2-18 b, below, is the cautionary case                                                                                                                                                                                                                                                                                                                                                                  |

At high abundance O₂ "was originally thought to be an unambiguous indicator for life", but sinks
may have kept biogenic O₂ out of Earth's air for billions of years, and abiotic processes can make a
great deal of it (Meadows et al. 2018). The methane pair is the one within reach: about ten
transits of TRAPPIST-1e with JWST's NIRSpec prism "may be sufficient to detect carbon dioxide and
constrain methane abundances", and a CO limit helps exclude abiotic methane (Krissansen-Totton et
al. 2018). Sulphur gases are the warning: Madhusudhan et al. (2025) claimed DMS or DMDS in K2-18 b
at 3σ, and a re-reduction found "87.5% of retrievals using our preferred MIRI binning scheme do
not" favour them, and blamed red noise (Stevenson et al. 2025).

Venus is the Solar System's case. Phosphine reported at about 20 ppb in its clouds, with no abiotic
source found (Greaves et al. 2021), lies 1.3 km/s from an SO₂ line at a resolution comparable to
the gap (Villanueva et al. 2021). SOFIA set a limit of 0.8 ppb at 75–110 km (Cordiner et al.
2022), against a 3 ppb candidate in the same data reduced again (Greaves et al. 2023). The absorber
behind the clouds' ultraviolet contrasts is still unidentified, and microorganisms are among the
candidates discussed (Limaye et al. 2018). **Lean:** a feature that no registry species fits is
reported as an unidentified absorber at its band, a line of evidence that reaches CoLD 1 at most. A
detection resting on one line counts only once a second band of the same species agrees.

Plan 14 has drafted the receiving end, P14.T53.d (`NotModelled` until a life plan exists): from net
surface fluxes of O₂, CH₄ and N₂O in units of Earth's present biotic flux of each, which request 2
asks to be the pre-industrial one, it computes O₂'s steady state against its sinks, O₃ from O₂ and
CH₄'s lifetime in an oxic air (an anoxic steady state is asked of it), and its aerosol rules form
the Archean haze at CH₄/CO₂ ≳ 0.1–0.2 on CO₂ worlds (P14.T24.c, citing Trainer et al. 2006 and Arney
et al. 2016). How the fluxes are set is [A biosphere changes its
planet](#a-biosphere-changes-its-planet)'s question. **Lean: one atmosphere, two kinds of source.**
The same chemistry takes abiotic fluxes from the world's own state for every world, inhabited or
not, so that a serpentinising lifeless world and a living one differ only by the hook's member: a
false positive is emergent physics, never a draw.

### Surface and temporal signatures

Deciduous leaves show "a sharp order-of-magnitude increase in leaf reflectance between
approximately 700 and 750 nm", the red edge; Seager et al. (2005) report it in Earthshine, find time
variability essential to separating it, and warn that some mineral edges are similar in slope and
strength; Earth's own edge, over 50% in a leaf's reflectance, is a few to ten percent in its
disc-integrated spectrum. Under another star the edge may lie elsewhere ([Pigment, colour and the
seasons](#pigment-colour-and-the-seasons) derives it from the light that reaches the ground), so
instruments look for edges at unknown wavelengths. Mats and blooms are signatures of the same kind,
smaller in area and so weaker from afar.

Temporal signatures are a biosphere's response to its seasons: photosynthesis and respiration swing
CO₂ and O₂ in antiphase, a few ppmv either way in Mauna Loa's detrended CO₂ (Olson et al. 2018,
Fig. 1). On a weakly oxygenated world the useful cycle is ozone's: an O₂ oscillation of 5–25 ppm
"imparts potentially observable O₃ seasonality" in the Hartley–Huggins bands at a signal-to-noise
ratio of about 6, but would be "unobservable in the O₂-A band for S/N < 375" (Olson et al. 2018).
Seasons have abiotic mimics too, and Mars shows two rungs disagreeing: Curiosity measured 0.41 ±
0.16 ppbv of methane at Gale with a "strong, repeatable seasonal variation" and spikes near 7 ppbv
(Webster et al. 2018); the Trace Gas Orbiter set a limit of about 0.05 ppbv (Korablev et al. 2019).
**Lean:** temporal signatures need one closed form, not a model of their own. Biogenic and abiotic
fluxes are functions of the season, from plan 14's orbit and obliquity through R09's climate.
T53.d's steady state takes their annual mean, and beside it each gas of lifetime τ adds a seasonal
anomaly ΔF τ cos(ωt − φ) ÷ √(1 + ω²τ²), with ω = 2π over the orbital period and φ = arctan ωτ (the
periodic solution of dN/dt = F − N ÷ τ, derived here), its ΔF summed over latitude so that
hemispheres in opposite seasons cancel in part, as they do on Earth (Olson et al. 2018). For a gas
whose lifetime far exceeds the orbit, as O₂'s and CH₄'s do, this tends to ΔF ÷ ω, the flux
integrated over the season, a quarter-period behind it; O₃ follows its steady state. So a cycle and
its mimics come from one evaluation, at the retarded time when seen from afar (plan 12).

### False positives, made by the same physics

| Mechanism                                                                     | Conditions                                                                                                                                                             | Mimics                                                                                                                                                                   | Discriminant                                                                                                              | Modelled by plan 14 today                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           | Missing                                                                               |
| ----------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------- |
| Water loss in a pre-main-sequence runaway (Luger and Barnes 2015)             | M-dwarf hosts, whose long pre-main-sequence phase holds habitable-zone planets in a runaway for several hundred Myr; weak surface oxygen sinks                         | O₂: "hundreds to thousands of bar", building at about 5 bar/Myr on an Earth-mass planet and up to 25 bar/Myr on super-Earths                                             | O₄ at 1.06 and 1.27 µm, the sign of an O₂ atmosphere "too massive to be biologically produced" (Schwieterman et al. 2016) | Partly. P14.T24.f (built, held until version 22) keeps a runaway world's O₂: the water's oxygen, less an escaping share and a surface sink. P14.T53.a (drafted) replaces it with fractionating escape and with Schaefer et al.'s (2016) melt uptake, timed over the loss, and tests a dimming M dwarf's planet that keeps only the oxygen freed after its mantle froze. Ruling 124.7 still counts the hottest luminosity from the main sequence, so that test needs the pre-main-sequence luminosity of requests 12 and 24. Until then a planet runaway only before the main sequence keeps nothing | The pre-main-sequence luminosity in the worst flux                                    |
| Few non-condensable gases (Wordsworth and Pierrehumbert 2014)                 | N₂ below "a few percent" of Earth's, so the cold trap fails: a moist upper atmosphere needs over 340 K at 1 bar of N₂, about 295 K at 0.1 bar, about 255 K at 0.01 bar | O₂ as the dominant gas: an N₂-poor Earth at today's XUV loses 28% of an ocean in 4 Gyr, 66.2 bar of O₂                                                                   | The O₂ to N₂ ratio, from the Rayleigh slope in transit or O₄                                                              | Drafted. P14.T53.a (23 → 24) takes up the oxygen of temperate worlds whose cold trap fails for want of a non-condensing background, after Wordsworth and Pierrehumbert (2014), on T13.a's nitrogen inventory, 0.5 dex about Earth's                                                                                                                                                                                                                                                                                                                                                                 | None beyond T53.a's build; its N₂ threshold as a cited constant                       |
| CO₂ photolysis (Tian et al. 2014; Harman et al. 2015; Gao et al. 2015)        | K and especially M hosts, whose weak near-ultraviolet leaves CO and O₂ unrecombined; weak ocean sinks, or a desiccated world                                           | O₂ near 2–3 × 10⁻³ in CO₂-rich air (Tian et al., as summarised by Wordsworth and Pierrehumbert 2014); O₃                                                                 | CO at 2.35 and 4.6 µm beside CO₂ (Schwieterman et al. 2016)                                                               | Partly. P14.T53.a (drafted) turns about 40% of a desiccated M-dwarf CO₂ world's CO₂ into CO and O₂ once its hydrogen falls below 1 ppm, after Gao et al. 2015; the wet case is not modelled                                                                                                                                                                                                                                                                                                                                                                                                         | The host's NUV and FUV history; the wet, low-NUV case                                 |
| Abiotic methane                                                               | Ultramafic crust, water and heat (serpentinisation); a reduced mantle; impacts                                                                                         | CH₄: vents give 0.18 and 0.35 Tmol/yr on 1 and 5 M⊕ planets, Hadean estimates about 1.5 Tmol/yr, Hadean impacts about 1.25 Tmol/yr (as compiled by Thompson et al. 2022) | The flux against a ceiling near 10 Tmol/yr; CO/CH₄; the sources' places from orbit; isotopes in situ                      | Partly. T24.f makes methane the carbon carrier of cold worlds beyond the snow line (a Titan), not of temperate ones; P14.T50.a's mantle redox and P14.T51.c's ultramafic crust are drafted; outgassing speciation is P14.T55.a, unscheduled, which would also set the carbon carrier from ΔIW in place of T24.f's rule                                                                                                                                                                                                                                                                              | A serpentinisation flux and redox outgassing of CH₄ and CO into P14.T53.d's chemistry |
| Energetic-particle N₂O (Airapetian et al. 2016, via Schwieterman et al. 2018) | Young or magnetically active hosts, whose EUV, XUV and particle flux dissociate and ionise N₂                                                                          | N₂O                                                                                                                                                                      | The host's activity measured; N₂O without a biological context                                                            | No                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | The host's energetic-particle history                                                 |
| Other mimics                                                                  | Spotted hosts; mineral surfaces; seasonal condensation; abiotic sulphur organics                                                                                       | Spectral features; red edges; cycles; DMS                                                                                                                                | Out-of-transit monitoring; edge shape and its variation                                                                   | Spots no; minerals through R10's materials; condensation cycles by P14.T51.a's `seasonal` flag; sulphur as SO₂ and acid only (P14.T24.g)                                                                                                                                                                                                                                                                                                                                                                                                                                                            | Spot coverage by host activity                                                        |

A spotted M dwarf's contamination of a transit spectrum can be "more than 10× larger than transit
depth changes expected for atmospheric features in rocky exoplanets" (Rackham, Apai and Giampapa
2018), the remote form of "contamination ruled out". The science moves, too: new water
cross-sections remove the low-outgassing O₂ false positive about Sun-like stars (Ranjan et al.
2020), so each mechanism's conditions are cited generator constants. **Lean:** the missing inputs
are asked of their owners, not approximated here. Until they exist the claim fold applies every
mechanism's discriminant to what was measured, and a readout says of a discriminant only
`UNMEASURED` or `NOT MEASURABLE`, never that a mechanism is unmodelled: a model's gap is a fact
about the generator, not the world, kept in the generator's version notes as a speculative setting
is kept off the consoles ([open question 71](#open-questions)).

### What each instrument sees, and from where

Instruments are not designed yet, so this section speaks in their parameters: aperture D,
wavelength range, resolving power, throughput, a starlight-suppression contrast floor and inner
working angle in units of λ/D, and integration time. Three facts of geometry set the rungs.

**Transits** need the observer within an angle R★ ÷ a of the orbital plane: 0.47% of the sky for an
Earth about the Sun, for about 13 hours a year. For a ship that is navigation, not luck: at 10 ly
the band is some 3,000 au wide either side of the plane. **Lean:** the planning display shows each
planet's transit band and next transit, and the crew chooses the geometry. The signal per scale
height, about 2 R_p H ÷ R★², is some 0.22 ppm for an Earth (H = 8.4 km) before the Sun and 70 times
more for the same planet before a star of 0.12 R☉, which makes M dwarfs the reachable targets. A
white dwarf of 0.0126 R☉ raises it some 6,300 times over the Sun's (computed here), which is why
Lin, Gonzalez Abad and Loeb (2014) chose one. With a 6.5 m telescope a feature's signal-to-noise is
"on the order of unity or less per transit - except for the closest stars", so transits are co-added
(Kaltenegger and Traub 2009), and refraction lets an Earth twin be probed to 1 bar "only around the
coolest stars" (Bétrémieux and Kaltenegger 2014). Close in, a transit becomes an occultation seen
from orbit, which sounds the limb height by height.

**Reflected light** needs the planet separated from its star. An Earth twin is about 10⁻¹⁰ of its
star in the visible and 10⁻⁷ in the thermal infrared (Kasting et al. 2009), and its separation must
exceed the inner working angle n λ ÷ D, so the greatest range is a D ÷ (n λ): with n = 3 at
0.55 µm, 9.6 ly per metre of aperture for a planet at 1 au, but 0.48 ly per metre at an M dwarf's
0.05 au, where the planet is brighter against its star (about 4 × 10⁻⁸ by the (R_p ÷ a)² scaling),
and three times less again at 1.65 µm, where methane absorbs. By the photon-counting arithmetic,
integration time grows as the square of the distance while the planet's photons limit it and as the
fourth power once exozodiacal dust does, so the remote rung is slow and selective, and the thermal
infrared, at 0.53 ly per metre for 1 au at 10 µm, wants an interferometer's baseline. A habitable
moon must be separated from its giant, not from its star: Europa, 0.0045 au from Jupiter, has a
greatest range of 0.043 ly per metre of aperture at 0.55 µm (computed here), so a moon's own
spectrum is an in-system rung. A rogue planet has no host, so neither transit nor reflected light
applies; only its own thermal emission does, with no contrast floor.

**In the system** neither limit binds. From 0.1 au a planet delivers some 4 × 10¹³ times the photons
it sends to 10 ly, tens of degrees from its star, so the remote rung's hardest measurements, O₄ and
CO, take seconds to hours, and seasonality an orbit's worth of them. From orbit the disc is
resolved, at 400 km to about 0.27 m for a 1 m aperture at 0.55 µm, and gases are mapped to their
sources.

| Signature                 | Interstellar, transit                                                                        | Interstellar, reflected light      | In the system, disc     | From orbit        | In situ                  |
| ------------------------- | -------------------------------------------------------------------------------------------- | ---------------------------------- | ----------------------- | ----------------- | ------------------------ |
| CH₄, CO₂, CO              | Best about M dwarfs; CO as a limit                                                           | Range ÷ 3 in the near infrared     | Yes, with seasons       | Mapped to sources | Isotopes; flux at a site |
| O₂, O₃, O₄                | O₃ and O₄ about M dwarfs                                                                     | A band, FGK hosts within range     | Yes                     | Mapped            | Yes                      |
| N₂O, CH₃Cl, sulphur gases | Rarely                                                                                       | Rarely                             | At high resolving power | Yes               | Yes                      |
| Red edge, pigments        | No                                                                                           | Weak; needs a time series          | Yes                     | Biome maps        | Pigment chemistry        |
| Seasonality               | No: every transit samples the same orbital phase, and so the same season (Olson et al. 2018) | Over years of epochs, best face-on | Over one orbit          | Per region        | Diel cycles              |

**Lean:** a measurement is one pure function, a spectral retrieval, from the true atmosphere and
surface, the geometry, the instrument's parameters, the integration time and the observation's key
in the session's stream to a measured value with its uncertainty, or an upper limit, per quantity.
Its error draws from that stream at that key
([Determinism of play](single-player-experience.md#determinism-of-play)), so a result is
reproducible and is not the truth with error bars drawn about it. A readout states only that,
`CH4 ~2100 µmol/mol ± 600 µmol/mol` or `CO < 400 µmol/mol`, with its light age when seen from afar,
in plain formulae and mole-fraction units ([What the guide must gain](#what-the-guide-must-gain),
items 5 and 6) and R10.T13's `±`, since its typeface sets no subscripts or superscript minus.

### A confidence ladder

The Confidence of Life Detection scale proposed by Green et al. (2021), of NASA, is a proof of
concept, offered "not as a prescription", with seven benchmarks; its six qualitative labels each
span two adjacent levels in the paper's Fig. 1:

| Level | Measurement indicator (Green et al. 2021, Fig. 1)                                              |
| ----- | ---------------------------------------------------------------------------------------------- |
| 1     | Detection of a signal known to result from a biological activity                               |
| 2     | Contamination ruled out                                                                        |
| 3     | Demonstration or prediction of biological production of signal in the environment of detection |
| 4     | All known non-biological sources of signal shown to be implausible in that environment         |
| 5     | Additional, independent signal from biology detected                                           |
| 6     | Future observations that rule out alternate hypotheses proposed after original announcement    |
| 7     | Independent, follow-up observations of predicted biological behavior in environment            |

| Qualitative assessment (Fig. 1)        | Spans levels |
| -------------------------------------- | ------------ |
| Detection of biogenic signal           | 1–2          |
| Environmental relevance                | 2–3          |
| Abiotic false positives discrimination | 3–4          |
| Independent biosignature               | 4–5          |
| Alternate hypotheses eliminated        | 5–6          |
| Confirmation of presence of biology    | 6–7          |

Its principles suit a game unusually well: a result at any level has value, may "move to lower
levels as those results are critically evaluated", and the top levels need others' participation.
Its authors place ALH84001 at levels 1–3, "but clearly not" higher, and call level 4 "a significant
challenge, especially for bodies/environments with remote sensing data only" (Green et al. 2021).

A ship has no community, so the top levels are adapted. **Lean:** each signature carries a rule in
the generator's tables, after the Ladder of Life Detection's decision rules (Neveu et al. 2018): the
measurement for level 1; contamination excluded for 2 (the host's spots out of transit, a second
instrument, a sample's blanks); the context for 3 (liquid water, temperature); a discriminant per
abiotic mechanism for 4; what counts as independent for 5; for 6, the alternatives the ship's own
analysis raises at level 5, each excluded by a named measurement; and for 7 a prediction of the
biological hypothesis, a seasonal phase or organisms at a second site of the habitat, then observed.

**Lean: how the console states it.** The server folds the ship's recorded measurements through those
rules, never consulting the truth, so a false positive reaches the level its evidence earns and
falls when a discriminant unmasks it: a record's Knowledge level never falls, but a claim can. The
claim is advisory: a crew may annotate it but never overrides it, and an agnostic measure such as an
assembly index (Marshall et al. 2021) is one line of evidence, never a level of its own ([open
question 21](#open-questions)). `LIFE` gives the claim as an inference with its rung after a middle
dot, as [Fit with the consoles](#fit-with-the-consoles) proposes (`BIOSPHERE CANDIDATE · LEVEL 3/7`,
under a heading that names the scale in words, `CONFIDENCE OF LIFE DETECTION`, since upper case
would turn CoLD into `COLD`), with the level's own measurement indicator in words, each line of
evidence, each abiotic mechanism still open with the measurement that would close it
(`ABIOTIC CH4 NOT EXCLUDED: CO/CH4 UNMEASURED`), and what the next level needs. `CONFIRMED` is level
7 alone. Absence is bounded, never resolved: a world that has shown nothing reads
`NO BIOSIGNATURE ABOVE LIMITS` with the limits, and "no biosphere" never reaches a client.

The common path, for an anoxic microbial world about a K dwarf: CH₄ and CO₂ by co-added transits,
with a CO limit (1); out-of-transit monitoring excludes spots (2); water and temperature show a
habitable surface (3); the implied CH₄ flux exceeds the abiotic ceiling, with CO/CH₄ < 1, which
makes serpentinisation, outgassing and impacts implausible together (4); from orbit, a seasonal
methane cycle over shallow seas, or a mat pigment (5); the cycle's own abiotic reading, a regolith
that takes up and releases methane with the seasons (Moores et al. 2019), excluded by mapping its
sources as diffuse sediments, not dry ground (6); and a sample from a predicted site, with cells,
homochirality and light isotopes (7).

### Technosignatures

A hook for the civilisations brainstorm. Galileo found "narrow-band, pulsed, amplitude-modulated
radio transmission" that "seems uniquely attributable to intelligence" (Sagan et al. 1993), and,
for an Earth-like planet transiting a white dwarf, JWST could detect or constrain industrial CCl₃F
and CF₄ at about ten times Earth's level in about 1.2 and 1.7 days of integration (Lin, Gonzalez
Abad and Loeb 2014). Either is a source term or a radio band, measured by the same retrieval and
assessed on the same ladder.

## Resources and deposits

P14.T26 gives each body a `ResourceAbundances`, a mass fraction of its accessible outer layers for
each of a closed list, and plan 14's composition ruling names what the crust is made of:
`CrustComposition`'s primary, secondary and tertiary lithologies, its provinces and its redox class,
as rows of one substance registry (P14.T51.c), beside the ices and liquids of its condensates
(P14.T51.a). They say nothing about _where_, and almost nothing worth finding is at the average. An
ore is rock that a process has enriched far above crustal abundance, as the median porphyry copper
grade of 0.44% (Singer, Berger and Moring 2008) is some 150 times the upper crust's 28 ppm (Rudnick
and Gao 2014). Processes put deposits in places, and the coarse field already holds the
places: boundaries and their kinds, each cell's crust, mapped to the crust record's lithologies by
R09's palette, climate classes, drainage, terminal basins and craters ([the coarse
pass](rendering-and-planets.md#the-coarse-global-pass-once-per-planet)).

### Ore-forming processes on the coarse field

Each process is a rule in three parts: a **setting** read from the coarse field's cells and the
lithology each names, **requirements** on the body read from plan 14's figures, crust record and
element inventory and from the biosphere's stage, and a **deposit model**, the grade–tonnage
distribution of [Deposits as places](#deposits-as-places).

| Process                         | Where on the coarse field                                                                                                                                                                                               | Needs                                                                                                                                                                                                  | Yields, and the Earth that calibrates it                                                                                                                                                                                                                                  |
| ------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Porphyry Cu–Mo–Au               | The arcs R09 places behind each subducting trench, with volcanic fronts 190 km (oceanic) or 260 km (continental) behind it                                                                                              | Subduction; water                                                                                                                                                                                      | Some 60–75% of Earth's copper (John et al. 2010; Sillitoe 2010), with half its molybdenum and perhaps a fifth of its gold, from the upper 4 km or so of arc crust (Sillitoe 2010); 422 well-explored deposits, median 240 Mt at 0.44% Cu (Singer, Berger and Moring 2008) |
| Volcanogenic massive sulfide    | Divergent boundaries, back-arcs and rifted arcs; on a stagnant lid, volcanic provinces under water                                                                                                                      | Standing water over magmatic heat                                                                                                                                                                      | 1,090 compiled (Mosier, Berger and Singer 2009), the survivors mostly from arcs and back-arcs (Galley, Hannington and Jonasson 2007); about 6 × 10⁸ t in today's neovolcanic zones (Hannington et al. 2011)                                                               |
| Magmatic Ni–Cu–PGE and chromite | `Province` cells and layered intrusions, richest where P14.T51.c's secondary crust is `komatiite`; ophiolites for podiform chromite; the melt sheets of large complex craters                                           | Magma reaching sulfide saturation, so the body's sulphur (P14.T50.a) and the crust's; neither water nor life                                                                                           | The Bushveld holds 72% of the world's platinum-group resources and Noril'sk, in the Siberian Traps, 10% (Zientek et al. 2017); Sudbury's ore lies in an impact melt dated 1,850 Ma (Davis 2008)                                                                           |
| Rare earths and thorium         | Carbonatites and alkaline complexes, most within stable plates, often on intracontinental rifts; laterite over granite or an alkaline intrusion under humid tropical (A) classes; monazite in river and coastal placers | A carbonatite or alkaline melt, needing neither water nor life; for the clays, tropical weathering under moderate to high rain                                                                         | Carbonatites the main light-REE source since the 1960s and southern China's ion-adsorption clays the heavy; thorium, insoluble where uranium moves, rides in their monazite, at up to 20% ThO₂ (Van Gosen et al. 2017)                                                    |
| Banded iron formation           | Continental shelves and intracratonic basins under the sea (Superior type); submarine volcanic belts (Algoma type)                                                                                                      | Fe(II) in an anoxic ocean, and an oxidant (below)                                                                                                                                                      | Giant formations of 2.6–2.4 Ga on passive margins (Bekker et al. 2010); median 170 Mt at 53% Fe, supergene-enriched ore included (Mosier and Singer, in Cox and Singer 1986)                                                                                              |
| Sedimentary manganese           | Continental shelves under the sea where oxygen-poor deep water wells up                                                                                                                                                 | Mn²⁺ in an oxygen-poor deep sea, precipitated where it mixes with oxidised surface water (Cannon, Kimball and Corathers 2017), so after `OxygenicPhotosynthesis`; which stage gates it is for research | The Kalahari's 70% of identified resources, interlayered with banded iron; marine beds from as long as 2.5 Ga ago (Cannon, Kimball and Corathers 2017)                                                                                                                    |
| Phosphorite                     | Continental margins under the sea where upwelling feeds high productivity (O'Brien and Veeh 1980)                                                                                                                       | High productivity, so a biosphere, and on Earth only once oxygen had risen; phosphorus in the inventory (request 14)                                                                                   | Phosphogenic events about 2.0 Ga and, larger, in the Neoproterozoic, tied to the stepwise rise of oxygen (Papineau 2010)                                                                                                                                                  |
| Evaporites and brines           | R09's terminal outlets under arid (B) classes; restricted marine embayments                                                                                                                                             | Evaporation above inflow; chlorine (P14.T50.a); seawater sulfate for gypsum                                                                                                                            | From seawater, gypsum once about 81% has evaporated, halite at 90%, polyhalite beyond 95% and potassium chlorides only from the last bitterns, beyond about 98% (Usiglio's data, in Stewart 1963); Uyuni's lithium brine (Orris 1995)                                     |
| Placers                         | Channels below eroding primary deposits, a past wet epoch's included                                                                                                                                                    | Flowing water; a source                                                                                                                                                                                | More than two thirds of the world's gold supply, fossil placers included (Yeend and Shawe 1989)                                                                                                                                                                           |
| Laterite and bauxite            | Humid tropical (A) classes on old, low-relief surfaces                                                                                                                                                                  | Over 1,100–1,200 mm of rain a year above 17–22 °C (Brlek et al. 2021); oxygen for iron caps                                                                                                            | About 90% of the world's bauxite (Foley et al. 2017)                                                                                                                                                                                                                      |
| Sediment-hosted Zn–Pb and Cu    | Rift and passive-margin basins of carbonate or red beds beside evaporites                                                                                                                                               | Basinal brines and seawater sulfate, so after `Oxygenation`: lead–zinc from 2.02 Ga (Leach et al. 2010), red-bed copper from the oxygenated aquifers of Hayes et al. (2015)                            | Over half the world's zinc and lead reserves (Emsbo et al. 2016); about a fifth of its copper (Hayes et al. 2015)                                                                                                                                                         |
| Uranium                         | Continental basins below eroding granite (placers); redox fronts and unconformities in sedimentary basins; sandstone aquifers                                                                                           | A fractionated granite as its source; before `Oxygenation`, detrital uraninite; after it, oxidised groundwater and a reductant, plant debris after `Land` or migrating hydrocarbons                    | Placers only 3.1–2.2 Ga, the first redox-front ore at Oklo about 2.0 Ga, the large unconformity ores late Palaeoproterozoic to early Mesoproterozoic, roll fronts essentially after land plants (Cuney 2010)                                                              |
| Coal                            | Subsiding wet lowlands                                                                                                                                                                                                  | Land plants abundant enough to build peat, lignin not required (Nelsen et al. 2016); an everwet climate                                                                                                | From the Early Devonian, greatest in the Carboniferous–Permian (Nelsen et al. 2016)                                                                                                                                                                                       |
| Petroleum                       | Sedimentary basins buried into the oil window                                                                                                                                                                           | Marine or lake productivity, anoxic preservation, 60–150 °C (Tissot and Welte 1984, not re-checked)                                                                                                    | Six intervals, a third of Phanerozoic time, sourced over 90% of reserves (Klemme and Ulmishek 1991)                                                                                                                                                                       |
| Abiotic hydrocarbons            | Seas, lakes and dunes of a cold methane-bearing world                                                                                                                                                                   | Methane photochemistry; already plan 14's condensates (P14.T51.a) and tholin cover (P14.T51.d)                                                                                                         | Titan: dozens of seas and lakes each hold more than Earth's known oil and gas (Lorenz et al. 2008)                                                                                                                                                                        |

**Some deposits need life, and the evidence for how is uneven.** Biochemical processes, oxygen above
all, may account directly or indirectly for most of Earth's 4,300 known mineral species (Hazen et
al. 2008). Giant banded iron formations were laid down about 2.6–2.4 Ga, vanished about 1.85 Ga and
returned briefly with the Neoproterozoic snowballs (Bekker et al. 2010), and their oxidant is
unsettled: oxygen from cyanobacteria is still the main view (Konhauser et al. 2017),
photoferrotrophs alone could have made it all (Kappler et al. 2005), abiotic ultraviolet
photo-oxidation is negligible (Konhauser et al. 2007), and an iron-silicate precursor is a contested
alternative (Rasmussen et al. 2015). **Lean:** banded iron formation is gated on the stage, mostly
from `OxygenicPhotosynthesis` until the deep ocean loses its iron some time after `Oxygenation`,
with a photoferrotrophic share from the first phototrophy (the epoch's dated `phototrophy`, [The
output](#the-output)) and a small abiotic share, both generator parameters, since the evidence
admits all three.

Oxygenation flips deposits whose metal moves in one redox state: detrital pyrite and uraninite
survive long transport only below about 3.2 × 10⁻⁵ atm of oxygen and short transport below 10⁻² atm
(Johnson et al. 2014); red-bed copper appears only after it (Hayes et al. 2015), sediment-hosted
lead–zinc first at 2.02 Ga (Leach et al. 2010), and gypsum was rare while seawater sulfate stayed
low before about 1.9 Ga (Grotzinger and Kasting 1993). Uranium moves as uranyl only once oxygen has
risen: its detrital placers formed only before that, its first redox-front ore at Oklo about 2.0 Ga,
and its roll fronts need a reductant in the aquifer as well, essentially buried plant debris after
`Land`, or less often migrating hydrocarbons (Cuney 2010). Supergene copper follows oxygen: its
enrichment needs sulfides weathered under oxidising conditions above the water table
(Sillitoe 2005), and its oxidised ores, malachite and azurite among them, may not have formed to any
extent before oxygenic photosynthesis (Hazen et al. 2008). Coal waits for a peat-forming flora after
`Land`, wood not required, its Palaeozoic peak set by everwet tropics and subsiding basins (Nelsen
et al. 2016, against Floudas et al. 2012), which the coarse field can evaluate. Petroleum needs
productivity, anoxic preservation and burial; it is meagre before `OxygenicPhotosynthesis` and after
it episodic: six intervals, a third of Phanerozoic time, sourced over 90% of Earth's reserves,
almost 70% of them generated since the Coniacian (Klemme and Ulmishek 1991). **Lean:** petroleum's
yield follows the stage's productivity times an anoxic-interval draw, and its survival falls with
the source rock's age. The abiotic inventory of Titan's kind (Lorenz et al. 2008) is plan 14's, in
P14.T26's hydrocarbon fraction, which knows nothing of life, and in P14.T51.a's liquids and
P14.T51.d's tholin; this layer's coal and petroleum are separate deposits gated on stage, and how a
readout sums them is open ([open question 54](#open-questions)).

**Water divides worlds more than plates or life do.** With no water there are no hydrothermal ores,
as the Moon shows. With water but no plates, as on early Mars, hydrothermal systems still run at
provinces and craters, so stagnant-lid sulfides, impact-hosted ores, evaporites, placers and clays
are permitted and porphyry is not, while large igneous provinces, over 10⁵ km³ in pulses of 1–5 Myr
(Bryan and Ernst 2008), need only magma. **Lean:** each row's requirements are predicates over plan
14's figures (liquid water or a `WetEpoch`, tectonic regime, volcanism level, heat flow), its crust
record (komatiite nickel only where P14.T51.c's secondary crust is `komatiite`, which it makes where
the mantle is hot) and the stage, at a formation age drawn in the window they allow. Survival to the
epoch falls with erosion since formation, so Meso-Cenozoic porphyries dominate the record, younger
arcs being least eroded (Sillitoe 2010), and seafloor deposits with their plate's age.

### Volatiles

**Cold traps** are plan 14's: P14.T51.f gives an airless body's cold-trapped condensates and their
area from `hyperion_surface::cold_trap` (Hayne, Aharonson and Schörghofer 2021's shadows at
Ingersoll, Svitek and Murray 1992's temperature, against each species' sublimation threshold, 110 K
for water after Schörghofer and Williams 2020), and R09.T13.f places them by cell, from the closed
form R09.T7.d builds early. Three of P14.T51.f's inputs are labelled stand-ins: the present
obliquity, though traps record its history; an exposed fraction f_x drawn log-uniform over
0.016–0.45, empirical "until a delivery and gardening budget exists"; and the trapped reservoir, a
placeholder until it is sourced from neutron and LCROSS figures. For a moon the tilt that matters is
its spin axis's to its planet's orbit: the Moon's is about 1.5° to the ecliptic (Schörghofer and
Rufu 2023), against its 6.7° obliquity to its orbit, and R09's reference Moon carries 1.535°. The
right input is the highest solar elevation, which R09.T7.d's closed form already takes, but T51.f
passes the obliquity, at which its own lunar test would fail. A generated Moon would miss its traps
even so, since P14.T14.a draws a moon's obliquity Rayleigh about 10° and keeps it once locked, where
a synchronous body settles into a Cassini state of small obliquity, Titan's about 0.3° (Stiles et
al. 2008) and Mercury's 2.11′ (Margot et al. 2007); and inside its planet's Laplace radius
(P14.T56.a) a moon's sun climbs as high as its planet's obliquity allows. The history and the other
two stand-ins matter as much. Ice shows at the optical surface in only about 3.5% of lunar cold
traps (Li et al. 2018), while Mercury's traps hold tens of metres, some 41 m in nine craters
(Deutsch et al. 2018), of nearly pure ice delivered within about 150 Myr (Deutsch, Head and Neumann
2019). And traps are younger than their craters: the LCROSS impact site in Cabeus has been dark for
only about 0.9 Gyr, and the Moon's permanent shadow was half today's 2.2 Ga ago (Schörghofer and
Rufu 2023). **Lean:** T51.f and R09.T13.f pass the greatest solar declination, from a synchronous
body's Cassini state rather than a drawn obliquity (request 19); and the budget is a column drawn
from a log-normal whose median falls with the time since the last large delivery (a comet, or
outgassing) and whose exposed share falls with gardening, bounded by the trap's age, which needs the
declination's history. It belongs with P14.T51.f and P14.T55.c, and this layer only reads the
trapped ice by cell as a volatile deposit. Traps under several suns, and on a world locked 1:1 to
its star, are open ([open questions 62 and 63](#open-questions)).

**Ground ice** lies where the annual mean at depth stays below the frost point, as on Mars poleward
of about ±60° in the uppermost metre (Feldman et al. 2002) and, at mid-latitudes, over 100 m thick
beneath 1–2 m of cover (Dundas et al. 2018). **Lean:** an ice-table depth per coarse cell from the
climate and the air's water, server-side, as it changes neither height nor material.

**Regolith helium-3.** A stellar wind implants helium in an airless surface, about 4.6 × 10⁻⁴ of it
³He (the Sun's wind as Genesis collected it, 4.48–4.77 × 10⁻⁴ from fast to slow; Heber et al. 2012),
nearly three times the protosolar 1.66 × 10⁻⁴ that a giant keeps. The Moon's total is put at
10⁶–2.5 × 10⁶ t, at 1.4–15.1 ppb at the landing sites (Wittenberg, Santarius and Kulcinski 1986;
Slyuta, Abdrakhimov and Galimov 2007), and Fa and Jin's (2007) fit over 25 Apollo soils is the
closed form wanted for a lunar-like body, extrapolated beyond lunar temperatures, titanium and
exposure: C = 0.56 TiO₂ F ÷ OMAT + 1.62 ppb, TiO₂ in wt%, F the normalised solar-wind flux and OMAT
the optical maturity, which falls as a soil matures (via Grava et al. 2021; OMAT's direction from
memory). **Lean:** P14.T26's helium-3, which names only giants, gains the airless case: a body mean
by that fit, with F from the host's wind at the body, scaled to the Sun's at the Moon, and OMAT a
decreasing function of P14.T51.d's maturity, which reads that wind times the surface age and reduces
it under a magnetic field or an atmosphere, plus a regolith depth, a temperature cut for hot bodies
and a shielding factor for a moon inside its giant's magnetosphere. This layer spreads the mean over
cells by their provinces' titanium and crater state, which needs a titanium indicator on P14.T51.c's
basaltic provinces, since the registry's `basalt` has no high-titanium variant. Both need a
main-sequence wind from the stellar stage ([What the earlier layers owe
us](#what-the-earlier-layers-owe-us)).

**Gas-giant fuel.** P14.T26 gives giants their helium-3 and deuterium fractions, and the
single-player brainstorm leans on [skimming](single-player-experience.md#refuelling) a giant for
hydrogen. Jupiter's ³He/⁴He of 1.66 × 10⁻⁴ (Guillot and Gautier 2015) and D/H of 2.6 × 10⁻⁵ from the
Galileo probe (Mahaffy et al. 1998; 2.25 × 10⁻⁵ from ISO, Lellouch et al. 2001, via Guillot and
Gautier 2015), with a helium mole fraction of 0.1359 (von Zahn, Hunten and Lehmacher 1998, which
confirms P14.T50.b's He/H₂ of about 0.16), make about 3 × 10⁻⁵ of a scoop's mass ³He and 3–4 × 10⁻⁵
deuterium, a tonne of ³He per some 34,000 t of gas (arithmetic here). Uranus and Neptune hold more
deuterium, 4.4 and 4.1 × 10⁻⁵ (Feuchtgruber et al. 2013), and cost less energy to leave, so NASA's
mining studies prefer them (Palaszewski 2015); fusion fuel as a consumable is a ship ruling.

### Small bodies

Plan 14 types small bodies itself: P14.T51.e gives each member a taxonomic surface from its
formation distance against the snow line, its size (a differentiated body keeps a basaltic or
metallic surface) and its volatile retention, from the registry's assemblages and lithologies
(`s_type`, `c_type`, `iron_nickel` and the rest). The Solar System calibrates it: in the Bus–DeMeo
classes (DeMeo et al. 2009), the main belt's types run E, S, C, P, D outwards and its mass is
C 52.5%, B 11.1%, P 11.0%, V 9.6%, S 8.4% and M 3.3% (DeMeo and Carry 2013), though Ceres, Pallas,
Vesta and Hygiea heavily skew those shares, so a generated belt is better tested on the paper's
shares with the four removed; samples tie S types to LL chondrites (Nakamura et al. 2011) and Ryugu,
a C-complex body, to CI (Yokoyama et al. 2023); and Psyche, the metal-rich exemplar, is only 30–60%
metal by volume (Elkins-Tanton et al. 2020), so a metallic surface is not a body of metal. **Lean:**
this layer adds no typing, only what a type holds: each assemblage carries its meteorite analogue's
composition, so a metallic surface is Fe–Ni of 5–18% nickel (McCoy et al. 2022), over a bulk as
mixed as Psyche's, and P14.T26's platinum-group rule then holds body by body. Comets, which plan 14
makes apparitions on `system.comet` rather than bodies, have no composition yet ([open question
75](#open-questions)).

### Deposits as places

Deposits are discrete places on Earth, and well described as populations: the US Geological Survey's
60 models, from over 3,900 well-explored deposits, make each type log-normal in tonnage and in most
grades (Cox and Singer 1986), the two nearly independent within a type (porphyry copper's r = 0.1 is
not significant; Singer, Berger and Moring 2008), widely spread and top-heavy, the top tenth by
contained metal, all types pooled, holding over 86% of the gold and 84% of the copper (Singer 1995).
**Lean:** each body with a solid surface gets a deposit list beside its crater list, each entry a
centre, a type, a formation age, a tonnage with grades by element, its ore and host minerals as
registry rows, a depth to the top, an extent, and the extent of its surface expression (a halo, a
gossan, an anomaly), far wider than the ore and what sensors find first.

**How many, and where.** A setting defines permissive tracts on the coarse field, each a run of
cells whose crust is the lithology its process needs: an arc segment, a ridge or back-arc, a
province, a terminal basin, the channels below a primary deposit. Density is not constant: porphyry
runs 2–128 deposits per 100,000 km² of permissive rock, median 15, falling as the tract grows
(Singer et al. 2005; Singer 2008). The models pool deposits worldwide into one grade distribution
per type, so the data support richer provinces having more deposits, not richer ore. **Lean:**
counts are Poisson in each contiguous tract about the type's regression on tract area and median
deposit size (Singer 2008), calibrated on the reference Earth and scaled by the body's P14.T26
fraction for the commodity over the reference value (and P14.T50.a's sulphur for the sulfide types,
its chlorine for the evaporites), once request 17 keys T26 by element, since it has no base- or
precious-metal group today, with positions uniform in the tract and grades and tonnages from the
type's model unscaled; below the tonnage that 90% of the type's deposits exceed (the USGS's 90th
percentile, 33 Mt for porphyry copper; Singer, Berger and Moring 2008), occurrences are a
statistical density made real near a landing party.

**The deposits realise plan 14's composition.** They realise the crust record, since a type is
placed only where a cell's crust is its lithology, and P14.T26's fractions, which they never
approach: as log-normals, the porphyry model's 422 deposits hold about 1.5 × 10⁹ t of copper,
against some 4.5 × 10¹³ t in the continents' upper 4 km at 28 ppm (arithmetic here), a few parts in
10⁵, under 1% with a hundredfold allowance for the undiscovered. The fractions stay the mean and the
deposits the tail, and a test bounds the list's metal by a stated share of fraction times layer
mass. Two gaps remain: the layer's depth is unstated, and P14.T26's groups do not map onto the
element registry, which has zinc and nickel but no copper, molybdenum, cobalt, lead, gold, silver,
lithium, uranium, thorium, platinum-group or rare-earth row, so grades and minerals cannot yet be
keyed.

**Determinism.** Deposits are truth, drawn on the server from the surface seed's `SurfaceCoarse`
streams, which the content passes that follow the coarse pass open as it does, on a new tag,
`surface.coarse.deposit`, keyed by `surface_item(n)` with n packing the process number, the tract's
anchor cell and the slot, so that a process added later moves no deposit ([Runtime and code
shape](#runtime-and-code-shape)). They follow the coarse pass, whose fields they read, and take the
stage at their formation age from the pure function of [Whether life
arises](#whether-life-arises-and-how-far-it-gets). Campaign time does not move them; play changes
them only by deltas, a depleted deposit keyed by that draw key, which survives any change that
leaves its tract ([Overlays and persistence](#overlays-and-persistence)). An Earth-like world holds
of order 10³–10⁴, some tens of bytes each, cached beside the field under the system's `ContentKey`.

**What the client receives.** The coarse field reaches a client exact because its synthesis reads it
([Knowledge, and the surface seed](rendering-and-planets.md#knowledge-and-the-surface-seed)). R09's
palette `Deposit` role names surface materials, a Mars's dust or a Ceres's carbonate; where an ore
deposit's ground is one, such as an evaporite crust, that entry is its authoritative surface, and
nothing else drawn reads an ore body; whether a surveyed halo or gossan is drawn is open ([open
question 76](#open-questions)). **Lean:** the list never leaves the server. A client learns of a
deposit as a survey result, a prospecting observation in the observations log with a position, an
extent and an uncertainty. From orbit and drones it becomes an `EXPLORATION TARGET`, tonnage and
grade given as ranges, as the JORC Code reports mineralisation not yet sampled enough to estimate
(JORC 2012, clause 17). Limited samples at an outcrop or a drill site make it `INFERRED`, implying
its continuity without verifying it, `INDICATED` once denser sampling lets continuity be assumed and
`MEASURED` once sampling confirms it (clauses 21–23), each grade an estimate in the samples log
(`Cu ~0.6 % ± 0.3 %`), in the categories the [consoles section](#fit-with-the-consoles) proposes:
the ladder of [Knowledge, surveys and the landing party](#knowledge-surveys-and-the-landing-party)
applied to rock, free to degrade.

### Prospecting

| Method                                | Senses                                                                                   | Depth sensed                                                                         | From orbit                                                                                                      | Notes                                                                                     |
| ------------------------------------- | ---------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| Reflectance spectroscopy, 0.43–3.0 µm | Iron-bearing silicates, hydrated and alteration minerals, carbonates, salts, surface ice | The upper microns (Elkins-Tanton et al. 2020)                                        | 70 or 140 m pixels from 100 km (Moon Mineralogy Mapper, PDS catalogue)                                          | Needs light; in permanent shadow, scattered light (Li et al. 2018)                        |
| Gamma-ray spectrometry                | K, Th and U; Fe, Ti and others excited by cosmic rays                                    | Decimetres                                                                           | About 150 km from 100 km (Lunar Prospector, PDS catalogue), about 80 km from a low orbit (Lawrence et al. 2003) | Lunar Prospector mapped Th, K and Fe (Lawrence et al. 1998); closed by a thick atmosphere |
| Neutron spectrometry                  | Hydrogen, so ice and hydrated minerals                                                   | The uppermost metre (Feldman et al. 2002)                                            | As gamma rays                                                                                                   | Found the lunar polar hydrogen (Feldman et al. 1998)                                      |
| Magnetometry                          | Magnetised crust, magnetite-rich iron formations, some sulfides                          | Kilometres                                                                           | About the altitude (Steele et al. 2024)                                                                         | Mute where the crust was never magnetised                                                 |
| Gravimetry                            | Density contrasts: chromitite, massive sulfide, empty tubes                              | Kilometres                                                                           | 9–15 km at degree 660 (Konopliv et al. 2013)                                                                    | GRAIL's lava-tube candidates are kilometres wide (Chappaz et al. 2017)                    |
| Radar sounding                        | Ice, layering, voids                                                                     | About 1 km for SHARAD (Seu et al. 2007b), over 3.7 km for MARSIS (Plaut et al. 2007) | 15 m vertical, 0.3–1 by 3–6 km across (SHARAD; Seu et al. 2007a)                                                | Showed Mars's debris aprons to be ice (Holt et al. 2008)                                  |

A deposit's signal is diluted by the ratio of its expression's area to the footprint: a porphyry's
altered rock, of median area 5.1 km² (John et al. 2010), fills about 10⁻³ of an 80 km gamma-ray
footprint, so a potassium anomaly that doubles the background over all of it moves the orbital count
by about a tenth of a per cent, and less where only the potassic zone is enriched (arithmetic here).
Orbit finds provinces and surface expressions, not ore bodies. **Lean:** each deposit carries, for
the sensors plan's instrument classes, its spectral class and halo extent, its K, Th and U
anomalies, hydrogen, susceptibility and density contrasts, volume and depth, and the sensors plan
computes detection from these and each instrument's footprint and noise. Orbit maps tracts, low
passes and drones find anomalies, and only sampling gives grade.

Markets and mining are out of scope; their hooks are the deposit's identity, tonnage, depth and
extent, and a homeworld's list for the [civilisations](#hooks-for-the-layers-above) to deplete.

## Geology as places

A landing party seeks places as well as ore, and each is one of two kinds. A **coarse-field list**,
like the craters, holds features large or rare enough to place once per world on the server, sent
with the surveyed field when it shapes height and otherwise only by survey. A **local synthesis
term** is a pattern too fine and numerous to list, made per query from the detail seed and
conditioned on the coarse field. The truth line decides which: whatever changes height or material
class is computed on both sides ([The line between truth and
decoration](rendering-and-planets.md#the-line-between-truth-and-decoration)).

| Landmark                        | Kind                                                                                                | Driven by                                                                                             | Where life favours it                        |
| ------------------------------- | --------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- | -------------------------------------------- |
| Lava tubes and skylights        | List per volcanic province; skylights as pits in the height function                                | Volcanism level, provinces, surface age, gravity                                                      | Caves as refugia                             |
| Hydrothermal fields and geysers | List, shared with the massive sulfides                                                              | Heat flow, boundaries and provinces, water                                                            | Chemolithotrophs; candidate origin sites     |
| Cryovolcanic plumes             | List of vents, output by orbital phase                                                              | Subsurface ocean, tidal heating, eccentricity                                                         | A sample of the ocean beneath                |
| Solar-driven jets               | A vent density on N₂ or CO₂ ice, active by month                                                    | Seasonal or perennial volatile ice, the sun's height over it                                          | None                                         |
| Salt flats                      | An evaporite form in R09's terminal basins, its salt a palette `Deposit`; the ore below on the list | Arid class, terminal outlets                                                                          | Halophiles; preserved organics               |
| Dune fields                     | Sand-sea cells in the coarse pass; a dune term in the synthesis                                     | Sediment supply, wind, air density, gravity                                                           | Little: hostile ground                       |
| Karst                           | A flag per cell; dolines by sparse convolution                                                      | Carbonate (`CaCO3`) under a humid class; evaporite under any wet one; organics under hydrocarbon rain | Made by life where its limestone is biogenic |
| Glaciers                        | Land ice of the coarse field, by substance; troughs in the synthesis                                | Ice areas by substance, elevation, temperature                                                        | Snow and ice communities                     |
| Central peaks and impact melt   | Fields of the coarse crater list                                                                    | Crater density, transition diameter, gravity                                                          | Impact-hydrothermal habitats                 |

**Lava tubes and skylights.** The Moon's Marius Hills hole is 65 m across and 80–88 m deep (Haruyama
et al. 2009), Mars's candidate skylights on Arsia Mons 100–225 m across (Cushing et al. 2007), and
tubes widen as gravity falls, with collapse widths of 20–30 m on Earth, 40–400 m on Mars and
500–900 m on the Moon (Sauro et al. 2020). A height field displaced along the radius can draw a
skylight's pit but never the void beside it. **Lean:** tubes are a server-side list per volcanic
province (centreline, width, roof thickness, age), widths fitted to those collapse widths, about
g^−1.8 across the three bodies (a fit here, for the effusion rates and roof strengths Sauro et al.
name as causes), capped by roof stability; a skylight is a pit in the height function where a roof
has failed, on the Moon where an impact or a wrinkle ridge triggered it, its roofs being stable
(Sauro et al. 2020), sent with the field like a coarse crater; and the interior is a hook, not
terrain. Caves are shielded refugia (Wynne et al. 2022), a habitat [Habitats beyond the
surface](#habitats-beyond-the-surface) does not yet list ([open question 15](#open-questions)).

**Hydrothermal fields, hot springs and geysers** need heat near the surface and water, so they lie
at boundaries and provinces where the climate is wet or the sea covers the vent, more densely as
heat flow rises. Geysers, fewer than 1,000 and nearly half at Yellowstone (Keller, Colman and Boyd
2023), also need silica and fractures sealed by low-permeability rock (Hurwitz and Manga 2017).
**Lean:** hydrothermal fields share one list with the massive sulfides, over their tracts and the
subaerial arcs and provinces, and geysers are its members on silicic crust, erupting on a monotone
phase, the galaxy brainstorm's construction for accumulation and release ([Events in
time](galaxy-generation.md#events-in-time)), on the event tag `body.geyser`, each eruption's length
a function of the interval after it, so that a long eruption is followed by a long wait, as Old
Faithful's predictions use (not re-checked). **Lean:** each geyser has its own series, keyed by the
body and the geyser's draw key, through a body-item event subject (`EventSubject` gains an item on a
body, since it names only a system or a body), a request to plan 01's identifiers and plan 06's
event key; until then a body has one eruptive clock, which all its geysers share. Vents are
candidate cradles of life (Martin et al. 2008), for [Energy: chemistry and
radiolysis](#energy-chemistry-and-radiolysis) and [What "appropriate conditions"
means](#what-appropriate-conditions-means).

**Cryovolcanic plumes.** Enceladus vents about 200 ± 30 kg/s of water vapour (Hansen et al. 2011),
with silica grains that imply reactions above 90 °C (Hsu et al. 2015) and hydrogen out of
equilibrium (Waite et al. 2017), and its plume is several times brighter near apocentre than
pericentre, as the fissures open under tension (Hedman et al. 2013). **Lean:** plume vents are a
coarse-field list on icy bodies with plan 14's subsurface-ocean flag and tidal heating, along
fractures where tidal stress peaks, their output a pure function of orbital phase: the vapour nearly
steady, since its flux varies by about 15% (Hansen et al. 2011), and the grains brighter near
apocentre. A plume lifts the ocean to orbit, the cheapest sample of a subsurface habitat there is,
for [Biosignatures](#biosignatures) to use ([open question 70](#open-questions)). The vents on the
floor beneath, which no coarse field maps, are open ([open question 59](#open-questions)).

**Solar-driven jets** are a second kind, sunlight trapped under a translucent volatile ice. Triton's
plumes rose about 8 km and trailed clouds over 100 km downwind, each eruption perhaps lasting a year
or more, driven in one model by a subsurface greenhouse in nitrogen ice (Soderblom et al. 1990), and
Mars's CO₂ jets vent from beneath translucent slab ice in its seasonal south polar cap (Kieffer,
Christensen and Titus 2006). **Lean:** a vent density on P14.T51.a's N₂ or CO₂ ice, active in the
months R09's sun stands high over it, needing no ocean and no tide, a local synthesis term since no
vent is large enough to list.

**Salt flats** cost nothing new: R10 gives the arid basins R09 filled an evaporite form, and R09's
palette names their salt as a `Deposit` entry, as it names Ceres's `Na2CO3`. They are evaporite
deposits and often brines, with lithium of 80–1,150 ppm in the brines of Uyuni and two nearby salars
(Ericksen, Vine and Ballón 1977), and Occator's faculae on Ceres, mostly sodium carbonate, are the
airless kind (De Sanctis et al. 2016). Halite keeps halophiles, some of them alive, and organic
matter in its fluid inclusions (Lowenstein, Schubert and Timofeeff 2011; Schreder-Gomes, Benison and
Bernau 2022).

**Dune fields** need sand, winds above the saltation threshold and converging transport. The
threshold friction velocity grows as the square root of gravity, grain size and grain density over
air density, putting Mars's an order of magnitude above Earth's (Kok et al. 2012), while Titan's
grains build dunes about 100 m high in winds near 0.5 m/s (Lorenz et al. 2006), covering 17.5 ± 1.5%
of the moon (Rodriguez et al. 2014). Giant dunes stop at a spacing set by the boundary layer, 300 m
to 3.5 km on Earth (Andreotti et al. 2009): terrain, not ripples. **Lean:** the coarse pass marks
sand seas where eroded, volcanic or photochemical grains meet converging winds above threshold, the
mark a member of the field since dunes change height (request 36), and the local synthesis adds a
dune term there, its form from the variability of R09's monthly winds and its spacing from the
boundary layer: an addition owed to R09's per-query evaluation.

**Karst** needs rock soluble in a world's surface liquid, and that liquid running through it:
carbonate needs acid as well, from the air's CO₂ or, on a living world, a soil's respiration, while
gypsum and salt dissolve directly. Carbonate underlies 15.2% of Earth's ice-free land (Goldscheider
et al. 2020), and life makes almost all the modern ocean's carbonate, but Archaean seawater made a
surplus abiotically (Grotzinger and Kasting 1993), so limestone records a biosphere only in its
fabric, skeletal grains and stromatolites, not by existing: carbonate karst is open to lifeless wet
worlds, and gypsum or salt karst to arid ones, as in Iran's salt diapirs (Bruthans et al. 2017).
Titan's polar depressions may be the cold case, organic sediment dissolved by liquid methane some 30
times more slowly than Earth's karst for want of year-round rain (Cornet et al. 2015). **Lean:** a
karst-prone flag, never gated on life, per coarse cell where carbonate (the sedimentary cover
request 36 asks for, from P14.T50.a's stored carbonate) lies under a humid class, evaporite under
any class that has rain or groundwater, or organic sediment under a hydrocarbon rain, the flag a
member of the field since dolines change height, dolines as a sparse-convolution term like small
craters, and caves as hooks.

**Glaciers** are R09's ice on land where it flows, of whichever substance P14.T51.a names. Pluto's
Sputnik Planitia is nitrogen ice in active flow with a crater retention age of 10 Myr or less
(McKinnon et al. 2016; Moore et al. 2016), and Mars's debris aprons are massive water ice under rock
(Holt et al. 2008). **Lean:** glaciers are coarse land-ice cells, the local synthesis carves troughs
where channels run under ice, and debris-covered ice is ground ice, a volatile deposit above; its
communities are in [Microbial biomes, the common case](#microbial-biomes-the-common-case).

**Central peaks and impact melt.** Peaks are natural drill cores: craters of 40–180 km lifted rock
from 5–30 km deep into theirs (Tompkins and Pieters 1999). Melt sheets, Sudbury's among them, and
hydrothermal systems follow in complex craters above 2–4 km on Earth and 5–10 km on Mars, their
altered rock feeding life long after (Osinski et al. 2013): about 67,000 years in a 30 km Martian
crater, 380,000 in a 180 km one (Abramov and Kring 2005), and over a million years, from an initial
300–400 °C, at Chicxulub (Kring et al. 2020). **Lean:** each coarse complex crater carries its
peak's sampling depth, melt volume and a hydrothermal cooling curve as closed forms of diameter and
gravity, and on a wet body a hydrothermal-list entry, so an old crater may have sheltered life.

## Knowledge, surveys and the landing party

### The ladder of knowledge, extended to life

The single-player brainstorm's ladder runs from point source, to orbit determined, to bulk
properties, to surface and atmosphere, to mapped from orbit
([Sensors and exploration](single-player-experience.md#sensors-and-exploration)). Content needs
three rungs more, matching R09's survey sources (orbital, close range, landed), then analysis.

| Rung                      | Method                                                                                    | Life and content it reveals                                                                       | Record                                                     |
| ------------------------- | ----------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- | ---------------------------------------------------------- |
| Point source, orbit, bulk | Detection, astrometry, photometry, mass and radius                                        | Nothing of life; the habitability context                                                         | Plan 14's `Contact` to `Bulk`                              |
| Surface and atmosphere    | Disc-integrated spectroscopy by transit, reflected light or emission, remote or in system | Gases, haze, edges, cycles: claims to level 4 or 5, a stage "consistent with"                     | The observation log; `Surface` once measured in the system |
| Mapped from orbit         | Imaging, imaging spectroscopy, thermal, radar, gamma-ray and neutron spectroscopy         | Biomes' surface expression per cell; gas sources; organisms some metres across; deposit provinces | R09's coverage; the observation log                        |
| Surveyed close            | Low passes, drones, the descent                                                           | Organisms by sight, vegetation structure, herds; anomalies                                        | Survey passes; sightings                                   |
| Landed                    | A landing party's instruments and sampling                                                | Microbes, mats, endoliths, site chemistry, grade at an outcrop                                    | Samples                                                    |
| Analysed                  | The ship's laboratory                                                                     | Biochemistry, chirality, isotopes, phylogeny                                                      | Analyses                                                   |

The stages show unevenly, which is the realistic answer. An origin shows only through metabolic
gases, and a biosphere of low productivity may show nothing, a false negative to produce honestly.
Oxygenic photosynthesis before oxygenation may show only as pigment from orbit, oxygenation as O₂
and O₃, and land life as edges and seasonal greening; complex cells and multicellularity have no
remote signature and are found by looking. Deposits follow the same ladder applied to rock
([Resources and deposits](#resources-and-deposits)).

### Detail levels for life, and how they degrade

Plan 14's records degrade by withholding sections of a true record (D16). Life cannot, because a
granted section states the truth, and the truth most worlds would state, that they have no
biosphere, is the one thing a crew can never measure. **Lean:** life follows plan 12's pattern,
where the only way out of the store is a view of what was measured (P12.T7), with levels ordered by
the highest rung reached, `Catalogued` and `Sampled` reached together where taxa are seen only in
samples:

- `Unexamined`: nothing.
- `Signatures`: the observation log's disc-integrated measurements, each a value with its
  uncertainty or a limit, never a truth field.
- `Mapped`: per surveyed coarse cell, the surface expression of its biome, computed by the client
  from the surveyed cells and the `LifeTable` exactly as the server computes it, because R11's
  instancing and the material class read it and the local synthesis agrees only if its inputs do; no
  byte of life rides in the field ([The per-cell state, the material and the
  instances](#the-per-cell-state-the-material-and-the-instances)). Cover and productivity are quoted
  with the survey's uncertainty, and biota with no surface expression, in an ocean or the crust,
  appear in no cell's state.
- `Catalogued`: taxa observed, as catalogue entries under opaque identifiers as plan 12's
  `ContactId` is, so that a designation cannot reveal the tree before analysis places it.
- `Sampled`: analyses of samples, below.

The biosphere's existence, stage and phylogeny are never sent; the console states them as
inferences, and names a surveyed cell's class by what was observed (`PIGMENTED CRUST, EDGE 0.70 µm`)
until the claim supports a biological name ([open question 71](#open-questions)): the rule is this
section's, and [Biomes as places](#biomes-as-places) applies it to its classes.

Plan 14's wire tests this. P14.T35.e (drafted) mirrors the whole surface section to the client:
`gases`, every species by registry key with its true mole fraction, and T24.c's aerosols and
absorbers, which R08 turns into the drawn sky, and T53.d's `biosphere` member with its fluxes.
T53.d's `biological` flag on the palette's `Cover` role would also reach the client, in R09's field
header. **Lean:** the rendering brainstorm's rule applies, that "Knowledge gates coverage for the
image, and coverage and resolution for every number" ([Knowledge, and the surface
seed](rendering-and-planets.md#knowledge-and-the-surface-seed)). The gases R08 draws reach the
client exact, because the sky is drawn from them, but only once the `Surface` level is granted,
which should follow the first measurement in the system, never a remote rung. They reach it only as
far as R08 draws them, which takes the main gases and the absorbers it has layers for: a trace gas
it does not draw, where most biosignature gases lie, stays on the server, one remainder entry
keeping the list's sum at 1 ([request 32](#requests-to-the-earlier-layers)). Every readout of
composition quotes the observation log, never `gases`. The `biosphere` member and the `biological`
flag never leave the server: nothing is drawn from them that the gases and the `LifeTable` do not
already give, and they are the one field that would state a biosphere outright. One channel of the
same kind remains, closed only for an honest client, as R09's survey stand-in is: R10's albedo
scale in block 0 is solved against the abiotic twin's geometric albedo and carries nothing of life,
but a client holding every cell's class could solve it again against the living disc's, which
R07's uniform disc reads, and compare the two ([The per-cell state, the material and the
instances](#the-per-cell-state-the-material-and-the-instances)).

### Survey passes and coverage

R09 records a survey as a pass (body, time span, source, resolution in metres, cells covered), one
versioned JSON line in `knowledge/surveys.v1.jsonl`, folded on load into a resolution byte per cell.
**Lean:** the same log serves content, each pass also recording the instrument classes it ran
(imaging, imaging spectroscopy by band, thermal, radar, gamma-ray and neutron) as the log's format
2, `surveys.v2.jsonl`, since a format-1 loader would drop the member silently, and every format-1
pass read as giving no imaging coverage. The fold gives a coverage byte per cell per layer, so that
"mapped in visible light at 30 m, never in the thermal infrared" is a state `SURVEY` can show.
Coverage stays the only gate on which cells a client holds, and imaging coverage gates life: the
`LifeTable` arrives with the first block holding a surveyed living cell under imaging coverage,
never with a margin cell or one seen only by radar, and life is drawn only where imaging coverage
exists. That last is a rule the client keeps, not a gate on data: once the table has arrived,
`life_at` would compute any cell the client holds, margin and radar-only cells included. How finely
the table is gated is [open question 78](#open-questions).

### Samples and on-board analysis

A landing party collects air in sealed bottles, regolith and sediment cores, filtered water, ice
cores, rock, swabs and specimens. [Organisms](#organisms) keys a sample to its site cell and the
samples already taken there and gives its taxa and cell density on `clade.sample`, the site's
community at instance 0 and each scoop at its sample number plus one; its abiotic contents, and the
crew's microbes after a breach, come from `surface.coarse.sample` ([Tags and scopes,
gathered](#tags-and-scopes-gathered)). This section owns what the analyses establish about the
claim, and under the owner's prior they are how most life is confirmed.

| Analysis                    | What it establishes                                                                                                          | Its trap                                                                                                                                                         |
| --------------------------- | ---------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Microscopy                  | Cells, size, division, motility                                                                                              | Inorganic silica-carbonate filaments "identical to currently accepted biogenic materials" (García-Ruiz et al. 2003)                                              |
| Mass spectrometry           | The organic inventory; life's selective use of few building blocks (McKay 2004); molecular complexity (Marshall et al. 2021) | Meteoritic and hydrothermal organics; the crew's own; abiotic mineral heteropolyanions with assembly indices up to 21, above the proposed 15 (Hazen et al. 2024) |
| Chirality                   | Homochirality, and which hand                                                                                                | Abiotic excesses: L-isovaline at 18.5 ± 2.6% in Murchison (Glavin and Dworkin 2009); dead material racemises                                                     |
| Stable isotopes             | Metabolic fractionation of C, S, H and N                                                                                     | Abiotic synthesis fractionates too (McCollom and Seewald 2006)                                                                                                   |
| Metabolic assays            | Uptake, gas exchange, growth                                                                                                 | Viking's reactive soil, explained by radiolysed perchlorate (Quinn et al. 2013)                                                                                  |
| Polymer capture and reading | A genetic polymer: repeating backbone charge, small alphabet, homochirality (Benner 2017); relatedness                       | What can be read is [What life is made of](#what-life-is-made-of-and-what-it-lives-on)'s question                                                                |

The last row is the sequencing analogue for alien life. A Darwinian genetic polymer "must be a
'polyelectrolyte'", so it concentrates on a charged support whatever its chemistry (Benner 2017):
finding it needs no alphabet, only reading it does. Reading gives the phylogeny
[Organisms](#organisms) generates, and comparing two worlds' polymers tests the shared descent that
panspermia ([Whether life arises](#whether-life-arises-and-how-far-it-gets)) implies. **Lean:**
each sample carries its collection record (sterile tools, blanks, witness material, containment)
and each analysis a result with its uncertainty; a broken procedure marks the sample "contamination
not excluded", capping its evidence at level 1. COSPAR wants an organic inventory of hardware "to
avoid any misinterpretation of terrestrial organics as indigenous biosignatures" (COSPAR 2025), so
the ship's own organics are a reference library against which the assays recognise them, and the
blanks measure how much reached the sample.

### Conditions for a landing party

A site is judged against human physiology, the crew being human, from the ship's measurements, never
the truth. Limits are partial pressures, to which the body responds; occupational limits in ppm at 1
atm convert at 101.325 kPa. **Lean:** the limits live in one type, `CrewTolerance`, holding human
values that suits, acclimatisation and the individual shift: over 7,000 people live permanently at
up to 5,100 m, and individuals have lived for two years at 5,950 m (West 2002), heights at which
aviation rules make an unacclimatised crew breathe oxygen (14 CFR 91.211).

| Hazard             | Human limit and its source                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | Read from                                          |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------- |
| Total pressure     | Below about 18.8 kPa even oxygen by mask cannot hold inspired PO₂ at 11.9 kPa: demand masks are certified to 12,200 m, where the air is 18.8 kPa, with 95% O₂ (14 CFR 25.1443(b)), so a pressure suit is needed. NASA's habitat floor of 16.9 kPa would need about 23 kPa of pure oxygen (NASA-STD-3001 Vol. 2 Rev. E, Table 6.2-1; normoxia on pure O₂ needs 26.2 kPa, [V2 6006]). Below the Armstrong limit, 6.28 kPa (water's vapour pressure at 37 °C, about 19 km on Earth), exposed body fluids boil. NASA designs its habitats for 34.5–103 kPa ([V2 6006]), a design range rather than a human limit: Titan's 146.7 kPa and Earth's lowest shores lie above it | Surface pressure, elevation                        |
| O₂, too little     | Inspired PO₂, (P − 6.3 kPa) × x(O₂), is 19.9 kPa at sea level. NASA's floor is 16.9 kPa indefinitely with monitoring and 16.3 kPa absolute (Table 6.2-1). Aviation requires crew oxygen beyond 30 min above 3,810 m (ambient PO₂ 13.2 kPa) and at all times above 4,270 m (12.5 kPa) (14 CFR 91.211). Acclimatised people live at 10.3 kPa inspired at La Rinconada (West 2002), and 5.7 kPa on Everest is "very near the limit for human survival" (West 1999)                                                                                                                                                                                                        | Measured O₂ and pressure                           |
| O₂, too much       | Inspired PO₂ up to 47.5 kPa indefinitely and 105.5 kPa for 6–9 h (Table 6.2-1). At 132 kPa the 1991 single-exposure limit for nervous-system toxicity is 180 min, which Hoyt et al. (2025) relax to 240 min working plus 240 min resting for divers. Oxygen-rich air is a fire hazard before it is a breathing one: "Oxygen-rich atmospheres, however, present safety hazards because of their ability to feed fires", which is why NASA's habitats keep an inert diluent ([V2 6006], rationale)                                                                                                                                                                       | As above                                           |
| CO₂                | 0.51 kPa as a 10 h average (NIOSH's REL; OSHA's 8 h PEL is the same), 3.0 kPa for 15 min, 4.05 kPa immediately dangerous (NIOSH: 5,000, 30,000 and 40,000 ppm); NASA holds habitats to 0.40 kPa over an hour ([V2 6004])                                                                                                                                                                                                                                                                                                                                                                                                                                               | Measured CO₂                                       |
| Toxic gases        | Working limits (NIOSH RELs): CO 3.5 Pa as a 10 h average, ceiling 20 Pa; NH₃ 2.5 Pa, 3.5 Pa for 15 min; H₂S ceiling 1.0 Pa (10 min); SO₂ 0.20 Pa, 0.51 Pa for 15 min. Escape only (IDLH, on a 30-minute basis): CO 122 Pa, NH₃ 30 Pa, H₂S and SO₂ 10 Pa. For a stay beyond a working day, NASA's limits for continuous exposure (JSC 20584 Rev. C, which NASA-STD-3001 Vol. 2 Rev. E [V2 6050] calls): CO 10 Pa for 24 h, 5.6 Pa for 7 d and 1.5 Pa from 30 d; NH₃ 2.0 Pa for 24 h and 0.30 Pa from 7 d; H₂S 0.13 Pa from 24 h to 30 d and 0.030 Pa at 180 d. The 1 h and 24 h values are for off-nominal exposure only                                                | Trace gases, as values or limits                   |
| Heat and cold      | Wet-bulb 35 °C in theory (Sherwood and Huber 2010); 30.6 ± 1.0 °C measured in young adults in humid heat (Vecellio et al. 2022). Tissue is injured once skin itself reaches 49 °C or 0 °C, whatever it touches (NASA-STD-3001 Vol. 2 Rev. E, [V2 9102] and Table 9.3-4); NASA's cabin range is 18–27 °C ([V2 6012])                                                                                                                                                                                                                                                                                                                                                    | Surface temperature, humidity                      |
| Ultraviolet        | Skin and eyes: 30 J/m² of S(λ)-weighted 180–400 nm exposure in 8 h, and 10 kJ/m² of unweighted UVA to the eyes, absolute for the eyes and advisory for skin, since noon summer sunlight at 0–40° latitude exceeds the first on bare skin within 5–10 min (ICNIRP 2004). For context, the DNA-weighted surface irradiance of an anoxic 3.9 Ga Earth is 611 times today's under the early Sun, 3,520 under an F0 star and 2.02 under GJ 581 (Rugheimer et al. 2015, Table 3)                                                                                                                                                                                             | Measured O₃ column; host spectrum                  |
| Ionising radiation | Career effective dose under 600 mSv, "universal for all ages and sexes" (NASA-STD-3001 Vol. 1 Rev. C, [V1 4030]); 30-day organ limits of 250 mGy-Eq to the blood-forming organs, 1,000 to the lens and 1,500 to skin (its Table 4.8-1)                                                                                                                                                                                                                                                                                                                                                                                                                                 | Column, magnetic field class (P14.T24.b), activity |
| Dust               | In a habitat, total dust below 3 mg/m³ and its respirable part below 1 mg/m³ ([V2 6052]), and lunar dust under 10 µm below 0.3 mg/m³ as a time-weighted average for up to six months ([V2 6053]); perchlorate at 0.4–0.6% by mass in Martian soil (Hecht et al. 2009); lunar dust's "intense respiratory and ocular irritation" in Apollo crews (Chang et al. 2024)                                                                                                                                                                                                                                                                                                    | Surface class; samples                             |

Doses take the units of [What the guide must gain](#what-the-guide-must-gain), item 5. **Lean:** a
site's `BREATHABILITY` is `SHIRTSLEEVE`, `MASK` or `PRESSURE SUIT`, a pure function of the measured
atmosphere and `CrewTolerance` that names each failing limit and its margin, and says so when an
uncertainty straddles a limit within two standard deviations rather than choosing:
`CO2 ~1.1 kPa ± 0.4 kPa · LIMIT 0.51 kPa (10 h)`. `SHIRTSLEEVE` needs inspired PO₂ of at least
11.9 kPa, the aviation rule's at 3,810 m, for an unacclimatised crew, so Earth's high cities pass
(Mexico City's air gives 14.9 kPa inspired, computed here in the standard atmosphere). NASA's
16.9 kPa, which air meets only below about 1,270 m, is the ship's own habitat standard. `MASK` is a
filtering mask where inspired O₂ meets that floor and every gas is below its IDLH and below its
working limit times the mask's protection factor (a factor of one for CO₂, which no filter removes),
and a supplied-gas mask otherwise, as oxygen-deficient air always needs (29 CFR 1910.134(d)); on
oxygen it holds the floor down to about 18.8 kPa total, below which a `PRESSURE SUIT` is needed.
Gases with the same effect are summed as fractions of their limits, which together may not exceed 1
(29 CFR 1910.1000(d)(2)(i)). An excursion within a day is judged on the RELs, a stay on the SMAC for
its length, so `CrewTolerance` takes the stay's duration. Temperature, ultraviolet, radiation and
fire are rated beside it, not inside it, ultraviolet as a time to its limit, so a Titan reads `MASK`
for its air and names its cold, and a move between the ship's pressure and a site's names the
decompression procedure `CrewTolerance` holds. None of it speaks of biology, which the protection
procedures govern. An upper bound on pressure, and gravity, wind and cold, are [open question
69](#open-questions)'s.

### Biohazard realism

The question is what an alien biochemistry can do to a human body. Toxicity, allergy and nutrition
are settled by [Chirality and alien biochemistry](#chirality-and-alien-biochemistry) and the defence
modes of [Hazards and use](#hazards-and-use): generic chemistry harms a human whatever made it,
receptor-specific toxins fit human ones only by chance, and alien tissue is not food, so the crew
eats from the ship's stores. Infection remains. Pathogenesis "requires specific adaptations to
overcome the extensive defenses possessed by all Earth organisms", so "virulent extraterrestrial
pathogens are unlikely", though "the risk is not zero" (National Research Council 1997). The
scientific community's recent caution about mirror-image life (Adamala et al. 2024) is a reminder
that an unfamiliar biochemistry's encounter with human defences cannot be assumed benign. So
infection is rare and severe, a colonised wound rather than an epidemic: a possibility the
procedures guard against, not a disease the game simulates.

**Lean:** the generated hazard follows these, with [Organisms](#organisms)' rare generated
infectivity for infection. A crew cannot know the biochemistry before analysis, so procedures run at
full strictness on any candidate biosphere and relax only on analysed results.

### Planetary protection as procedures

COSPAR's policy, approved on 7 November 2025, sorts missions by target and type, and maps onto a
crew's procedures with little change (COSPAR 2025):

| Category             | COSPAR                                                                                                                                                                                                                                                                                                                                                                                                        | On the ship                                                                             |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| I                    | Targets "not of direct interest" to chemical evolution or the origin of life                                                                                                                                                                                                                                                                                                                                  | No procedure                                                                            |
| II                   | Interest, but "a remote chance of contamination"                                                                                                                                                                                                                                                                                                                                                              | Log impacts and landing sites                                                           |
| III                  | Flyby or orbiter where the chance is "significant"                                                                                                                                                                                                                                                                                                                                                            | No uncontrolled descent; disposal orbits                                                |
| IV                   | Lander or probe there. For Mars: IVa's surface cleanliness (≤ 3 × 10⁵ spores, ≤ 300 per m²); ≤ 30 spores on the landed system or its sample path for life detection (IVb), or on what touches a Special Region, "a region within which terrestrial organisms are likely to replicate" (IVc). For Icy Worlds, a probability of contamination below 1 × 10⁻⁴ over a 1,000-year period of biological exploration | Suits, sterile tools, controls on sample paths; only sterile hardware touches a habitat |
| V, restricted return | Containment of samples and contacted hardware, and "absolute prohibition of destructive impact upon return"                                                                                                                                                                                                                                                                                                   | Samples contained; crew quarantine                                                      |

With a human crew its guidance for crewed Mars missions applies as written, and reads as checklist
items already: "safeguarding the Earth from potential back contamination is the highest planetary
protection priority", which for a ship means the crew, the home base and every world visited after;
"any uncharacterized Martian site should be evaluated by robotic precursors prior to crew access";
"a quarantine capability and protocols for the crew should be provided"; and "an onboard crewmember
should be given primary responsibility" (COSPAR 2025). **Lean:** the science station assigns a
category to each activity at each body (orbit, landing, return) from what the ship knows (habitats
measured, the claim's level), as COSPAR assigns V in addition to the outbound category, which arms
its procedures in the guide's style: checklists, `ARM` and `EXECUTE` on irreversible steps such as
opening containment, a drone before the crew at an uncharacterised site ([open question
68](#open-questions)), and a planetary protection officer as a named role. A forward-contamination
breach is a delta in the overlay that marks later samples from the site as contaminated, stalling
their claim at level 1, since human crews carry microbial populations that "vary in both kind and
quantity" (COSPAR 2025); back contamination is the quarantine of exposed crew, their monitoring for
the colonised wound of [Biohazard realism](#biohazard-realism), and its alerts. Whether a crew may
enter a Special Region is [open question 79](#open-questions).

### What is stored

Knowledge is play state in the universe's `knowledge/` directory, beside P12.T7's built
`contacts.v1.jsonl` and R09's `surveys.v1.jsonl`. **Lean:** three more versioned JSON-lines logs,
for observations (prospecting's measurements too), samples (rock assays too) and the catalogue (an
entry per taxon, biome, landmark or deposit); the claim's level, the life detail level, per-layer
coverage and the protection categories are folds of these logs, R09's and the contamination delta
([What play changes](#what-play-changes)), never stored. Their lines, helpers and keys are [The
Knowledge overlay](#the-knowledge-overlay)'s.

## Fit with the consoles

The guide was written for a ship that measures stars and orbits, and planet content asks three
things of it for which it has no grammar yet: maps of a body's surface, some of whose layers are
categories rather than quantities; claims that are categorical and uncertain at once, such as "this
world has a biosphere"; and a catalogue of what has been found. Each fits the guide's principles
without strain, since honest data is exactly what a survey produces, but each needs words, units
and rules the guide does not have. They are collected here as edits for the owner to sign off, as
[the rendering brainstorm's were](rendering-and-planets.md#what-the-guide-must-gain), and none is
made.

### The displays, and who owns them

A science station, its name not yet fixed, owns surveys, and the single seat holds it with every
other. Displays are the unit of reuse, so any station may open one; what stays with the owning
station is the authority to command what it shows, such as a survey pass or a sample's analysis,
under the guide's closed-loop rule. **Lean:** five new displays, their names provisional:

| Display     | What it shows                                                                                                                                                                                                                                                                                                                                  | Owner                               | Also read at                              |
| ----------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------- | ----------------------------------------- |
| `SURVEY`    | A body's map, as a globe and a flat projection: one raster layer at a time with at most one overlay, the survey's coverage and its edge, marks for sites, deposits, landmarks, samples and the sources of signatures, and a keyboard cursor that reads every layer at its cell. A selected site's landing conditions stand in its `SITE` panel | Science                             | Helm, for the site; the Captain           |
| `LIFE`      | Each line of evidence about the body's life, with its instrument, time, value or limit, and the inferences drawn from it (whether a biosphere exists, its stage, its habitats), each with its confidence rung and the false positives not yet excluded                                                                                         | Science                             | The Captain                               |
| `ANALYSIS`  | The samples held: site, time, mass and containment, and each analysis run or pending with its result and uncertainty                                                                                                                                                                                                                           | Science                             | The Captain, for containment              |
| `PROSPECT`  | Deposits found: kind, geological setting, tonnage and grade, confidence category, and how each was found                                                                                                                                                                                                                                       | Science, until mining has a station | Engineering, for propellant and volatiles |
| `CATALOGUE` | Every taxon, biome, landmark and deposit recorded, by body and by kind: designation, discovery time and position, the measured record, and any generated description                                                                                                                                                                           | Science                             | Every station                             |

What each shows belongs to its own section: the evidence and its ladder to
[Biosignatures](#biosignatures), the samples and the site's conditions to [Knowledge, surveys and
the landing party](#knowledge-surveys-and-the-landing-party), the deposits to [Resources and
deposits](#resources-and-deposits). The single-player brainstorm puts a chronological discovery log
on `SENSORS`. **Lean:** the catalogue is one Knowledge log ([The Knowledge
overlay](#the-knowledge-overlay)), which `CATALOGUE` shows by subject and `SENSORS` by time, beside
the contacts log's discoveries, a field guide and a diary of one record, so that the two displays
cannot disagree. Two commands need the guide's `ARM` and `EXECUTE`, because both are irreversible:
an analysis that consumes a sample, and opening a sample's containment.

### The body map

The guide's three-dimensional spatial displays are orthographic and its galaxy map is face-on; a
body's surface is a sphere, and every flat map of it distorts. Cartography's answer is to name the
projection and state where its scale is true. For coordinates, the IAU's working group keeps two
systems: planetocentric, whose longitude always increases eastward, and planetographic, whose
longitude increases westward on a body that turns prograde, and it sees no advantage in switching
where planetographic coordinates are established; Mars, Mercury and the Moon are now mapped in
east-positive longitude (Archinal et al. 2018). A generated world has nothing established. **Lean:**
planetocentric latitude and east-positive longitude on every body, with the body's north at the pole
the right-hand rule makes positive, as the working group defines it for dwarf planets and small
bodies, so that one rule serves a world that turns backwards; a map names it `BODY NORTH`, so that
it is never read as the galactic `NORTH`. Plan 14's `BodyFixedFrame` supplies the pole, by the
right-hand rule, and the prime meridian, as a `Bulk` section (P14.T46.b, built). A body without an
orbit, a rogue planet, has none yet, since the frame measures its meridian from the equator's node
on the orbital plane ([request 21](#requests-to-the-earlier-layers)). A globe is a map too, not a
three-dimensional spatial display: its marks lie on the surface, with no reference plane or stalks,
so it turns about the body's pole and tilts in latitude to show either pole, `BODY NORTH` up and
never rolled. What time and what knowledge its layers show is [open question 72](#open-questions).

Its layers are of three kinds, and the guide covers only the first:

- **Quantities**, as single-hue rasters under the existing rule: elevation, survey resolution,
  vegetation cover, canopy height, net primary production, biomass, a permissive tract's expected
  density of deposits where [Resources and deposits](#resources-and-deposits) assesses one, and the
  uncertainty of any of them, offered as a second quantity in place of the first.
- **Classes**: R09's climate class, the surface class and the biome
  ([Biomes as places](#biomes-as-places)). A single hue cannot tell more than five or six classes
  apart, so these need a rule of their own.
- **The image**: a true-colour mosaic from the ship's own imaging, which is photometry and not
  symbology, and so falls under the rendered image's exception rather than under any raster rule.

A deposit is a point, not a field, so its grade is not a raster: deposits are a mark layer, each
mark carrying its kind, grade, tonnage and confidence category in the list and the cursor's
reading. Marks for places, a site, a deposit, a landmark, a sample or a signature's source, need
outlines that the ship-wide symbol set does not use: a square that is a black hole on the chart and
a deposit on the map would break "one ship, one system".

### Colour for classes

A land-cover map is conventionally green, blue and amber, which here would spend nearly every
reserved colour as decoration. The six chromatic tokens sit at CIELAB lightness L\* 64–81 with
chroma 33–78, and between them leave almost no hue free; the standard palettes for colour-blind
readers fail for the same reason, six of Okabe and Ito's seven colours and six of Tol's nine muted
ones lying within ΔE₀₀ 22 of a reserved token (both computed for this section from the stylesheet's
values and the published hex values, with the CIEDE2000 difference of Sharma, Wu and Dalal 2005).
What is free is darkness and greyness. The guide's rule for graph series, tints of `--text` and
`--accent`, gives two hues, too few to keep eight fills apart in dichromacy, and an accent tint
would read as interactive, so class fills avoid `--accent` and `--target` as well as the status
colours. **Lean:** class tokens `--class-1` to `--class-8`, dark and muted (L\* 20–45, chroma at
most 30), each at least ΔE₀₀ 22 from every status, accent and target token and as far from each
other as can be had, all scored in normal vision and in each of the three dichromacies of Machado,
Oliveira and Fernandes's (2009) model. A set of eight meeting this has been found, with every pair
about ΔE₀₀ 9.5 or more apart in all four kinds of vision (the closest 9.48, under deuteranopia):
enough to tell adjacent regions apart, not to name a class by its colour, and that is the point. The
fill is never the only signal. Each region large enough carries its class code on a `--surface-0`
plate, since `--text` reaches only 3.8:1 on the lightest fill; boundaries are `1px` `--text-muted`
lines with the raster rule's `--surface-0` casing on each side, since uncased they reach only
2.0–4.7:1 on the fills, short of the guide's 6:1, and the selected class's outline is cased alike;
the legend names every class in words with its area; and the cursor reads the class at its cell. A
layer with more than eight classes shows groups, and the legend and the cursor name the class within
the group. The legend's codes and structural names come from a static `BiomeClass` table compiled
into the client, not from the field's header, and the client computes the categorical life layers
itself ([The per-cell state, the material and the
instances](#the-per-cell-state-the-material-and-the-instances)), so a token is indexed by class and
nothing of life is sent per cell. Until the claim supports a biological name, the legend and the
cursor name each class by what was observed instead, composed from the table's structure and the
measured edge (`PIGMENTED CRUST, EDGE 0.70 µm`; [Detail levels for
life](#detail-levels-for-life-and-how-they-degrade)).

### Confidence, estimates and limits

A content readout makes four kinds of statement, and the guide has a form for one of them:

- **A measured number** is Estimated, with its one-standard-deviation uncertainty in R10.T13's
  drafted form, still awaiting the owner, which is NIST's (SP 811 §7.7):
  `NPP ~650 g/(m²·yr) ± 200 g/(m²·yr)`. A reading taken from afar is also Observed, with its light
  age, as any other is.
- **A non-detection** is a limit, and a value in its own right: `CH4 < 0.5 µmol/mol`, with the
  limit's confidence stated once for its section. It is never `0`, never the em dash, which says
  that there is no value at all (none computed, or the ground `NOT SURVEYED`), and never `NONE`.
- **A categorical claim**, that a biosphere exists, what its stage is, that a deposit is there, is
  an inference, and takes a confidence rung after a middle dot instead of the `~`. For life the
  rungs are [Biosignatures'](#biosignatures) ladder, shown as `LEVEL 3/7` under a heading that names
  the scale in words, `CONFIDENCE OF LIFE DETECTION`, since upper case would turn CoLD into `COLD`.
  For deposits they are mineral exploration's own, the JORC Code's `EXPLORATION TARGET`,
  `INFERRED`, `INDICATED` and `MEASURED`, borrowed for their geological confidence and sampling
  alone, without the economic test the code sets every resource or its reserve categories: a
  deposit known only remotely is a target, its tonnage and grade quoted as ranges, and `INFERRED`
  needs samples.
- **An identification** short of certainty takes biology's open nomenclature: `CF.` where most
  characters match a known taxon but some are unclear, `AFF.` for a taxon akin to a known one but
  not identical to it, and `SP.` for one not identified to species (Sigovini, Keppel and
  Tagliapietra 2016).

**Lean:** the `~` stays a mark on numbers, and no confidence is shown by colour. One consequence
belongs in the guide. Its rule that "none" is a value, as an airless world's atmosphere is shown
with no air in it, cannot reach life: no measurement shows a world to be sterile, so `LIFE` never
says `NONE`, only `NO BIOSIGNATURE ABOVE LIMITS`, beside the limits that make it so ([A confidence
ladder](#a-confidence-ladder)). The guide already reasons this way about aerosols, whose
`NOT YET MODELLED` note "is never read as 'none'", because the air is not known to be clear of them.
Nor does a reading say how its universe was generated: a speculative setting such as
`exotic_solvents` or `giant_atmosphere_life` is recorded in `universe.json` and shown where the
universe is created, never on a console, since the crew measures its world as it is
([Solvents](#solvents)).

### What the guide must gain

Collected so that a plan can make the edits in one pass:

1. **Five displays and a panel**, `SURVEY` with its `SITE` panel, `LIFE`, `ANALYSIS`, `PROSPECT`
   and `CATALOGUE`, in the nomenclature list, their names the owner's to choose.
2. **Body maps, a class of display.** The projection always named (`ORTHOGRAPHIC`,
   `EQUIRECTANGULAR`, `POLAR STEREOGRAPHIC`); the 1-2-5 scale bar labelled where it is true
   (`SCALE TRUE AT 0°`); a graticule in `--line` at a stated interval; `BODY NORTH` up, never
   rolled, the polar projection with its central meridian up and stated (`0° UP`), and the globe
   tilting in latitude without roll; the frame `BODY-FIXED <designation>` (the word the owner's), a
   frame of its own, since the orbit map's `BODY <designation>` keeps galactic axes and does not
   turn while a map turns with its body (plan 14's `BodyFixedFrame`); and
   `PLANETOCENTRIC · EAST LONGITUDE` in the legend. A position is `LAT +12.35° LON 047.21°`, the
   latitude signed and zero-padded as an elevation is, the longitude zero-padded as a bearing is.
   The survey edge is drawn, an unsurveyed region never looks empty, and its readings are the em
   dash with R10.T13's `NOT SURVEYED`. Marks for places take outlines of their own, each named in
   words in the legend and the list.
3. **The class raster**, with the class tokens and the rules above, stated as the single-hue rule's
   first exception; the raster rule's floor, log scale and ban on invented values still bind every
   quantity layer.
4. **The image layer**, under the rendered image's exception, labelled with its source and
   resolution (`IMAGE: ORBITAL · 120 m`), never blended with a class fill, and with every mark over
   it cased.
5. **Units.** Net primary production, `NPP`, is carbon mass per area per time, in `g/(m²·yr)`. The
   carbon is named by the label and not by the unit, since SI mixes no information about a quantity
   into its unit (NIST SP 811 §7.5: "5 ng/L but not: 5 ng Pb/L"), and ecology's `g C m⁻² yr⁻¹` could
   not be set anyway: B612 has no superscript minus, and a second solidus needs parentheses
   (§6.1.6). A world's total is in `Pg/yr` (Earth's about 105 Pg of carbon a year; Field et al.
   1998), and biomass in `kg/m²` and `Pg` of carbon (Earth's about 550 Pg; Bar-On, Phillips and Milo
   2018). Organisms are counted per area, `/km²`, as the guide composes `/ly³`. A gas's abundance is
   a mole fraction, scaled `%`, `µmol/mol`, `nmol/mol`: atmospheric chemistry says ppm and ppb, and
   the WMO reports in them (WMO 2025), but NIST rules all three of ppm, ppb and ppt out for use with
   the SI (§7.10.3) and the SI Brochure advises against ppb and ppt as language-dependent (BIPM
   2019, §5.4.7), while µmol/mol is the unit the WMO's own scale is defined in (Hall et al. 2021).
   Partial pressure is in `Pa`, `kPa` and `MPa`, as plan 14's `formatPressure` has it. A solid's
   abundance is a mass fraction in `%`, `g/t` and `mg/t`, as mining reports grade. Since `%` heads
   both ladders, the label names which fraction a value is (`MOLE FRACTION`, `MASS FRACTION`, or an
   ore's `GRADE`), as NIST SP 811 §7.10.2 puts such information in the quantity and never in the
   unit, and `%` takes a space as any unit does (`0.6 %`). Salinity is in `g/kg`; tonnage in `t` to
   `Gt`, which SI usage allows (NIST SP 811 §6.2.8). With a human crew, doses take NASA's units:
   effective dose in sieverts (`mSv`, and `µSv/h` as a rate), against which the career limit of
   600 mSv is set, and an acute dose to an organ in gray-equivalents (`mGy-Eq`), in which the 30-day
   and one-year organ limits are set (NASA-STD-3001 Vol. 1 Rev. C, 2023). Absorbed dose to
   instruments, samples and organisms that are not the crew stays in grays (`mGy`, `µGy/h`), because
   the sievert's weighting is defined for human tissue alone.
6. **Chemical and isotopic names.** B612 has none of the subscript digits, as the console-ux
   skill's glyph script shows, so document prose writes CH₄, as plan 14's prose does, and console
   strings write `CH4`, the substance registry's ASCII formula key (`SubstanceKey`), reached through
   `Gas::substance()`'s row rather than `Gas::formula()`, which returns `CH₄`; a material keyed by
   name (`basalt`) is shown by its name in words. A formula's counts are not a symbol's subscript,
   which the guide writes in parentheses (`A(V)`). On a console the gas's name in words is the
   accessible name. B612's superscripts stop at `³`, so `¹⁸O` and `³⁴S` cannot be set, and isotopes
   are written `He-3`, `C-13` and `O-18` alike, every chemical symbol keeping its case in an
   upper-case label as a unit symbol does, since `CO` and `Co` are different substances. An isotope
   ratio is `δ(C-13)`, as the guide writes a symbol's subscript; its `δ` and `‰` are not in B612
   either: they are drawn as inline SVG glyphs with the accessible names "delta" and "per mil", as
   `☉` is. Nor are `≤` and `≥`, so limits are `<` and `>`.
7. **The four kinds of uncertain statement** above, with `NO BIOSIGNATURE ABOVE LIMITS`, listed as a
   four-word status as `NO SYSTEMS IN MAP` is, the limit's form and the rule that `LIFE` never says
   `NONE`. A body's readout gains sections for life, deposits and landmarks, each `NOT YET MODELLED`
   until a generator version computes it, composed as the guide's `ATMOSPHERE: NOT YET MODELLED` is
   (`LIFE: NOT YET MODELLED`). Past that, none is withheld, since its detail is a view of what was
   measured ([Detail levels for life](#detail-levels-for-life-and-how-they-degrade)). Before the
   first measurement each says so (`LIFE: NOT EXAMINED`, a new phrase), and none ever shows
   `NOT RESOLVED`, which would say that a true section is held back.
8. **Nomenclature.** The stages as [The transitions, and whether each is
   hard](#the-transitions-and-whether-each-is-hard) names them, `Origin` needing another console
   word, since the guide's `ORIGIN` label means where and how an object formed; the habitats
   (`SURFACE OCEAN`, `SUBSURFACE OCEAN`, `CRUST`, `CLOUD DECK` and the rest); biome and climate
   names, whose codes keep their case under the guide's rule for classification symbols, as Köppen's
   `Af` and `BWh` do, and the observational names that stand for them until the claim supports a
   biological name; the confidence rungs (`LEVEL 3/7`, under `CONFIDENCE OF LIFE DETECTION`), the
   JORC categories and the open-nomenclature qualifiers; deposit kinds; landmark kinds in plain
   English (`GEYSER FIELD`, `LAVA TUBE`, `JET FIELD` for solar-driven jets; [Geology as
   places](#geology-as-places)), with generated place names taking an IAU descriptor term as their
   generic part (`<name> MONS`, `<name> VALLIS`), as the IAU's gazetteer does; the map's
   `BODY NORTH` and `BODY-FIXED <designation>`; and `NOT ANALYSED`, for a sample's analysis not yet
   run, and `NOT EXAMINED`, for a section before its first measurement, beside `NOT SURVEYED`. Three
   names collide with the guide's list. `SITE` is already a belt's label, so the panel takes another
   word, such as `LANDING SITE`, or the belt's label does. Once `LIFE` exists the chart's `LIVING`
   filter would read as systems with life, and wants another word, the owner's. And `CLASS` is
   already a star's, a planet's and a ring's, so the cursor names a cell's classes `BIOME`,
   `CLIMATE` and `SURFACE`, never `CLASS`.
9. **Names of taxa.** A binomial is italic by convention, which the guide forbids. A taxon's
   identity is its designation from its ID, as a body's is; a generated binomial is a name, in upper
   case on a label as `GC 47 TUC` is, and in mixed case without italics in running text
   ([Organisms](#organisms)).
10. **Generated text is labelled.** Enrichment is mixed-case running text, as the guide already has
    it, under a heading carrying `GENERATED`, because it is not a measurement. Keyed to what the
    ship has established ([Stored enrichment](#stored-enrichment)), it is superseded when that
    changes; where it and a readout still disagree, the readout is the ship's.
11. **Accessibility.** The map's canvas is paired with a DOM list of its marks, as every spatial
    display's is, and the cursor moves from the keyboard and reads, in an `output` element, every
    layer at its cell, as the galaxy map's `CURSOR` panel does. The class legend is a list from
    which a class is selected, and the selected class is outlined, cased as any line over a raster
    is, rather than recoloured, so that selection is a shape. Nothing on the map is told by fill
    alone, and the class tokens are scored by the contrast script on every surface and by a
    dichromacy check that the console-ux skill gains.
12. **Procedures.** A site's `BREATHABILITY` with its ratings and each failing limit's margin
    ([Conditions for a landing party](#conditions-for-a-landing-party)); a body's planetary
    protection category for each activity there, orbit, landing or return, and its checklists
    ([Planetary protection as procedures](#planetary-protection-as-procedures)); and the two
    irreversible commands, an analysis that consumes a sample and opening a sample's containment,
    entered under the guide's existing `ARM` and `EXECUTE` rule.

## Hooks for the layers above

History wants to be generated top-down over a galaxy only ever generated lazily
([Overlays and persistence](galaxy-generation.md#overlays-and-persistence)). The species and
civilisations brainstorm needs from this layer facts per body, densities per large cell, a rule for
homeworlds, the ladder's next rung with a number, and what a human crew implies.

### Per body

From the record at any t: the stage and when each was reached, so complex life's age is t −
T(`ComplexMulticellularity`) and land life's t − T(`Land`); the habitats complex life holds, ocean
and land, and others where [Habitats beyond the surface](#habitats-beyond-the-surface) populates
them; the remaining lifespan, to brightening, deoxygenation or the host's death, which bounds any
species there; the lineage seed; dated extinction pulses; fossil epochs. Beside them, plan 14's land
fraction and its O₂ partial pressure once P14.T53.d is fed, since sustained open-air combustion, the
kind industry needs, requires at least 87.8% PAL of O₂ (Balbi and Frank 2024, as Mills et al. 2025
report), above the 15–17% O₂ at which wildfire leaves charcoal ([Land](#land)).

### Per large cell

D17 makes habitable worlds per large cell a quadrature over stellar mass and metallicity. Life adds
age, because it depends on how long a world has been habitable. **Lean:** the density of worlds at
stage k or beyond at clock time t is

```text
n_k(x, t) = Σ_p ρ_p(x) ∫∫∫ φ_p(m) ζ_p(Z) a_p(age) η_T(m, Z, age + t) F_k(m, Z, age + t) dm dZ d(age)
```

over populations p with densities ρ_p, mass functions φ_p, metallicity and age distributions ζ_p and
a_p, η_T being plan 14's expected `Temperate` bodies per such host and F_k the chance that one has
reached stage k, with a twin for `SubsurfaceOcean`. F_k is tabulated offline by running this model
over a grid, say 32 masses, 8 metallicities, 64 ages and 7 stages, some 10⁵ entries and under a
megabyte, versioned with the generator; the recovery gate enters as a factor of position. Because
the argument is age + t, the table serves any past epoch, unborn systems dropping out, so history
can count worlds with complex life per cell at −2 Gyr, −1 Gyr and now (the systems now in each cell,
since none is followed back in position), and emit pinned content. "Large cell" is any grid history
chooses, the integrand being a density, and the table must match lazily generated bodies in
expectation, a statistical test.

### A pinned homeworld and the biosphere lazy generation gives it

- **Selection.** Having drawn from n_k a species arising in cell c near epoch t_e, history walks the
  cell's systems in a fixed order, generates each, and takes the first body not already selected, in
  the order the species were drawn, whose record has complex life (on land with O₂ above the fire
  gate, for a species that needs fire) by t_e and no setback since; the time of intelligence is
  drawn conditional on that record. Nothing is overridden, so nothing can disagree. It costs the
  reciprocal of the qualifying share per system: about 200 systems at the default, some 0.2 s at
  plan 14's target of 1 ms a system (1,098 µs measured, provisionally and under load), and some
  3 × 10⁴, under a minute once at campaign set-up, under strict hard steps.
- **Override.** Pinned content replaces the record, under a rule: the whole record, never single
  fields; satisfying every gate the body's plan 14 history gives, so no forest without surface
  water; and applied before any consumer reads it, so biomes, atmosphere and signatures follow.

**Lean:** selection for every generated homeworld, override only for hand-authored worlds such as a
starting system. The galaxy brainstorm's overlays carry both.

### The next rung, and the tension with a game that wants civilisations

The owner's ladder continues to crude intelligence. Increased intelligence "arose many times in
vastly distantly related organisms", while the origin of life, and possibly oxygenesis and
technological intelligence, are the steps Bains and Schulze-Makuch (2016) cannot class as likely;
Kipping (2020) gives 3:2 odds that intelligence is rare, and all eight hard-step studies tabulated
by Mills et al. (2025) name our own species. So the rung splits. **Lean, provisional for the next
brainstorm:** crude intelligence at 2 per Gyr once complex multicellularity exists, reaching 86% of
macroscopic biospheres; technological intelligence at λ_T = 0.01 per Gyr, the geometric middle of
10⁻⁴–1, once crude intelligence and land life exist and O₂ allows open fire. If every land-life
world cleared that gate, it would put a technological species on about 0.2% of `Temperate` bodies,
one land-life world in fifty: some 600 within 1,000 ly over the Galaxy's history, and of order 10⁷
in the Galaxy at the same placeholder ratio of `Temperate` bodies to systems. Worlds whose O₂ stays
below the gate lower both. Strict hard steps with λ_T = 10⁻³ give about 0.3 within 1,000 ly and
5,000 in the Galaxy (computed). How many coexist is that number times a civilisation's lifetime over
the span in which they arose, the next brainstorm's question.

That is the tension: a realistic reading of the hard-steps argument leaves no technological
neighbour within reach, and the game wants factions and languages. The next brainstorm can resolve
it without bending this layer: accept rarity and make an empty galaxy the setting; let a few origins
expand, the realistic way to fill a region with factions from few species; set λ_T at the easy end,
which Mills et al. (2025) make defensible, though Bains and Schulze-Makuch (2016) leave
technological intelligence among the steps they cannot class as likely; or condition the campaign on
civilisations near its start, a selection like the anthropic argument itself. Each leaves this
section's record and densities unchanged; only λ_T and history's choices move.

### The crew is human

The owner has ruled that the crew is human (2026-10-10). The galaxy has no Sol, so where humans come
from is the civilisations brainstorm's question, but two things follow here. A human homeworld, if
there is one, is pinned content whose biosphere this layer could have generated: a world past
`Land`, with land life long enough for technological intelligence and O₂ enough for fire and for
breathing, since long-term human habitation needs about 53–59% PAL (Mills et al. 2025). By the lean
above it is found by selection, with those limits and plan 14's gravity and temperature added to the
search. At the default one system in 200 has land life, so even a narrow Earth-like filter is a
search of thousands of systems, not an override; under strict hard steps, with one in about 3 × 10⁴,
it is a search of 10⁵–10⁶ systems, minutes at set-up (computed). And humans, one observed instance
of the intelligence rung, should not shift its rarity: an observer always finds itself, so its own
existence says almost nothing about λ_T. Correcting for that selection, as Spiegel and Turner (2012)
did for the origin, Earth's record yields only 3:2 odds for rare intelligence (Kipping 2020).
**Lean:** if there is a human homeworld, it is the one instance the campaign conditions on, and λ_T
governs every other.

## Overlays and persistence

Everything beside the immutable galaxy is a layer stored by ID: pinned content, deltas, enrichment
and Knowledge ([Overlays and persistence](galaxy-generation.md#overlays-and-persistence)), in the
homes plan 04 reserves in a universe's directory (`pinned/`, `deltas/`, `enrichment/`, `knowledge/`,
`session.json`). This layer adds to each, and writes down no biosphere, phylogeny, field or deposit.

### What play changes

| Change                                  | Kept as                                | Key                                                               | Read by                                                                                                                                                                                 |
| --------------------------------------- | -------------------------------------- | ----------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A sample taken                          | Knowledge, `samples.v1.jsonl`          | Site (a finest-level surface cell), time                          | The sample function, whose sample number is the count of earlier samples at the site ([Organisms](#organisms))                                                                          |
| Forward contamination                   | Delta, `deltas/contamination.v1.jsonl` | Body, site cell, time, the procedure broken                       | The sample function, which adds the crew's microbes to later samples there; the claim fold, capping them at CoLD 1; the protection categories' fold ([What is stored](#what-is-stored)) |
| An animal's flight from the ship        | Session state, in `session.json`       | Individual, time, bearing, distance, recovery time                | The pose function on both sides, until it expires                                                                                                                                       |
| An animal struck                        | Delta, `deltas/biota.v1.jsonl`         | Body, the taxon's clade key, octave cell, index, generation; time | Placement on both sides, the client's copy by the taxon's handle on the scene topic; it lapses when the generation turns over                                                           |
| A plant felled, ground cleared or burnt | Delta, `deltas/flora.v1.jsonl`         | Flora instance (octave cell, index) or coarse cell; time          | Flora and `life_at` on both sides ([Biomes as places](#biomes-as-places))                                                                                                               |
| A deposit or trapped ice depleted       | Delta, `deltas/deposits.v1.jsonl`      | Body and the deposit's draw key; body and cell                    | The ore-deposit list ([Resources and deposits](#resources-and-deposits))                                                                                                                |

A sample is Knowledge: the count that keys the next one is a fold of the samples log, so no delta
repeats it, and the log also orders two samples taken in one tick; a flight lasts hours at most and
a replay regenerates it, so it is saved with the clock. **Lean:** a delta is keyed as generation
keys its subject, never by a position in a list: a deposit by its draw key (process, the tract's
anchor cell, slot), so that it would survive a later migration across a change that leaves the
tract in place. A delta that removes an authoritative instance reaches the client before its
synthesis uses the cell: flora by a delta revision per body on the scene topic, beside R09's
`surface_revisions`, whose revision counts survey passes only; strikes and flights on the scene
topic.

### The Knowledge overlay

Knowledge is append-only JSON-lines logs in `knowledge/` on R09.T18's shared helpers
(`knowledge/jsonl.rs`: a `{"format":N}` header, each line synced before it is applied, one writer
per universe, and a build reading only its own format), folded on load into what the views read:

| Log                                         | One line                                                                                                                                                               | Owner                                                                    |
| ------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------ |
| `contacts.v1.jsonl`                         | A sighting or an acknowledgement (P12.T7, built)                                                                                                                       | Plan 12                                                                  |
| `surveys.v1.jsonl`, then `surveys.v2.jsonl` | A pass: body, span, source, resolution, cells (R09.T18, built); format 2 adds the instrument classes it ran, and a format-1 pass is read as giving no imaging coverage | R09; [Survey passes and coverage](#survey-passes-and-coverage)           |
| `observations.v1.jsonl`                     | A measurement: body or contact, span, observer, method, instrument parameters, values with uncertainties or limits                                                     | [Biosignatures](#biosignatures); [Prospecting](#prospecting)             |
| `samples.v1.jsonl`                          | A sample's site, time, collection record and containment, then each analysis and its result                                                                            | [Samples and on-board analysis](#samples-and-on-board-analysis)          |
| `catalogue.v1.jsonl`                        | An entry: subject kind (taxon, biome, landmark, deposit), opaque ID, rung, placement, merges, names the crew gives                                                     | [Organisms](#organisms); [Fit with the consoles](#fit-with-the-consoles) |

**Lean:** three new logs rather than five: a prospecting anomaly is an observation, an assay is a
sample's analysis, and one catalogue for every subject is the one record shown two ways that [Fit
with the consoles](#fit-with-the-consoles) wants. Lines take sequential IDs, as `ContactId` does;
true identities (an unresolved `BodyId`, a deposit's key, a clade's path) leave only through views,
as a contact's event does (P12.T7.a). What a console states is a fold, never stored: the CoLD claim,
the life detail level ([Detail levels for life](#detail-levels-for-life-and-how-they-degrade)),
coverage per layer (about 2.4 MB for six layers at level 8), the protection category, JORC
categories and the sample count per site. Measured values are stored rather than re-derived, as plan
12 stores a sighting's flux.

### Pinned content

[A pinned homeworld](#a-pinned-homeworld-and-the-biosphere-lazy-generation-gives-it) gives the rule,
selection for generated homeworlds and whole-record overrides checked against every gate for
authored ones, so `pinned/life.v1.jsonl` holds two kinds of line:

- **A selection**: the role, the search's inputs (cell, epoch, filter), the `BodyId` found and a
  digest of its record, compared on load, so that a moved record refuses the universe rather than
  swapping its homeworld. The human homeworld is a selection with human limits in the filter
  ([The crew is human](#the-crew-is-human)), and so is the single-player starting system, chosen by
  rule or by the player
  ([Starting, failing and purpose](single-player-experience.md#starting-failing-and-purpose)).
- **An override**: the record as data, checked at load against the body's habitability timeline; one
  that needs a gate the timeline keeps shut refuses the universe, naming the gate.

**Lean:** overrides only for worlds authored as data, as scenarios are. R09's field cache keys by
`GalaxyKey`, (seed, generator version), which every universe of a seed shares, yet an override
changes the second pass's air and all after it, so caches downstream of the life record key by a
`ContentKey`: the `GalaxyKey` with a digest of the overrides in the system, zero for none, so that
systems no override touches share their entries across universes.

### Stored enrichment

**Lean:** a line of `enrichment/enrichment.v1.jsonl` is keyed by subject (a catalogue ID or a body),
knowledge rung and a digest of its fact sheet, with the model and prompt version. The fact sheet is
a fold of Knowledge, never of the truth, so the text states nothing the ship has not established
([Naming, cataloguing and enrichment](#naming-cataloguing-and-enrichment)); a text is superseded,
and kept, when the digest changes: a new rung, a merge, a claim that falls. Nothing waits on it.

### What is never stored, and the generator version

Every generated thing is recomputed and cached only while wanted: both records, the
`BiosphereHistory` and its forcing, the `LifeTable` and every `LifeState`, phenology, flora, the
phylogeny, pools, individuals and paths, drawing records, sample contents (a function of site,
sample number and contamination), the ore-deposit and landmark lists and the volatile fields. A save
made under another generator version refuses to open (plan 04; [open question
4](rendering-and-planets.md#open-questions) of the rendering brainstorm), so nothing stored meets a
changed truth, and how Knowledge crosses a bump once a release keeps old saves is [open question
77](#open-questions). **Lean:** no Knowledge or delta line carries a generator version of its own;
`universe.json` holds it, and the batch that first fills plan 14's slot bumps it ([Generator
version](#generator-version)).

## Runtime and code shape

Truth is computed on the server in `hyperion-sim`; whatever a client draws it computes itself, from
the same quantised inputs, in `hyperion-surface`, and the dependency runs sim → surface → base
([Runtime and code shape](rendering-and-planets.md#runtime-and-code-shape)).

### Where each part runs

| Part                                                                                                        | Crate and module (leans)                                                                       | Runs   | When                                             |
| ----------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- | ------ | ------------------------------------------------ |
| The biosphere slot, `BiosphereSource`, `NoLife`; the habitability timeline and the callable O₂ steady state | `hyperion_sim::planetary::hooks::biosphere`, plan 14's                                         | Server | Every body query                                 |
| `BiosphereHistory`, forcing, body-level productivity, chemotrophy                                           | `hyperion_sim::life::{system, clock, forcing, energy}`                                         | Server | Once per system, cached                          |
| The `LifeTable`: strategies, pigments, covers, two global constants                                         | `hyperion_sim::life::strategies`                                                               | Server | Once per living world, in the field's bulk job   |
| `life_at`, `LifeState`, `BiomeClass`, phenology, the productivity kernel                                    | `hyperion_surface::life`                                                                       | Both   | Per cell read                                    |
| Flora, individuals, paths, flight excursions                                                                | `hyperion_surface::{flora, biota}`, flora through R10's `BakeHook`, as R11's rocks fill it     | Both   | Per finest patch; per frame near the ground      |
| Phylogeny, pools, samples, designations, CoLD rule tables                                                   | `hyperion_sim::life::{phylogeny, pools, samples, designation, claims}`                         | Server | Lazily, on survey, sighting or analysis          |
| Ore deposits, landmarks, volatile fields                                                                    | `hyperion_sim::content::{deposits, landmarks, volatiles}`                                      | Server | After the coarse pass, memoised beside the field |
| The retrieval; logs, folds, views, deltas, pins, enrichment                                                 | `hyperion_sim::observe::retrieval`; `hyperion_server::{knowledge, deltas, pinned, enrichment}` | Server | Per observation; on change and on load           |

`content` sits above `planetary::surface` and `life`, as `life` sits above `planetary`, so the
module graph has no cycle. Productivity splits as [The energy budget, in closed
form](#the-energy-budget-in-closed-form) leans: the per-cell kernel in the surface crate, the
body-level form and the photon yields at the ground of the abiotic record, from PHOENIX's spectra,
resolved into the `LifeTable` by month and latitude, in the sim.

### On a ship's arrival

```text
1  system = planetary::generate(seed, ctx)                       cached per system, plan 14
2  wet    = system.habitability_timelines(ctx)                   every body's, abiotic
3  life   = life::system(seed, ctx, &system, &wet, &pins)        a BiosphereHistory per body
4  record = system.body_at(ctx, i, t, &life)                     forcing at t: sky, readouts
5  base   = system.body_at(ctx, i, UniverseTime::EPOCH, &life)   the record the caches read
   field  = coarse_pass(surface_seed, &CoarseInputs::for_body(&base, ctx))       R09
6  table  = life::strategies::life_table(&life, &base, &field), one life_at sweep,
            the two constants and R07's disc map                                 the biome step
   scale  = R10's albedo scale, on the substrate against the abiotic twin's p    without the sweep
7  ore    = content::{deposits, landmarks, volatiles}(&field, &base, &life)
8  biota  = pools, drawing records and sample contents, lazily, as cells are surveyed or sampled
```

Step 3 reads only abiotic state: the origin is judged on `wet`, and the later O₂ gates
(`Oxygenation`, p_cx, `Land`) on plan 14's callable O₂ steady state (request 7), evaluated on the
abiotic snapshot under the forcing of the stages already reached, never on the second record; a
stage's flux depends only on stages already reached, so the order stays acyclic ([Where life sits
in the pipeline](#where-life-sits-in-the-pipeline)). Steps 5–7 are one bulk job of R09's
`SurfaceService`, which its `prefetch` starts on approach. The history does not depend on t, so a
long stay re-evaluates only step 4. **Lean:** steps 5–7 read the record at the epoch, so their
caches need no time; a stage transition inside the clock window, about 0.25 within 1,000 ly a
window and some 2,100–7,000 galaxy-wide ([Time](#time)), moves the gases at t and not the coarse
layer. Step 6 realises the snapshot's cover fraction and productivity without iteration through the
two constants of [Vegetation as structure](#vegetation-as-structure), applied on both sides: first
a cover threshold, the point at which the area-weighted cumulative cover of the cells, sorted by
value and cell index, meets the snapshot's, to within one cell; then a productivity scale, the
snapshot's total over the covered cells', summed in cell-index order. Where the snapshot's cover
exceeds the area with any potential, the threshold stops at zero and the shortfall is recorded. The
snapshot's cover albedo is a body-level form, never the sweep's, so the forcing never reads the
field. R10.T10.a's albedo scale does not read the sweep: it is solved on the field's substrate
against the abiotic twin's geometric albedo, and the `LifeTable`'s biological covers lie over it
unscaled, so block 0 carries nothing of life (request 38).

### Costs and caches

| Step                                  | Estimate (owning section)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | Budget                                                                                                                                                                                                                                                                                                                    |
| ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Life records and the second pass      | Of order 10²–10³ µs a candidate, a handful of bounded searches on the host's track, cached per system, panspermia O(n²) over a system's n candidates, under 1 KB a record (estimated here); the extinction pulses, if listed, about 8,100 bins of 2⁴⁴ s over 4.5 Gyr at 15–50 ns a bin, 0.1–0.4 ms a system, so listed only on request (estimated here); the second pass, the atmosphere and what reads it derived again, with plan 14's O₂ steady state at each gate, on each body whose record holds an epoch at t or one ended within some 17 Myr, whatever its present habitability ([Where life sits in the pipeline](#where-life-sits-in-the-pipeline)) | Plan 14's target of 1 ms a system, which its provisional benchmark, measured under load, already exceeds: 1,098 µs for `generate` plus `snapshot_at` a field system and 724 µs for `generate` alone, so some 374 µs for `snapshot_at` before life adds its pass; 1,330 µs deriving through `body_at` per body (P14.T46.b) |
| The coarse pass, on the biotic record | Unchanged: seconds at level 8, tens of seconds with the climate model (R09)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   | R09's, started on approach                                                                                                                                                                                                                                                                                                |
| `LifeTable` and the sweep             | 1.9 × 10⁷ closed forms at level L − 1, 0.2–1 s; 1–2 kB ([The cost](#the-cost))                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | The same bulk job                                                                                                                                                                                                                                                                                                         |
| Deposits, landmarks, volatiles        | O(cells); under 1 MB of deposits ([Deposits as places](#deposits-as-places)); 2.4 MB of volatile fields, three `u16` a cell at level 8 (computed here)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        | `HYPERION_SURFACE_CACHE_MB`, default 128                                                                                                                                                                                                                                                                                  |
| A pool                                | Size × depth in node visits, at most 8 × 10⁴ nodes for 10⁴ taxa ([How many kinds, and of what](#how-many-kinds-and-of-what))                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | Per request                                                                                                                                                                                                                                                                                                               |
| Homeworld selection                   | About 200 systems, 0.2 s, at the default rates; about 3 × 10⁴ systems, half a minute, under strict hard steps, and 10⁵–10⁶, minutes, with an Earth-like filter; at plan 14's target of 1 ms a system, about 1.1 ms as provisionally measured ([A pinned homeworld](#a-pinned-homeworld-and-the-biosphere-lazy-generation-gives-it))                                                                                                                                                                                                                                                                                                                           | Campaign set-up, once                                                                                                                                                                                                                                                                                                     |
| The client                            | Microseconds a cell for `life_at`; a few stems and tens of plants a 17.7 m patch ([The per-cell state](#the-per-cell-state-the-material-and-the-instances))                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   | The brainstorm's 40 ms a patch; R11's scatter budget                                                                                                                                                                                                                                                                      |

The coarse field is 14.75 MB for an Earth at level 8 against R03's 15 MiB check, a margin of
0.98 MB, and each further byte a cell costs 0.39 MB (R09, Risks), so nothing here spends it. Caches
are the caller's, bounded by bytes and safe to evict
([Runtime and code shape](galaxy-generation.md#runtime-and-code-shape)): life records beside the
system cache under (`ContentKey`, `SystemId`); the `LifeTable` and content lists under R09's field
key and budget; clade traits in an LRU by (`ContentKey`, `BodyId`, the epoch's window and attempt,
path), since a path names a clade only within one epoch's tree; on the client, drawing records per
pool, `LifeState` beside `SynthCache`, individuals beside R11's `RockCache`. This layer's own
figures are unmeasured estimates; those quoted from plan 14 and R09 are as they state them.

### What reaches the wire

| What                                                               | Reaches a client                                                      | When                                                                       |
| ------------------------------------------------------------------ | --------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| The gases R08 draws and T24.c's aerosols and absorbers, exact      | Yes, for R08's sky; a trace gas R08 does not draw stays on the server | Once `Surface` is granted, after the first measurement in the system       |
| T53.d's `biosphere` and `biological`; stage, forcing, abiotic twin | Never                                                                 |                                                                            |
| The coarse field and its palette                                   | Yes, exact                                                            | Surveyed cells and margin (R09)                                            |
| The `LifeTable`, with the world's biological covers                | Yes                                                                   | With the first block holding a surveyed living cell under imaging coverage |
| `LifeState`, phenology, flora                                      | Computed by the client                                                |                                                                            |
| Drawing records (an opaque handle and a taxon seed each)           | Yes                                                                   | Per pool, when a view descends near enough to resolve individuals          |
| Ore-deposit and landmark lists                                     | Never; skylights go with the field                                    |                                                                            |
| Observations, analyses, catalogue, claims, enrichment              | As views                                                              | As Knowledge grows                                                         |

Two leaks close in the sections that own them. The palette rides in block 0 of every payload, so
biological covers live in the `LifeTable`'s own list, which `cover_entry` indexes ([The per-cell
state, the material and the instances](#the-per-cell-state-the-material-and-the-instances)); and a
radar pass would let `life_at` draw vegetation never imaged, so the life layer and the table follow
imaging coverage ([Survey passes and coverage](#survey-passes-and-coverage)). A rich pool's drawing
records reach about 1 MB ([Organisms](#organisms)), so they are a bulk kind of their own on R03's
binary frames. **Lean:** what a client does not draw from has no wire form: the history, T53.d's
member, the `LineageSeed` and its clades, the content lists and unanalysed samples have no protocol
type and no conversion in the server, and what a console states leaves only through views, as
`KnowledgeStore::view` is. What a client draws from (the field, the `LifeTable`, drawing records) is
exact and gated by coverage, a discipline for an honest client as R09's survey stand-in is.

### Tags and scopes, gathered

| Tag                                                 | Scope           | Key                                                                                              | Registry            | Argued in                                                                      | Note                                                                                                                                                                      |
| --------------------------------------------------- | --------------- | ------------------------------------------------------------------------------------------------ | ------------------- | ------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `body.life.steps`                                   | `Body`          | `BodyId`; per (window, attempt, transition, redraw), the stand-in O₂ delays on reserved words    | Sim                 | [The output](#the-output)                                                      | The transition clocks; a setback's redraw shifts no other draw                                                                                                            |
| `body.life.transfer`                                | `Body`          | The receiver's `BodyId`; per (window, attempt, donor), the receiver's epoch to be                | Sim                 | [Panspermia within a system](#panspermia-within-a-system)                      | Panspermia; a spent clock leaves the next epoch its own draw                                                                                                              |
| `body.life.gases`                                   | `Body`          | `BodyId`; per (window, attempt, stage)                                                           | Sim                 | [A biosphere changes its planet](#a-biosphere-changes-its-planet)              | The forcing, held for the stage                                                                                                                                           |
| `body.life.lineage`                                 | `Body`          | `BodyId`; a block per (window, attempt)                                                          | Sim                 | [Streams and tags](#streams-and-tags)                                          | The `LineageSeed`, one per epoch                                                                                                                                          |
| `body.life.biochemistry`                            | `Body`          | `BodyId`; a word per (window, attempt)                                                           | Sim                 | [Chirality and alien biochemistry](#chirality-and-alien-biochemistry)          | Handedness of an independent origin                                                                                                                                       |
| `body.life.metabolism`                              | `Body`          | `BodyId`; words per (window, attempt)                                                            | Sim                 | [The output](#the-output)                                                      | The menu and each epoch's first phototrophy; a clade's own metabolism is a `clade.*` draw                                                                                 |
| `body.life.strategies`                              | `Body`          | `BodyId`; words per (window, attempt)                                                            | Sim                 | [Choosing a classification](#choosing-a-classification)                        | Growth strategies only: pigments are computed, never drawn                                                                                                                |
| `body.life.designation`                             | `Body`          | `BodyId`: one key for the body's permutation                                                     | Sim                 | [Naming, cataloguing and enrichment](#naming-cataloguing-and-enrichment)       | Round keys of the Feistel network over the epoch's (window, attempt) and the clade key, so designations are unique across the body's epochs                               |
| `system.life.pulse`                                 | `Event`         | `SystemId`                                                                                       | Sim, event `0x0500` | [Streams and tags](#streams-and-tags)                                          | Poisson bins of 2⁴⁴ s, the longest `PoissonBins` allows, listed only on request; opens `0x0500`–`0x05FF`, the first free block, once plan 06 sets it aside (request 22)   |
| `body.geyser`                                       | `Event`         | The body: one eruptive clock per body, until `EventSubject` gains an item on a body (request 22) | Sim, event `0x0501` | [Geology as places](#geology-as-places)                                        | Monotone phase. **Lean:** a body-item subject, from plan 01's identifiers and plan 06's event key, gives each geyser its own; plan 14's free `0x0405` would serve as well |
| `clade.traits`, `.children`, `.transfer`, `.taxon`  | `Clade`         | `ObjectKey::clade(path, depth)`                                                                  | Sim                 | [Descent, not assembly](#descent-not-assembly)                                 | Opened only by a `LineageSeed`, as `clade.sample` is                                                                                                                      |
| `clade.sample`                                      | `Clade`         | `surface_cell`; instance 0 for the site's community, the sample number plus one for each scoop   | Sim                 | [Microbial biospheres, the common case](#microbial-biospheres-the-common-case) | A sample's living contents; 65,535 samples a site                                                                                                                         |
| `surface.coarse.sample`                             | `SurfaceCoarse` | `surface_cell`; instance 0 for the site, the sample number plus one for each scoop               | Surface             | [Samples and on-board analysis](#samples-and-on-board-analysis)                | A sample's abiotic contents, and the crew's microbes after a breach, on every world; server-only, opened by the sample function as the content passes open theirs         |
| `surface.coarse.biome`                              | `SurfaceCoarse` | `surface_cell`                                                                                   | Surface             | [The per-cell state](#the-per-cell-state-the-material-and-the-instances)       | **Clash:** `life_at` runs on the client, which cannot open `SurfaceCoarse`. Withdrawn; `surface.life` reserved                                                            |
| `surface.coarse.deposit`, `surface.coarse.landmark` | `SurfaceCoarse` | `surface_item`                                                                                   | Surface             | [Deposits as places](#deposits-as-places)                                      | The registry says only the coarse pass opens these; it gains the content passes and the sample function                                                                   |
| `surface.flora`, `surface.flora.shape`              | `SurfaceDetail` | `surface_cell`                                                                                   | Surface             | [The per-cell state](#the-per-cell-state-the-material-and-the-instances)       | Parallel to R11's `surface.scatter` and `surface.scatter.shape`                                                                                                           |
| `surface.dune`, `surface.doline`                    | `SurfaceDetail` | `surface_cell`                                                                                   | Surface             | [Geology as places](#geology-as-places)                                        | Height terms, a bump each                                                                                                                                                 |
| `surface.biota.place`, `.range`, `.path`, `.shape`  | `Biota`         | `surface_cell`                                                                                   | Surface             | [Instantiation near the observer](#instantiation-near-the-observer)            | **Clash:** the registry asserts every name begins `surface.` and every non-coarse tag is `SurfaceDetail`                                                                  |
| `play.observation`                                  | A session scope | The observation's ID in its log                                                                  | Sim                 | [What each instrument sees](#what-each-instrument-sees-and-from-where)         | No session seed or scope exists yet                                                                                                                                       |

No name collides with the snapshot's `rng/tags.rs` or `hyperion_surface::tags` (belt taxonomy uses
P14.T51.e's own tag), and the sim's tags append at the end of `domain_tags!`. An epoch's draws are
keyed by (window, attempt), never by the epoch's ordinal: the window is the index of the open
interval of the abiotic habitability timeline in which the epoch's origin falls, the attempt its
number within that window, so a changed rate, which moves no window, renumbers no surviving
biosphere. **Lean:** the biota tags are `surface.biota.*` in the surface registry, which the client
ships, with R09's Design note 2, the surface registry's text and its key-form test amended so that
`surface.coarse.*` is `SurfaceCoarse`, `surface.biota.*` is `Biota` and the rest `SurfaceDetail`,
and R04's Design note 4 listing them among the surface registry's tags. The new scopes follow R09's
seeds: `Clade` (`clade.traits`, `.children`, `.transfer`, `.taxon` and `.sample`) opened only by a
server-only `LineageSeed`, `Biota` only by a `TaxonSeed` sent in drawing records, both in base's
`rng` with `ObjectKey::clade(path, depth)`, and `Stream::open` refusing both.

### Code shape

```rust
// hyperion_sim::planetary::hooks::biosphere: P14.T53.d's slot, filled
pub struct Biosphere { fluxes: Vec<(SubstanceId, f64)>, weathering: f64, cover: CoverTerm,
                       branch: RedoxBranch }
pub trait BiosphereSource { fn biosphere(&self, i: BodyIndex, t: UniverseTime)
                                -> Option<Biosphere>; }
pub struct NoLife;  // body_at(&self, ctx, i, t, life: &impl BiosphereSource), snapshot_at alike

// hyperion_sim::life: abiotic state in, a BiosphereSource out. The origin is judged on `wet`;
// Oxygenation, p_cx and Land on plan 14's callable O₂ steady state (request 7), on the abiotic
// snapshot under the forcing of the stages already reached; never on the second record
pub fn system(seed: Seed, ctx: &SystemContext, system: &PlanetarySystem,
              wet: &[HabitabilityTimeline], pins: &LifePins) -> SystemLife; // a BiosphereSource
pub struct BiosphereHistory { epochs: Vec<Epoch> }  // at most four; Stage as in the stage table
pub struct EpochKey { window: u16, attempt: u16 }   // every per-epoch draw's key
pub struct Epoch { key: EpochKey, origin: Gigayears, source: OriginSource,
                   biochemistry: Biochemistry, lineage: LineageSeed,
                   reached: [Option<Gigayears>; 6], phototrophy: Option<Gigayears>,
                   setbacks: Vec<(Gigayears, Stage)>, end: Option<(Gigayears, EndCause)> }

// hyperion_surface, on both sides; R11's scatter::ExtraLayer gains Flora(Vec<Plant>)
pub fn life_at(field: &impl FieldView, table: &LifeTable, cell: PatchKey) -> Option<LifeState>;
pub fn pose(record: &DrawingRecord, id: IndividualId, t: UniverseTime, flights: &[Flight]) -> Pose;
// UniverseTime: the sim's today, and moved into base before this lands (request 29), since the
// surface crate cannot depend on the sim; BodyIndex, SystemContext, SubstanceId: the sim's;
// Gigayears, PatchKey, FieldView: as built
```

## Testing

The galaxy brainstorm's kinds of test apply unchanged ([Testing](galaxy-generation.md#testing)), as
does the rendering brainstorm's rule that whatever both sides compute is equal on native,
`wasm32-wasip1` and `wasm32-unknown-unknown` ([Testing](rendering-and-planets.md#testing)). Life
adds a test of its own: nothing of life reaches a client before the crew has covered where it shows,
and no view states what was not measured.

### Fixtures from the Solar System

Plan 14's Solar System table, `planetary::testing::solar_system_bodies` (in its Provides, not yet
built; the eight planets are `derive::solar::solar_system()` today), with Europa on its own elements
as P14.T56.d takes it, and R09's `planetary::surface::reference` worlds, coarse inputs for the
surface steps, are the fixtures, Earth pinned twice: with `NoLife`, and with Earth's forcing given
by hand, as P14.T53.d's own test gives it.

| Fixture                | What it must show                                                                                                                                                                                                                                                                                        | Sections                                                                                                                                                                                                                                          |
| ---------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Earth, `NoLife`        | No O₂ or O₃; once T13.c is refitted, several times the CO₂ and roughly 10–30 K warmer, by climate sensitivity; its Köppen classes on bare ground; no `LifeTable`; no giant banded iron, no coal, no sediment-hosted lead–zinc or red-bed copper                                                          | [A biosphere changes its planet](#a-biosphere-changes-its-planet); [Biomes as places](#biomes-as-places); [Resources and deposits](#resources-and-deposits)                                                                                       |
| Earth with its forcing | O₂ 0.21 bar within a factor of two (P14.T53.d); ozone near 279 DU (Cooke et al. 2022); CO₂ near 280 ppmv; NPP on land within 36–74 Pg C a year (Zaks et al. 2007) and in the ocean within a factor of two of 48.5 (Field et al. 1998), the energy budget's one test; an edge at 700–750 nm; stage `Land` | [A biosphere changes its planet](#a-biosphere-changes-its-planet); [The energy budget, in closed form](#the-energy-budget-in-closed-form); [Biomes as places](#biomes-as-places); [Whether life arises](#whether-life-arises-and-how-far-it-gets) |
| Mars                   | A fossil epoch or none, by its draws; a crust biosphere from some 8 to 24 km at 19 mW m⁻², its pressure gate reading pore water's hydrostatic pressure; `PRESSURE SUIT`; a methane claim on which Curiosity's and the Trace Gas Orbiter's rungs disagree                                                 | [Whether life arises](#whether-life-arises-and-how-far-it-gets); [The deep crust](#the-deep-crust); [Biosignatures](#biosignatures); [Conditions for a landing party](#conditions-for-a-landing-party)                                            |
| Venus                  | No surface biosphere; the cloud deck off at `exotic_solvents` = 0; its claim trajectory                                                                                                                                                                                                                  | [Cloud decks](#cloud-decks); [Biosignatures](#biosignatures)                                                                                                                                                                                      |
| Europa, Enceladus      | A subsurface origin or none; if one, chemotrophic, Europa at 1.2 × 10⁶–1.2 × 10⁸ kg C a year (10⁸–10¹⁰ mol C) and Enceladus at 10³–10⁶ kg C a year; no surface expression; Enceladus's plume brightest near apocentre                                                                                    | [Whether life arises](#whether-life-arises-and-how-far-it-gets); [Subsurface oceans under ice](#subsurface-oceans-under-ice); [Geology as places](#geology-as-places)                                                                             |
| Titan                  | No life at `exotic_solvents` = 0; its hydrocarbons plan 14's abiotic inventory, not this layer's petroleum; `MASK` for its air, with its cold named beside it                                                                                                                                            | [Solvents](#solvents); [Resources and deposits](#resources-and-deposits); [Conditions for a landing party](#conditions-for-a-landing-party)                                                                                                       |
| Moon, Ceres            | The Moon's regolith ³He of 1.4–15.1 ppb, 10⁶–2.5 × 10⁶ t in all; deposit and landmark lists as goldens for both                                                                                                                                                                                          | [Volatiles](#volatiles); [Geology as places](#geology-as-places)                                                                                                                                                                                  |

Pinned worlds add an Archean-like Earth (CH₄ near 10³ ppmv, a haze), an oxygenated microbial world
at 10⁻²–0.05 PAL, an M-dwarf world with a flaring host that never reaches `Land`, a world sterilised
1 Myr ago that keeps about half its O₂, a red-giant moon, a desiccated M-dwarf planet and an N₂-poor
Earth, which the generator makes too once P14.T53.a's cold-trap leak lands, each with a claim
trajectory as a golden ([A confidence ladder](#a-confidence-ladder)).

### Tests by section

| Section                                                                                | Property                                                                                                                                                                                                                                                                                                                                                                                                                            | Statistical (slow)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               | Golden                                                                                            |
| -------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| [Whether life arises](#whether-life-arises-and-how-far-it-gets)                        | Stages monotone between setbacks; no clock runs while its gate is shut; `stage_at` agrees with the stored times; no record for a never-wet body                                                                                                                                                                                                                                                                                     | Shares by stage against the section's table within Monte Carlo error over 10⁵ synthetic habitability timelines, as `synthetic_star` builds stars, since 10⁵ generated `Temperate` bodies take some 2 × 10⁶ systems; hypoexponential times with every gate open (Kolmogorov–Smirnov); the λ₀ sweep; the F_k table against generated cells                                                                                                                                                                                                                                                                                                                                                                         | The Solar System's records; a flaring M dwarf; a red-giant moon                                   |
| [A biosphere changes its planet](#a-biosphere-changes-its-planet)                      | `NoLife` reproduces plan 14 as built bit for bit; each oxic regime's O₂ in its band; ozone and CH₄'s lifetime monotone in O₂ and near Cooke et al.'s points; gases continuous in time but at recorded changes                                                                                                                                                                                                                       | Per-cell cover against the snapshot's to one cell and productivity against its total, the two constants near neutral on the reference Earth                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      | The pinned worlds above                                                                           |
| [What life is made of](#what-life-is-made-of-and-what-it-lives-on)                     | The photon yield at 5,772 K within 1% of the blackbody's 4.57 mol MJ⁻¹ and monotone in T_eff; ice-equilibrium water activity monotone and 1 at 0 °C; the growing season in 0–1                                                                                                                                                                                                                                                      | `p_left` within its binomial interval over 10⁵ origins                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           | The reference Earth's quantised productivity                                                      |
| [Biomes as places](#biomes-as-places)                                                  | A lifeless world has no `LifeTable` and empty states; flora independent of order; trunks within their contact shape; under the Sun, blue and red absorption and an edge at 700–750 nm                                                                                                                                                                                                                                               | The treeline at a seasonal mean root-zone temperature of 6.7 ± 0.8 °C, as at high-altitude treelines (Körner and Paulsen 2004), and the dry woodland edge near 100 mm of rain a year (Sankaran et al. 2005), the two points that fit the maintenance cost on standing rigid biomass ([Choosing a classification](#choosing-a-classification)) and then hold it; woody cover under Sankaran et al.'s bound, closing near 650 mm; biome NPP shares against Field et al.'s (1998) natural-vegetation totals, their cultivated row reassigned, tropical rainforests 17.8 and savannas 16.8 Pg C a year among them; ocean NPP ordered open, coastal, upwelling; stems more regular than Poisson (Clark–Evans above 1) | Vegetated and microbial Earths, an M-dwarf world; `life_at`, flora and phenology on three targets |
| [Organisms](#organisms)                                                                | Caps never exceeded; convergent traits identical in any order of walking; lateral donors shallower; no individual outside its habitat or home range                                                                                                                                                                                                                                                                                 | Clade sizes geometric given age (χ²); species per genus Yule–Simon; herbivorous mammals within an order of magnitude of Damuth's line; Kleiber's ¾ slope and 72 kcal a day per kg^(3/4) recovered for endotherms; carnivores near 90 kg per 10,000 kg of prey; counts per cell Poisson                                                                                                                                                                                                                                                                                                                                                                                                                           | A microbial tree and a sample at a pinned site; a complex pool, poses at three times, one flight  |
| [Biosignatures](#biosignatures); [Knowledge](#knowledge-surveys-and-the-landing-party) | The claim fold reads no truth; levels monotone, claims able to fall; `SHIRTSLEEVE`, `MASK` and `PRESSURE SUIT` at sea level, on Everest and on Mars                                                                                                                                                                                                                                                                                 | Retrieval pulls standard normal; 3σ limits covering the truth at their rate; lifeless worlds reaching CoLD 1–3 at their mechanisms' rates and never 7                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            | Claim trajectories                                                                                |
| [Resources and deposits](#resources-and-deposits)                                      | No porphyry on a stagnant lid, no hydrothermal deposit on a never-wet world, no giant banded iron or phosphorite on a lifeless one, no sediment-hosted lead–zinc or red-bed copper before `Oxygenation`, no detrital uraninite placer formed after it, no roll-front uranium before `Land` without migrating hydrocarbons, no coal before `Land`; a new process moves no deposit; list metal within its share of P14.T26's fraction | Counts per tract in Poisson intervals; tonnage and grade against each type's USGS model (Kolmogorov–Smirnov), porphyry's median 240 Mt at 0.44% Cu (Singer, Berger and Moring 2008)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | Deposit and landmark lists; dune and doline heights on three targets                              |
| [Fit with the consoles](#fit-with-the-consoles)                                        | `LIFE` never renders `NONE`; the limit, `±` (R10.T13's spacing), mole-fraction (the ladder of `%`, `µmol/mol` and `nmol/mol`, never E notation), isotope and `LAT`/`LON` formatters; the class tokens' ΔE₀₀ floors in normal vision and three dichromacies                                                                                                                                                                          | None                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | An inhabited and a sterile Earth-like world whose readouts differ only by measurement             |

### Determinism and Knowledge gating

- **Order and partition.** Life records whatever order bodies and systems are asked in, flora and
  individuals whatever order cells are queried in, ore deposits, and the homeworld search, which
  never gives two species one body, through the testkit's `assert_order_independent`; the
  extinction pulses through `events::testing::assert_partition_independent`, as plan 06's flares
  are tested.
- **Streams.** The registries disjoint; each surface tag with its one key form; `Stream::open`
  refusing `Clade` and `Biota`; a changed λ₀ or other rate moving no surviving epoch's phylogeny or
  handedness, since epochs are keyed by window and attempt; designations one-to-one across a body's
  epochs. No step iterates to convergence: a computed O₂ crossing is a bracketed root found in a
  fixed number of steps.
- **Client and server.** `life_at`, phenology, flora, poses and flight excursions in goldens equal
  on the three targets, and over a real socket a client's decoded field giving the server's own
  `LifeState` at every surveyed cell.
- **Knowledge**, extending R09.T19.c's socket test. No `LifeTable` before a surveyed living cell
  under imaging coverage, and none for a lifeless world, whose payload has the shape of a living
  world's barren region; drawing records only for surveyed pools; no key named `biosphere`,
  `biological`, `lineage` or `surface_seed`, nor a life stage, in any response or notification; no
  deposit, landmark or taxon in any response that the logs do not hold; no `Surface` grant before a
  measurement in the system; no designation before analysis. Two worlds with the same logs and
  different truths give identical Knowledge views, differing only in what a client draws from: the
  gases R08 draws and, where the crew has covered, the field, the `LifeTable` and drawing records.

### Offline, by hand and in CI

`just ci` runs the property tests, the fast goldens on three targets and the socket tests; the
statistical tests join `just ci-slow`. Offline, in `hyperion-fit` with provenance: the PHOENIX
photon-yield table, Cooke et al.'s (2022) ozone and methane-lifetime tables, the CH₄ and N₂O grey-τ
fits, the regulated O₂ steady state, the weathering refit, the F_k table and the maintenance cost on
standing rigid biomass. By hand: R09's `surface_map` example gains biome, cover and deposit layers,
and a person reads the enrichment, whose one automatic test is that no simulated value changes with
the store present or absent.

## Requests to the earlier layers

What this layer asks of the layers beneath it, gathered from every section, merged and grouped by
owner. Each item gives the request, the section that argues it, why, and when it matters: before a
named task lands, before life is built, or later. An item marked **Finding** came out of checking
this layer against plan 14's and R09's text and code. It is advice for the owning plan's later
ruling, not a ruling, and a physics finding is researched before anyone rules on it. An item marked
**Met** keeps its number, since the body cites it, and says what remains; a partly met one says what
has landed. Beyond these, this layer reads plan 14's built T24.f and, as written, its drafted
T24.c–e, T24.g, T52.c and Phase L, so the drafts' acceptance is itself a dependency.

**Plan 14, the planetary systems.** The first three are text, cheapest before P14.T48.e (in the
21 → 22 batch) and T35.e (directly after it) are built; after them they are code and wire changes
but no version bump, since T53.d's member is absent until a life plan fills it and plan 14 lets
such a member be reshaped until a build first sends a value.

1. **A `BiosphereSource` argument.** `PlanetarySystem::body_at` and `snapshot_at` take one, with
   no default, so the abiotic twin is the same call and plan 14 holds no life code; only `life`,
   tests and goldens pass `NoLife`, and the server's system cache holds the system with its life
   records ([What plan 14 must take](#what-plan-14-must-take)). Before P14.T48.e.
2. **T53.d's slot widened to the forcing.** Net fluxes in units of Earth's pre-industrial biotic
   flux, where T53.d drafts the present one, with T35.e's `flux_earth` documented so, each flux
   with its seasonal amplitude and phase, since T53.d drafts one annual scalar a substance ([Surface
   and temporal signatures](#surface-and-temporal-signatures)); CO, which a biosphere draws down,
   and optionally CH₃Cl and the sulphur gases ([Atmospheric signatures](#atmospheric-signatures));
   a weathering factor; a cover term; the redox branch; all on the snapshot path ([A biosphere
   changes its planet](#a-biosphere-changes-its-planet)). Text before P14.T48.e.
3. **Finding: the `biosphere` member's wire exposure.** P14.T35.e (drafted) mirrors T53.d's member
   and its fluxes to the client, and T53.d's `biological` flag on the palette's `Cover` role would
   reach it in R09's `FieldHeader.palette`, each stating a biosphere outright. Advice: keep the
   member on the server, and biological covers out of the `Cover` role, whose doc comment
   (`hyperion-surface/src/field/palette.rs`) reserves it for them ([Detail levels for
   life](#detail-levels-for-life-and-how-they-degrade)). Before T35.e.
4. **A habitability timeline.** T48.b's wet epoch, one span of surface liquid, generalised to every
   interval of the liquid-water flag, surface state, class and subsurface ocean as closed-form
   crossing times, a dimming host's maximum-greenhouse limit sweeping inward among them, with ocean
   fraction, heat flow, magnetic-field class and the end of the ocean- and photic-zone-boiling
   impacts ([What "appropriate conditions" means](#what-appropriate-conditions-means); [The
   output](#the-output)). Before life: the origin's gates read it first.
5. **The reductant supply.** T50.a's redox and T55.a's outgassing speciation, scheduled before life,
   with serpentinisation H₂ from T51.c's ultramafic crust, radiolytic H₂ and upwelling Fe²⁺
   ([Energy: chemistry and radiolysis](#energy-chemistry-and-radiolysis)). Oxygenation and the
   chemotrophs read them; before life.
6. **Abiotic sources in the same chemistry.** Serpentinisation, redox outgassing of CH₄ and CO and
   Hadean impact methane as T53.d sources on every world, so false positives emerge. P14.T53.a
   (drafted) already takes up the oxygen of a temperate world whose cold trap fails for want of
   nitrogen (Wordsworth and Pierrehumbert 2014), and T55.a, which now also sets the carbon carrier
   from ΔIW, gives the outgassing once scheduled ([False positives, made by the same
   physics](#false-positives-made-by-the-same-physics)). Before life.
7. **Steady states beyond an oxic air.** T24.c's ozone columns and T53.d's CH₄ lifetimes from Cooke
   et al.'s (2022) tables, the lifetimes valid near 0.8 ppmv of CH₄ only; a callable O₂ steady state
   for the stage gates, evaluated on the abiotic snapshot under the forcing of the stages already
   reached; and each gas's seasonal response beside its steady state,
   ΔF τ cos(ωt − φ) ÷ √(1 + ω²τ²), from its lifetime τ and its flux's seasonal amplitude ΔF
   (request 2; [Surface and temporal signatures](#surface-and-temporal-signatures)). As findings,
   an anoxic CH₄ state, T24.c's photolysis against T53.a's diffusion-limited hydrogen escape with
   methane's hydrogen counted (Krissansen-Totton, Olson and Catling 2018, supplement section S6),
   T53.a's whole-molecule methane escape being for Titan-like airs, and CO on Ranjan et al.'s
   (2020) cross-sections ([The rise of oxygen](#the-rise-of-oxygen)). With T53.d.
8. **A weathering steady state.** T13.c's `TEMPERATE_AIRBORNE_CARBON` becomes Krissansen-Totton,
   Arney and Catling's (2018) steady state with f_bio, giving ocean pH, carbonate saturation for
   [microbial reefs](#microbial-biomes-the-common-case) and a phosphorus proxy, fitted so that
   Earth with its biosphere keeps its pre-industrial CO₂, on a greenhouse that saturates at the
   maximum-greenhouse limit, which T13.c's findings record the √p term lacks ([Land](#land)).
   Before life, after T24.b's outgassing and P14.T53.a's CO₂ form, in T53.a's refit (23 → 24) with
   request 9, not in T57.a's; it moves every temperate world ([open question 7](#open-questions)).
9. **Greenhouse and albedo terms for life.** T13.c's grey τ gains CH₄ and N₂O terms fitted to
   published radiative grids at temperate temperatures only, a stated exception to P14.T57.a's rule
   of fits on measured states, since no measured world holds a temperate methane greenhouse. The CH₄
   term is zero at Titan's temperatures, so that the nitrogen term fitted on Titan keeps Titan's
   methane (T24.f), and stands beside the collision-induced term T13.c's findings ask for cold
   methane airs. With them, a haze term on the snapshot path, the cover term in T51.b's area mix,
   and T57.a's water constant and Earth's 0.294 refitted with Earth's gases and cover ([Keeping one
   climate](#keeping-one-climate)). Before life, in P14.T53.a's refit of every greenhouse constant
   (23 → 24), not in T57.a's, with request 8, on request 10's partial pressures.
10. **Finding: partial pressures from mass, not mole fraction.** T13.c sets p_i = M_i g ÷ 4πR²
    (`planetary/derive/atmosphere.rs`, `greenhouse`'s `partials_at`, lines 1242–1271, and the
    weight at line 1439), off by μ_i ÷ μ̄: Earth's CO₂ at 42.3 Pa against the 27.8 Pa that Dalton's
    law gives it in Earth's air, 52% high, and biotic CH₄ 45% low. T24.a's mole fractions, taken as
    p_i ÷ p_s, inherit it as mass fractions, and T51.a's drafted cap, min(M_s g ÷ 4πR², p_sat),
    repeats it for a minor condensable. Advice: Dalton's law, with a capped gas's airborne mass
    solved from its vapour pressure, then refit T13.c, T51.a and T24.f, landed before P14.T57.a's
    fits at the measured inventories (22 → 23), so that those fits read the right partials ([What
    plan 14 must take](#what-plan-14-must-take)). Before requests 8–9.
11. **Finding: the runaway limit.** P14.T12.b's S_eff☉ = 1.0385 is the one-dimensional, cloud-free
    limit, which a body at Earth's present flux reaches about 0.40 Gyr later on plan 06's track,
    before the deoxygenation flux S_deox (about 1.11 S⊕ for a Sun-like host), so that its whole
    biosphere ends at once. Advice: Kopparapu et al.'s (2014) Leconte-scaled coefficients, 1.107 for
    1 M⊕ (0.99 at 0.1 M⊕ and 1.188 at 5 M⊕), with 1.0385 recorded and Wolf and Toon's (2015)
    stability to at least 1.21 S⊕ the less conservative bound, since even 1.107 arrives within about
    10 Myr of S_deox; and whether P14.T25's `Temperate` class should still end at the conservative
    zone's moist-greenhouse edge, 1.0146, which Kopparapu et al. (2014) replace by the runaway
    limit ([When life ends](#when-life-ends-and-when-it-is-set-back)). A generator-version change,
    before life.
12. **Finding: the pre-main-sequence runaway.** The worst flux counts the host from its arrival on
    the main sequence (`max_luminosity_until`, plan 06's recorded deviation and ruling 124.7), so
    M-dwarf planets miss Luger and Barnes's (2015) desiccation and abiotic O₂, though P06.T15's
    tracks hold the luminosity before the arrival and P14.T53.a's drafted test of a dimming M
    dwarf's runaway presumes it. Advice: a peak or fluence since formation for the runaway test,
    leaving the maxima as they are, with request 24 ([Host type](#host-type)). A generator-version
    change, before P14.T53.a, whose drafted test needs it.
13. **Subsurface oceans on every icy body.** Plan 14 already specifies T24.b's heat flow from
    radiogenic, residual and tidal heat, and T49.b's salt and ammonia eutectics. What remains: the
    ocean on planets, dwarf planets and rogues as well as moons; `has_subsurface_ocean` reading that
    whole heat flow rather than the tidal flux alone, and P14.T56.d's branch with it; the melting
    point lowered by T49.b's eutectics, a moon's ammonia from P14.T56.c's nitrogen rank; shell
    thickness, ocean depth, seafloor pressure and heat flux; high-pressure ice between ocean and
    rock; and the nursery's later ruling on P14.T17.a's finding F2, without which no generated
    Europa analogue has an ocean ([Subsurface oceans under ice](#subsurface-oceans-under-ice)).
    Before life, since they are populated by default.
14. **Crust, phosphorus and dose.** The crust's water, as a reservoir of T50.a's inventory, its
    porosity by depth this layer's to state, since plan 14 leaves porosity to its readers
    (science-r09-drainage.md §3); phosphorus in T50.a's inventory, which omits it though the
    registry has P, with its weatherable share; the dose at airless surfaces, a parent's
    magnetosphere included ([The deep crust](#the-deep-crust); [The energy budget, in closed
    form](#the-energy-budget-in-closed-form); [Ore-forming processes on the coarse
    field](#ore-forming-processes-on-the-coarse-field); [Subsurface oceans under
    ice](#subsurface-oceans-under-ice)). Before the habitats they gate.
15. **A surface under a thick envelope.** A surface state, and a class or `has_solid_surface`
    reading, for envelopes above `THIN_ENVELOPE_FRACTION` that lie over rock, so that Stevenson's
    (1999) 10⁴-bar case, 0.9% of an Earth's mass, is neither a `SubNeptune` nor `GasEnvelope`; it
    amends decision-p14-t35e-wire's rule that exactly one of the surface and envelope sections
    applies, as T53.b's `SteamEnvelope` does ([Rogue planets and tidally heated
    moons](#rogue-planets-and-tidally-heated-moons)). Later.
16. **A count of `Temperate` bodies.** P14.T25's slow test is to report the share of FGK hosts with
    a `Temperate` body. What remains: that test widened to the count per system by host type, M
    dwarfs included, and by age, with the share that has exposed land from P14.T24.b's ocean
    fraction, replacing the placeholders of 3 × 10⁵ within 1,000 ly and of 80% with land; and D17's
    quadrature extended by age ([Expected statistics at the
    default](#expected-statistics-at-the-default)). Before the default of [open question
    1](#open-questions) is fixed.
17. **Finding: the registry's missing elements.** P14.T49–T50's 25 elements have Zn and Ni but no
    Cu, Mo, Co, Pb, Au, Ag, Li, U, Th, platinum group or rare earths, and P14.T26 has no base- or
    precious-metal group to scale their deposits by. Advice: append them, each with T50.a's atomic
    weight, protosolar abundance and condensation temperature; key P14.T26's metal groups by
    element; and add ore minerals (`CuFeS2`) as deposits need them, those a lifeless world can hold
    told apart from those that need `Oxygenation` ([Deposits as places](#deposits-as-places)).
    Before the deposit pass.
18. **P14.T26's depth and airless helium-3.** The depth or mass of its "accessible outer layers"; an
    airless ³He case from the Fa–Jin fit with T51.d's maturity, a regolith depth, a temperature cut
    and a moon's shielding, the solar wind's 4.6 × 10⁻⁴ apart from the protosolar 1.66 × 10⁻⁴; a
    titanium indicator on T51.c's basaltic provinces, since the registry's `basalt` has no
    high-titanium variant ([Volatiles](#volatiles)). Before ³He by cell.
19. **Finding: the Moon's tilt for cold traps.** T51.f reads the present obliquity, but a moon's
    traps follow the sun's highest elevation, set by the tilt of its pole to its planet's orbit:
    the Moon's is about 1.5° to the ecliptic (Schörghofer and Rufu 2023), against its 6.7° obliquity
    to its own orbit, at which T51.f's own lunar test fails. R09.T7.d's `cold_trap::fraction`
    already takes the highest solar elevation, and R09's reference Moon carries 1.535°, so the fix
    is its callers' input, T51.f's and R09.T13.f's. And P14.T14.a gives a locked moon a drawn
    obliquity of about 10°, kept once locked, where real ones settle into Cassini states of small
    tilt. Advice: a moon's highest solar elevation from its Cassini state about P14.T56.a's Laplace
    pole rather than a drawn obliquity; its history; and a delivery and gardening budget for f_x
    and the reservoir ([Volatiles](#volatiles)). Before T51.f and R09.T13.f.
20. **Finding: `Gas::formula()` and the consoles.** It returns Unicode subscripts (`CH₄`, `CO₂`)
    that B612 cannot draw. Advice: build no console string from it, which nothing calls yet, and
    take the registry's ASCII formula key (`CH4`) through `Gas::substance()`'s row instead; a
    material keyed by name is shown by its name in words ([What the guide must
    gain](#what-the-guide-must-gain), item 6). Before a console shows a formula.
21. **One coordinate rule.** **Met** (P14.T46.b, version 20): `BodyFixedFrame`'s pole is the spin
    axis by the right-hand rule and its prime meridian lies at W(t) from the equator's ascending
    node on the orbital plane (`planetary/frames.rs`), for every planet, dwarf planet, belt member
    and moon, as the record's `Bulk` section `rotation`, on the wire. What remains: two kinds of
    body have no frame, P14.T27.b's rogue planets, which have no orbit to measure from and which
    this layer populates with subsurface life, and a `generate_planets` system's bodies, whose
    rotation is `NotModelled`. **Lean:** a rogue planet's prime meridian is the ascending node of
    its equator on the galactic plane, a function of its pole that needs no draw, labelled in the
    map's legend; plan 14's to rule, with the pole, in P14.T27.b, before `SURVEY` quotes
    coordinates on one ([The body map](#the-body-map)).
22. **Smaller findings.** T51.e's belt test, qualitative on DeMeo and Carry 2014, made quantitative
    on DeMeo and Carry's (2013) "Largest Removed" shares ([Small bodies](#small-bodies)). For
    geysers, with plans 01 and 06: the event block `0x0500`–`0x05FF` set aside for this layer in
    `id/event_tags.rs`, which needs plan 06's agreement, since plan 06's table of blocks leaves
    everything from `0x0500` unallocated; the geyser event at `0x0501` there or at plan 14's free
    `0x0405`, plan 14's call. **Lean:** an event subject for an item on a body, asked of plan
    01's identifiers and plan 06's event key, since `EventSubject` names only a system or a body, so
    that each geyser has its own phase; until then each body has one eruptive clock, which its
    geysers share ([Tags and scopes, gathered](#tags-and-scopes-gathered)). Von Zahn, Hunten and
    Lehmacher's (1998) helium confirms the He/H₂ of about 0.16 that P14.T24.d and T50.b mark from
    memory. Later; the subject before geysers erupt.

**Plan 06, the stellar stage.**

23. **Ultraviolet, flares and spots.** Near- and far-UV histories by T_eff and activity, the
    near-UV over 200–280 nm, for a surface origin, ozone and CO₂ photolysis; flares with proton
    events (P06.T28.a), their frequency at U-band energies, and a particle history; spot covering
    fraction and contrast by activity for transits, beyond the BY Draconis modulation P06.T26.c
    already gives ([Host type](#host-type); [False
    positives](#false-positives-made-by-the-same-physics)). The near-UV before life.
24. **The pre-main-sequence peak.** P06.T15's tracks give the luminosity before the main sequence,
    but `StarModel::max_luminosity_until` counts from the arrival by plan 06's recorded deviation,
    on which ruling 124.7 rests. **Lean:** a separate peak or fluence since formation for
    desiccation, leaving the maxima as they are; request 12 needs it ([Host type](#host-type)).
    With request 12, before P14.T53.a, whose drafted test needs it.
25. **Winds, and spectra beside R06's.** A main-sequence wind and helium flux in closed form of age
    and activity, which T51.d also needs ([Volatiles](#volatiles)), plan 06's. The photon-yield
    table reads the T_eff and log g that `StarState` already gives, from the spectra R06.T3.a's
    `star_colour` task fetches, with a brown-dwarf grid below PHOENIX's 2,300 K and a blackbody at
    T_eff for the hosts its grid does not span (white dwarfs, hosts above 12,000 K, a giant
    parent's glow), under the stellar libraries' open licence ruling ([Energy:
    light](#energy-light)). The spectra before the table; the wind later.

**Plans 02 and 09, the galaxy's fields and features.**

26. **Explosions near a world.** A closed-form core-collapse rate density by position over a
    system's age, from plan 02's `Component::{density, ages, metallicity}` as plan 09's
    `NurseryRates` combines them, with the young population's metallicity, and a thermonuclear rate
    density from the old population beside it; P09.T32–T35's class grid for explosions in play,
    which alone dates arrivals inside the clock window; `system.life.pulse` drawing explosions only
    before the catalogue's source horizon, −(H + L), the catalogue's own serving after it ([Streams
    and tags](#streams-and-tags)); a long-gamma-ray-burst class, which the galaxy lacks (plan 09's
    or the stellar stage's) ([When life ends](#when-life-ends-and-when-it-is-set-back)). Before the
    extinction pulses.

**Plan 12, observation and Knowledge.**

27. **Three logs and a holder.** `observations.v1.jsonl` (prospecting included), `samples.v1.jsonl`
    (assays included) and `catalogue.v1.jsonl` beside `contacts.v1.jsonl`, on R09.T18's
    `knowledge/jsonl.rs`, and P12.T8, which P12.T7's as-built note and R09.T18 already name as the
    holder of a universe's Knowledge in `AppState`, holding every log, one writer each ([The
    Knowledge overlay](#the-knowledge-overlay)). Before life's first measurement.
28. **Life as views.** Life's detail levels follow P12.T7's pattern, where only a view of what was
    measured leaves the store, with taxa under opaque identifiers as `ContactId` is ([Detail levels
    for life](#detail-levels-for-life-and-how-they-degrade)). With request 27.

**R03, R04 and plan 04, transport and determinism.**

29. **Two scopes and two seeds in base's `rng`, and the time.** `TagScope::{Clade, Biota}`, the
    server-only `LineageSeed` and the `TaxonSeed`, `ObjectKey::clade(path, depth)`, `Stream::open`
    refusing both, and R09's Design note 2, the surface registry's text and its key-form test
    amended so that `surface.biota.*` is `Biota`, with R04's Design note 4 listing it among the
    surface registry's tags ([Tags and scopes, gathered](#tags-and-scopes-gathered)); and
    `UniverseTime` moved from the sim's `time` into base, for the surface crate's phenology,
    generations and poses ([Code shape](#code-shape)). Before any life tag opens.
30. **A drawing-record bulk kind.** On R03's binary frames, in plan 04's table of reserved kinds
    with its size class, asked for as a view descends; with view kinds for life, the catalogue and
    samples, and scene-topic fields for flights, strikes and a flora delta revision per body beside
    R09's `surface_revisions`, whose revision counts survey passes only ([What reaches the
    wire](#what-reaches-the-wire); [What play changes](#what-play-changes)). Before organisms reach
    a client.

**R08, atmospheres.**

31. **The light at the ground.** T24.a carries only the stellar flux, but pigments, productivity and
    eyes need the photon spectrum at the surface, in two uses kept apart. Pigments and body-level
    productivity read the abiotic record's, with the stage's ozone column from the oxic regime's
    table (Cooke et al. 2022) for screening pigments' ultraviolet, so that the cover albedo plan
    14's second pass reads depends on nothing that pass or the Surface path makes; eyes, the light
    covers are drawn under, and consoles read the living record's, biotic gases and haze included.
    **Lean:** a closed form on the server over T24.c's absorbers and modes, plan 14's, with R08's
    reduced cross-sections as its data and R08's reference tracer in `hyperion-fit` as its test
    ([Pigment, colour and the seasons](#pigment-colour-and-the-seasons)). Before the pigment model.
32. **The sky from exact gases, granted late.** R08 keeps drawing T35.e's exact `gases` and T24.c's
    aerosols, a living world's O₃ among the absorbers and its haze among the modes, while the
    `Surface` grant, which plan 14's D16 leaves to the Knowledge overlay, follows the first
    measurement in the system, never a remote rung; and `gases` reaches the client only as far as
    R08 draws it, its main gases and the absorbers it has layers for, so that a trace gas it does
    not draw, where most biosignature gases lie, stays on the server, with one remainder entry so
    that the list still sums to 1 within the 10⁻⁹ R08.T3.c requires ([Detail levels for
    life](#detail-levels-for-life-and-how-they-degrade); [open question 22](#open-questions)).
    Before T35.e.

**R09, the surface generator.**

33. **The biome step and the content passes.** A step after the coarse pass (R09's steps 1–6), on
    its quantised field, in `SurfaceService`'s bulk job, then the content passes and the sample
    function, which open `surface.coarse.*` tags (`surface.coarse.sample` among them) under an
    amended registry text; `FieldKey` keyed by a `ContentKey`;
    `surface.life` reserved in `SurfaceDetail`, with no coarse biome tag ([On a ship's
    arrival](#on-a-ships-arrival)). Before life.
34. **A `LifeTable` slot, and no life in the header.** A payload slot sent with the first block
    holding a surveyed living cell under imaging coverage, before R09.T19's first send or with a
    format and version bump after; no biological covers in `FieldHeader.palette`, so
    `PaletteRole::Cover`'s documentation drops its reservation for a life plan, as P14.T53.d drops
    its flag (request 3), and an albedo scale that carries nothing of life (request 38); `life_at`'s
    reads in T8's read-set test ([What reaches the wire](#what-reaches-the-wire)). Before R09.T19.
35. **Instrument classes on survey passes.** R09.T18's built `SurveyPass` records the instrument
    classes it ran, folded into coverage per layer with imaging among them, as format 2 of the
    survey log, with every format-1 pass read as giving no imaging coverage. Format 1 would not do:
    its loader ignores members it does not know, so an older build would silently drop the classes
    and append passes without them ([Survey passes and coverage](#survey-passes-and-coverage)).
    Before more passes are written.
36. **Fields that life and content read.** The monthly surface shortwave flux by latitude and host,
    by the locked coordinates on a locked world, from R09's climate step, handed to the server's
    `LifeTable` rather than stored in the field; land and basin connectivity; volcanic provinces as
    a list; sediment thickness on land, and a sedimentary cover per cell (carbonate, evaporite,
    clastic or organic), the body's stored carbonate (P14.T50.a) spread over its shelves and basins,
    with the karst-prone and sand-sea marks as members of the field, since dolines and dunes change
    height. The water supply is [Biomes as places](#biomes-as-places)' own, from R09's precipitation
    and runoff bytes ([The energy budget, in closed form](#the-energy-budget-in-closed-form);
    [Ore-forming processes](#ore-forming-processes-on-the-coarse-field); [Geology as
    places](#geology-as-places)). With R09.T12–T15.
37. **Labels and later height terms.** Erosion and precipitation labelled vegetation-free; map
    layers for biome, cover and deposits; later, height terms for dunes, dolines, glacial troughs
    and skylight pits, on request 36's sedimentary cover and marks, each a version change with
    goldens on three targets, and a climate history past the wet epoch ([Geology as
    places](#geology-as-places); [What the coarse field reads](#what-the-coarse-field-reads); [open
    question 55](#open-questions)). Later.

**R10 and R07, terrain and lit bodies.**

38. **Life in the material.** R10.T1.c's cover term, which R10's re-validation adds with
    `Material { class, entry }` (decision-composition), takes each cell's cover from `life_at`
    through a second index space beside it; a cover channel per texel and in R07's disc map.
    **Lean:** T10.a's albedo scale solved on the substrate against the abiotic twin's p, with the
    `LifeTable`'s biological covers unscaled and laid over it, so that block 0 carries nothing of
    life, where a scale solved after the life sweep would let a radar-only survey read life in the
    header ([The per-cell state](#the-per-cell-state-the-material-and-the-instances); [What reaches
    the wire](#what-reaches-the-wire)). Before life is drawn.
39. **A canopy row and a general `±`.** A row of R07's phase table for vegetation's backscatter
    hotspot under lunar-Lambert, R10's to rule ([The per-cell
    state](#the-per-cell-state-the-material-and-the-instances)); R10.T12's `±` formatter, to
    R10.T13's drafted form, generalised to any quantity, with `NOT SURVEYED` shared ([Confidence,
    estimates and limits](#confidence-estimates-and-limits)). The formatter before the guide's
    drafts; the row later.

**R11, surface detail.**

40. **Seams for flora and fauna.** R11 leaves vegetation to this layer, so the life plan adds
    `scatter::ExtraLayer::Flora` with its bake through R10's `BakeHook`, its draw rule and the
    `surface.flora` tags, and an individuals cache beside `RockCache`. Of R11 it asks that the
    could-touch region take a mover's speed over its horizon, so that moving capsule proxies lie
    inside it, and that R11.T6.a's decoration cull take a margin per layer, so that herbaceous cover
    and small animals are culled against a body within their height of the ground ([Truth and
    decoration](#truth-and-decoration)). After R11's rock tasks.
41. **Ocean colour.** The biosphere's pigment absorption and backscatter in R11.T8's ocean, from the
    `LifeTable` and each surveyed cell's productivity, never from the liquid's palette entry,
    labelled derived ([Oceans, ice and shallow water](#oceans-ice-and-shallow-water)). Later.

**The sessions and sensors plans, not yet written.**

42. **Sessions.** A session seed, and a session `TagScope` in base's `rng` beside request 29's, for
    play's draws, a measurement's error among them (`play.observation`); flight records in
    `session.json` and the replay log, pushed on the scene topic; closed-loop sample, analysis and
    containment commands ([What play changes](#what-play-changes); [What each instrument
    sees](#what-each-instrument-sees-and-from-where); [The displays, and who owns
    them](#the-displays-and-who-owns-them)). Before a measurement or a flight is real.
43. **Sensors.** Instrument definitions (aperture, bands, resolving power, throughput, contrast
    floor, inner working angle) and a noise model, with a systematic floor per instrument, below
    which co-adding no longer helps, and a table of near-coincident lines by which a retrieval may
    misidentify a species, so that a remote false positive can arise from the instrument as well as
    from the world ([Atmospheric signatures](#atmospheric-signatures)); the caller of `survey_pass`
    with each pass's instrument classes, and which are imaging; deposit detection; `SENSORS`'s
    discovery log, read from `catalogue.v1.jsonl`; and the body-level overlay R09 and R10 already
    ask of it ([What each instrument sees](#what-each-instrument-sees-and-from-where)). Before real
    instruments.

**The UX guide, as edits for the owner.**

44. **Displays and maps** (items 1–4 of [What the guide must gain](#what-the-guide-must-gain)). Five
    displays and a `SITE` panel, a name a belt's record already uses; body maps as a class of
    display, planetocentric and east-positive; the class raster, the single-hue rule's first
    exception; the image layer ([open questions 17, 20 and 52](#open-questions)). Drafted once
    R10.T13's `±` and `NOT SURVEYED` are signed off.
45. **Units and chemistry** (items 5–6). `NPP` in `g/(m²·yr)` and biomass in `kg/m²`; mole fractions
    in `%`, `µmol/mol` and `nmol/mol`, never bare E notation; partial pressures in `Pa`, `kPa` and
    `MPa`; grades in `%`, `g/t` and `mg/t`, the label naming which fraction a value is; crew doses
    in `mSv` and `mGy-Eq`, others in grays; formulae as `CH4`, every chemical symbol in its case
    (`Cu`), isotopes as `He-3`, `δ` and `‰` as SVG glyphs ([open questions 18, 19 and
    52](#open-questions)). With request 44.
46. **Uncertainty, names and labels** (items 7–11). Four kinds of uncertain statement, `LIFE` never
    `NONE`; readout sections for life, deposits and landmarks, `NOT EXAMINED` before the first
    measurement and never `NOT RESOLVED`; names for stages, habitats, biomes, CoLD rungs, the JORC
    Code's categories with the Exploration Target below them, open-nomenclature qualifiers, deposit
    and landmark kinds, and `NOT ANALYSED`; taxa by designation, never italic; `GENERATED` over
    enrichment; the map's DOM list and a class legend selected by outline ([open question
    21](#open-questions)). With request 44.
47. **Procedures and a tool** (item 12). `BREATHABILITY`, with its ratings and failing margins
    ([Conditions for a landing party](#conditions-for-a-landing-party)), the confidence rung's
    notation (`LEVEL 3/7` under a heading that names the CoLD scale in words), COSPAR checklists,
    and the commands for destructive analysis and opening containment under the guide's existing
    `ARM` and `EXECUTE` ([Planetary protection as
    procedures](#planetary-protection-as-procedures)); a dichromacy and ΔE₀₀ script beside the
    console-ux skill's `contrast.py`, before the class tokens (item 11).

## Decisions

Settled with the project owner on 2026-10-10:

- **The crew is human**, because real humans play it: "The crew is human, because they are being
  played by real humans (recall that this is a bridge simulator)." This settles the crew question
  the galaxy brainstorm deferred on 2026-09-20. Landing-party conditions, biohazards and doses are
  judged against human physiology ([Conditions for a landing
  party](#conditions-for-a-landing-party)). Where humans come from, in a fictional galaxy with no
  Sol, is the civilisations brainstorm's question ([The crew is human](#the-crew-is-human)).
- **How common life is: the owner's prior, to be checked against the literature.** "I suspect
  virus-like/microbial life is quite common where conditions are appropriate. From there I suspect
  each 'stage' (simple multicellular, macroscopic plants/animals, crude intelligence, etc) is
  exponentially more rare than the previous. However we should check these predictions against the
  literature where possible." The check, in [How common life
  is](#how-common-life-is-the-rate-of-origin) and [The
  transitions](#the-transitions-and-whether-each-is-hard), found the first half supported weakly and
  contradicted by nothing: Kipping (2020) favours a rapid origin by a Bayes factor of at least 2.8,
  8.7 on the 4.1 Ga zircon, while Spiegel and Turner (2012) show that Earth's early start cannot
  exclude very low rates. The second half holds only for the singular steps, oxygenic photosynthesis
  and the complex cell; steps that arose many times (Knoll 2011) and steps gated by oxygen (Mills et
  al. 2025) are easy once their gates open, so the later rungs are far less rare than the prior
  expects. Which default the generator ships stays open as [open question 1](#open-questions), and
  whether a universe may choose its own as [open question 73](#open-questions).

Inherited from the earlier brainstorms, and unchanged except where noted:

- **The realism ruling applies**: where a choice is open, the most realistic answer wins unless it
  is technically infeasible ([Decisions](galaxy-generation.md#decisions); the infeasibility clause
  from [Decisions](single-player-experience.md#decisions)). Where the science is unconstrained, as
  abiogenesis's rate is, the quantity is a generator parameter with stated bounds and a default, as
  the rogue-planet abundance is.
- **The galaxy is a function, not a database** ([What to take from the
  references](galaxy-generation.md#what-to-take-from-the-references)). Life and content are pure
  functions of the universe seed, the `BodyId`, the time and the earlier layers' outputs, with no
  stored state; play's changes are deltas in an overlay.
- **Life was deferred on 2026-09-20, and is now partly answered.** The galaxy brainstorm deferred
  life, civilisations and campaign set-up, including how common life is, where a campaign starts and
  whether the crew is human ([Decisions](galaxy-generation.md#decisions)). The crew question is
  settled above; how common life is has the owner's prior, checked, with its default open; where a
  campaign starts and everything of civilisations stay deferred.
- **The Knowledge principle** ([Overlays and
  persistence](galaxy-generation.md#overlays-and-persistence)). The server holds the truth; consoles
  receive only what sensors have resolved; a readout quotes only what the ship has measured, marked
  as an estimate with its uncertainty until resolved, in the guide's Estimated state.
- **The surface seed is server-only, and the coarse field is computed on the server** and sent
  region by region as the ship surveys ([Knowledge, and the surface
  seed](rendering-and-planets.md#knowledge-and-the-surface-seed); [Who computes the coarse
  field](rendering-and-planets.md#who-computes-the-coarse-field-and-why-it-is-the-server)).
- **Truth and decoration** ([The line between truth and
  decoration](rendering-and-planets.md#the-line-between-truth-and-decoration)). Every rigid instance
  0.2 m tall or more is authoritative, and the material is authoritative, baked beside the height
  and never re-decided by a shader; anything below the line is decoration, labelled so where it
  matters.
- **LLM enrichment is stored and never fed back** ([Overlays and
  persistence](galaxy-generation.md#overlays-and-persistence)). It is generated once, lives in the
  server, and may read this layer's facts, here only those Knowledge holds.
- **An empty biosphere slot.** The composition ruling, signed off on 2026-10-09 under the owner's
  delegation, has the pipeline reserve an empty biosphere slot for the life generator to fill, and
  make no life itself ([Hooks for the layers
  above](galaxy-generation.md#hooks-for-the-layers-above);
  [Decisions](rendering-and-planets.md#decisions)). P14.T53.d's `Biosphere` hook is that slot.

Recommended here, each argued in the section it links to. They are technical choices, not the
owner's rulings. Many were settled among the drafts by the technical rulings of 2026-10-10, among
them where the per-cell life state is computed, where biological covers live, the fluxes' unit, the
tags and the first phototrophy, and they stand as leans open to review; those that rest on an open
question link it, and stand until it is answered, by the owner where it is marked (owner):

- **Life is a consequence of its world, known only by measurement.** Taxa, biomes and deposits
  follow from the world's physics and history and share descent, and they reach the crew through a
  ladder of instruments: the common lesson of the references ([What to take from the
  references](#what-to-take-from-the-references)).
- **Two passes through one slot.** Plan 14 computes the abiotic snapshot. Life's origin is judged on
  that lifeless twin's timeline, and each later gate of oxygen on plan 14's callable O₂ steady
  state, evaluated on the abiotic snapshot under the forcing of the stages already reached, so the
  order stays acyclic. Life fills plan 14's reserved slot through an injected `BiosphereSource`,
  which `body_at` takes with no default, only `life`, tests and goldens passing `NoLife`; plan 14
  recomputes the same passes with the slot filled, and every reader reads the second record, with no
  iteration to a fixed point. A biosphere's own warming or cooling is recorded there, never judged,
  since plan 14's states are set by the host's flux alone ([Where life sits in the
  pipeline](#where-life-sits-in-the-pipeline); [open questions 7 and 56](#open-questions)).
- **The forcing is biological rates only.** Net fluxes of CH₄, O₂ and N₂O, with a CO sink and
  optionally CH₃Cl and the sulphur gases, in units of Earth's pre-industrial biotic flux, which
  request 2 asks P14.T53.d, drafted in present units, to adopt; a weathering factor f_bio; a cover
  term whose area and albedo are body-level forms of the stage under the abiotic record's light; and
  the redox branch, each drawn on `body.life.gases` and held for the stage. Plan 14 owns every
  equation that turns them into gases, albedo and temperature, never into another `SurfaceState` ([A
  biosphere changes its planet](#a-biosphere-changes-its-planet); [open question
  57](#open-questions)).
- **Streams per property group, never by time.** One tag per property group of life, appended at the
  end of the sim's `domain_tags!` and named in one table, every per-epoch draw keyed by its
  habitable window and attempt, never by the epoch's ordinal, so that a changed rate renumbers no
  surviving biosphere. The extinction pulses are keyed by system on `system.life.pulse`, in Poisson
  bins of 2⁴⁴ s listed only on request, at `0x0500` in the first free event block once plan 06 sets
  it aside, beside `body.geyser` at `0x0501`, one eruptive clock a body until `EventSubject` gains
  an item on a body. Designations are keyed once per body on `body.life.designation`, and a
  server-only `LineageSeed` and a `TaxonSeed` open `Clade` and `Biota` scopes that `Stream::open`
  refuses ([Streams and tags](#streams-and-tags); [Tags and scopes,
  gathered](#tags-and-scopes-gathered)).
- **Time in closed form.** A stage is the count of dated transitions that age + t has passed, and
  after each every gas relaxes to its new steady state as exp(−Δt ÷ τ), so any moment is evaluated
  directly. Transitions inside the clock window are allowed and continuous, about 0.25 within
  1,000 ly a window, and O₂'s 1.7 Myr residence leaves a world sterilised a million years ago about
  half its oxygen ([Time](#time); [open question 33](#open-questions)).
- **The biogenic atmosphere by stage.** Before oxygen, a temperate CH₄ greenhouse term, capped above
  10⁻³ and zero at Titan's temperatures, and the haze's cooling on the snapshot path, from both of
  T24.c's thresholds, the two stated exceptions to P14.T57.a's rule of fits on measured states
  ([open question 8](#open-questions)); and a chemotrophic CH₄ flux drawn as a share of plan 14's
  reductant supply, of a stand-in supply until P14.T55.a lands ([open question
  74](#open-questions)). After it, three oxic regimes keyed to the stage record's crossings
  (10⁻² PAL to p_cx, p_cx to 1 PAL, and 0.7–1.5 PAL at Earth's pressure with land), with ozone and
  CH₄'s lifetime from Cooke et al.'s (2022) three-dimensional runs ([Before
  oxygen](#before-oxygen-the-commonest-living-world); [The rise of oxygen](#the-rise-of-oxygen);
  [open question 31](#open-questions)).
- **Oxygenation as a redox crossing.** It is the first time after oxygenic photosynthesis that the
  reductant sink falls below the O₂ sources, biological and abiotic (Claire, Catling and Zahnle
  2006), a bracketed root in age on the abiotic snapshot under the forcing of the stages reached.
  Until plan 14's reductant supply lands, stand-in delays on reserved words of `body.life.steps`
  time it, and the computed crossing then replaces them under a version bump ([The rise of
  oxygen](#the-rise-of-oxygen)).
- **One climate, refitted.** A weathering steady state with f_bio replaces T13.c's fixed airborne
  carbon, on a greenhouse that saturates at the maximum-greenhouse limit; Dalton's law corrects the
  partial pressures before P14.T57.a's fits, and plan 14's temperate constants are refitted on Earth
  with Earth's forcing in P14.T53.a's refit of every constant, rather than in a third; no biological
  cloud effect is modelled. So a lifeless Earth twin has no O₂ and several times the CO₂, and is
  roughly 10–30 K warmer, by climate sensitivity ([Land](#land); [Keeping one
  climate](#keeping-one-climate); [open question 7](#open-questions)).
- **Truth has no wire form.** The biosphere's history, forcing, stage and abiotic twin, T53.d's
  member and `biological` flag, lineage seeds and clades, content lists and unanalysed samples have
  no protocol type and no conversion in the server, and leave it only as views. What a client draws
  from (the field, the `LifeTable`, drawing records, and the gases R08 draws once `Surface` is
  granted after the first measurement in the system) is exact and gated by coverage, a trace gas R08
  does not draw staying on the server. Life's Knowledge follows plan 12's view pattern,
  `Unexamined`, `Signatures`, `Mapped`, `Catalogued` and `Sampled`, since a world's lack of a
  biosphere can never be measured ([What reaches the wire](#what-reaches-the-wire); [Detail levels
  for life](#detail-levels-for-life-and-how-they-degrade); [open question 22](#open-questions)).
- **Transitions as gated clocks, in a `BiosphereHistory`.** Each stage is an exponential wait at
  rate λ_k that runs only while its gate, read from plan 14 and earlier stages, is open.
  `hyperion_sim::life` computes up to four epochs a body, per system, by exact inversion over plan
  14's gate intervals, the O₂ gates re-found after each stage and each computed O₂ crossing a
  bracketed root in age, with no time step ([The shape of the model](#the-shape-of-the-model); [The
  output](#the-output); [open question 36](#open-questions)).
- **The origin as a parameter on plan 14's conditions.** λ₀ per gigayear of open gate, per body
  whatever its size ([open question 61](#open-questions)), bounded to 10⁻³–10², its default of 2
  left to [open question 1](#open-questions). The conditions are surface liquid water, solid crust,
  and the last giant impact and ocean-boiling era past; a surface origin also needs land and
  near-ultraviolet at the ground, and ice-covered oceans originate at w_v = 0.1 of the rate ([How
  common life is](#how-common-life-is-the-rate-of-origin); [What "appropriate conditions"
  means](#what-appropriate-conditions-means); [open question 3](#open-questions)).
- **A rate per step, not a uniform factor.** Seven stages, `Origin` to `Land`, in a strict chain;
  singular steps at the geometric middle of their bounds, and steps that happened many times fast
  once gated; the O₂ gates as plan 14's partial pressures, 10⁻⁵ PAL and p_cx = 10³ Pa ([The
  transitions, and whether each is hard](#the-transitions-and-whether-each-is-hard)).
- **First phototrophy as a dated field.** Each epoch carries a `phototrophy` date drawn on
  `body.life.metabolism`, at 5 per Gyr after `Origin` and no later than `OxygenicPhotosynthesis`,
  rather than a `Stage`; microbial land cover, wet ground's productivity and photoferrotrophic
  banded iron gate on it ([The output](#the-output)).
- **Endings kept apart.** Sterilisation ends an epoch and allows a re-origin, the old one kept as a
  fossil; a lasting gate closure sets stages back ([open question 58](#open-questions)), a snowball
  judged on the abiotic twin and deoxygenation past a fixed effective flux S_deox, about 1.11 S⊕ for
  a Sun-like host, calibrated by flux, never by years; extinction pulses are dated and change no
  stage, radiation acting only through a 10 Myr recovery gate, with gamma-ray bursts asked of a
  generator; and flares shut the land gate about an active host on a body without a magnetic field
  ([When life ends, and when it is set back](#when-life-ends-and-when-it-is-set-back); [open
  questions 30 and 34](#open-questions)).
- **Host type through gates, and panspermia within a system.** M-dwarf suppression comes from
  pre-main-sequence desiccation, weak near-ultraviolet, flares and light, not a penalty. Panspermia
  is an extra origin rate κ from inhabited neighbours, and none between systems, whose life would
  then hang on past galactic orbits; the receiver records donor and time, copies the `Biochemistry`
  whole and walks the donor's lineage only above the transfer ([Host type](#host-type); [Panspermia
  within a system](#panspermia-within-a-system); [open question 35](#open-questions)).
- **Hooks by age, homeworlds by selection.** D17's habitable-world density per large cell gains an
  age dimension through an offline F_k table of some 10⁵ entries. Homeworlds, the human one included
  if there is one, are found by selection, with whole-record overrides only for worlds authored as
  data, and every cache downstream of the life record keys on a `ContentKey`, with a digest of the
  overrides in the system ([Per large cell](#per-large-cell); [Pinned content](#pinned-content);
  [open question 25](#open-questions)).
- **The next rung, provisional for the next brainstorm.** Crude intelligence at 2 per Gyr after
  complex multicellularity, and technological intelligence at λ_T = 0.01 per Gyr once crude
  intelligence and land life exist and O₂ allows sustained open-air combustion, at least 87.8% PAL;
  a human homeworld, if there is one, does not move λ_T ([The next
  rung](#the-next-rung-and-the-tension-with-a-game-that-wants-civilisations); [open question
  26](#open-questions)).
- **Water and carbon by default.** A habitat's solvent is what plan 14's condensates, its
  subsurface-ocean flag and crustal water say a body holds, ammonia–water counted as water with an
  antifreeze; carbon only, with no silicon branch; exotic solvents and giant atmospheres only behind
  speculative parameters, which are universe settings, recorded in `universe.json` and never shown
  on a console ([Solvents](#solvents); [Scaffolds](#scaffolds); [open questions 4, 5 and
  73](#open-questions)).
- **One growth envelope.** Tested per coarse cell and month, its passing share the growing season:
  liquid water from −15 °C (a parameter to −20 °C) to 122 °C, water activity staged by stage with
  ice-equilibrium brines, MgCl₂ under about 2.3 M, pH about 0 to 12.5, pressure to 125 MPa, and
  radiation by shielding depth ([The limits of known life](#the-limits-of-known-life); [open
  question 37](#open-questions)).
- **A `Biochemistry` per independent origin.** Solvent, scaffold and a handedness bit drawn on
  `body.life.biochemistry` at `p_left` = 0.5 ([open question 6](#open-questions)). Compatibility
  with human biochemistry is a ladder, on which only shared lineage could feed a human ([Chirality
  and alien biochemistry](#chirality-and-alien-biochemistry)).
- **Light from PHOENIX.** A photon-yield table by T_eff and log g, built offline, and a blackbody at
  T_eff for the hosts its grid does not span: white dwarfs, hosts above 12,000 K and a giant
  parent's glow. Productivity reads that light as it reaches the ground of the abiotic record; one
  oxygenic band of 400–750 nm serves every host, and the light gate sits at the growth floor of
  about 0.17 µmol photons m⁻² s⁻¹ ([Energy: light](#energy-light)).
- **Productivity in closed form.** P = min(f_T × P_L, P_W, P_N) × f_C per cell and month, CASA's
  form with Miami's structure and ε_q = 0.0060 mol C per mole of absorbed photons, P_L zero above a
  phototroph ceiling of 73 °C, P_N = k_P × S ÷ S⊕ with k_P the one constant the reference Earth
  fits, and f_C a C3-like or C4-like ramp by each strategy's carbon pathway, the body-level form
  mixing them at the C4 share s₄; anoxygenic productivity capped by electron donors, and chemotrophy
  as Σ R |ΔG| ÷ G. The kernel runs per strategy in `hyperion_surface::life` at level L − 1, the
  body-level form in the sim scales plan 14's fluxes, and the reference Earth is the layer's one
  acceptance test ([The energy budget, in closed form](#the-energy-budget-in-closed-form); [open
  questions 38 and 39](#open-questions)).
- **Habitats populated by default.** Subsurface oceans, chemotrophic, and the deep crust are on; ice
  veins and brines belong to their ocean or cell; cloud decks and giant atmospheres are off; rogue
  planets and dark moons are on wherever a surface passes the envelope ([What is populated by
  default](#what-is-populated-by-default); [open questions 59 and 60](#open-questions)).
- **Biomes in closed form, on both sides.** `life_at` reads the quantised field and the biosphere's
  `LifeTable`, with no bytes per cell and no draws. One unconstrained sweep, then a cover threshold
  on the cells' area-weighted cumulative cover and a productivity scale over the covered cells, both
  carried in the table, meet the snapshot's figures without iteration ([Biomes as
  places](#biomes-as-places); [The per-cell
  state](#the-per-cell-state-the-material-and-the-instances); [open question 24](#open-questions)).
- **Mechanism apart from label.** Up to sixteen growth strategies a biosphere on
  `body.life.strategies`, by a stratified design over the trade-offs, each with a carbon pathway,
  scored per cell in closed form. A strategy survives where its productivity, net of what it builds
  into rigid tissue and of a maintenance cost on its standing rigid biomass, stays positive, the
  cost fitted so that the reference Earth's treeline and dry woodland edge fall where they do, and
  the survivors share the light by height, tallest first. The biome is a structural label,
  IGBP-style thresholds extended to crusts, mats, reefs and algae, with Longhurst's four ocean
  biomes checked against Field et al. (1998); Earth names are display labels at most ([Choosing a
  classification](#choosing-a-classification); [Oceans, ice and shallow
  water](#oceans-ice-and-shallow-water); [open question 41](#open-questions)).
- **Microbial biomes, the common case.** Six classes, crust, mat, brine bloom, microbial reef, snow
  and ice algae and plankton province, each sited on the coarse field and visible from the epoch's
  first phototrophy. Reefs grow on every lit shelf of a biosphere without macroscopic grazers whose
  ocean is carbonate-saturated by request 8's chemistry, every temperate water ocean counted
  saturated until then, their lithified heads of 0.2 m or more authoritative ([Microbial biomes, the
  common case](#microbial-biomes-the-common-case)).
- **Vegetation under its world's gravity.** Canopy height is the least of productivity's, the
  strategy's and a hydraulic limit, 122 m at 1 g and about 320 m at 0.38 g; stature grows after
  `Land` over some 82–92 Myr, Earth's from its land-plant spores by 475 Ma to its forests at
  393–383 Ma; fire holds savanna open only above about 16% O₂ at Earth's pressure ([Vegetation as
  structure](#vegetation-as-structure); [open question 42](#open-questions)).
- **Pigment from Kiang's rules.** The primary absorber placed by Kiang's rules, computed for each
  stage a biosphere reaches from the photon spectrum at the ground of the abiotic record, the
  stage's ozone column screening its ultraviolet; noise-cancelling as the cross-check and the rules'
  spread as each cover's uncertainty; screening pigments on exposed microbial surfaces; phenology
  computed on both sides after Jolly, Nemani and Running (2005); covers drawn under the host's light
  through R07's luminance-only exposure, and ocean colour from the alien pigment's absorption,
  labelled derived ([Pigment, colour and the seasons](#pigment-colour-and-the-seasons); [open
  questions 10 and 40](#open-questions)).
- **Biological covers in the `LifeTable`, behind imaging.** The covers are a list in the table,
  never in the field header's palette, which every payload's first block carries. The table travels
  with the first block holding a surveyed living cell under imaging coverage, and the material
  learns of life through R10's cover term, laid unscaled over an albedo scale solved on the
  substrate against the abiotic twin, so that block 0 carries nothing of life ([What reaches the
  wire](#what-reaches-the-wire); [open questions 43 and 78](#open-questions)).
- **Life on the truth line.** Flora is instanced as rocks are, with a rigid trunk and a compliant
  crown, and herbaceous cover is decoration at any height ([open question 9](#open-questions)).
  Animals 0.2 m tall or more and fliers 0.2 m across or more are authoritative movers with capsule
  proxies, smaller life is decoration, and a struck animal is a delta ([Truth and
  decoration](#truth-and-decoration)).
- **A phylogeny by descent.** A lazy, server-only tree of eight ranks per epoch, named for Earth's
  ([open question 11](#open-questions)): conserved traits inherited and convergent ones recomputed
  from physics; a clade's children geometric given its age, so that clades of random age give Yule's
  hollow curve, with the means per rank in Mora et al.'s (2011) proportions, set once per epoch and
  scaled to richness; lateral transfer for microbes only from shallower donors; and no assembly from
  parts ([Descent, not assembly](#descent-not-assembly)).
- **Richness, pools and samples in closed form.** Richness from area, temperature and time, pruned
  once below a minimum viable population, the microbial default the census's 2.2–4.3 million taxa;
  macroscopic pools per realm and biome above a 1 cm floor, microbial pools per habitat worldwide;
  samples as functions on `clade.sample`, their contents by rarefaction, each analysis placing a
  taxon only to the rank it resolves ([How many kinds, and of what](#how-many-kinds-and-of-what);
  [Microbial biospheres, the common case](#microbial-biospheres-the-common-case); [open questions 45
  and 48](#open-questions)).
- **Physical caps per body plan.** Land support ∝ g⁻³, calibrated at 1 g to a stated maximum;
  flapping flight ∝ g⁻³ρⁿ, calibrated to 200–250 kg and never above the land limit; an O₂-dependent
  tracheate ceiling; visual pigments spanning the surface photon band, the longest capped by a
  thermal-noise limit that already binds for warm endotherms under the Sun; and gaits by Froude
  number; extrapolated rows marked ([Body plans under physical
  constraint](#body-plans-under-physical-constraint); [open question 46](#open-questions)).
- **Densities from an energy budget.** Productivity to herbivore consumption, capped; then a
  transfer efficiency by link, a share of metabolised energy, land endotherms' fixed by the tests;
  then log-normal shares divided by metabolic rate, tested against Damuth's line for herbivorous
  mammals and Carbone and Gittleman's ratio ([Ecology and densities](#ecology-and-densities); [open
  question 47](#open-questions)).
- **Individuals as a Poisson process.** Placed per size octave as R11's rocks are and clustered for
  social taxa, moving as a bounded sum of sinusoids in activity time, with flight from the ship the
  one reaction, a server record held as session state ([Instantiation near the
  observer](#instantiation-near-the-observer); [open questions 12 and 64](#open-questions)).
- **Hazards high-level, procedures strict.** Infection rare and never an adapted disease, toxins by
  mode and predators as a rating; alien tissue is not food; procedures run at full strictness on any
  candidate biosphere until its biochemistry is analysed ([Hazards and use](#hazards-and-use);
  [Biohazard realism](#biohazard-realism); [open questions 13 and 14](#open-questions)).
- **Designations that reveal nothing.** A taxon is the body's designation and a keyed permutation of
  its epoch, by window and attempt, and its clade key, keyed once per body on
  `body.life.designation` and invertible only by the server. Enrichment is keyed by subject, rung
  and the digest of a fact sheet folded from Knowledge, so it never states what the ship has not
  measured ([Naming, cataloguing and enrichment](#naming-cataloguing-and-enrichment); [open
  questions 27 and 49](#open-questions)).
- **One atmosphere, two kinds of source.** P14.T53.d's chemistry takes abiotic fluxes from every
  world's own state beside the biotic ones, so false positives are emergent physics, never a draw,
  and a mechanism the generator cannot yet make is stated as unmodelled ([Atmospheric
  signatures](#atmospheric-signatures); [False
  positives](#false-positives-made-by-the-same-physics); [open question 71](#open-questions)).
- **A measurement is a retrieval.** One pure function from truth, geometry, instrument, integration
  time and the observation's key in the session's stream to a value with its uncertainty, or a
  limit, its error drawn from that stream. Temporal signatures need one closed-form seasonal
  response beside T53.d's steady state and no model of their own, and transit geometry is a
  navigation choice ([What each instrument sees, and from
  where](#what-each-instrument-sees-and-from-where); [Surface and temporal
  signatures](#surface-and-temporal-signatures); [open question 50](#open-questions)).
- **CoLD per signature, and no lifeless state.** Each signature carries a rule table, and the claim
  is folded from measurements and can fall, shown as `LEVEL 3/7` under a heading that names the
  scale in words. `CONFIRMED` is level 7 alone, an absorber no registry species fits reaches level 1
  at most, and a silent world reads `NO BIOSIGNATURE ABOVE LIMITS`, never "none" ([A confidence
  ladder](#a-confidence-ladder); [open question 21](#open-questions)).
- **Coverage per layer.** R09's survey passes record the instrument classes they ran, as the survey
  log's format 2, every format-1 pass read as giving no imaging coverage; the fold gives a coverage
  byte per cell per layer, and imaging alone shows life ([Survey passes and
  coverage](#survey-passes-and-coverage); [open questions 44 and 66](#open-questions)).
- **Samples and protection as procedure.** A sample's collection record caps its evidence, the
  ship's own organics subtracted. COSPAR categories are assigned per activity at each body from what
  is known, with checklists under `ARM` and `EXECUTE`, a drone before the crew at an uncharacterised
  site, a protection officer's role, and a breach a delta that contaminates later samples ([Samples
  and on-board analysis](#samples-and-on-board-analysis); [Planetary protection as
  procedures](#planetary-protection-as-procedures); [open questions 51 and 79](#open-questions)).
- **Human limits in one type.** `CrewTolerance` holds NASA's, NIOSH's, OSHA's and ICNIRP's human
  limits, shifted by suit, acclimatisation and the individual, and takes a stay's duration, the RELs
  within a day and NASA's SMACs beyond. A site is `SHIRTSLEEVE` where inspired PO₂ reaches 11.9 kPa,
  the aviation rule's at 3,810 m; `MASK` where a filtering or supplied-gas mask makes it safe,
  holding that floor on oxygen down to about 18.8 kPa total; and `PRESSURE SUIT` below. NASA's
  16.9 kPa is the ship's habitat standard, and every rating is from measurements only ([Conditions
  for a landing party](#conditions-for-a-landing-party); [open question 69](#open-questions)).
- **Deposits from processes on the coarse field.** Each ore-forming process is a setting,
  requirements as predicates on plan 14's crust and inventory and the biosphere's stage, and a
  grade–tonnage model, evaluated at its formation age and surviving by erosion. Banded iron is gated
  on stage, as are the deposits oxygen flips (lead–zinc, red-bed copper and redox-front uranium
  after `Oxygenation`, roll fronts after `Land`); coal and petroleum are this layer's, petroleum's
  yield the stage's productivity times an anoxic-interval draw; and the abiotic hydrocarbons are
  plan 14's ([Ore-forming processes on the coarse
  field](#ore-forming-processes-on-the-coarse-field); [open question 54](#open-questions)).
- **A deposit list that realises plan 14 and stays on the server.** `OreDeposit`s beside the crater
  list, from a memoised content pass after the coarse pass, with Poisson counts per tract scaled by
  P14.T26's fractions, grades unscaled and the list's metal bounded by them. Clients see only
  prospecting observations, catalogue entries on the JORC ladder, an Exploration Target with ranges
  of tonnage and grade until samples make a deposit `INFERRED`, and assays ([Deposits as
  places](#deposits-as-places); [Prospecting](#prospecting); [open questions 53 and
  76](#open-questions)).
- **Small bodies typed by plan 14 alone.** This layer adds no typing, only what a type holds: each
  assemblage carries its meteorite analogue's composition, and a metallic surface lies over a bulk
  as mixed as Psyche's, so that P14.T26's platinum-group rule holds body by body ([Small
  bodies](#small-bodies); [open question 75](#open-questions)).
- **Volatiles, and landmarks by the truth line.** A delivery and gardening budget for cold traps,
  which take the greatest solar declination from a synchronous body's Cassini state rather than a
  drawn obliquity; a ground-ice table per cell, regolith ³He as P14.T26's airless case and giant
  fuel from plan 14 alone. A landmark is a coarse-field list or a local synthesis term: lava tubes
  server-side with skylights as pits; geysers in one hydrothermal list with the massive sulfides,
  one eruptive clock a body until a body-item event subject exists; karst and sand seas marked in
  the field, since they change height; solar-driven jets a vent density on volatile ice
  ([Volatiles](#volatiles); [Geology as places](#geology-as-places); [open questions 15, 55, 62 and
  63](#open-questions)).
- **Five displays and body maps.** `SURVEY` with its `SITE` panel, `LIFE`, `ANALYSIS`, `PROSPECT`
  and `CATALOGUE`, their names provisional, commands staying with the owning station, and
  irreversible sample commands under `ARM` and `EXECUTE`; a body's readout gains life, deposit and
  landmark sections, each `NOT YET MODELLED` until a generator version computes it and
  `NOT EXAMINED` before its first measurement. Body maps are a class of display, planetocentric and
  east-positive with a right-hand pole, `BODY NORTH`, on every body plan 14 gives a frame ([The
  displays, and who owns them](#the-displays-and-who-owns-them); [The body map](#the-body-map);
  [open questions 17, 68 and 72](#open-questions)).
- **Class tokens and four kinds of uncertainty.** `--class-1` to `--class-8`, at least ΔE₀₀ 22 from
  every reserved colour in normal vision and three dichromacies, the class raster the single-hue
  rule's first exception. A number takes `~` and R10.T13's `±`, a non-detection a limit, a
  categorical claim its rung and an identification `CF.`, `AFF.` or `SP.`, and `LIFE` never says
  `NONE`; mole fractions run `%`, `µmol/mol` and `nmol/mol`, and no console says how a universe was
  generated ([Colour for classes](#colour-for-classes); [Confidence, estimates and
  limits](#confidence-estimates-and-limits); [open questions 18, 20 and 52](#open-questions)).
- **Knowledge logs and deltas.** Three new logs beside the contacts and the surveys, every claim,
  level and category a fold of them, and no generator version per line. Samples are Knowledge,
  forward contamination a delta and flights session state, and deltas are keyed by what generation
  keys, a deposit by its draw key ([The Knowledge overlay](#the-knowledge-overlay); [What play
  changes](#what-play-changes); [open questions 67 and 77](#open-questions)).
- **Arrival, code and tests.** On arrival: generate, every body's abiotic timeline, `life::system`,
  `body_at` with the life source, R09's pass on the record at the epoch, the biome step, the content
  passes, then organisms lazily. `hyperion_sim::{life, content}` sit above `planetary` and
  `hyperion_surface::{life, flora, biota}` run on both sides; Earth is pinned twice among the Solar
  System fixtures, with `NoLife` and with its forcing given by hand ([On a ship's
  arrival](#on-a-ships-arrival); [Code shape](#code-shape); [Testing](#testing)).

## Open questions

Each question is marked **Closed**, where a ruling or research settled it; **Lean**, where research
recommends an answer and a check or a ruling remains; or **Open**. Questions for the owner are
marked (owner). Those of the first draft, 1–28, come first, though the review of 2026-10-10 found 6,
7 and 24 technical; the questions that review added, 56–79, follow the rest, the owner's among them
marked; and 52 returns to the owner with the guide edits it rests on. Closed questions keep their
place and their answers, so that the numbers cited elsewhere stay put. The technical rulings of
2026-10-10 settled further questions among the drafts, among them where the per-cell life state is
computed, where biological covers live, the unit of the fluxes, the tags and the first phototrophy,
and their answers are leans in the body, listed under [Decisions](#decisions). What other layers
must answer is under [Requests to the earlier layers](#requests-to-the-earlier-layers).

1. **How common life is: the default (owner).** **Lean:** the literature-leaning per-step default,
   λ₀ = 2 per Gyr and the singular steps at the geometric middle of their bounds, over the strict
   hard-steps setting that reproduces the owner's ladder ([The
   transitions](#the-transitions-and-whether-each-is-hard)). On a placeholder count within 1,000 ly,
   the default gives 2.6 × 10⁵ worlds with life, 44,000 with complex cells, 35,000 with macroscopic
   life and 30,000 with land life; strict hard steps, 2.6 × 10⁵, 2,800, 250 and 210. Whether the
   rates are fixed by the generator version or chosen per universe is [open question
   73](#open-questions).
2. **Is the crew human? (owner)** **Closed** (2026-10-10): "The crew is human, because they are
   being played by real humans (recall that this is a bridge simulator)." Landing-party limits,
   biohazards and doses are human ones, with `CrewTolerance` kept for suits, acclimatisation and the
   individual; where humans come from is the civilisations brainstorm's ([The crew is
   human](#the-crew-is-human)).
3. **Whether ice-covered oceans originate life (owner).** **Lean:** yes, at a tenth of the surface
   rate, w_v = 0.1, which puts life in some 57% of long-lived subsurface oceans; 0.03 gives 25%, and
   0 makes them hosts reached only by seeding. Such worlds may outnumber rocky habitable-zone
   planets by a few orders of magnitude (Lingam and Loeb 2019c), so w_v decides whether most of the
   Galaxy's life is under ice. Plan 14 makes no generated Europa analogue wet until a later ruling
   on the moons' nursery (P14.T17.a's finding F2), so until then the generated population lacks the
   rock-rich, thin-shelled ocean moons, the strongest chemotrophic case ([What "appropriate
   conditions" means](#what-appropriate-conditions-means); [Expected statistics at the
   default](#expected-statistics-at-the-default); [Subsurface oceans under
   ice](#subsurface-oceans-under-ice)).
4. **Exotic solvents (owner).** **Lean:** `exotic_solvents` stays 0. The evidence is against
   Titan-type life, and Venus's water activity rules out life as known, so a non-zero value would be
   a gameplay choice, not a reading of the literature; like every speculative parameter it is a
   universe setting, recorded in `universe.json` and shown where a universe is created, never on a
   console, since the crew measures its world as it is ([Solvents](#solvents); [Cloud
   decks](#cloud-decks); [open question 73](#open-questions)).
5. **Life in giant-planet atmospheres (owner).** **Lean:** `giant_atmosphere_life` stays off, for
   the same reason and as the same kind of universe setting: Sagan and Salpeter's floaters are a
   1976 thought experiment, with no later support found ([Giant-planet
   atmospheres](#giant-planet-atmospheres)).
6. **Handedness.** **Lean:** `p_left` = 0.5 for each independent origin, a scientific default with
   no lever in play, since either hand is inedible. A galaxy-wide bias towards L would make more
   biospheres same-handed; the meteoritic L-isovaline excess is real but may be local to a forming
   system ([Chirality and alien biochemistry](#chirality-and-alien-biochemistry)).
7. **The nine plan-14 amendments.** **Lean:** accept the two-pass design and all nine, requests 1–9
   ([What plan 14 must take](#what-plan-14-must-take)). They change every temperate world, living or
   not: the weathering refit alone leaves a lifeless Earth twin with several times the CO₂ and
   roughly 10–30 K warmer, by climate sensitivity ([Land](#land)). The partial-pressure finding
   (request 10) lands before P14.T57.a's fits in the 22 → 23 batch, and this layer's terms (requests
   8 and 9, the methane term a stated exception to plan 14's rule of fits on measured states) go
   into P14.T53.a's refit of every greenhouse constant in the 23 → 24 batch, rather than a third
   refit after it ([Keeping one climate](#keeping-one-climate)).
8. **The haze feedback (owner).** **Lean:** the haze's cooling, and only it, enters plan 14's
   snapshot path, because the hazy Archean-like world is the commonest living one: a closed form of
   CH₄/CO₂ and, on N₂–CH₄ airs, of CH₄, as T24.c's two thresholds have it, fitted in P14.T53.a's
   refit as a stated exception to its rule of fits on measured states. Its cooling, as any of a
   biosphere's own, is recorded and sets no stage back ([open question 56](#open-questions)). It
   departs from decision-composition's delegated rule that the Surface path never feeds back
   ([Before oxygen](#before-oxygen-the-commonest-living-world)).
9. **Tall grass as decoration (owner).** **Lean:** herbaceous cover of any height is decoration,
   since the 0.2 m rule is about rigid instances: a stand of 1 m grass is drawn but never collided
   with, and is culled where a body meets it. It reads the truth-and-decoration rule, so it needs
   the owner ([The per-cell state](#the-per-cell-state-the-material-and-the-instances); [Truth and
   decoration](#truth-and-decoration)).
10. **White balance of alien colour (owner).** **Open:** whether the main screen shows vegetation in
    its physical reflectance under the host's light, as R07's luminance-only exposure draws every
    world now, which makes an M-dwarf world red-lit, or adapted to the host's white by a
    chromatic-adaptation step R07 does not have. **Lean:** physical reflectance, as R07 draws it,
    any adaptation being a later ruling of R07's owner, since it would change how every world looks,
    living or not ([Pigment, colour and the seasons](#pigment-colour-and-the-seasons)).
11. **Earth's rank names (owner).** **Lean:** the phylogeny's eight ranks are named for Earth's,
    since the crew is human, though neutral labels remain possible ([Descent, not
    assembly](#descent-not-assembly)).
12. **Fleeing (owner).** **Lean:** animals flee the ship through a server-recorded flight record
    held as session state, the one reaction modelled. The owner may want none, which is cheaper, or
    more ([Instantiation near the observer](#instantiation-near-the-observer)).
13. **Predators (owner).** **Open.** Whether a predator may ever attack a landing party needs an
    on-foot design that does not exist ([open question 68](#open-questions)); until then a predator
    carries a rating, not staged attacks ([Hazards and use](#hazards-and-use)).
14. **A staged infection (owner).** **Lean:** infection stays a rare generated hazard of microbial
    taxa, a colonised wound rather than an epidemic, guarded by procedure. Whether the game may ever
    stage an infected crew member is a product choice ([Biohazard realism](#biohazard-realism)).
15. **Caves for landing parties (owner).** **Open.** A height field has no overhangs, so entering a
    lava tube or a cave is a new volumetric rendering and collision feature. Until then tubes are
    server-side data and skylights are pits. Caves are a habitat as well as a place: they shield
    against dose and ultraviolet, so on a world whose surface fails only on those they could hold
    life between the surface and the deep crust, and no default populates them. **Lean:** a cave
    habitat on the tube and karst lists, where the surface fails only on dose or ultraviolet and the
    interior lies in the envelope, after research on cave interiors' temperature and water ([Geology
    as places](#geology-as-places); [Habitats beyond the surface](#habitats-beyond-the-surface)).
16. **Deuterium and helium-3 as consumables (owner).** **Open.** Whether either is ever reactor fuel
    is a ship-systems ruling; the single-player lean scoops hydrogen as reaction mass, and this
    layer supplies the fractions either way ([Volatiles](#volatiles)).
17. **Display names (owner).** **Lean:** `SURVEY` with its `SITE` panel, `LIFE`, `ANALYSIS`,
    `PROSPECT` and `CATALOGUE`, all provisional, for a science station whose name is open, as is
    whether `PROSPECT` moves once mining is gameplay. Three names collide with the guide's list:
    `SITE` is already a belt's label, so the panel takes another word, such as `LANDING SITE`, or
    the belt's label does; the stage `Origin` needs another console word, since `ORIGIN` means where
    an object formed; and the chart's `LIVING` filter would read as systems with life once `LIFE`
    exists. The cursor names a cell's classes `BIOME`, `CLIMATE` and `SURFACE`, never `CLASS`;
    covers are named by structure (`CRUST`, `MAT`, IGBP's forms) and, before the claim supports a
    biological name, by what was observed (`PIGMENTED CRUST, EDGE 0.70 µm`); Earth names are display
    labels at most; and `BODY NORTH`, `BODY-FIXED <designation>`, whose word is the owner's, and
    `NOT EXAMINED` are new words ([The displays, and who owns
    them](#the-displays-and-who-owns-them); [What the guide must gain](#what-the-guide-must-gain),
    item 8).
18. **`µmol/mol` against ppm (owner).** **Lean:** mole fractions in `µmol/mol` and `nmol/mol`, which
    NIST and the SI Brochure support, rather than the ppm and ppb that atmospheric chemistry and the
    WMO use. The realism ruling cuts both ways here, the discipline's practice against the standards
    ([What the guide must gain](#what-the-guide-must-gain), item 5).
19. **SI style for `mGy-Eq` (owner).** **Lean:** acute organ doses in `mGy-Eq`, as NASA-STD-3001
    states its limits. Strict SI attaches no information to a unit (NIST SP 811 §7.4), as item 5
    argues for the carbon in `NPP`, and would put the weighting in the label; the operational
    standard wins, as the guide's opening says it does ([What the guide must
    gain](#what-the-guide-must-gain), item 5).
20. **The class raster (owner).** **Lean:** eight class tokens for categorical layers, the guide's
    single-hue raster rule's first exception. **Open:** whether a pairwise floor of about ΔE₀₀ 9.5
    between fills (the closest pair 9.48, under deuteranopia) is enough in practice, since it was
    computed, not tested with people; and whether fills of equal lightness, which a monochromat or a
    greyscale screen cannot tell apart, may share a boundary, or the token search adds a lightness
    floor between neighbours ([Colour for classes](#colour-for-classes)).
21. **Who states the claim of life (owner).** **Lean:** the server folds the measurements through
    fixed rules into an advisory claim, which a crew may annotate but never overrides, on CoLD's
    ordinal scale with levels 6 and 7 adapted to a ship; agnostic measures such as an assembly index
    are one line of evidence only. The rung is `LEVEL 3/7` under a heading that names the scale in
    words, `CONFIDENCE OF LIFE DETECTION`, since upper case turns CoLD into `COLD`. Catling et al.'s
    (2018) Bayesian posterior, mapped to five verbal levels, is the alternative, not taken because
    its prior would be the generator's own λ₀, which no console should state. **Open:** whether
    `LIFE` shows the rung's number, its words or both ([A confidence ladder](#a-confidence-ladder)).
22. **The biosphere member and the `Surface` grant (owner, for the grant's timing).** **Lean:** as
    request 3 has it, T53.d's member and flag stay on the server, while the gases R08 draws reach
    its sky exact once `Surface` is granted after a first measurement in the system, under the
    rendering brainstorm's rule: exact for the image, measured for every number. A trace gas R08
    does not draw stays on the server, one remainder entry keeping the list's sum at 1 (request 32).
    The guide's `ATMOSPHERE: APPROXIMATE` note for an absorbing gas the console cannot draw is text
    derived from those gases, and should read only gases the ship has measured, or it becomes a
    detector, R08's to rule with the owner's sign-off on the guide ([Detail levels for
    life](#detail-levels-for-life-and-how-they-degrade)).
23. **The first ship's instruments (owner).** **Open.** Aperture, inner working angle, contrast
    floor and resolving power decide how much of the galaxy's life is detectable from light-years
    away, and so the pace of discovery. They are the sensors plan's, but the pace is a design choice
    ([What each instrument sees, and from where](#what-each-instrument-sees-and-from-where)).
24. **Whether the life state is sent.** **Closed** by the technical ruling of 2026-10-10: computed
    on both sides, because R09's Earth field is already about 14.7 MB of R03's 15 MiB. **Lean:** if
    the client's cost forbids it, the server sends the state at level L − 1, 6 bytes per four cells
    and 15.3 MB in all, over the rendering brainstorm's "roughly 2 to 15 MB" ([The per-cell
    state](#the-per-cell-state-the-material-and-the-instances)).
25. **Pinned worlds (owner, with the civilisations brainstorm).** **Lean:** generated homeworlds,
    the human homeworld if there is one, and the starting system are found by selection; overrides
    only for worlds authored as data, replacing the whole life record and validated against every
    gate at load. An authored world pins its stage history with the bulk and inventory it needs, and
    the two passes derive its twin and its living record, so that its gates, false positives and
    goldens read the same chemistry as a generated world's. Overrides are fixed when a universe is
    created, `universe.json` recording their digest, so that a changed `pinned/` refuses the
    universe as a version mismatch does and no observation meets a changed truth; a scenario that
    authors a world later starts a new universe. An override enters the transfer pass as a generated
    record does, so its siblings may be seeded from it, and one whose epoch names a donor is refused
    unless the donor has life at the transfer time ([A pinned
    homeworld](#a-pinned-homeworld-and-the-biosphere-lazy-generation-gives-it); [Pinned
    content](#pinned-content)).
26. **The next rung and the rare-life tension (owner, for the next brainstorm).** **Lean,
    provisional:** technological intelligence at λ_T = 0.01 per Gyr, once crude intelligence and
    land life exist and O₂ allows sustained open-air combustion, at least 87.8% PAL, about 18.4% at
    Earth's pressure (Balbi and Frank 2024, as Mills et al. 2025 report), a gate kept apart from
    vegetation's 16% for landscape fire. If every land-life world cleared it, that gives some 600
    species ever within 1,000 ly, an upper bound; strict hard steps with λ_T = 10⁻³ give about 0.3
    within 1,000 ly and 5,000 in the Galaxy. A human homeworld does not move λ_T, since an observer
    always finds itself. Four resolutions of the tension are set out in [The next
    rung](#the-next-rung-and-the-tension-with-a-game-that-wants-civilisations).
27. **Enrichment (owner).** **Lean:** keyed by subject, rung and fact-sheet digest, so that a text
    never states what the ship does not know, generated at first sighting and again after analysis
    and regenerated only when the sheet changes, with superseded texts kept in the log. Whether
    `CATALOGUE` shows them as history is a display choice ([Naming, cataloguing and
    enrichment](#naming-cataloguing-and-enrichment); [Stored enrichment](#stored-enrichment)).
28. **The content's granularity and schedule (owner).** **Open.** The deposit list, of order 10³–10⁴
    deposits on an Earth-like world, those below the tonnage 90% of a type's deposits exceed
    statistical, wants confirming against whatever mining design follows; and whether the dune,
    doline, trough and skylight terms land with R09 or a later surface plan, on request 36's
    sedimentary cover and marks, is the owner's scheduling ([Deposits as
    places](#deposits-as-places); [Geology as places](#geology-as-places)).
29. **The count of `Temperate` bodies.** **Open,** for plan 14 (request 16). Every count here rests
    on 3 × 10⁵ within 1,000 ly, one per 20 systems with exposed land on 80% of them, both stand-ins;
    plan 14's η⊕ of 0.173 per FGK host counts planets in the zone, not `Temperate` bodies, and
    leaves out M dwarfs. **Lean:** P14.T25's slow test, which reports only the share of FGK hosts
    with a `Temperate` body, is widened to the count per system by host type, M dwarfs included, and
    by age, with the land share from P14.T24.b's ocean fraction, and the counts are rescaled
    ([Expected statistics at the default](#expected-statistics-at-the-default)).
30. **Plan 14's rulings that this layer waits on.** **Open,** for plan 14: the runaway limit, the
    partial pressures, the pre-main-sequence runaway and the Moon's declination with synchronous
    bodies' Cassini states (requests 10–12 and 19), each a physics ruling to be researched before it
    is made, and the geyser's event subject, an item on a body, with its number (request 22). The
    runaway limit alone sets when most temperate biospheres end: even Kopparapu et al.'s (2014)
    S_eff of 1.107 arrives within about 10 Myr of the deoxygenation flux, so a biosphere outliving
    its oxygen needs Wolf and Toon's later bound or an ocean-loss time ([When life
    ends](#when-life-ends-and-when-it-is-set-back); [What plan 14 must
    take](#what-plan-14-must-take); [Host type](#host-type); [Volatiles](#volatiles)).
31. **The oxic regimes and the O₂ model.** **Lean:** a post-oxygenation plateau of 10⁻² PAL to p_cx,
    with Planavsky et al.'s ≤ 0.1% PAL as the recorded alternative, since the sources disagree and
    the choice rests on two stability analyses, one read only as reported. **Open:** which published
    model plan 14's regulated O₂ steady state is fitted to, and whether it reproduces all three
    regimes' bands; and how fire's limits, and with them the land band's floor, move with total
    pressure, not researched ([The rise of oxygen](#the-rise-of-oxygen); [Land](#land)).
32. **What biology does beyond the forcing.** **Open:** the weathering factor of a microbial
    biosphere before land, default 1 with no global estimate found; whether vegetation should change
    R09's erosion or precipitation, through roots and transpiration, for which no closed form was
    found and whose omission is labelled; and whether biology's nitrogen cycle should move N₂ over
    gigayears, which no section models ([Land](#land); [What the coarse field
    reads](#what-the-coarse-field-reads)).
33. **Stage changes during play.** **Closed** by the technical ruling of 2026-10-10: transitions
    inside the clock window are allowed and continuous, about 0.25 within 1,000 ly a window and some
    2,100–7,000 in the Galaxy, and clades gated by a stage appear at its time. **Lean:** they never
    re-run the coarse layer, whose caches read the record at the epoch, since gases relax over τ and
    cover would follow only over geological time, at a pace [open question 42](#open-questions) asks
    ([Time](#time); [On a ship's arrival](#on-a-ships-arrival)).
34. **Radiation, resets and bursts.** **Lean:** radiation never resets a stage except through the
    recovery gate, default 10 Myr within 5–20 Myr, since Earth survived the Ordovician and
    end-Devonian events, the pulses graded by fluence with a supernova's delayed cosmic rays; the
    literature disagrees on where the Galaxy is safest. The drawn supernovae stop at the catalogue's
    source horizon, after which the catalogue's own, with plan 09's class grid in play, are the only
    ones, and the hazard history uses the system's present position over its whole age, labelled,
    since past galactic orbits are not modelled, a migration term waiting on plan 02. **Lean:**
    gamma-ray bursts get a generator, in plan 09 or the stellar stage (request 26). **Open:** the
    thermonuclear rate's delay-time distribution, taken from memory ([When life
    ends](#when-life-ends-and-when-it-is-set-back); [Streams and tags](#streams-and-tags)).
35. **Panspermia's rates.** **Lean:** κ as chosen, since nothing measures it; at the default λ₀ it
    barely matters, and at λ₀ ≤ 10⁻² it decides whether life clusters in systems; a sterilised
    body's own returning ejecta are left out as a donor, a simplification. "Neighbouring" means any
    two rocky bodies inside the snow line, either way, with a hundredth of κ between a rocky planet
    and a giant's moon, either way, 1 per Gyr between moons of one giant and a hundredth of that
    between moons of different giants. A seeded world takes its donor's `phototrophy` and
    `OxygenicPhotosynthesis` if the donor had reached them by the transfer, prokaryotic traits of
    its founders, while `Oxygenation` and later stages are its own. **Closed** by the technical
    ruling of 2026-10-10: a seeded world's tree walks its donor's lineage only for ranks older than
    the transfer. **Open:** whether eukaryotic founders may travel too ([Panspermia within a
    system](#panspermia-within-a-system)).
36. **The ladder's shape.** **Lean:** a strict chain with `Land` after `ComplexMulticellularity`, no
    minimum durations, up to four epochs a body with fossil epochs as content, and first phototrophy
    at a chosen 5 per Gyr after `Origin`. A body that would hold a fifth epoch keeps its latest
    four, dropping the oldest fossil, and a slow test reports how often that happens. **Open:** how
    many independent origins of complex multicellularity and of land colonisation a biosphere's tree
    gets ([The transitions](#the-transitions-and-whether-each-is-hard); [The output](#the-output);
    [Descent, not assembly](#descent-not-assembly)).
37. **The envelope's gates.** **Lean:** water activity staged, 0.611 before `ComplexCells` and 0.585
    from it; a pressure gate, `max_growth_pressure` at 125 MPa, since every other limit is one of
    demonstrated growth, though liquid water at 0 °C persists to about 630 MPa before ice V forms.
    **Open:** a dose gate for airless icy surfaces, which needs the dose at airless surfaces, a
    parent's magnetosphere included, that request 14 asks of plan 14; until it exists, an icy moon's
    life belongs to its ocean and the ice–ocean interface. And whether a giant's radiation belts
    gate a habitable moon's surface: the lean is that the land gate also reads the surface dose from
    the parent's trapped particles, request 14's dose extended to moons with air ([The limits of
    known life](#the-limits-of-known-life); [Host type](#host-type)).
38. **The productivity form's checks.** **Lean:** one oxygenic band for every host, an upper bound
    about M dwarfs; and f_C by each strategy's carbon pathway, C3-like or C4-like, the body-level
    form mixing them at s₄, the C4 share of oxygenic productivity, about 0.23 on land, C4 plants'
    share of land gross productivity (Still et al. 2003). **Open:** the temperature response, since
    evaluated by month the cool cells rise to some 330–370 and 190–240 g C m⁻² yr⁻¹, so where the
    reference Earth lands in Zaks et al.'s 36–74 Pg C a year is not established, and a failure low
    changes f_T's shape, not ε_q; water-use efficiency, to which Lieth's water term is blind though
    thin air loses more water per carbon (Gale 1972a), a physics ruling that needs research first;
    the CO₂ ramps' ends; whether ocean phytoplankton's carbon-concentrating mechanisms count towards
    s₄, not researched; and a fixed s₄, which ignores that C4 arose late on Earth ([The energy
    budget, in closed form](#the-energy-budget-in-closed-form); [Vegetation as
    structure](#vegetation-as-structure)).
39. **Nutrients on the client.** **Open:** no published closed form of phosphorus supply from
    weathering and hydrothermal sources was found. **Lean:** P_N = k_P × S ÷ S⊕, the supply S on
    land from the cell's `steepness`, `drainage` and `runoff` in closed form, after research and a
    ruling, with a server-resolved factor in the `LifeTable` as the fallback, at sea from upwelling
    and river outlets, and everywhere a hydrothermal floor. **Open:** a nitrogen factor on p(N₂), 1
    at Earth's and 0 at 1 mbar, below the 5 mbar at which nitrogen fixers still grew (Klingler,
    Mancinelli and White 1989), so that an N₂-poor world's productivity falls ([The energy budget,
    in closed form](#the-energy-budget-in-closed-form); [Oceans, ice and shallow
    water](#oceans-ice-and-shallow-water)).
40. **The pigment rules.** **Open.** Under the Sun Kiang's rule gives about 685 nm and the
    noise-cancelling optimum the blue Soret pair; they part by some 200 nm about K and early-M hosts
    and meet near 1 µm for late M dwarfs. **Lean:** Kiang's rules for the primary and
    noise-cancelling as the cross-check, confirmed by research before the model is written, under
    the abiotic record's light at the ground with the stage's ozone column, recomputed for each
    stage a biosphere reaches. **Open:** which moment within a stage sets that light, the lean being
    the transition's, held until the next; and how several hosts' light combines, the lean Kiang's
    first rule taking the global maximum of their time-mean summed photon flux ([Pigment, colour and
    the seasons](#pigment-colour-and-the-seasons)).
41. **The strategy count.** **Lean:** sixteen strategies by a stratified design over the trade-offs,
    the reference Earth testing a fixed Earth-like set. Their survival is a carbon balance net of
    construction and of a maintenance cost on standing rigid biomass, fitted to the reference
    Earth's treeline and dry woodland edge, and their light is shared by height, tallest first, so
    the calibration rests on those two tests. **Open:** whether sixteen suffice: with ten random
    strategies JeDi's global NPP ran from 29 to 57 Pg C across seeds, against a standard 2,000, and
    more cost linearly, 64 taking some 0.8–4 s an Earth, four times sixteen's ([Choosing a
    classification](#choosing-a-classification); [Vegetation as
    structure](#vegetation-as-structure); [The cost](#the-cost)).
42. **Vegetation's structure.** **Lean:** the hydraulic height limit scales as 1/g to first order,
    untested beyond 1 g and an upper bound at low g, with wind loading left out until R09's winds
    are trusted at canopy height; stature grows after `Land` over some 82–92 Myr, a parameter.
    **Open:** a term below the cover bound for savanna, without which the reference Earth's woody
    cover reaches IGBP's forest, 60%, at about 530 mm of rain (computed from Sankaran et al.'s
    bound); the lean is fire above about 16% O₂ at Earth's pressure, and herbivory once organisms
    exist. And whether cover and productivity ramp too, over some 25–30 Myr after `Land`, as a model
    has Earth's cryptogamic cover reaching about 30% of today's land productivity by about 445 Ma,
    plants having colonised land from about 470 Ma (Lenton et al. 2016), and relax after a setback,
    so that no stage change repaints a continent at once ([Vegetation as
    structure](#vegetation-as-structure); [Microbial biomes, the common
    case](#microbial-biomes-the-common-case)).
43. **Life in R10's and R07's materials.** **Open,** for their owners: how
    `Material { class, entry }` and R07's disc map, which holds at most 16 laws a body, take a
    second index space for biological covers, and whether lunar-Lambert gains a row for a canopy's
    backscatter hotspot (requests 38–39); and how many covers a cell carries, since the state holds
    one while strategies share the cover. **Lean:** two, the leading rigid and the leading
    non-rigid, split by the `rigid` share, blended in the bake ([The per-cell
    state](#the-per-cell-state-the-material-and-the-instances)).
44. **Which surveys show life.** **Lean:** life follows imaging coverage, never radar, and R09.T18's
    survey line gains the instrument classes as the log's format 2, every format-1 pass read as
    giving no imaging coverage: a build reads only its own format, and format 1's loader ignores
    members it does not know, so an older build would drop the classes silently and append passes
    without them. **Open:** which of the instrument classes count as imaging, the sensors plan's
    call ([Survey passes and coverage](#survey-passes-and-coverage)).
45. **Richness.** **Lean:** the census's full-length count of about 2.2–4.3 million species-level
    taxa as the microbial default, over the extrapolated 10¹², since a leaf is a species in the 95%
    ANI sense and the census's 0.8–1.6 million are coarser 16S-V4 clusters, which sets how often a
    sample shows new taxa; the kinetic temperature term damped; and z ≈ 0.25 within a realm, a
    steeper z, a parameter, across realms of different area. **Open:** rank widths against
    truncation: under a heavy-tailed draw of mean ten an 8-bit child index truncates about 2 × 10⁻³
    of nodes at any richness, and each further three bits cut that only about tenfold, so the ranks
    need unequal widths or the species index the spare bits of `sub`; z by organism size; and τ_div
    ([How many kinds, and of what](#how-many-kinds-and-of-what); [Descent, not
    assembly](#descent-not-assembly)).
46. **Body-plan caps.** **Lean:** the flapping-flight limit calibrated to pterosaurs' 200–250 kg
    rather than a soaring bird's 41 kg, and never allowed above the land limit, which is calibrated
    at 1 g to a stated maximum, the largest sauropods a floor, with no default yet. **Open:** its
    density exponent n, 1 under power balance within 0–3, which at Mars's 0.38 g⊕ and about
    0.015 kg/m³ moves the limit from about 18 times Earth's at n = 0 through about a quarter at n =
    1 to about 3 × 10⁻⁵ at n = 3, while at Titan's 0.138 g⊕ it stays below the land limit for any n;
    whether the limit should instead be the least of the power, frequency and stress criteria, g⁻³ρ,
    g⁻³ρ³ and g⁻³, each calibrated at 1 g, which needs research on each calibration; and the
    exponent of the tracheate ceiling on P_O₂, which is contested ([Body plans under physical
    constraint](#body-plans-under-physical-constraint)).
47. **Transfer between trophic levels.** **Lean:** ε, the share of a level's metabolised energy that
    reaches the next, resolved by link after Wu et al. (2026): 8% at sea; on land 3.8% for
    invertebrate consumers and 7.2% for vertebrate ectotherms, and land endotherms' fixed by
    Damuth's and Carbone and Gittleman's tests, since Wu et al.'s 0.24% would give carnivores about
    a quarter of the latter's ratio; consumption converted by an assimilation efficiency per diet;
    and McNaughton's consumed share capped, at one half by default. **Open:** a fixed ε makes
    predator biomass proportional to prey biomass, not Hatton et al.'s ¾ power: whether ε should
    fall with prey biomass, and how that squares with Carbone and Gittleman's per-species ratio
    ([Ecology and densities](#ecology-and-densities)).
48. **Drawing records.** **Lean:** whole pools of surveyed cells travel as their own bulk kind when
    a view descends, not per-cell occurrence lists. **Open:** a byte budget per pool and per
    descent, since a rich pool reaches 10⁴ records, about 1 MB; the 1 cm floor itself, since most
    species are small; and how a client learns a cell's realm, which it cannot compute from surveyed
    cells alone: the lean is a realm list per surveyed block in the `LifeTable`'s payload, a realm
    byte per cell at level L − 2, about 0.1 MB on an Earth, the alternative ([Instantiation near the
    observer](#instantiation-near-the-observer); [How many kinds, and of
    what](#how-many-kinds-and-of-what)).
49. **Designations and binomials.** **Open:** the keyed permutation needs a small-domain cipher over
    the epoch's window and attempt and the clade key, keyed once per body on
    `body.life.designation`, a Feistel network on base's Threefry the lean, and its length in digits
    may want shortening for consoles; and whether generated binomials exist at all, or taxa carry
    designations alone ([Naming, cataloguing and enrichment](#naming-cataloguing-and-enrichment)).
50. **Measurement error.** **Lean:** drawn from a session stream keyed by the observation, never
    truth-centred, since truth-centred readouts leak exact values and the presence of sub-threshold
    gases. **Open:** the session seed and the scope for play's draws, `play.observation`, which the
    sessions plan owns; and whether the error is independent and Gaussian alone, under which
    co-added transits average every error away and no line is ever misidentified, or carries each
    instrument's systematic floor and a table of near-coincident lines, as request 43 now asks, so
    that a claim like K2-18 b's can arise and fall ([What each instrument sees, and from
    where](#what-each-instrument-sees-and-from-where); [Atmospheric
    signatures](#atmospheric-signatures)).
51. **Landing-party procedures.** **Lean:** forward contamination is a site delta that contaminates
    later samples, not introduced organisms that spread, which is a later model. **Open:** the
    decompression procedure between the ship's cabin pressure and a site's, and back from a denser
    air, which `CrewTolerance` should hold and NASA-STD-3001 should source; and what ends a crew
    quarantine, what quarantined crew may still do and whether a contamination delta lapses, since
    COSPAR's period of biological exploration is 1,000 years for Icy Worlds (COSPAR 2025). **Lean:**
    a quarantine ends on analysed results that close the protection category, with a fixed fallback
    period, and the delta keeps its full effect ([Planetary protection as
    procedures](#planetary-protection-as-procedures); [Conditions for a landing
    party](#conditions-for-a-landing-party)).
52. **Console text (owner).** **Lean,** pending the owner's sign-off of the guide edits: console
    formulae are plain (`CH4`), the substance registry's ASCII key, not the guide's parenthesised
    subscript; mole fractions take item 5's `%`, `µmol/mol` and `nmol/mol`
    (`CH4 ~2100 µmol/mol ± 600 µmol/mol`, `CO < 400 µmol/mol`), with the guide's E notation, three
    significant figures, only for a value outside its unit's ladder; uncertainty takes R10.T13's
    `±`, the unit on both numbers and a space either side; every chemical symbol keeps its case
    (`Cu`), `%` takes a space (`0.6 %`) and the label names which fraction a value is; isotopes are
    in the hyphen form throughout (`He-3`, `O-18`), since B612's superscripts stop at `³`; and the
    projections are `ORTHOGRAPHIC`, `EQUIRECTANGULAR` and `POLAR STEREOGRAPHIC`, others added when a
    task needs them ([What the guide must gain](#what-the-guide-must-gain), items 2, 5 and 6).
53. **Deposit statistics.** **Lean:** P14.T26's fraction scales counts, not grades; Singer's
    regression of count on area is calibrated on the reference Earth's own tracts; and JORC's
    categories are borrowed for their geological criteria alone, a deposit known only remotely an
    Exploration Target, its tonnage and grade quoted as ranges, and `INFERRED` needing samples. That
    deposits are a memoised content pass after the coarse pass is **Closed**, by the technical
    ruling of 2026-10-10. **Open:** whether extreme metallicities should shift grades, which the
    pooled models cannot test; and kimberlite diamonds, whose cratonic setting is from memory and
    waits on research ([Deposits as places](#deposits-as-places); [Ore-forming processes on the
    coarse field](#ore-forming-processes-on-the-coarse-field)).
54. **Deposits that life makes.** **Open:** the default photoferrotrophic and abiotic shares of
    banded iron, which the evidence bounds between none and all, for research; petroleum before
    oxygenic photosynthesis, meagre but not absent; and whether a readout of a body's hydrocarbons
    sums plan 14's abiotic inventory with this layer's coal and petroleum or shows them apart.
    **Lean:** detrital uraninite's redox limit carries the transport distance; roll-front uranium is
    gated on `Land` as well as `Oxygenation`, needing buried plant debris or migrating hydrocarbons
    (Cuney 2010); supergene enrichment is an oxygen-gated step, sulfides weathering above the water
    table (Sillitoe 2005); and petroleum's yield is the stage's productivity times an
    anoxic-interval draw, its survival falling with the source rock's age ([Ore-forming processes on
    the coarse field](#ore-forming-processes-on-the-coarse-field)).
55. **Past climates and cold traps.** **Lean:** the climate-gated rows, laterite, coal and
    evaporites, read R09's present climate class at their drawn formation age, a proxy until a
    climate history past the wet epoch exists (request 37); the budget replacing T51.f's f_x and its
    reservoir, plan 14's, calibrated to a patchy Moon, about 3.5% of traps exposed, and a Mercury
    holding tens of metres of recent ice; and T51.f and R09.T13.f passing the greatest solar
    declination, from a synchronous body's Cassini state rather than a drawn obliquity (request 19).
    **Open:** the declination's history, which the traps' ages need ([Ore-forming processes on the
    coarse field](#ore-forming-processes-on-the-coarse-field); [Volatiles](#volatiles)).
56. **A biosphere's own climate.** **Lean:** recorded, not judged. Plan 14's `Temperate`, `Snowball`
    and `RunawayGreenhouse` states are set by the host's flux alone, so a biosphere's own warming or
    cooling, a methane collapse at oxygenation or a haze's cooling, shows in the second record and
    on consoles and sets no stage back, while the stage gates read the abiotic timeline and the O₂
    plan 14's steady state gives under the forcing of the stages reached. **Open:** the alternative,
    one lagged check at stage times, which would evaluate plan 14's snapshot with each stage's
    forcing at its time, apply there a state the endings table ends or sets back, and recompute the
    forcing once from the reduced stage, the order staying acyclic; and whether P14.T53.a's escape
    integral, which sums loss over a body's history, reads the forcing of the stages reached as a
    step function of age, so that biogenic methane's hydrogen escape, the oxidation behind
    oxygenation (Catling, Zahnle and McKay 2001), is plan 14's one integral, the lean, rather than
    abiotic escape alone ([Where life sits in the pipeline](#where-life-sits-in-the-pipeline); [When
    life ends](#when-life-ends-and-when-it-is-set-back); [The rise of oxygen](#the-rise-of-oxygen)).
57. **Which climate the body-level forms read.** **Lean:** the body-level productivity and cover
    that scale the forcing read the abiotic twin's climate and the light at its ground, with the
    stage's ozone column, so that nothing the second pass makes feeds them. **Open:** how far that
    departs from the living per-cell field the sweep's two constants scale to it, since the twin
    holds several times the CO₂ and is roughly 10–30 K warmer, while the reference Earth's
    acceptance test runs on R09's living climate; the alternatives are a closed-form correction of
    the twin's temperature and CO₂ by the weathering steady state for the forcing's f_bio, or a
    fixed third pass for the cover term alone ([What the coarse field
    reads](#what-the-coarse-field-reads); [The energy budget, in closed
    form](#the-energy-budget-in-closed-form)).
58. **How long a gate stays shut before stages, or an epoch, are lost.** **Lean:** a gate shut for
    longer than the recovery time, default 10 Myr, sets back the stages above it, and a shorter
    closure pauses their clocks and leaves them standing; the runaway, a snowball and `Destroyed`
    always count as lasting. **Open:** how long dormancy can pause an epoch before a shut gate
    sterilises it, a parameter whose default should come from researched survival records, not yet
    read here ([When life ends](#when-life-ends-and-when-it-is-set-back); [The limits of known
    life](#the-limits-of-known-life)).
59. **Dark habitats: which originate, and how far they get (owner).** **Lean:** the deep crust is a
    refuge, not an origin site: a surface epoch that loses its water sets back to `Origin` and
    continues in the crust where crustal water passes the envelope. A rogue's buried water
    originates at λ₀ w_v on its heat flow, with no life from before ejection, which plan 14 does not
    record (P14.T27.b), while a body unbound from a generated system keeps its epochs. Dark habitats
    stop at `Origin`, their energy at most some 10⁻⁵ of Earth's productivity, a cap recorded as such
    rather than an emergent result, the alternative gating `ComplexCells` on an ocean's dissolved
    O₂, which radiolysis may give a Europa (Hand, Carlson and Chyba 2007). A dark habitat burns its
    H₂ with CO₂ and, where sulfide is present, with radiolytic sulfate in proportion to its
    radiolytic H₂, O₂ coming only from photosynthesis or surface radiolysis, after research. A
    buried ocean's floor, which no coarse field maps, carries a body-level vent density from heat
    flow and seafloor area, with no positions, known through plumes. **Open:** whether complex life
    may ever arise under ice, which decides whether the commonest habitats hold anything but
    microbes ([Subsurface oceans under ice](#subsurface-oceans-under-ice); [The deep
    crust](#the-deep-crust); [Rogue planets and tidally heated
    moons](#rogue-planets-and-tidally-heated-moons); [Geology as places](#geology-as-places)).
60. **Several habitats or liquids on one body.** **Lean:** habitats in water contact, a surface
    ocean, the crust beneath and an ice-covered ocean, share one origin and one `Biochemistry`,
    while a habitat in another solvent runs its own clock at `exotic_solvents` × λ₀, gated on plan
    14's liquid or deck, within the four epochs a body holds, each origin's `Biochemistry` naming
    its habitat's solvent. **Open:** whether two chemistries may coexist on one body at all
    ([Solvents](#solvents); [The output](#the-output)).
61. **Whether the origin rate scales with the habitat's size.** **Lean:** λ₀ per body, since nothing
    measures a scaling, Scharf and Cronin's (2016) heuristic for origins on a planetary scale naming
    the parameters, not their values; so a Mars-sized world and a 5 M⊕ one with the same gates
    originate equally fast, and any land opens the surface term. **Open:** the recorded alternative,
    a surface term in proportion to land area and w_v to wet seafloor area, normalised to Earth,
    behind a parameter, which would move life towards large worlds and water worlds ([What
    "appropriate conditions" means](#what-appropriate-conditions-means)).
62. **More than one star.** **Lean:** the host terms combine over the stars at the body as plan 14's
    limits do: g's near-ultraviolet summed, S_deox read against the summed flux, the land gate shut
    while any star giving more than a tenth of the body's flux is highly active, and a circumbinary
    world's climate swings plan 14's state. A cell is a cold trap only if it is one under every
    host, each host's highest elevation over the spin axis read from its own orbit, since an
    inclined companion can light polar craters and a Sun-like one within about 12.5 au √(L ÷ L☉)
    warms a floor facing it above water's 110 K (computed). **Open:** how a pigment chooses between
    two hosts' peaks, on which [open question 40](#open-questions) leans, and how two suns set an
    animal's day, on which [open question 63](#open-questions) leans ([Host type](#host-type);
    [Volatiles](#volatiles)).
63. **Tidally locked and slowly rotating worlds.** **Lean:** what Earth's rotation sets is taken
    from the physics R09's locked coordinates carry. Open water on a world without an Earth-like
    circulation is classed by its light by month and its mixing, Longhurst's names used only where
    the bands exist; phenology's day length, which the section reads from the cell's daily
    insolation, is the lit fraction of the solar day, its thresholds Earth's as fractions, with no
    photoperiod factor where the solar day outlasts R09's month; animals' activity follows local
    insolation from plan 14 by a threshold per activity, integrated in closed form under one sun and
    by a fixed number of steps otherwise; and R09.T7.d's zonal cold-trap fraction gains a
    longitudinal branch for 1:1 locks, read by P14.T51.f and R09.T13.f alike, since an airless body
    locked to its star never lights the hemisphere beyond its terminator's libration, some 47% of
    its surface at e = 0.05, at a temperature its heat flow sets, about 36 K at 0.09 W m⁻² (both
    computed). **Open:** each, none tested on such a world ([Oceans, ice and shallow
    water](#oceans-ice-and-shallow-water); [Pigment, colour and the
    seasons](#pigment-colour-and-the-seasons); [Instantiation near the
    observer](#instantiation-near-the-observer); [Volatiles](#volatiles)).
64. **Animals in three dimensions and by season.** **Lean:** a migratory taxon carries a smooth
    displacement field of the month on its own stream, and the 3 × 3 search runs about the point
    less the displacement, so that a herd keeps its identity between ranges and densities stay
    annual. A flier's height and a swimmer's depth are each a bounded sinusoid about a niche height
    or depth drawn on `clade.traits`, swimmers authoritative only within a craft's draught, and
    aquatic densities per area from the water column's productivity. **Open:** whether submerged
    craft are in scope, which is the owner's ([Instantiation near the
    observer](#instantiation-near-the-observer); [Truth and decoration](#truth-and-decoration)).
65. **Whether a sighting places a taxon by its true path or by what was seen.** **Lean:** by what
    was seen. The section's rule, that a sighting places an animal only to the rank its visible
    traits decide, is read as placement by those traits against the crew's own entries, not as the
    true path cut short, which would make every placement right where convergence misleads a real
    observer; so convergent forms may be lumped until analysis splits or moves them, as a field
    notebook errs, and placement stays a fold of Knowledge. **Open:** how fallible the catalogue
    should be ([Naming, cataloguing and enrichment](#naming-cataloguing-and-enrichment)).
66. **What a crew sees of a survey's completeness, and what a negative survey bounds.** **Lean:** a
    survey shows only what was measured: coverage per layer and instrument, and for each pool the
    crew has sampled an estimate of the taxa not yet seen, from the distribution of its own
    sightings among taxa by ecology's non-parametric estimators (Colwell and Coddington 1994), with
    its uncertainty, never a true total, a percentage complete or a count of unseen siblings in the
    tree. Each instrument class carries a cover detection limit per cell by band and resolution, and
    `NO BIOSIGNATURE ABOVE LIMITS` states the fraction surveyed, the cover limit and the orbital
    phases observed, an unobserved season reading as unobserved, never as absent, since one viewing
    in the wrong season can miss a seasonal signal (Olson et al. 2018). **Open:** the limits
    themselves, the sensors plan's, and whether the estimate is shown at all ([Survey passes and
    coverage](#survey-passes-and-coverage); [A confidence ladder](#a-confidence-ladder); [Naming,
    cataloguing and enrichment](#naming-cataloguing-and-enrichment)).
67. **Discovery: credit, names and worth (owner).** **Lean:** a catalogue entry records the
    observation that first placed it, with its time, ship and station; names a crew gives, if
    players may give them, as the single-player brainstorm's [open question
    9](single-player-experience.md#open-questions) asks, are an overlay on designations in that
    universe's Knowledge, shown beside the designation and never replacing it; and this layer
    supplies measures of a find's novelty as hooks, its rung, whether it is the first of its kind on
    the body or in its region, and its distance in the tree from taxa already catalogued, leaving
    their worth to the campaign. **Open:** whether a name belongs to a player or to the ship,
    whether a universe's catalogue and names can pass to another crew on the same seed, and what a
    discovery is worth, every reference paying for finds in some currency while this document leaves
    trade out of scope ([Naming, cataloguing and enrichment](#naming-cataloguing-and-enrichment);
    [The Knowledge overlay](#the-knowledge-overlay); [Hooks for the layers
    above](#hooks-for-the-layers-above)).
68. **Who the landing party is, and which station operates its instruments (owner).** **Open:**
    whether a landing party is crew that the science station commands as a ship system, as a drone
    is, a player on foot, which needs the on-foot design open questions 13 and 15 wait on, or drones
    alone. **Lean:** commanded from the science station's displays, drones before the crew at an
    uncharacterised site, as COSPAR asks, until an on-foot design exists, so that a station stays a
    set of displays and the authority to command a set of systems and every command is closed-loop
    ([The displays, and who owns them](#the-displays-and-who-owns-them); [Planetary protection as
    procedures](#planetary-protection-as-procedures)).
69. **A landing party's limits beyond the air.** **Open:** three limits the table lacks. An upper
    bound on total pressure for `SHIRTSLEEVE` and `MASK`, from the inert gas's partial pressure, the
    breathing gas's density and the decompression owed on return (NASA-STD-3001 Vol. 2 Rev. E,
    [V2 6008]), with a class above it for a Venus or an abiotic-oxygen world; gravity, for which no
    human standard bounds work on a surface, NASA-STD-3001's acceleration limits being for vehicles;
    and wind and cold across air densities, since the force on a person goes as ½ρv² and heat loss
    depends on the gas. **Lean:** a fourth class, `SEALED SUIT`, for air that cannot be breathed
    through a mask at ambient pressure; a provisional gravity band in `CrewTolerance`, rated beside
    `BREATHABILITY` and used by the human-homeworld filter; wind rated by dynamic pressure against
    an Earth reference and cold by heat loss in the site's own gas; each threshold researched before
    a ruling ([Conditions for a landing party](#conditions-for-a-landing-party); [The crew is
    human](#the-crew-is-human)).
70. **Sampling without landing.** **Lean:** a pass through a plume or an upper atmosphere takes a
    sample whose collection record holds the encounter speed, the encounter speeds that best keep
    its organics being 3–5 km/s for bare amino and fatty acids and 4–6 km/s inside ice grains
    (Jaramillo-Botero et al. 2021), under the flyby's protection category, so that a subsurface
    ocean can reach `Sampled` with no landing party. **Open:** whether such a pass is a playable
    manoeuvre, which is the owner's ([Samples and on-board
    analysis](#samples-and-on-board-analysis); [Geology as places](#geology-as-places)).
71. **Mechanisms the generator cannot make, and when a class earns a biological name.** **Lean:**
    the claim fold applies every mechanism's discriminant, modelled or not, so a rung depends on
    measurements alone and no claim stalls for want of a model; and a surface class keeps its
    observed name (`PIGMENTED CRUST, EDGE 0.70 µm`) until a sample from a cell of that class is
    analysed at level 5 or above, so a world may show a biological name on one shore and an observed
    one on another. The console says of a discriminant only `UNMEASURED` or `NOT MEASURABLE`, which
    a crew can act on, and never that a mechanism is unmodelled, since a model's gap is a fact about
    the generator, not the world, as a speculative setting is under [open question
    73](#open-questions) (a technical ruling of 2026-10-10, over an earlier lean of naming
    unmodelled mechanisms) ([False positives, made by the same
    physics](#false-positives-made-by-the-same-physics); [Detail levels for
    life](#detail-levels-for-life-and-how-they-degrade)).
72. **What time and what knowledge a body map shows.** **Lean:** a quantity layer is a page of
    readings, drawn at the survey's resolution as its readings would quote it, with its uncertainty
    as the second quantity, never the exact cell the client holds, and a class that needs a seasonal
    record, such as Köppen's, is drawn only once the passes span a year, as an inference with its
    rung. Each layer shows its latest pass per cell, the cursor giving that pass's time and the
    legend the oldest pass's age (`OLDEST PASS 214 d`), seasonal layers naming their pass's month;
    and the image draws a surveyed cell as of now, under the rendered image's exception, while its
    cover and productivity are quoted as of the survey, Observed with their epoch, as every older
    measurement is. **Open:** how the climate class is inferred from the ship's own passes, and
    whether drawn phenology may show a season the ship never imaged ([The body map](#the-body-map);
    [Detail levels for life](#detail-levels-for-life-and-how-they-degrade)).
73. **Life's parameters: fixed by the generator version or chosen per universe (owner).** **Lean:**
    life's parameters, λ₀, w_v, the step rates and λ_T beside the speculative `exotic_solvents` and
    `giant_atmosphere_life`, are universe settings within their stated bounds, defaulting to the
    generator's values, recorded in `universe.json` and digested into the `ContentKey`, shown where
    a universe is created and never on a console, since the crew measures its world as it is.
    **Open:** whether the owner wants that choice at all, rather than one set fixed per generator
    version, as the rogue-planet abundance is, or a few named presets. A free setting needs the F_k
    table built per value or at universe creation, sixteen values of λ₀ coming to about 1.8 × 10⁶
    entries and under 16 MB (computed), and under strict hard steps a homeworld search costs some
    150 times the default's ([How common life is](#how-common-life-is-the-rate-of-origin);
    [Solvents](#solvents); [Per large cell](#per-large-cell); [Pinned content](#pinned-content)).
74. **The anoxic biosphere's methane flux before P14.T55.a.** **Lean:** until plan 14's outgassing
    speciation lands, the CH₄ flux is a share of a stand-in reductant supply drawn in Kharecha,
    Kasting and Siefert's (2005) range for an Archean Earth, replaced by T55.a's supply under a
    version bump, as the O₂ stand-ins are. **Open:** whether the stand-in should read the world
    instead, T24.b's present outgassing scaled by T50.a's redox class against the Archean Earth's,
    since T24.b (21 → 22) and T50.a (22 → 23) land before T53.d; or whether request 5's rescheduling
    of T55.a before life makes it unnecessary ([Before
    oxygen](#before-oxygen-the-commonest-living-world); [Suggested order of
    attack](#suggested-order-of-attack)).
75. **Comets and other volatile deposits beyond cold traps.** **Lean:** a comet apparition, which
    plan 14 makes on `system.comet` and identifies by its `EventId`, with no composition, carries
    one drawn on its event stream by reservoir class, Oort or Jupiter-family, its water and CO or
    CO₂ with a dust-to-ice ratio, since a comet's water is its own: 67P's D/H is (5.3 ± 0.7) × 10⁻⁴,
    about three times Earth's (Altwegg et al. 2015). **Open:** whether a comet rendezvous is play at
    all, the single-player brainstorm's later source of ice, which the owner decides ([Small
    bodies](#small-bodies); [Volatiles](#volatiles)).
76. **Whether alteration halos and gossans show in the image.** **Lean:** once a survey records a
    halo or a gossan, the client draws the recorded extent as a material overlay, a view of
    Knowledge, never of the deposit list, since drawing halos as truth would leak deposits' places
    under the truth line, and never drawing them leaves plain ground where the spectrometer flags a
    district. **Open:** whether the image may be a view of Knowledge at all, the truth line's reach
    ([Deposits as places](#deposits-as-places); [Prospecting](#prospecting)).
77. **Knowledge across a generator-version bump.** **Open.** A save of another version refuses to
    open, and plan 04 leaves keeping old generators runnable to the first release, while life's
    Knowledge holds true identities a bump can move or delete: a clade path behind each catalogue
    entry and crew name, a site behind each analysed sample, a draw key behind each depletion, a
    struck animal's key. **Lean:** keep each save's generator runnable rather than migrate, since
    re-resolving an entry needs the old truth, which only the old generator holds; keys made as
    generation keys its subject help only a migration ([What is never stored, and the generator
    version](#what-is-never-stored-and-the-generator-version); the rendering brainstorm's [open
    question 4](rendering-and-planets.md#open-questions)).
78. **How finely the `LifeTable` and drawing records are gated, and what makes a cell living.**
    **Lean:** the table's strategies and covers keep stable indices but travel masked until a
    surveyed cell under imaging uses them, a pool's drawing records go only for taxa whose range
    meets a surveyed cell, and a cell counts as living for the table's first send only where its
    cover exceeds the imaging pass's detection floor at its resolution and bands, so that the
    table's arrival is itself a measurement; the 4-bit cover index and the handles stay stable
    either way. R10's albedo scale in block 0, solved on the substrate against the abiotic twin's
    geometric albedo, carries nothing of life, but beside the living disc's albedo, which R07's disc
    reads, a client holding every cell's class could infer life's cover and the twin's albedo: a
    leak of the exact-gases kind, closed only for an honest client and stated so. **Open:** whether
    to send the scale only with imaging coverage instead ([What reaches the
    wire](#what-reaches-the-wire); [Survey passes and coverage](#survey-passes-and-coverage); [The
    per-cell state](#the-per-cell-state-the-material-and-the-instances)).
79. **Whether a crew may enter a COSPAR Special Region (owner).** **Open:** COSPAR's category IVc
    allows at most 30 spores on whatever touches "a region within which terrestrial organisms are
    likely to replicate", a bound no person can meet, so the protection table lets only sterile
    hardware touch a habitat (COSPAR 2025). Whether a crew may ever enter one, on foot or suited,
    decides whether a landing party can walk where life is likeliest. **Lean:** only after drones
    have characterised the site, as COSPAR's guidance for crews asks of robotic precursors, and only
    as a recorded breach: the entry is a forward-contamination delta that caps later samples there
    at level 1, so the crew may choose and the science bears the cost ([Planetary protection as
    procedures](#planetary-protection-as-procedures)).

## Suggested order of attack

Not a plan, only the dependency order a plan would follow. Steps 1, 2, 4 and 6 are the ones that
retire risk; everything else is additive but step 12's recovery gate, which moves stages under a
version bump. Much of what life reads is unbuilt. Of plan 14's surface section only T24.f is built,
held until its version bump: the 21 → 22 batch builds T24.a–b and T48.a–e with T35.e's wire; the
22 → 23 batch builds Phase L's elements, redox, condensates and crust with T24.c–e and T24.g, the
moons rulings (T56.a, T56.c and T56.d) and P14.T57.a's greenhouse refit on Venus and Earth; the
23 → 24 batch builds T53.a–d, among them T53.a's new CO₂ form, which refits every greenhouse
constant again, and the biosphere hook, with T51.d–f; and T25, T26 and T55 come later. R09's frame
and its survey log (R09.T18) are built, and its coarse steps (T11–T15) are not. Plan 12's store
(P12.T7) is built, and P12.T8, which will hold a universe's Knowledge, is not. The sessions and
sensors plans, steps 1 and 8 of the [single-player
order](single-player-experience.md#suggested-order-of-attack), have not been written. Until a
generator version computes life, a body's readout shows `LIFE: NOT YET MODELLED`, which can ship
first.

Some things are done now rather than in a step. Six cost little today and a version or format bump
later. Plan 14's text amendments (requests 1–3) land before P14.T48.e and T35.e are built in the
21 → 22 batch; the partial-pressure finding (request 10), Dalton's law, before P14.T57.a's fits in
the 22 → 23 batch, so that no fit is redone for it; and the cold traps' input (request 19) before
P14.T51.f and R09.T13.f are built. Base's `rng` scopes, seeds and tags (request 29) land before any
life tag is opened, since `domain_tags!`'s order fixes `tags::ALL`. R09.T18's survey log gains its
instrument classes as format 2 (request 35) before more passes are written. And R09's payload gains
the `LifeTable`'s slot (request 34) before R09.T19's first send. Beside them, the guide's drafts
(requests 44–47), docs only, go to the owner as soon as R10.T13's `±` and `NOT SURVEYED` are signed
off, since step 5's readouts need their forms.

1. **Plan 14's refit.** This layer's terms in P14.T53.a's refit of every greenhouse constant, in the
   23 → 24 batch rather than a third refit after it: the weathering steady state, on a greenhouse
   that saturates at the maximum-greenhouse limit (request 8); the grey τ's temperate CH₄ and N₂O
   terms, fitted offline from published radiative grids as stated exceptions to P14.T57.a's rule of
   fits on measured states, the haze's cooling and the area mix's cover term (request 9), refitted
   on Earth's measured gases and cover given by hand, since T53.d's chemistry, which would compute
   them, is built beside it in the same batch; and researched rulings on the runaway limit
   (request 11) and the pre-main-sequence runaway (requests 12 and 24), the latter before T53.a,
   whose drafted test needs it. It waits on Dalton's law, done now; on plan 14's 22 → 23 batch
   (T50.a, T51.a–c, T24.c with T24.g, T57.a, then T54.a) and T24.b's outgassing; and on T53.a's new
   CO₂ form. It retires the first risk: it moves every temperate world and plan 14's goldens, and it
   decides whether a living Earth and its lifeless twin both come out right, so it is settled before
   anything is calibrated against it.
2. **The history.** `life::system` and `BiosphereHistory`: the gated clocks inverted over plan 14's
   habitability timelines (request 4), every per-epoch draw keyed by window and attempt, the
   `phototrophy` field, the stand-in O₂ delays and the system pass for panspermia, with Solar System
   goldens (Earth, Mars's wet epoch, Europa) and property tests; then the statistical test against
   the table of [Expected statistics at the default](#expected-statistics-at-the-default) and a λ₀
   sweep. It waits on the tags done now, on request 4 and on plan 06's near-ultraviolet history
   (request 23), and its counts on request 16's widening of P14.T25's slow test. It retires the
   owner's risk: [open question 1](#open-questions) becomes measurable on generated bodies rather
   than a Monte Carlo.
3. **Chemistry's envelope and body-level forms.** The growth envelope, the ice-equilibrium water
   activity and the `Biochemistry` record, which wait on nothing but the tags; the PHOENIX
   photon-yield table, offline in `hyperion-fit`, with its blackbody for the hosts PHOENIX does not
   span, which reads the T_eff and log g that `StarState` already gives and waits on R06.T3.a's
   spectra and the stellar libraries' licence ruling (request 25); and the body-level productivity
   and chemotrophy that the snapshot path needs, which wait on the light at the ground of the
   abiotic record (request 31), the reductant supply (request 5) and T24.b's heat flow. Until
   P14.T55.a lands, the anoxic CH₄ flux takes the stand-in supply of [open question
   74](#open-questions).
4. **The forcing and the second pass.** The stage-to-flux mapping and its bands by regime, each flux
   with its seasonal amplitude (request 2); T24.c's ozone table and T53.d's chemistry with its
   lifetimes, its callable O₂ steady state for the stage gates and each gas's seasonal response
   (request 7), whose computed crossings replace step 2's stand-in delays, under a version bump,
   once the reductant supply lands; then the second pass behind a hold constant, Earth pinned with
   `NoLife` and with its forcing given by hand, the pinned living worlds, and the version bump. It
   waits on steps 1–3, on T55.a or the stand-in supply, and on P14.T53.d in plan 14's 23 → 24 batch.
   It retires the architecture's risk, showing whether two passes through one slot give both Earths
   right without iteration.
5. **The thin first slice: a living world's air, measured and claimed.** The retrieval, a pure
   function that can be built against Solar System atmospheres at any time, on a stand-in instrument
   until the sensors plan defines real ones; the observations log on P12.T8 (request 27); the rule
   tables and the claim fold for the commonest detection, CH₄ beside CO₂ about a K dwarf, with
   whatever abiotic sources plan 14 has by then (request 6); `LIFE` with its claim and its limits;
   and R08's sky drawing the gases it draws once `Surface` is granted, the rest withheld behind a
   remainder entry (request 32). It waits on step 4, P12.T8, the owner's sign-off of the guide's
   uncertainty forms, drafted now, and the sessions plan's seed for the measurement's error, or a
   stand-in. It is the smallest thing that makes life visible end to end, from the generator through
   plan 14's air to a console, before any surface is generated.
6. **The biome step.** R09's coarse pass (R09.T11–T15), then `hyperion_surface::life`: `life_at`,
   the strategies with their pathways, their costs and the light shared by height, their scoring,
   the structural labels, microbial siting, ocean biomes, phenology and the `LifeTable` with its two
   constants, with goldens on three targets, the reference-Earth productivity test and the treeline
   and woodland-edge tests that fit the maintenance cost; and coverage per layer gating the
   `LifeTable`, on the survey log's instrument classes (request 35). The pigment model is tested
   alone against the niche battery first, after research advice ([open question
   40](#open-questions)), and P_N waits on its own ruling ([open question 39](#open-questions)). It
   waits on step 4's snapshot cover and productivity, which the two constants realise; on
   R09.T13–T15, R10.T1's classifier order, R09.T19's first send and the light at the ground (request
   31); and on the sensors plan's word on which instrument classes image, or a stand-in. It retires
   the client's risk, whether life computed on both sides fits the client's budget and R03's 15 MiB,
   and microbial covers on surveyed cells are the second slice, life seen from orbit.
7. **Life drawn.** R10's cover term and second index space, the cover channel in the bake and in
   R07's disc map, R10.T10.a's albedo scale solved on the substrate against the abiotic twin's
   geometric albedo (request 38), and the canopy's hotspot row (request 39); then R11's flora layer
   and the ocean's colour (requests 40–41). It waits on step 6, R10.T1.c, T2 and T10, R11's rock
   tasks and R11.T8.
8. **The content passes.** The process table as data with its sources, research that can start at
   once, and the analogue compositions for P14.T51.e's assemblages, which are independent of life;
   then ground ice and ³He by cell (R09.T13, P14.T51.d and request 25's wind); the deposit pass
   (R09.T10.c–T15, P14.T51.c and T50.a through T54.a, request 17 and step 2's stage at age); and the
   landmark lists (request 36's provinces, sedimentary cover and marks, and P14.T17's tidal heating
   as P14.T56.d amends it), the geysers on one eruptive clock a body until request 22's event
   subject exists. The local synthesis's height terms follow later, each a version bump with goldens
   on three targets.
9. **Organisms.** Clade keys and the tree, on step 2's epochs and the scopes done now, with the
   designations' permutation keyed once per body; microbial pools, communities and samples, the
   common case first; the physical caps; richness, realms and macroscopic pools on R09's field and
   step 6's biomes; densities; then instantiation, movement and the drawing-record bulk kind
   (request 30), with the truth-and-decoration integration through R11's contact query. It waits on
   steps 2 and 6, R03's binary frames, plan 04's table of kinds and R11's scatter machinery; flight
   records wait on the sessions plan.
10. **Knowledge of life, and the landing party.** The samples log, with samples and analyses on the
    sample function of [Organisms](#organisms) and the `Biochemistry` record; `CrewTolerance` and
    `BREATHABILITY`, which need only a measured atmosphere and can come as early as step 5;
    planetary protection's categories and procedures; prospecting on the JORC ladder, from an
    Exploration Target to `MEASURED`, and the catalogue log. It waits on the sensors plan's
    instruments and body-level overlay (request 43), the sessions plan's closed-loop commands
    (request 42) and, for prospecting, step 8's deposits.
11. **The consoles.** The class tokens, after the console-ux skill gains its dichromacy check
    (request 47); then `SURVEY` (R09's field on the wire and R10's coverage, in P14.T46.b's built
    frame, a rogue planet waiting on request 21's remaining frame), `ANALYSIS` (sessions),
    `PROSPECT` (step 8) and `CATALOGUE` (the sensors plan's `SENSORS` log). `LIFE` arrives in
    step 5.
12. **Deltas, pins, hooks and enrichment.** Contamination, struck animals and depletion as deltas,
    pinned selections and overrides under the `ContentKey`, and flight records, once the sessions
    plan has a session seed and `session.json`; the extinction pulses and the recovery gate, once
    plans 02 and 09 give the explosions' rates (request 26), the recovery gate moving stages, and so
    step 2's goldens, under a version bump; the offline F_k table and the per-cell density on D17's
    quadrature, then the homeworld search the civilisations brainstorm calls; and enrichment last,
    on the catalogue and the model's place in the server. R09.T19.c's socket test grows with each
    item that reaches the wire.

The end state is this document's goal: every body's life and content a pure function of the seed,
its ID and the time, which the crew comes to know only by measuring it, and on which the next
brainstorm's species can arise.

## Sources

Every work cited above is listed once, under the topic where it does most of its work, with the
figure or claim it supports here, and within each topic broadly in the order of its first citation.
Works with two or three authors are cited by all their names and works with four or more by the
first and "et al.", so that "Stevenson et al. 2015" and "Stevenson, Lunine and Clancy 2015" are
different papers; letters separate only works whose citations would otherwise coincide; and each
work takes its print year. The research was done on 2026-10-10, its notes holding the full
references, and checked the same day against primary sources wherever those could be reached, and
again in a second review. Each entry takes the stronger of the two readings, and follows the check
wherever it corrected an attribution. "Verified in the source's own text" means that the figure was
read in the source itself, and the entry says where (abstract, body, table) when that matters;
"verified via a secondary source" means it was read only where another work quotes it, and names
that work. **Not re-checked** marks a citation whose existence is confirmed but whose figure was
taken from a citing document, or not read at all. **From memory** marks a citation recalled rather
than read: its details, and the figure it supports, must be checked before either becomes code.
"Unreachable" says what was tried. Everything here should be re-checked against these sources when
it becomes code, in the same way the galaxy and rendering brainstorms' figures are. Volumes, pages
and DOIs missing from the notes were completed from Crossref on 2026-10-10, which confirms a
reference, not the figure it supports. Games, wikis and other community sources stand for what
developers and players report, not for measurement, and community pages are marked as such. The list
holds 507 works: 470 verified in the source's own text and 26 via a secondary source; 8 marked **not
re-checked** and 4 **from memory**; and 17 whose source was unreachable in the research or a check.
An entry checked in parts counts under each status it carries.

**Games and references**

- Duncan 2015, _No Man's Sky: How I Learned to Love Procedural Art_, GDC 2015,
  <https://www.youtube.com/watch?v=vcEA41eBOGs>: creatures as artists' templates over few skeletons,
  with gait following proportion (the "lumber" quotation); verified in the source's own text (the
  talk's auto-captions, read in the research, corroborated in _Game Informer_ of 24 December 2014);
  the check could not reach the captions.
- Murray 2016, interview in _PC Gamer_, 3 March 2016,
  <https://www.pcgamer.com/no-mans-sky-interview-sean-murray/>: one biome per world by design ("it
  wouldn't make you want to go and visit other planets"); verified in the source's own text.
- Maiberg 2016, _No Man's Sky_ review, _Vice_, 21 August 2016,
  <https://www.vice.com/en/article/no-mans-sky-review/>: Compton's "10,000 Bowls of Oatmeal
  problem", variety that is combinatorial but not perceived; verified in the source's own text.
- Hello Games 2024 and 2025, _Worlds Part I_ and _Worlds Part II_ release notes,
  <https://www.nomanssky.com/worlds-part-i-update/> and
  <https://www.nomanssky.com/worlds-part-ii-update/>: that the _Worlds_ updates kept one biome per
  world, neither describing a world with several; verified in the source's own text.
- No Man's Sky community wiki, nomanssky.fandom.com, pages "Biome", "Planet", "Planet - Discovery
  Menu", "Discoveries" and "Fauna": a world's fauna counted against their total and its flora and
  minerals only as found, resources keyed to biome, and diet and temperament as labels; verified in
  the source's own text (community-sourced).
- A Steam community post of 15 August 2016 on No Man's Sky: life on nearly every world, "by players'
  account"; **not re-checked** (community-sourced; no press source was found, and the notes give no
  URL).
- Frontier Developments (W. Flanagan) 2018, _Content Recap: Beyond - Chapter Four Livestream -
  Exploration and the Codex_, forum post of 18 October 2018,
  <https://forums.frontier.co.uk/threads/content-recap-beyond-chapter-four-livestream-exploration-and-the-codex.451420/>:
  the Codex by galactic region, with `Rumoured`, `Reported` and `Discovered` states, one commander's
  report updating everyone's Codex, and the first discoverer's and mapper's names on bodies, the
  post placing the Codex, the system scan and probe mapping in _Beyond_'s Chapter Four; verified in
  the source's own text (in the second review through a Wayback copy of 21 October 2025). A second
  post, of 1 November 2018, read through Wayback copies, covers ring hotspots and the assay of each
  rock.
- Elite Dangerous community wiki, elite-dangerous.fandom.com, pages "Genetic Sampler",
  "Exobiologist", "Vista Genomics", "Planets" and the genus pages, read through its API: some twenty
  sampleable genera, fifteen of them from _Odyssey_; many only at or below 0.27 g; clonal colony
  ranges of 100–1,000 m; three samples to a species; colour variants by star class or rare material;
  and life on unlandable worlds only as a planet class, Earth-like, water and ammonia worlds and gas
  giants with water- or ammonia-based life; verified in the source's own text (community-derived
  from play; the Aleoida pages conflict, 150 m against 200 m).
- IGN 2023, _Starfield_ review, <https://www.ign.com/articles/starfield-review>: worlds "fairly
  barren and lifeless", as "planets that're mostly uninhabitable" would be, with little to do but
  "scan rocks or zap them with a mining laser"; verified in the source's own text.
- Starfield community wikis, starfieldwiki.net (UESP) and starfield.fandom.com, pages "Planets",
  "Planetary Traits", "Biomes", "Jemison" and "Planet Surveying": twenty-two biome kinds drawn into
  some 430 variants, the four survey kinds, the named traits and their categories, mineral resources
  from orbital scan and Jemison's biome shares; verified in the source's own text
  (community-sourced).
- SpaceEngine, manual, "Creating a planet",
  <https://spaceengine.org/manual/making-addons/creating-a-planet/>: one or two life records per
  world, "each describing a different class of life", each organic or exotic, unicellular or
  multicellular, and subglacial, marine, terrestrial or aerial, and a `Panspermia` flag; verified in
  the source's own text.
- SpaceEngine 2018, developer blog of 10 December 2018, <https://spaceengine.org/news/blog181210/>:
  life seeded after the planetary system from surface volatiles, allowed to change the atmosphere,
  and wiped out or moved if the climate collapses; verified in the source's own text.
- SpaceEngine 2017, developer blog of 26 September 2017, <https://spaceengine.org/news/blog170926/>:
  Earth's oxygen timetable on every world, no oxygen before 2.5 Gyr and the Great Oxygenation Event
  after it; verified in the source's own text.
- The Ur-Quan Masters, Star Control II's open-source port: its v0.7 user manual and source files
  `planets.h`, `elemdata.h`, `scan.c` and `lander.c`, read from the MegaMod fork,
  <https://raw.githubusercontent.com/Serosis/UQM-MegaMod/master/doc/users/manual.txt>: the three
  scans, weather and tectonics classes 1 to 8 with temperature in °C, hazard chances per class
  rolled each tick, and eight mineral tiers from `Common` to `Exotic`; verified in the source's own
  text (in the research and the check).
- Ultronomicon, the Ur-Quan Masters community wiki, <https://wiki.uqm.stack.nl/Planet_Lander>: the
  lander's finite cargo, crew and room for biological data; verified in the source's own text
  (community-sourced).
- Subnautica community wiki, subnautica.fandom.com, pages "Biomes", "Crash Zone", "Stalker", "Reaper
  Leviathan", "Ampeel", "Update History" and "Solar Panel": the Crash Zone from the surface to about
  550 m (its own page gives 0–560 m), fauna that respawn when eaten or killed, leviathans and
  Ampeels apart, and solar power falling with depth by biome; verified in the source's own text
  (community-sourced).
- Kerbal Space Program official wiki, wiki.kerbalspaceprogram.com, pages "Science" and "Biome",
  <https://wiki.kerbalspaceprogram.com/wiki/Science>: experiment results keyed by six situations
  and, for many, by biome, with diminishing returns on repetition; verified in the source's own text
  in the research (community-sourced), the check finding the wiki behind a bot wall.
- Adams 2008, _Interview: The Making of Dwarf Fortress_, by John Harris, Gamasutra (now _Game
  Developer_), 27 February 2008,
  <https://www.gamedeveloper.com/design/interview-the-making-of-dwarf-fortress>: biomes as
  thresholds on temperature, rainfall, elevation, drainage and salinity, and plants and animals
  placed "strictly by biome type", a limitation Tarn Adams wanted to replace with geographic ranges;
  verified in the source's own text.
- Hecker et al. 2008, _Real-time motion retargeting to highly varied user-created morphologies_, ACM
  Transactions on Graphics 27, doi:10.1145/1360612.1360626: Spore's motion authored once and applied
  to skeletons "that did not exist when the animation was authored"; verified in the source's own
  text (abstract).
- Bohannon 2008, _Flunking Spore_, Science 322, 531, doi:10.1126/science.322.5901.531b: the review
  that failed Spore's mutation, selection, genetics and drift; verified in the source's own text (in
  the research; the check saw only the metadata, the text being paywalled).
- Dixon 1981, _After Man: A Zoology of the Future_: animals 50 Myr from now descended from the
  survivors of a mass extinction just after our own time; verified via a secondary source
  (Wikipedia; the Guardian and the Verge could not be fetched).
- Barlowe 1990, _Expedition: Being an Account in Words and Artwork of the 2358 A.D. Voyage to Darwin
  IV_, Workman: no animal of Darwin IV has eyes, hair or external ears, by the author's own ground
  rule; verified in the source's own text (the author's page,
  <https://waynebarlowe.com/artwork/expedition/>: "There was to be no hair, no eyes or external
  ears"), with the year and publisher only from Wikipedia's infobox.
- Kösemen, _Snaiad_, FAQ, <http://www.cmkosemen.com/snaiad_web/sndfaq.html>: a world begun from "a
  common body plan" "evolved … in successive variations"; verified in the source's own text.
- Bajda, _Serina_, <https://sites.google.com/site/worldofserina/home>: a terraformed moon seeded
  with a handful of Earth species (among them grasses, sunflowers, ants, crickets, guppies and the
  canary, its only land vertebrate), radiating along a dated timeline with cladograms; verified in
  the source's own text (the home page, and the epoch and cladogram pages).
- van Dijk 2010, _Size matters, but so does gravity II_, Furahan Biology and Allied Matters,
  <https://planetfuraha.blogspot.com/2010/07/size-matters-but-so-does-gravity-ii.html>: Furaha's
  limbs worked out under gravity; verified in the source's own text (the post of 18 July 2010, in
  the second review; no figure is taken from it).

**The pipeline and planetary inputs**

- Rugheimer, Segura, Kaltenegger and Sasselov 2015, _UV surface environment of Earth-like planets
  orbiting FGKM stars through geological evolution_, ApJ 806, 137, doi:10.1088/0004-637X/806/1/137:
  a prebiotic Earth's DNA-weighted surface dose from 3,520 times modern Earth's about an F0V star to
  about twice it about an M dwarf, and, beside the human ultraviolet limit, the anoxic 3.9 Ga
  Earth's 611 times today's under the early Sun, 3,520 under an F0 star and 2.02 under GJ 581 (Table
  3); verified in the source's own text (abstract; Table 3 in the check).
- Goldblatt, Lenton and Watson 2006, _Bistability of atmospheric oxygen and the Great Oxidation_,
  Nature 443, 683, doi:10.1038/nature05169: O₂'s two stable states, below 10⁻⁵ and above 5 × 10⁻³
  PAL, which make a coupled fixed point non-unique and the rise of oxygen fast; verified in the
  source's own text (abstract).
- Catling 2014, _The Great Oxidation Event transition_, Treatise on Geochemistry, 2nd edn, 177–195,
  doi:10.1016/B978-0-08-095975-7.01307-3: Earth's surface O₂ sink of 2.21 × 10¹³ mol yr⁻¹, behind
  O₂'s 1.7 Myr residence; verified via a secondary source (Luger and Barnes 2015, §2.5.1, which plan
  14 follows), the 1.7 Myr computed from Earth's 3.7 × 10¹⁹ mol of O₂.
- Gibling and Davies 2012, _Palaeozoic landscapes shaped by plant evolution_, Nature Geoscience 5,
  99, doi:10.1038/ngeo1376: that plants "irrevocably altered the shape and form of fluvial systems"
  in the Palaeozoic, the effect R09's erosion leaves out; verified in the source's own text
  (abstract, through nature.com, in the second review).

**Origins and evolutionary transitions**

- Carter 1983, _The anthropic principle and its implications for biological evolution_,
  Philosophical Transactions of the Royal Society A 310, 347, doi:10.1098/rsta.1983.0096: the
  hard-steps argument from the near-coincidence of the time intelligence took on Earth and the Sun's
  lifetime, with memoryless waits, and "at least one but probably not more than two links that were
  highly improbable"; verified in the source's own text (abstract, through OpenAlex, in the second
  review).
- Watson 2008, _Implications of an anthropic model of evolution for emergence of complex life and
  intelligence_, Astrobiology 8, 175, doi:10.1089/ast.2006.0115: fewer than 10 hard steps, best
  guess 4, falling evenly through Earth's history whatever their rates; verified in the source's own
  text.
- Wilde, Valley, Peck and Graham 2001, _Evidence from detrital zircons for the existence of
  continental crust and oceans on the Earth 4.4 Gyr ago_, Nature 409, 175, doi:10.1038/35051550:
  liquid water by 4.40 Ga, from a zircon of 4,404 ± 8 Myr; verified in the source's own text
  (abstract).
- Allwood et al. 2006, _Stromatolite reef from the Early Archaean era of Australia_, Nature 441,
  714, doi:10.1038/nature04764: life by 3.43 Ga in widely accepted stromatolites, "of probable
  biological origin", a reef of seven morphotypes over a 3,430 Myr old peritidal carbonate platform,
  for the microbial reef and the microbial expressions; verified in the source's own text (abstract;
  biogenicity debated).
- Tashiro et al. 2017, _Early trace of life from 3.95 Ga sedimentary rocks in Labrador, Canada_,
  Nature 549, 516, doi:10.1038/nature24019: life probably before 3.95 Ga; verified in the source's
  own text.
- Bell, Boehnke, Harrison and Mao 2015, _Potentially biogenic carbon preserved in a 4.1
  billion-year-old zircon_, PNAS 112, 14518, doi:10.1073/pnas.1517557112: possible life by 4.10 Ga;
  verified in the source's own text.
- Moody et al. 2024, _The nature of the last universal common ancestor and its impact on the early
  Earth system_, Nature Ecology and Evolution 8, 1654, doi:10.1038/s41559-024-02461-1: LUCA at about
  4.2 Ga (4.09–4.33 Ga), already a prokaryote-grade anaerobic acetogen in an ecosystem; verified in
  the source's own text.
- Spiegel and Turner 2012, _Bayesian analysis of the astrobiological implications of life's early
  emergence on Earth_, PNAS 109, 395, doi:10.1073/pnas.1111694108: Earth's early start "consistent
  with an arbitrarily low intrinsic probability of abiogenesis"; about 12% of the posterior below 1
  per Gyr in their Optimistic model, under a prior uniform in log λ over 10⁻³–10³ per Gyr; their
  lower limits, 10⁻³ per Gyr "once per 200 stars" and 10⁻¹¹ "once in our galaxy"; and an independent
  origin as much stronger evidence; verified in the source's own text (full text; the lower limits'
  wording in the check).
- Kipping 2020, _An objective Bayesian analysis of life's early start and our late arrival_, PNAS
  117, 11995, doi:10.1073/pnas.1921655117: Bayes factors of at least 2.8 and 8.7 for a rapid origin
  on the 3.465 Ga microfossils and the 4.1 Ga zircon, reversed had first life been younger than 680
  Ma, and 3:2 odds that intelligence is rare; verified in the source's own text (full text).
- Scharf and Cronin 2016, _Quantifying the origins of life on a planetary scale_, PNAS 113, 8127,
  doi:10.1073/pnas.1523233113: "a simple, heuristic formula" for the number of origins, whose
  parameters include the building blocks available and "the effective chemical search space", with
  no measured values; verified in the source's own text (abstract, arXiv:1511.02549, in the second
  review).
- Sleep, Zahnle, Kasting and Morowitz 1989, _Annihilation of ecosystems by large asteroid impacts on
  the early Earth_, Nature 342, 139, doi:10.1038/342139a0: ecosystems independent of photosynthesis
  possible from about 4.44 Ga, and photic-zone impacts over after about 3.8 Ga; verified in the
  source's own text.
- Abramov and Mojzsis 2009, _Microbial habitability of the Hadean Earth during the late heavy
  bombardment_, Nature 459, 419, doi:10.1038/nature08015: no plausible bombardment fully sterilised
  Earth's habitable zone after the Moon-forming impact; verified in the source's own text.
- Patel et al. 2015, _Common origins of RNA, protein and lipid precursors in a cyanosulfidic
  protometabolism_, Nature Chemistry 7, 301, doi:10.1038/nchem.2202: precursors of nucleotides,
  amino acids and lipids in steps driven by ultraviolet; verified in the source's own text.
- Rimmer et al. 2018, _The origin of RNA precursors on exoplanets_, Science Advances 4, eaar3302,
  doi:10.1126/sciadv.aar3302: enough near-ultraviolet from hosts hotter than K5 (4,400 K), and from
  cooler ones only if they flare often enough, as "~30% of stars cooler than K5, and ~20% of the
  early M dwarfs" do; and the threshold of (6.8 ± 3.6) × 10⁹ photons cm⁻² s⁻¹ Å⁻¹ at the surface
  over 200–280 nm (its eq. 33), the surface origin's light gate; verified in the source's own text
  (full text, through Europe PMC, PMC6070314).
- Damer and Deamer 2020, _The hot spring hypothesis for an origin of life_, Astrobiology 20, 429,
  doi:10.1089/ast.2019.2045: wet–dry cycling in hot-spring pools on land assembling protocells;
  verified in the source's own text.
- Weiss et al. 2016, _The physiology and habitat of the last universal common ancestor_, Nature
  Microbiology 1, 16116, doi:10.1038/nmicrobiol.2016.116: LUCA anaerobic, H₂-dependent and
  thermophilic, supporting "an autotrophic origin of life … in a hydrothermal setting"; verified in
  the source's own text.
- Snyder-Beattie, Sandberg, Drexler and Bonsall 2021, _The timing of evolutionary transitions
  suggests intelligent life is rare_, Astrobiology 21, 265, doi:10.1089/ast.2019.2149: with
  log-uniform priors, expected transition times "likely exceed the lifetime of Earth, perhaps by
  many orders of magnitude"; verified in the source's own text (full text).
- Knoll 2011, _The multiple origins of complex multicellularity_, Annual Review of Earth and
  Planetary Sciences 39, 217, doi:10.1146/annurev.earth.031208.100209: simple multicellularity
  "numerous times within the Eukarya", complex multicellularity in six clades, for the stage list
  and the tree's kingdoms; verified in the source's own text (abstract, through Crossref).
- Bains and Schulze-Makuch 2016, _The cosmic zoo: the (near) inevitability of the evolution of
  complex, macroscopic life_, Life 6, 25, doi:10.3390/life6030025: oxygenic photosynthesis and the
  eukaryotic cell arising once, perhaps by inventors that "pull[ed] up the ladder after themselves";
  intelligence many times; the origin of life, and possibly oxygenesis and technological
  intelligence, as the steps it cannot class as likely; eukaryotes by 1.9–1.7 Ga; recovery from mass
  extinctions in 5–20 Myr; phototrophs through the Palaeoproterozoic snowball; verified in the
  source's own text (full text; wording corrected in the check).
- Mills, Macalady, Frank and Wright 2025, _A reassessment of the "hard-steps" model for the
  evolution of intelligent life_, Science Advances 11, eads5698, doi:10.1126/sciadv.ads5698: no hard
  steps, the pace set by "the sequential opening of new global environmental windows of
  habitability"; O₂ thresholds (eukaryogenesis 0.001–0.4% PAL, animals about 0.1–1%, a
  post-oxidation floor usually 1–10% and possibly 0.05–1%, near-present levels by 420–400 Ma,
  sustained open-air combustion at least 87.8%, long-term human habitation 53–59%), the 87.8% from
  Balbi and Frank 2024; a remaining biosphere life of 1.0–1.9 Gyr; and Table 1's eight hard-step
  studies, all naming our own species; verified in the source's own text (full text), though its O₂
  figures summarise primary sources that were **not re-checked**.
- Crowe et al. 2013, _Atmospheric oxygenation three billion years ago_, Nature 501, 535,
  doi:10.1038/nature12426: O₂ of at least 3 × 10⁻⁴ PAL by about 3 Ga; verified in the source's own
  text.
- Albut et al. 2018, _Modern rather than Mesoarchaean oxidative weathering responsible for the heavy
  stable Cr isotopic signatures of the 2.95 Ga old Ijzermijn iron formation (South Africa)_,
  Geochimica et Cosmochimica Acta 228, 157–189, doi:10.1016/j.gca.2018.02.034: the dispute of Crowe
  et al.'s 3 Ga oxygen; **not re-checked** (title only, in the check).
- Gumsley et al. 2017, _Timing and tempo of the Great Oxidation Event_, PNAS 114, 1811,
  doi:10.1073/pnas.1608824114: oxygenation's onset at 2,460–2,426 Ma; verified in the source's own
  text.
- Miao, Yin, Knoll, Qu and Zhu 2024, _1.63-billion-year-old multicellular eukaryotes from the
  Chuanlinggou Formation in North China_, Science Advances 10, eadk3208, doi:10.1126/sciadv.adk3208:
  multicellular filaments at 1,635 Ma; verified in the source's own text.
- Bobrovskiy et al. 2018, _Ancient steroids establish the Ediacaran fossil Dickinsonia as one of the
  earliest animals_, Science 361, 1246, doi:10.1126/science.aat7228: the Ediacara biota of
  571–541 Ma, with _Dickinsonia_ among its animals; verified in the source's own text.
- Wellman, Osterloff and Mohiuddin 2003, _Fragments of the earliest land plants_, Nature 425, 282,
  doi:10.1038/nature01884: land-plant spores by 475 Ma; verified in the source's own text.
- Delwiche and Cooper 2015, _The evolutionary origin of a terrestrial flora_, Current Biology 25,
  R899, doi:10.1016/j.cub.2015.08.029: land plants "a monophyletic lineage", so plants came ashore
  once; verified in the source's own text (abstract, through Europe PMC, in the second review).
- Lozano-Fernandez et al. 2016, _A molecular palaeobiological exploration of arthropod
  terrestrialization_, Philosophical Transactions of the Royal Society B 371, 20150133,
  doi:10.1098/rstb.2015.0133: "the three independent, Palaeozoic arthropod terrestrialization events
  (those of Myriapoda, Hexapoda and Arachnida)", so animals came ashore several times; verified in
  the source's own text (abstract, through OpenAlex, in the second review).
- Segura et al. 2003, _Ozone concentrations and ultraviolet fluxes on Earth-like planets around
  other stars_, Astrobiology 3, 689, doi:10.1089/153110703322736024: high surface ultraviolet below
  about 10⁻² PAL of O₂, the land gate; an F2V planet with high O₂ shielded better than Earth; and
  the one-dimensional columns, 266 DU at 0.1 PAL and about 124 at 0.01, with the stellar factor kept
  from it; verified in the source's own text (abstract), its columns via a secondary source (Cooke
  et al. 2022).
- Catling, Glein, Zahnle and McKay 2005, _Why O₂ is required by complex life on habitable planets
  and the concept of planetary "oxygenation time"_, Astrobiology 5, 415, doi:10.1089/ast.2005.5.415:
  P_O₂ above about 10³ Pa for diffusion-reliant organisms of a millimetre and 10³–10⁴ Pa beyond a
  centimetre with circulation, which set p_cx and the oxygen row of the body-plan table; an
  "oxygenation time" defined at 10⁴ Pa; complex life precluded about short-lived stars; verified in
  the source's own text (abstract and full text).
- Caldeira and Kasting 1992, _The life span of the biosphere revisited_, Nature 360, 721,
  doi:10.1038/360721a0: a C4-plant biosphere lasting 0.9–1.5 Gyr more, 150 ppm of CO₂ as "the
  critical level for C3 photosynthesis" and a C4 floor of 10 ppm, the ends of the productivity's
  C3-like and C4-like CO₂ ramps; verified in the source's own text (abstract).
- Ozaki and Reinhard 2021, _The future lifespan of Earth's oxygenated atmosphere_, Nature Geoscience
  14, 138, doi:10.1038/s41561-021-00693-5: O₂ above 1% PAL for 1.08 ± 0.14 Gyr more, falling before
  the moist greenhouse, which calibrates S_deox, a fixed effective flux, at about 1.11 S⊕ on plan
  06's 1 M☉ track (computed in the second review); verified in the source's own text.
- O'Malley-James, Greaves, Raven and Cockell 2013, _Swansong biospheres: refuges for life and novel
  microbial biospheres on terrestrial planets near the end of their habitable lifetimes_,
  International Journal of Astrobiology 12, 99, doi:10.1017/S147355041200047X: microbial refuges up
  to 2.8 Gyr hence; verified in the source's own text.
- Kopparapu et al. 2013, _Habitable zones around main-sequence stars: new estimates_, ApJ 765, 131,
  doi:10.1088/0004-637X/765/2/131, and its erratum, ApJ 770, 82, doi:10.1088/0004-637X/770/1/82: the
  one-dimensional, cloud-free runaway limit S_eff = 1.0385 that plan 14 uses; verified in the
  source's own text (the erratum's Table 3, in the check; the research took it from the project's
  `habitable_zone.rs`).
- Kopparapu et al. 2014, _Habitable zones around main-sequence stars: dependence on planetary mass_,
  ApJL 787, L29, doi:10.1088/2041-8205/787/2/L29: that the one-dimensional model puts Earth "right
  on this limit" for want of humidity and clouds, the Leconte-scaled S_eff☉ = 1.107 for 1 M⊕ asked
  of plan 14, with 0.99 at 0.1 M⊕ and 1.188 at 5 M⊕ (Table 1), and the runaway taken as the inner
  edge in place of the moist greenhouse, "Both these limits occur in our model at S_eff values
  within 2% of each other"; verified in the source's own text (in the check, and the full text,
  arXiv:1404.5292, in the second review).
- Leconte et al. 2013, _Increased insolation threshold for runaway greenhouse processes on
  Earth-like planets_, Nature 504, 268, doi:10.1038/nature12827: a three-dimensional runaway
  threshold near 375 W/m², about 1.10 S⊕; verified in the source's own text.
- Wolf and Toon 2015, _The evolution of habitable climates under the brightening Sun_, JGR
  Atmospheres 120, 5775, doi:10.1002/2015JD023302: Earth "stable against a thermal runaway up to at
  least +21% S0", though then above 360 K, and losing significant water only from +19%, the less
  conservative runaway bound; verified in the source's own text (abstract, through OpenAlex and
  Crossref, in the second review).
- Luger and Barnes 2015, _Extreme water loss and abiotic O₂ buildup on planets throughout the
  habitable zones of M dwarfs_, Astrobiology 15, 119, doi:10.1089/ast.2014.1231: several hundred Myr
  of pre-main-sequence runaway about M dwarfs with oceans lost, and the abiotic O₂ it leaves,
  "hundreds to thousands of bar" at about 5 bar/Myr on an Earth-mass planet and up to 25 bar/Myr on
  super-Earths, which plan 14 already models; verified in the source's own text (abstract).
- Lingam and Loeb 2019b, _Role of stellar physics in regulating the critical steps for life_,
  International Journal of Astrobiology 18, 527, doi:10.1017/S1473550419000016: the critical-step
  model's suppression of M-dwarf habitability, and K dwarfs as where most microbial biosignatures
  are expected; verified in the source's own text.
- Sato et al. 2014, _Habitability around F-type stars_, International Journal of Astrobiology 13,
  244, doi:10.1017/S1473550414000020: F-star ultraviolet doing 2.5–7.1 times Earth's DNA damage;
  verified in the source's own text.
- Ramirez and Kaltenegger 2016, _Habitable zones of post-main sequence stars_, ApJ 823, 6,
  doi:10.3847/0004-637X/823/1/6: a planet staying 200 Myr to 9 Gyr in the outward-moving habitable
  zone after the main sequence; verified in the source's own text.
- Agol 2011, _Transit surveys for Earths in the habitable zones of white dwarfs_, ApJL 731, L31,
  doi:10.1088/2041-8205/731/2/L31: a white dwarf's habitable zone at 0.005–0.02 au lasting at least
  3 Gyr; verified in the source's own text.
- Barnes and Heller 2013, _Habitable planets around white and brown dwarfs: the perils of a cooling
  primary_, Astrobiology 13, 279, doi:10.1089/ast.2012.0867: planets of both kinds "at risk of never
  achieving habitable conditions", through water lost before the cooling habitable zone reaches
  them; verified in the source's own text (abstract, arXiv:1211.6467, in the second review).
- Bolmont, Raymond and Leconte 2011, _Tidal evolution of planets around brown dwarfs_, A&A 535, A94,
  doi:10.1051/0004-6361/201117734: planets that "can survive in the habitable zone for Gyr
  timescales", though in many cases the zone moves inward past them "in just tens to hundreds of
  Myr"; verified in the source's own text (abstract, arXiv:1109.2906, in the second review).
- Mileikowsky et al. 2000, _Natural transfer of viable microbes in space: 1. From Mars to Earth and
  Earth to Mars_, Icarus 145, 391, doi:10.1006/icar.1999.6317: transfer of microbes from Mars to
  Earth "not only possible but also highly probable"; verified in the source's own text.
- Worth, Sigurdsson and House 2013, _Seeding life on the moons of the outer planets via
  lithopanspermia_, Astrobiology 13, 1155, doi:10.1089/ast.2013.1028: transfer to the moons of
  Jupiter and Saturn possible but rare, and a planet's own ejecta re-impacting "in the first million
  years after ejection", which "could re-seed life" after sterilisation and which this layer leaves
  out; verified in the source's own text (abstract), the authors from Crossref and Semantic Scholar.
- Lingam and Loeb 2017, _Enhanced interplanetary panspermia in the TRAPPIST-1 system_, PNAS 114,
  6689, doi:10.1073/pnas.1703517114: transfer in a compact system potentially orders of magnitude
  more likely than from Earth to Mars; verified in the source's own text.
- Hohmann-Marriott and Blankenship 2011, _Evolution of photosynthesis_, Annual Review of Plant
  Biology 62, 515, doi:10.1146/annurev-arplant-042110-103811: "the earliest forms of photosynthetic
  life were almost certainly anoxygenic"; verified in the source's own text (abstract, through
  Europe PMC, in the second review).
- Tice and Lowe 2004, _Photosynthetic microbial mats in the 3,416-Myr-old ocean_, Nature 431, 549,
  doi:10.1038/nature02888: photosynthetic mats by 3,416 Ma; verified in the source's own text
  (abstract, in the second review).
- Lingam and Loeb 2019c, _Subsurface exolife_, International Journal of Astrobiology 18, 112,
  doi:10.1017/S1473550418000083: worlds with buried oceans possibly outnumbering rocky
  habitable-zone planets by a few orders of magnitude; verified in the source's own text.
- Balbi and Frank 2024, _The oxygen bottleneck for technospheres_, Nature Astronomy 8, 39 (online
  2023), doi:10.1038/s41550-023-02112-8: open-air combustion, "needed, for example, for metallurgy",
  possible only where O₂ is at least 18% of an Earth-like air, the 87.8% PAL that Mills et al. 2025
  quote, the fire gate for technology; verified in the source's own text (abstract,
  arXiv:2308.01160, in the second review), the 87.8% via a secondary source (Mills et al. 2025, its
  ref. 126).

**Sterilisation and hazards from the Galaxy and the host**

- Thomas et al. 2016, _Terrestrial effects of nearby supernovae in the early Pleistocene_, ApJL 826,
  L3, doi:10.3847/2041-8205/826/1/L3: a supernova's cosmic rays arriving after its light, "a
  significant prompt flux at 500 yr" at 100 pc, with tropospheric ionisation near ten times today's
  "for at least 1000 yr" and tapering "by 5 kyr", so a pulse of 10³–10⁴ years; verified in the
  source's own text (§3.2 and §4 of arXiv:1605.04926, in the second review).
- Melott, Thomas, Kachelrieß, Semikoz and Overholt 2017, _A supernova at 50 pc: effects on the
  Earth's atmosphere and biota_, ApJ 840, 105, doi:10.3847/1538-4357/aa6c57: at 50 pc the cosmic
  rays, ionisation and muon dose that "All return to pre-burst levels within 10kyr"; verified in the
  source's own text (abstract, arXiv:1702.04365, in the second review).
- Gehrels et al. 2003, _Ozone depletion from nearby supernovae_, ApJ 585, 1169, doi:10.1086/346127:
  a supernova within about 8 pc roughly doubling biologically active surface ultraviolet, some 1.5
  times per Gyr at the Sun; verified in the source's own text.
- Melott and Thomas 2011, _Astrophysical ionizing radiation and Earth: a brief review and census of
  intermittent intense sources_, Astrobiology 11, 343, doi:10.1089/ast.2010.0603: supernovae within
  D below 100 pc at about 2 per Myr × (D ÷ 100 pc)³, an extinction-level distance of 8–10 pc (one or
  two per Gyr), a long gamma-ray burst's of about 2 kpc, and 100 kJ/m² removing some 30% of the
  ozone; verified in the source's own text (full text).
- Brunton et al. 2023, _X-ray luminous supernovae: threats to terrestrial biospheres_, ApJ 947, 42,
  doi:10.3847/1538-4357/acc728: interacting Type IIn supernovae lethal by X-rays to about 50 pc;
  verified in the source's own text.
- Piran and Jimenez 2014, _Possible role of gamma ray bursts on life extinction in the universe_,
  Physical Review Letters 113, 231102, doi:10.1103/PhysRevLett.113.231102: a 95% chance of a lethal
  burst within 4 kpc of the Galactic centre; verified in the source's own text.
- Spinelli et al. 2021, _The best place and time to live in the Milky Way_, A&A 647, A41,
  doi:10.1051/0004-6361/202039507: 2–8 kpc safest over the last 4 Gyr when weighted by metallicity,
  and the late-Ordovician extinction probably a burst; verified in the source's own text.
- Lineweaver, Fenner and Gibson 2004, _The Galactic habitable zone and the age distribution of
  complex life in the Milky Way_, Science 303, 59, doi:10.1126/science.1092322: the galactic
  habitable zone at 7–9 kpc, assuming such events reset complex life; verified in the source's own
  text.
- Fields et al. 2020, _Supernova triggers for end-Devonian extinctions_, PNAS 117, 21008,
  doi:10.1073/pnas.2013774117: the end-Devonian extinction, perhaps a supernova at about 20 pc;
  verified in the source's own text.
- Segura et al. 2010, _The effect of a strong stellar flare on the atmospheric chemistry of an
  Earth-like planet orbiting an M dwarf_, Astrobiology 10, 751, doi:10.1089/ast.2009.0376: a great
  flare's protons removing up to 94% of the ozone while surface ultraviolet exceeds Earth's for
  under 100 s; verified in the source's own text.
- Tilley et al. 2019, _Modeling repeated M dwarf flaring at an Earth-like planet in the habitable
  zone: atmospheric effects for an unmagnetized planet_, Astrobiology 19, 64,
  doi:10.1089/ast.2017.1794: repeated flares with proton events depleting ozone by 94% in ten years,
  recovery unlikely, the ocean a refuge; verified in the source's own text.
- Greene et al. 2023, _Thermal emission from the Earth-sized exoplanet TRAPPIST-1 b using JWST_,
  Nature 618, 39, doi:10.1038/s41586-023-05951-7: no thick atmosphere on TRAPPIST-1 b; verified in
  the source's own text.
- Zieba et al. 2023, _No thick carbon dioxide atmosphere on the rocky exoplanet TRAPPIST-1 c_,
  Nature 620, 746, doi:10.1038/s41586-023-06232-z: no thick atmosphere on TRAPPIST-1 c; verified in
  the source's own text.

**Chemistry, limits of life and energy**

- Chua et al. 2023, _Low-temperature specific heat capacity of water–ammonia mixtures down to the
  eutectic_, ACS Earth and Space Chemistry 7, 1971, doi:10.1021/acsearthspacechem.3c00091: the
  water–ammonia eutectic at 176 K and near 33% NH₃ by mass, ammonia–water as water with an
  antifreeze; verified in the source's own text (full text).
- Bains 2004, _Many chemistries could be used to build living systems_, Astrobiology 4, 137,
  doi:10.1089/153110704323175124: an "ammonochemistry" proposed for pure ammonia; verified in the
  source's own text (abstract).
- Stevenson, Lunine and Clancy 2015, _Membrane alternatives in worlds without oxygen: creation of an
  azotosome_, Science Advances 1, e1400067, doi:10.1126/sciadv.1400067: azotosome membranes in
  methane by molecular dynamics, the case for; verified in the source's own text (abstract).
- Sandström and Rahm 2020, _Can polarity-inverted membranes self-assemble on Titan?_, Science
  Advances 6, eaax0272, doi:10.1126/sciadv.aax0272: azotosomes "not viable candidates for
  self-assembly"; verified in the source's own text (abstract).
- Vu and Hodyss 2026, _Experimental insights into the azotosome hypothesis in Titan's lake fluids_,
  Science Advances 12, eaed1426, doi:10.1126/sciadv.aed1426: the azotosome's acrylonitrile
  co-crystallising with ethane; verified in the source's own text (abstract).
- Seager S. et al. 2024, _Year-long stability of nucleic acid bases in concentrated sulfuric acid:
  implications for the persistence of organic chemistry in Venus' clouds_, Life 14, 538,
  doi:10.3390/life14050538: Venus's droplets of 81–98% H₂SO₄ by mass, and nucleobases surviving a
  year in them; verified in the source's own text (abstract).
- Seager M. D., Seager, Bains and Petkowski 2024, _Stability of 20 biogenic amino acids in
  concentrated sulfuric acid: implications for the habitability of Venus' clouds_, Astrobiology 24,
  386, doi:10.1089/ast.2023.0082: 19 of 20 amino acids keeping their backbones for four weeks, and
  Venus's clouds at 48–60 km at Earth-surface temperatures; verified in the source's own text
  (abstract).
- Hallsworth et al. 2021, _Water activity in Venus's uninhabitable clouds and other planetary
  atmospheres_, Nature Astronomy 5, 665, doi:10.1038/s41550-021-01391-3: Venus's cloud water
  activity at most 0.004, "two orders of magnitude below the 0.585 limit for known extremophiles",
  while Jupiter's clouds pass, and ice's water activity "less than 0.585" below −59 °C; verified in
  the source's own text (abstract; the −59 °C in the full text, the Queen's University Belfast copy,
  in the second review).
- Budisa and Schulze-Makuch 2014, _Supercritical carbon dioxide and its potential as a
  life-sustaining solvent in a planetary environment_, Life 4, 331, doi:10.3390/life4030331: enzymes
  stable and selective in supercritical CO₂; verified in the source's own text (abstract).
- Benner, Ricardo and Carrigan 2004, _Is there a common chemical model for life in the universe?_,
  Current Opinion in Chemical Biology 8, 672, doi:10.1016/j.cbpa.2004.10.003: supercritical H₂–He
  mixtures named as a solvent; verified in the source's own text (abstract).
- Seager, Huang, Petkowski and Pajusalu 2020, _Laboratory studies on the viability of life in
  H₂-dominated exoplanet atmospheres_, Nature Astronomy 4, 802, doi:10.1038/s41550-020-1069-4: _E.
  coli_ and yeast that "survive and grow in a 100% H₂ atmosphere"; verified in the source's own text
  (abstract).
- Petkowski, Bains and Seager 2020, _On the potential of silicon as a building block for life_, Life
  10, 84, doi:10.3390/life10060084: "in no environment is a life based primarily around silicon
  chemistry a plausible option", the case for carbon only, though concentrated sulfuric acid
  "appears to be able to support a much larger diversity of organosilicon chemistry than water";
  verified in the source's own text (abstract).
- Takai et al. 2008, _Cell proliferation at 122 °C and isotopically heavy CH₄ production by a
  hyperthermophilic methanogen under high-pressure cultivation_, PNAS 105, 10949,
  doi:10.1073/pnas.0712334105: growth at 122 °C under 20 MPa and 116 °C at 0.4 MPa, the envelope's
  upper temperature; verified in the source's own text (abstract).
- Mykytczuk et al. 2013, _Bacterial growth at −15 °C; molecular insights from the permafrost
  bacterium Planococcus halocryophilus Or1_, ISME Journal 7, 1211, doi:10.1038/ismej.2013.8: growth
  at −15 °C in 18% salt and activity to −25 °C, the default T_min; verified in the source's own text
  (in the check; not read in the research).
- Panikov and Sizova 2007, _Growth kinetics of microorganisms isolated from Alaskan soil and
  permafrost in solid media frozen down to −35 °C_, FEMS Microbiology Ecology 59, 500,
  doi:10.1111/j.1574-6941.2006.00210.x: continuous growth on frozen media to −17 °C; verified in the
  source's own text (in the check; not read in the research).
- Rivkina, Friedmann, McKay and Gilichinsky 2000, _Metabolic activity of permafrost bacteria below
  the freezing point_, Applied and Environmental Microbiology 66, 3230,
  doi:10.1128/AEM.66.8.3230-3233.2000: activity at −20 °C in permafrost; verified in the source's
  own text (abstract, in the second review).
- Merino et al. 2019, _Living at the extremes: extremophiles and the limits of life in a planetary
  context_, Frontiers in Microbiology 10, 780, doi:10.3389/fmicb.2019.00780: the limits table's
  compilation, pH −0.06 and 12.5 (_Serpentinomonas_), −20 °C activity, 125 MPa and 30 kGy; verified
  in the source's own text (full text). Its attribution of activity at 2,000 MPa to Vanlint et al.
  2011 does not match that paper and is not used.
- Clarke et al. 2013, _A low temperature limit for life on Earth_, PLoS ONE 8, e66207,
  doi:10.1371/journal.pone.0066207: free cells vitrifying at −10 to −26 °C, the cold limit as the
  cytoplasm's glass transition, "few multicellular organisms" completing their life cycle below
  about −2 °C, and water clouds keeping transported cells unfrozen; verified in the source's own
  text (abstract).
- Stevenson et al. 2017, _Aspergillus penicillioides differentiation and cell division at 0.585
  water activity_, Environmental Microbiology 19, 687, doi:10.1111/1462-2920.13597: germination and
  cell division at 0.585, the eukaryote-like floor; verified in the source's own text (abstract).
- Stevenson et al. 2015, _Is there a common water-activity limit for the three domains of life?_,
  ISME Journal 9, 1333, doi:10.1038/ismej.2014.219: halophilic archaea growing at 0.635, an
  extrapolated prokaryotic floor of 0.611, saturated NaCl at 0.755, and "a common water-activity
  limit that is determined by physicochemical constraints for the three domains of life"; verified
  in the source's own text (abstract).
- Hallsworth et al. 2007, _Limits of life in MgCl₂-containing environments: chaotropicity defines
  the window_, Environmental Microbiology 9, 801, doi:10.1111/j.1462-2920.2006.01212.x: no active
  cells above about 2.3 M MgCl₂ "in the absence of compensating (e.g. kosmotropic) solutes";
  verified in the source's own text (abstract).
- Steinle et al. 2018, _Life on the edge: active microbial communities in the Kryos MgCl₂-brine
  basin at very low water activity_, ISME Journal 12, 1414, doi:10.1038/s41396-018-0107-z: sulfate
  reduction reported near 0.4 water activity, contested and not adopted; verified in the source's
  own text (abstract).
- Schleper et al. 1995, _Picrophilus gen. nov., fam. nov.: a novel aerobic, heterotrophic,
  thermoacidophilic genus and family comprising archaea capable of growth around pH 0_, Journal of
  Bacteriology 177, 7050, doi:10.1128/jb.177.24.7050-7059.1995: growth "around pH 0", the acid end
  of the envelope; verified in the source's own text (the title, and the abstract's "growth occurred
  even around pH 0" in the second review).
- Dalmasso et al. 2016, _Thermococcus piezophilus sp. nov., a novel hyperthermophilic and
  piezophilic archaeon with a broad pressure range for growth, isolated from a deepest hydrothermal
  vent at the Mid-Cayman Rise_, Systematic and Applied Microbiology 39, 440,
  doi:10.1016/j.syapm.2016.08.003: growth to 125 MPa, the default `max_growth_pressure`; verified
  via a secondary source (Merino et al. 2019).
- Sharma et al. 2002, _Microbial activity at gigapascal pressures_, Science 295, 1514,
  doi:10.1126/science.1068018: formate oxidation reported to 1,060 MPa and viability in ice-VI
  inclusions at 1,200–1,600 MPa, disputed; verified in the source's own text (abstract).
- Yayanos 2002, _Are cells viable at gigapascal pressures?_, Science 297, 295,
  doi:10.1126/science.297.5580.295a: the dispute of Sharma et al., a technical comment on their
  result, which "stands in marked contrast to all previous studies"; verified in the source's own
  text (its opening paragraph, through OpenAlex, in the second review).
- Vanlint et al. 2011, _Rapid acquisition of gigapascal-high-pressure resistance by Escherichia
  coli_, mBio 2, e00130-10, doi:10.1128/mBio.00130-10: survival of gigapascal pressures after
  directed evolution, not activity; verified in the source's own text (in the check; not read in the
  research).
- Jolivet et al. 2003, _Thermococcus gammatolerans sp. nov., a hyperthermophilic archaeon from a
  deep-sea hydrothermal vent that resists ionizing radiation_, International Journal of Systematic
  and Evolutionary Microbiology 53, 847, doi:10.1099/ijs.0.02503-0: isolation after, so survival of,
  30 kGy of γ rays; verified in the source's own text (abstract, in the second review).
- Murphy and Koop 2005, _Review of the vapour pressures of ice and supercooled water for atmospheric
  applications_, Quarterly Journal of the Royal Meteorological Society 131, 1539,
  doi:10.1256/qj.04.94: the fits behind brine-against-ice water activity, 0.86 at −15 °C and 0.585
  only near −59 °C; **from memory** for the fits (the abstract read in the second review, which
  gives the parametrisations, the paper's body unreachable behind Cloudflare; the reference
  completed from Crossref); as recalled they reproduce ice's and water's 611.66 Pa at the triple
  point and give 0.864 at −15 °C and 0.585 at −58.8 °C (computed), and Hallsworth et al. 2021 put
  ice's water activity below 0.585 under −59 °C.
- Glavin and Dworkin 2009, _Enrichment of the amino acid L-isovaline by aqueous alteration on CI and
  CM meteorite parent bodies_, PNAS 106, 5487, doi:10.1073/pnas.0811618106: L-isovaline in excess by
  18.5 ± 2.6% in Murchison and 15.2 ± 4.0% in Orgueil, the prebiotic bias and the chirality
  analysis's abiotic trap; verified in the source's own text (abstract).
- Ono, Takigawa and Yamada 2020, _L-glucose: another path to cancer cells_, Cancers 12, 850,
  doi:10.3390/cancers12040850: L-glucose "an unmetabolizable sugar except in some bacteria", so
  opposite-handed biomass is no food; verified in the source's own text (abstract).
- Kiang, Siefert, Govindjee and Blankenship 2007a, _Spectral signatures of photosynthesis. I. Review
  of Earth organisms_, Astrobiology 7, 222, doi:10.1089/ast.2006.0105: oxygenic photosynthesis
  limited to 400–730 nm and anoxygenic reaching 1,015–1,020 nm with bacteriochlorophyll _b_; 1,100
  nm as a possible long limit; absorption to 730 nm "just the tail end … and presumed not to
  contribute a large amount"; and the three rules of pigment placement, with P680 near 685 nm and
  the O₂ A band at 761 nm, for the pigment lean; verified in the source's own text (full text of the
  preprint, arXiv:astro-ph/0701382).
- Nürnberg et al. 2018, _Photochemistry beyond the red limit in chlorophyll f-containing
  photosystems_, Science 360, 1210, doi:10.1126/science.aar8313: oxygenic charge separation with
  chlorophyll _f_ at 745 nm in photosystem I and chlorophyll _f_ or _d_ at 727 nm in photosystem II;
  verified in the source's own text (abstract).
- Lehmer, Catling, Parenteau and Hoehler 2018, _The productivity of oxygenic photosynthesis around
  cool, M dwarf stars_, ApJ 859, 171, doi:10.3847/1538-4357/aac104: "the flux of 400–750 nm photons
  may be just a few percent that of Earth's", a TRAPPIST-1-like outer zone light-limited, and the
  400–750 nm oxygenic band; verified in the source's own text (abstract).
- Lingam and Loeb 2019a, _Photosynthesis on habitable planets around low-mass stars_, MNRAS 485,
  5924, doi:10.1093/mnras/stz847: Earth-like productivity possibly out of reach below about 0.2 M☉,
  and the 400–750 nm band; verified in the source's own text (abstract).
- Battistuzzi et al. 2023, _Oxygenic photosynthetic responses of cyanobacteria exposed under an
  M-dwarf starlight simulator: implications for exoplanet's habitability_, Frontiers in Plant
  Science 14, 1070359, doi:10.3389/fpls.2023.1070359: cyanobacteria photosynthesising much as under
  sunlight in a simulated M-dwarf spectrum, "at the tested light intensities"; verified in the
  source's own text (abstract).
- Husser et al. 2013, _A new extensive library of PHOENIX stellar atmospheres and synthetic
  spectra_, A&A 553, A6, doi:10.1051/0004-6361/201219058: the PHOENIX library's 2,300–12,000 K,
  behind the photon-yield table; verified in the source's own text (abstract).
- Hancke et al. 2018, _Extreme low light requirement for algae growth underneath sea ice: a case
  study from Station Nord, NE Greenland_, JGR Oceans 123, 985, doi:10.1002/2017JC013263: ice algae
  growing below 0.17 µmol photons m⁻² s⁻¹, the light gate's floor; verified in the source's own text
  (abstract).
- Sherwood Lollar et al. 2014, _The contribution of the Precambrian continental lithosphere to
  global H₂ production_, Nature 516, 379, doi:10.1038/nature14017: about 10¹¹ mol of H₂ a year from
  the marine lithosphere and 0.36–2.27 × 10¹¹ from the Precambrian continents; verified in the
  source's own text (abstract).
- Lin et al. 2006, _Long-term sustainability of a high-energy, low-diversity crustal biome_, Science
  314, 479, doi:10.1126/science.1127376: a sulfate reducer 2.8 km down living with "no apparent
  reliance on photosynthetically derived substrates"; verified in the source's own text (abstract).
- Schink 1997, _Energetics of syntrophic cooperation in methanogenic degradation_, Microbiology and
  Molecular Biology Reviews 61, 262, doi:10.1128/mmbr.61.2.262-280.1997: some 20–25 kJ per mole for
  a reaction to be harnessed; verified in the source's own text (abstract).
- Heijnen and van Dijken 1992, _In search of a thermodynamic description of biomass yields for the
  chemotrophic growth of microorganisms_, Biotechnology and Bioengineering 39, 833,
  doi:10.1002/bit.260390806: about 1,060 kJ to fix a mole of carbon from H₂ and 3,500 kJ from
  sulfide or ferrous iron; verified via a secondary source (Böhnke et al. 2019).
- Böhnke et al. 2019, _Parameters governing the community structure and element turnover in Kermadec
  volcanic ash and hydrothermal fluids…_, Frontiers in Microbiology 10, 2296,
  doi:10.3389/fmicb.2019.02296: the secondary source for Heijnen and van Dijken's energy costs;
  verified in the source's own text (full text).
- McCollom and Amend 2005, _A thermodynamic assessment of energy requirements for biomass synthesis
  by chemolithoautotrophic micro-organisms in oxic and anoxic environments_, Geobiology 3, 135,
  doi:10.1111/j.1472-4669.2005.00045.x: "only about 10% or less" of a chemolithoautotroph's energy
  reaching biomass; verified in the source's own text (abstract).
- LaRowe and Amend 2015, _Power limits for microbial life_, Frontiers in Microbiology 6, 718,
  doi:10.3389/fmicb.2015.00718: about 190 zW a cell in a starved sediment, with a minimum near 1 zW,
  the maintenance population; verified in the source's own text (abstract).
- Hoehler and Jørgensen 2013, _Microbial life under extreme energy limitation_, Nature Reviews
  Microbiology 11, 83, doi:10.1038/nrmicro2939: cells turning over "on timescales of centuries to
  millennia"; verified in the source's own text (abstract).
- Potter et al. 1993, _Terrestrial ecosystem production: a process model based on global satellite
  and surface data_, Global Biogeochemical Cycles 7, 811, doi:10.1029/93GB02725: CASA's
  light-use-efficiency form, 48 Pg C a year of land NPP at a maximum of 0.39 g C per MJ of absorbed
  light, from which ε_q is derived; verified in the source's own text (abstract).
- Lieth 1975, _Modeling the primary productivity of the world_, in Lieth and Whittaker (eds.),
  _Primary Productivity of the Biosphere_, Springer, Ecological Studies, 237–263,
  doi:10.1007/978-3-642-80913-2_12: Lieth's Miami model, a temperature-limited rate against a
  water-limited one, and its two terms f_T and P_W; verified via a secondary source
  (Bertsch-Hoermann et al. 2021, and Albrecht et al. 2017, Scientific Reports 7, 10399,
  doi:10.1038/s41598-017-10772-6, for the dry-matter units), the chapter's title and pages from
  Crossref, the chapter itself not read; Zaks et al. (2007) date the model to 1973.
- Bertsch-Hoermann et al. 2021, _Agroforestry trade-offs between biomass provision and aboveground
  carbon sequestration in the alpine Eisenwurzen region, Austria_, Regional Environmental Change 21,
  77, doi:10.1007/s10113-021-01794-y: the secondary source that prints the Miami model's
  coefficients; verified in the source's own text (full text).
- Zhu, Long and Ort 2008, _What is the maximum efficiency with which photosynthesis can convert
  solar energy into biomass?_, Current Opinion in Biotechnology 19, 153,
  doi:10.1016/j.copbio.2008.02.004: the ceiling of 4.6% of sunlight into biomass for C3 plants and
  6% for C4; verified in the source's own text (abstract).
- Castenholz 2015, _Portrait of a geothermal spring, Hunter's Hot Springs, Oregon_, Life 5, 332,
  doi:10.3390/life5010332: "Synechococcus at 73-74 °C (the world-wide upper limit for
  photosynthesis)" and Chloroflexus at 68–70 °C, the phototroph ceiling's default; and Fig. 1's mats
  zoned by temperature, from over ~93 °C at the source to Synechococcus's green biofilm (73–74 °C)
  that ends at 54–55 °C under Geitlerinema's dark brown cover over a salmon-coloured undermat of
  Chloroflexus, the hydrothermal field's colours; verified in the source's own text (abstract,
  through OpenAlex, and Fig. 1's caption, through Europe PMC, PMC4390855, in the second review).
- Kees, Murugapiran, Bennett and Hamilton 2022, _Distribution and genomic variation of thermophilic
  cyanobacteria in diverse microbial mats at the upper temperature limits of photosynthesis_,
  mSystems 7, e00317-22, doi:10.1128/msystems.00317-22: photosynthesis "strictly limited to
  temperatures below 73°C"; verified in the source's own text (abstract, through OpenAlex, in the
  second review).
- Glaser et al. 2020, _Detectability of life using oxygen on pelagic planets and water worlds_, ApJ
  893, 163, doi:10.3847/1538-4357/ab822d: phosphorus limiting a water world without land to O₂
  levels indistinguishable from abiotic ones; verified in the source's own text (abstract).
- Lingam and Loeb 2018, _Is extraterrestrial life suppressed on subsurface ocean worlds due to the
  paucity of bioessential elements?_, AJ 156, 151, doi:10.3847/1538-3881/aada02: a subsurface
  ocean's phosphorus possibly "a few orders of magnitude" below Earth's; verified in the source's
  own text (abstract).
- Postberg et al. 2023, _Detection of phosphates originating from Enceladus's ocean_, Nature 618,
  489, doi:10.1038/s41586-023-05987-9: plume grains with "at least 100-fold" Earth's phosphate, so
  the hydrothermal floor is a habitat input; verified in the source's own text (abstract).
- Klingler, Mancinelli and White 1989, _Biological nitrogen fixation under primordial Martian
  partial pressures of dinitrogen_, Advances in Space Research 9, 173,
  doi:10.1016/0273-1177(89)90225-1: _Azotobacter vinelandii_ and _Azomonas agilis_ growing on N₂
  down to 5 mb but not at 1 mb or below, their biomass falling below 400 mb, the nitrogen factor's
  floor; verified in the source's own text (abstract, in the second review).
- Still, Berry, Collatz and DeFries 2003, _Global distribution of C3 and C4 vegetation: carbon cycle
  implications_, Global Biogeochemical Cycles 17, 1006, doi:10.1029/2001GB001807: C4 plants' 35.3 Pg
  C a year of gross production, about 23% of the land's, over 18.8 million km², the land value of
  s₄; verified in the source's own text (abstract, through OpenAlex, in the second review), the
  article number 1006 from memory, Crossref giving none.
- Field, Behrenfeld, Randerson and Falkowski 1998, _Primary production of the biosphere: integrating
  terrestrial and oceanic components_, Science 281, 237, doi:10.1126/science.281.5374.237: global
  NPP of 104.9 Pg C a year, 56.4 on land and 48.5 in the ocean; the ocean's mean of 140 g C m⁻² yr⁻¹
  with eutrophic waters giving under 18%; productivity limited by light, nutrients and temperature;
  and Table 1's biome totals (tropical rainforests 17.8, savannas 16.8 Pg C a year), the reference
  Earth's tests; verified in the source's own text (abstract, p. 238 and Table 1, the last two in
  the check).
- Zaks, Ramankutty, Barford and Foley 2007, _From Miami to Madison: investigating the relationship
  between climate and terrestrial net primary production_, Global Biogeochemical Cycles 21,
  doi:10.1029/2006GB002705: land NPP of 36–74 Pg C a year across statistical models with paired
  climatic variables, the reference Earth's band; verified in the source's own text (abstract).
- Bar-On, Phillips and Milo 2018, _The biomass distribution on Earth_, PNAS 115, 6506,
  doi:10.1073/pnas.1711842115: about 550 Gt C of standing biomass, plants 450, bacteria 70, archaea
  7 and marine biota about 6, behind the residence time, ocean colour and the biomass unit; verified
  in the source's own text (abstract).
- Magnabosco et al. 2018, _The biomass and biodiversity of the continental subsurface_, Nature
  Geoscience 11, 707, doi:10.1038/s41561-018-0221-6: 2–6 × 10²⁹ cells in the continental subsurface,
  prokaryotes revised to 23–31 Pg C in all, and cell numbers predicted from heat flow, temperature,
  depth and lithology; verified in the source's own text (abstract).

**Habitats beyond the surface**

- Waite et al. 2017, _Cassini finds molecular hydrogen in the Enceladus plume: evidence for
  hydrothermal processes_, Science 356, 155, doi:10.1126/science.aai8703: H₂ out of equilibrium that
  "favors the formation of methane from CO₂", and the plume's 0.4–1.4% H₂ beside 0.3–0.8% CO₂;
  verified in the source's own text (abstract), the mixing ratios via a secondary source (Taubner et
  al. 2018, Table 1).
- Taubner et al. 2018, _Biological methane production under putative Enceladus-like conditions_,
  Nature Communications 9, 748, doi:10.1038/s41467-018-02876-y: the tabulation of Waite et al.'s
  plume composition; verified in the source's own text (full text, Table 1).
- Steel, Davila and McKay 2017, _Abiotic and biotic formation of amino acids in the Enceladus
  ocean_, Astrobiology 17, 862, doi:10.1089/ast.2017.1673: serpentinisation supplying 0.6–34 mol of
  H₂ a second, enough for 4 × 10⁴–2 × 10⁶ kg of biomass a year and 80–4,250 cells per cm³; verified
  in the source's own text (abstract).
- Hand, Carlson and Chyba 2007, _Energy, chemical disequilibrium, and geological constraints on
  Europa_, Astrobiology 7, 1006, doi:10.1089/ast.2007.0156: Europa's ocean reaching "O₂
  concentrations comparable to those found in terrestrial surface waters" even with some 10⁹ mol a
  year of hydrothermal reductants; verified in the source's own text (abstract).
- Vance, Hand and Pappalardo 2016, _Geophysical controls of chemical disequilibria in Europa_, GRL
  43, 4871, doi:10.1002/2016GL068547: Europa's water–rock H₂ about a tenth of Earth's, with an
  oxidant flux of the same order; verified in the source's own text (abstract).
- Kallmeyer et al. 2012, _Global distribution of microbial abundance and biomass in subseafloor
  sediment_, PNAS 109, 16213, doi:10.1073/pnas.1203849109: 2.9 × 10²⁹ cells in subseafloor sediment,
  about 4.1 Pg C; verified in the source's own text (abstract).
- Pollack, Hurter and Johnson 1993, _Heat flow from the Earth's interior: analysis of the global
  data set_, Reviews of Geophysics 31, 267, doi:10.1029/93RG01249: continents losing 65 mW m⁻² and
  oceans 101, 87 on average, for the 122 °C floor's depth; verified in the source's own text
  (abstract).
- Parro et al. 2017, _Present-day heat flow model of Mars_, Scientific Reports 7, 45629,
  doi:10.1038/srep45629: Mars losing 14–25 mW m⁻², 19 on average, so its crust may hold life to
  about 24 km; verified in the source's own text (abstract).
- Price 2000, _A habitat for psychrophiles in deep Antarctic ice_, PNAS 97, 1247,
  doi:10.1073/pnas.97.3.1247: liquid veins keeping at least a cell per cm³ in Antarctic glacial ice;
  verified in the source's own text (abstract).
- Schmidt et al. 2011, _Active formation of 'chaos terrain' over shallow subsurface water on
  Europa_, Nature 479, 502, doi:10.1038/nature10608: water lenses as shallow as 3 km; verified in
  the source's own text (abstract).
- Rivera-Valentín et al. 2020, _Distribution and habitability of (meta)stable brines on present-day
  Mars_, Nature Astronomy 4, 756, doi:10.1038/s41550-020-1080-9: Mars's surface brines forming only
  as the lowest eutectics, under 225 K and outside known tolerances; verified in the source's own
  text (abstract).
- Seager et al. 2021, _The Venusian lower atmosphere haze as a depot for desiccated microbial life:
  a proposed life cycle for persistence of the Venusian aerial biosphere_, Astrobiology 21, 1206,
  doi:10.1089/ast.2020.2244: a proposed life cycle outlasting droplet settling as desiccated spores
  in the lower haze at 33–48 km; verified in the source's own text (abstract).
- Sagan and Salpeter 1976, _Particles, environments, and possible ecologies in the Jovian
  atmosphere_, ApJS 32, 737, doi:10.1086/190414: the Jovian ecology of "sinkers", "floaters",
  "hunters" and "scavengers", niches for the first three that "appear to exist"; verified in the
  source's own text (abstract, through OpenAlex).
- Stevenson 1999, _Life-sustaining planets in interstellar space?_, Nature 400, 32,
  doi:10.1038/21811: an ejected Earth-mass body keeping 10²–10⁴ bar of hydrogen and surface water on
  radiogenic heat, at an effective temperature near 30 K; verified in the source's own text
  (abstract).
- Abbot and Switzer 2011, _The Steppenwolf: a proposal for a habitable planet in interstellar
  space_, ApJL 735, L27, doi:10.1088/2041-8205/735/2/L27: a subglacial ocean in a rogue of about 3.5
  M⊕, or 0.3 M⊕ with ten times the water or a thick frozen atmosphere, detectable only in the far
  infrared at close range; verified in the source's own text (abstract).
- Pierrehumbert and Gaidos 2011, _Hydrogen greenhouse planets beyond the habitable zone_, ApJL 734,
  L13, doi:10.1088/2041-8205/734/1/L13: 40 bar of H₂ keeping a 3 M⊕ planet at 280 K out to 10 au, on
  which P14.T53.a's H₂ greenhouse is calibrated; verified in the source's own text (abstract).
- Ávila et al. 2021, _Presence of water on exomoons orbiting free-floating planets: a case study_,
  International Journal of Astrobiology 20, 300, doi:10.1017/S1473550421000173: a rogue giant's moon
  holding surface water under CO₂, tides its main heat; verified in the source's own text (full
  text).

**The biosphere's effect on the atmosphere and climate**

- Kharecha, Kasting and Siefert 2005, _A coupled atmosphere–ecosystem model of the early Archean
  Earth_, Geobiology 3, 53, doi:10.1111/j.1472-4669.2005.00049.x: an anoxic biosphere "limited by
  the downward flux of H₂, CO, S₈, and H₂S … and by the upwelling rate of Fe²⁺", its NPP at least 40
  times below the modern ocean's, a CH₄ flux of a third to 2.5 times modern's and 100 to 35,000 ppmv
  of CH₄, which cap the anoxygenic productivity and span the drawn CH₄ flux; verified in the
  source's own text (abstract).
- Catling, Zahnle and McKay 2001, _Biogenic methane, hydrogen escape, and the irreversible oxidation
  of early Earth_, Science 293, 839, doi:10.1126/science.1061976: 10²–10³ ppmv of Archean CH₄ and
  some 10¹²–10¹³ mol yr⁻¹ of oxygen left behind by hydrogen escape; verified in the source's own
  text (abstract).
- Catling and Zahnle 2020, _The Archean atmosphere_, Science Advances 6, eaax1420,
  doi:10.1126/sciadv.aax1420: Archean O₂ under 10⁻⁶ PAL, CO₂ some 10 to 2,500 times modern and CH₄
  10² to 10⁴ times, for 0–40 °C under the fainter Sun, and the xenon record's evidence of rapid
  hydrogen escape; verified in the source's own text (abstract).
- Byrne and Goldblatt 2015, _Diminished greenhouse warming from Archean methane due to solar
  absorption lines_, Climate of the Past 11, 559, doi:10.5194/cp-11-559-2015: methane's warming
  diminished above a mixing ratio of 10⁻³ by its shortwave absorption, the cap on the CH₄ term;
  verified in the source's own text (the discussion paper's abstract, Climate of the Past
  Discussions 10, 4229, 2014; the final paper confirmed in the check).
- Haqq-Misra et al. 2008, _A revised, hazy methane greenhouse for the Archean Earth_, Astrobiology
  8, 1127, doi:10.1089/ast.2007.0197: about 0.03 bar of CO₂ needed besides methane in the late
  Archean; verified in the source's own text (abstract).
- Arney et al. 2016, _The pale orange dot: the spectrum and habitability of hazy Archean Earth_,
  Astrobiology 16, 873, doi:10.1089/ast.2015.1422: an organic haze that "can cool the planet's
  surface by about 20 K", limits itself and cuts surface ultraviolet by about 97%; verified in the
  source's own text (abstract). The haze threshold CH₄/CO₂ ≈ 0.1–0.2 is plan 14's citation of it and
  Trainer et al. 2006, not in its abstract.
- Rutherford et al. 2017, _Albedo feedbacks to future climate via climate change impacts on dryland
  biocrusts_, Scientific Reports 7, 44188, doi:10.1038/srep44188: biocrusts up to 70% of dryland
  cover, whose loss raised shortwave albedo by over 30% in a warming and watering experiment on the
  Colorado Plateau; verified in the source's own text (abstract).
- Claire, Catling and Zahnle 2006, _Biogeochemical modelling of the rise in atmospheric oxygen_,
  Geobiology 4, 239, doi:10.1111/j.1472-4669.2006.00084.x: oxygen rising when reducing gases' flux
  "falls below the flux of organic carbon burial that produces O₂"; verified in the source's own
  text (abstract).
- Krissansen-Totton, Kipp and Catling 2021, _Carbon cycle inverse modeling suggests large changes in
  fractional organic burial are consistent with the carbon isotope record and may have contributed
  to the rise of oxygen_, Geobiology 19, 342, doi:10.1111/gbi.12440: fractional organic burial
  possibly rising two to five times since the Archean, a rising O₂ source beside a falling sink;
  verified in the source's own text (abstract).
- Lyons, Reinhard and Planavsky 2014, _The rise of oxygen in Earth's early ocean and atmosphere_,
  Nature 506, 307, doi:10.1038/nature13068: Earth's record of an initial rise, a delayed build-up
  and near-modern levels two billion years later; verified in the source's own text (abstract).
- Planavsky et al. 2014, _Low Mid-Proterozoic atmospheric oxygen levels and the delayed rise of
  animals_, Science 346, 635, doi:10.1126/science.1258410: the mid-Proterozoic at most 0.1% PAL from
  chromium isotopes, the recorded alternative; verified in the source's own text (abstract).
- Daines, Mills and Lenton 2017, _Atmospheric oxygen regulation at low Proterozoic levels by
  incomplete oxidative weathering of sedimentary organic carbon_, Nature Communications 8, 14379,
  doi:10.1038/ncomms14379: regulation near 0.1 PAL losing stability below 0.01; verified in the
  source's own text (abstract).
- Gregory, Claire and Rugheimer 2021, _Photochemical modelling of atmospheric oxygen levels confirms
  two stable states_, EPSL 561, 116818, doi:10.1016/j.epsl.2021.116818: O₂ unstable between 3 × 10⁻⁸
  and 10⁻² PAL; verified via a secondary source (Cooke et al. 2022, §2).
- Cooke et al. 2022, _A revised lower estimate of ozone columns during Earth's oxygenated history_,
  Royal Society Open Science 9, 211165, doi:10.1098/rsos.211165: the three-dimensional ozone columns
  (279 DU pre-industrial, 169 at 0.1 PAL, 136 at 0.05, 66 at 0.01, 45 at 0.005, 18 at 0.001);
  methane's surface lifetime ratios (1.061 at 1.5 PAL, 0.903 at 0.5, 0.70 at 0.1, 0.52 at 0.05,
  0.067 at 0.01, 0.033 at 0.005 and 0.010 at 0.001) at a fixed 0.8 ppmv of CH₄ (Table 2); 10⁻³ to
  1.5 PAL since the Great Oxidation, the land band's upper end; and the ozone fixture; verified in
  the source's own text (full text; the whole of Table 2 in the second review).
- Olson, Reinhard and Lyons 2016, _Limited role for methane in the mid-Proterozoic greenhouse_, PNAS
  113, 11447, doi:10.1073/pnas.1608549113: anaerobic oxidation with sulphate holding CH₄ under
  10 ppmv "regardless of atmospheric pO₂"; verified in the source's own text (abstract).
- Pavlov et al. 2000, _Greenhouse warming by CH₄ in the atmosphere of early Earth_, JGR Planets 105,
  11981, doi:10.1029/1999JE001134: the collapse of the methane greenhouse proposed as the trigger of
  the first widespread glaciation; verified in the source's own text (abstract).
- Schwieterman et al. 2022, _Evaluating the plausible range of N₂O biosignatures on exo-Earths: an
  integrated biogeochemical, photochemical, and spectral modeling approach_, ApJ 937, 109,
  doi:10.3847/1538-4357/ac8cfb: N₂O fluxes of 0.01–100 Tmol yr⁻¹, with 10 Tmol yr⁻¹ giving at most
  about 5 ppm about the Sun and 30 ppm on an Earth-like TRAPPIST-1e; verified in the source's own
  text (abstract).
- Porada et al. 2016, _High potential for weathering and climate effects of non-vascular vegetation
  in the Late Ordovician_, Nature Communications 7, 12113, doi:10.1038/ncomms12113: vegetation
  enhancing weathering about four to seven times and small watersheds two to sixteen, as it cites
  them, and Ordovician mosses and lichens weathering three times today's flux; verified in the
  source's own text (full text), the 4–7 and 2–16 secondary to its references.
- Krissansen-Totton, Arney and Catling 2018, _Constraining the climate and ocean pH of the early
  Earth with a geological carbon cycle model_, PNAS 115, 4105, doi:10.1073/pnas.1721296115: the
  continental weathering law F = f_bio f_land F_mod (pCO₂ ÷ pCO₂,mod)^α exp(ΔT ÷ T_e), its f_bio 1
  for modern Earth; verified in the source's own text (full text, eq. 1).
- Krissansen-Totton and Catling 2017, _Constraining climate sensitivity and continental versus
  seafloor weathering using an inverse geological carbon cycle model_, Nature Communications 8,
  15423, doi:10.1038/ncomms15423: α 0.24–0.44 (median 0.33) and T_e 22–45 K (median 34), both 1σ, a
  climate sensitivity of 5.6 K per doubling, and Walker, Hays and Kasting's 13.7 K as it states it;
  verified in the source's own text (full text), Walker, Hays and Kasting's figure via it.
- Walker, Hays and Kasting 1981, _A negative feedback mechanism for the long-term stabilization of
  Earth's surface temperature_, JGR Oceans 86, 9776, doi:10.1029/JC086iC10p09776: the weathering
  model's e-folding temperature of 13.7 K; verified via a secondary source (Krissansen-Totton and
  Catling 2017), the reference from Crossref.
- Lenton et al. 2016, _Earliest land plants created modern levels of atmospheric oxygen_, PNAS 113,
  9704, doi:10.1073/pnas.1604787113: present O₂ levels by 420–400 Ma, where "fossil charcoal
  indicates O₂ >15–17%", and "fire-mediated negative feedbacks" after, the land regime's 0.7 PAL
  floor; "cryptogamic vegetation cover could have achieved ∼30% of today's global terrestrial net
  primary productivity by ∼445 Ma", with plants "from ∼470 Ma onward", for open question 42;
  verified in the source's own text (abstract, re-read in the second review).
- Quinn and Bates 2011, _The case against climate regulation via oceanic phytoplankton sulphur
  emissions_, Nature 480, 51, doi:10.1038/nature10580: "it is time to retire the CLAW hypothesis",
  so no biological cloud effect is modelled; verified in the source's own text (abstract).
- Kappler, Pasquero, Konhauser and Newman 2005, _Deposition of banded iron formations by anoxygenic
  phototrophic Fe(II)-oxidizing bacteria_, Geology 33, 865, doi:10.1130/G21658.1: banded iron
  formations precipitated by anoxygenic phototrophs that oxidise Fe²⁺, which alone could have made
  them all, the photoferrotrophic share; verified in the source's own text (full text).
- Konhauser et al. 2017, _Iron formations: a global record of Neoarchaean to Palaeoproterozoic
  environmental history_, Earth-Science Reviews 172, 140, doi:10.1016/j.earscirev.2017.06.012:
  oxygen from cyanobacteria still the main view of banded iron's oxidant; verified in the source's
  own text (abstract; its weighting of biological oxidation as the research reported it, the check
  finding the HAL and GEOMAR copies behind bot challenges).
- Kennedy et al. 2006, _Late Precambrian oxygenation; inception of the clay mineral factory_,
  Science 311, 1446, doi:10.1126/science.1118929: biologically active soils making clays that
  preserve buried organic matter, tied to the late Precambrian rise of oxygen; verified in the
  source's own text (abstract).
- Wordsworth and Pierrehumbert 2014, _Abiotic oxygen-dominated atmospheres on terrestrial habitable
  zone planets_, ApJL 785, L20, doi:10.1088/2041-8205/785/2/L20: water photolysis building an
  O₂-dominated air where N₂ is below "a few percent" of Earth's and the cold trap fails (a moist
  upper atmosphere above 340 K at 1 bar of N₂, 295 K at 0.1 bar, 255 K at 0.01 bar), an N₂-poor
  Earth losing 28% of an ocean in 4 Gyr to 66.2 bar of O₂, and its summary of Tian et al.'s 2–3 ×
  10⁻³; verified in the source's own text (full text).

**Biomes, vegetation and pigments**

- Beven and Kirkby 1979, _A physically based, variable contributing area model of basin hydrology_,
  Hydrological Sciences Bulletin 24, 43, doi:10.1080/02626667909491834: the wetness index ln(a ÷ tan
  β) that concentrates run-on precipitation; verified via a secondary source (Sørensen, Zinko and
  Seibert 2006, Hydrology and Earth System Sciences 10, 101, doi:10.5194/hess-10-101-2006, for the
  formula; the venue from Crossref).
- Whittaker 1975, _Communities and Ecosystems_, 2nd edn, Macmillan: the biome diagram of mean annual
  temperature and precipitation, a display and a test target; verified via a secondary source
  (Wikipedia, "Biome": Whittaker's types on precipitation and temperature, 1962–1975), its nine
  formations not checked.
- Holdridge 1947, _Determination of world plant formations from simple climatic data_, Science 105,
  367, doi:10.1126/science.105.2727.367: life zones from biotemperature, precipitation and the ratio
  of potential evapotranspiration to it; verified via a secondary source (Wikipedia, "Holdridge life
  zones"; PubMed holds no abstract).
- Beck et al. 2018, _Present and future Köppen–Geiger climate classification maps at 1-km
  resolution_, Scientific Data 5, 180214, doi:10.1038/sdata.2018.214: Köppen's boundaries drawn
  along vegetation, "crystallized, visible climate"; verified in the source's own text (body, in the
  check).
- Prentice et al. 1992, _A global biome model based on plant physiology and dominance, soil
  properties and climate_, Journal of Biogeography 19, 117, doi:10.2307/2845499: BIOME, plant
  functional types with bioclimatic limits, the potentially dominant types chosen among those the
  climate admits, and biomes as their combinations; verified in the source's own text (abstract,
  through the HAL record, in the second review; its count of types and its dominance rule not read).
- Kaplan et al. 2003, _Climate change and Arctic ecosystems: 2. Modeling, paleodata-model
  comparisons, and future projections_, JGR Atmospheres 108, doi:10.1029/2002JD002559: BIOME4, a
  small set of plant functional types; verified in the source's own text (abstract).
- Sitch et al. 2003, _Evaluation of ecosystem dynamics, plant geography and terrestrial carbon
  cycling in the LPJ dynamic global vegetation model_, Global Change Biology 9, 161,
  doi:10.1046/j.1365-2486.2003.00569.x: LPJ's ten plant functional types; verified in the source's
  own text (abstract, in the check).
- Kleidon and Mooney 2000, _A global distribution of biodiversity inferred from climatic
  constraints: results from a process-based modelling study_, Global Change Biology 6, 507,
  doi:10.1046/j.1365-2486.2000.00332.x: random parameter sets of a generic plant model, tested for
  survival on a global grid and grouped from grass-like to tree-like prototypes into biomes like the
  observed; verified in the source's own text (abstract).
- Pavlick, Drewry, Bohn, Reu and Kleidon 2013, _The Jena Diversity-Dynamic Global Vegetation Model
  (JeDi-DGVM): a diverse approach to representing terrestrial biogeography and biogeochemistry based
  on plant functional trade-offs_, Biogeosciences 10, 4137, doi:10.5194/bg-10-4137-2013: strategies
  of 15 traits in trade-offs comparing favourably with fixed-type models, weighting by living
  biomass, a spin-up of some 3,500 years, and global NPP of 29–57 Pg C a year across seeds with 10
  random strategies against a standard 2,000, which make the strategy draw stratified; verified in
  the source's own text (abstract, and §2–§5 in the check).
- Sulla-Menashe and Friedl 2018, _User Guide to Collection 6 MODIS Land Cover (MCD12Q1 and MCD12C1)
  Product_, USGS LP DAAC, <https://lpdaac.usgs.gov/documents/101/MCD12_User_Guide_V6.pdf>: IGBP's
  structural thresholds, forest a canopy above 2 m over 60% of the ground, savanna 10–30%, shrubs of
  1–2 m, grassland herbs under 2 m and barren under 10% cover; verified in the source's own text
  (Table 3).
- Longhurst 1998, _Ecological Geography of the Sea_, Academic Press: the ocean's Polar, Westerlies,
  Trade-wind and Coastal biomes, recognised in every major basin; verified via a secondary source
  (Flanders Marine Institute 2009, _Longhurst Provinces_ dataset page,
  <https://www.marineregions.org/sources.php>, which the body names).
- Homann et al. 2018, _Microbial life and biogeochemical cycling on land 3,220 million years ago_,
  Nature Geoscience 11, 665, doi:10.1038/s41561-018-0190-9: fossil mats draping river gravels of the
  Moodies Group, isotopically distinct from marine mats nearby, so wet and periodically exposed
  ground was matted early; verified in the source's own text (abstract and body).
- Rodriguez-Caballero et al. 2018, _Dryland photoautotrophic soil surface communities endangered by
  global change_, Nature Geoscience 11, 185, doi:10.1038/s41561-018-0072-1: biocrusts on about 12%
  of Earth's land; verified in the source's own text (abstract).
- Elbert et al. 2012, _Contribution of cryptogamic covers to the global cycles of carbon and
  nitrogen_, Nature Geoscience 5, 459, doi:10.1038/ngeo1486: cryptogamic covers taking up about 3.9
  Pg C a year, some 7% of terrestrial NPP; verified in the source's own text (abstract).
- Soule et al. 2009, _A comparative genomics approach to understanding the biosynthesis of the
  sunscreen scytonemin in cyanobacteria_, BMC Genomics 10, 336, doi:10.1186/1471-2164-10-336:
  scytonemin, the crust's UV-screening pigment; verified in the source's own text (abstract).
- Des Marais 2003, _Biogeochemistry of hypersaline microbial mats illustrates the dynamics of modern
  microbial ecosystems and the early evolution of the biosphere_, Biological Bulletin 204, 160,
  doi:10.2307/1543552: a mat as a complete ecosystem at the millimetre scale; verified in the
  source's own text (abstract).
- Rothermich, Guerrero, Lenz and Goodwin 2000, _Characterization, seasonal occurrence, and diel
  fluctuation of poly(hydroxyalkanoate) in photosynthetic microbial mats_, Applied and Environmental
  Microbiology 66, 4279, doi:10.1128/AEM.66.10.4279-4291.2000: a mat's green cyanobacterial layer
  over a pink purple-sulfur one; verified in the source's own text (abstract).
- DasSarma, DasSarma, Laye and Schwieterman 2020, _Extremophilic models for astrobiology:
  haloarchaeal survival strategies and pigments for remote sensing_, Extremophiles 24, 31,
  doi:10.1007/s00792-019-01126-3: red-orange carotenoids and purple retinal proteins in haloarchaeal
  blooms visible from space; verified in the source's own text (abstract).
- Dill, Shinn, Jones, Kelly and Steinen 1986, _Giant subtidal stromatolites forming in normal
  salinity waters_, Nature 324, 55, doi:10.1038/324055a0: columnar stromatolites over 2 m high in
  the Bahamas, the reef's upper size; verified in the source's own text (abstract and first page,
  through nature.com, in the second review).
- Suosaari et al. 2016, _New multi-scale perspectives on the stromatolites of Shark Bay, Western
  Australia_, Scientific Reports 6, 20557, doi:10.1038/srep20557: relief of 20 cm to about 1 m, "the
  larger columns reaching 1 m in height"; verified in the source's own text (full text, through
  Europe PMC, PMC4738353, in the second review).
- Lutz et al. 2016, _The biogeography of red snow microbiomes and their role in melting arctic
  glaciers_, Nature Communications 7, 11968, doi:10.1038/ncomms11968: red snow lowering snow albedo
  by 13% over a melt season; verified in the source's own text (abstract).
- Williamson et al. 2020, _Algal photophysiology drives darkening and melt of the Greenland Ice
  Sheet_, PNAS 117, 5694, doi:10.1073/pnas.1918412117: glacier algae with 11 times their chlorophyll
  in phenolic pigment, explaining up to 75% of the albedo variability across south-western
  Greenland; verified in the source's own text (abstract).
- Awramik 1971, _Precambrian columnar stromatolite diversity: reflection of metazoan appearance_,
  Science 174, 825, doi:10.1126/science.174.4011.825: columnar stromatolites losing diversity
  600–700 Myr ago as metazoans appeared, so reefs are the biome of a world without grazers; verified
  in the source's own text (abstract).
- Greco et al. 2020, _Microbial diversity of pinnacle and conical microbial mats in the perennially
  ice-covered Lake Untersee, East Antarctica_, Frontiers in Microbiology 11, 607251,
  doi:10.3389/fmicb.2020.607251: microbial cones growing under perennial lake ice, so reefs need no
  warmth; verified in the source's own text (in the check; not read in the research).
- Sankaran et al. 2005, _Determinants of woody cover in African savannas_, Nature 438, 846,
  doi:10.1038/nature04070: over 854 sites the 99th-quantile bound on woody cover rising at 0.14% per
  mm of mean annual precipitation, from zero near 100 mm to about 77% at a break of 650 ± 134 mm,
  above which only fire and herbivory hold a canopy open; the cover test, and with the treeline the
  fit of the maintenance cost on standing rigid biomass; verified in the source's own text
  (abstract, and Fig. 1's caption in the check).
- Belcher and McElwain 2008, _Limits for combustion in low O₂ redefine paleoatmospheric predictions
  for the Mesozoic_, Science 321, 1197, doi:10.1126/science.1160978: combustion needing about 15% O₂
  at Earth's pressure; verified in the source's own text (abstract).
- Belcher, Yearsley, Hadden, McElwain and Rein 2010, _Baseline intrinsic flammability of Earth's
  ecosystems estimated from paleoatmospheric oxygen over the past 350 million years_, PNAS 107,
  22448, doi:10.1073/pnas.1011974107: landscape fire "entirely switched off below 16%" O₂ and
  "greatly suppressed below 18.5%", so no fire-held savanna below about 16%; verified in the
  source's own text (abstract, in the check).
- Carvalhais et al. 2014, _Global covariation of carbon turnover times with climate in terrestrial
  ecosystems_, Nature 514, 213, doi:10.1038/nature13731: whole ecosystems' carbon residence of about
  23 years, against gross, not net, production; verified in the source's own text (abstract, and eq.
  1 in the check).
- Moles et al. 2009, _Global patterns in plant height_, Journal of Ecology 97, 923,
  doi:10.1111/j.1365-2745.2009.01526.x: wettest-month precipitation the best single predictor of
  plant height (R² 0.256), the mean maximum height of species 7.8 m within 15° of the equator
  against 27 cm at 60–75° N, and a wide range in cold, dry, unproductive systems; verified in the
  source's own text (abstract).
- Koch, Sillett, Jennings and Davis 2004, _The limits to tree height_, Nature 428, 851,
  doi:10.1038/nature02417: the redwoods' 122–130 m set by leaf water stress from gravity and
  path-length resistance, and Table 1's midday water potential, −0.00973 MPa m⁻¹ from −0.71 MPa at
  the ground to a limit of −1.9 MPa at 122 m, behind H_hyd and its scaling with gravity; verified in
  the source's own text (abstract, and Table 1 in the check through the Plazi record).
- McMahon 1973, _Size and shape in biology_, Science 179, 1201, doi:10.1126/science.179.4079.1201:
  elastic similarity, length as the ⅔ power of diameter, for girth and the stems per height octave;
  verified in the source's own text (abstract).
- Greenhill 1881, _Determination of the greatest height consistent with stability that a vertical
  pole or mast can be made_, Proceedings of the Cambridge Philosophical Society 4: the buckling
  height 0.79 (E ÷ ρg)^⅓ D^⅔; verified via a secondary source (Wikipedia, "Self-buckling", for
  Greenhill's constant 7.8373), the 0.79 coefficient computed here for a solid circular section,
  (7.8373 ÷ 16)^⅓ = 0.788; the paper and its pages not read.
- Niklas 2007, _Maximum plant height and the biophysical factors that limit it_, Tree Physiology 27,
  433, doi:10.1093/treephys/27.3.433: that the mechanical requirements for vertical growth impose no
  intrinsic limit on height, so buckling alone caps nothing; verified in the source's own text
  (quoted in the check; not read in the research).
- Gale 1972a, _Availability of carbon dioxide for photosynthesis at high altitudes: theoretical
  considerations_, Ecology 53, 494, doi:10.2307/1934239: CO₂ uptake holding at a fixed mixing ratio
  in thin air, diffusion compensating the lower partial pressure; verified in the source's own text
  (abstract, in the check).
- Gale 1972b, _Elevation and transpiration: some theoretical considerations with special reference
  to Mediterranean-type climate_, Journal of Applied Ecology 9, 691, doi:10.2307/2401898: thin air
  losing more water per carbon; **not re-checked** (citation only, through Crossref).
- Hyvönen, Hedderson, Smith Merrill, Gibbings and Koskinen 1998, _On phylogeny of the
  Polytrichales_, The Bryologist 101, 489, doi:10.1639/0007-2745(1998)101[489:OPOTP]2.0.CO;2:
  "giants of Papua New Guinea like Dawsonia gigantea Geh. with stems reaching up to 80 cm", the
  tallest mosses; verified in the source's own text (its opening, as OpenAlex holds it, in the
  second review).
- Boyce et al. 2007, _Devonian landscape heterogeneity recorded by a giant fungus_, Geology 35, 399,
  doi:10.1130/G23384A.1: the fungus-like Prototaxites of several metres, the exception to low
  non-vascular stature; **not re-checked** (citation confirmed; the height unreachable,
  GeoScienceWorld returning 403 and Crossref and Semantic Scholar holding no abstract, and found
  only on Wikipedia).
- Hegde, Paulino-Lima, Kent, Kaltenegger and Rothschild 2015, _Surface biosignatures of exo-Earths:
  remote detection of extraterrestrial life_, PNAS 112, 3886, doi:10.1073/pnas.1421237112: red-edged
  vegetation widespread for only about 460 Myr, and the spectra of 137 microorganisms; verified in
  the source's own text (abstract).
- Stein et al. 2020, _Mid-Devonian Archaeopteris roots signal revolutionary change in earliest
  fossil forests_, Current Biology 30, 421, doi:10.1016/j.cub.2019.11.067: forests by the Mid
  Devonian, 393–383 Ma, for the stature bound's growth over the `Land` stage; verified in the
  source's own text (in the check).
- Kiang et al. 2007b, _Spectral signatures of photosynthesis. II. Coevolution with other stars and
  the atmosphere on extrasolar worlds_, Astrobiology 7, 252, doi:10.1089/ast.2006.0108: peak pigment
  absorbance near 451 nm for an F2V host, 667 nm for K2V, 754 nm for M1V and 1,045 nm for M5V, M
  stars' bands at 0.93–1.1, 1.1–1.4, 1.5–1.8 and 1.8–2.5 µm, and underwater life below 1.4 µm,
  likely 1.1 µm; verified in the source's own text (abstract, and §3 in the check).
- Arp et al. 2020, _Quieting a noisy antenna reproduces photosynthetic light-harvesting spectra_,
  Science 368, 1490, doi:10.1126/science.aba6630: the noise-cancelling antenna, the pigment lean's
  cross-check, and Table 1's light niches (purple bacteria's 801 and 857 nm under a canopy,
  bacteriochlorophylls _c_ and _e_ under 2 m of seawater); verified in the source's own text
  (abstract, and Table 1 in the check).
- Duffy et al. 2023, _Photosynthesis under a red Sun: predicting the absorption characteristics of
  an extraterrestrial light-harvesting antenna_, MNRAS 526, 2265, doi:10.1093/mnras/stad2823: the
  noise-cancelling rule applied from 2,300 to 5,800 K, its solar optimum the blue Soret pair at 385
  and 415 nm (about 443 nm through air) with chlorophyll's red bands only local maxima, red
  chlorophyll-like bands near 3,300 K, bacteriochlorophyll _a_'s 800–850 nm at 2,800 K and about
  1,000 nm below 2,600 K; verified in the source's own text (abstract, and §3.1 and §4 in the
  check).
- Claudi et al. 2021, _Super-Earths, M dwarfs, and photosynthetic organisms: habitability in the
  lab_, Life 11, 10, doi:10.3390/life11010010: cyanobacteria making oxygen under a simulated M-dwarf
  spectrum, and far-red shade's chlorophylls _d_ and _f_ to 750 nm in the test battery; verified in
  the source's own text (abstract).
- DasSarma and Schwieterman 2021, _Early evolution of purple retinal pigments on Earth and
  implications for exoplanet biosignatures_, International Journal of Astrobiology 20, 241 (online
  2018), doi:10.1017/S1473550418000423: retinal phototrophy absorbing at 568 nm, in the gap
  chlorophyll leaves, and the speculative "Purple Earth"; verified in the source's own text
  (abstract).
- Coelho et al. 2024, _Purple is the new green: biopigments and spectra of Earth-like purple
  worlds_, MNRAS 530, 1363, doi:10.1093/mnras/stae601: purple bacteria colouring anoxic and oxic
  habitats alike; verified in the source's own text (abstract).
- Schwieterman, Cockell and Meadows 2015, _Nonphotosynthetic pigments as potential biosignatures_,
  Astrobiology 15, 341, doi:10.1089/ast.2014.1178: how often a surface's colour is not its light
  harvester's; verified in the source's own text (abstract).
- Jolly, Nemani and Running 2005, _A generalized, bioclimatic index to predict foliar phenology in
  response to climate_, Global Change Biology 11, 619, doi:10.1111/j.1365-2486.2005.00930.x: the
  growing-season index, a product of minimum-temperature, vapour-pressure-deficit and day-length
  indices, following satellite greenness at r > 0.8 across nine ecosystems and dating leaf flush to
  about 3 days at one temperate deciduous forest; verified in the source's own text (abstract; its
  thresholds not read).
- Lee, O'Keefe, Holbrook and Feild 2003, _Pigment dynamics and autumn leaf senescence in a New
  England deciduous forest, eastern USA_, Ecological Research 18, 677,
  doi:10.1111/j.1440-1703.2003.00588.x: "Yellow coloration was a result of the unmasking of
  xanthophyll pigments in senescing chloroplasts", the colours left when the primary pigment goes;
  verified in the source's own text (abstract, through OpenAlex, in the second review).
- Ryther 1969, _Photosynthesis and fish production in the sea_, Science 166, 72,
  doi:10.1126/science.166.3901.72: the pre-satellite ordering, open ocean below coast below
  upwelling, a test of order alone; unreachable (Science paywalled; no abstract on Crossref or
  Semantic Scholar), its ocean total of about 20 Pg C a year verified via a secondary source
  (Jennings and Collingridge 2015, PLoS ONE 10, e0133794, doi:10.1371/journal.pone.0133794, full
  text: "Primary production of 20 × 10⁹ t C yr⁻¹").
- Morel and Maritorena 2001, _Bio-optical properties of oceanic waters: a reappraisal_, JGR Oceans
  106, 7163, doi:10.1029/2000JC000319: case 1 relations of reflectance to chlorophyll, rebuilt here
  from the pigment's own absorption; verified in the source's own text (abstract).
- Pope and Fry 1997, _Absorption spectrum (380–700 nm) of pure water. II. Integrating cavity
  measurements_, Applied Optics 36, 8710, doi:10.1364/AO.36.008710: pure water's absorption, from
  which R11 draws an Earth's ocean; verified in the source's own text (abstract, in the second
  review: 380–700 nm, a minimum of 0.0044 ± 0.0006 m⁻¹ at 418 nm), R11's tabulated values, which its
  Design note 12 quotes, not compared with the paper's table.
- Bréon, Maignan, Leroy and Grant 2002, _Analysis of hot spot directional signatures measured from
  space_, JGR Atmospheres 107, doi:10.1029/2001JD001094: POLDER's "first quantitative observation of
  the hot spot signature of vegetated surfaces", "Most of the measured hot spot half widths are
  between 1° and 2°", the canopy's backscatter row; verified in the source's own text (abstract,
  through OpenAlex, in the second review).
- West, Enquist and Brown 2009, _A general quantitative theory of forest structure and dynamics_,
  PNAS 106, 7040, doi:10.1073/pnas.0812294106: stems falling as r⁻² in linear bins of radius, for
  the flora's octaves; verified in the source's own text (abstract, and the body in the check).
- Körner and Paulsen 2004, _A world-wide study of high altitude treeline temperatures_, Journal of
  Biogeography 31, 713, doi:10.1111/j.1365-2699.2003.01043.x: high-altitude treelines at a seasonal
  mean root-zone temperature of 6.7 ± 0.8 °C, the closed-canopy test, and with the dry woodland edge
  the fit of the maintenance cost on standing rigid biomass; verified in the source's own text
  (abstract).

**Organisms and ecology**

- Donley, Sepulveda, Konstantinidis, Gemballa and Shadwick 2004, _Convergent evolution in mechanical
  design of lamnid sharks and tunas_, Nature 429, 61, doi:10.1038/nature02435: one thunniform body
  in whales, ichthyosaurs, lamnid sharks and tunas, convergence forced by physics; verified in the
  source's own text (abstract).
- Maina 2000, _What it takes to fly: the structural and functional respiratory refinements in birds
  and bats_, Journal of Experimental Biology 203, 3045, doi:10.1242/jeb.203.20.3045: "the
  development of flight in only four animal taxa"; verified in the source's own text (abstract).
- Wiens and Graham 2005, _Niche conservatism: integrating evolution, ecology, and conservation
  biology_, Annual Review of Ecology, Evolution, and Systematics 36, 519,
  doi:10.1146/annurev.ecolsys.36.102803.095431: the inherited niche as phylogenetic niche
  conservatism; verified in the source's own text (abstract, in the second review: "the tendency of
  species to retain ancestral ecological characteristics").
- Kendall 1948, _On the generalized "birth-and-death" process_, Annals of Mathematical Statistics
  19, 1, doi:10.1214/aoms/1177730285: one lineage's descendants under a constant-rate birth–death
  process as a zero term plus a geometric, the clade's child count; the paper unreachable (Project
  Euclid's bot wall), the result reproduced here by simulation (λ = 1, μ = 0.5, t = 2, 2 × 10⁵ runs:
  P(0) 0.3870 against 0.3873, and n = 1–5 within 0.0014).
- Yule 1925, _A mathematical theory of evolution, based on the conclusions of Dr. J. C. Willis_,
  Philosophical Transactions of the Royal Society B 213, 21, doi:10.1098/rstb.1925.0002: the hollow
  curve of species per genus; **from memory** for the hollow curve (the abstract, read in the second
  review, confirms only the work's basis in Willis's conclusions).
- Mora et al. 2011, _How many species are there on Earth and in the ocean?_, PLoS Biology 9,
  e1001127, doi:10.1371/journal.pbio.1001127: higher classification that "follows a consistent and
  predictable pattern", and 8.7 ± 1.3 million eukaryote species, 86% undescribed; verified in the
  source's own text (abstract).
- Doolittle 1999, _Phylogenetic classification and the universal tree_, Science 284, 2124,
  doi:10.1126/science.284.5423.2124: prokaryotic genomes that "contain genes from multiple sources",
  so "the history of life cannot properly be represented as a tree", hence lateral transfer;
  verified in the source's own text (abstract).
- Louca et al. 2019, _A census-based estimate of Earth's bacterial and archaeal diversity_, PLoS
  Biology 17, e3000106, doi:10.1371/journal.pbio.3000106: 0.8–1.6 million prokaryotic taxa at 97%
  similarity over the 16S V4 region from 1.7 × 10⁹ 16S sequences, and about 2.2–4.3 million over the
  gene's full length, the default richness, refuting "trillions"; most microbial taxa "globally
  distributed"; log-normal abundances; verified in the source's own text (abstract).
- Locey and Lennon 2016, _Scaling laws predict global microbial diversity_, PNAS 113, 5970,
  doi:10.1073/pnas.1521291113: an extrapolation of upward of 10¹² microbial species, the parameter's
  upper bound; verified in the source's own text (abstract).
- Colwell and Coddington 1994, _Estimating terrestrial biodiversity through extrapolation_,
  Philosophical Transactions of the Royal Society B 345, 101, doi:10.1098/rstb.1994.0091: local
  richness extrapolated "using non-parametric techniques based on the distribution of individuals
  among species or of species among samples", a survey's estimate of the taxa not yet seen; verified
  in the source's own text (abstract, through Europe PMC, in the second review).
- Drakare, Lennon and Hillebrand 2006, _The imprint of the geographical, evolutionary and ecological
  context on species–area relationships_, Ecology Letters 9, 215,
  doi:10.1111/j.1461-0248.2005.00848.x: 794 species–area relations, slopes steeper at low latitudes
  and for larger organisms and dependent on scale and sampling, so z is a parameter; verified in the
  source's own text (abstract).
- MacArthur and Wilson 1967, _The Theory of Island Biogeography_, Princeton University Press
  (reprinted 2001, doi:10.1515/9781400881376): island species–area slopes of 0.20–0.35; verified via
  a secondary source (Gao and Perry 2016).
- Rosenzweig 1995, _Species Diversity in Space and Time_, Cambridge University Press,
  doi:10.1017/CBO9780511623387: island slopes narrowed to 0.25–0.33, behind the default z ≈ 0.25;
  verified via a secondary source (Gao and Perry 2016).
- Lomolino and Weiser 2001, _Towards a more general species–area relationship: diversity on all
  islands, great and small_, Journal of Biogeography 28, 431, doi:10.1046/j.1365-2699.2001.00550.x:
  with Rosenzweig 2004, the slopes by scale, 0.10–0.20 on small islands, 0.25–0.45 on intermediate
  ones and above 0.60 where speciation fills large ones, as Gao and Perry 2016 give them; verified
  via a secondary source (Gao and Perry 2016), the reference from Crossref.
- Rosenzweig 2004, _Applying species–area relationships to the conservation of diversity_, in
  Lomolino and Heaney (eds.), _Frontiers of Biogeography: New Directions in the Geography of
  Nature_, Sinauer Associates, 325–343: with Lomolino and Weiser 2001, the slopes by scale, as Gao
  and Perry 2016 give them; verified via a secondary source (Gao and Perry 2016, whose reference
  list gives the title and pages).
- Gao and Perry 2016, _Species–area relationships and additive partitioning of diversity of native
  and nonnative herpetofauna of the West Indies_, Ecology and Evolution 6, 7742,
  doi:10.1002/ece3.2511: the secondary source for MacArthur and Wilson's island slopes of 0.20–0.35
  and Rosenzweig's 0.25–0.33, "a standard reference for numerous studies", and for slopes by scale,
  0.10–0.20 on small islands, 0.25–0.45 on intermediate ones and above 0.60 where speciation fills
  large ones, after Lomolino and Weiser 2001 and Rosenzweig 2004; verified in the source's own text
  (full text, introduction, through Europe PMC, PMC6093157). The same authors' _Detecting the small
  island effect and nestedness of herpetofauna of the West Indies_ (Ecology and Evolution 6, 5390,
  doi:10.1002/ece3.2289) gives the same island ranges.
- Allen, Brown and Gillooly 2002, _Global biodiversity, biochemical kinetics, and the
  energetic-equivalence rule_, Science 297, 1545, doi:10.1126/science.1072380: ectotherm richness
  rising as e^(−E/kT), the kinetic prediction of the latitudinal gradient; verified in the source's
  own text (abstract, in the research).
- Hawkins et al. 2007, _A global evaluation of metabolic theory as an explanation for terrestrial
  species richness gradients_, Ecology 88, 1877, doi:10.1890/06-1444.1: only one of 46 terrestrial
  data sets fitting it, the theory "a poor predictor of observed diversity gradients", hence the
  factor a below 1; verified in the source's own text (in the research).
- Brook, Traill and Bradshaw 2006, _Minimum viable population sizes and global extinction risk are
  unrelated_, Ecology Letters 9, 375, doi:10.1111/j.1461-0248.2006.00883.x: the median minimum
  viable population of 1,377, the pool's pruning threshold; verified in the source's own text
  (abstract).
- Burness, Diamond and Flannery 2001, _Dinosaurs, dragons, and dwarfs: the evolution of maximal body
  size_, PNAS 98, 14518, doi:10.1073/pnas.251548698: the largest species' size rising with land
  area; top ectotherms outweighing top endotherms on a given area; the largest dinosaurs exceeding
  the prediction, so the support limit is a ceiling; and a top herbivore eating about eight times
  what a top carnivore does; verified in the source's own text (abstract).
- Holt et al. 2013, _An update of Wallace's zoogeographic regions of the world_, Science 339, 74,
  doi:10.1126/science.1228282: Earth's vertebrates in 20 regions within 11 realms, the macroscopic
  pools' realms; verified in the source's own text (abstract).
- ter Steege et al. 2013, _Hyperdominance in the Amazonian tree flora_, Science 342, 1243092,
  doi:10.1126/science.1243092: 227 of some 16,000 Amazonian tree species making half of all trees, a
  ranked abundance fitted by Fisher's log-series, for steeply uneven abundances; verified in the
  source's own text (abstract, and the log-series fit in the main text, through the Naturalis
  repository, in the second review).
- Flemming and Wuertz 2019, _Bacteria and archaea on Earth and their abundance in biofilms_, Nature
  Reviews Microbiology 17, 247, doi:10.1038/s41579-019-0158-9: about 1.2 × 10³⁰ prokaryotic cells, 4
  × 10²⁹ in the deep oceanic subsurface, 3 × 10²⁹ each in the deep continental subsurface and soil,
  1 × 10²⁹ in the ocean and 5 × 10²⁸ in the upper oceanic sediment, to which sample densities are
  calibrated; verified in the source's own text (in the check; not re-read in the research).
- Friedmann 1982, _Endolithic microorganisms in the Antarctic cold desert_, Science 215, 1045,
  doi:10.1126/science.215.4536.1045: endoliths in "a narrow subsurface zone" of porous desert rock;
  verified in the source's own text (abstract).
- Jain et al. 2018, _High throughput ANI analysis of 90K prokaryotic genomes reveals clear species
  boundaries_, Nature Communications 9, 5114, doi:10.1038/s41467-018-07641-9: 99.8% of 8 × 10⁹
  genome pairs above 95% average nucleotide identity within a species or below 83% between, so
  prokaryotic species are a real unit; verified in the source's own text (abstract).
- Biewener 1989, _Scaling body support in mammals: limb posture and muscle mechanics_, Science 245,
  45, doi:10.1126/science.2740914: peak bone stress nearly independent of size, achieved by posture;
  verified in the source's own text (abstract).
- Biewener 2005, _Biomechanical consequences of scaling_, Journal of Experimental Biology 208, 1665,
  doi:10.1242/jeb.01520: stresses fairly constant from 0.04 to 300 kg as effective mechanical
  advantage goes as W^0.25, with more skeletal allometry and less agility above; verified in the
  source's own text (abstract).
- Sato et al. 2009, _Scaling of soaring seabirds and implications for flight abilities of giant
  pterosaurs_, PLoS ONE 4, e5400, doi:10.1371/journal.pone.0005400: a soaring limit near 41 kg and
  5.1 m span, and the flapping-frequency model (∝ M^(−1/3)) behind the flight limit's exponents;
  verified in the source's own text (abstract).
- Chatterjee, Templin and Campbell 2007, _The aerodynamics of Argentavis, the world's largest flying
  bird from the Miocene of Argentina_, PNAS 104, 12398, doi:10.1073/pnas.0702040104: _Argentavis_ at
  about 70 kg unable to sustain flapping; verified in the source's own text (abstract).
- Witton and Habib 2010, _On the size and flight diversity of giant pterosaurs, the use of birds as
  pterosaur analogues and comments on pterosaur flightlessness_, PLoS ONE 5, e13982,
  doi:10.1371/journal.pone.0013982: pterosaurs of 200–250 kg that flew, the flight limit's
  calibration; verified in the source's own text (abstract).
- Kaiser et al. 2007, _Increase in tracheal investment with beetle size supports hypothesis of
  oxygen limitation on insect gigantism_, PNAS 104, 13198, doi:10.1073/pnas.0611544104: beetle
  tracheae scaling as M^1.29; verified in the source's own text (abstract).
- Clapham and Karr 2012, _Environmental and biotic controls on the evolutionary history of insect
  body size_, PNAS 109, 10927, doi:10.1073/pnas.1204026109: insect size tracking O₂ for 150 Myr,
  then decoupling as birds radiated; verified in the source's own text (abstract).
- Harrison, Kaiser and VandenBrooks 2010, _Atmospheric oxygen level and the evolution of insect body
  size_, Proceedings of the Royal Society B 277, 1937, doi:10.1098/rspb.2010.0001: the oxygen–size
  link that can be neither accepted nor rejected, so it is labelled contested; verified in the
  source's own text (abstract).
- Douglas, Mullineaux and Partridge 2000, _Long-wave sensitivity in deep-sea stomiid dragonfish with
  far-red bioluminescence: evidence for a dietary origin of the chlorophyll-derived retinal
  photosensitizer of Malacosteus niger_, Philosophical Transactions of the Royal Society B 355,
  1269, doi:10.1098/rstb.2000.0681: deep-sea fish pigments clustering at 450–500 nm, where the light
  is; verified in the source's own text (abstract).
- Luo, Yue, Ala-Laurila and Yau 2011, _Activation of visual pigments by light and heat_, Science
  332, 1307, doi:10.1126/science.1200172: a pigment's thermal noise rising with its peak wavelength,
  which "presumably explains" why infrared ones do not exist; verified in the source's own text
  (abstract).
- Bowmaker and Dartnall 1980, _Visual pigments of rods and cones in a human retina_, Journal of
  Physiology 298, 501, doi:10.1113/jphysiol.1980.sp013097: long-wave cones peaking at 562.8 ± 4.7 nm
  (n = 19), middle-wave at 533.8 nm, short-wave at 420.3 nm and rods at 497.6 nm; verified in the
  source's own text (abstract, through Europe PMC, in the second review).
- Gillooly, Brown, West, Savage and Charnov 2001, _Effects of size and temperature on metabolic
  rate_, Science 293, 2248, doi:10.1126/science.1061967: metabolism ∝ M^(3/4)·e^(−E/kT) with E
  averaging 0.62 eV; verified in the source's own text (in the research; not re-read in the check).
- Purcell 1977, _Life at low Reynolds number_, American Journal of Physics 45, 3,
  doi:10.1119/1.10903: swimming by cilia and flagella below a Reynolds number near 1; **from
  memory**.
- Kram, Domingo and Ferris 1997, _Effect of reduced gravity on the preferred walk-run transition
  speed_, Journal of Experimental Biology 200, 821, doi:10.1242/jeb.200.4.821: the walk–run
  transition near a Froude number of 0.5 for bipeds, the same in reduced gravity; verified in the
  source's own text (abstract).
- Kleiber 1932, _Body size and metabolism_, Hilgardia 6, 315, doi:10.3733/hilg.v06n11p315:
  metabolism ∝ M^(3/4), about 72 kcal a day per kg^(3/4) across his groups (Table 1's mean of 13,
  71.8), a slow test; verified in the source's own text (in the research; the check could not reach
  it).
- White and Seymour 2003, _Mammalian basal metabolic rate is proportional to body mass^2/3_, PNAS
  100, 4046, doi:10.1073/pnas.0436428100: mammalian basal rate nearer M^(2/3) over 619 species;
  verified in the source's own text (in the research).
- Glazier 2005, _Beyond the '3/4-power law': variation in the intra- and interspecific scaling of
  metabolic rate in animals_, Biological Reviews 80, 611, doi:10.1017/S1464793105006834: "most
  metabolic scaling exponents are between 2/3 and 1"; verified in the source's own text (in the
  research; not re-read in the check).
- Damuth 1981, _Population density and body size in mammals_, Nature 290, 699, doi:10.1038/290699a0:
  log D = 4.23 − 0.75 log M for mammalian primary consumers, within about an order of magnitude,
  from which the 95 per km² of a 1 kg herbivore and 3 of a 100 kg one are computed, and a test;
  verified in the source's own text (Fig. 1, in the research; "about an order of magnitude" not
  re-read in the check).
- Damuth 1987, _Interspecific allometry of population density in mammals and other animals: the
  independence of body mass and population energy-use_, Biological Journal of the Linnean Society
  31, 193, doi:10.1111/j.1095-8312.1987.tb01990.x: a population's energy use independent of body
  mass; verified in the source's own text (abstract).
- McNaughton, Oesterheld, Frank and Williams 1989, _Ecosystem-level patterns of primary productivity
  and herbivory in terrestrial habitats_, Nature 341, 142, doi:10.1038/341142a0: consumption on land
  growing as aboveground productivity to the 1.38 power, which the budget caps; verified in the
  source's own text (in the research; not re-read in the check).
- Cyr and Pace 1993, _Magnitude and patterns of herbivory in aquatic and terrestrial ecosystems_,
  Nature 361, 148, doi:10.1038/361148a0: aquatic herbivores taking on average 51% of primary
  production, three times terrestrial ones' share; verified in the source's own text (in the
  research).
- Pauly and Christensen 1995, _Primary production required to sustain global fisheries_, Nature 374,
  255, doi:10.1038/374255a0: a mean transfer of 10% a level in aquatic systems; verified in the
  source's own text (in the research).
- Eddy et al. 2021, _Energy flow through marine ecosystems: confronting transfer efficiency_, Trends
  in Ecology and Evolution 36, 76, doi:10.1016/j.tree.2020.09.006: marine transfer efficiencies from
  below 1% to 52%; verified in the source's own text (the accepted manuscript, in the research; not
  re-read in the check).
- Wu et al. 2026, _Global synthesis reveals systematic variation in trophic transfer efficiency
  across and within ecosystems_, Science Advances 12, eaeg4002, doi:10.1126/sciadv.aeg4002: 2,052
  estimates averaging 5.9%, 8.1% at sea and 1.5% on land, dominated by plant-to-herbivore links on
  data "substantially underrepresented", consumers of heterotrophs at 7.2% against 3.5% for
  consumers of autotrophs, and on land endotherms at 0.24%, for their "inherently low production
  efficiencies", against invertebrates' 3.8%, so ε depends on the link; verified in the source's own
  text (in the research and the check, and the full text, PMC13330895, in the second review; recent,
  worth a second reading before it becomes a default).
- Hatton et al. 2015, _The predator-prey power law: biomass scaling across terrestrial and aquatic
  biomes_, Science 349, aac6284, doi:10.1126/science.aac6284: predator biomass going as prey biomass
  to near the ¾ power; verified in the source's own text (abstract).
- Carbone and Gittleman 2002, _A common rule for the scaling of carnivore density_, Science 295,
  2273, doi:10.1126/science.1067994: 10,000 kg of prey supporting about 90 kg of one carnivore
  species, a test; verified in the source's own text (abstract).
- Carbone, Mace, Roberts and Macdonald 1999, _Energetic constraints on the diet of terrestrial
  carnivores_, Nature 402, 286, doi:10.1038/46266: carnivores switching to prey near their own mass
  at 21.5–25 kg, which makes any carnivore of a person's size dangerous; verified in the source's
  own text (abstract).
- Sheldon, Prakash and Sutcliffe 1972, _The size distribution of particles in the ocean_, Limnology
  and Oceanography 17, 327, doi:10.4319/lo.1972.17.3.0327: biomass roughly constant per logarithmic
  size class from bacteria to whales; verified in the source's own text (in the research).
- Hatton, Heneghan, Bar-On and Galbraith 2021, _The global ocean size spectrum from bacteria to
  whales_, Science Advances 7, eabh3732, doi:10.1126/sciadv.abh3732: near 1 Gt wet weight "within
  most order of magnitude size classes", bacteria above it and large marine mammals below; verified
  in the source's own text (in the research, and the abstract, through Europe PMC, in the second
  review).
- Savage, Gillooly, Brown, West and Charnov 2004, _Effects of body size and temperature on
  population growth_, American Naturalist 163, 429, doi:10.1086/381872: carrying capacity falling
  with temperature, so warm ectotherms are fewer on the same supply; verified in the source's own
  text (abstract).
- Stalmans et al. 2019, _War-induced collapse and asymmetric recovery of large-mammal populations in
  Gorongosa National Park, Mozambique_, PLoS ONE 14, e0212864, doi:10.1371/journal.pone.0212864:
  large herbivores at more than 7,500–9,000 kg/km² in Ngorongoro, the Serengeti and pre-war
  Gorongosa (1968–1972), the productive-savanna test; verified in the source's own text (in the
  research; its Ngorongoro and Serengeti figures are its citations of Coe, Cumming and Phillipson
  1976).
- Coe, Cumming and Phillipson 1976, _Biomass and production of large African herbivores in relation
  to rainfall and primary production_, Oecologia 22, 341, doi:10.1007/BF00345312: the Ngorongoro and
  Serengeti herbivore biomasses Stalmans et al. 2019 cite; verified via a secondary source (Stalmans
  et al. 2019), the reference from Crossref.
- Speakman 2005, _Body size, energy metabolism and lifespan_, Journal of Experimental Biology 208,
  1717, doi:10.1242/jeb.01556: lifespan scaling as M^0.15 to M^0.3, the generation count; verified
  in the source's own text (abstract).
- Lindstedt, Miller and Buskirk 1986, _Home range, time, and body size in mammals_, Ecology 67, 413,
  doi:10.2307/1938584: home range scaling with an exponent near 1.0 for herbivores and carnivores
  alike; verified in the source's own text (abstract, through Europe PMC).
- Kelt and Van Vuren 2001, _The ecology and macroecology of mammalian home range area_, American
  Naturalist 157, 637, doi:10.1086/320621: home-range slopes steeper than energetics predicts;
  verified in the source's own text (abstract).
- Jetz, Carbone, Fulford and Brown 2004, _The scaling of animal space use_, Science 306, 266,
  doi:10.1126/science.1102138: neighbours overlapping more as size grows; verified in the source's
  own text (abstract).
- Carbone, Cowlishaw, Isaac and Rowcliffe 2005, _How far do animals go? Determinants of day range in
  mammals_, American Naturalist 165, 290, doi:10.1086/426790: day range near M^(1/4), steepest in
  faunivores; verified in the source's own text (abstract).
- Stankowich and Blumstein 2005, _Fear in animals: a meta-analysis and review of risk assessment_,
  Proceedings of the Royal Society B 272, 2627, doi:10.1098/rspb.2005.3251: a threat's size, speed
  and directness consistently amplifying flight initiation distance, and larger groups fleeing
  sooner except fish: "While fish tolerated closer approach when in larger schools, other taxa had
  greater flight initiation distances when in larger groups"; verified in the source's own text
  (abstract).
- Fotowat and Gabbiani 2011, _Collision detection as a model for sensory-motor integration_, Annual
  Review of Neuroscience 34, 1, doi:10.1146/annurev-neuro-061010-113632: escape from looming objects
  widespread among sighted animals, so flight assumes nothing of alien minds; verified in the
  source's own text (abstract).
- Milton, Milton and Kent 1992, _Total chemical synthesis of a D-enzyme: the enantiomers of HIV-1
  protease show reciprocal chiral substrate specificity_, Science 256, 1445,
  doi:10.1126/science.1604320: enantiomeric enzymes that "cut only the corresponding substrate
  enantiomer"; verified in the source's own text (abstract).
- Chipinda, Hettick and Siegel 2011, _Haptenation: chemical reactivity and protein binding_, Journal
  of Allergy 2011, 839682, doi:10.1155/2011/839682: small reactive molecules sensitising by bonding
  covalently to proteins, potency set largely by the rate of binding; verified in the source's own
  text (abstract).
- Summers and Clough 2001, _The evolution of coloration and toxicity in the poison frog family
  (Dendrobatidae)_, PNAS 98, 6227, doi:10.1073/pnas.101134898: conspicuousness evolving with
  toxicity, the warning-colour heuristic; verified in the source's own text (abstract).

**Biosignatures and detection**

- Sagan, Thompson, Carlson, Gurnett and Hord 1993, _A search for life on Earth from the Galileo
  spacecraft_, Nature 365, 715, doi:10.1038/365715a0: the control experiment, "abundant gaseous
  oxygen, a widely distributed surface pigment with a sharp absorption edge in the red part of the
  visible spectrum, and atmospheric methane in extreme thermodynamic disequilibrium", and the radio
  transmission "uniquely attributable to intelligence", for the technosignature hook; verified in
  the source's own text (abstract, through Europe PMC).
- Schwieterman et al. 2018, _Exoplanet biosignatures: a review of remotely detectable signs of
  life_, Astrobiology 18, 663, doi:10.1089/ast.2017.1729: a short-lived gas as a signature because
  its abundance needs a flux; N₂O's abiotic sources (chemodenitrification, lightning, energetic
  particles about young or active stars, §4.2.4), CH₃Cl's (biomass burning, possibly volcanoes,
  §4.2.6), DMS from marine phytoplankton (§4.2.5) and the haze at CH₄/CO₂ "about 0.2" (§4.2.7);
  verified in the source's own text (abstract, and the sections in the check).
- Thompson, Krissansen-Totton, Wogan, Telus and Fortney 2022, _The case and context for atmospheric
  methane as an exoplanet biosignature_, PNAS 119, e2117933119, doi:10.1073/pnas.2117933119:
  methane's life under about 1 Myr about a Sun-like star; Earth's emissions of about 30 Tmol/yr
  "predominantly produced directly by life (including anthropogenic sources)" against "comparatively
  tiny" abiotic ones; a flux above about 10 Tmol/yr with CO/CH₄ < 1; and the abiotic fluxes it
  compiles (vents 0.18 and 0.35 Tmol/yr on 1 and 5 M⊕ planets, Hadean estimates about 1.5 and Hadean
  impacts about 1.25 Tmol/yr); verified in the source's own text (full text and SI).
- Meadows et al. 2018, _Exoplanet biosignatures: understanding oxygen as a biosignature in the
  context of its environment_, Astrobiology 18, 630, doi:10.1089/ast.2017.1727: O₂ at high abundance
  "originally thought to be an unambiguous indicator for life", in the published abstract's words,
  sinks that may have kept it from Earth's air for billions of years, and the discriminating context
  (water present, no O₄, no CO); verified in the source's own text (abstract, and §6 in the check).
- Schwieterman et al. 2016, _Identifying planetary biosignature impostors: spectral features of CO
  and O₄ resulting from abiotic O₂/O₃ production_, ApJL 819, L13, doi:10.3847/2041-8205/819/1/L13:
  O₄ at 1.06 and 1.27 µm, the sign of an O₂ atmosphere "too massive to be biologically produced", CO
  at 2.35 and 4.6 µm beside CO₂, and the O₂ A band at 0.76 µm; verified in the source's own text
  (abstract, and §3 in the check).
- Olson et al. 2018, _Atmospheric seasonality as an exoplanet biosignature_, ApJL 858, L14,
  doi:10.3847/2041-8213/aac171: O₃ responding to O₂ far below today's, Mauna Loa's few ppmv of
  seasonal CO₂ (Fig. 1), and a 5–25 ppm O₂ oscillation giving O₃ seasonality at S/N about 6 but
  "unobservable in the O₂-A band for S/N < 375", and, in §5, that "transit spectroscopy samples the
  same seasonal view" while reflected light sees seasons best face-on; verified in the source's own
  text (full text).
- Krissansen-Totton, Olson and Catling 2018, _Disequilibrium biosignatures over Earth history and
  implications for detecting exoplanet life_, Science Advances 4, eaao5747,
  doi:10.1126/sciadv.aao5747: CH₄ above 10⁻³ "potentially biogenic" and above 10⁻² "likely
  biogenic", stronger without CO; and its supplement's section S6, the anoxic CH₄ steady state,
  f_CH₄ ≈ F_CH₄ ÷ 6,680 Tmol/yr from diffusion-limited hydrogen escape, "expected to apply to all
  habitable exoplanets with anoxic atmospheres", which request 7 asks of plan 14; verified in the
  source's own text (abstract; section S6 in the preprint arXiv:1801.08211, which carries the
  supplement, in the second review).
- Ranjan et al. 2020, _Photochemistry of anoxic abiotic habitable planet atmospheres: impact of new
  H₂O cross sections_, ApJ 896, 148, doi:10.3847/1538-4357/ab9363: new water cross-sections that
  remove the low-outgassing O₂ false positive about Sun-like stars and may suppress abiotic CO too;
  verified in the source's own text (abstract).
- Wogan and Catling 2020, _When is chemical disequilibrium in Earth-like planetary atmospheres a
  biosignature versus an anti-biosignature? Disequilibria from dead to living worlds_, ApJ 892, 127,
  doi:10.3847/1538-4357/ab7b81: CO or H₂ beside water as an anti-biosignature, food a biosphere
  would eat; verified in the source's own text (abstract, arXiv:1911.06852, in the second review).
- Airapetian et al. 2016, _Prebiotic chemistry and atmospheric warming of early Earth by an active
  young Sun_, Nature Geoscience 9, 452, doi:10.1038/ngeo2719: abiotic N₂O from energetic particles
  about young or active stars; verified via a secondary source (Schwieterman et al. 2018, §4.2.4).
- Segura et al. 2005, _Biosignatures from Earth-like planets around M dwarfs_, Astrobiology 5, 706,
  doi:10.1089/ast.2005.5.706: N₂O and CH₃Cl living longer and accumulating further under an M
  dwarf's ultraviolet; verified in the source's own text (abstract).
- Hänni et al. 2024, _Evidence for abiotic dimethyl sulfide in cometary matter_, ApJ 976, 74,
  doi:10.3847/1538-4357/ad8565: evidence of abiotic DMS in comet 67P; verified in the source's own
  text (abstract).
- Krissansen-Totton, Garland, Irwin and Catling 2018, _Detectability of biosignatures in anoxic
  atmospheres with the James Webb Space Telescope: a TRAPPIST-1e case study_, AJ 156, 114,
  doi:10.3847/1538-3881/aad564: about ten transits with NIRSpec's prism that "may be sufficient to
  detect carbon dioxide and constrain methane abundances", with a CO limit; verified in the source's
  own text (abstract).
- Madhusudhan et al. 2025, _New constraints on DMS and DMDS in the atmosphere of K2-18 b from JWST
  MIRI_, ApJL 983, L40, doi:10.3847/2041-8213/adc1c8: the claim of DMS or DMDS at 3σ; verified in
  the source's own text (abstract).
- Stevenson et al. 2025, _K2-18b does not meet the standards of evidence for life_, AJ 170, 257,
  doi:10.3847/1538-3881/ae0338: "87.5% of retrievals using our preferred MIRI binning scheme do not"
  favour them, blamed on red noise; verified in the source's own text (abstract).
- Greaves et al. 2021, _Phosphine gas in the cloud decks of Venus_, Nature Astronomy 5, 655 (online
  2020), doi:10.1038/s41550-020-1174-4: about 20 ppb of PH₃ in Venus's clouds, with no abiotic
  source among those examined; verified in the source's own text (abstract, arXiv:2009.06593, in the
  second review).
- Villanueva et al. 2021, _No evidence of phosphine in the atmosphere of Venus from independent
  analyses_, Nature Astronomy 5, 631, doi:10.1038/s41550-021-01422-z: an SO₂ line "only 1.3 km/s
  away from the PH3 line", at spectral resolutions comparable to the separation; verified in the
  source's own text (abstract, in the second review).
- Cordiner et al. 2022, _Phosphine in the Venusian atmosphere: a strict upper limit from SOFIA GREAT
  observations_, GRL 49, e2022GL101055, doi:10.1029/2022GL101055: an upper limit of 0.8 ppb at
  75–110 km; verified in the source's own text (abstract, arXiv:2210.13519, in the second review).
- Greaves et al. 2023, _Comment on "Phosphine in the Venusian atmosphere: a strict upper limit from
  SOFIA GREAT observations" by Cordiner et al._, GRL 50, e2023GL103539, doi:10.1029/2023GL103539: "a
  5.7σ candidate detection, suggesting approx. 3 ppb" in the same SOFIA data; verified in the
  source's own text (abstract, arXiv:2211.09852, in the second review). The authors' reply (Cordiner
  et al. 2023, GRL 50, e2023GL106136, doi:10.1029/2023GL106136) was not read.
- Limaye et al. 2018, _Venus' spectral signatures and the potential for life in the clouds_,
  Astrobiology 18, 1181, doi:10.1089/ast.2017.1783: the clouds' ultraviolet absorbers "still
  unknown", with microorganisms discussed among their candidates; verified in the source's own text
  (abstract, through Europe PMC, in the second review).
- Trainer et al. 2006, _Organic haze on Titan and the early Earth_, PNAS 103, 18035,
  doi:10.1073/pnas.0608561103: the haze threshold CH₄/CO₂ ≳ 0.1–0.2 that plan 14 cites; verified via
  a secondary source (Schwieterman et al. 2018, §4.2.7, and plan 14's P14.T24.c).
- Seager, Turner, Schafer and Ford 2005, _Vegetation's red edge: a possible spectroscopic
  biosignature of extraterrestrial plants_, Astrobiology 5, 372, doi:10.1089/ast.2005.5.372: "a
  sharp order-of-magnitude increase in leaf reflectance between approximately 700 and 750 nm", seen
  in Earthshine, with time variability essential and some mineral edges similar, Earth's
  disc-integrated edge "weak (a few to ten percent vs. the >∼ 50% in leaf reflectance)", the
  full-sun test and the surface signature; verified in the source's own text (abstract).
- Webster et al. 2018, _Background levels of methane in Mars' atmosphere show strong seasonal
  variations_, Science 360, 1093, doi:10.1126/science.aaq0131: Curiosity's 0.41 ± 0.16 ppbv at Gale
  with a "strong, repeatable seasonal variation" and spikes near 7 ppbv; verified in the source's
  own text (abstract).
- Korablev et al. 2019, _No detection of methane on Mars from early ExoMars Trace Gas Orbiter
  observations_, Nature 568, 517, doi:10.1038/s41586-019-1096-4: the Trace Gas Orbiter's limit of
  about 0.05 ppbv; verified in the source's own text (abstract).
- Schaefer, Wordsworth, Berta-Thompson and Sasselov 2016, _Predictions of the atmospheric
  composition of GJ 1132b_, ApJ 829, 63, doi:10.3847/0004-637X/829/2/63: a magma ocean that "absorbs
  at most ∼10% of the O2 produced" by water loss while "more than 90% is lost to space", the melt
  uptake P14.T53.a drafts; verified in the source's own text (abstract, through OpenAlex, in the
  second review), its FeO to FeO₁.₅ form only as plan 14 cites it.
- Tian, France, Linsky, Mauas and Vieytes 2014, _High stellar FUV/NUV ratio and oxygen contents in
  the atmospheres of potentially habitable planets_, EPSL 385, 22, doi:10.1016/j.epsl.2013.10.024:
  abiotic O₂ near 2–3 × 10⁻³ from CO₂ photolysis about K and M hosts; verified via a secondary
  source (Wordsworth and Pierrehumbert 2014).
- Harman, Schwieterman, Schottelkotte and Kasting 2015, _Abiotic O₂ levels on planets around F, G,
  K, and M stars: possible false positives for life?_, ApJ 812, 137,
  doi:10.1088/0004-637X/812/2/137: CO₂ photolysis as a false positive about K and especially M
  hosts, given weak sinks; verified in the source's own text (abstract).
- Gao et al. 2015, _Stability of CO₂ atmospheres on desiccated M dwarf exoplanets_, ApJ 806, 249,
  doi:10.1088/0004-637X/806/2/249: about 40% of a desiccated M-dwarf world's CO₂ turned to CO and O₂
  once its hydrogen falls below 1 ppm, which P14.T53.a drafts; verified in the source's own text
  (abstract, in the check).
- Rackham, Apai and Giampapa 2018, _The transit light source effect: false spectral features and
  incorrect densities for M-dwarf transiting planets_, ApJ 853, 122, doi:10.3847/1538-4357/aaa08c:
  spot contamination "more than 10× larger than transit depth changes expected for atmospheric
  features in rocky exoplanets"; verified in the source's own text (abstract).
- Kaltenegger and Traub 2009, _Transits of Earth-like planets_, ApJ 698, 519,
  doi:10.1088/0004-637X/698/1/519: a feature's signal-to-noise "on the order of unity or less per
  transit" with a 6.5 m telescope, so transits are co-added; verified in the source's own text
  (abstract).
- Bétrémieux and Kaltenegger 2014, _Impact of atmospheric refraction: how deeply can we probe
  exo-Earth's atmospheres during primary eclipse observations?_, ApJ 791, 7,
  doi:10.1088/0004-637X/791/1/7: refraction letting an Earth twin be probed to 1 bar "only around
  the coolest stars"; verified in the source's own text (abstract).
- Kasting et al. 2009, _Exoplanet characterization and the search for life_, Astro2010 white paper,
  arXiv:0911.2936: an Earth twin about 10⁻¹⁰ of its star in the visible and 10⁻⁷ in the thermal
  infrared; verified in the source's own text (full text, p. 4).
- Green et al. 2021, _Call for a framework for reporting evidence for life beyond Earth_, Nature
  598, 575, doi:10.1038/s41586-021-03804-9: the Confidence of Life Detection scale, offered "not as
  a prescription", its seven measurement indicators and six qualitative assessments each spanning
  two levels (Fig. 1), its principles, ALH84001 at levels 1–3 "but clearly not" higher, and level 4
  "a significant challenge"; verified in the source's own text (full text and Fig. 1).
- Neveu, Hays, Voytek, New and Schulte 2018, _The ladder of life detection_, Astrobiology 18, 1375,
  doi:10.1089/ast.2017.1773: the decision rules after which each signature's rules are written;
  verified in the source's own text (abstract).
- Catling et al. 2018, _Exoplanet biosignatures: a framework for their assessment_, Astrobiology 18,
  709, doi:10.1089/ast.2017.1737: a Bayesian posterior that joins the data's likelihoods with "Prior
  knowledge of factors influencing planetary inhabitation", mapped to "five confidence levels,
  ranging from 'very likely' (90-100%) to 'very unlikely' (<10%)", the alternative to an ordinal
  rung; verified in the source's own text (abstract, arXiv:1705.06381, in the second review).
- Lin, Gonzalez Abad and Loeb 2014, _Detecting industrial pollution in the atmospheres of earth-like
  exoplanets_, ApJL 792, L7, doi:10.1088/2041-8205/792/1/L7: JWST detecting or constraining CCl₃F
  and CF₄ at about ten times Earth's level in about 1.2 and 1.7 days for a planet transiting a white
  dwarf; verified in the source's own text (abstract, in the check).
- Moores et al. 2019, _Methane seasonal cycle at Gale Crater on Mars consistent with regolith
  adsorption and diffusion_, Nature Geoscience 12, 321, doi:10.1038/s41561-019-0313-y: Gale's
  seasonal methane read abiotically, as the regolith taking up and releasing it; **not re-checked**
  (title and metadata only, through Crossref; the text not read).
- García-Ruiz et al. 2003, _Self-assembled silica-carbonate structures and detection of ancient
  microfossils_, Science 302, 1194, doi:10.1126/science.1090163: inorganic filaments "identical to
  currently accepted biogenic materials", microscopy's trap; verified in the source's own text
  (abstract).
- McKay 2004, _What is life—and how do we search for it in other worlds?_, PLoS Biology 2, e302,
  doi:10.1371/journal.pbio.0020302: life's selective use of few building blocks; verified in the
  source's own text (full text, in the check).
- Marshall et al. 2021, _Identifying molecules as biosignatures with assembly theory and mass
  spectrometry_, Nature Communications 12, 3033, doi:10.1038/s41467-021-23258-x: molecular
  complexity measured by mass spectrometry; verified in the source's own text (abstract).
- Hazen et al. 2024, _Molecular assembly indices of mineral heteropolyanions: some abiotic molecules
  are as complex as large biomolecules_, Journal of the Royal Society Interface 21, 20230632,
  doi:10.1098/rsif.2023.0632: abiotic heteropolyanions with assembly indices from 2 to 21, so
  "values of molecular assembly indices ≥15 do not represent unambiguous biosignatures", mass
  spectrometry's trap; verified in the source's own text (abstract, through Europe PMC, in the
  second review).
- McCollom and Seewald 2006, _Carbon isotope composition of organic compounds produced by abiotic
  synthesis under hydrothermal conditions_, EPSL 243, 74, doi:10.1016/j.epsl.2006.01.027: abiotic
  synthesis fractionating isotopes too; **from memory** (the citation is confirmed on Crossref; the
  abstract was unreachable, ScienceDirect returning 403 and neither Crossref nor OpenAlex holding
  it; in the second review the WHOI repository's security check and the ADS firewall blocked it
  too).
- Quinn et al. 2013, _Perchlorate radiolysis on Mars and the origin of martian soil reactivity_,
  Astrobiology 13, 515, doi:10.1089/ast.2013.0999: Viking's reactive soil explained by radiolysed
  perchlorate, the metabolic assay's trap; verified in the source's own text (abstract).
- Benner 2017, _Detecting Darwinism from molecules in the Enceladus plumes, Jupiter's moons, and
  other planetary water lagoons_, Astrobiology 17, 840, doi:10.1089/ast.2016.1611: a Darwinian
  genetic polymer that "must be a 'polyelectrolyte'", with a small alphabet and homochirality, so
  finding it needs no alphabet; verified in the source's own text (abstract).
- Jaramillo-Botero et al. 2021, _Understanding hypervelocity sampling of biosignatures in space
  missions_, Astrobiology 21, 421, doi:10.1089/ast.2020.2301: organics in a fly-through kept best at
  3–5 km/s for bare amino and fatty acids and 4–6 km/s inside ice grains; verified in the source's
  own text (abstract, through Europe PMC, in the second review).

**Human physiology and planetary protection**

- West 2002, _Highest permanent human habitation_, High Altitude Medicine and Biology 3, 401,
  doi:10.1089/15270290260512882: over 7,000 people living permanently at up to 5,100 m and
  individuals for two years at 5,950 m, and La Rinconada's 10.3 kPa of inspired O₂; verified in the
  source's own text (abstract).
- 14 CFR 91.211, _Supplemental oxygen_, <https://www.ecfr.gov/current/title-14/section-91.211>: crew
  oxygen beyond 30 min above 12,500 ft (3,810 m) and at all times above 14,000 ft (4,270 m), the
  unacclimatised crew's default, whose 11.9 kPa inspired at 3,810 m (computed in the standard
  atmosphere) is `SHIRTSLEEVE`'s floor; verified in the source's own text (eCFR text through Cornell
  LII, in the check; the metric heights and ambient PO₂ are its conversions in the standard
  atmosphere).
- 14 CFR 25.1443(b), _Minimum mass flow of supplemental oxygen_,
  <https://www.ecfr.gov/current/title-14/section-25.1443>: demand equipment holding a mean tracheal
  PO₂ of 122 mmHg to 35,000 ft and 95% oxygen between 35,000 and 40,000 ft (12,200 m), where the
  standard atmosphere's 18.75 kPa gives 11.9 kPa inspired (computed), `MASK`'s floor; verified in
  the source's own text (eCFR, in the second review).
- NASA-STD-3001 Vol. 2 Rev. E (NASA 2025), _NASA Spaceflight Human-System Standard_, Volume 2,
  _Human Factors, Habitability, and Environmental Health_, NTRS 20250004555,
  <https://ntrs.nasa.gov/citations/20250004555>: Table 6.2-1's inspired PO₂ limits (a floor of
  16.9 kPa indefinitely with monitoring, 16.3 kPa absolute, 47.5 kPa indefinitely and 105.5 kPa for
  6–9 h); [V2 6006]'s 26.2 kPa for normoxia on pure O₂, its design range of 34.5–103 kPa and its
  rationale on fire, "Oxygen-rich atmospheres, however, present safety hazards because of their
  ability to feed fires"; [V2 6004]'s 0.40 kPa of CO₂ over an hour; [V2 6012]'s cabin range of
  18–27 °C; [V2 6050]'s call on JSC 20584's SMACs, the short-term ones "intended for off-nominal
  events"; [V2 6052]'s habitat dust, total under 3 mg/m³ and respirable under 1 mg/m³, and
  [V2 6053]'s lunar dust under 10 µm, below 0.3 mg/m³ as a time-weighted average for up to six
  months; [V2 9102] with Table 9.3-4's skin temperatures of 49 °C or more and 0 °C or less;
  [V2 6008]'s decompression limits, "DCS ≤ 15%" and "Prevent Type II DCS"; and §6.5's acceleration
  limits, set for seated and standing crews in vehicles; behind the landing-party table and the
  ship's habitat standard; verified in the source's own text (in the check, and the full text in the
  second review).
- West 1999, _Barometric pressures on Mt. Everest: new data and physiological significance_, Journal
  of Applied Physiology 86, 1062, doi:10.1152/jappl.1999.86.3.1062: 5.7 kPa of inspired O₂ on
  Everest "very near the limit for human survival", and the barometric fit used for La Rinconada;
  verified in the source's own text (abstract).
- Hoyt et al. 2025, _Revised guideline for central nervous system oxygen toxicity exposure limits
  when using an inspired PO₂ of 1.3 atmospheres_, Diving and Hyperbaric Medicine 55, 262,
  doi:10.28920/dhm55.3.262-270: the 1991 single-exposure limit of 180 min at 1.3 atm (132 kPa),
  relaxed to 240 min working plus 240 min resting for divers; verified in the source's own text
  (abstract).
- NIOSH, _Pocket Guide to Chemical Hazards_, entries for carbon dioxide, carbon monoxide, hydrogen
  sulfide, sulfur dioxide and ammonia, <https://www.cdc.gov/niosh/npg/>: the recommended exposure
  limits (CO₂ 5,000 ppm as a 10 h average and 30,000 ppm for 15 min; CO, NH₃, H₂S and SO₂), the IDLH
  escape figures on a 30-minute basis (CO₂ 40,000, CO 1,200, NH₃ 300, H₂S and SO₂ 100 ppm) and
  OSHA's 8 h PEL for CO₂, converted at 101.325 kPa; verified in the source's own text (the entries
  in the research, the definitions in the check).
- JSC 20584 Rev. C (NASA 2024), _Spacecraft Maximum Allowable Concentrations for Airborne
  Contaminants_, NTRS 20240007713, <https://ntrs.nasa.gov/citations/20240007713>: the SMACs at 1 h,
  24 h, 7 d, 30 d, 180 d and 1,000 d (CO 425, 100, 55, 15, 15 and 15 ppm; NH₃ 30, 20, 3, 3, 3 and
  3 ppm; H₂S 5, 1.3, 1.3, 1.3 and 0.3 ppm, none set at 1,000 d), CO₂'s deleted, the limits for a
  stay beyond a working day, converted at 101.325 kPa; verified in the source's own text (full text,
  in the second review).
- Sherwood and Huber 2010, _An adaptability limit to climate change due to heat stress_, PNAS 107,
  9552, doi:10.1073/pnas.0913352107: a wet-bulb limit of 35 °C in theory; verified in the source's
  own text (abstract).
- Vecellio, Wolf, Cottle and Kenney 2022, _Evaluating the 35 °C wet-bulb temperature adaptability
  threshold for young, healthy subjects (PSU HEAT Project)_, Journal of Applied Physiology 132, 340,
  doi:10.1152/japplphysiol.00738.2021: 30.6 ± 1.0 °C measured in young adults in humid heat;
  verified in the source's own text (abstract).
- ICNIRP 2004, _Guidelines on limits of exposure to ultraviolet radiation of wavelengths between
  180 nm and 400 nm (incoherent optical radiation)_, Health Physics 87, 171,
  doi:10.1097/00004032-200408000-00006: 30 J/m² of S(λ)-weighted exposure in 8 h for skin and eyes,
  and 10 kJ/m² of unweighted UVA to the eyes, "absolute limits for direct exposure of the eye and
  'advisory' for skin", which noon summer sunlight at 0–40° latitude exceeds on skin within 5–10
  min; verified in the source's own text (read from icnirp.org in the check; the journal reference
  completed from Crossref).
- NASA-STD-3001 Vol. 1 Rev. C (NASA 2023), _NASA Space Flight Human-System Standard_, Volume 1 (no
  URL recorded; none found on NTRS in the second review): [V1 4030]'s career effective dose under
  600 mSv, "universal for all ages and sexes", and Table 4.8-1's 30-day and one-year organ limits in
  mGy-Eq (250 to the blood-forming organs, 1,000 to the lens, 1,500 to skin over 30 days), and the
  units of crew dose; verified in the source's own text (in the check).
- Hecht et al. 2009, _Detection of perchlorate and the soluble chemistry of martian soil at the
  Phoenix lander site_, Science 325, 64, doi:10.1126/science.1172466: perchlorate at 0.4–0.6% by
  mass in Martian soil; verified in the source's own text (abstract).
- Chang et al. 2024, _Artificial space weathering to mimic solar wind enhances the toxicity of lunar
  dust simulants in human lung cells_, GeoHealth 8, e2023GH000840, doi:10.1029/2023GH000840: lunar
  dust's "intense respiratory and ocular irritation" in Apollo crews; verified in the source's own
  text (abstract).
- 29 CFR 1910.134(d), _Respiratory protection_, selection of respirators,
  <https://www.ecfr.gov/current/title-29/section-1910.134>: air immediately dangerous to life or
  health needing a pressure-demand self-contained breathing apparatus or a supplied-air respirator
  with an auxiliary supply, and "all oxygen-deficient atmospheres shall be considered IDLH", for
  `MASK`'s supplied-gas form; verified in the source's own text (eCFR, in the second review).
- 29 CFR 1910.1000(d)(2)(i), _Air contaminants_, mixtures,
  <https://www.ecfr.gov/current/title-29/section-1910.1000>: gases of like effect summed as E_m = Σ
  Cₙ ÷ Lₙ, which may not exceed 1; verified in the source's own text (eCFR, in the second review).
- National Research Council 1997, _Mars Sample Return: Issues and Recommendations_, National Academy
  Press, doi:10.17226/5563: pathogenesis that "requires specific adaptations to overcome the
  extensive defenses possessed by all Earth organisms", so "virulent extraterrestrial pathogens are
  unlikely", though "the risk is not zero"; verified in the source's own text (executive summary and
  chapter 4).
- Adamala et al. 2024, _Confronting risks of mirror life_, Science 386, 1351,
  doi:10.1126/science.ads9158, with its _Technical Report on Mirror Bacteria: Feasibility and
  Risks_, <https://purl.stanford.edu/cv716pj4036>: the scientific community's recent caution about
  mirror-image life; verified in the source's own text (the technical report's abstract; the Science
  article itself is paywalled and was not read).
- COSPAR 2025, _COSPAR Policy on Planetary Protection_, approved by the COSPAR Bureau on 7 November
  2025, Space Research Today 224 (January 2026),
  <https://cosparhq.cnes.fr/assets/uploads/2026/01/SRT_224_Editorial_PPP-Nov-2025.pdf>: categories
  I–V with IVa's and IVb's bioburden limits, IVc's for what touches a Special Region, "a region
  within which terrestrial organisms are likely to replicate", Icy Worlds' probability of
  contamination below 1 × 10⁻⁴ over 1,000 years, and V assigned beside the outbound category, the
  organic inventory of hardware "to avoid any misinterpretation of terrestrial organics as
  indigenous biosignatures", the crewed-Mars guidance on back contamination, robotic precursors,
  quarantine and an onboard officer, and crews' microbial populations that "vary in both kind and
  quantity"; verified in the source's own text (full text).

**Resources, ore deposits and volatiles**

- Singer, Berger and Moring 2008, _Porphyry copper deposits of the world: database and grade and
  tonnage models_, USGS Open-File Report 2008-1155, doi:10.3133/ofr20081155: 422 well-explored
  deposits with a median of 240 Mt at 0.44% Cu, grade and tonnage nearly independent (r = 0.1, not
  significant), the porphyry row, its arithmetic and its test; verified in the source's own text
  (Table 1 and p. 4).
- Rudnick and Gao 2014, _Composition of the continental crust_, in Holland and Turekian (eds.),
  _Treatise on Geochemistry_, 2nd edn, vol. 4, Elsevier, 1–51,
  doi:10.1016/B978-0-08-095975-7.00301-6: the upper crust's 28 ± 4 µg/g of copper (Table 3),
  estimates that "vary by a factor of 2 or more", behind the ore's enrichment and the continents'
  copper inventory; verified in the source's own text (the authors' copy, read by a research agent
  in the second review), the chapter and pages from Crossref, the volume as that agent gives it, the
  editors from memory.
- John et al. 2010, _Porphyry copper deposit model_, USGS Scientific Investigations Report
  2010-5070-B, doi:10.3133/sir20105070B: porphyry deposits giving more than 60% of annual world
  copper production and about 65% of known resources, the lower end of 60–75%, and altered rock of
  "0.24 to 82 km2 with a median area of 5.1 km2", the halo's dilution; verified in the source's own
  text (in the check; the altered area in the USGS PDF, in the second review).
- Sillitoe 2010, _Porphyry copper systems_, Economic Geology 105, 3, doi:10.2113/gsecongeo.105.1.3:
  "nearly three-quarters of the world's Cu, half the Mo, perhaps one-fifth of the Au", from the
  upper 4 km or so of arc crust, with Meso-Cenozoic porphyries dominating because younger arcs are
  least eroded; verified in the source's own text (full text, through a university copy, in the
  research); unreachable in the check (GeoScienceWorld 403 and a Cloudflare challenge; its abstract
  holds none of these figures).
- Mosier, Berger and Singer 2009, _Volcanogenic massive sulfide deposits of the world: database and
  grade and tonnage models_, USGS Open-File Report 2009-1034, doi:10.3133/ofr20091034: 1,090
  massive-sulfide deposits compiled; verified in the source's own text.
- Galley, Hannington and Jonasson 2007, _Volcanogenic massive sulphide deposits_, in Goodfellow
  (ed.), _Mineral Deposits of Canada_, Geological Association of Canada, Mineral Deposits Division,
  Special Publication 5: surviving massive sulfides mostly from arcs and back-arcs; verified in the
  source's own text (a cached copy, in the research; unreachable in the check).
- Hannington et al. 2011, _The abundance of seafloor massive sulfide deposits_, Geology 39, 1155,
  doi:10.1130/G32468.1: about 6 × 10⁸ t of massive sulfide in today's neovolcanic zones; verified in
  the source's own text (abstract; full text in the research).
- Zientek et al. 2017, _Platinum-group elements_, USGS Professional Paper 1802-N,
  doi:10.3133/pp1802N: the Bushveld's 72% of the world's platinum-group resources and Noril'sk's
  10%, in the Siberian Traps; verified in the source's own text (p. N32).
- Davis 2008, _Sub-million-year age resolution of Precambrian igneous events by thermal
  extraction–thermal ionization mass spectrometer Pb dating of zircon: application to
  crystallization of the Sudbury impact melt sheet_, Geology 36, 383, doi:10.1130/G24502A.1:
  Sudbury's ore in an impact melt dated 1,850 Ma (1,849.53 ± 0.21 Ma); verified in the source's own
  text (in the research; the check confirmed the title).
- Van Gosen, Verplanck, Seal, Long and Gambogi 2017, _Rare-earth elements_, chap. O of Schulz,
  DeYoung, Seal and Bradley (eds.), _Critical Mineral Resources of the United States_, USGS
  Professional Paper 1802, O1–O31, doi:10.3133/pp1802O: carbonatites "the world's main source for
  the light REEs since the 1960s" and southern China's ion-adsorption clays the heavy; carbonatites
  mostly within stable plates, often on intracontinental rifts; the clays on granite under tropical,
  moderate to high rain; monazite of up to 20% ThO₂, thorium insoluble where uranium moves, and
  river and coastal monazite placers; verified in the source's own text (full text, in the second
  review).
- Bekker et al. 2010, _Iron formation: the sedimentary product of a complex interplay among mantle,
  tectonic, oceanic, and biospheric processes_, Economic Geology 105, 467,
  doi:10.2113/gsecongeo.105.3.467: giant Superior-type formations of 2.6–2.4 Ga on passive margins,
  vanishing about 1.85 Ga and returning briefly with the Neoproterozoic snowballs; verified in the
  source's own text (full text).
- Cox and Singer (eds.) 1986, _Mineral deposit models_, USGS Bulletin 1693, doi:10.3133/b1693: 60
  grade–tonnage models from over 3,900 well-explored deposits, log-normal in tonnage and most
  grades, and, in Mosier and Singer's chapter, the banded iron model's median of 170 Mt at 53% Fe
  with supergene ore included; verified in the source's own text (introduction, and figs. 172–173).
- Cannon, Kimball and Corathers 2017, _Manganese_, chap. L of the same Professional Paper 1802,
  L1–L28, doi:10.3133/pp1802L: the principal deposits "in ancient marine sedimentary rocks",
  precipitated where oxygen-poor deep water mixes with oxidised surface water on shelves, the
  Kalahari's 70% of identified resources, interlayered with banded iron, and marine beds as old as
  2.5 Ga; verified in the source's own text (full text, in the second review).
- O'Brien and Veeh 1980, _Holocene phosphorite on the East Australian continental margin_, Nature
  288, 690, doi:10.1038/288690a0: phosphorite forming on margins "characterized by an eastern
  boundary current, with strong coastal upwelling and associated high organic productivity";
  verified in the source's own text (abstract, through nature.com, in the second review).
- Papineau 2010, _Global biogeochemical changes at both ends of the Proterozoic: insights from
  phosphorites_, Astrobiology 10, 165, doi:10.1089/ast.2009.0360: "The oldest known phosphogenic
  event occurred around 2.0 Ga", and a larger one in the Neoproterozoic, tied to "the stepwise rise
  of atmospheric oxygen"; verified in the source's own text (abstract, through PubMed, in the second
  review).
- Stewart 1963, _Marine evaporites_, USGS Professional Paper 440-Y, doi:10.3133/pp440Y: Usiglio's
  data, gypsum once about 81% of seawater has evaporated, halite at 90%, polyhalite beyond 95% and
  potassium chlorides only in the last bitterns beyond about 98%; verified in the source's own text
  (Table 10 and pp. Y12, Y22–23).
- Orris 1995, _Bibliography and summary of data available for the Salar de Uyuni, Bolivia_, USGS
  Open-File Report 95-618, doi:10.3133/ofr95618: Uyuni as one of the largest lithium brines;
  verified in the source's own text.
- Yeend and Shawe 1989, _Gold in placer deposits_, USGS Bulletin 1857-G, doi:10.3133/b1857G: placers
  giving more than two thirds of the world's gold supply, fossil placers included; verified in the
  source's own text (p. G1).
- Brlek et al. 2021, _Karst bauxite formation during Miocene Climatic Optimum (central Dalmatia,
  Croatia): mineralogical, compositional and geochronological perspectives_, International Journal
  of Earth Sciences 110, 2899, doi:10.1007/s00531-021-02091-z: laterite needing over 1,100–1,200 mm
  of rain a year above 17–22 °C; verified in the source's own text (an open manuscript, in the
  research; unreachable in the check, the copy behind a JavaScript application).
- Foley, Jaskula, Kimball and Schulte 2017, _Gallium_, USGS Professional Paper 1802-H,
  doi:10.3133/pp1802H: lateritic bauxite about 90% of the world's resources; verified in the
  source's own text.
- Leach et al. 2010, _Sediment-hosted lead-zinc deposits in Earth history_, Economic Geology 105,
  593, doi:10.2113/gsecongeo.105.3.593: clastic-dominated and Mississippi Valley-type lead–zinc
  first at 2.02 Ga, ores that "recorded the oxygenation of Earth's hydrosphere", sulfate evaporites
  and sulfate-reducing bacteria among the causes, so after `Oxygenation`; verified in the source's
  own text (abstract, through OpenAlex, in the second review).
- Hayes et al. 2015, _Sediment-hosted stratabound copper deposit model_, USGS Scientific
  Investigations Report 2010-5070-M, doi:10.3133/sir20105070M: red beds and sediment-hosted copper
  only since the Great Oxidation Event, so after `Oxygenation`, and about a fifth of the world's
  copper; verified in the source's own text (abstract).
- Emsbo et al. 2016, _Sedimentary exhalative (sedex) zinc-lead-silver deposit model_, USGS
  Scientific Investigations Report 2010-5070-N, doi:10.3133/sir20105070N: over half the world's zinc
  and lead reserves; verified in the source's own text (introduction).
- Cuney 2010, _Evolution of uranium fractionation processes through time: driving the secular
  variation of uranium deposit types_, Economic Geology 105, 553, doi:10.2113/gsecongeo.105.3.553:
  detrital uraninite placers "only during" 3.1–2.2 Ga, the first redox-controlled ore "at about
  2.0 Ga in the Oklo area of Gabon", the unconformity ores late Palaeoproterozoic to early
  Mesoproterozoic, basal, roll-front and tabular ores after 0.45 Ga on plant debris as
  "intraformational reduced traps", hydrocarbons and H₂S making some sandstone ores "older than the
  Silurian", and fractionated granite the source; verified in the source's own text (abstract,
  through OpenAlex, in the second review).
- Nelsen, DiMichele, Peters and Boyce 2016, _Delayed fungal evolution did not cause the Paleozoic
  peak in coal production_, PNAS 113, 2442, doi:10.1073/pnas.1517943113: coal from a peat-forming
  flora with lignin not required, from the Early Devonian, greatest in the Carboniferous–Permian
  under everwet tropics and subsiding basins; verified in the source's own text (abstract and body).
- Floudas et al. 2012, _The Paleozoic origin of enzymatic lignin decomposition reconstructed from 31
  fungal genomes_, Science 336, 1715, doi:10.1126/science.1221748: the lignin-decay explanation of
  the coal peak that Nelsen et al. reject; verified in the source's own text (abstract).
- Tissot and Welte 1984, _Petroleum Formation and Occurrence_, Springer,
  doi:10.1007/978-3-642-87813-8: the oil window of 60–150 °C; verified via a secondary source
  (Rasmussen and Muhling 2023, Science Advances 9, doi:10.1126/sciadv.add7925; other sources give
  60–120 °C).
- Klemme and Ulmishek 1991, _Effective petroleum source rocks of the world: stratigraphic
  distribution and controlling depositional factors_, AAPG Bulletin 75, 1809,
  doi:10.1306/0C9B2A47-1710-11D7-8645000102C1865D: six intervals, a third of Phanerozoic time,
  sourcing over 90% of reserves, "almost 70%" of them generated since the Coniacian; verified in the
  source's own text (abstract).
- Lorenz et al. 2008, _Titan's inventory of organic surface materials_, GRL 35, L02206,
  doi:10.1029/2007GL032118: dozens of Titan's seas and lakes each holding more than Earth's known
  oil and gas, plan 14's abiotic inventory; verified in the source's own text (abstract).
- Hazen et al. 2008, _Mineral evolution_, American Mineralogist 93, 1693, doi:10.2138/am.2008.2955:
  biochemical processes perhaps "responsible, directly or indirectly, for most of Earth's 4300 known
  mineral species", and supergene chrysocolla, malachite and azurite that "may not have occurred to
  any significant extent prior to biological oxygenesis" (p. 1707); verified in the source's own
  text (abstract, through OpenAlex; p. 1707 in the full text, read by a research agent in the second
  review).
- Konhauser et al. 2007, _Decoupling photochemical Fe(II) oxidation from shallow-water BIF
  deposition_, EPSL 258, 87, doi:10.1016/j.epsl.2007.03.026: abiotic ultraviolet photo-oxidation
  negligible as banded iron's oxidant; verified in the source's own text (abstract, in the research;
  unreachable in the check, the copies behind bot challenges).
- Rasmussen, Krapež, Muhling and Suvorova 2015, _Precipitation of iron silicate nanoparticles in
  early Precambrian oceans marks Earth's first iron age_, Geology 43, 303, doi:10.1130/G36309.1: the
  contested iron-silicate precursor, an oxidant-free route; verified in the source's own text
  (abstract).
- Johnson, Gerpheide, Lamb and Fischer 2014, _O₂ constraints from Paleoproterozoic detrital pyrite
  and uraninite_, GSA Bulletin 126, 813, doi:10.1130/B30949.1: detrital pyrite and uraninite
  surviving long transport only below about 3.2 × 10⁻⁵ atm of O₂ and short transport below 10⁻² atm;
  verified in the source's own text (abstract).
- Grotzinger and Kasting 1993, _New constraints on Precambrian ocean composition_, Journal of
  Geology 101, 235, doi:10.1086/648218: gypsum rare while seawater sulfate stayed low before about
  1.9 Ga, and life making almost all modern marine carbonate while Archaean seawater made a surplus
  abiotically, so limestone records life only in its fabric; verified in the source's own text (the
  Caltech copy).
- Sillitoe 2005, _Supergene oxidized and enriched porphyry copper and related deposits_, in
  _Economic Geology One Hundredth Anniversary Volume_, Society of Economic Geologists, 723–768,
  doi:10.5382/AV100.22: sulfides oxidised "above the water table", their copper "transferred
  downward to the reduced environment … where sulfide enrichment takes place"; verified in the
  source's own text (abstract, through OpenAlex and Crossref, in the second review).
- Bryan and Ernst 2008, _Revised definition of large igneous provinces (LIPs)_, Earth-Science
  Reviews 86, 175, doi:10.1016/j.earscirev.2007.08.008: large igneous provinces over 10⁵ km³ in
  pulses of 1–5 Myr; verified in the source's own text (the accepted manuscript).
- Hayne, Aharonson and Schörghofer 2021, _Micro cold traps on the Moon_, Nature Astronomy 5, 169
  (online 2020), doi:10.1038/s41550-020-1198-9: the bowl shadows behind
  `hyperion_surface::cold_trap`; verified in the source's own text (abstract and Methods).
- Ingersoll, Svitek and Murray 1992, _Stability of polar frosts in spherical bowl-shaped craters on
  the Moon, Mercury, and Mars_, Icarus 100, 40, doi:10.1016/0019-1035(92)90016-Z: the shadow's
  temperature in plan 14's cold traps, 97 K at a lunar pole at d/D 0.2 in R09.T7.d's test; verified
  via a secondary source (Hayne, Aharonson and Schörghofer 2021; the citation confirmed on
  Crossref).
- Schörghofer and Williams 2020, _Mapping of ice storage processes on the Moon with time-dependent
  temperatures_, Planetary Science Journal 1, 54, doi:10.3847/PSJ/abb6ff: the 110 K sublimation
  threshold for water that plan 14's cold traps take (its Table 1, as plan 14 cites it); **not
  re-checked** (the record from OpenAlex and Crossref; its abstract states no threshold).
- Schörghofer and Rufu 2023, _Past extent of lunar permanently shadowed areas_, Science Advances 9,
  eadh4302, doi:10.1126/sciadv.adh4302: the Moon's spin axis about 1.5° to the ecliptic, its 6.7°
  obliquity to its orbit less the orbit's 5.1° inclination (θ = ε − i), the LCROSS site in Cabeus
  dark for only about 0.9 Gyr, and the permanent shadow half today's 2.2 Ga ago; verified in the
  source's own text (full text, PMC10499310); R09's reference Moon carries 1.535°, NSSDC's 6.68°
  less its 5.145°.
- Stiles et al. 2008, _Determining Titan's spin state from Cassini RADAR images_, AJ 135, 1669,
  doi:10.1088/0004-6256/135/5/1669: Titan's pole implying "about 0.3° obliquity", a synchronous
  body's small Cassini-state tilt; verified in the source's own text (abstract, through OpenAlex, in
  the second review).
- Margot, Peale, Jurgens, Slade and Holin 2007, _Large longitude libration of Mercury reveals a
  molten core_, Science 316, 710, doi:10.1126/science.1140514: Mercury in "a Cassini state with
  obliquity of 2.11 +/- 0.1 arc minutes"; verified in the source's own text (abstract, through
  PubMed, in the second review).
- Li et al. 2018, _Direct evidence of surface exposed water ice in the lunar polar regions_, PNAS
  115, 8907, doi:10.1073/pnas.1802345115: ice at the optical surface in only about 3.5% of lunar
  cold traps, and spectroscopy in permanent shadow by scattered light; verified in the source's own
  text (abstract).
- Deutsch, Head, Chabot and Neumann 2018, _Constraining the thickness of polar ice deposits on
  Mercury using the Mercury Laser Altimeter and small craters in permanently shadowed regions_,
  Icarus 305, 139, doi:10.1016/j.icarus.2018.01.013: tens of metres of ice, some 41 m in nine
  craters; verified in the source's own text (NTRS abstract).
- Deutsch, Head and Neumann 2019, _Age constraints of Mercury's polar deposits suggest recent
  delivery of ice_, EPSL 520, 26, doi:10.1016/j.epsl.2019.05.027: nearly pure ice delivered within
  about 150 Myr; verified in the source's own text.
- Feldman et al. 2002, _Global distribution of neutrons from Mars: results from Mars Odyssey_,
  Science 297, 75, doi:10.1126/science.1073541: Martian ground ice poleward of about ±60° in the
  uppermost metre, the neutron sensing depth; verified in the source's own text.
- Dundas et al. 2018, _Exposed subsurface ice sheets in the Martian mid-latitudes_, Science 359,
  199, doi:10.1126/science.aao1619: mid-latitude ice over 100 m thick beneath 1–2 m of cover;
  verified in the source's own text (abstract).
- Heber et al. 2012, _Isotopic mass fractionation of solar wind: evidence from fast and slow solar
  wind collected by the Genesis mission_, ApJ 759, 121, doi:10.1088/0004-637X/759/2/121: implanted
  helium about 4.6 × 10⁻⁴ ³He (4.48–4.77 × 10⁻⁴ from fast to slow wind); verified in the source's
  own text (Table 2, in the check).
- Wittenberg, Santarius and Kulcinski 1986, _Lunar source of ³He for commercial fusion power_,
  Fusion Technology 10, 167, doi:10.13182/FST86-A24972: the Moon's ³He of about 10⁹ kg, the lower
  end of 10⁶–2.5 × 10⁶ t; verified in the source's own text (abstract).
- Slyuta, Abdrakhimov and Galimov 2007, _The estimation of helium-3 probable reserves in lunar
  regolith_, Lunar and Planetary Science XXXVIII, abstract 2175,
  <https://www.lpi.usra.edu/meetings/lpsc2007/pdf/2175.pdf>: 1.4–15.1 ppb at the landing sites and
  2.47 × 10⁶ t in all, for regolith 4.4–10.1 m thick; verified in the source's own text (Tables
  1–2).
- Fa and Jin 2007, _Quantitative estimation of helium-3 spatial distribution in the lunar regolith
  layer_, Icarus 190, 15, doi:10.1016/j.icarus.2007.03.014: the fit C = 0.56 TiO₂ F ÷ OMAT + 1.62
  ppb over 25 Apollo soils, the closed form for a lunar-like body; verified via a secondary source
  (Grava et al. 2021); unreachable in the primary (Elsevier closed, ADS returning 405).
- Grava et al. 2021, _Volatiles and refractories in surface-bounded exospheres in the inner Solar
  System_, Space Science Reviews 217, 61, doi:10.1007/s11214-021-00833-8: the secondary source
  quoting Fa and Jin's fit and its calibration; verified in the source's own text (full text).
- Guillot and Gautier 2015, _Giant planets_, Treatise on Geophysics, 2nd edn, 529,
  doi:10.1016/B978-0-444-53802-4.00176-7 (arXiv:1405.3752): Jupiter's ³He/⁴He of 1.66 × 10⁻⁴, also
  the protosolar ratio a giant keeps, and its Table 4's ISO D/H; verified in the source's own text
  (Table 4).
- Mahaffy, Donahue, Atreya, Owen and Niemann 1998, _Galileo probe measurements of D/H and ³He/⁴He in
  Jupiter's atmosphere_, Space Science Reviews 84, 251, doi:10.1023/A:1005091806594: Jupiter's D/H
  of 2.6 × 10⁻⁵ from the Galileo probe; verified via a secondary source (Blake et al. 2021,
  arXiv:2108.09951); unreachable in the primary (a Springer challenge).
- Lellouch et al. 2001, _The deuterium abundance in Jupiter and Saturn from ISO-SWS observations_,
  A&A 370, 610, doi:10.1051/0004-6361:20010259: Jupiter's D/H of 2.25 × 10⁻⁵ from ISO; verified via
  a secondary source (Guillot and Gautier 2015, Table 4).
- von Zahn, Hunten and Lehmacher 1998, _Helium in Jupiter's atmosphere: results from the Galileo
  probe Helium Interferometer Experiment_, JGR Planets 103, 22815, doi:10.1029/98JE00695: a helium
  mole fraction of 0.1359, confirming P14.T50.b's He/H₂ of about 0.16 and giving a tonne of ³He per
  some 34,000 t of gas; verified in the source's own text (abstract).
- Feuchtgruber et al. 2013, _The D/H ratio in the atmospheres of Uranus and Neptune from
  Herschel-PACS observations_, A&A 551, A126, doi:10.1051/0004-6361/201220857: D/H of 4.4 and 4.1 ×
  10⁻⁵ in Uranus and Neptune; verified in the source's own text (abstract).
- Altwegg et al. 2015, _67P/Churyumov-Gerasimenko, a Jupiter family comet with a high D/H ratio_,
  Science 347, 1261952, doi:10.1126/science.1261952: 67P's D/H of (5.3 ± 0.7) × 10⁻⁴, about three
  times Earth's, so a comet's water is its own; verified in the source's own text (abstract, through
  PubMed, in the second review).
- Palaszewski 2015, _Atmospheric mining in the outer solar system: resource capturing, exploration,
  and exploitation_, NASA/TM-2015-218096, <https://ntrs.nasa.gov/citations/20150009496>: NASA's
  mining studies preferring Uranus and Neptune for their lower energy to leave; verified in the
  source's own text (p. 7).
- DeMeo, Binzel, Slivan and Bus 2009, _An extension of the Bus asteroid taxonomy into the
  near-infrared_, Icarus 202, 160, doi:10.1016/j.icarus.2009.02.005: the Bus–DeMeo classes; verified
  in the source's own text.
- DeMeo and Carry 2013, _The taxonomic distribution of asteroids from multi-filter all-sky
  photometric surveys_, Icarus 226, 723, doi:10.1016/j.icarus.2013.06.027: the main belt's types
  running E, S, C, P, D outwards, and its mass by type (C 52.5%, B 11.1%, P 11.0%, V 9.6%, S 8.4%, M
  3.3%; Table 5), and Ceres, Pallas, Vesta and Hygiea, which "heavily skew the values", so that its
  shares with the four removed test a generated belt better; verified in the source's own text (the
  skew in its abstract, arXiv:1307.2424, in the second review).
- DeMeo and Carry 2014, _Solar System evolution from compositional mapping of the asteroid belt_,
  Nature 505, 629, doi:10.1038/nature12908: a belt "more compositionally diverse with size and
  distance from the Sun than had previously been known", the review on which P14.T51.e's qualitative
  belt test rests, as plan 14 cites it; verified in the source's own text (abstract,
  arXiv:1408.2787, in the second review).
- Nakamura et al. 2011, _Itokawa dust particles: a direct link between S-type asteroids and ordinary
  chondrites_, Science 333, 1113, doi:10.1126/science.1207758: S types tied to LL chondrites;
  verified in the source's own text.
- Yokoyama et al. 2023, _Samples returned from the asteroid Ryugu are similar to Ivuna-type
  carbonaceous meteorites_, Science 379, eabn7850, doi:10.1126/science.abn7850: C types tied to CI
  chondrites; verified in the source's own text.
- Elkins-Tanton et al. 2020, _Observations, meteorites, and models: a preflight assessment of the
  composition and formation of (16) Psyche_, JGR Planets 125, e2019JE006296,
  doi:10.1029/2019JE006296: Psyche only 30–60% metal by volume, and the upper-micron sensing depth
  of reflectance spectroscopy; verified in the source's own text.
- McCoy et al. 2022, _Deciphering redox state for a metal-rich world_, Space Science Reviews 218, 6,
  doi:10.1007/s11214-022-00872-9: iron meteorites' 5–18% nickel, a metallic body's composition;
  verified in the source's own text.
- Singer 1995, _World class base and precious metal deposits; a quantitative analysis_, Economic
  Geology 90, 88, doi:10.2113/gsecongeo.90.1.88: the top tenth of deposits by contained metal
  holding over 86% of the gold and 84% of the copper, all types pooled; verified in the source's own
  text (abstract).
- Singer, Berger, Menzie and Berger 2005, _Porphyry copper deposit density_, Economic Geology 100,
  491, doi:10.2113/gsecongeo.100.3.491: 2–128 porphyry deposits per 100,000 km² of permissive rock,
  median 15, falling as the tract grows; verified in the source's own text (abstract).
- Singer 2008, _Mineral deposit densities for estimating mineral resources_, Mathematical
  Geosciences 40, 33, doi:10.1007/s11004-007-9127-3: counts regressed on tract area and median
  deposit size; verified in the source's own text (abstract; its coefficients not read).
- Moon Mineralogy Mapper, PDS instrument catalogue (CH1M3_0004), in the volume at
  <https://pds-imaging.jpl.nasa.gov/volumes/m3.html>: 430–3,000 nm at 70 or 140 m pixels from
  100 km; verified in the source's own text.
- Lunar Prospector, PDS mission catalogue, in the archive at
  <https://pds-geosciences.wustl.edu/missions/lunarp/index.htm>: a gamma-ray footprint of about
  150 km from 100 km; verified in the source's own text.
- Lawrence et al. 2003, _Small-area thorium features on the lunar surface_, JGR Planets 108,
  doi:10.1029/2003JE002050: about 80 km resolution from a low orbit; verified in the source's own
  text.
- Lawrence et al. 1998, _Global elemental maps of the Moon: the Lunar Prospector gamma-ray
  spectrometer_, Science 281, 1484, doi:10.1126/science.281.5382.1484: Lunar Prospector's maps of
  Th, K and Fe; verified in the source's own text.
- Feldman et al. 1998, _Fluxes of fast and epithermal neutrons from Lunar Prospector: evidence for
  water ice at the lunar poles_, Science 281, 1496, doi:10.1126/science.281.5382.1496: the lunar
  polar hydrogen found by neutron spectrometry; verified in the source's own text.
- Steele et al. 2024, _Weak magnetism of Martian impact basins may reflect cooling in a reversing
  dynamo_, Nature Communications 15, 6831, doi:10.1038/s41467-024-51092-4: magnetometry resolving
  features about as small as the altitude; verified in the source's own text.
- Konopliv et al. 2013, _The JPL lunar gravity field to spherical harmonic degree 660 from the GRAIL
  Primary Mission_, JGR Planets 118, 1415, doi:10.1002/jgre.20097: gravimetry to 9–15 km at degree
  660; verified in the source's own text (in the research; unreachable in the check, Wiley returning
  403, so the figure, an effective resolution against degree 660's half-wavelength of about 8.3 km,
  should be re-read).
- Chappaz et al. 2017, _Evidence of large empty lava tubes on the Moon using GRAIL gravity_, GRL 44,
  105, doi:10.1002/2016GL071588: GRAIL's lava-tube candidates kilometres wide; verified in the
  source's own text (abstract).
- Seu et al. 2007b, _Accumulation and erosion of Mars' south polar layered deposits_, Science 317,
  1715, doi:10.1126/science.1144120: SHARAD's echoes from about 1 km down; verified in the source's
  own text.
- Plaut et al. 2007, _Subsurface radar sounding of the south polar layered deposits of Mars_,
  Science 316, 92, doi:10.1126/science.1139672: MARSIS sounding over 3.7 km; verified in the
  source's own text.
- Seu et al. 2007a, _SHARAD sounding radar on the Mars Reconnaissance Orbiter_, JGR Planets 112,
  E05S05, doi:10.1029/2006JE002745: SHARAD's 15 m vertical and 0.3–1 by 3–6 km horizontal
  resolution; verified in the source's own text.
- Holt et al. 2008, _Radar sounding evidence for buried glaciers in the southern mid-latitudes of
  Mars_, Science 322, 1235, doi:10.1126/science.1164246: Mars's debris aprons shown by radar to be
  massive water ice under rock; verified in the source's own text.

**Geology as places**

- Haruyama et al. 2009, _Possible lunar lava tube skylight observed by SELENE cameras_, GRL 36,
  L21206, doi:10.1029/2009GL040635: the Marius Hills hole, 65 m across and 80–88 m deep; verified in
  the source's own text (abstract).
- Cushing, Titus, Wynne and Christensen 2007, _THEMIS observes possible cave skylights on Mars_, GRL
  34, L17201, doi:10.1029/2007GL030709: candidate skylights on Arsia Mons, 100–225 m across;
  verified in the source's own text (abstract).
- Sauro et al. 2020, _Lava tubes on Earth, Moon and Mars: a review on their size and morphology
  revealed by comparative planetology_, Earth-Science Reviews 209, 103288,
  doi:10.1016/j.earscirev.2020.103288: collapse widths of 20–30 m on Earth, 40–400 m on Mars and
  500–900 m on the Moon, attributed to effusion rate and roof stability, with lunar collapses
  triggered by impacts or wrinkle ridges, behind the g^−1.8 fit (a fit here); verified in the
  source's own text (conclusions of the author's postprint).
- Wynne et al. 2022, _Fundamental science and engineering questions in planetary cave exploration_,
  JGR Planets 127, e2022JE007194, doi:10.1029/2022JE007194: caves as shielded refugia; verified in
  the source's own text.
- Keller, Colman and Boyd 2023, _An active microbiome in Old Faithful geyser_, PNAS Nexus 2,
  pgad066, doi:10.1093/pnasnexus/pgad066: fewer than 1,000 geysers, nearly half at Yellowstone;
  verified in the source's own text (its introduction, itself citing Bryan 2008).
- Hurwitz and Manga 2017, _The fascinating and complex dynamics of geyser eruptions_, Annual Review
  of Earth and Planetary Sciences 45, 31, doi:10.1146/annurev-earth-063016-015605: geysers needing
  silica and fractures sealed by low-permeability rock; verified in the source's own text
  (abstract).
- Martin, Baross, Kelley and Russell 2008, _Hydrothermal vents and the origin of life_, Nature
  Reviews Microbiology 6, 805, doi:10.1038/nrmicro1991: vents as candidate cradles of life; verified
  in the source's own text.
- Hansen et al. 2011, _The composition and structure of the Enceladus plume_, GRL 38,
  doi:10.1029/2011GL047415: Enceladus venting about 200 ± 30 kg/s of water vapour, a flux varying by
  about 15%, so the vapour is nearly steady; verified in the source's own text (abstract).
- Hsu et al. 2015, _Ongoing hydrothermal activities within Enceladus_, Nature 519, 207,
  doi:10.1038/nature14262: silica grains implying reactions above 90 °C; verified in the source's
  own text (abstract, in the research; the figure unreachable in the check).
- Hedman et al. 2013, _An observed correlation between plume activity and tidal stresses on
  Enceladus_, Nature 500, 182, doi:10.1038/nature12371: the plume several times brighter near
  apocentre than pericentre, in its grains; verified in the source's own text (abstract, in the
  research; the factor unreachable in the check).
- Soderblom et al. 1990, _Triton's geyser-like plumes: discovery and basic characterization_,
  Science 250, 410, doi:10.1126/science.250.4979.410: dark columns "rising to an altitude of about 8
  kilometers", clouds that "drift downwind over 100 kilometers", heating of nitrogen ice "in a
  subsurface greenhouse" as one model, and eruptions that "may last a year or more"; verified in the
  source's own text (abstract, through PubMed, in the second review).
- Kieffer, Christensen and Titus 2006, _CO₂ jets formed by sublimation beneath translucent slab ice
  in Mars' seasonal south polar ice cap_, Nature 442, 793, doi:10.1038/nature04945: an "impermeable,
  translucent slab of CO2 ice that sublimates from the base", venting sand-sized grains in jets;
  verified in the source's own text (abstract, through PubMed, in the second review).
- Ericksen, Vine and Ballón 1977, _Lithium-rich brines at Salar de Uyuni and nearby salars in
  southwestern Bolivia_, USGS Open-File Report 77-615, doi:10.3133/ofr77615: lithium of 80–1,150 ppm
  in the brines of Uyuni and two nearby salars; verified in the source's own text (p. 12).
- De Sanctis et al. 2016, _Bright carbonate deposits as evidence of aqueous alteration on (1)
  Ceres_, Nature 536, 54, doi:10.1038/nature18290: Occator's faculae, mostly sodium carbonate, the
  airless salt flat; verified in the source's own text.
- Lowenstein, Schubert and Timofeeff 2011, _Microbial communities in fluid inclusions and long-term
  survival in halite_, GSA Today 21, 4, doi:10.1130/GSATG81A.1: halite's fluid inclusions holding
  "an ecosystem of 'salt-loving' (halophilic) prokaryotes and eukaryotes, some of which are alive";
  verified in the source's own text (abstract, through OpenAlex, in the second review).
- Schreder-Gomes, Benison and Bernau 2022, _830-million-year-old microorganisms in primary fluid
  inclusions in halite_, Geology 50, 918, doi:10.1130/G49957.1: primary inclusions that "contain
  organic solids and liquids"; verified in the source's own text (abstract, through OpenAlex, in the
  second review).
- Kok et al. 2012, _The physics of wind-blown sand and dust_, Reports on Progress in Physics 75,
  106901, doi:10.1088/0034-4885/75/10/106901: the saltation threshold growing as the square root of
  gravity, grain size and grain density over air density, Mars's an order of magnitude above
  Earth's; verified in the source's own text (§2.4.1.1).
- Lorenz et al. 2006, _The sand seas of Titan: Cassini RADAR observations of longitudinal dunes_,
  Science 312, 724, doi:10.1126/science.1123257: Titan's dunes about 100 m high in winds near 0.5
  m/s; verified in the source's own text (abstract).
- Rodriguez et al. 2014, _Global mapping and characterization of Titan's dune fields with Cassini:
  correlation between RADAR and VIMS observations_, Icarus 230, 168,
  doi:10.1016/j.icarus.2013.11.017: dunes covering 17.5 ± 1.5% of Titan; verified in the source's
  own text (a HAL abstract, in the research; unreachable in the check, behind a bot block; Lorenz et
  al. 2008 say "some 20%").
- Andreotti et al. 2009, _Giant aeolian dune size determined by the average depth of the atmospheric
  boundary layer_, Nature 457, 1120, doi:10.1038/nature07787: giant dunes stopping at a spacing set
  by the boundary layer, 300 m to 3.5 km on Earth; verified in the source's own text (abstract, in
  the research).
- Goldscheider et al. 2020, _Global distribution of carbonate rocks and karst water resources_,
  Hydrogeology Journal 28, 1661, doi:10.1007/s10040-020-02139-5: carbonate under 15.2% of Earth's
  ice-free land; verified in the source's own text (abstract).
- Bruthans, Kamas, Filippi, Zare and Mayo 2017, _Hydrogeology of salt karst under different cap
  soils and climates (Persian Gulf and Zagros Mts., Iran)_, International Journal of Speleology 46,
  303, doi:10.5038/1827-806X.46.2.2109: salt karst on Iran's diapirs under arid and semi-arid
  climates; verified in the source's own text (abstract, in the second review).
- Cornet et al. 2015, _Dissolution on Titan and on Earth: toward the age of Titan's karstic
  landscapes_, JGR Planets 120, 1044, doi:10.1002/2014JE004738: Titan's organic sediment dissolving
  as salts or carbonates do on Earth, some 30 times more slowly for its seasonal rain; verified in
  the source's own text (abstract, in the second review).
- McKinnon et al. 2016, _Convection in a volatile nitrogen-ice-rich layer drives Pluto's geological
  vigour_, Nature 534, 82, doi:10.1038/nature18289: Sputnik Planitia's nitrogen ice in active flow;
  verified in the source's own text.
- Moore et al. 2016, _The geology of Pluto and Charon through the eyes of New Horizons_, Science
  351, 1284, doi:10.1126/science.aad7055: Sputnik Planitia's crater retention age of 10 Myr or less;
  verified in the source's own text.
- Tompkins and Pieters 1999, _Mineralogy of the lunar crust: results from Clementine_, Meteoritics
  and Planetary Science 34, 25, doi:10.1111/j.1945-5100.1999.tb01729.x: craters of 40–180 km lifting
  rock from 5–30 km deep into their central peaks; verified in the source's own text (abstract).
- Osinski et al. 2013, _Impact-generated hydrothermal systems on Earth and Mars_, Icarus 224, 347,
  doi:10.1016/j.icarus.2012.08.030: melt sheets and hydrothermal systems in complex craters above
  2–4 km on Earth and 5–10 km on Mars, their altered rock feeding life long after; verified in the
  source's own text (abstract; the thresholds not attempted in the check).
- Abramov and Kring 2005, _Impact-induced hydrothermal activity on early Mars_, JGR Planets 110,
  doi:10.1029/2005JE002453: hydrothermal lifetimes of about 67,000 years in a 30 km Martian crater
  and 380,000 in a 180 km one; verified in the source's own text (abstract).
- Kring et al. 2020, _Probing the hydrothermal system of the Chicxulub impact crater_, Science
  Advances 6, eaaz3053, doi:10.1126/sciadv.aaz3053: Chicxulub's system lasting over a million years
  from an initial 300–400 °C, so a cooling curve; verified in the source's own text (full text, in
  the check).

**Standards, conventions and data**

- NIST Chemistry WebBook, phase-change data for methane, ethane, ammonia, water and carbon dioxide,
  <https://webbook.nist.gov>: the solvents' liquid ranges at 1 bar and critical points (water 647 K
  and 221 bar, ammonia 405 K and 113 bar, CO₂ above 304 K and 74 bar); verified in the source's own
  text (read 2026-10-10; ammonia's boiling point, about 240 K, was not in the table read).
- IAPWS R14-08(2011), _Revised release on the pressure along the melting and sublimation curves of
  ordinary water substance_, <https://iapws.org/technical-guidance/release/MeltSub>: the ice
  V–VI–liquid triple point at 273.31 K and 632.4 MPa (Table 1), and ice V's melting curve (eq. 3),
  on which liquid water at 0 °C meets ice V at about 629 MPa, beyond any demonstrated growth;
  verified in the source's own text (Table 1 in the check; the whole release in the second review,
  eq. 3 reproducing its own check value of 479.640 MPa at 265 K; the research could not fetch it).
- JORC 2012, _Australasian Code for Reporting of Exploration Results, Mineral Resources and Ore
  Reserves_, <https://www.jorc.org/docs/JORC_code_2012.pdf>: Inferred, Indicated and Measured
  resources "in order of increasing geological confidence", resting on "limited geological evidence
  and sampling" (clause 21), evidence "sufficient to assume" continuity (clause 22) and "sufficient
  to confirm" it (clause 23); the Exploration Target below them, "quoted as a range of tonnes and a
  range of grade" for mineralisation with "insufficient exploration to estimate a Mineral Resource"
  (clause 17); the economic test every resource must pass ("reasonable prospects for eventual
  economic extraction", clause 20) and the reserve categories left out, for deposits' confidence
  rungs; verified in the source's own text (clauses 17 and 20–23).
- Archinal et al. 2018, _Report of the IAU Working Group on Cartographic Coordinates and Rotational
  Elements: 2015_, Celestial Mechanics and Dynamical Astronomy 130, 22,
  doi:10.1007/s10569-017-9805-5: planetocentric longitude always increasing eastward, planetographic
  westward for prograde rotation, no advantage in switching where planetographic coordinates are
  established, Mars, Mercury and the Moon mapped east-positive, and the right-hand-rule pole for
  dwarf planets and small bodies; verified in the source's own text (the USGS reprint, through
  Wayback).
- IAU and USGS, _Gazetteer of Planetary Nomenclature_, descriptor terms,
  <https://planetarynames.wr.usgs.gov/DescriptorTerms>: descriptor terms such as Mons and Vallis as
  the generic part of generated place names; verified in the source's own text.
- Okabe and Ito 2002 (revised 2008), _Color Universal Design_, <https://jfly.uni-koeln.de/color/>:
  the colour-blind-safe palette, six of whose seven colours lie within ΔE₀₀ 22 of a reserved token
  (computed here); verified in the source's own text (the RGB values).
- Tol, colour schemes, <https://sronpersonalpages.nl/~pault/>: the "muted" palette, six of whose
  nine colours lie within ΔE₀₀ 22 of a reserved token (computed here); verified in the source's own
  text (the hex values).
- Sharma, Wu and Dalal 2005, _The CIEDE2000 color-difference formula: implementation notes,
  supplementary test data, and mathematical observations_, Color Research and Application 30, 21,
  doi:10.1002/col.20070: the CIEDE2000 difference behind every ΔE₀₀ above; verified in the source's
  own text (abstract, and the authors' test data, whose pairs 1 and 7, 2.0425 and 2.3669, the
  implementation reproduces, in the second review); all 34 pairs to be run before it becomes code.
- Machado, Oliveira and Fernandes 2009, _A physiologically-based model for simulation of color
  vision deficiency_, IEEE Transactions on Visualization and Computer Graphics 15, 1291,
  doi:10.1109/TVCG.2009.113: the three dichromacies in which the class tokens are scored; verified
  in the source's own text (the severity-1.0 matrices on the authors' page; applied in linear RGB,
  which the page does not specify).
- NIST SP 811 (Thompson and Taylor 2008), _Guide for the Use of the International System of Units_,
  <https://www.nist.gov/pml/special-publication-811>: the uncertainty form (§7.7), no information
  mixed into units ("5 ng/L but not: 5 ng Pb/L", §7.5), parentheses for a second solidus (§6.1.6),
  ppm, ppb and ppt not for use with the SI (§7.10.3), prefixes with the tonne (§6.2.8), and
  information attached to no unit, a fraction's kind named in the quantity (§7.4 and §7.10.2);
  verified in the source's own text, §7.4 and §7.10.2 **not re-checked** (cited as the text cites
  them).
- Sigovini, Keppel and Tagliapietra 2016, _Open nomenclature in the biodiversity era_, Methods in
  Ecology and Evolution 7, 1217, doi:10.1111/2041-210X.12594: the qualifiers cf., aff. and sp.;
  verified in the source's own text (the definitions).
- WMO 2025, _Greenhouse Gas Bulletin_ No. 21,
  <https://wmo.int/sites/default/files/2025-10/GHG-21_en.pdf>: atmospheric abundances reported in
  ppm as dry-air mole fractions; verified in the source's own text.
- BIPM 2019, _The International System of Units_, 9th edition,
  <https://www.bipm.org/en/publications/si-brochure>: ppb and ppt to be avoided as
  language-dependent (§5.4.7); verified in the source's own text.
- Hall et al. 2021, _Revision of the World Meteorological Organization Global Atmosphere Watch
  (WMO/GAW) CO₂ calibration scale_, Atmospheric Measurement Techniques 14, 3015,
  doi:10.5194/amt-14-3015-2021: the WMO's own scale defined in µmol/mol; verified in the source's
  own text.
