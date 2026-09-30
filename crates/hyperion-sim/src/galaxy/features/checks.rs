//! The globular system's tests and the Milky Way checks (plan 09, P09.T11–T14).

use hyperion_testkit::stats::{ALPHA, assert_p_value, assert_poisson_count, ks_one_sample};

use super::FeatureProcess;
use super::catalogue::{FeatureCatalogue, FeatureMarks, FeatureRecord, bulk_velocity};
use super::cluster::{ClusterModel, central_escape_speed};
use super::interior::retention::{EFFECTIVE_ESCAPE_FACTOR, retention};
use super::kinds::globular::{
    GLOBULAR_CUT, GlobularMarks, GlobularSystem, HALF_MASS_RADIUS_LAW, history, history_on_orbit,
};
use super::testing::milky_way_globulars;
use crate::Seed;
use crate::galaxy::consts::{LIGHT_YEARS_PER_KILOPARSEC, LIGHT_YEARS_PER_PARSEC};
use crate::galaxy::imf::MassFunctionKind;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::{Galaxy, PointLy};
use crate::math;
use crate::stellar::Composition;
use crate::stellar::remnant::StandardKickLaw;
use crate::units::{KilometresPerSecond, LightYears, SolarMasses, Years};

const SEED: u64 = 0x0912_0000_0000_0000;

fn milky_way(n: u64) -> Galaxy {
    Galaxy::from_params(Seed::new(SEED | n), GalaxyParams::milky_way_like()).unwrap()
}

fn globulars(galaxy: &Galaxy) -> Vec<(FeatureRecord, GlobularMarks)> {
    FeatureCatalogue::walk_process(galaxy, FeatureProcess::Globular)
        .filter_map(|f| match f.marks() {
            FeatureMarks::Globular(m) => Some((f, *m)),
            _ => None,
        })
        .collect()
}

fn radius_ly(f: &FeatureRecord) -> f64 {
    let p = PointLy::from(f.position());
    (p.x * p.x + p.y * p.y + p.z * p.z).sqrt()
}

// --- P09.T12.a ---

#[test]
fn every_seed_holds_80_to_800_and_the_fixture_about_160() {
    for n in 0..200 {
        let params = GalaxyParams::from_seed(Seed::new(SEED | n), MassFunctionKind::default());
        let count = params.accretion().globular_count();
        assert!((80..=800).contains(&count), "seed {n}: {count}");
    }
    let count = GalaxyParams::milky_way_like().accretion().globular_count();
    assert!((130..=190).contains(&count), "{count}");
}

#[test]
fn the_placed_globulars_number_as_their_law_expects() {
    // Ruling 126.5: the metal-rich part is placed whole, the metal-poor 0.82 of it: 0.874 of the
    // untruncated count.
    let mut observed = 0_u64;
    let mut expected = 0.0;
    for n in 0..4 {
        let galaxy = milky_way(n);
        let count = galaxy.feature_shares().globulars().count();
        expected += count * GlobularSystem::placed_share();
        observed += u64::try_from(globulars(&galaxy).len()).unwrap();
    }
    assert_poisson_count("globulars", observed, expected, ALPHA);
}

/// Ruling 126.5's medians inside the cuts: 5.0–5.6 kpc for the metal-poor globulars, 2.8–3.3 for
/// the metal-rich (Harris 2010: 5.2 and 3.05), over 32 galaxies, so the median's standard error
/// (about 0.1 kpc) is well inside the window (8 galaxies gave 4.91 kpc against the law's 5.28).
#[test]
#[ignore = "slow: walks the globulars of 32 galaxies"]
fn the_globulars_medians_follow_harris() {
    let (mut rich, mut poor) = (Vec::new(), Vec::new());
    for n in 0..32 {
        let galaxy = milky_way(n);
        for (f, m) in globulars(&galaxy) {
            let r = radius_ly(&f) / LIGHT_YEARS_PER_KILOPARSEC;
            assert!(radius_ly(&f) < GLOBULAR_CUT);
            if m.origin().is_metal_rich() {
                rich.push(r);
            } else {
                poor.push(r);
            }
        }
    }
    let median = |v: &mut Vec<f64>| {
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    };
    let (r, p) = (median(&mut rich), median(&mut poor));
    eprintln!("medians: metal-rich {r:.2} kpc, metal-poor {p:.2} kpc");
    assert!((2.8..=3.3).contains(&r), "metal-rich {r} kpc");
    assert!((5.0..=5.6).contains(&p), "metal-poor {p} kpc");
}

// --- P09.T12.b ---

#[test]
fn masses_and_sizes_follow_their_closed_forms() {
    let mut masses = Vec::new();
    let mut size_normals = Vec::new();
    let mut function = None;
    for n in 0..6 {
        let galaxy = milky_way(n);
        function.get_or_insert_with(|| galaxy.feature_shares().globulars().mass_function().clone());
        for (f, m) in globulars(&galaxy) {
            masses.push(m.mass().value());
            assert!(m.mass().value() >= 1e3, "a globular with no mass");
            let r_kpc = radius_ly(&f) / LIGHT_YEARS_PER_KILOPARSEC;
            let (r0, slope, scatter) = HALF_MASS_RADIUS_LAW;
            let law = r0 * math::powf(r_kpc.max(0.05), slope) * LIGHT_YEARS_PER_PARSEC;
            size_normals.push(math::log10(m.half_mass_radius().value() / law) / scatter);
        }
    }
    let function = function.unwrap();
    let ks = ks_one_sample(&mut masses, |m| function.cumulative(SolarMasses::new(m)));
    assert_p_value("globular masses", ks.p_value, ALPHA);
    let ks = ks_one_sample(&mut size_normals, |z| {
        0.5 * math::erfc(-z * core::f64::consts::FRAC_1_SQRT_2)
    });
    assert_p_value("globular sizes", ks.p_value, ALPHA);
}

// --- P09.T13 ---

/// The catalogue's clusters' initial masses and birth escape speeds by the inversion, at 12 Gyr on
/// their catalogue orbits.
fn catalogue_histories(galaxy: &Galaxy) -> Vec<(f64, f64, f64)> {
    milky_way_globulars()
        .iter()
        .map(|g| {
            let h = history_on_orbit(
                galaxy,
                LightYears::new(g.pericentre_kpc * LIGHT_YEARS_PER_KILOPARSEC),
                LightYears::new(g.apocentre_kpc * LIGHT_YEARS_PER_KILOPARSEC),
                SolarMasses::new(g.mass),
                LightYears::new(g.half_mass_radius_pc * LIGHT_YEARS_PER_PARSEC),
                Years::new(12e9),
            );
            let ratio = (h.initial_mass.value() / g.mass)
                * (g.half_mass_radius_pc * LIGHT_YEARS_PER_PARSEC
                    / h.birth_half_mass_radius.value());
            (
                math::log10(h.initial_mass.value()),
                g.log_initial_mass,
                ratio.sqrt(),
            )
        })
        .collect()
}

/// P09.T13 after ruling 126.1–2: the initial masses' median offset from the catalogue's within
/// ±0.25 dex and at least 70% within 0.25 dex (the catalogue's come from backward orbit
/// integration with dynamical friction, which no closed form reaches to 0.1 dex); the median birth
/// escape speed, today's times `√(M₀ ÷ M)`, 1.5–2.3 times today's.
#[test]
fn the_inversion_against_the_catalogue() {
    let galaxy = milky_way(0);
    let rows = catalogue_histories(&galaxy);
    let mut offsets: Vec<f64> = rows.iter().map(|(ours, theirs, _)| ours - theirs).collect();
    let within = offsets.iter().filter(|d| d.abs() <= 0.25).count();
    let mut ratios: Vec<f64> = rows.iter().map(|r| r.2).collect();
    offsets.sort_by(f64::total_cmp);
    ratios.sort_by(f64::total_cmp);
    let median_offset = offsets[offsets.len() / 2];
    let median_ratio = ratios[ratios.len() / 2];
    #[expect(clippy::cast_precision_loss, reason = "a count below 200 is exact")]
    let share = within as f64 / rows.len() as f64;
    eprintln!(
        "initial masses: median offset {median_offset:.3} dex, {share:.2} within 0.25 dex; \
         median birth escape over today's {median_ratio:.2}"
    );
    assert!(median_offset.abs() <= 0.25, "{median_offset}");
    assert!(share >= 0.7, "{share} within 0.25 dex");
    assert!((1.5..=2.3).contains(&median_ratio), "{median_ratio}");
}

// --- P09.T11 ---

#[test]
fn a_cluster_with_a_birth_escape_speed_of_50_km_s_keeps_a_tenth_of_its_neutron_stars() {
    let imf = MassFunctionKind::default().to_mass_function();
    let kept = retention(
        &StandardKickLaw::default(),
        imf.as_ref(),
        &Composition::SOLAR,
        KilometresPerSecond::new(50.0 * EFFECTIVE_ESCAPE_FACTOR),
    );
    assert!(kept.neutron_stars >= 0.1, "{}", kept.neutron_stars);
}

// --- P09.T14 ---

/// The fixture's globulars over 50 seeds, with their orbits: the brainstorm's checks of the
/// globular system (P09.T14).
#[test]
#[ignore = "slow: builds the full potential of 50 galaxies and every globular's orbit"]
fn the_milky_way_s_globular_system() {
    let mut inner = Vec::new();
    let mut outer = Vec::new();
    let mut sizes = Vec::new();
    let mut eccentricities = Vec::new();
    let mut pericentres = Vec::new();
    let mut escapes = Vec::new();
    for n in 0..50 {
        let galaxy = milky_way(n).with_full_potential();
        for (f, m) in globulars(&galaxy) {
            let r = radius_ly(&f);
            let log_m = math::log10(m.mass().value());
            if r < 5.0 * LIGHT_YEARS_PER_KILOPARSEC {
                inner.push(log_m);
            } else {
                outer.push(log_m);
            }
            sizes.push((
                math::log10(r / LIGHT_YEARS_PER_KILOPARSEC),
                math::log10(m.half_mass_radius().value() / LIGHT_YEARS_PER_PARSEC),
            ));
            let h = history(
                &galaxy,
                f.position(),
                bulk_velocity(&galaxy, &f),
                m.mass(),
                m.half_mass_radius(),
                m.age(),
            );
            eccentricities.push(h.eccentricity);
            pericentres.push(h.pericentre.value() / LIGHT_YEARS_PER_KILOPARSEC);
            escapes.push(
                central_escape_speed(m.mass(), m.half_mass_radius(), m.core_radius()).value(),
            );
        }
    }
    let median = |v: &mut Vec<f64>| {
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    };
    // The turnover (the peak of dN ÷ d log M, in 0.2 dex bins) near 2 × 10⁵ M☉, inside and
    // outside 5 kpc alike.
    let peak = |v: &[f64]| {
        let mut bins = [0_u32; 20];
        for &x in v {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "3 ≤ x ≤ 7"
            )]
            let b = (((x - 3.0) / 0.2) as usize).min(19);
            bins[b] += 1;
        }
        let (i, _) = bins.iter().enumerate().max_by_key(|(_, c)| **c).unwrap();
        #[expect(clippy::cast_precision_loss, reason = "a bin index")]
        let centre = 3.0 + 0.2 * (i as f64 + 0.5);
        centre
    };
    let (p_in, p_out) = (peak(&inner), peak(&outer));
    eprintln!("turnover inside {p_in:.2}, outside {p_out:.2}");
    assert!(
        (4.9..=5.7).contains(&p_in) && (4.9..=5.7).contains(&p_out),
        "{p_in} {p_out}"
    );
    assert!((p_in - p_out).abs() <= 0.4);
    // Sizes against radius: the least-squares slope of log r_h on log R is the law's 0.41.
    #[expect(clippy::cast_precision_loss, reason = "a sample count")]
    let k = sizes.len() as f64;
    let (mx, my) = (
        sizes.iter().map(|s| s.0).sum::<f64>() / k,
        sizes.iter().map(|s| s.1).sum::<f64>() / k,
    );
    let slope = sizes.iter().map(|s| (s.0 - mx) * (s.1 - my)).sum::<f64>()
        / sizes.iter().map(|s| (s.0 - mx) * (s.0 - mx)).sum::<f64>();
    assert!((slope - 0.41).abs() < 0.05, "{slope}");
    let (e, peri, v) = (
        median(&mut eccentricities),
        median(&mut pericentres),
        median(&mut escapes),
    );
    eprintln!("median eccentricity {e:.2}, pericentre {peri:.2} kpc, escape speed {v:.1} km/s");
    assert!((0.5..=0.75).contains(&e), "median eccentricity {e}");
    assert!((1.0..=2.5).contains(&peri), "median pericentre {peri} kpc");
    // Ruling 126.8: under 1% above 100 km/s, a 99th percentile under 100, a median of 17–22 (the
    // Milky Way's 0 of 165 above 88 km/s is P ≈ 0.3 under a continuous law).
    assert!((17.0..=22.0).contains(&v), "median escape speed {v}");
    let fast = escapes.iter().filter(|&&x| x > 100.0).count();
    #[expect(clippy::cast_precision_loss, reason = "counts below 10⁵")]
    let fast_share = fast as f64 / escapes.len() as f64;
    let p99 = escapes[escapes.len() * 99 / 100];
    eprintln!(
        "escape speeds above 100 km/s: {fast_share:.4}, p99 {p99:.1}, fastest {:.0} km/s",
        escapes[escapes.len() - 1]
    );
    assert!(p99 < 100.0, "p99 {p99}");
    assert!(fast_share < 0.01, "{fast_share}");
}

/// P09.T11 over the named clusters with their histories: every globular the fixture places is a
/// cluster model with mass, and its black holes number no more than its mass allows.
#[test]
fn every_placed_globular_is_a_cluster_with_mass() {
    let galaxy = milky_way(1);
    for (f, _) in globulars(&galaxy).iter().take(10) {
        let model = ClusterModel::from_record(&galaxy, f).expect("a globular is a cluster");
        assert!(model.mass().value() >= 1e3);
        assert!(model.initial_mass().value() >= model.mass().value() / 0.70 * 0.999);
        assert!(model.black_hole_count() * 15.0 <= 0.07 * model.mass().value());
    }
}
