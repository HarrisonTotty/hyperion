//! The branches' tests (R09.T14.c; the lane's design §4.5, decision-r09-t14-choices Q2, Q4 and
//! the "T14.d follow-up"): which branch a world takes and the share of its epoch; a surface no
//! older than its epoch's end, and a world never wet, left untouched to the bit; the young surface
//! inside the waiting craters' rims eroded by the solver but shedding no runoff of its own; the
//! paleo-water stored to its inventory; the outputs the step writes on the reference Mars; and the
//! final field's realised relief on the fast reference worlds (decision-r09-t14-choices, "T14.c
//! follow-up" item 3 (i)).

use hyperion_surface::field::{Crust, FlowDirection};
use hyperion_testkit::float::assert_same_bits;

use super::super::branches::{
    Branch, dry_now, dry_now_years, epoch_share, erode_dry_now, inside_waiting_rims, paleo_water_m3,
};
use super::super::erodibility::Fluid;
use super::super::{provisional, run};
use super::*;
use crate::planetary::surface::inputs::WetEpoch;
use crate::planetary::surface::reference::builder_of;
use crate::planetary::surface::steps::relief::budget;
use crate::units::Gigayears;

/// An epoch from `start` to `end`, Ga, of the reference Mars's flow, runoff and inventory.
fn epoch(start: f64, end: f64) -> WetEpoch {
    WetEpoch {
        start: Gigayears::new(start),
        end: Gigayears::new(end),
        ..*reference::mars_like().wet_epoch().unwrap()
    }
}

/// `working` whole, as text: two states print the same exactly where every value is the same to
/// the bit (Rust's shortest round-trip form, which tells −0 from +0).
fn bits(working: &Working) -> String {
    format!("{working:?}")
}

/// The branch follows the wet epoch (the lane's design §4.2): the reference Moon is never wet, the
/// reference Earth wet now (its epoch ends at 0) and the reference Mars dry now; and the share of
/// the epoch a surface saw is none at or below the end, (age − end) ÷ (start − end) inside it, 1
/// above its start and 1 for any surface older than a zero-length epoch (decision-r09-t14-choices
/// Q4).
#[test]
fn the_branch_follows_the_wet_epoch() {
    assert_eq!(Branch::of(&reference::moon_like()), Branch::NeverWet);
    assert_eq!(Branch::of(&reference::earth_like()), Branch::WetNow);
    assert_eq!(Branch::of(&reference::mars_like()), Branch::DryNow);
    let mars = epoch(3.9, 3.6);
    assert_eq!(epoch_share(&mars, Gigayears::new(3.6)), None);
    assert_eq!(epoch_share(&mars, Gigayears::new(3.0)), None);
    let inside = epoch_share(&mars, Gigayears::new(3.75)).unwrap();
    assert!((inside - 0.5).abs() < 1e-12, "{inside}");
    assert_same_bits(epoch_share(&mars, Gigayears::new(3.9)).unwrap(), 1.0);
    assert_same_bits(epoch_share(&mars, Gigayears::new(4.2)).unwrap(), 1.0);
    let instant = epoch(3.7, 3.7);
    assert_same_bits(epoch_share(&instant, Gigayears::new(3.8)).unwrap(), 1.0);
    assert_eq!(epoch_share(&instant, Gigayears::new(3.7)), None);
}

/// The reference Mars's wet epoch (the lane's design §4.5 test 3): the effective flow 2.2 × 10⁵
/// yr at 0.1 m a⁻¹ of water, the share 1 (surface 3.9 Ga, the epoch's start), and K t (r ÷ 1 m
/// a⁻¹)^0.45 = 1.866 m^0.1 (computed: K = 2.391 × 10⁻⁵ m^0.1 a⁻¹ from GM ÷ R² = 3.7279 m s⁻² and
/// ρ 999.79 kg m⁻³; decision-r09-t14-choices, "T14.b follow-up" (a)), within 2% of Design note 9's
/// 1.9 (1.8% below it).
#[test]
fn the_reference_mars_s_epoch_is_its_calibration() {
    let inputs = reference::mars_like();
    let epoch = inputs.wet_epoch().unwrap();
    assert_same_bits(epoch.effective_flow.value(), 2.2e5);
    assert_same_bits(epoch.runoff_m_per_year, 0.1);
    assert_eq!(Fluid::of_epoch(epoch).key.as_str(), "H2O");
    let years = dry_now_years(epoch, inputs.surface_age()).unwrap();
    assert_same_bits(years, 2.2e5);
    let k = super::super::erodibility::Erodibility::of(&inputs)
        .unwrap()
        .of_crust(Crust::Lid);
    let kt = k * years * math::powf(epoch.runoff_m_per_year, 0.45);
    eprintln!("the reference Mars: K {k:.4e} m^0.1 a⁻¹, K t r^0.45 {kt:.4} m^0.1 (1.866 computed)");
    assert!(
        (kt / 1.9 - 1.0).abs() <= 0.02,
        "K t r^0.45 = {kt} m^0.1, computed 1.866, is not within 2% of 1.9"
    );
}

/// A surface no older than its epoch's end is left untouched (decision-r09-t14-choices Q4, test
/// (1)): the reference Mars at a surface age of 3.5 Ga, below its epoch's 3.6 Ga end, staged to
/// the climate's end, is the same state to the bit after the step: its elevation unchanged, no
/// runoff, every cell terminal, the field not routed.
#[test]
fn a_surface_younger_than_its_epoch_s_end_is_left_untouched() {
    let inputs = builder_of(&reference::mars_like())
        .surface_age(Gigayears::new(3.5))
        .build()
        .unwrap();
    let staged = Staged::of(inputs);
    let pass = staged.pass();
    assert_eq!(Branch::of(pass.inputs()), Branch::DryNow);
    let mut working = staged.working.clone();
    run(&pass, &mut working);
    assert_eq!(bits(&working), bits(&staged.working));
    assert!(!working.routed);
    for cell in &working.cells {
        assert_same_bits(cell.runoff_m_per_year, 0.0);
        assert_eq!(cell.flow, FlowDirection::Terminal);
    }
}

/// A world never wet is left untouched (the lane's design §4.5 test 2; decision-r09-t14-choices,
/// "T14.d follow-up" C): the reference Moon staged to the climate's end is the same state to the
/// bit after the step, every cell terminal with no drainage, steepness or runoff, unrouted.
#[test]
fn a_world_never_wet_is_left_untouched() {
    let staged = Staged::of(reference::moon_like());
    let pass = staged.pass();
    let mut working = staged.working.clone();
    run(&pass, &mut working);
    assert_eq!(bits(&working), bits(&staged.working));
    assert!(!working.routed);
    for cell in &working.cells {
        assert_eq!(cell.flow, FlowDirection::Terminal);
        assert_same_bits(cell.drainage.value(), 0.0);
        assert_same_bits(cell.steepness_index_m0_9, 0.0);
        assert_same_bits(cell.runoff_m_per_year, 0.0);
        assert_same_bits(cell.catchment_runoff_m_per_year, 0.0);
    }
}

/// The final field's realised relief lies within R09.T12.e's 6–16 `σ_h` on the fast reference
/// worlds (decision-r09-t14-choices, "T14.c follow-up" item 3 (i); the reference Earth's is the
/// slow `earth` test's): the reference Mars, Moon and Ceres, each staged to the climate's end and
/// taken through the rest of the pass, the erosion and the late coarse craters, have a cells'
/// range of 6 to 16 times their `σ_h` ([`assert_final_relief`]).
#[test]
fn the_final_field_s_relief_is_six_to_sixteen_sigma_h() {
    for (name, inputs) in [
        ("reference Mars", reference::mars_like()),
        ("reference Moon", reference::moon_like()),
        ("reference Ceres", reference::ceres_like()),
    ] {
        let staged = Staged::of(inputs);
        assert_final_relief(name, &staged.inputs, &staged.finished());
    }
}

/// The young surface erodes and sheds no runoff (decision-r09-t14-choices Q4, tests (2c) and (3),
/// as the "T14.d follow-up" A amends them): on the reference Mars, the waiting craters' cells are
/// not bounds, so on the solver's own output some cell inside a waiting crater's rim lies below
/// the elevation that entered the step (the rim cells' mean lowering printed beside every eroded
/// cell's); and after the step no cell inside a waiting rim has runoff of its own, every other
/// cell the epoch's 0.1 m a⁻¹.
#[test]
fn the_young_surface_erodes_but_sheds_no_runoff_of_its_own() {
    let staged = Staged::of(reference::mars_like());
    let pass = staged.pass();
    let young = inside_waiting_rims(&pass, &staged.working);
    let waiting = staged
        .working
        .craters
        .iter()
        .filter(|c| craters::waits(&pass, c.age))
        .count();
    let inside = young.iter().filter(|&&y| y).count();
    eprintln!("{waiting} waiting craters, {inside} cells inside their rims");
    assert!(inside > 0);
    let outcome = erode_dry_now(&pass, &staged.working).unwrap();
    let z0: Vec<f64> = staged
        .working
        .cells
        .iter()
        .map(|c| c.elevation.value())
        .collect();
    let omega = pass.grid().solid_angles_sr();
    let mean = |keep: &dyn Fn(usize) -> bool| {
        let (mut sum, mut area) = (0.0, 0.0);
        for c in (0..z0.len()).filter(|&c| outcome.bounds[c].is_none() && keep(c)) {
            sum += omega[c] * (z0[c] - outcome.z_m[c]);
            area += omega[c];
        }
        sum / area
    };
    let lowered = (0..z0.len())
        .filter(|&c| young[c] && outcome.z_m[c] < z0[c])
        .count();
    eprintln!(
        "{lowered} rim cells lowered; mean lowering {:.2} m inside the waiting rims, {:.2} m over \
         every eroded cell",
        mean(&|c| young[c]),
        mean(&|_| true)
    );
    assert!(lowered > 0);
    let mut working = staged.working.clone();
    run(&pass, &mut working);
    assert!(working.routed);
    for (c, cell) in working.cells.iter().enumerate() {
        let renewed = cell.crust == Crust::Province;
        let expected = if young[c] || renewed { 0.0 } else { 0.1 };
        assert_same_bits(cell.runoff_m_per_year, expected);
    }
}

/// The paleo-water is the inventory (decision-r09-t14-choices Q2): on the reference Mars the
/// cells' paleo-water sums to the inventory over the sphere, its mass ÷ water's triple-point
/// density × Σ Ω R², to 10⁻¹², and the bodies hold it all, with what they discharged (none, with
/// no sea), to 10⁻⁹ relative.
#[test]
fn the_paleo_water_stored_is_the_inventory() {
    let staged = Staged::of(reference::mars_like());
    let pass = staged.pass();
    let epoch = pass.inputs().wet_epoch().unwrap();
    let water = paleo_water_m3(&pass, epoch);
    let r = pass.inputs().radius().value();
    let sphere: f64 = pass.grid().solid_angles_sr().iter().sum();
    let depth = epoch.paleo_inventory.value() / Fluid::of_epoch(epoch).density_kg_m3;
    let total: f64 = water.iter().sum();
    let expected = depth * sphere * r * r;
    assert!(
        (total / expected - 1.0).abs() < 1e-12,
        "{total} m³ against {expected}"
    );
    let outcome = erode_dry_now(&pass, &staged.working).unwrap();
    let held: f64 = outcome.bodies.bodies().iter().map(|b| b.amount).sum();
    let stored = held + outcome.bodies.discharged();
    eprintln!(
        "paleo-water {total:.6e} m³ ({depth:.3} m deep), held {held:.6e} m³, discharged {:.3e}",
        outcome.bodies.discharged()
    );
    assert!(
        (stored / total - 1.0).abs() < 1e-9,
        "{stored} m³ stored of {total}"
    );
}

/// The step's provisional outputs on the reference Mars (the lane's design §4.3; Q7's step 1,
/// before R09.T14.d's re-match): the field routed; every eroded cell at the branch's elevation,
/// its construction less its valley's void, and every bound at its ground, to the bit; the water
/// surface the elevation everywhere (dry now); each cell's channel incision the branch's, z0 less
/// its solved channel, never negative and at least its lowering, outside a waiting crater's rim,
/// and zero inside it, whose ground formed after the epoch and which neither carries a valley nor
/// lowers for one (R09.T6.c); every cell's drainage at
/// least its own area and its catchment's runoff the accumulated own runoffs over it, at most the
/// epoch's; a steepness index on every cell that drains, none at a root; and the state
/// quantising.
#[test]
fn the_step_writes_the_reference_mars_s_outputs() {
    let staged = Staged::of(reference::mars_like());
    let pass = staged.pass();
    let dry = dry_now(&pass, &staged.working).unwrap();
    let young = dry.young.clone();
    assert_eq!(young, inside_waiting_rims(&pass, &staged.working));
    let mut working = staged.working.clone();
    let provisional = provisional(&pass, &mut working).expect("the reference Mars is eroded");
    assert_eq!(provisional.branch, Branch::DryNow);
    assert!(working.routed);
    let omega = pass.grid().solid_angles_sr();
    let r = pass.inputs().radius().value();
    let mut routed = 0_usize;
    for (c, (cell, was)) in working.cells.iter().zip(&staged.working.cells).enumerate() {
        let z0 = was.elevation.value();
        match dry.outcome.bounds[c] {
            Some(_) => assert_same_bits(cell.elevation.value(), z0),
            None => assert_same_bits(cell.elevation.value(), dry.elevation_m[c]),
        }
        assert_same_bits(cell.water_surface.value(), cell.elevation.value());
        let incision = if young[c] { 0.0 } else { dry.incision_m[c] };
        assert_same_bits(cell.channel_incision.value(), incision);
        assert!(dry.incision_m[c] >= 0.0, "cell {c}");
        assert!(dry.lowering_m[c] <= dry.incision_m[c], "cell {c}");
        if young[c] {
            assert_same_bits(dry.lowering_m[c], 0.0);
            assert_same_bits(dry.valley_m[c], 0.0);
        }
        let own = omega[c] * r * r;
        assert!(cell.drainage.value() >= own * (1.0 - 1e-12), "cell {c}");
        assert!(cell.catchment_runoff_m_per_year <= 0.1 * (1.0 + 1e-12));
        assert!(cell.steepness_index_m0_9 >= 0.0);
        if cell.flow == FlowDirection::Terminal {
            assert_same_bits(cell.steepness_index_m0_9, 0.0);
        } else {
            routed += 1;
        }
    }
    eprintln!(
        "{routed} of {} cells drain to a neighbour, {} inside a waiting crater's rim",
        working.cells.len(),
        young.iter().filter(|&&late| late).count()
    );
    assert!(routed > 0);
    assert_eq!(
        budget::records(pass.inputs(), &working).count(),
        working.cells.len()
    );
}

/// The provinces' own surface (decision-r09-t14-mars-ceiling §4.1, Q4's test (4)): on the
/// reference Mars, whose provinces are younger than its epoch's end, every `Province` cell's
/// elevation from the solve (the step's provisional outputs, before R09.T14.d's re-match scales the
/// whole field) is the elevation that entered it, to the bit, and after the whole step it has no
/// runoff of its own and no channel incision; and on a Mars whose provinces are as old as its
/// surface, some `Province` cell is lowered, their bed is the lid's saturated regolith, and their
/// lowering is printed against (B), the area p99 of 350 m × 10 km ÷ each cell's width (pending,
/// reported to main before it is asserted).
#[test]
fn the_provinces_erode_by_their_own_surface_s_age() {
    let staged = Staged::of(reference::mars_like());
    let pass = staged.pass();
    let mut working = staged.working.clone();
    let provisional = provisional(&pass, &mut working).expect("the reference Mars is eroded");
    let solved = working.clone();
    let _record = super::super::outputs::finish(&pass, &mut working, provisional);
    let mut provinces = 0_usize;
    for ((cell, was), solved) in working
        .cells
        .iter()
        .zip(&staged.working.cells)
        .zip(&solved.cells)
    {
        if was.crust == Crust::Province {
            provinces += 1;
            assert_same_bits(solved.elevation.value(), was.elevation.value());
            assert_same_bits(cell.runoff_m_per_year, 0.0);
            assert_same_bits(cell.channel_incision.value(), 0.0);
        }
    }
    eprintln!("the reference Mars: {provinces} province cells, each untouched");
    assert!(provinces > 0);
    let mars = reference::mars_like();
    let old = builder_of(&mars)
        .province_surface_age(mars.surface_age())
        .build()
        .unwrap();
    let staged = Staged::of(old);
    let pass = staged.pass();
    let dry = dry_now(&pass, &staged.working).unwrap();
    let omega = pass.grid().solid_angles_sr();
    let r = pass.inputs().radius().value();
    let cells: Vec<usize> = (0..dry.lowering_m.len())
        .filter(|&c| {
            staged.working.cells[c].crust == Crust::Province && dry.outcome.bounds[c].is_none()
        })
        .collect();
    let lowered = cells.iter().filter(|&&c| dry.lowering_m[c] > 0.0).count();
    let mut ratios: Vec<(f64, f64)> = cells
        .iter()
        .map(|&c| {
            let bound = 350.0 * 10.0e3 / (r * omega[c].sqrt());
            (dry.lowering_m[c] / bound, omega[c])
        })
        .collect();
    ratios.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
    let total: f64 = ratios.iter().map(|x| x.1).sum();
    let mut held = 0.0;
    let p99 = ratios
        .iter()
        .find(|x| {
            held += x.1;
            held >= 0.99 * total
        })
        .map_or(0.0, |x| x.0);
    eprintln!(
        "a Mars of old provinces: {lowered} of {} province cells lowered; their (B) area p99 {p99:.3} \
         of the bound ({})",
        cells.len(),
        if p99 <= 1.0 {
            "holds"
        } else {
            "FAILS, pending"
        }
    );
    assert!(lowered > 0);
    let erodibility = super::super::erodibility::Erodibility::of(pass.inputs()).unwrap();
    assert_same_bits(
        erodibility.of_crust(Crust::Province),
        erodibility.of_crust(Crust::Lid),
    );
}
