//! The natal kick law (plan 06, P06.T19): the reference population's rank table (T19.b), the six
//! kick-law tests of the brainstorm's Testing list (T19.d), and a golden of three pinned systems'
//! kicks.
//!
//! The statistical tests are slow: each builds the full tracks of 20,000 massive stars.

use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::placement::{CellKey, SystemRecord, generate_cell, resolve};
use hyperion_sim::id::{Layer, SystemId};
use hyperion_sim::stellar::remnant::reference::{
    KickObservables, QUANTILES, ReferencePopulation, kick_observables, sampled_kick,
    score_quantiles,
};
use hyperion_sim::stellar::remnant::{
    KickLawParams, KickMode, KickRankTable, RemnantKind, StandardKickLaw,
};
use hyperion_sim::stellar::system::SystemStars;
use hyperion_sim::units::KilometresPerSecond;
use hyperion_sim::{Seed, math};
use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::stats::{ALPHA, assert_p_value, ks_one_sample, normal_cdf};

// --- P06.T19.b: the reference population and its rank table ---

/// A seed the committed table was not made from.
const FRESH_SEED: u64 = 0x0619_b000_0000_0002;

/// Two runs of the quantile step over the same sample give the same table bit for bit, and it is
/// strictly increasing.
#[test]
fn two_runs_give_the_same_table_bit_for_bit() {
    let pop = ReferencePopulation::default();
    let a = score_quantiles(&pop, 48, Seed::new(FRESH_SEED));
    let b = score_quantiles(&pop, 48, Seed::new(FRESH_SEED));
    for (x, y) in a.iter().zip(&b) {
        assert_same_bits(*x, *y);
    }
    assert!(a.windows(2).all(|q| q[0] < q[1]));
    assert_eq!(a.len(), QUANTILES);
}

/// The committed table's quantiles are strictly increasing.
#[test]
fn the_committed_quantiles_are_strictly_increasing() {
    let table = KickRankTable::generator();
    assert_eq!(table.quantiles().len(), QUANTILES);
    assert!(table.quantiles().windows(2).all(|q| q[0] < q[1]));
    assert!(KickRankTable::new(table.quantiles().to_vec()).is_ok());
}

/// The ranks of 10⁵ fresh reference scores on the committed table are uniform (K–S at plan 01's
/// α): the table is the population's own distribution.
#[test]
#[ignore = "slow: 10⁵ reference scores, about 170,000 full tracks of massive stars"]
fn ranks_of_fresh_reference_scores_are_uniform() {
    const N: usize = 100_000;
    let pop = ReferencePopulation::default();
    let table = KickRankTable::generator();
    let seed = Seed::new(FRESH_SEED);
    let mut ranks = Vec::with_capacity(N);
    let mut start = 0;
    while ranks.len() < N {
        ranks.extend(
            pop.scores(seed, start..start + 4_096)
                .into_iter()
                .map(|x| table.rank(x)),
        );
        start += 4_096;
    }
    ranks.truncate(N);
    let ks = ks_one_sample(&mut ranks, |r| r.clamp(0.0, 1.0));
    eprintln!(
        "ranks of {N} fresh scores: D = {:.5}, p = {:.4}",
        ks.statistic, ks.p_value
    );
    assert_p_value("fresh reference ranks", ks.p_value, ALPHA);
}

// --- P06.T19.d: the kick-law tests ---

/// The seed and size of the kick-law tests' sample: a Kroupa sample of 8–150 M☉ at Z = 0.02,
/// about 12,000 neutron stars and 7,000 black holes.
const OBSERVABLES_SEED: u64 = 0x0619_d000_0000_0001;
const OBSERVABLE_STARS: u64 = 20_000;

fn observables() -> KickObservables {
    kick_observables(
        &KickLawParams::default(),
        &KickRankTable::generator(),
        OBSERVABLE_STARS,
        Seed::new(OBSERVABLES_SEED),
    )
}

/// Test 1: ordinary-mode neutron speeds against Disberg and Mandel's (2025) log-normal: the
/// moments of ln v within 5.60 ± 0.12 and 0.68 ± 0.10, and K–S against the log-normal truncated
/// at the rank clamp. The clamp holds the 0.1% of ranks below 0.001 at 0.001 and those above
/// 0.999 at 0.999, so the speeds' distribution function is the log-normal's own at every speed
/// inside the clamp and at both of its ends, which is the function the K–S test reads there. It
/// pins the rank table and the clamp. The sample is the reference population's
/// own (single and wind-stripped iron-core progenitors), whose speeds the table maps onto the
/// measurement; the whole ordinary mode, companion-stripped stars above the ramp included, is
/// reported beside it.
#[test]
#[ignore = "slow: 20,000 full tracks of 8–150 M☉"]
fn ordinary_neutron_star_speeds_follow_the_log_normal() {
    let o = observables();
    let p = KickLawParams::default();
    let (mean, sd) = o.reference_ln_moments;
    let (all_mean, all_sd, all_n) = o.ordinary_ln_moments;
    eprintln!(
        "ordinary NS (reference): n = {}, ln v mean {mean:.4}, sd {sd:.4}; all ordinary: n = \
         {all_n}, mean {all_mean:.4}, sd {all_sd:.4}",
        o.reference_ln_speeds.len()
    );
    assert!((mean - 5.60).abs() <= 0.12, "mean ln v {mean}");
    assert!((sd - 0.68).abs() <= 0.10, "sd ln v {sd}");
    let mut speeds = o.reference_ln_speeds.clone();
    let ks = ks_one_sample(&mut speeds, |x| normal_cdf((x - p.ln_mu) / p.ln_sigma));
    eprintln!(
        "K-S against the log-normal: D = {:.5}, p = {:.4}",
        ks.statistic, ks.p_value
    );
    assert_p_value("ordinary speeds", ks.p_value, ALPHA);
}

/// Test 2: isolated pulsars (every ordinary-mode neutron star, and the low-mode ones of single
/// stars, the electron captures of the 0.1 M☉ window): 5 ± 2% under 50 km/s on the sky, averaged
/// over isotropic viewing directions (Willcox et al. 2021). It pins the single-star window.
#[test]
#[ignore = "slow: 20,000 full tracks of 8–150 M☉"]
fn isolated_pulsars_are_rarely_slow_on_the_sky() {
    let share = observables().isolated_slow_share;
    eprintln!("isolated pulsars under 50 km/s on the sky: {share:.4}");
    assert!((0.03..=0.07).contains(&share), "{share}");
}

/// Test 3: the low mode is 20 ± 10% of neutron stars (Igoshev et al. 2021). It pins the 2–3 M☉
/// ramp and the provisional stripped share.
#[test]
#[ignore = "slow: 20,000 full tracks of 8–150 M☉"]
fn the_low_mode_is_a_fifth_of_neutron_stars() {
    let share = observables().low_mode_share;
    eprintln!("low-mode share of neutron stars: {share:.4}");
    assert!((0.10..=0.30).contains(&share), "{share}");
}

/// Test 4: retention, judging low-mode neutron stars on a pair recoil of a third of their kick:
/// at least a tenth under 50 km/s; the shares under 20 and 100 km/s are reported against the
/// brainstorm's 8–12% and 18–26%. It pins the low mode's σ and the ramp.
#[test]
#[ignore = "slow: 20,000 full tracks of 8–150 M☉"]
fn a_tenth_of_neutron_stars_is_retained_by_a_cluster() {
    let [under_20, under_50, under_100] = observables().retention;
    eprintln!("retained under 20 / 50 / 100 km/s: {under_20:.4} / {under_50:.4} / {under_100:.4}");
    assert!(under_50 >= 0.10, "{under_50}");
}

/// Test 5: the toy double neutron stars (Hills 1983; Brandt and Podsiadlowski 1995): more than
/// half of the pairs that survive have e < 0.3. It pins the ramp for companion-stripped stars.
#[test]
#[ignore = "slow: 20,000 full tracks of 8–150 M☉"]
fn most_double_neutron_stars_are_nearly_circular() {
    let o = observables();
    let (share, survivors, pairs) = o.double_neutron_stars;
    let (helium, helium_survivors) = o.double_neutron_stars_helium_core;
    eprintln!(
        "toy double neutron stars: {survivors} of {pairs} survive, {share:.4} with e < 0.3; with \
         the helium core as the exploding mass, {helium_survivors} survive, {helium:.4} with e < 0.3"
    );
    assert!(share > 0.5, "{share}");
}

/// Test 6: black holes. At least half are unkicked; among those under 12 M☉, 50–70% unkicked and
/// 12–26% above 100 km/s (Nagarajan and El-Badry 2025). It pins the black-hole factor of 0.75.
#[test]
#[ignore = "slow: 20,000 full tracks of 8–150 M☉"]
fn most_black_holes_are_unkicked() {
    let o = observables();
    let (light_unkicked, light_fast, light) = o.light_black_holes;
    eprintln!(
        "black holes: {} in all, {:.4} unkicked; {light} under 12 M_sun, {light_unkicked:.4} \
         unkicked and {light_fast:.4} above 100 km/s",
        o.counts.1, o.black_holes_unkicked
    );
    assert!(o.black_holes_unkicked >= 0.5, "{}", o.black_holes_unkicked);
    assert!((0.50..=0.70).contains(&light_unkicked), "{light_unkicked}");
    assert!((0.12..=0.26).contains(&light_fast), "{light_fast}");
}

/// The electron-capture windows (design note 12, one of the four defaults): at Z = 0.02 the
/// lowest initial mass that collapses by electron capture is some 0.1 M☉ below the iron cores for
/// a single star and 1 M☉ below them for one whose companion-stripped mark is set, and every star
/// between that and the iron cores collapses so. The track's winds make the initial masses about
/// 0.1 M☉ above the windows' own, which are in the mass `m_c_bagb` reads (ruling 45.1).
#[test]
fn the_stripped_window_is_ten_times_the_single_stars() {
    use hyperion_sim::rng::Mark;
    use hyperion_sim::stellar::Composition;
    use hyperion_sim::stellar::draws::{StarDraws, StarDrawsParts};
    use hyperion_sim::stellar::remnant::DeathKind;
    use hyperion_sim::stellar::system::StarModel;
    use hyperion_sim::units::{SolarMasses, Years};

    let death_of = |m: f64, stripped: bool| {
        let mark = Mark::from_word(if stripped { 0 } else { u64::MAX });
        let draws = StarDraws::from_parts(StarDrawsParts {
            stripped: mark,
            ..StarDrawsParts::MEDIAN
        });
        let star = StarModel::new(
            SolarMasses::new(m),
            Composition::SOLAR,
            draws,
            Years::new(1e6),
        )
        .expect("a valid star");
        star.death().expect("a star dies").kind()
    };
    let masses: Vec<f64> = (0..=32).map(|i| 7.0 + 0.05 * f64::from(i)).collect();
    let first_capture = |stripped: bool| {
        masses
            .iter()
            .copied()
            .find(|&m| death_of(m, stripped) == DeathKind::ElectronCapture)
            .expect("a window of electron captures")
    };
    let iron = masses
        .iter()
        .copied()
        .find(|&m| matches!(death_of(m, false), DeathKind::CoreCollapse { .. }))
        .expect("iron cores from about 8.3 M_sun");
    let (single, stripped) = (first_capture(false), first_capture(true));
    eprintln!("electron capture from {single} M_sun, stripped from {stripped}; iron from {iron}");
    // The scan's 0.05 M☉ step: the first iron core lies up to a step above the window's end.
    assert!(
        (0.05..=0.2).contains(&(iron - single)),
        "single: {single} to {iron}"
    );
    assert!(
        (0.95..=1.15).contains(&(iron - stripped)),
        "stripped: {stripped} to {iron}"
    );
    for &m in masses.iter().filter(|&&m| m >= stripped && m < iron) {
        assert_eq!(death_of(m, true), DeathKind::ElectronCapture, "{m} M_sun");
    }
}

// --- The kicks of three pinned systems ---

/// The universe the pinned kicks live in: the Milky Way fixture of `tests/stellar_system.rs`.
const SYSTEMS_SEED: u64 = 0x0600_0029_b000_5eed;

fn milky_way() -> Galaxy {
    Galaxy::from_params(Seed::new(SYSTEMS_SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral")
}

/// A pinned system: what its primary's kick is, the search's predicate, and the ID it found.
struct PinnedKick {
    name: &'static str,
    predicate: fn(&SystemStars) -> bool,
    id: u64,
}

fn primary_kick_is(stars: &SystemStars, kind: RemnantKind, mode: KickMode) -> bool {
    stars.primary().remnant().is_some_and(|r| r.kind() == kind)
        && stars.natal_kick().is_some_and(|k| k.mode() == mode)
}

/// The three pinned systems of layer E, found by [`find_kick`] near the solar circle.
const PINNED_KICKS: [PinnedKick; 3] = [
    PinnedKick {
        name: "an ordinary-mode neutron star",
        predicate: |s| primary_kick_is(s, RemnantKind::NeutronStar, KickMode::Ordinary),
        id: 0x8200_b2e0_0000_0000,
    },
    PinnedKick {
        name: "a low-mode neutron star",
        predicate: |s| primary_kick_is(s, RemnantKind::NeutronStar, KickMode::Low),
        id: 0x8200_b2e0_0000_000c,
    },
    PinnedKick {
        name: "a kicked black hole",
        predicate: |s| primary_kick_is(s, RemnantKind::BlackHole, KickMode::Ordinary),
        id: 0x8200_b2e0_0000_0005,
    },
];

/// The cell of layer E 26,000 ly from the centre along +y, in the plane, `step` cells along x.
fn solar_cell(step: i32) -> CellKey {
    let size = i32::try_from(Layer::E.cell_size_ly()).expect("a small cell size");
    CellKey::new(Layer::E, [step, 26_000 / size, 0]).expect("a cell of the grid")
}

/// The first record of layer E near the solar circle, in cell and index order, whose system
/// satisfies `predicate`, within `budget` records.
fn find_kick(galaxy: &Galaxy, predicate: fn(&SystemStars) -> bool, budget: usize) -> Option<u64> {
    let mut cell: Vec<SystemRecord> = Vec::new();
    let mut read = 0;
    for step in 0..10_000 {
        generate_cell(galaxy, solar_cell(step), &mut cell);
        for record in cell.drain(..) {
            if read == budget {
                return None;
            }
            read += 1;
            if predicate(&SystemStars::generate(galaxy, &record)) {
                return Some(record.id().raw());
            }
        }
    }
    None
}

/// The three pinned systems' primary kicks, new at generator version 11 with P06.T19: the remnant,
/// the death and its stripping, and the kick's mode, speed and direction.
#[test]
fn pinned_kicks_are_pinned() {
    let galaxy = milky_way();
    let mut w = GoldenWriter::new();
    w.header(hyperion_sim::GENERATOR_VERSION.get());
    for pinned in &PINNED_KICKS {
        let id = SystemId::from_raw(pinned.id).expect("a pinned ID is well formed");
        let record = resolve(&galaxy, id).expect("a pinned ID names a system");
        let stars = SystemStars::generate(&galaxy, &record);
        assert!(
            (pinned.predicate)(&stars),
            "{} ({:#018x}) is no longer one",
            pinned.name,
            pinned.id
        );
        let primary = stars.primary();
        let death = primary.death().expect("a dead primary has a death");
        let remnant = primary.remnant().expect("a dead primary has a remnant");
        let kick = stars.natal_kick().expect("the pinned primary has a kick");
        w.line("");
        w.u64_hex(pinned.name, record.id().raw());
        w.f64("initial mass", record.primary_initial_mass().value());
        w.line(&format!(
            "{:?}, {:?}, {:?}",
            death.kind(),
            death.progenitor().stripping(),
            remnant.kind()
        ));
        w.f64("co core", death.progenitor().co_core_mass().value());
        w.f64("remnant mass", remnant.mass().value());
        w.line(&format!("mode {:?}", kick.mode()));
        w.f64("speed m/s", kick.speed().value());
        for (axis, c) in ["x", "y", "z"]
            .into_iter()
            .zip(kick.direction().components())
        {
            w.f64(&format!("direction {axis}"), c);
        }
    }
    golden!("stellar/kicks", w.as_str());
}

/// The search, run again, finds the pinned IDs.
#[test]
#[ignore = "slow: searches layer E for the pinned kicks"]
fn the_search_reproduces_the_pinned_kick_ids() {
    let galaxy = milky_way();
    let moved: Vec<String> = PINNED_KICKS
        .iter()
        .filter_map(|pinned| {
            let found = find_kick(&galaxy, pinned.predicate, 5_000);
            (found != Some(pinned.id)).then(|| {
                let found = found.map_or_else(|| "none".to_owned(), |id| format!("{id:#018x}"));
                format!("{}: found {found}", pinned.name)
            })
        })
        .collect();
    assert!(moved.is_empty(), "{moved:#?}");
}

// --- Measurements against the kick law's sources (reported) ---

/// The kick distributions against their sources, printed for the report: the 3D and 1D speeds of
/// every neutron star against Hobbs et al. (2005, a Maxwellian of 1D σ = 265 km/s, 3D mean 400 ±
/// 40 km/s), Verbunt, Igoshev and Cator (2017, two Maxwellians of 75 and 316 km/s, 42% in the
/// first) and Igoshev (2020, 56 and 336 km/s, 20% in the first); and the share above the disc's
/// escape speed near the Sun, 550 km/s (Koppelman and Helmi 2021 find "closer to 550"; Piffl et
/// al. 2014 533; Monari et al. 2018 580), for the kick alone and added isotropically to a 230 km/s
/// circular speed.
#[test]
#[ignore = "slow: 20,000 full tracks of 8–150 M☉; a report, with loose bands"]
fn kick_distributions_against_their_sources() {
    let law = StandardKickLaw::default();
    let seed = Seed::new(OBSERVABLES_SEED);
    let mut speeds = Vec::new();
    let mut components = Vec::new();
    for i in 0..OBSERVABLE_STARS {
        let s = sampled_kick(&law, seed, i);
        if s.remnant().kind() != RemnantKind::NeutronStar {
            continue;
        }
        let kick = s.kick().expect("a neutron star has a kick");
        let v = KilometresPerSecond::from(kick.speed()).value();
        speeds.push(v);
        components.extend(kick.velocity().map(|c| c / 1e3));
    }
    let n = f64::from(u32::try_from(speeds.len()).unwrap());
    let mean_3d = speeds.iter().sum::<f64>() / n;
    let rms_1d = (components.iter().map(|c| c * c).sum::<f64>() / (3.0 * n)).sqrt();
    let count = |k: usize| f64::from(u32::try_from(k).expect("a small sample"));
    let share_above = |limit: f64| count(speeds.iter().filter(|&&v| v > limit).count()) / n;
    // Added to a circular speed of 230 km/s along x̂ at a direction uniform on the sphere: the
    // kick's own direction is isotropic, so its component along the rotation is v cos θ.
    let escape_with_rotation = |limit: f64| {
        let mut unbound = 0.0;
        for (v, c) in speeds.iter().zip(components.chunks(3)) {
            let along = c[0];
            let total2 = v * v + 230.0 * 230.0 + 2.0 * 230.0 * along;
            unbound += f64::from(u8::from(total2 > limit * limit));
        }
        unbound / n
    };
    eprintln!(
        "neutron stars: {} ; 3D mean {mean_3d:.1} km/s (Hobbs 400 ± 40; Maxwellian of 265: 423); \
         1D rms {rms_1d:.1} km/s (Hobbs 265)",
        speeds.len()
    );
    for limit in [50.0, 100.0, 265.0, 550.0, 570.0, 1_000.0] {
        eprintln!(
            "  share of 3D speeds above {limit} km/s: {:.4}; with the 230 km/s rotation: {:.4}",
            share_above(limit),
            escape_with_rotation(limit)
        );
    }
    // Two-Maxwellian mixtures: the share below 100 km/s they predict, against ours.
    let maxwell_below = |sigma: f64, v: f64| {
        let x = v / sigma;
        math::erf(x / core::f64::consts::SQRT_2)
            - (2.0 / core::f64::consts::PI).sqrt() * x * math::exp(-x * x / 2.0)
    };
    for (name, w, s1, s2) in [
        ("Verbunt et al. 2017", 0.42, 75.0, 316.0),
        ("Igoshev 2020", 0.20, 56.0, 336.0),
        ("Hobbs et al. 2005", 0.0, 1.0, 265.0),
    ] {
        let below = |v| w * maxwell_below(s1, v) + (1.0 - w) * maxwell_below(s2, v);
        eprintln!(
            "  {name}: below 50 / 100 / 550 km/s {:.4} / {:.4} / {:.4}",
            below(50.0),
            below(100.0),
            below(550.0)
        );
    }
    eprintln!(
        "  ours: below 50 / 100 / 550 km/s {:.4} / {:.4} / {:.4}",
        1.0 - share_above(50.0),
        1.0 - share_above(100.0),
        1.0 - share_above(550.0)
    );
    assert!((200.0..500.0).contains(&mean_3d), "{mean_3d}");
    assert!((0.05..0.30).contains(&share_above(550.0)));
}
