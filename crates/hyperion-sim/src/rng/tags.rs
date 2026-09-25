//! The single registry of domain tags.
//!
//! Every domain tag in the crate is declared here and nowhere else, so that the collision check,
//! a `const` assertion over this one list, covers them all. The rules:
//!
//! - A tag is never renamed or removed. Its name is hashed into the key of every stream it opens,
//!   so a rename moves every value drawn under it.
//! - A new property group gets a new tag, so that adding it moves nothing already generated.
//! - Each plan adds its tags under its own heading, as `CONST_NAME: Scope = "tag.name";`, in the
//!   task that first opens a stream under them. Names match
//!   `[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)+`.
//! - The tag behind each event tag is an ordinary entry of scope `Event`, which the event-tag
//!   registry in `id/event_tags.rs` names by constant.

use super::domain_tag::domain_tags;

domain_tags! {
    // Plan 01: the determinism foundation.

    /// A stream for tests and golden files, never opened by a generator.
    SELFTEST_STREAM: SelfTest = "selftest.stream";

    /// The key derivation of event tag `0x0001`,
    /// [`SELF_TEST`](crate::id::event_tags::SELF_TEST), which no generator emits.
    EVENT_SELFTEST: Event = "event.selftest";

    // Plan 02: the galaxy model. Every tag has scope `Galaxy` and draws one parameter of
    // `galaxy::params`. Progenitor numbers: the dominant merger 0, the lesser progenitors 1–5,
    // the recent ones from 6.

    // Scalar parameters, each opened with `ObjectKey::galaxy()`.

    /// The galaxy's stellar mass, log-uniform on 3–10 × 10¹⁰ M☉.
    GALAXY_PARAMS_STELLAR_MASS: Galaxy = "galaxy.params.stellar_mass";

    /// The thick disc's share of systems.
    GALAXY_PARAMS_SHARE_THICK: Galaxy = "galaxy.params.share.thick";

    /// The combined share of the bulge and the long bar.
    GALAXY_PARAMS_SHARE_BULGE_BAR: Galaxy = "galaxy.params.share.bulge_bar";

    /// The long bar's part of the bulge-and-bar share.
    GALAXY_PARAMS_SHARE_BAR_OF_BULGE: Galaxy = "galaxy.params.share.bar_of_bulge";

    /// The nuclear disc's share of systems.
    GALAXY_PARAMS_SHARE_NUCLEAR_DISC: Galaxy = "galaxy.params.share.nuclear_disc";

    /// The halo's share of systems.
    GALAXY_PARAMS_SHARE_HALO: Galaxy = "galaxy.params.share.halo";

    /// The timescale of the thin disc's declining formation rate.
    GALAXY_PARAMS_SFH_TIMESCALE: Galaxy = "galaxy.params.sfh.timescale";

    /// The scatter of the thin disc's scale length about its mass coupling, dex.
    GALAXY_PARAMS_THIN_LENGTH_SCATTER: Galaxy = "galaxy.params.thin.length.scatter";

    /// The old thin disc's mean scale height.
    GALAXY_PARAMS_THIN_MEAN_HEIGHT: Galaxy = "galaxy.params.thin.mean_height";

    /// The young thin disc's scale height.
    GALAXY_PARAMS_YOUNG_HEIGHT: Galaxy = "galaxy.params.young.height";

    /// The thick disc's scale length over the thin disc's.
    GALAXY_PARAMS_THICK_LENGTH_RATIO: Galaxy = "galaxy.params.thick.length_ratio";

    /// The thick disc's scale height over the thin disc's mean.
    GALAXY_PARAMS_THICK_HEIGHT_RATIO: Galaxy = "galaxy.params.thick.height_ratio";

    /// The scatter of the bulge's long scale length about its mass coupling, dex.
    GALAXY_PARAMS_BULGE_LENGTH_SCATTER: Galaxy = "galaxy.params.bulge.length.scatter";

    /// The bulge's middle axis over its long axis.
    GALAXY_PARAMS_BULGE_B_OVER_A: Galaxy = "galaxy.params.bulge.b_over_a";

    /// The bulge's short axis over its long axis.
    GALAXY_PARAMS_BULGE_C_OVER_A: Galaxy = "galaxy.params.bulge.c_over_a";

    /// The bulge's vertical exponent.
    GALAXY_PARAMS_BULGE_BOXINESS: Galaxy = "galaxy.params.bulge.boxiness";

    /// The scatter of the long bar's half-length about its mass coupling, dex.
    GALAXY_PARAMS_BAR_LENGTH_SCATTER: Galaxy = "galaxy.params.bar.length.scatter";

    /// The long bar's width over its half-length.
    GALAXY_PARAMS_BAR_WIDTH_RATIO: Galaxy = "galaxy.params.bar.width_ratio";

    /// The long bar's scale height.
    GALAXY_PARAMS_BAR_HEIGHT: Galaxy = "galaxy.params.bar.height";

    /// The bar's corotation radius over its half-length.
    GALAXY_PARAMS_BAR_COROTATION_RATIO: Galaxy = "galaxy.params.bar.corotation_ratio";

    /// The scatter of the nuclear disc's scale length about its mass coupling, dex.
    GALAXY_PARAMS_NUCLEAR_LENGTH_SCATTER: Galaxy = "galaxy.params.nuclear.length.scatter";

    /// The nuclear disc's scale height over its scale length.
    GALAXY_PARAMS_NUCLEAR_HEIGHT_RATIO: Galaxy = "galaxy.params.nuclear.height_ratio";

    /// The scatter of the nuclear cluster's mass, dex.
    GALAXY_PARAMS_NUCLEAR_CLUSTER_MASS_SCATTER: Galaxy = "galaxy.params.nuclear_cluster.mass.scatter";

    /// The number of spiral arms, two or four.
    GALAXY_PARAMS_ARMS_COUNT: Galaxy = "galaxy.params.arms.count";

    /// The arms' pitch angle.
    GALAXY_PARAMS_ARMS_PITCH: Galaxy = "galaxy.params.arms.pitch";

    /// The young disc's arm width `σ_w`.
    GALAXY_PARAMS_ARMS_YOUNG_WIDTH: Galaxy = "galaxy.params.arms.young_width";

    /// The young disc's arm amplitude A.
    GALAXY_PARAMS_ARMS_YOUNG_FRACTION: Galaxy = "galaxy.params.arms.young_fraction";

    /// The old discs' arm amplitude a.
    GALAXY_PARAMS_ARMS_OLD_AMPLITUDE: Galaxy = "galaxy.params.arms.old_amplitude";

    /// The gas disc's mass over the thin disc's stellar mass.
    GALAXY_PARAMS_GAS_MASS_FRACTION: Galaxy = "galaxy.params.gas.mass_fraction";

    /// The gas disc's scale length over the thin disc's.
    GALAXY_PARAMS_GAS_LENGTH_RATIO: Galaxy = "galaxy.params.gas.length_ratio";

    /// The stellar-to-halo mass factor f★, log-uniform.
    GALAXY_PARAMS_DARK_F_STAR: Galaxy = "galaxy.params.dark.f_star";

    /// The scatter of the dark halo's concentration, dex.
    GALAXY_PARAMS_DARK_CONCENTRATION_SCATTER: Galaxy = "galaxy.params.dark.concentration.scatter";

    /// The scatter of the black hole's mass about the M–σ relation, dex.
    GALAXY_PARAMS_BH_SCATTER: Galaxy = "galaxy.params.bh.scatter";

    /// The discs' radial metallicity gradient.
    GALAXY_PARAMS_METALLICITY_GRADIENT: Galaxy = "galaxy.params.metallicity.gradient";

    /// The in-situ halo's share of the halo, before renormalising.
    GALAXY_PARAMS_HALO_IN_SITU_SHARE: Galaxy = "galaxy.params.halo.in_situ.share";

    /// The in-situ halo's axis ratio.
    GALAXY_PARAMS_HALO_IN_SITU_FLATTENING: Galaxy = "galaxy.params.halo.in_situ.flattening";

    /// The in-situ halo's core radius.
    GALAXY_PARAMS_HALO_IN_SITU_CORE: Galaxy = "galaxy.params.halo.in_situ.core";

    /// The dominant merger's share of the halo, before renormalising.
    GALAXY_PARAMS_HALO_DOMINANT_SHARE: Galaxy = "galaxy.params.halo.dominant.share";

    /// The dominant merger's axis ratio.
    GALAXY_PARAMS_HALO_DOMINANT_FLATTENING: Galaxy = "galaxy.params.halo.dominant.flattening";

    /// The dominant merger's core radius.
    GALAXY_PARAMS_HALO_DOMINANT_CORE: Galaxy = "galaxy.params.halo.dominant.core";

    /// The dominant merger's break radius.
    GALAXY_PARAMS_HALO_DOMINANT_BREAK_RADIUS: Galaxy = "galaxy.params.halo.dominant.break_radius";

    /// How much the dominant merger's slope steepens beyond its break.
    GALAXY_PARAMS_HALO_DOMINANT_BREAK_STEEPENING: Galaxy = "galaxy.params.halo.dominant.break_steepening";

    /// The number of lesser old progenitors, 2–5.
    GALAXY_PARAMS_HALO_LESSER_COUNT: Galaxy = "galaxy.params.halo.lesser.count";

    /// The lesser progenitors' share of the halo together, before renormalising.
    GALAXY_PARAMS_HALO_LESSER_SHARE_TOTAL: Galaxy = "galaxy.params.halo.lesser.share_total";

    /// The globular-born debris' share of the halo, before renormalising.
    GALAXY_PARAMS_HALO_DEBRIS_SHARE: Galaxy = "galaxy.params.halo.debris.share";

    /// The globular-born debris' power-law slope.
    GALAXY_PARAMS_HALO_DEBRIS_SLOPE: Galaxy = "galaxy.params.halo.debris.slope";

    /// The globular-born debris' core radius.
    GALAXY_PARAMS_HALO_DEBRIS_CORE: Galaxy = "galaxy.params.halo.debris.core";

    /// The halo's discrete share: streams and dwarf cores.
    GALAXY_PARAMS_HALO_DISCRETE_SHARE: Galaxy = "galaxy.params.halo.discrete_share";

    /// The time of the last major merger.
    GALAXY_PARAMS_ACCRETION_LAST_MAJOR_MERGER: Galaxy = "galaxy.params.accretion.last_major_merger";

    /// The number of recent progenitors, Poisson.
    GALAXY_PARAMS_ACCRETION_RECENT_COUNT: Galaxy = "galaxy.params.accretion.recent.count";

    /// The scatter of the globular cluster count, dex.
    GALAXY_PARAMS_ACCRETION_GLOBULAR_COUNT_SCATTER: Galaxy = "galaxy.params.accretion.globular_count.scatter";

    // List parameters, each opened with `ObjectKey::galaxy_item(n)`.

    /// A halo component's slope; n is its place: in situ 0, dominant 1, lesser 2–6.
    GALAXY_PARAMS_HALO_COMPONENT_SLOPE: Galaxy = "galaxy.params.halo.component.slope";

    /// A halo component's age range; n as above, debris 7.
    GALAXY_PARAMS_HALO_COMPONENT_AGE: Galaxy = "galaxy.params.halo.component.age";

    /// A lesser halo component's \[Fe/H\]; n as above.
    GALAXY_PARAMS_HALO_COMPONENT_FEH: Galaxy = "galaxy.params.halo.component.feh";

    /// A lesser progenitor's stick-breaking fraction; n is its number, 1–5.
    GALAXY_PARAMS_HALO_LESSER_SPLIT: Galaxy = "galaxy.params.halo.lesser.split";

    /// A lesser progenitor's axis ratio; n is its number, 1–5.
    GALAXY_PARAMS_HALO_LESSER_FLATTENING: Galaxy = "galaxy.params.halo.lesser.flattening";

    /// A recent progenitor's stellar mass; n is its progenitor number.
    GALAXY_PARAMS_ACCRETION_PROGENITOR_MASS: Galaxy = "galaxy.params.accretion.progenitor.mass";

    /// A progenitor's accretion time; n is its progenitor number.
    GALAXY_PARAMS_ACCRETION_PROGENITOR_TIME: Galaxy = "galaxy.params.accretion.progenitor.time";

    /// A progenitor's orbit: apocentre, pericentre, inclination, node, phase.
    GALAXY_PARAMS_ACCRETION_PROGENITOR_ORBIT: Galaxy = "galaxy.params.accretion.progenitor.orbit";

    // Plan 03: placement. The cell's tag is keyed by the cell's word (`SystemId::cell_word`), every
    // other tag by the candidate's or the system's ID.

    /// A generation cell's candidate count, one Poisson draw.
    GALAXY_CELL_CANDIDATES: Cell = "galaxy.cell.candidates";

    /// A candidate's position in its cell: words 0–2, one per axis.
    GALAXY_CANDIDATE_POSITION: System = "galaxy.candidate.position";

    /// A candidate's acceptance mark, word 0, which thins it or picks its component.
    GALAXY_CANDIDATE_ACCEPT: System = "galaxy.candidate.accept";

    /// An accepted system's primary initial mass, within its layer's band.
    SYSTEM_PRIMARY_MASS: System = "system.primary_mass";

    /// An accepted system's signed age at the epoch.
    SYSTEM_AGE: System = "system.age";

    /// A system's velocity at the epoch: reserved for plan 08 and opened by nothing before it.
    SYSTEM_VELOCITY: System = "system.velocity";

    // Plan 06: the stars. Every `star.*` draw tag has scope `Body`, is opened with
    // `ObjectKey::from(BodyId)` (body 0 is a system's primary) and holds one fixed draw of
    // `stellar::draws::StarDraws`; attempt k of a redraw starts at word 64k.

    /// The Reimers mass-loss efficiency η: one standard normal.
    STAR_ETA: Body = "star.eta";

    /// The rotation rank: one uniform.
    STAR_ROTATION: Body = "star.rotation";

    /// Whether a fossil magnetic field is present, and its strength: one mark.
    STAR_MAGNETISM: Body = "star.magnetism";

    /// The spin axis: one isotropic direction (two words).
    STAR_SPIN_AXIS: Body = "star.spin_axis";

    /// The lifetime of the protostellar disc: one uniform.
    STAR_DISC_LIFETIME: Body = "star.disc_lifetime";

    /// The provisional companion-stripped mark (plan 11 conditions its binaries on it): one mark.
    STAR_STRIPPED: Body = "star.stripped";

    /// Neutron star or black hole after core collapse: one mark.
    STAR_REMNANT_TYPE: Body = "star.remnant.type";

    /// Whether a black hole forms by complete fallback: one mark.
    STAR_REMNANT_FALLBACK: Body = "star.remnant.fallback";

    /// The scatter of the remnant's mass: one standard normal.
    STAR_REMNANT_MASS: Body = "star.remnant.mass";

    /// The ordinary kick's score factor: successive standard normals, the first acceptable used.
    STAR_KICK_SCORE: Body = "star.kick.score";

    /// Whether a companion-stripped progenitor takes the low kick mode: one mark.
    STAR_KICK_MODE: Body = "star.kick.mode";

    /// The low kick mode's three Maxwellian components: three standard normals.
    STAR_KICK_LOW: Body = "star.kick.low";

    /// The kick's direction: one isotropic direction (two words).
    STAR_KICK_DIRECTION: Body = "star.kick.direction";

    /// A white dwarf's atmosphere, hydrogen or helium: one mark.
    STAR_WD_ATMOSPHERE: Body = "star.wd.atmosphere";

    /// Whether a cool helium-atmosphere white dwarf shows carbon (DQ): one mark.
    STAR_WD_CARBON: Body = "star.wd.carbon";

    /// Whether a white dwarf is polluted by metals: one mark.
    STAR_WD_METALS: Body = "star.wd.metals";

    /// A neutron star's birth spin period: successive standard normals, the first acceptable used.
    STAR_NS_SPIN: Body = "star.ns.spin";

    /// A neutron star's birth dipole field: one standard normal.
    STAR_NS_FIELD: Body = "star.ns.field";

    /// A neutron star's spin axis (two words) and magnetic inclination (one uniform).
    STAR_NS_GEOMETRY: Body = "star.ns.geometry";

    /// A pulsar's rotational phase at the epoch: one uniform.
    STAR_NS_PHASE: Body = "star.ns.phase";

    /// A black hole's spin: one standard normal.
    STAR_BH_SPIN: Body = "star.bh.spin";

    /// A planetary nebula's expansion speed: one uniform.
    STAR_NEBULA: Body = "star.nebula";

    // Plan 06, the system's own draws (P06.T3): scope `System`, opened with
    // `ObjectKey::from(SystemId)`, one stream per system shared by all its stars.

    /// A grid system's iron abundance \[Fe/H\]: one standard normal (two words) scaling its
    /// density component's scatter about its mean (`stellar::system::draw_metallicity`).
    SYSTEM_METALLICITY: System = "system.metallicity";

    // Plan 06, event tags (P06.T27.a): each backs an entry of `id/event_tags.rs` in the block
    // 0x0100–0x01FF, numbered in this order. An event's marks come from its own event stream, so
    // no kind has a second tag.

    /// Flares of stars with convective envelopes (Poisson bins), event tag `0x0100`.
    STAR_EV_FLARE: Event = "star.ev.flare";

    /// Glitches of Crab-like pulsars (Poisson bins), event tag `0x0101`.
    STAR_EV_GLITCH: Event = "star.ev.glitch";

    /// Glitches of Vela-like pulsars (monotone phase), event tag `0x0102`.
    STAR_EV_GLITCH_CYCLE: Event = "star.ev.glitch_cycle";

    /// A magnetar's active episodes (Poisson bins), event tag `0x0103`.
    STAR_EV_MAGNETAR_EPISODE: Event = "star.ev.magnetar_episode";

    /// A magnetar's short bursts within an episode (Poisson bins), event tag `0x0104`.
    STAR_EV_MAGNETAR_BURST: Event = "star.ev.magnetar_burst";

    /// A magnetar's giant flares (Poisson bins), event tag `0x0105`.
    STAR_EV_MAGNETAR_GIANT: Event = "star.ev.magnetar_giant";

    /// FU Orionis outbursts of young stars (Poisson bins), event tag `0x0106`.
    STAR_EV_FU_ORIONIS: Event = "star.ev.fu_orionis";

    /// Giant eruptions of luminous blue variables (Poisson bins), event tag `0x0107`.
    STAR_EV_LBV_ERUPTION: Event = "star.ev.lbv_eruption";

    /// Thermal pulses on the asymptotic giant branch (monotone phase), event tag `0x0108`.
    STAR_EV_THERMAL_PULSE: Event = "star.ev.thermal_pulse";

    /// The cycle-keyed irregularity of pulsating variables, S Doradus cycles included (monotone
    /// phase), event tag `0x0109`.
    STAR_VAR_CYCLE: Event = "star.var.cycle";

    // Plan 07: the gas and dust field. Both tags have scope `Galaxy`. This heading is appended
    // after plan 06's event tags because the macro's order fixes `ALL`, which
    // `tests/golden/rng/tags.golden` pins.

    /// The gas parameters plan 02 does not draw, one uniform per word at the parameter's fixed
    /// index in plan 07's Design note 3 table, keyed by `ObjectKey::galaxy()`.
    GAS_PARAMS: Galaxy = "gas.params";

    /// The gas's lattice noise, one standard normal per lattice point of each octave, each on its
    /// own `ObjectKey::galaxy_item` of the point's packed lattice word (plan 07, Design note 8).
    GAS_NOISE: Galaxy = "gas.noise";

    // Plan 14: planetary systems. Every name the plan uses is fixed here, so that no later task
    // picks another; each entry arrives with the task that first opens a stream under it.
    //
    // Scope `System`, opened with `ObjectKey::from(SystemId)`, the orbit host and the planet's
    // slot in the draw number (plan 14, design note 4): `planet.disc`, `planet.plane`,
    // `planet.class`, `planet.count`, `planet.spacing`, `planet.mass`, `planet.secondgen`,
    // `belt.population`, `cometary.population`.
    //
    // Scope `Body`, opened with `ObjectKey::from(BodyId)`: `planet.orbit`, `planet.radius`,
    // `planet.volatiles`, `planet.spin`, `planet.origin`, `planet.scatter` (ruling 80),
    // `moon.count`, `moon.mass`, `moon.orbit`, `moon.impact`, `moon.capture`, `ring.system`,
    // `belt.member`, `body.surface`, `body.resources`.
    //
    // Scope `Event`, each behind an event tag of plan 06's block 0x0400–0x04FF: `body.impact`
    // (0x0400), `body.eruption` (0x0401), `body.storm` (0x0402), `body.duststorm` (0x0403),
    // `system.comet` (0x0404).

    /// A host's protoplanetary disc (P14.T3): its gas mass, corotation period and characteristic
    /// radius, three standard normals, then the rank of a circumbinary disc's lifetime, one
    /// uniform; orbit host h reads words 16h onwards (`planetary::disc::DISC_WORDS_PER_HOST`).
    PLANET_DISC: System = "planet.disc";

    /// Planet spacings in mutual Hill radii (P14.T6.b): orbit host h's mean spacings, three
    /// standard normals from word 8h, and the spacing of the pair whose outer planet is in slot s,
    /// up to seventeen standard normals from word 2,048 + 64s
    /// (`planetary::placement::spacing`).
    PLANET_SPACING: System = "planet.spacing";

    /// An orbit host's architecture class (P14.T4.c): one mark, picked against the class weights;
    /// orbit host h reads word 4h (`planetary::architecture::CLASS_WORDS_PER_HOST`), and words
    /// 4h + 1 to 4h + 3 are reserved.
    PLANET_CLASS: System = "planet.class";

    /// Planet masses (P14.T7): the planet in slot s reads words 8s onwards
    /// (`planetary::placement::masses::MASS_WORDS_PER_SLOT`), its own within-system scatter and
    /// its group's between-system scatter, two standard normals, then the rank of a group's mass
    /// law, one uniform.
    PLANET_MASS: System = "planet.mass";

    /// The group-level draws of a host's class placer (P14.T8): whether each group of its class
    /// template is present, how many bodies it places and where its first body goes, the chain's
    /// dynamically hot variant and resonance, and a flanking group's side; orbit host h reads words
    /// 64h onwards, and the pair whose outer planet is in slot s its resonance offset at word
    /// 16,384 + 4s (`planetary::placement::classes`).
    PLANET_COUNT: System = "planet.count";

    /// An orbit host's planetary plane (P14.T8.d): its normal, isotropic, from two uniforms; orbit
    /// host h reads words 4h and 4h + 1 (`planetary::placement::classes::orbits`).
    PLANET_PLANE: System = "planet.plane";

    /// A planet's own orbit (P14.T8.d): the ranks of its eccentricity and of its inclination to its
    /// host's plane, and the uniform angles of its node on that plane, its periapsis and its mean
    /// anomaly at the epoch, words 0–4 (`planetary::placement::classes::orbits`).
    PLANET_ORBIT: Body = "planet.orbit";

    /// A planet's formation (P14.T28.a): the rank of a giant's formation age between 0.5 Myr and
    /// its disc's lifetime, word 0, and the rank of the host age at which a terrestrial planet's
    /// magma ocean ends, word 1, one uniform each, drawn for every planet; words 2–7 are reserved
    /// (`planetary::hosts::young::FormationDraws`).
    PLANET_ORIGIN: Body = "planet.origin";

    /// A planet's radius rank (P14.T30.a, design note 8): the one uniform that places it within
    /// Chen and Kipping's scatter at its mass, or for a rocky outcome within the observed spread of
    /// core fractions, word 0; words 1–7 are reserved (`planetary::system::RADIUS_WORDS`).
    PLANET_RADIUS: Body = "planet.radius";

    /// What a planet takes from each ejection it survives after a supernova (P14.T28.c, ruling
    /// 80): its k-th such ejection reads words 4k to 4k + 3, the rank of its new eccentricity in
    /// Ford and Rasio's (2008) truncated normal, word 4k, and its mean anomaly at the death as a
    /// share of a turn, word 4k + 1, one open uniform each; words 4k + 2 and 4k + 3 are reserved
    /// (`planetary::fate::ScatterDraws`).
    PLANET_SCATTER: Body = "planet.scatter";
    /// A giant's regular satellite system (P14.T17.a), on the planet's own key: the standard
    /// normal of its total mass, words 0–1; the rank of its count of major moons, word 2; the
    /// count of its small inner moonlets, a Poisson draw from word 3; whether it is Titan-like and
    /// its outermost moon's share if so, one uniform each at words 4 and 5; words 6–7 are reserved
    /// (`planetary::moons::regular`).
    MOON_COUNT: Body = "moon.count";

    /// The masses of a giant's regular moons (P14.T17.a): moon k, counted from 1 inside out,
    /// reads its within-system scatter, a standard normal, at words 2(k − 1) and 2(k − 1) + 1
    /// (`planetary::moons::regular`).
    MOON_MASS: Body = "moon.mass";

    /// The orbits of a giant's regular moons (P14.T17.a): the innermost moon's place, one uniform
    /// at word 0, words 1–15 reserved; then moon k, from 1 inside out, at words 16 + 64(k − 1)
    /// onwards: its spacing from its inner neighbour, seventeen attempts of two words each, the
    /// rank of that pair's resonance, its free and forced eccentricities, its inclination, and
    /// its node, periapsis and mean anomaly, one uniform each at offsets 34–40
    /// (`planetary::moons::regular`).
    MOON_ORBIT: Body = "moon.orbit";

    /// A planet's or dwarf planet's giant-impact moon (P14.T18): whether it has one, the rank of
    /// its mass ratio, its mean anomaly at the epoch, the rank of its formation distance and, for
    /// a dwarf planet, its class, one uniform each at words 0–4; words 5–7 are reserved
    /// (`planetary::moons::impact`).
    MOON_IMPACT: Body = "moon.impact";

    /// A planet's captured moons (P14.T19): a giant's population count, a standard normal at
    /// words 0–1, and the rank of its largest member's diameter, word 2; whether a large capture (an ice giant's) or small ones (a rocky planet's)
    /// happened, and how many, words 8 and 9; a large capture's mass ratio, axis, inclination and
    /// angles, words 16–21; and captured body k, from 1, at words 32 + 16(k − 1) onwards: its size
    /// (a giant's body 1 is its largest member and reads none), sense, inclination, axis, eccentricity, node, periapsis and mean anomaly, one uniform each
    /// (`planetary::moons::irregular`).
    MOON_CAPTURE: Body = "moon.capture";

    // Plan 11: multiplicity and binaries. Every name the plan uses is fixed in its Provides; each
    // entry arrives with the task that first opens a stream under it. Attempt n of a redraw
    // (`stellar::multiplicity::RedrawAttempt`) reads words 64n to 64n + 63 of every stream here.
    //
    // Scope `System`, opened with `ObjectKey::from(SystemId)`: `system.multiplicity`,
    // `system.hierarchy` (reserved: the hierarchy draw picks each companion's node with its
    // period, on `binary.orbit`), `system.substellar`.
    //
    // Scope `Body`, opened with `ObjectKey::from(BodyId)` of the star an orbit brings in, the
    // lowest-indexed star of the pair's outer member (plan 11, design note 5): `binary.orbit`,
    // `binary.orientation`, `binary.phase`, `binary.kick`, `binary.ia_mark`, `binary.ce`.
    //
    // Scope `System`, under plan 09's prefix `class.`: `class.awd`, `class.xrb`, `class.merger`,
    // `class.nsm`.

    /// Whether a system is multiple and how many stellar companions it has: one mark each, words
    /// 64n and 64n + 1 of attempt n (`stellar::multiplicity::draw_hierarchy`).
    SYSTEM_MULTIPLICITY: System = "system.multiplicity";

    /// A companion's node and period (one mark, picking the node among the candidates' period
    /// windows and the period inside it), mass ratio and eccentricity (one uniform each): try r
    /// of attempt n at words 64n + 3r to 64n + 3r + 2.
    BINARY_ORBIT: Body = "binary.orbit";

    /// A companion's orbital plane and periapsis, isotropic: the cosine of the inclination, the
    /// ascending node and the argument of periapsis, one uniform each, try r of attempt n at words
    /// 64n + 3r to 64n + 3r + 2.
    BINARY_ORIENTATION: Body = "binary.orientation";

    /// A companion's mean anomaly at the epoch: one uniform, try r of attempt n at word 64n + r.
    BINARY_PHASE: Body = "binary.phase";

    // Plan 14, phase D: rings, belts and the cometary halo (P14.T20, T21), under the names and
    // scopes of the "Plan 14" heading above. Appended here, after plan 11's entries, because the
    // macro's order fixes `ALL`.

    /// A giant's rings (P14.T20), keyed by the planet: the massive ring's mark, word 0, and the
    /// ranks of its outer edge, its mass, the dusty ring's outer edge and its optical depth, words
    /// 1–4, one uniform each; words 5–7 are reserved (`planetary::rings::RingDraws`).
    RING_SYSTEM: Body = "ring.system";

    /// A system's belts (P14.T21.a–b): the belt in belt slot n reads words 8n onwards, the ranks of
    /// its depletion factor and of its size slope, one uniform each, a Kuiper-like belt's bright
    /// mark and its bright efficiency's standard normal (words 8n + 3 and 8n + 4), and the rank of
    /// its largest member's share (word 8n + 5, ruling 100.3); words 8n + 6 and 8n + 7 are
    /// reserved (`planetary::belts::BeltDraws`).
    BELT_POPULATION: System = "belt.population";

    /// A belt's named member (P14.T21.c), keyed by the member: the rank of its component, its
    /// semi-major axis, eccentricity and inclination, its node, periapsis and mean anomaly, and
    /// its radius rank, words 0–7, and the growth-cap spread of the rank law's candidate of its
    /// sub-index, word 8 (ruling 100.4), one uniform each (`planetary::belts::MemberDraws`).
    BELT_MEMBER: Body = "belt.member";

    /// A system's cometary halo (P14.T21.d): the rank of its number of comets, word 0, one
    /// uniform; words 1–7 are reserved (`planetary::halo`).
    COMETARY_POPULATION: System = "cometary.population";

    // Plan 06's reference population (P06.T19.b), under the name and scope the "Plan 06" heading
    // reserves. Appended here, after plan 14's phase D, because the macro's order fixes `ALL`.

    /// The kick law's reference population and its test sample (P06.T19.b, T19.d), keyed by
    /// `ObjectKey::galaxy_item(i)` of sample i: its initial mass's rank, word 0; Reimers η's
    /// standard normal, words 1–2; the remnant's type and fallback marks, words 3 and 4, and its
    /// mass's standard normal, words 5–6; the kick score's eight tries, words 7–22; the stripped
    /// and kick-mode marks, words 23 and 24; the low mode's three normals, words 25–30; the kick's
    /// direction, words 31–32; the test-only toy binary's separation rank, word 33; words 34–63
    /// are reserved (`stellar::remnant::reference`).
    STELLAR_REFERENCE: Galaxy = "stellar.reference";

    // Plan 08: velocities, kicks and displaced objects (P08.T1). Appended here, after plan 06's
    // reference population, because the macro's order fixes `ALL`. `system.velocity` is plan 03's
    // reserved tag above, which plan 08 opens. Scope `System` tags are opened with
    // `ObjectKey::from(SystemId)`; `halo.kinematics` with `ObjectKey::galaxy_item(n)`, n the halo
    // component's item number (`HaloComponentKind::item`).

    /// A displaced or layer-E record's kind (alive, retained, a displaced class's remnant, runaway
    /// or walkaway), and layer D's runaway reduction: one mark per attempt (P08.T12.c).
    DISPLACED_KIND: System = "displaced.kind";

    /// A displaced or layer-E record's initial mass from its class's conditional table: one
    /// uniform per attempt (P08.T12.c).
    DISPLACED_MASS: System = "displaced.mass";

    /// A displaced or retained record's time since death, and its birth component: one uniform
    /// each per attempt (P08.T12.c).
    DISPLACED_DEATH: System = "displaced.death";

    /// A displaced record's formation radius in the mixed age bins: one uniform (P08.T12.c).
    DISPLACED_BIRTH: System = "displaced.birth";

    /// The speed bin an unbound record's kick is drawn in, for the fastest classes: one mark
    /// (P08.T12.c).
    DISPLACED_ORIGIN_BIN: System = "displaced.origin_bin";

    /// A runaway or walkaway's ejection speed: one uniform (P08.T12.c).
    RUNAWAY_SPEED: System = "runaway.speed";

    /// A runaway or walkaway's ejection channel and age: one mark and two uniforms (P08.T12.c).
    RUNAWAY_EJECTION: System = "runaway.ejection";

    /// A lesser halo progenitor's kinematics (P08.T3): its anisotropy β, uniform on 0.3–0.7, word
    /// 0; its net rotation, uniform on ±0.25 `v_c`, word 1.
    HALO_KINEMATICS: Galaxy = "halo.kinematics";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_two_tags_share_a_name_or_a_hash() {
        for (i, a) in ALL.iter().enumerate() {
            for b in &ALL[..i] {
                assert_ne!(a.name(), b.name());
                assert_ne!(a.hash(), b.hash(), "{} and {}", a.name(), b.name());
            }
        }
    }

    #[test]
    fn plan_06_registers_its_star_draw_tags_with_body_scope() {
        let star = [
            STAR_ETA,
            STAR_ROTATION,
            STAR_MAGNETISM,
            STAR_SPIN_AXIS,
            STAR_DISC_LIFETIME,
            STAR_STRIPPED,
            STAR_REMNANT_TYPE,
            STAR_REMNANT_FALLBACK,
            STAR_REMNANT_MASS,
            STAR_KICK_SCORE,
            STAR_KICK_MODE,
            STAR_KICK_LOW,
            STAR_KICK_DIRECTION,
            STAR_WD_ATMOSPHERE,
            STAR_WD_CARBON,
            STAR_WD_METALS,
            STAR_NS_SPIN,
            STAR_NS_FIELD,
            STAR_NS_GEOMETRY,
            STAR_NS_PHASE,
            STAR_BH_SPIN,
            STAR_NEBULA,
        ];
        for tag in star {
            assert_eq!(tag.scope(), crate::rng::TagScope::Body, "{}", tag.name());
            assert!(tag.name().starts_with("star."), "{}", tag.name());
            assert!(ALL.contains(&tag));
        }
    }

    #[test]
    fn plan_06_registers_the_system_metallicity_tag_with_system_scope() {
        assert_eq!(SYSTEM_METALLICITY.name(), "system.metallicity");
        assert_eq!(SYSTEM_METALLICITY.scope(), crate::rng::TagScope::System);
        assert!(ALL.contains(&SYSTEM_METALLICITY));
    }

    #[test]
    fn plan_07_registers_the_gas_parameter_tag() {
        assert_eq!(GAS_PARAMS.name(), "gas.params");
        assert_eq!(GAS_PARAMS.scope(), crate::rng::TagScope::Galaxy);
        assert!(ALL.contains(&GAS_PARAMS));
    }

    #[test]
    fn plan_07_registers_the_gas_noise_tag_after_its_parameters() {
        assert_eq!(GAS_NOISE.name(), "gas.noise");
        assert_eq!(GAS_NOISE.scope(), crate::rng::TagScope::Galaxy);
        let noise = ALL.iter().position(|t| *t == GAS_NOISE);
        let params = ALL.iter().position(|t| *t == GAS_PARAMS);
        assert_eq!(noise, params.map(|p| p + 1));
    }

    #[test]
    fn plan_14_registers_the_disc_tag_with_system_scope() {
        assert_eq!(PLANET_DISC.name(), "planet.disc");
        assert_eq!(PLANET_DISC.scope(), crate::rng::TagScope::System);
        assert!(ALL.contains(&PLANET_DISC));
    }

    #[test]
    fn plan_11_registers_its_hierarchy_tags_with_their_scopes() {
        use crate::rng::TagScope;
        let tags = [
            (SYSTEM_MULTIPLICITY, "system.multiplicity", TagScope::System),
            (BINARY_ORBIT, "binary.orbit", TagScope::Body),
            (BINARY_ORIENTATION, "binary.orientation", TagScope::Body),
            (BINARY_PHASE, "binary.phase", TagScope::Body),
        ];
        for (tag, name, scope) in tags {
            assert_eq!(tag.name(), name);
            assert_eq!(tag.scope(), scope, "{name}");
            assert!(ALL.contains(&tag), "{name}");
        }
    }

    #[test]
    fn plan_14_registers_the_spacing_tag_with_system_scope() {
        assert_eq!(PLANET_SPACING.name(), "planet.spacing");
        assert_eq!(PLANET_SPACING.scope(), crate::rng::TagScope::System);
        assert!(ALL.contains(&PLANET_SPACING));
    }

    #[test]
    fn plan_14_registers_the_class_tag_with_system_scope() {
        assert_eq!(PLANET_CLASS.name(), "planet.class");
        assert_eq!(PLANET_CLASS.scope(), crate::rng::TagScope::System);
        assert!(ALL.contains(&PLANET_CLASS));
    }

    #[test]
    fn plan_14_registers_the_mass_tag_with_system_scope() {
        assert_eq!(PLANET_MASS.name(), "planet.mass");
        assert_eq!(PLANET_MASS.scope(), crate::rng::TagScope::System);
        assert!(ALL.contains(&PLANET_MASS));
    }

    #[test]
    fn plan_14_registers_the_placer_s_three_tags() {
        assert_eq!(PLANET_COUNT.name(), "planet.count");
        assert_eq!(PLANET_COUNT.scope(), crate::rng::TagScope::System);
        assert_eq!(PLANET_PLANE.name(), "planet.plane");
        assert_eq!(PLANET_PLANE.scope(), crate::rng::TagScope::System);
        assert_eq!(PLANET_ORBIT.name(), "planet.orbit");
        assert_eq!(PLANET_ORBIT.scope(), crate::rng::TagScope::Body);
        assert!(
            [PLANET_COUNT, PLANET_PLANE, PLANET_ORBIT]
                .iter()
                .all(|tag| ALL.contains(tag))
        );
    }

    #[test]
    fn plan_14_registers_the_radius_tag_with_body_scope() {
        assert_eq!(PLANET_RADIUS.name(), "planet.radius");
        assert_eq!(PLANET_RADIUS.scope(), crate::rng::TagScope::Body);
        assert!(ALL.contains(&PLANET_RADIUS));
    }

    #[test]
    fn plan_14_registers_the_scatter_tag_with_body_scope() {
        assert_eq!(PLANET_SCATTER.name(), "planet.scatter");
        assert_eq!(PLANET_SCATTER.scope(), crate::rng::TagScope::Body);
        assert!(ALL.contains(&PLANET_SCATTER));
    }

    #[test]
    fn plan_14_registers_the_origin_tag_with_body_scope() {
        assert_eq!(PLANET_ORIGIN.name(), "planet.origin");
        assert_eq!(PLANET_ORIGIN.scope(), crate::rng::TagScope::Body);
        assert!(ALL.contains(&PLANET_ORIGIN));
    }

    #[test]
    fn plan_01_registers_its_two_tags() {
        assert_eq!(SELFTEST_STREAM.name(), "selftest.stream");
        assert_eq!(SELFTEST_STREAM.scope(), crate::rng::TagScope::SelfTest);
        assert_eq!(EVENT_SELFTEST.name(), "event.selftest");
        assert_eq!(EVENT_SELFTEST.scope(), crate::rng::TagScope::Event);
        assert!(ALL.contains(&SELFTEST_STREAM) && ALL.contains(&EVENT_SELFTEST));
    }

    #[test]
    fn plan_06_registers_the_reference_tag_with_galaxy_scope() {
        assert_eq!(STELLAR_REFERENCE.name(), "stellar.reference");
        assert_eq!(STELLAR_REFERENCE.scope(), crate::rng::TagScope::Galaxy);
        assert!(ALL.contains(&STELLAR_REFERENCE));
    }

    #[test]
    fn plan_08_registers_its_tags_with_their_scopes() {
        let system = [
            DISPLACED_KIND,
            DISPLACED_MASS,
            DISPLACED_DEATH,
            DISPLACED_BIRTH,
            DISPLACED_ORIGIN_BIN,
            RUNAWAY_SPEED,
            RUNAWAY_EJECTION,
        ];
        for tag in system {
            assert_eq!(tag.scope(), crate::rng::TagScope::System, "{}", tag.name());
            assert!(ALL.contains(&tag));
        }
        assert_eq!(SYSTEM_VELOCITY.name(), "system.velocity");
        assert_eq!(SYSTEM_VELOCITY.scope(), crate::rng::TagScope::System);
        assert_eq!(HALO_KINEMATICS.name(), "halo.kinematics");
        assert_eq!(HALO_KINEMATICS.scope(), crate::rng::TagScope::Galaxy);
        assert_eq!(ALL.last(), Some(&HALO_KINEMATICS));
    }
}
