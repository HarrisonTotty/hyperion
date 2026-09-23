//! Six systems' hierarchies and star positions, pinned (plan 11, P11.T2 and T3.b).
//!
//! `stellar/hierarchies` holds, for six grid IDs of the Milky Way fixture, the hierarchy
//! `draw_hierarchy` gives in three ways, `Free` at attempts 0 and 1 and `ForcedMultiple` with no
//! separation limit at attempt 0, and the stars' positions about the barycentre at −H, the epoch
//! and +H for the first and the last. Every mass, period, element and position is pinned by its
//! bits, so the golden covers every draw the hierarchy makes: the multiplicity marks, the node and
//! period mark, the mass ratio, eccentricity, orientation and phase of every try, and the
//! stability test that decides which try is kept.
//!
//! # Why these IDs
//!
//! Each is the first system of one cell, so the IDs depend on placement alone and not on the draw
//! they pin: the cells of layers C, D and E at the Sun-like point, where the plan's figures are
//! quoted, and the layer-A and layer-E cells of the outer bulge and a layer-E cell on a young arm
//! ridge, as plan 03's `placement/ids` takes them. They span the mass range from M dwarfs to
//! O stars and the tide from the solar circle's to the bulge's. `ForcedMultiple` makes each one a
//! multiple whatever its own draw says, so that every ID pins at least one orbit.
//!
//! Nothing generated reads the hierarchy yet (P11.T2.c wires it in), so this golden is new at
//! generator version 11 and moves nothing else.

#[expect(
    dead_code,
    reason = "the pinned cells use the Sun-like point of tests/common alone"
)]
mod common;

use common::sunlike_point;
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{CellKey, SystemRecord, generate_cell};
use hyperion_sim::id::Layer;
use hyperion_sim::math;
use hyperion_sim::stellar::multiplicity::{
    HierarchyNode, MultiplicityContext, RedrawAttempt, SystemHierarchy, draw_hierarchy,
    star_positions_at,
};
use hyperion_sim::time::{ClockWindow, UniverseTime};
use hyperion_sim::units::Days;
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

/// The seed of the Milky Way fixture, plan 03's pinned fixture galaxy.
const FIXTURE_SEED: u64 = 0x0308_d000_0000_0000;

/// Where the outer-bulge cells sit: in the plane, at 45° to both axes (plan 03's golden).
const BULGE_LY: f64 = 3_200.0;

/// A point in the plane at 45° to both axes, `r` light-years from the centre.
fn diagonal(r: f64) -> GalacticPosition {
    let half = 0.5_f64.sqrt();
    GalacticPosition::from_light_years([r * half, r * half, 0.0])
        .expect("a point a few thousand light-years out is inside the cube")
}

/// A point on the young disc's first arm ridge, just outside the bar's end.
fn arm_ridge(galaxy: &Galaxy) -> GalacticPosition {
    let arms = galaxy.fields().arms();
    let r = 1.15 * arms.bar_half_length().value();
    let (sin, cos) = math::sin_cos(arms.ridge_azimuth(r, 0));
    GalacticPosition::from_light_years([r * cos, r * sin, 0.0])
        .expect("a point on a ridge is inside the cube")
}

/// The six pinned systems, each the first of its cell, with its label.
fn pinned(galaxy: &Galaxy) -> Vec<(&'static str, SystemRecord)> {
    let sun = sunlike_point(galaxy);
    let bulge = diagonal(BULGE_LY);
    let arm = arm_ridge(galaxy);
    let cells = [
        ("sun.C", Layer::C, &sun),
        ("sun.D", Layer::D, &sun),
        ("sun.E", Layer::E, &sun),
        ("bulge.A", Layer::A, &bulge),
        ("bulge.E", Layer::E, &bulge),
        ("arm.E", Layer::E, &arm),
    ];
    let mut systems = Vec::new();
    let mut cell = Vec::new();
    for (label, layer, at) in cells {
        let key = CellKey::containing(layer, at).expect("a stellar layer inside the cube");
        generate_cell(galaxy, key, &mut cell);
        let record = *cell
            .first()
            .unwrap_or_else(|| panic!("the pinned cell {label} holds no system"));
        systems.push((label, record));
    }
    systems
}

/// Writes one hierarchy: its stars, then its nodes depth first with every pair's elements.
fn write_hierarchy(w: &mut GoldenWriter, label: &str, h: &SystemHierarchy) {
    w.line(&format!(
        "{label}.stars = {}, dropped {}",
        h.star_count(),
        h.dropped_companions()
    ));
    for star in h.stars() {
        let index = star.body().body_index();
        w.f64(
            &format!("{label}.star{index}.mass_msun"),
            star.initial_mass().value(),
        );
    }
    for (node, i) in h.nodes().iter().zip(0_u32..) {
        let at = format!("{label}.node{i}");
        match node {
            HierarchyNode::Star(star) => w.line(&format!("{at} = star {}", star.get())),
            HierarchyNode::Pair {
                inner,
                outer,
                orbit,
            } => {
                w.line(&format!(
                    "{at} = pair of node {} and node {}",
                    inner.get(),
                    outer.get()
                ));
                w.f64(
                    &format!("{at}.period_d"),
                    Days::from(orbit.period()).value(),
                );
                w.f64(
                    &format!("{at}.semi_major_axis_m"),
                    orbit.semi_major_axis().value(),
                );
                w.f64(&format!("{at}.eccentricity"), orbit.eccentricity().value());
                w.f64(
                    &format!("{at}.inclination_rad"),
                    orbit.inclination().value(),
                );
                w.f64(
                    &format!("{at}.ascending_node_rad"),
                    orbit.ascending_node().value(),
                );
                w.f64(
                    &format!("{at}.argument_of_periapsis_rad"),
                    orbit.argument_of_periapsis().value(),
                );
                w.f64(
                    &format!("{at}.mean_anomaly_at_epoch_rad"),
                    orbit.mean_anomaly_at_epoch().value(),
                );
                w.f64(
                    &format!("{at}.mu_m3_s2"),
                    orbit.gravitational_parameter().value(),
                );
            }
        }
    }
}

/// Writes every star's position at −H, the epoch and +H.
fn write_positions(w: &mut GoldenWriter, label: &str, h: &SystemHierarchy) {
    let mut positions = Vec::new();
    for (when, t) in [
        ("minus_h", ClockWindow::START),
        ("epoch", UniverseTime::EPOCH),
        ("plus_h", ClockWindow::END),
    ] {
        star_positions_at(h, t, &mut positions);
        for (body, at) in &positions {
            for (axis, x) in ["x", "y", "z"].into_iter().zip(at.metres()) {
                w.f64(
                    &format!("{label}.star{}.at_{when}.{axis}_m", body.body_index()),
                    x,
                );
            }
        }
    }
}

/// Six systems' hierarchies and positions (P11.T2's golden hierarchies for six pinned IDs, with
/// P11.T3.b's positions).
#[test]
fn hierarchies_of_six_pinned_systems_are_pinned() {
    let galaxy = Galaxy::from_params(Seed::new(FIXTURE_SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral");
    let systems = pinned(&galaxy);
    assert_eq!(systems.len(), 6, "the task asks for six pinned IDs");
    let forced = MultiplicityContext::ForcedMultiple {
        max_separation: None,
    };
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for (label, record) in &systems {
        w.u64_hex(&format!("{label}.id"), record.id().raw());
        w.f64(
            &format!("{label}.primary_mass_msun"),
            record.primary_initial_mass().value(),
        );
        let free = draw_hierarchy(
            &galaxy,
            record,
            MultiplicityContext::Free,
            RedrawAttempt::FIRST,
        );
        write_hierarchy(&mut w, &format!("{label}.free.0"), &free);
        write_positions(&mut w, &format!("{label}.free.0"), &free);
        let second = RedrawAttempt::FIRST.next().expect("eight attempts");
        let redrawn = draw_hierarchy(&galaxy, record, MultiplicityContext::Free, second);
        write_hierarchy(&mut w, &format!("{label}.free.1"), &redrawn);
        let multiple = draw_hierarchy(&galaxy, record, forced, RedrawAttempt::FIRST);
        assert!(
            multiple.star_count() > 1,
            "{label}: a forced multiple came out single"
        );
        write_hierarchy(&mut w, &format!("{label}.forced.0"), &multiple);
        write_positions(&mut w, &format!("{label}.forced.0"), &multiple);
    }
    golden!("stellar/hierarchies", w.as_str());
}
