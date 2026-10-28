//! A twin of ruling 133.5's guard, `planetary_properties::no_planet_hotter_than_its_star`
//! (P14.T16.b), inside the crate, so that it runs under the escape's batch and with every 24 hold
//! in force, which an integration test cannot set: P14.T53.a's thick-air closure's acceptance
//! (decision-p14-t53a-thick-co2 §7.1, §7.2 and the guard's part of §7.5).
//!
//! Its systems, hosts, stars and draws are the integration test's, built as
//! `tests/planetary_support` builds them and walked in the same order, so that every sample is
//! the same body at the same time: the Milky Way fixture at seed `0x5eed_0000_0014_0016`, 600
//! systems at each of 0.1, 1, 5 and 12 Gyr near the Sun-like point, each planet's radius and
//! volatile ranks drawn from one generator, each derived at −H, 0 and +H. The guard places no
//! young magma ocean (its bodies carry no `molten_until`), so every crust it derives is solid.
//!
//! It copies that support and the guard's own helpers, about 250 lines: a change to either is
//! made here too. The 39 bodies' test, which finds each by name at all three times, fails if they
//! drift apart.

use hyperion_testkit::lcg::Lcg;

use super::{
    Gas, GreenhouseCeiling, GreenhouseConstants, PASCALS_PER_BAR, SILICATE_SOLIDUS, SurfaceState,
    THICK_AIR_ANCHOR_PRESSURE, VolatileDraws, grey_surface_temperature,
    water_saturation_temperature, without_thick_air,
};
use crate::Seed;
use crate::coords::GalacticPosition;
use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{CellKey, SystemOrigin, SystemRecord};
use crate::id::{Layer, SystemId};
use crate::planetary::architecture::template::EARTH_MASSES_PER_JUPITER_MASS;
use crate::planetary::architecture::{CLOSE_BINARY_CUTOFF_AU, class_weights, draw_class};
use crate::planetary::context::XuvHistory;
use crate::planetary::derive::habitable_zone::HabitableLimit;
use crate::planetary::derive::passes::Convergence;
use crate::planetary::derive::{BodyHosts, DerivedBody, HostLight, PlacedBody, derive_body};
use crate::planetary::disc::Disc;
use crate::planetary::placement::classes::orbits::{HostPlane, SystemPlane};
use crate::planetary::placement::{
    HostPlacement, OrbitHost, OrbitZone, PlacedPlanet, PlacementHost, ZoneDiscInputs,
    ZoneHierarchy, ZoneStar, place, stable_zones,
};
use crate::planetary::testing::with_every_24_hold;
use crate::stellar::Composition;
use crate::stellar::draws::{StarDraws, UnitUniform};
use crate::stellar::multiplicity::{
    HierarchyNode, MultiplicityContext, NodeIndex, RedrawAttempt, SystemHierarchy, draw_hierarchy,
};
use crate::stellar::sse::{MIN_INITIAL_MASS, ZCoeffs, zams};
use crate::stellar::substellar;
use crate::stellar::system::{StarModel, draw_metallicity};
use crate::stellar::{Phase, StarState};
use crate::time::{CLOCK_WINDOW_H, UniverseTime};
use crate::units::{
    AstronomicalUnits, Dex, EarthMasses, HeliumExcess, Kelvin, Kilograms, Metres, Pascals,
    SolarMasses, Years,
};

/// The integration test's galaxy seed.
const SEED: Seed = Seed::new(0x5eed_0000_0014_0016);

/// The 39 bodies whose surfaces the escape's batch held at their star's effective temperature at
/// all three times, 117 of the guard's 30,063 samples, before the thick-air closure (B3's k-2b:
/// `ids-s02.txt`, step 02, identical at every later step): each system's ID and the body's index,
/// all under 764–16,400 bar of nearly pure carbon dioxide (decision-p14-t53a-thick-co2 §5).
const SATURATED_UNDER_THE_BATCH: [(&str, u16); 39] = [
    ("42002cb200000015", 0x0700),
    ("42002cb200000081", 0x0300),
    ("42002cb2000000aa", 0x0200),
    ("42002cb2000000ed", 0x0100),
    ("42002cb2000000fe", 0x0100),
    ("42002cb200000107", 0x0100),
    ("42002cb200000118", 0x0100),
    ("42002cb20000011f", 0x0100),
    ("42002cb20000012a", 0x0300),
    ("42002cb20000016d", 0x0200),
    ("42002cb20000017d", 0x0100),
    ("42002cb200000181", 0x0700),
    ("42002cb200000181", 0x0800),
    ("42002cb20000018f", 0x0600),
    ("42002cb200000197", 0x0100),
    ("42002cb2000001d1", 0x0100),
    ("42002cb2000001ef", 0x0c00),
    ("42002cb2000001f5", 0x0200),
    ("42002cb200000203", 0x0b00),
    ("42002cb200000207", 0x0200),
    ("42002cb200000214", 0x0100),
    ("42002cb200000243", 0x0100),
    ("42002cb200000251", 0x0100),
    ("42002cb200000252", 0x0200),
    ("42002cb2000186a3", 0x0200),
    ("42002cb2000187b5", 0x0400),
    ("42002cb20001881f", 0x0100),
    ("42002cb2000188e8", 0x0100),
    ("42002cb200030d83", 0x0400),
    ("42002cb200030e0f", 0x0100),
    ("42002cb200030e2d", 0x0200),
    ("42002cb200030e43", 0x0200),
    ("42002cb200030e52", 0x0200),
    ("42002cb200030f8d", 0x0200),
    ("42002cb200049423", 0x0100),
    ("42002cb200049426", 0x0100),
    ("42002cb20004946d", 0x0300),
    ("42002cb200049487", 0x0e00),
    ("42002cb2000495a9", 0x0100),
];

/// The nine bodies the thick-air closure puts on a bracketed jump, each with its first round's
/// residual on P14.T54.b.k's tree (decision-p14-t53a-thick-co2 §11.1; 42002cb200018844.0400 jumps
/// there without the closure too, at 0.0026): the guard's six and the systems' three, every one
/// an icy water world published `Snowball` at 647–811 K under 2.9 kbar to 2.2 Mbar, in P14.T53.b
/// part 2's section, where the column's water steps from its saturation to all airborne at its
/// critical point. Recorded jumps, as `close_binary` 0800's is; their owner is P14.T53.b part 3 at
/// 25, whose cap at the critical isochore removes the step. A new jump elsewhere, one that leaves
/// the section, or one whose state moves goes to main.
pub(crate) const CLOSURE_JUMPS: [(&str, f64); 9] = [
    ("42002cb20000010c.0900", 0.009_17),
    ("42002cb2000001b1.0500", 0.013_51),
    ("42002cb2000187b7.0700", 0.027_70),
    ("42002cb200018844.0400", 0.054_49),
    ("42002cb200030f55.0500", 0.021_84),
    ("42002cb200049556.0500", 0.039_41),
    ("02001b2ca0020000.0200", 0.011_73),
    ("02000b2ce0040000.0600", 0.020_37),
    ("02000b2d9ffe0000.0700", 0.028_64),
];

/// Whether the body `id` may take a new jump under the thick-air closure ([`CLOSURE_JUMPS`]), the
/// twin's jump exemption: by name, and only where its published state is the one it had without
/// the closure and a supercritical reservoir hides its surface (part 2's section).
pub(crate) fn closure_jump_exempt(id: &str, before: &DerivedBody, after: &DerivedBody) -> bool {
    CLOSURE_JUMPS.iter().any(|&(name, _)| name == id)
        && before.atmosphere().state() == after.atmosphere().state()
        && after
            .atmosphere()
            .supercritical_reservoir(after.surface_gravity())
            .is_some()
}

/// Count (xvii)'s tally (decision-p14-t53a-thick-co2 §11.2): the `MagmaOcean` bodies whose
/// greenhouse solve held O₂, which the melt takes only as the published air is finished
/// (`escaping::finish`), each's published `T_s` against its published air's own surface: how many,
/// how many differ by more than 1%, the largest difference, and how many of those airs lie under
/// the silicate solidus, by name.
#[derive(Debug, Default)]
pub(crate) struct MeltOxygen {
    bodies: u32,
    above_one_percent: u32,
    largest: (f64, String),
    under_solidus: Vec<String>,
}

impl MeltOxygen {
    /// Counts the derived body `body` of the ID `id` about a hottest host at `hottest`, if it is
    /// a `MagmaOcean` whose airborne inventory holds O₂: its published air's surface is
    /// `GreenhouseCeiling::surface_temperature` over that air's τ at its equilibrium temperature,
    /// no ceiling on a molten crust, held at `hottest` (the ruled follow-up's evaluation, before
    /// its solidus).
    pub(crate) fn add(&mut self, id: &str, body: &DerivedBody, hottest: Kelvin) {
        let air = body.atmosphere();
        if air.state() != SurfaceState::MagmaOcean
            || air.inventory_weights().of(Gas::Oxygen).value() <= 0.0
        {
            return;
        }
        let constants = GreenhouseConstants::in_force();
        let (partials, base, gravity) = (
            air.partial_pressures(),
            body.equilibrium_temperature(),
            body.surface_gravity(),
        );
        let tau = constants.optical_depth(partials, base, gravity);
        let published_air = GreenhouseCeiling::None
            .surface_temperature(&constants, (partials, tau), base, gravity)
            .value()
            .min(hottest.value());
        let t_s = air.surface_temperature().value();
        self.bodies += 1;
        self.above_one_percent += u32::from((t_s / published_air - 1.0).abs() > 0.01);
        if (t_s - published_air).abs() > self.largest.0 {
            self.largest = (
                (t_s - published_air).abs(),
                format!("{id}: {t_s:.1} K against {published_air:.1} K"),
            );
        }
        if published_air < SILICATE_SOLIDUS.value() {
            self.under_solidus.push(format!(
                "{id}: {t_s:.1} K, its published air {published_air:.1} K"
            ));
        }
    }

    /// The tally, for a census line.
    pub(crate) fn line(&self) -> String {
        format!(
            "{} molten bodies with O₂ in their solve; {} differ by more than 1% from their \
             published air's surface, the largest by {:.1} K ({}); {} whose published air lies \
             under the solidus: [{}]",
            self.bodies,
            self.above_one_percent,
            self.largest.0,
            self.largest.1,
            self.under_solidus.len(),
            self.under_solidus.join("; ")
        )
    }
}

/// The Milky Way fixture every sample is drawn in (`planetary_support::galaxy`).
fn galaxy() -> Galaxy {
    Galaxy::from_params(SEED, GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral")
}

/// The record of candidate `index` of a run of layer C cells near the Sun-like point, 26,000 ly
/// out on the +y axis in the plane, whose primary formed with `mass` M☉ `age` ago
/// (`planetary_support::record`).
fn record(galaxy: &Galaxy, index: u32, mass: f64, age: Years) -> SystemRecord {
    let first = CellKey::new(Layer::C, [0, 812, 0]).expect("a cell near the solar circle");
    let capacity = first.index_capacity();
    let along = i32::try_from(index / capacity).expect("a few thousand cells at most");
    let key = CellKey::new(Layer::C, [along, 812, 0]).expect("a cell near the first");
    let id = key
        .candidate_id(index % capacity)
        .expect("inside the index field");
    let at = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("inside the cube");
    let component = galaxy
        .fields()
        .component_id(0)
        .expect("the fixture has components");
    SystemRecord::from_parts(
        id,
        at,
        SystemOrigin::Grid(component),
        galaxy.fields().component(component).population(),
        SolarMasses::new(mass),
        age,
    )
}

/// One orbit host of a sampled system: its zone, disc and planets.
struct Host {
    zone: OrbitZone,
    disc: Disc,
    placement: HostPlacement,
}

/// A sampled system: its ID, age, hierarchy, composition and hosts.
struct System {
    id: SystemId,
    age: Years,
    hierarchy: SystemHierarchy,
    composition: Composition,
    hosts: Vec<Host>,
}

/// The pair node whose members are exactly `members`, a bit per body index.
fn pair_with(hierarchy: &SystemHierarchy, members: u32) -> Option<NodeIndex> {
    let mut under = vec![0_u32; hierarchy.nodes().len()];
    for (i, node) in hierarchy.nodes().iter().enumerate().rev() {
        under[i] = match *node {
            HierarchyNode::Star(star) => 1 << star.get(),
            HierarchyNode::Pair { inner, outer, .. } => {
                under[usize::from(inner.get())] | under[usize::from(outer.get())]
            }
        };
    }
    hierarchy
        .pairs()
        .find_map(|(index, _)| (under[usize::from(index.get())] == members).then_some(index))
}

/// The plane a zone's planets orbit in: a close pair's own for its circumbinary zone, and
/// otherwise one drawn.
fn plane_of(hierarchy: &SystemHierarchy, zone: &OrbitZone) -> HostPlane {
    match zone.host() {
        OrbitHost::Star(_) | OrbitHost::Body(_) => HostPlane::Isotropic,
        OrbitHost::Pair(_) | OrbitHost::Barycentre => {
            let members = zone.members().fold(0_u32, |bits, m| bits | 1 << m);
            let close = Metres::from(AstronomicalUnits::new(CLOSE_BINARY_CUTOFF_AU));
            pair_with(hierarchy, members)
                .and_then(|pair| match hierarchy.node(pair) {
                    HierarchyNode::Pair { orbit, .. } if orbit.semi_major_axis() < close => Some(
                        HostPlane::Aligned(SystemPlane::of_orbit(orbit.orientation())),
                    ),
                    HierarchyNode::Pair { .. } | HierarchyNode::Star(_) => None,
                })
                .unwrap_or(HostPlane::Isotropic)
        }
    }
}

/// The system of `record`, of iron abundance `fe_h`, with its hierarchy drawn under `context`:
/// zones, then per zone its disc, class and planets (`planetary_support::generate`).
fn generate(
    galaxy: &Galaxy,
    record: &SystemRecord,
    fe_h: Dex,
    context: MultiplicityContext,
) -> System {
    let seed = galaxy.seed();
    let id = record.id();
    let hierarchy = draw_hierarchy(galaxy, record, context, RedrawAttempt::FIRST);
    let composition = Composition::from_fe_h(fe_h, HeliumExcess::ZERO);
    let coeffs = ZCoeffs::new(composition.z_fit());
    let stars: Vec<ZoneStar> = hierarchy
        .stars()
        .iter()
        .map(|slot| ZoneStar {
            zams_luminosity: zams::luminosity(slot.initial_mass(), &coeffs),
            zams_radius: zams::radius(slot.initial_mass(), &coeffs),
            disc_lifetime_rank: StarDraws::for_star(seed, slot.body()).disc_lifetime(),
        })
        .collect();
    let zones = stable_zones(&ZoneHierarchy::from(&hierarchy));
    let mut slot = 1;
    let hosts = zones
        .into_iter()
        .map(|zone| {
            let inputs = ZoneDiscInputs::for_zone(seed, id, &zone, &stars, composition.fe_h())
                .expect("every component has a zero-age state");
            let disc = inputs.derive();
            let weights = class_weights(zone.host_mass(), composition.fe_h());
            let constraints = zone.class_constraints(&disc);
            let class = draw_class(seed, id, zone.host_number(), &weights, &constraints);
            let host = PlacementHost::new(
                zone.host_number(),
                *inputs.host(),
                plane_of(&hierarchy, &zone),
            );
            let placement = place(seed, id, &host, zone.truncation(), &disc, class, slot);
            slot = placement.next_slot();
            Host {
                zone,
                disc,
                placement,
            }
        })
        .collect();
    System {
        id,
        age: record.age_at_epoch(),
        hierarchy,
        composition,
        hosts,
    }
}

/// `n` systems of the galaxy's mass function over 0.08–3 M☉, their \[Fe/H\] drawn, at `age`
/// (`planetary_properties::systems`).
fn systems(galaxy: &Galaxy, first: u32, n: u32, age: Years) -> Vec<System> {
    let mut lcg = Lcg::new(0x0014_0016 ^ u64::from(first));
    (first..first + n)
        .map(|i| {
            let mass = galaxy
                .mass_function()
                .quantile_in(0.08, 3.0, lcg.next_f64());
            let r = record(galaxy, i, mass, age);
            let fe_h = draw_metallicity(galaxy, &r).fe_h();
            generate(galaxy, &r, fe_h, MultiplicityContext::Free)
        })
        .collect()
}

/// Every star of `system` as plan 06's model at its age, and its X-ray and ultraviolet history
/// from its zero-age luminosity (`planetary_properties::stars`).
fn stars(system: &System, seed: Seed) -> Vec<(StarModel, XuvHistory)> {
    let coeffs = ZCoeffs::new(system.composition.z_fit());
    system
        .hierarchy
        .stars()
        .iter()
        .map(|slot| {
            let mass = slot.initial_mass();
            let draws = StarDraws::for_star(seed, slot.body());
            let model = StarModel::new(mass, system.composition, draws, system.age)
                .expect("a sampled star's mass and age are in range");
            let zams_luminosity = if mass >= MIN_INITIAL_MASS {
                zams::luminosity(mass, &coeffs)
            } else {
                substellar::cooling(mass, Years::new(5e9), &system.composition)
                    .expect("a star below 0.1 M_sun lies inside the cooling fits")
                    .luminosity()
            };
            (model, XuvHistory::new(mass, zams_luminosity))
        })
        .collect()
}

/// The half-width of the clock window, H, in whole years: 1,000.
fn window() -> i64 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "H is a whole thousand years, exact in both types"
    )]
    let years = CLOCK_WINDOW_H.as_julian_years_f64() as i64;
    years
}

/// A rank drawn from `lcg`.
fn rank(lcg: &mut Lcg) -> UnitUniform {
    UnitUniform::new(lcg.next_f64().clamp(1e-12, 1.0 - 1e-12)).expect("inside (0, 1)")
}

/// A body's radius rank and volatile ranks, drawn as the guard draws them.
fn ranks(lcg: &mut Lcg) -> (UnitUniform, VolatileDraws) {
    let radius = rank(lcg);
    let volatiles = VolatileDraws {
        water: rank(lcg),
        carbon: rank(lcg),
        nitrogen: rank(lcg),
        ..VolatileDraws::MEDIAN
    };
    (radius, volatiles)
}

/// One of the guard's samples: a planet of a host of a system, its drawn ranks, and its time.
struct Sample<'a> {
    system: &'a System,
    host: &'a Host,
    planet: &'a PlacedPlanet,
    drawn: (UnitUniform, VolatileDraws),
    stars: &'a [(StarModel, XuvHistory)],
    /// The time, in whole years from the epoch.
    years: i64,
}

impl Sample<'_> {
    /// The body's ID, its system's and its index.
    fn id(&self) -> String {
        format!("{}.{:04x}", self.system.id, self.planet.index().get())
    }

    /// The body derived at its time about its zone's stars, as the guard derives it, under the
    /// rules in force on the calling thread; and the hottest of those stars' effective
    /// temperatures. `None` about a black hole, before the body forms, or where its derivation
    /// fails, as `planetary_properties::derive_at` counts only the samples it derives.
    fn derive(&self) -> Option<(DerivedBody, Kelvin)> {
        let t = UniverseTime::from_julian_years(self.years).expect("inside the clock window");
        let states: Vec<Option<StarState>> =
            self.stars.iter().map(|(s, _)| s.state_at(t)).collect();
        let mut lights = Vec::new();
        let mut hottest = Kelvin::ZERO;
        for m in self.host.zone.members() {
            let state = states[usize::from(m)]?;
            if state.phase() == Phase::BlackHole {
                return None;
            }
            let (model, xuv) = &self.stars[usize::from(m)];
            lights.push(
                HostLight::new(
                    state.luminosity(),
                    state.effective_temperature(),
                    state.radius(),
                )
                .expect("a star's state is finite and not negative")
                .with_history(*xuv, model.max_luminosity_until(t)),
            );
            if state.effective_temperature() > hottest {
                hottest = state.effective_temperature();
            }
        }
        let hosts = BodyHosts::new(
            Kilograms::from(self.host.zone.host_mass()),
            self.system.composition,
            &lights,
            &[],
        )
        .expect("a zone's mass is positive and it orbits a star");
        let (rank, volatiles) = self.drawn;
        let p = self.planet;
        let placed = PlacedBody::new(p.mass(), *p.orbit(), p.formation_distance(), rank)
            .expect("a placed planet's mass and formation distance are positive")
            .with_volatiles(volatiles);
        let disc = self.host.disc.profile()?;
        // The guard counts only the samples it derives (`planetary_properties::derive_at`), and so
        // does its twin; the census below lists a sample that derives under one rule alone.
        let body = derive_body(&placed, &hosts, disc, self.system.age, t).ok()?;
        Some((body, hottest))
    }
}

/// Every sample of the guard, in its order, given to `visit`, the systems generated under the
/// rules in force on the calling thread.
fn guard_walk(visit: &mut dyn FnMut(&Sample<'_>)) {
    let galaxy = galaxy();
    let mut lcg = Lcg::new(0x0016_b0d1);
    for (k, age) in [1e8, 1e9, 5e9, 1.2e10].into_iter().enumerate() {
        let first = 100_000 * u32::try_from(k).expect("four ages");
        for system in systems(&galaxy, first, 600, Years::new(age)) {
            let stars = stars(&system, galaxy.seed());
            for host in &system.hosts {
                for planet in host.placement.planets() {
                    let drawn = ranks(&mut lcg);
                    for years in [-window(), 0, window()] {
                        visit(&Sample {
                            system: &system,
                            host,
                            planet,
                            drawn,
                            stars: &stars,
                            years,
                        });
                    }
                }
            }
        }
    }
}

/// A derived body's thick-air figures under the constants in force: its grey surface from its
/// published τ, and the closure's (`τ_a`, `T_a`, `T_ad`) over its published air, each at its
/// equilibrium temperature, which is its base unless its night floor lies above it.
fn thick_air_figures(body: &DerivedBody) -> (f64, Option<(f64, Kelvin, Kelvin)>) {
    let air = body.atmosphere();
    let equilibrium = body.equilibrium_temperature();
    (
        grey_surface_temperature(equilibrium, air.optical_depth()).value(),
        GreenhouseConstants::in_force().thick_air(
            air.partial_pressures(),
            equilibrium,
            body.surface_gravity(),
        ),
    )
}

/// The body's runaway and maximum-greenhouse ratios, its flux over each limit's `S_eff` at its
/// hottest host's effective temperature (Kopparapu et al. 2013's fit, held to its range, as the
/// generator reads it): the sum's own for a single host, and an approximation about a pair.
fn limit_ratios(body: &DerivedBody, hottest: Kelvin) -> (f64, f64) {
    let ratio = |limit: HabitableLimit| body.flux().value() / limit.flux(hottest).value();
    (
        ratio(HabitableLimit::RunawayGreenhouse),
        ratio(HabitableLimit::MaximumGreenhouse),
    )
}

/// A census line of the body `body` about a host at `hottest` ([`thick_air_line`]), its limit
/// ratios its flux's over its hottest host's ([`limit_ratios`]).
fn body_line(body: &DerivedBody, hottest: Kelvin) -> String {
    thick_air_line(body, hottest, limit_ratios(body, hottest))
}

/// A census line of the body `body` about a hottest host at `hottest` with the runaway and
/// maximum-greenhouse ratios `(runaway, maximum)`: its class, state, published state, deck, carbon
/// carrier, `T_s`, `T_s` ÷ `T★`, `p_s`, x(CO₂), τ, the closure's `τ_a`, `T_a` and `T_ad`
/// ([`thick_air_figures`]), its grey, water's saturation temperature at its `p_s` and its side,
/// whether a supercritical reservoir hides its surface (P14.T53.b part 2's section), both ratios,
/// its passes' count and their convergence; and its airborne inventory's oxygen and total, as
/// column weights, with the closure over that inventory: the column the greenhouse's steps read on
/// a molten body, whose melt takes the oxygen only as the published air is finished
/// (`escaping::finish`). For P14.T53.a's thick-air closure's censuses (this module's and the
/// systems').
pub(crate) fn thick_air_line(
    body: &DerivedBody,
    hottest: Kelvin,
    (runaway, maximum): (f64, f64),
) -> String {
    let air = body.atmosphere();
    let p_s = air.surface_pressure().map_or(0.0, Pascals::value);
    let (grey, closure) = thick_air_figures(body);
    let moles = |gas: Gas| air.partial_pressures().of(gas).value() / gas.molar_mass_g_per_mol();
    let all = Gas::ALL.into_iter().fold(0.0, |sum, gas| sum + moles(gas));
    let saturation =
        water_saturation_temperature(Pascals::new(p_s)).map_or(f64::NAN, Kelvin::value);
    format!(
        "{:?} {:?} (published {:?}) {:?} {:?}, T_s {:.1} K ({:.4} of T★ {:.0} K), p_s {:.4e} bar, \
         x(CO₂) {:.5}, τ {:.4e}, closure {}, grey {:.1} K, T_sat {:.2} K ({}), section {}, \
         runaway {:.3}, max {:.3}, {} passes {:?}; its airborne inventory's O₂ {:.4e} bar and p_s \
         {:.4e} bar, the closure over it {}",
        body.class(),
        air.state(),
        air.published_state(),
        air.runaway_deck(),
        air.carbon_carrier(),
        air.surface_temperature().value(),
        air.surface_temperature().value() / hottest.value(),
        hottest.value(),
        p_s / PASCALS_PER_BAR,
        moles(Gas::CarbonDioxide) / all,
        air.optical_depth(),
        closure.map_or_else(
            || "none".to_owned(),
            |(tau_a, top, adiabat)| format!(
                "τ_a {tau_a:.2} T_a {:.1} K T_ad {:.1} K",
                top.value(),
                adiabat.value()
            )
        ),
        grey,
        saturation,
        if air.surface_temperature().value() >= saturation {
            "above"
        } else {
            "below"
        },
        air.supercritical_reservoir(body.surface_gravity())
            .is_some(),
        runaway,
        maximum,
        body.atmosphere_passes(),
        body.convergence(),
        air.inventory_weights().of(Gas::Oxygen).value() / PASCALS_PER_BAR,
        air.inventory_weights().total().value() / PASCALS_PER_BAR,
        GreenhouseConstants::in_force()
            .thick_air(
                air.inventory_weights(),
                body.equilibrium_temperature(),
                body.surface_gravity()
            )
            .map_or_else(
                || "none".to_owned(),
                |(_, top, adiabat)| format!(
                    "T_a {:.1} K T_ad {:.1} K",
                    top.value(),
                    adiabat.value()
                )
            ),
    )
}

/// The guard (ruling 133.5) under the rules in force on the calling thread: every sample's
/// equilibrium temperature under its hottest host's, its surface at most at it; the share
/// saturated at it, printed as its FINDING and held to 10⁻³; the largest `T_s` ÷ `T★` of a body
/// with air; and every saturated sample by name. It returns the saturated count, the largest
/// ratio and its body, and its sample count.
fn guard(step: &str) -> (u32, f64, String, u32) {
    let (mut bodies, mut giants, mut airless, mut aired) = (0_u32, 0_u32, 0_u32, 0_u32);
    let (mut samples, mut saturated) = (0_u32, 0_u32);
    let (mut largest, mut largest_at) = (0.0_f64, String::new());
    guard_walk(&mut |sample| {
        if sample.years == window() {
            bodies += 1;
            giants += u32::from(
                sample.planet.mass() >= EarthMasses::new(0.3 * EARTH_MASSES_PER_JUPITER_MASS),
            );
        }
        let Some((body, hottest)) = sample.derive() else {
            return;
        };
        let equilibrium = body
            .effective_temperature()
            .unwrap_or_else(|| body.equilibrium_temperature());
        let surface = body.atmosphere().surface_temperature();
        assert!(
            equilibrium < hottest,
            "{}: {equilibrium:?} against {hottest:?}",
            sample.id()
        );
        assert!(
            surface <= hottest,
            "{}: {surface:?} against {hottest:?}",
            sample.id()
        );
        samples += 1;
        if surface >= hottest {
            saturated += 1;
            eprintln!(
                "SATURATED {step} {} at {} yr: {}",
                sample.id(),
                sample.years,
                body_line(&body, hottest)
            );
        }
        if body.atmosphere().state() == SurfaceState::Airless {
            airless += 1;
        } else {
            aired += 1;
            let ratio = surface.value() / hottest.value();
            if ratio > largest {
                largest = ratio;
                largest_at = format!("{} at {} yr", sample.id(), sample.years);
            }
        }
    });
    assert!(
        bodies > 2_000 && giants > 100 && airless > 100 && aired > 100,
        "{bodies} bodies, {giants} giants, {airless} airless, {aired} with air"
    );
    let share = f64::from(saturated) / f64::from(samples);
    eprintln!(
        "FINDING {step}: surfaces saturated at their host's temperature: {saturated} of {samples} \
         ({share:.2e}); the largest T_s ÷ T★ of an air {largest:.4}, {largest_at}"
    );
    assert!(share <= 1e-3, "{saturated} of {samples} surfaces saturated");
    (saturated, largest, largest_at, samples)
}

/// The guard's twin under P14.T53.a's thick-air closure (decision-p14-t53a-thick-co2 §7.1), run by
/// name: under the escape's batch alone (B3's step 02, the Rayleigh floor switched with it) and
/// with every 24 hold in force (step F), no surface saturates at its host's effective temperature,
/// where 117 of the 30,063 samples did before it; the largest `T_s` ÷ `T★` of an air is printed
/// (the replica's 0.69).
#[test]
#[ignore = "slow: the guard's 30,063 samples twice, under the escape's batch and every 24 hold"]
fn under_the_escape_batch_and_every_24_hold_no_planet_is_hotter_than_its_star() {
    use crate::planetary::chem::escape::{EscapeRules, with_rules};
    let (step_02, ..) = with_rules(EscapeRules::Batch, || guard("02"));
    let (step_f, largest, at, samples) = with_every_24_hold(|| guard("F"));
    eprintln!(
        "the guard: {step_02} saturated at step 02, {step_f} at F, of {samples}; the largest \
         ratio {largest:.4} ({at})"
    );
    assert_eq!((step_02, step_f), (0, 0));
}

/// The 39 bodies the escape's batch saturated ([`SATURATED_UNDER_THE_BATCH`]), each by name at
/// all three times with every 24 hold in force (decision-p14-t53a-thick-co2 §7.2): finite, under
/// its star, its surface at least its top's grey temperature and at most its grey and its star.
/// Each is stated (`T_s`, `p_s`, τ, `τ_a`, `T_a`, its crust's state, its deck, part 2's section and
/// both limit ratios), with the count that stay molten against the replica's 13 ± 5.
#[test]
fn under_every_24_hold_the_guard_s_39_thick_carbon_dioxide_airs_are_finite_and_under_their_stars() {
    let named: std::collections::BTreeSet<String> = SATURATED_UNDER_THE_BATCH
        .iter()
        .map(|&(system, index)| format!("{system}.{index:04x}"))
        .collect();
    let mut seen = std::collections::BTreeMap::<String, u32>::new();
    let (mut molten, mut solid, mut sectioned) = (0_u32, 0_u32, 0_u32);
    let mut largest = 0.0_f64;
    with_every_24_hold(|| {
        guard_walk(&mut |sample| {
            let id = sample.id();
            if !named.contains(&id) {
                return;
            }
            let (body, hottest) = sample.derive().expect("each of the 39 is derived");
            let air = body.atmosphere();
            let t_s = air.surface_temperature().value();
            let (grey, closure) = thick_air_figures(&body);
            eprintln!(
                "{id} at {:+} yr, {:.1e} yr old: {}",
                sample.years,
                sample.system.age.value(),
                body_line(&body, hottest)
            );
            let (_, top, _) = closure.expect("each lies beyond the anchor");
            assert!(t_s.is_finite() && t_s < hottest.value(), "{id}: {t_s}");
            assert!(
                top.value() <= t_s * (1.0 + 1e-9),
                "{id}: {t_s} under its top's {top:?}"
            );
            assert!(
                t_s <= grey * (1.0 + 1e-9),
                "{id}: {t_s} above its grey {grey}"
            );
            assert!(air.surface_pressure().expect("an air") > THICK_AIR_ANCHOR_PRESSURE);
            largest = largest.max(t_s / hottest.value());
            if sample.years == 0 {
                if air.state() == SurfaceState::MagmaOcean {
                    assert!(t_s >= SILICATE_SOLIDUS.value(), "{id}");
                    molten += 1;
                } else {
                    solid += 1;
                }
                sectioned += u32::from(
                    air.supercritical_reservoir(body.surface_gravity())
                        .is_some(),
                );
            }
            *seen.entry(id).or_default() += 1;
        });
    });
    eprintln!(
        "the 39 at the epoch: {molten} MagmaOcean (the replica's 13 ± 5), {solid} solid, \
         {sectioned} under a supercritical reservoir (part 2's section); the largest T_s ÷ T★ \
         {largest:.4}"
    );
    assert_eq!(seen.len(), SATURATED_UNDER_THE_BATCH.len(), "{seen:?}");
    assert!(seen.values().all(|&n| n == 3), "{seen:?}");
}

/// The census of the guard's samples that P14.T53.a's thick-air closure moves, with every 24
/// hold in force (decision-p14-t53a-thick-co2 §7.5, the guard's 30,063 samples): each sample
/// derived with the closure and with it lifted ([`without_thick_air`]), every one that differs
/// listed with both lines, counted by flip (state, into or out of part 2's section and arm (c),
/// decked to deckless, a new jump or bracketed residual above 10⁻⁶), by dominant gas; any that
/// derives under one rule alone; count (xvii) over every sample ([`MeltOxygen`], §11.2); and the
/// knife edges,
/// within 20 K of the silicate solidus, of water's critical temperature and of the acid's 430 K.
/// Every new jump is one of the closure's recorded nine ([`CLOSURE_JUMPS`]), in part 2's section
/// and in its state as before; any other goes to main.
#[test]
#[ignore = "a measurement for P14.T53.a's thick-air closure's record, run by name"]
#[expect(
    clippy::too_many_lines,
    reason = "one walk of the guard's samples, each mover listed and tallied"
)]
fn under_every_24_hold_the_thick_air_closure_s_movers_among_the_guard_s_samples_are_stated() {
    use std::collections::BTreeMap;
    let mut flips = BTreeMap::<String, u32>::new();
    let mut by_gas = BTreeMap::<String, u32>::new();
    let mut edges = Vec::new();
    let (mut samples, mut movers) = (0_u32, 0_u32);
    let (mut melt_oxygen, mut to_main) = (MeltOxygen::default(), Vec::new());
    with_every_24_hold(|| {
        guard_walk(&mut |sample| {
            let (after, before) = (sample.derive(), without_thick_air(|| sample.derive()));
            let ((after, hottest), (before, _)) = match (after, before) {
                (Some(after), Some(before)) => (after, before),
                (None, None) => return,
                (after, before) => {
                    eprintln!(
                        "MOVER {} at {:+} yr derives under one rule alone: with the closure {}, \
                         without it {}",
                        sample.id(),
                        sample.years,
                        after.is_some(),
                        before.is_some()
                    );
                    movers += 1;
                    *flips
                        .entry("derived under one rule alone".to_owned())
                        .or_default() += 1;
                    return;
                }
            };
            samples += 1;
            melt_oxygen.add(
                &format!("{} at {:+} yr", sample.id(), sample.years),
                &after,
                hottest,
            );
            if after == before {
                return;
            }
            movers += 1;
            let (a, b) = (after.atmosphere(), before.atmosphere());
            let id = format!(
                "{} at {:+} yr ({:.1e} yr old)",
                sample.id(),
                sample.years,
                sample.system.age.value()
            );
            eprintln!(
                "MOVER {id}\n  before {}\n  after  {}",
                body_line(&before, hottest),
                body_line(&after, hottest)
            );
            let dominant = Gas::ALL
                .into_iter()
                .max_by(|x, y| {
                    let moles = |gas: Gas| {
                        a.partial_pressures().of(gas).value() / gas.molar_mass_g_per_mol()
                    };
                    moles(*x).total_cmp(&moles(*y))
                })
                .expect("nine gases");
            *by_gas.entry(format!("{dominant:?}")).or_default() += 1;
            let section = |body: &DerivedBody| {
                body.atmosphere()
                    .supercritical_reservoir(body.surface_gravity())
                    .is_some()
            };
            // Arm (c)'s: a published runaway between the limits now.
            let hothouse = |body: &DerivedBody| {
                let (runaway, maximum) = limit_ratios(body, hottest);
                body.atmosphere().state() == SurfaceState::RunawayGreenhouse
                    && runaway < 1.0
                    && maximum >= 1.0
            };
            let residual = |body: &DerivedBody| match body.convergence() {
                Convergence::Converged => 0.0,
                Convergence::Bracketed { residual, .. } => residual,
            };
            let new_jump = (after.convergence().is_jump() && !before.convergence().is_jump())
                || (residual(&after) > 1e-6 && residual(&before) <= 1e-6);
            if new_jump && !closure_jump_exempt(&sample.id(), &before, &after) {
                to_main.push(format!("{id}: {:?}", after.convergence()));
            }
            for (what, was, now) in [
                (
                    "state",
                    format!("{:?}", b.state()),
                    format!("{:?}", a.state()),
                ),
                (
                    "deck",
                    format!("{:?}", b.runaway_deck()),
                    format!("{:?}", a.runaway_deck()),
                ),
                (
                    "section",
                    format!("{}", section(&before)),
                    format!("{}", section(&after)),
                ),
                (
                    "arm (c)",
                    format!("{}", hothouse(&before)),
                    format!("{}", hothouse(&after)),
                ),
                (
                    "jump",
                    format!("{}", before.convergence().is_jump()),
                    format!("{}", after.convergence().is_jump()),
                ),
                (
                    "bracketed above 1e-6",
                    format!("{}", residual(&before) > 1e-6),
                    format!("{}", residual(&after) > 1e-6),
                ),
            ] {
                if was != now {
                    *flips.entry(format!("{what} {was} → {now}")).or_default() += 1;
                }
            }
            let t = a.surface_temperature().value();
            for (edge, at) in [
                ("solidus", SILICATE_SOLIDUS.value()),
                ("water's T_c", 647.096),
                ("the acid's 430 K", 430.0),
            ] {
                if (t - at).abs() <= 20.0 {
                    edges.push(format!("{id}: {t:.2} K within 20 K of {edge}"));
                }
            }
        });
    });
    eprintln!("{movers} of {samples} samples move under the closure");
    eprintln!("FLIPS: {flips:?}");
    eprintln!("BY DOMINANT GAS: {by_gas:?}");
    eprintln!("KNIFE EDGES:");
    for edge in &edges {
        eprintln!("  {edge}");
    }
    eprintln!(
        "COUNT (xvii) over the guard's samples: {}",
        melt_oxygen.line()
    );
    assert!(movers > 0);
    assert_eq!(
        to_main,
        Vec::<String>::new(),
        "TO MAIN: a new jump outside the closure's named nine, out of part 2's section or whose \
         state moved (decision-p14-t53a-thick-co2 §11.1)"
    );
}
