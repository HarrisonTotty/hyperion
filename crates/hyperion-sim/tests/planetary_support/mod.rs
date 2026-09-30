//! Sampled planetary systems for plan 14's placement and property tests (P14.T10, T16.b).
//!
//! `SystemContext` (P14.T1.d) and the assembled generator (P14.T30.a) are not built, so each
//! system is built here as T30.a will build it, from plain arguments: a system record at the
//! Sun-like point with a chosen primary mass and \[Fe/H\], its hierarchy drawn by plan 11, its
//! stable zones (P14.T9, through `ZoneHierarchy::from(&SystemHierarchy)`), each zone's disc from
//! its components' zero-age states (plan 06's `zams` values, HPT's) and their own disc-lifetime
//! ranks, the zone's class drawn under its constraints (P14.T4.c), and its planets placed
//! (P14.T8) host by host in hierarchy order, each host's slots following the last's. A
//! circumbinary zone of a pair closer than 47 au takes the pair's plane (P14.T8.d).

use hyperion_sim::Seed;
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{CellKey, SystemOrigin, SystemRecord};
use hyperion_sim::id::{Layer, SystemId};
use hyperion_sim::planetary::architecture::{CLOSE_BINARY_CUTOFF_AU, class_weights, draw_class};
use hyperion_sim::planetary::disc::Disc;
use hyperion_sim::planetary::placement::classes::orbits::{HostPlane, SystemPlane};
use hyperion_sim::planetary::placement::{
    HostPlacement, OrbitHost, OrbitZone, PlacementHost, ZoneDiscInputs, ZoneHierarchy, ZoneStar,
    place, stable_zones,
};
use hyperion_sim::stellar::Composition;
use hyperion_sim::stellar::draws::StarDraws;
use hyperion_sim::stellar::multiplicity::{
    HierarchyNode, MultiplicityContext, NodeIndex, RedrawAttempt, SystemHierarchy, draw_hierarchy,
};
use hyperion_sim::stellar::sse::{ZCoeffs, zams};
use hyperion_sim::units::{AstronomicalUnits, Dex, HeliumExcess, Metres, SolarMasses, Years};

/// The Milky Way fixture every sample is drawn in.
pub fn galaxy(seed: Seed) -> Galaxy {
    Galaxy::from_params(seed, GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral")
}

/// The record of candidate `index` of a run of layer C cells near the Sun-like point, 26,000 ly
/// out on the +y axis in the plane, whose primary formed with `mass` M☉ `age` ago.
pub fn record(galaxy: &Galaxy, index: u32, mass: f64, age: Years) -> SystemRecord {
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
pub struct Host {
    pub zone: OrbitZone,
    pub disc: Disc,
    pub placement: HostPlacement,
}

/// A sampled system: its ID, age, hierarchy, composition and hosts.
pub struct System {
    pub id: SystemId,
    pub age: Years,
    pub hierarchy: SystemHierarchy,
    pub composition: Composition,
    pub hosts: Vec<Host>,
}

impl System {
    /// The component masses, by body index.
    pub fn star_mass(&self, index: u8) -> SolarMasses {
        self.hierarchy.stars()[usize::from(index)].initial_mass()
    }
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

/// The system of `record`, of iron abundance `fe_h`, with its hierarchy drawn under `context`,
/// generated as P14.T30.a will: zones, then per zone its disc, class and planets.
pub fn generate(
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
