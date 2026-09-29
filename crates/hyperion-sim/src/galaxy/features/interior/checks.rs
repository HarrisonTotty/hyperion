//! The class device's tests over the named clusters and the Milky Way's globulars (plan 09,
//! P09.T8.b, P09.T9 and the checks of P09.T11 that need no member draws).

use std::fmt::Write as _;

use hyperion_testkit::lcg::Lcg;
use hyperion_testkit::stats::{ALPHA, assert_p_value, chi_square_gof};

use super::super::cluster::ClusterModel;
use super::super::testing::{
    M4, OMEGA_CEN, PAL_5, TUC_47, catalogue_parameters, milky_way_globulars, named_cluster,
};
use super::*;
use crate::Seed;
use crate::galaxy::consts::LIGHT_YEARS_PER_YEAR_PER_KM_S;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::{Galaxy, PointLy};
use crate::rng::Mark;
use crate::units::LightYears;

fn galaxy() -> Galaxy {
    Galaxy::from_params(Seed::new(0x0909_0000), GalaxyParams::milky_way_like()).unwrap()
}

fn model(galaxy: &Galaxy, name: &str) -> ClusterModel {
    ClusterModel::new(galaxy, &catalogue_parameters(named_cluster(name)))
}

/// A table with the tails out to four tidal radii along +x.
fn table(galaxy: &Galaxy, model: &ClusterModel) -> MemberClassTable {
    let reach = LightYears::new(4.0 * model.tidal_radius().value());
    MemberClassTable::new(galaxy, model, reach, [1.0, 0.0, 0.0])
}

// --- P09.T8.b ---

#[test]
fn class_frequencies_follow_the_odds_at_fixed_positions() {
    let galaxy = galaxy();
    let model = model(&galaxy, M4);
    let table = table(&galaxy, &model);
    let mut lcg = Lcg::new(0x09_08b0);
    let r_h = model.half_mass_radius().value();
    for (band, p) in [
        (MassBand::A, PointLy::new(0.2 * r_h, 0.0, 0.0)),
        (MassBand::C, PointLy::new(0.0, r_h, 0.3 * r_h)),
        (
            MassBand::A,
            PointLy::new(1.6 * model.tidal_radius().value(), 0.0, 1.0),
        ),
    ] {
        let mut weights = Vec::new();
        table.weights(band, &p, &mut weights);
        let bound = 1.5 * weights.iter().sum::<f64>();
        let n = 50_000_u32;
        let mut counts = vec![0_u64; weights.len() + 1];
        let classes: Vec<MemberClass> = table
            .classes(band)
            .map(|(c, _)| *c)
            .chain(table.tail(band).map(|t| *t.class()))
            .collect();
        for _ in 0..n {
            match table.pick(band, &p, Mark::from_word(lcg.next_u64()), bound) {
                Some(class) => {
                    let i = classes.iter().position(|c| *c == class).unwrap();
                    counts[i] += 1;
                }
                None => counts[weights.len()] += 1,
            }
        }
        let mut expected: Vec<f64> = weights.iter().map(|w| f64::from(n) * w / bound).collect();
        expected.push(f64::from(n) * (1.0 - weights.iter().sum::<f64>() / bound));
        // Pool the rarest classes, which the chi-square cannot test one by one.
        let (mut obs, mut exp) = (Vec::new(), Vec::new());
        let (mut o_rest, mut e_rest) = (0, 0.0);
        for (&o, &e) in counts.iter().zip(&expected) {
            if e >= 20.0 {
                obs.push(o);
                exp.push(e);
            } else {
                o_rest += o;
                e_rest += e;
            }
        }
        if e_rest > 0.0 {
            obs.push(o_rest);
            exp.push(e_rest);
        }
        let test = chi_square_gof(&obs, &exp);
        assert_p_value(&format!("{band:?} at {p:?}"), test.p_value, ALPHA);
    }
}

/// A random cell of a nested level, as P09.T20's grid will cut them: its edge a power of two
/// from 1 ⁄ 64 ly to 1,024 ly times a unit's fraction, placed anywhere within the reach.
fn random_cell(lcg: &mut Lcg, reach: f64) -> LocalCell {
    let level = lcg.next_u64() % 17;
    let edge = f64::from(1_u32 << level) / 64.0;
    let r = |lcg: &mut Lcg| (unit(lcg) * 2.0 - 1.0) * reach;
    let min = [r(lcg), r(lcg), r(lcg)].map(|c| (c / edge).floor() * edge);
    LocalCell { min, edge }
}

fn unit(lcg: &mut Lcg) -> f64 {
    #[expect(clippy::cast_precision_loss, reason = "53 bits are exact")]
    let u = (lcg.next_u64() >> 11) as f64;
    u / 9_007_199_254_740_992.0
}

#[test]
fn no_density_exceeds_its_cell_bound_over_a_million_positions() {
    let galaxy = galaxy();
    let mut lcg = Lcg::new(0x09_08b1);
    let mut weights = Vec::new();
    for name in [M4, PAL_5] {
        let model = model(&galaxy, name);
        let table = table(&galaxy, &model);
        let reach = 4.5 * model.tidal_radius().value();
        for _ in 0..50_000 {
            let cell = random_cell(&mut lcg, reach);
            for band in MassBand::ALL {
                let bound = table.cell_bound(band, &cell);
                for _ in 0..2 {
                    let p = PointLy::new(
                        cell.min[0] + unit(&mut lcg) * cell.edge,
                        cell.min[1] + unit(&mut lcg) * cell.edge,
                        cell.min[2] + unit(&mut lcg) * cell.edge,
                    );
                    table.weights(band, &p, &mut weights);
                    let sum: f64 = weights.iter().sum();
                    assert!(
                        sum <= bound,
                        "{name} {band:?} at {p:?}: {sum} above {bound}"
                    );
                }
            }
        }
    }
}

// --- P09.T9 over the named clusters ---

#[test]
fn neutron_stars_black_holes_and_pulsars_of_the_named_clusters() {
    let galaxy = galaxy();
    let mut report = String::new();
    for name in [TUC_47, M4, PAL_5, OMEGA_CEN] {
        let model = model(&galaxy, name);
        let table = table(&galaxy, &model);
        let ns = table.expected_of(ClassKind::NeutronStar);
        let bh = table.expected_of(ClassKind::BlackHole);
        writeln!(
            report,
            "{name}: {ns:.0} neutron stars, {bh:.0} black holes (f {:.4}), {:.1} pulsars, t/t★ {:.2}, \
             core-collapsed {}, v_esc {:.1} km/s",
            model.black_hole_fraction(),
            model.millisecond_pulsars(),
            model.age().value() / model.relaxation_time().value(),
            model.is_core_collapsed(),
            model.escape_speed_central().value(),
        )
        .expect("a String takes any write");
    }
    eprintln!("{report}");
    let ns = |name: &str| table(&galaxy, &model(&galaxy, name)).expected_of(ClassKind::NeutronStar);
    let bh = |name: &str| model(&galaxy, name).black_hole_count();
    // P09.T9.b (ruling 126.3): 47 Tucanae 1,000–5,000 neutron stars, Palomar 5 under one
    // expected. M4 170–600 (ruling 140.5): 126.3's 100–350, stated at w 0.181 and the scratch
    // Chabrier scale 0.68, × 1.46–1.54 for the retention at v_eff 50 km/s and w 0.2675 (as ruling
    // 137.3) and × 1.13–1.18 for the progenitors per M☉ formed at the fitted 0.9201. Ye et al.
    // 2019's CMC models give 150–225 at M4's mass, about half the measured 396: a tension on w
    // per primary-born neutron star, re-checked at P11.T6/T11 with the companions' neutron stars.
    let (tuc, m4, pal) = (ns(TUC_47), ns(M4), ns(PAL_5));
    assert!((1_000.0..=5_000.0).contains(&tuc), "47 Tuc: {tuc}");
    assert!((170.0..=600.0).contains(&m4), "M4: {m4}");
    assert!(pal < 1.0, "Palomar 5: {pal}");
    // P09.T9.c (ruling 126.4): none in the dynamically old M4, 20–400 in 47 Tucanae, 3,000–20,000
    // in ω Centauri.
    assert!(bh(M4) < 1.0, "M4: {}", bh(M4));
    let tuc_bh = bh(TUC_47);
    assert!((20.0..=400.0).contains(&tuc_bh), "47 Tuc: {tuc_bh}");
    let omega = bh(OMEGA_CEN);
    assert!((3_000.0..=20_000.0).contains(&omega), "ω Cen: {omega}");
}

#[test]
fn tails_hold_what_the_cluster_lost_and_no_second_population() {
    use super::super::kinds::open_cluster::evolution_survival;
    use super::counts::tail_window;
    let galaxy = galaxy();
    let model = model(&galaxy, TUC_47);
    let reach = LightYears::new(4.0 * model.tidal_radius().value());
    let table = table(&galaxy, &model);
    let window = tail_window(&galaxy, &model, reach);
    let mut members = 0.0;
    let mut mass = 0.0;
    for band in MassBand::ALL {
        if let Some(tail) = table.tail(band) {
            assert_eq!(tail.class().generation, Generation::First);
            members += tail.expected();
            mass += tail.expected() * tail.class().mean_mass.value();
        }
        for (class, _) in table.classes(band) {
            assert_ne!(class.kind, ClassKind::Tail);
        }
    }
    // Ruling 139.4: the count times the mean lost mass is the mass lost in τ, and no tail holds
    // more than the stars the cluster has lost.
    assert!(members > 0.0);
    assert!(
        (mass / window.mass.value() - 1.0).abs() < 1e-9,
        "{mass} against {}",
        window.mass.value()
    );
    let lost =
        model.initial_mass().value() * evolution_survival(model.age()) - model.mass().value();
    assert!(mass <= lost * (1.0 + 1e-12), "{mass} against {lost}");
    eprintln!(
        "47 Tuc: v_drift {:.2} km/s, tail {:.0} ly for {:.2e} yr, {members:.0} members",
        window.drift_speed,
        window.length.value(),
        window.duration.value()
    );
    // 47 Tucanae was born above 10⁵ M☉: its profiled classes have both populations.
    let second = table
        .classes(MassBand::A)
        .filter(|(c, _)| c.generation == Generation::Second)
        .count();
    assert!(second > 0);
}

#[test]
fn a_cluster_s_classes_hold_its_present_mass() {
    let galaxy = galaxy();
    for name in [TUC_47, M4, OMEGA_CEN] {
        let model = model(&galaxy, name);
        let table = table(&galaxy, &model);
        let mut mass = 0.0;
        for band in MassBand::ALL {
            for (class, n) in table.classes(band) {
                mass += n * class.mean_mass.value();
            }
        }
        assert!(
            (mass / model.mass().value() - 1.0).abs() < 1e-9,
            "{name}: {mass}"
        );
    }
}

// --- P09.T9 and P09.T11 over the whole catalogue ---

#[test]
#[ignore = "slow: builds a cluster model for every Milky Way globular"]
fn the_milky_way_s_globulars_have_their_observed_remnants() {
    let galaxy = galaxy();
    let mut pulsars = 0.0;
    let mut black_holes = 0.0;
    let mut collapsed = 0_u32;
    let mut capped = 0_u32;
    let mut n = 0_u32;
    for g in milky_way_globulars() {
        let model = ClusterModel::new(&galaxy, &catalogue_parameters(g));
        // Ruling 126.3: no cluster's pulsars outnumber its neutron stars. The encounter-rate law
        // alone would in a few clusters with more encounters than neutron stars, so the table
        // caps the pulsars at the class they are marks in (a deviation recorded in plan 09); the
        // test holds that cap and the number of clusters it binds in.
        let table = table(&galaxy, &model);
        let neutron_stars = table.expected_of(ClassKind::NeutronStar);
        let held = table.millisecond_pulsars(&model);
        assert!(
            held <= neutron_stars,
            "{}: {held} pulsars, {neutron_stars} neutron stars",
            g.name
        );
        if model.millisecond_pulsars() > neutron_stars {
            capped += 1;
        }
        pulsars += held;
        black_holes += model.black_hole_count();
        if model.is_core_collapsed() {
            collapsed += 1;
        }
        n += 1;
    }
    let collapsed_share = f64::from(collapsed) / f64::from(n);
    eprintln!(
        "pulsars {pulsars:.0} ({capped} clusters capped at their neutron stars), black holes \
         {black_holes:.0}, core-collapsed {collapsed_share:.3}"
    );
    // Five clusters of 165 bind the cap, each with well under one pulsar expected.
    assert!(capped <= 8, "{capped} clusters capped");
    // P09.T9.e: about 4,000 (2,000–8,000) pulsars over the system.
    assert!((2_000.0..=8_000.0).contains(&pulsars), "{pulsars} pulsars");
    // P09.T9.d: about a fifth (0.12–0.28) over the core-collapse line (Trager et al. 1995).
    assert!(
        (0.12..=0.28).contains(&collapsed_share),
        "{collapsed_share}"
    );
    // P09.T11: black holes over the system of order 10⁴–10⁵.
    assert!(
        (1e4..=1e5).contains(&black_holes),
        "{black_holes} black holes"
    );
}

/// Ruling 139.4: a nursery of 241 M☉ at birth, 69 Myr old at the solar circle, has a tail as long
/// as its oldest escaper has drifted, `r_t + v_drift × age`, holding the 147 M☉ of stars it has
/// lost (the research's figure). The ruling's "about 70 ly" and "300–420 members" took a flat
/// rotation curve and a mean lost mass of 0.35–0.5 M☉; the fixture's Ω and κ give `4Ω² ÷ κ² − 1`
/// = 1.27 (87 ly), and the tail's bottom-heavy band shares as built a mean of 0.235 M☉ (623
/// members), which plan 09 records.
#[test]
fn a_young_cluster_s_tail_holds_the_stars_it_has_lost() {
    use super::super::cluster::{ClusterKind, ClusterMarks, ClusterParameters};
    use super::super::kinds::open_cluster::present_mass;
    use super::counts::tail_window;
    use crate::galaxy::Population;
    use crate::units::{Dex, SolarMasses, Years};
    let galaxy = galaxy();
    let (m0, age) = (SolarMasses::new(241.0), Years::new(69e6));
    let mass = present_mass(m0, age);
    assert!((55.0..65.0).contains(&mass.value()), "{mass:?}");
    let model = ClusterModel::new(
        &galaxy,
        &ClusterParameters {
            kind: ClusterKind::Open,
            population: Population::YoungThinDisc,
            component: None,
            position: PointLy::new(0.0, 26_700.0, 0.0),
            age,
            fe_h: Dex::ZERO,
            mass,
            initial_mass: m0,
            half_mass_radius: LightYears::new(4.75),
            birth_half_mass_radius: LightYears::new(4.75),
            core_radius: None,
            concentration: 1.0,
            mass_loss_rate: 1.6e-6,
            age_spread: Years::ZERO,
            marks: ClusterMarks::MEDIAN,
        },
    );
    let reach = LightYears::new(1_750.0);
    let window = tail_window(&galaxy, &model, reach);
    let table = MemberClassTable::new(&galaxy, &model, reach, [1.0, 0.0, 0.0]);
    let members: f64 = MassBand::ALL
        .iter()
        .filter_map(|&b| table.tail(b))
        .map(TailClass::expected)
        .sum();
    let r_t = model.tidal_radius().value();
    let v = window.drift_speed * LIGHT_YEARS_PER_YEAR_PER_KM_S;
    eprintln!(
        "nursery: r_t {r_t:.1} ly, v_drift {:.3} km/s, tail {:.1} ly, {:.1} M☉, {members:.0} members",
        window.drift_speed,
        window.length.value(),
        window.mass.value()
    );
    assert!((140.0..155.0).contains(&window.mass.value()), "{window:?}");
    assert!(
        (window.length.value() / (v * age.value()) - 1.0).abs() < 1e-12,
        "{window:?}"
    );
    let lost_mass: f64 = MassBand::ALL
        .iter()
        .filter_map(|&b| table.tail(b))
        .map(|t| t.expected() * t.class().mean_mass.value())
        .sum();
    assert!((lost_mass / window.mass.value() - 1.0).abs() < 1e-9);
    assert!((250.0..1_000.0).contains(&members), "{members}");
    for band in MassBand::ALL {
        if let Some(tail) = table.tail(band) {
            assert!(tail.reach() <= r_t + v * age.value() * (1.0 + 1e-12));
        }
    }
}
