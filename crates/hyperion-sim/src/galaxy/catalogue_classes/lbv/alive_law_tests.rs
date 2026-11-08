//! The tests of the luminous blue variables' half of the carve-out's residuals (P09.T35's census
//! C1, P09.T43; research-leans-49 §1; on the record from 31): the class's living hosts follow the
//! grid's alive law, `StayMarks`' alive density over the share born within the reference lifetime.
//!
//! - Analytic, per young component and sampler node: the class's expected hosts per layer-E system,
//!   read from its own weights, bounds and odds, equal the grid's alive density integrated over the
//!   node's masses and host ages, and the odds never pass their node's bound (to 30 the class
//!   held about 0.6 of the grid's).
//! - Slow: over 2,048 ridge cells of layer E, the grid's living records that pass `may_host`
//!   against the class's living candidates there, thinned on the alive law.

use hyperion_testkit::stats::{ALPHA, assert_p_value, normal_cdf};

use super::*;
use crate::GENERATOR_VERSION;
use crate::Seed;
use crate::galaxy::ages::FieldShareRules;
use crate::galaxy::catalogue_classes::grid::candidate_count;
use crate::galaxy::displaced::site::CarveOutRules;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{CandidateOutcome, CellKey, evaluate_candidate};
use crate::galaxy::rules::GalaxyRules;
use crate::planetary::testing::in_shares;
use crate::stellar::draws::StarDraws;

/// The carve-out census's fixture seed.
const SEED: u64 = 0x0935_0000_0000_0001;

/// The census's fixture at the budget without the carve-out.
fn budget() -> Galaxy {
    Galaxy::from_params_with_rules(
        Seed::new(SEED),
        GalaxyParams::milky_way_like(),
        GalaxyRules {
            field_share: FieldShareRules::None,
            carve_out: CarveOutRules::Unclaimed,
            ..GalaxyRules::of_version(GENERATOR_VERSION)
        },
    )
    .expect("the Milky Way fixture builds")
}

/// Per young component and sampler node, the class's expected hosts per layer-E system equal the grid's alive law's, to rounding: the class's from its node weights, its
/// node's bound and the alive odds at each mass, `Σ_k w_k ÷ (span_k b_k) ∫ p(m) ÷ P_k × odds ×
/// A(m) dm`, the grid's from `StayMarks` directly, `share × odds_alive × ∫ ρ̂(m) ÷ F(τ_ref) ×
/// A(m) dm`, with `A(m)` the share of the component's ages in the node's host ages and below
/// `τ_ref(m)` (the median star's lifetime at the reference composition), 48 log-spaced masses per
/// node. The odds stay under their node's bound at every mass.
#[test]
fn living_hosts_follow_the_grid_s_alive_law_node_by_node() {
    let galaxy = budget();
    let process = LbvProcess::new(&galaxy);
    let table = galaxy
        .fields()
        .displaced()
        .expect("the classes are installed")
        .class_table();
    let mass_function = galaxy.mass_function();
    let draws = StarDraws::median();
    assert_eq!(
        process.components().len(),
        3,
        "the young disc and the nuclear disc's two"
    );
    for (slot, young) in process.components().iter().enumerate() {
        let id = young.id();
        let law = process
            .alive_law(slot)
            .expect("a young component's alive law");
        let marks = table.stay_marks(id);
        let rho = marks
            .mass_density(StayCategory::Alive)
            .expect("an alive density");
        let grid_share = galaxy.field_component_share(MassBand::E, id) * marks.odds()[0];
        let ages = galaxy.fields().component(id).ages();
        let z_ref = table.reference_composition(table.component_source(id));
        let (mut class_all, mut grid_all) = (0.0, 0.0);
        for (k, node) in process.nodes().iter().enumerate() {
            let (lo, hi) = (node.masses().0.value(), node.masses().1.value());
            let (h0, h1) = (node.ages().0.value(), node.ages().1.value());
            let span = h1 - h0;
            let in_node = mass_function.integral(lo, hi);
            let steps = 48;
            let (mut class, mut grid) = (0.0, 0.0);
            for i in 0..steps {
                let t = (f64::from(i) + 0.5) / f64::from(steps);
                let m = lo * crate::math::powf(hi / lo, t);
                let dm = m * crate::math::ln(hi / lo) / f64::from(steps);
                let mass = SolarMasses::new(m);
                let tau = lifetime_under(mass, &z_ref, &draws, galaxy.rules().stellar);
                let top = h1.min(tau.value()).min(ages.max().value());
                let bottom = h0.max(ages.min().value());
                let a = if top > bottom {
                    ages.cdf(Years::new(top)) - ages.cdf(Years::new(bottom))
                } else {
                    0.0
                };
                let odds = law.odds(&galaxy, id, mass, tau);
                assert!(
                    odds <= law.bound(k),
                    "{id:?} {m} M☉: {odds} over {}",
                    law.bound(k)
                );
                class += young.node_weights()[k] / (span * law.bound(k))
                    * (mass_function.pdf(m) / in_node)
                    * odds
                    * a
                    * dm;
                let born = born_within(ages, tau.value());
                if born > 0.0 {
                    grid += rho.pdf(m) / born * a * dm;
                }
            }
            let (class, grid) = (class * young.share(), grid * grid_share);
            assert!(
                (class - grid).abs() <= 1e-9 * grid.abs().max(f64::MIN_POSITIVE),
                "{id:?} node {k}: the class's {class:e} against the grid's {grid:e}"
            );
            eprintln!(
                "{:?} node {k}: grid {grid:.4e} per layer-E system",
                galaxy.fields().component(id).population()
            );
            class_all += class;
            grid_all += grid;
        }
        eprintln!(
            "{:?}: the class {:.6} of the grid's {grid_all:.4e}",
            galaxy.fields().component(id).population(),
            class_all / grid_all
        );
    }
}

/// `count` layer-E cells along each arm's ridge in the plane at each of `radii` ly.
fn ridge_cells(galaxy: &Galaxy, radii: &[f64], count: i32) -> Vec<CellKey> {
    let arms = galaxy.fields().arms();
    let mut keys = Vec::new();
    for &r in radii {
        for arm in 0..4 {
            let (sin, cos) = crate::math::sin_cos(arms.ridge_azimuth(r, arm));
            for step in -count / 2..count / 2 {
                let along = f64::from(step) * f64::from(Layer::E.cell_size_ly());
                let at = GalacticPosition::from_light_years([
                    r * cos - along * sin,
                    r * sin + along * cos,
                    0.0,
                ])
                .expect("in the cube");
                let key = CellKey::containing(Layer::E, &at).expect("a cell of the cube");
                if !keys.contains(&key) {
                    keys.push(key);
                }
            }
        }
    }
    keys
}

/// The grid's living layer-E field records in `cells` that pass `may_host`, and the class's
/// candidates there that are alive at the epoch (their age under their lifetime at their own
/// composition and draws) and pass the alive law's thinning: two Poisson counts of one intensity;
/// and the thinnings whose odds passed their node's bound.
fn ridge_counts(galaxy: &Galaxy, cells: &[CellKey]) -> (u64, u64, usize) {
    let process = LbvProcess::new(galaxy);
    let grid: u64 = in_shares(16, 16, |share| {
        let mut n = 0_u64;
        for &key in cells.iter().skip(share).step_by(16) {
            for index in 0..crate::galaxy::placement::candidate_count(galaxy, key) {
                if let CandidateOutcome::Accepted(record) = evaluate_candidate(galaxy, key, index)
                    && record.placement_class() == PlacementClass::Alive
                    && may_host(record.primary_initial_mass(), record.age_at_epoch())
                {
                    n += 1;
                }
            }
        }
        n
    })
    .into_iter()
    .sum();
    let mut class_cells: Vec<CatalogueCellKey> = Vec::new();
    for key in cells {
        let centre = key.cell_box().centre();
        let at = GalacticPosition::from_light_years([centre.x, centre.y, centre.z])
            .expect("in the cube");
        let class = CatalogueCellKey::containing(ClassId::LUMINOUS_BLUE_VARIABLE, &at)
            .expect("a class cell");
        if !class_cells.contains(&class) {
            class_cells.push(class);
        }
    }
    let parts = in_shares(16, 16, |share| {
        let over = alive_over_bound();
        let mut n = 0_u64;
        for key in class_cells.iter().skip(share).step_by(16) {
            let bound = process.bound(galaxy, key);
            for index in 0..candidate_count(galaxy.seed(), key, bound) {
                let id = key.candidate_id(index, 0).expect("a candidate");
                let Some(candidate) = process.candidate(galaxy, key, id, bound) else {
                    continue;
                };
                let Ok(layer_e) = CellKey::containing(Layer::E, &candidate.site) else {
                    continue;
                };
                if !cells.contains(&layer_e) || !may_host(candidate.mass, candidate.age_at_epoch) {
                    continue;
                }
                let record = candidate.record();
                let life = lifetime_under(
                    candidate.mass,
                    &candidate.composition,
                    &primary_draws(galaxy, &record),
                    galaxy.rules().stellar,
                );
                if candidate.age_at_epoch.value() < life.value()
                    && process.lives_on_the_grid(galaxy, &candidate)
                {
                    n += 1;
                }
            }
        }
        (n, alive_over_bound() - over)
    });
    let class = parts.iter().map(|p| p.0).sum();
    let over = parts.iter().map(|p| p.1).sum();
    (grid, class, over)
}

/// The ridge cells' living hosts, the grid's against the class's: one intensity (asserted at α;
/// to 30 the class held about 0.6 of the grid's).
#[test]
#[ignore = "slow: 2,048 ridge cells of layer E and the class's cells about them"]
fn ridge_cells_living_hosts_match_the_grid() {
    let galaxy = budget();
    let cells = ridge_cells(&galaxy, &[20_000.0, 24_000.0, 26_000.0, 28_000.0], 128);
    let (grid, class, over) = ridge_counts(&galaxy, &cells);
    assert_eq!(over, 0, "alive odds over their node's bound");
    let (a, b) = (
        f64::from(u32::try_from(grid).unwrap()),
        f64::from(u32::try_from(class).unwrap()),
    );
    let z = (a - b) / (a + b).max(1.0).sqrt();
    let p = 2.0 * normal_cdf(-z.abs());
    eprintln!(
        "{} cells, the grid's {grid} living may-host records, the class's \
         {class}, z = {z:.2}, p = {p:.2e}",
        cells.len()
    );
    assert!(grid > 0);
    assert_p_value("the ridge cells' living hosts", p, ALPHA);
}
