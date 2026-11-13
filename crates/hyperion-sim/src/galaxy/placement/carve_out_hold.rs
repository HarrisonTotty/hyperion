//! The tests of P09.T35's carve-out, the field half, on the record from generator version 27,
//! against the grid the tests build without it (`CarveOutRules::Unclaimed`; decision-galaxy-25-train
//! §1.5 and §1.6): the version's rules (D3) and `Galaxy::with_rules`'
//! refusal; layer D's ancient Type Ia; under every 26 hold, the layer-E records the classes claim,
//! each the record the grid places without the carve-out and resolving to no such system, every
//! other outcome the same (D2, pinned for the WebAssembly suite, D5); the same seed twice (D4); the
//! claim's wiring, the luminous blue variables' asked with or without a site; a star that dies at
//! +1,500 yr; the hook's cost under both arms; and the census C1, the classes' counts against the
//! grid's carve (research item 4 of the galaxy leans).
//!
//! The bit-identity of the version in force is held by the goldens, the carve-out's pre-pin
//! (`placement/carve_out`, D1) among them.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::golden::f64_digest;
use hyperion_testkit::order::assert_order_independent;
use hyperion_testkit::stats::{ALPHA, assert_p_value, normal_cdf};

use super::candidate::catalogue_claims;
use super::*;
use crate::GENERATOR_VERSION;
use crate::Seed;
use crate::coords::GalacticPosition;
use crate::galaxy::ages::FieldShareRules;
use crate::galaxy::catalogue_classes::core_collapse::{CoreCollapseProcess, recent_death_claims};
use crate::galaxy::catalogue_classes::lbv::{LbvProcess, horizon_window, lbv_claims, may_host};
use crate::galaxy::catalogue_classes::type_ia::ancient_share;
use crate::galaxy::catalogue_classes::{
    CatalogueCellKey, CatalogueClassCell, ClassId, ClassProcess,
};
use crate::galaxy::consts::LIGHT_YEARS_PER_YEAR_PER_KM_S;
use crate::galaxy::displaced::hold::field_only_rules;
use crate::galaxy::displaced::site::{
    CarveOutRules, ExplosionSite, explosion_site, recent_death_claim,
};
use crate::galaxy::imf::MassBand;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::query::{SubstellarRequest, epoch_velocity, expected_counts};
use crate::galaxy::rules::GalaxyRules;
use crate::galaxy::snr::SHELL_WINDOW_CAP;
use crate::galaxy::{Galaxy, Population};
use crate::id::Layer;
use crate::math;
use crate::planetary::testing::in_shares;
use crate::stellar::multiplicity::MultiplicityContext;
use crate::stellar::remnant::KickLawParams;
use crate::stellar::system::{ClockDeath, SystemStars};
use crate::time::{CLOCK_WINDOW_H, LIGHT_CROSSING_L, UniverseTime};
use crate::units::{LightYears, SolarMasses, Years};

/// The seed of the hold's Milky Way fixture, and of the pre-pin's.
const SEED: u64 = 0x0935_0000_0000_0001;

/// Every 26 hold but the carve-out: the grid keeps what the classes would claim.
fn unclaimed_rules() -> GalaxyRules {
    GalaxyRules {
        carve_out: CarveOutRules::Unclaimed,
        ..GalaxyRules::of_version(crate::GENERATOR_VERSION)
    }
}

/// The fixture under `rules`.
fn milky_way(rules: GalaxyRules) -> Galaxy {
    Galaxy::from_params_with_rules(Seed::new(SEED), GalaxyParams::milky_way_like(), rules)
        .expect("the Milky Way fixture builds")
}

/// The fixture under the generator version's rules, which claim.
fn version() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture builds")
    })
}

/// The fixture under every 26 hold but the carve-out.
fn unclaimed() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| milky_way(unclaimed_rules()))
}

/// The fixture under every 26 hold: the classes claim their hosts.
fn claimed() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| milky_way(GalaxyRules::of_version(crate::GENERATOR_VERSION)))
}

/// `count` layer-E cells along each of `galaxy`'s arm ridges, in the plane, at each radius of
/// `radii` ly: the young field's richest cells, where the recently dead are.
fn ridge_cells(galaxy: &Galaxy, radii: &[f64], count: i32) -> Vec<CellKey> {
    let arms = galaxy.fields().arms();
    let mut keys = Vec::new();
    for &r in radii {
        for arm in 0..4 {
            let (sin, cos) = math::sin_cos(arms.ridge_azimuth(r, arm));
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

/// The sample of the hold's records: layer-E cells on the ridges at 20,000 to 28,000 ly.
fn sample_cells() -> Vec<CellKey> {
    ridge_cells(claimed(), &[20_000.0, 24_000.0, 26_000.0, 28_000.0], 128)
}

/// Which class claimed a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Class {
    CoreCollapse,
    LuminousBlueVariable,
}

/// The class that claims `record` in `galaxy`, by the claims the carve-out asks in its order.
fn class_of(galaxy: &Galaxy, record: &SystemRecord) -> Option<Class> {
    if explosion_site(galaxy, record).is_some_and(|site| recent_death_claims(galaxy, &site, record))
    {
        Some(Class::CoreCollapse)
    } else if lbv_claims(galaxy, record) {
        Some(Class::LuminousBlueVariable)
    } else {
        None
    }
}

/// The carve-out over `keys`: every candidate of each cell evaluated without the carve-out
/// (`off`) and with it (`on`), the same but where a class claims the record `off` places; each
/// claimed record, with its class, in cell and index order.
fn carved(off: &Galaxy, on: &Galaxy, keys: &[CellKey]) -> Vec<(SystemRecord, Class)> {
    let mut claimed = Vec::new();
    for &key in keys {
        assert_eq!(candidate_count(off, key), candidate_count(on, key));
        for index in 0..candidate_count(off, key) {
            match (
                evaluate_candidate(off, key, index),
                evaluate_candidate(on, key, index),
            ) {
                (CandidateOutcome::Accepted(record), CandidateOutcome::ClaimedByCatalogue) => {
                    let class = class_of(on, &record).expect("a claimed record has its class");
                    claimed.push((record, class));
                }
                (before, after) => assert_eq!(before, after, "{key:?} candidate {index}"),
            }
        }
    }
    claimed
}

/// D3: a galaxy built the usual way carries the version's carve-out; one built under the
/// version's rules explicitly is the same galaxy; and one built under every 26 hold claims, its
/// layer D keeping each component's `1 − ancient_share` of its population, bit for bit. Plan 08's
/// displaced classes, also in every 26 hold, take their share of layer D's budget first (P08.T12),
/// so the field keeps `1 − ancient_share ÷ stay` of what stays, `stay` the class table's budget
/// stay share.
#[test]
fn a_galaxy_carries_the_version_s_carve_out() {
    let explicit = Galaxy::from_params_with_rules(
        Seed::new(SEED),
        GalaxyParams::milky_way_like(),
        GalaxyRules::of_version(GENERATOR_VERSION),
    )
    .expect("the Milky Way fixture builds");
    assert_eq!(&explicit, version());
    assert_eq!(version().rules().carve_out, CarveOutRules::Claimed);
    assert!(recent_death_claim(version().rules().carve_out).is_some());
    assert!(recent_death_claim(CarveOutRules::Unclaimed).is_none());
    assert_eq!(
        claimed().rules(),
        GalaxyRules::of_version(crate::GENERATOR_VERSION)
    );
    assert_ne!(claimed(), unclaimed());
    let fields = claimed().fields();
    for (id, component) in fields.component_ids().zip(fields.components()) {
        let kept = claimed()
            .layer_d_kept(id)
            .expect("every 26 hold installs layer D's factors");
        let stay = claimed()
            .class_table()
            .expect("every 26 hold installs the displaced classes")
            .budget_stay_share(MassBand::D, id);
        let budget = 1.0 - ancient_share(claimed(), component.population());
        let composed = if stay > 0.0 {
            (1.0 - (1.0 - budget) / stay).max(0.0)
        } else {
            budget
        };
        assert_same_bits(kept, composed);
        assert!(kept > 0.0 && kept <= 1.0, "{id:?}: {kept}");
        assert_eq!(unclaimed().layer_d_kept(id), None);
    }
}

/// `Galaxy::with_rules` refuses a change of the carve-out, which the galaxy reads when it is built.
#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "the carve-out is read when a galaxy is built")]
fn with_rules_refuses_to_change_the_carve_out() {
    let _ = unclaimed()
        .clone()
        .with_rules(GalaxyRules::of_version(crate::GENERATOR_VERSION));
}

/// Layer D gives up its ancient Type Ia: every population's factor but the young disc's falls
/// below 1 by a few per cent; the expected counts of layer D in a sphere at the Sun-like point
/// fall between the smallest and largest factor, the other layers' the same; and in layer-D cells
/// there, the records kept are the budget's, a subset bit for bit and in order, and fewer.
#[test]
fn layer_d_gives_up_its_ancient_type_ia() {
    let (off, on) = (unclaimed(), claimed());
    let fields = on.fields();
    let kept: Vec<f64> = fields
        .component_ids()
        .map(|id| on.layer_d_kept(id).unwrap())
        .collect();
    let old = fields
        .component_ids()
        .find(|&id| fields.component(id).population() == Population::ThickDisc)
        .unwrap();
    let (lo, hi) = kept
        .iter()
        .fold((1.0_f64, 0.0_f64), |(lo, hi), &k| (lo.min(k), hi.max(k)));
    eprintln!("layer D keeps {kept:?}");
    assert!(on.layer_d_kept(old).unwrap() < 1.0);
    for (id, component) in fields.component_ids().zip(fields.components()) {
        let k = on.layer_d_kept(id).unwrap();
        if component.population() != Population::YoungThinDisc {
            assert!(k < 1.0 && k > 0.9, "{:?}: {k}", component.population());
        }
    }
    let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap();
    let radius = LightYears::new(100.0);
    let none = SubstellarRequest::None;
    let (budget_counts, field_counts) = (
        expected_counts(off, &sun, radius, none),
        expected_counts(on, &sun, radius, none),
    );
    let (budget_d, field_d) = (budget_counts.get(Layer::D), field_counts.get(Layer::D));
    assert!(
        field_d < budget_d && field_d >= lo * budget_d && field_d <= hi * budget_d,
        "{field_d} against {budget_d}"
    );
    for layer in [Layer::A, Layer::B, Layer::C, Layer::E] {
        assert_same_bits(field_counts.get(layer), budget_counts.get(layer));
    }
    let [x, y, z] = CellKey::containing(Layer::D, &sun)
        .unwrap()
        .gen_cell()
        .to_array();
    let (mut budget, mut field) = (0, 0);
    for i in 0..8 {
        let key = CellKey::new(Layer::D, [x + i, y, z]).unwrap();
        let (mut all, mut survivors) = (Vec::new(), Vec::new());
        generate_cell(off, key, &mut all);
        generate_cell(on, key, &mut survivors);
        let mut rest = all.iter();
        for record in &survivors {
            assert!(rest.any(|other| other == record), "{:?}", record.id());
        }
        budget += all.len();
        field += survivors.len();
    }
    eprintln!("layer D at the Sun-like point: {field} of {budget} kept");
    assert!(field < budget, "{field} of {budget}");
}

/// The digest (`f64_digest`) of the claimed records of D2's sample under every 26 hold: each
/// record's ID (high, low 32 bits), mass and age, and its class (0 for a core collapse, 1 for a
/// luminous blue variable), in cell and index order. Blessed natively on x86-64; the wasip1 suite
/// compares it. Re-pinned by name when plan 08's displaced classes joined every 26 hold
/// (P08.T12), whose marks redraw layer E's records; re-pinned at generator versions 25 and 26
/// (under 26's with the retargets, the carve-out at 27), and at 27 (it was `0x69d8_3742_c8f4_ae5e`),
/// whose galaxy the fixture is, every 27 hold's, and every 26 hold's with its retargets until the
/// bump; and at 31 (it was `0x9440_c055_5711_2d4b`), whose carve-out residuals move layer E's
/// recently dead records' ages.
const CLAIMED_DIGEST: u64 = 0x1f3f_9796_3c8e_61c3;

/// D2 under every 26 hold: in the sample's layer-E cells on the arm ridges, the classes claim
/// some of the records the grid places without the carve-out, each the core collapses' or the
/// luminous blue variables', and every other candidate's outcome is the same, bit for bit; each
/// claimed record resolves to no such system with the carve-out and to itself without it. Named
/// `pinned` for the WebAssembly suite's filter (D5): the arm puts the stellar stage on the
/// placement path, so the claimed records' digest is pinned, as blessed natively.
#[test]
fn the_classes_claim_their_hosts_from_the_grid_pinned() {
    let (off, on) = (unclaimed(), claimed());
    let keys = sample_cells();
    let claimed = carved(off, on, &keys);
    let mut values = Vec::new();
    let mut by_class: BTreeMap<Class, usize> = BTreeMap::new();
    for (record, class) in &claimed {
        assert_eq!(
            resolve(on, record.id()),
            Err(ResolveSystemError::NoSuchSystem)
        );
        assert_eq!(resolve(off, record.id()), Ok(*record));
        assert!(!catalogue_claims(off, record, None));
        let raw = record.id().raw();
        values.push(f64::from(u32::try_from(raw >> 32).unwrap()));
        values.push(f64::from(u32::try_from(raw & 0xffff_ffff).unwrap()));
        values.push(record.primary_initial_mass().value());
        values.push(record.age_at_epoch().value());
        values.push(f64::from(u8::from(*class == Class::LuminousBlueVariable)));
        *by_class.entry(*class).or_default() += 1;
    }
    let digest = f64_digest(&values);
    eprintln!(
        "{} cells: {} claimed, {by_class:?}, digest {digest:#018x}; the first cells {:?}",
        keys.len(),
        claimed.len(),
        claimed
            .iter()
            .map(|(r, _)| CellKey::containing(Layer::E, r.epoch_position()).unwrap())
            .take(3)
            .collect::<Vec<_>>()
    );
    assert!(by_class.get(&Class::CoreCollapse).copied().unwrap_or(0) > 0);
    assert_eq!(digest, CLAIMED_DIGEST, "{digest:#018x}");
}

/// D4: the same seed twice under every 26 hold claims the same records.
#[test]
fn the_same_seed_twice_claims_the_same_records() {
    let again = milky_way(GalaxyRules::of_version(crate::GENERATOR_VERSION));
    assert_eq!(&again, claimed());
    let keys = ridge_cells(claimed(), &[24_000.0], 64);
    for key in keys {
        let (mut a, mut b) = (Vec::new(), Vec::new());
        generate_cell(&again, key, &mut a);
        generate_cell(claimed(), key, &mut b);
        assert_eq!(a, b, "{key:?}");
    }
}

/// `record` re-aged to `age` ly and re-massed to `mass`, its ID, position and component kept.
fn reshaped(record: &SystemRecord, mass: f64, age: f64) -> SystemRecord {
    SystemRecord::from_parts(
        record.id(),
        *record.epoch_position(),
        record.origin(),
        record.population(),
        SolarMasses::new(mass),
        Years::new(age),
    )
}

/// The years from the epoch to the clock time `t`.
fn years_after_epoch(t: UniverseTime) -> f64 {
    t.since_epoch().as_julian_years_f64()
}

/// The claim's wiring under every 26 hold, on young-disc records re-massed and re-aged: a record
/// whose primary is now a luminous blue variable and dies after +H, so has no site, is claimed,
/// the site's gate not holding the class's claim back; one that died recently with a shell is
/// claimed, as the core collapses' entry; the claim is `class_of`'s on every record; and without
/// the carve-out none is claimed.
#[test]
fn the_carve_out_asks_the_luminous_blue_variables_with_or_without_a_site() {
    let (off, on) = (unclaimed(), claimed());
    let mut young = Vec::new();
    for key in ridge_cells(on, &[26_000.0], 32) {
        let mut cell = Vec::new();
        generate_cell(off, key, &mut cell);
        young.extend(
            cell.into_iter()
                .filter(|r| r.population() == Population::YoungThinDisc),
        );
    }
    assert!(young.len() >= 8, "{} young-disc records", young.len());
    let (mut hosts, mut deaths, mut checked) = (0, 0, 0);
    // Probed long dead, so that the track is built over the whole life: a living star's ends at
    // +H, before its window.
    let probe_age = 3.0e7;
    for (n, base) in young.iter().take(8).enumerate() {
        let mass = [40.0, 50.0, 60.0, 80.0][n % 4];
        let probe = reshaped(base, mass, probe_age);
        let stars = SystemStars::generate_in(on, &probe, MultiplicityContext::ForcedSingle);
        let Some((window_start, _)) = stars.lbv_window() else {
            continue;
        };
        // Into its window by a thousand years, so the window holds the epoch.
        let host = reshaped(
            base,
            mass,
            probe_age + years_after_epoch(window_start) + 1.0e3,
        );
        if may_host(host.primary_initial_mass(), host.age_at_epoch())
            && explosion_site(on, &host).is_none()
            && horizon_window(&SystemStars::generate_in(
                on,
                &host,
                MultiplicityContext::ForcedSingle,
            ))
            .is_some()
        {
            assert!(catalogue_claims(on, &host, None), "{:?}", host.id());
            assert_eq!(class_of(on, &host), Some(Class::LuminousBlueVariable));
            hosts += 1;
        }
        if let ClockDeath::At(death, _) = stars.death_time() {
            // Dead fifty thousand years.
            let dead = reshaped(base, mass, probe_age + years_after_epoch(death) + 5.0e4);
            assert!(explosion_site(on, &dead).is_some());
            if class_of(on, &dead) == Some(Class::CoreCollapse) {
                deaths += 1;
            }
            assert_eq!(
                catalogue_claims(on, &dead, None),
                class_of(on, &dead).is_some()
            );
            assert!(!catalogue_claims(off, &dead, None));
        }
        assert!(!catalogue_claims(off, &host, None));
        checked += 1;
    }
    eprintln!("{checked} records: {hosts} luminous blue variables with no site, {deaths} deaths");
    assert!(
        hosts > 0,
        "no luminous blue variable without a site among {checked}"
    );
    assert!(deaths > 0, "no recent death claimed among {checked}");
    for record in young.iter().take(64) {
        assert_eq!(
            catalogue_claims(on, record, None),
            class_of(on, record).is_some()
        );
    }
}

/// The plan's: a grid star due to die at +1,500 yr, past +H, is still a grid star under every 26
/// hold, the carve-out claiming nothing it has no site for; it dies in place, at its epoch
/// position moved along its velocity, and has a shell when evaluated then: its death makes the
/// marks of a supernova there.
#[test]
fn a_star_dying_at_plus_1_500_years_is_a_grid_star() {
    let on = claimed();
    let (mut records, mut cell) = (Vec::new(), Vec::new());
    for key in ridge_cells(on, &[26_000.0], 16) {
        generate_cell(on, key, &mut cell);
        records.extend_from_slice(&cell);
    }
    let base = records
        .iter()
        .find(|r| r.population() == Population::YoungThinDisc)
        .expect("a young-disc record on the ridge");
    let mass = 20.0;
    let probe = reshaped(base, mass, 1.0e6);
    let stars = SystemStars::generate_in(on, &probe, MultiplicityContext::ForcedSingle);
    let ClockDeath::At(death, _) = stars.death_time() else {
        panic!("a 20 M☉ star dies on the clock");
    };
    let record = reshaped(base, mass, 1.0e6 + years_after_epoch(death) - 1_500.0);
    let stars = SystemStars::generate_in(on, &record, MultiplicityContext::ForcedSingle);
    let ClockDeath::At(death, _) = stars.death_time() else {
        panic!("a 20 M☉ star dies on the clock");
    };
    let after = years_after_epoch(death);
    assert!((after - 1_500.0).abs() < 50.0, "{after}");
    assert!(explosion_site(on, &record).is_none());
    assert!(!catalogue_claims(on, &record, None));
    let velocity = epoch_velocity(on, &record);
    let position = record
        .epoch_position()
        .translated(velocity.displacement_over(crate::units::Seconds::new(
            death.since_epoch().as_seconds_f64(),
        )))
        .unwrap();
    let site = ExplosionSite::new(position, death);
    let marks =
        crate::galaxy::catalogue_classes::supernova::DeathMarks::of(on, &stars, &site, velocity);
    assert!(marks.is_some(), "its death makes a supernova's shell");
}

/// The hook's cost, the bench of P09.T35: microseconds a layer-E cell generated at the Sun-like
/// point, on the young arm ridge at the solar circle and at the centre, without the carve-out
/// and with it, over the same cells, best of three.
#[test]
#[ignore = "bench: the carve-out's cost per layer-E cell, under both arms"]
fn bench_the_carve_out_hook() {
    let (off, on) = (unclaimed(), claimed());
    let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap();
    let centre = GalacticPosition::from_light_years([64.0, 64.0, 0.0]).unwrap();
    let around = |at: &GalacticPosition, n: i32| -> Vec<CellKey> {
        let [x, y, z] = CellKey::containing(Layer::E, at)
            .unwrap()
            .gen_cell()
            .to_array();
        (0..n)
            .map(|i| CellKey::new(Layer::E, [x + i % 8, y + i / 8, z]).unwrap())
            .collect()
    };
    let places = [
        ("the Sun-like point", around(&sun, 64)),
        ("the young arm ridge", ridge_cells(on, &[26_000.0], 64)),
        ("the centre", around(&centre, 4)),
    ];
    for (name, keys) in places {
        let time = |galaxy: &Galaxy| {
            (0..3)
                .map(|_| {
                    let start = std::time::Instant::now();
                    let mut cell = Vec::new();
                    for &key in &keys {
                        generate_cell(galaxy, key, &mut cell);
                    }
                    start.elapsed().as_secs_f64() * 1e6
                        / f64::from(u32::try_from(keys.len()).unwrap())
                })
                .fold(f64::INFINITY, f64::min)
        };
        let (before, after) = (time(off), time(on));
        eprintln!(
            "{name}, {} layer-E cells: {before:.1} µs a cell without the carve-out, {after:.1} with \
             it",
            keys.len()
        );
    }
}

/// The young field's expected layer-E systems in `key`: its components' layer-E share times their
/// density bound over the cell, the young thin disc's and the nuclear disc's, whose stars alone are
/// young enough to die recently or to be luminous blue variables.
fn young_mean(galaxy: &Galaxy, key: CellKey) -> f64 {
    let fields = galaxy.fields();
    let cube = key.cell_box();
    fields
        .component_ids()
        .filter(|&id| {
            matches!(
                fields.component(id).population(),
                Population::YoungThinDisc | Population::NuclearDisc
            )
        })
        .fold(0.0, |sum, id| {
            sum + galaxy.field_component_share(MassBand::E, id) * fields.component_bound(id, &cube)
        })
        * key.volume_ly3()
}

/// The young field's least expected systems in a layer-E cell that the census walks: 10⁻⁶, so that
/// the cells it skips hold together well under one claim.
const NEGLIGIBLE_YOUNG: f64 = 1e-6;

/// The layer-E columns, as [x, y] cell indices within 65,536 ly of the axis, whose cell in the
/// plane holds the young field, in sixteen shares, in x order; on wasm32-wasip1, which has no
/// threads, those within 2,048 ly of the Sun-like point alone.
fn layer_e_columns(galaxy: &Galaxy) -> Vec<[i32; 2]> {
    let reach = 65_536 / i32::try_from(Layer::E.cell_size_ly()).unwrap();
    let xs: Vec<i32> = (-reach..reach).collect();
    let shares = 16;
    let mut columns: Vec<[i32; 2]> = in_shares(shares, shares, |share| {
        let mut out = Vec::new();
        for &x in xs.iter().skip(share).step_by(shares) {
            for y in -reach..reach {
                if x * x + y * y > reach * reach {
                    continue;
                }
                if cfg!(target_family = "wasm") && (x.abs() > 16 || (y - 203).abs() > 16) {
                    continue;
                }
                let key = CellKey::new(Layer::E, [x, y, 0]).unwrap();
                if young_mean(galaxy, key) >= NEGLIGIBLE_YOUNG {
                    out.push([x, y]);
                }
            }
        }
        out
    })
    .into_iter()
    .flatten()
    .collect();
    columns.sort_unstable();
    columns
}

/// The remnants' reach in layer-E cells, 112 (14,336 ly): the farthest a natal kick carries a
/// remnant from its explosion site by the epoch, [`remnant_reach_ly`], rounded up.
///
/// It bounds the ballistic bins' remnants, whose vertical speed is their height over the time
/// since the kick, held to the kick's speed (`kinematics::displaced`). Past those bins a remnant's
/// place is its class's form and its velocity the class's Gaussian, which nothing here bounds: a
/// walk of every 31st of the census's columns to the cube's edge under the budget (2026-11-06)
/// carved 38 of its 523 core collapses past the young field's stop and none 112 cells or more past
/// it, the farthest 102, a remnant of the nuclear disc's fastest old class.
const REMNANT_REACH_CELLS: i32 = 112;

/// The farthest a natal kick carries a remnant from its explosion site by the epoch, ly: the kick
/// law's truncation, 1,000 km/s ([`KickLawParams::max_speed_km_s`]; Disberg and Mandel 2025, eq. 5,
/// ruling 96.2), over the longest time from a death in the core collapses' interval to the epoch,
/// `W_cap` + L + H = 4,263,144 Julian years (the interval's start, `displaced::site`), about
/// 14,220 ly. The kick's rank clamp, 0.999, holds its speed under 990 km/s, so the cells' 14,336 ly
/// leave about 18 km/s for the progenitor's own motion; and in the fixture (`v_ref` 224 km/s) the
/// kick loop's floor of the fastest bin, 2.8 `v_ref`, is 627 km/s, under it.
fn remnant_reach_ly() -> f64 {
    let years = SHELL_WINDOW_CAP.value()
        + LIGHT_CROSSING_L.as_julian_years_f64()
        + CLOCK_WINDOW_H.as_julian_years_f64();
    KickLawParams::default().max_speed_km_s * years * LIGHT_YEARS_PER_YEAR_PER_KM_S
}

/// The census's walk past the young field's stop is the natal kick's reach rounded up to whole
/// layer-E cells, about 14,220 ly in 112: it covers the reach and one cell fewer would not, so a
/// change of the kick law's truncation or of the interval fails here, not in the census, which
/// is slow.
#[test]
fn the_census_walk_covers_a_natal_kick_s_reach() {
    let reach = remnant_reach_ly();
    let cell = f64::from(Layer::E.cell_size_ly());
    assert!((reach - 14_220.3).abs() < 0.1, "{reach} ly");
    assert!(f64::from(REMNANT_REACH_CELLS) * cell >= reach, "{reach} ly");
    assert!(
        f64::from(REMNANT_REACH_CELLS - 1) * cell < reach,
        "{reach} ly"
    );
}

/// The census's columns per chunk: the unit its checkpoint records and a resumed run skips.
const CENSUS_CHUNK_COLUMNS: usize = 512;

/// The environment variable naming the census's checkpoint file: a header naming the run, then one
/// line per finished chunk of [`CENSUS_CHUNK_COLUMNS`] columns, appended as each finishes, which a
/// run whose header is the same reads first and whose chunks it skips (LANE-RULES v7, "Long
/// runs"); a run of another header refuses the file. Unset, nothing is written or read.
const CENSUS_CHECKPOINT: &str = "HYPERION_CENSUS_CHECKPOINT";

/// What one chunk of the census's columns carved: each claimed record's ID with its class, in the
/// order the walk met them, and the core collapses carved past the young field's stop by the cells
/// past it. A pure function of the chunk's columns, so a chunk read back from the checkpoint is
/// the one a run computes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct CensusChunk {
    carved: Vec<(u64, Class)>,
    past: BTreeMap<i32, usize>,
}

impl CensusChunk {
    /// The chunk's checkpoint line: `chunk <index> past <cells>:<deaths>,… carved <c|l><id hex>,…
    /// end`, `-` for an empty list.
    fn line(&self, index: usize) -> String {
        let past: Vec<String> = self.past.iter().map(|(k, n)| format!("{k}:{n}")).collect();
        let carved: Vec<String> = self
            .carved
            .iter()
            .map(|(id, class)| {
                let tag = match class {
                    Class::CoreCollapse => 'c',
                    Class::LuminousBlueVariable => 'l',
                };
                format!("{tag}{id:x}")
            })
            .collect();
        let list = |v: &[String]| {
            if v.is_empty() {
                "-".to_owned()
            } else {
                v.join(",")
            }
        };
        format!(
            "chunk {index} past {} carved {} end",
            list(&past),
            list(&carved)
        )
    }

    /// The chunk of a checkpoint line, with its index; `None` for a line cut short.
    fn parse(line: &str) -> Option<(usize, Self)> {
        let words: Vec<&str> = line.split(' ').collect();
        let ["chunk", index, "past", past, "carved", carved, "end"] = words[..] else {
            return None;
        };
        let mut chunk = Self::default();
        if past != "-" {
            for entry in past.split(',') {
                let (k, n) = entry.split_once(':')?;
                chunk.past.insert(k.parse().ok()?, n.parse().ok()?);
            }
        }
        if carved != "-" {
            for entry in carved.split(',') {
                let class = match entry.as_bytes().first()? {
                    b'c' => Class::CoreCollapse,
                    b'l' => Class::LuminousBlueVariable,
                    _ => return None,
                };
                chunk
                    .carved
                    .push((u64::from_str_radix(&entry[1..], 16).ok()?, class));
            }
        }
        Some((index.parse().ok()?, chunk))
    }
}

/// A checkpoint line is read back as the chunk it was written from, and a line cut short is
/// refused.
#[test]
fn a_census_checkpoint_line_reads_back() {
    let chunk = CensusChunk {
        carved: vec![
            (0x0123_4567_89ab_cdef, Class::CoreCollapse),
            (7, Class::LuminousBlueVariable),
        ],
        past: BTreeMap::from([(0, 3), (12, 1)]),
    };
    let line = chunk.line(41);
    assert_eq!(CensusChunk::parse(&line), Some((41, chunk)));
    assert_eq!(
        CensusChunk::parse(&CensusChunk::default().line(0)),
        Some((0, CensusChunk::default()))
    );
    assert_eq!(CensusChunk::parse(&line[..line.len() - 4]), None);
}

/// The carve-out over the columns `columns` of a galaxy's young field: every layer-E cell's
/// claimed records, in ID order, a column at a time from the plane outwards until the young
/// field's mean is negligible and on for the remnants' reach; and the core collapses carved past
/// that stop, by the cells past it. The grid places a kicked remnant at its epoch position, away
/// from the site where the class lists its death, so the carve reaches past the young field's stop
/// ([`REMNANT_REACH_CELLS`] gives the measured tail).
fn carved_columns(off: &Galaxy, on: &Galaxy, columns: &[[i32; 2]]) -> CensusChunk {
    let mut chunk = CensusChunk::default();
    for &[x, y] in columns {
        for side in [1, -1] {
            let mut stop = None;
            for k in 0.. {
                let z = if side > 0 { k } else { -1 - k };
                let Ok(key) = CellKey::new(Layer::E, [x, y, z]) else {
                    break;
                };
                if stop.is_none() && young_mean(on, key) < NEGLIGIBLE_YOUNG {
                    stop = Some(k);
                }
                if stop.is_some_and(|s| k >= s + REMNANT_REACH_CELLS) {
                    break;
                }
                let cell = carved(off, on, &[key]);
                if let Some(s) = stop {
                    let deaths = cell
                        .iter()
                        .filter(|(_, class)| *class == Class::CoreCollapse)
                        .count();
                    if deaths > 0 {
                        *chunk.past.entry(k - s).or_default() += deaths;
                    }
                }
                chunk.carved.extend(
                    cell.iter()
                        .map(|(record, class)| (record.id().raw(), *class)),
                );
            }
        }
    }
    chunk
}

/// The carve-out over a galaxy's young field ([`carved_columns`]) in chunks of
/// [`CENSUS_CHUNK_COLUMNS`] columns, taken in turn by sixteen threads: each claimed record's ID
/// with its class, in ID order, and the core collapses carved past the young field's stop by the
/// cells past it. Under [`CENSUS_CHECKPOINT`] each finished chunk is appended to the checkpoint,
/// and the chunks it already holds are read back, not walked again; each is a pure function of its
/// columns, so the result is the uninterrupted run's bit for bit.
fn carved_galaxy(off: &Galaxy, on: &Galaxy) -> (Vec<(u64, Class)>, BTreeMap<i32, usize>) {
    use std::io::Write as _;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let columns = layer_e_columns(on);
    let chunks: Vec<&[[i32; 2]]> = columns.chunks(CENSUS_CHUNK_COLUMNS).collect();
    let path = std::env::var_os(CENSUS_CHECKPOINT).map(std::path::PathBuf::from);
    // The run's header: the version, both galaxies' seeds and rules, the chunks' size and the
    // columns' count and digest, so that a checkpoint is read back only by its own run.
    let digest = f64_digest(
        &columns
            .iter()
            .flat_map(|&[x, y]| [f64::from(x), f64::from(y)])
            .collect::<Vec<_>>(),
    );
    let header = format!(
        "census v{} seed {:#x} off {:?} on {:?} chunk {CENSUS_CHUNK_COLUMNS} columns {} {digest:#018x}",
        GENERATOR_VERSION.get(),
        on.seed().get(),
        off.rules(),
        on.rules(),
        columns.len()
    );
    let mut done: BTreeMap<usize, CensusChunk> = BTreeMap::new();
    if let Some(path) = &path
        && let Ok(text) = std::fs::read_to_string(path)
        && !text.is_empty()
    {
        let mut lines = text.lines();
        assert_eq!(
            lines.next(),
            Some(header.as_str()),
            "the checkpoint {} is another run's",
            path.display()
        );
        for (index, chunk) in lines.filter_map(CensusChunk::parse) {
            done.insert(index, chunk);
        }
    }
    let todo: Vec<usize> = (0..chunks.len())
        .filter(|i| !done.contains_key(i))
        .collect();
    eprintln!(
        "census: {} columns in {} chunks, {} from the checkpoint, {} to walk",
        columns.len(),
        chunks.len(),
        done.len(),
        todo.len()
    );
    let file = path.as_ref().map(|path| {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .expect("the census's checkpoint opens");
        if done.is_empty() && file.metadata().is_ok_and(|m| m.len() == 0) {
            writeln!(file, "{header}").expect("the checkpoint writes");
        }
        Mutex::new(file)
    });
    let next = AtomicUsize::new(0);
    let walked = Mutex::new(BTreeMap::new());
    let started = std::time::Instant::now();
    in_shares(16, 16, |_| {
        loop {
            let n = next.fetch_add(1, Ordering::Relaxed);
            let Some(&index) = todo.get(n) else {
                break;
            };
            let chunk = carved_columns(off, on, chunks[index]);
            if let Some(file) = &file {
                let mut file = file.lock().expect("no writer panicked");
                writeln!(file, "{}", chunk.line(index)).expect("the checkpoint writes");
                file.flush().expect("the checkpoint flushes");
            }
            eprintln!(
                "census: chunk {index} done ({} of {} walked) at {:.0} s",
                n + 1,
                todo.len(),
                started.elapsed().as_secs_f64()
            );
            walked
                .lock()
                .expect("no walker panicked")
                .insert(index, chunk);
        }
    });
    done.extend(walked.into_inner().expect("no walker panicked"));
    assert_eq!(done.len(), chunks.len(), "every chunk walked or read back");
    let mut all = Vec::new();
    let mut past: BTreeMap<i32, usize> = BTreeMap::new();
    for chunk in done.into_values() {
        all.extend(chunk.carved);
        for (cells, deaths) in chunk.past {
            *past.entry(cells).or_default() += deaths;
        }
    }
    all.sort_by_key(|&(id, _)| id);
    (all, past)
}

/// A class's entries over the galaxy: every class cell whose mean is not negligible, a column at a
/// time from the plane outwards (the bound never rises away from the plane), in sixteen shares.
fn class_entries<P: ClassProcess + Sync>(galaxy: &Galaxy, process: &P, class: ClassId) -> usize {
    let edge = 1_i32 << class.cell_log2_ly().expect("a gridded class");
    let reach = 65_536 / edge;
    let column = |x: i32, y: i32| {
        let mut entries = 0;
        for side in [1, -1] {
            for k in 0.. {
                let z = if side > 0 { k } else { -1 - k };
                let Ok(key) = CatalogueCellKey::new(class, [x, y, z]) else {
                    break;
                };
                if process.bound(galaxy, &key).value() * key.volume_ly3() < 1e-9 {
                    break;
                }
                entries += CatalogueClassCell::generate(galaxy, process, key)
                    .entries()
                    .len();
            }
        }
        entries
    };
    let shares = 16;
    in_shares(shares, shares, |share| {
        let mut entries = 0;
        for x in (-reach..reach).skip(share).step_by(shares) {
            for y in -reach..reach {
                entries += column(x, y);
            }
        }
        entries
    })
    .into_iter()
    .sum()
}

/// Every 26 hold at the budget, the field keeping its whole budget (`FieldShareRules::None`), with
/// the carve-out `carve_out`.
fn budget_rules(carve_out: CarveOutRules) -> GalaxyRules {
    GalaxyRules {
        field_share: FieldShareRules::None,
        carve_out,
        ..GalaxyRules::of_version(crate::GENERATOR_VERSION)
    }
}

/// C1, the carve-out's census under every 26 hold at the budget, the carve-out on against off
/// (decision-galaxy-25-train §1.6), over the fixture's young field in layer E, with its kinematic
/// tables: every claimed record listed by class, with its ID; each one the grid places without the
/// carve-out, every other outcome the same; and, research item 4 of the galaxy leans, the classes'
/// entries against the grid's carve over the galaxy: two Poisson counts of one intensity, the core
/// collapses' and the luminous blue variables', each difference against the two counts' summed
/// variance, both printed before either is asserted. On wasm32-wasip1, the Sun's columns alone and
/// no comparison.
///
/// Both arms take 31's carve-out residuals (research-leans-49), which closed the residuals the
/// census measured under 30's rules
/// (core collapses 16,228 against 16,888, z −3.63; luminous blue variables 279 against 168, z
/// +5.25). The walk takes hours: under `HYPERION_CENSUS_CHECKPOINT` it records each finished chunk
/// of columns and resumes past them ([`carved_galaxy`]).
///
/// Both arms are at the budget (`FieldShareRules::None`), as
/// `core_collapses_over_seeds_under_every_26_hold` builds its budget. The field's share φ
/// (P09.T2.c) is a thinning that the grid's displaced picks do not take: it thins only a field
/// pick, and a displaced class's pick is at its whole budget (`settle`). The class holds its field
/// term and its walkaways at the field's part and only its runaways at the whole budget, the
/// features listing the rest (P09.T33.b), so under the version's rules the two counts are two
/// intensities (research-leans-45 item 2). The field share has its own census, that test's.
#[test]
#[ignore = "slow: the fixture's whole layer E under either arm, and two classes' cells"]
fn census_of_the_carve_out() {
    assert!(
        f64::from(REMNANT_REACH_CELLS) * f64::from(Layer::E.cell_size_ly()) >= remnant_reach_ly(),
        "the walk's reach covers the remnants' {} ly",
        remnant_reach_ly()
    );
    let off = milky_way(budget_rules(CarveOutRules::Unclaimed)).with_full_potential();
    let on = milky_way(budget_rules(CarveOutRules::Claimed)).with_full_potential();
    let (carved, past) = carved_galaxy(&off, &on);
    let mut by_class: BTreeMap<Class, Vec<u64>> = BTreeMap::new();
    for &(id, class) in &carved {
        by_class.entry(class).or_default().push(id);
    }
    for (class, ids) in &by_class {
        eprintln!("{class:?}: {} carved out", ids.len());
        for id in ids {
            eprintln!("  {class:?} {id:#018x}");
        }
    }
    eprintln!(
        "core collapses carved past the young field's stop, by the cells past it: {past:?} ({} in \
         all)",
        past.values().sum::<usize>()
    );
    let grid = |class| by_class.get(&class).map_or(0, Vec::len);
    // On wasm32-wasip1, only the Sun's columns: too few for the classes' counts.
    if cfg!(target_family = "wasm") {
        return;
    }
    let core = class_entries(&on, &CoreCollapseProcess::new(&on), ClassId::CORE_COLLAPSE);
    let lbv = class_entries(&on, &LbvProcess::new(&on), ClassId::LUMINOUS_BLUE_VARIABLE);
    // Both classes' counts are printed before either is asserted, so that one class's failure
    // does not hide the other's figures.
    let p_values: Vec<(&str, f64)> = [
        ("core collapses", grid(Class::CoreCollapse), core),
        (
            "luminous blue variables",
            grid(Class::LuminousBlueVariable),
            lbv,
        ),
    ]
    .into_iter()
    .map(|(name, a, b)| {
        let (a, b) = (
            f64::from(u32::try_from(a).unwrap()),
            f64::from(u32::try_from(b).unwrap()),
        );
        let z = (a - b) / (a + b).max(1.0).sqrt();
        let p = 2.0 * normal_cdf(-z.abs());
        eprintln!("{name}: the grid carves {a}, the class holds {b}, z = {z:.2}, p = {p:.2e}");
        (name, p)
    })
    .collect();
    for (name, p) in p_values {
        assert_p_value(&format!("the {name}' carve against the class"), p, ALPHA);
    }
    assert!(grid(Class::CoreCollapse) > 0 && grid(Class::LuminousBlueVariable) > 0);
}

/// The class's band: a claim is asked of layer E alone.
#[test]
fn only_layer_e_is_claimed() {
    let on = claimed();
    let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap();
    for layer in [Layer::A, Layer::B, Layer::C, Layer::D] {
        let key = CellKey::containing(layer, &sun).unwrap();
        let mut cell = Vec::new();
        generate_cell(on, key, &mut cell);
        for record in &cell {
            assert!(!catalogue_claims(on, record, None), "{:?}", record.id());
        }
    }
}

/// The cells of the carve-out's pre-pin, `placement/carve_out` (`tests/placement_golden.rs`'s
/// `CARVE_OUT_CELLS`, of its `CARVE_OUT_SEED`, which is [`SEED`]).
const PRE_PIN_CELLS: [[i32; 3]; 3] = [[147, 56, 0], [-68, 141, 0], [87, 168, 0]];

/// The pre-pin's premise: each of its cells holds a record that the core collapses claim under
/// every 26 hold but plan 08's displaced classes, so that the bump cannot leave its golden unmoved.
/// The classes redraw every layer-E record's marks (P08.T12.c), which moves the pre-pin's cells
/// at the bump in any case (checked under every 26 hold), and under them its first cell holds no
/// claimed death.
#[test]
fn the_pre_pin_s_cells_hold_claimed_deaths() {
    let field_only = |carve_out| {
        milky_way(GalaxyRules {
            carve_out,
            ..field_only_rules()
        })
    };
    let (off, on) = (
        field_only(CarveOutRules::Unclaimed),
        field_only(CarveOutRules::Claimed),
    );
    for cell in PRE_PIN_CELLS {
        let key = CellKey::new(Layer::E, cell).unwrap();
        let claimed = carved(&off, &on, &[key]);
        assert!(
            claimed
                .iter()
                .any(|(_, class)| *class == Class::CoreCollapse),
            "{cell:?}: {claimed:?}"
        );
    }
}

/// P09.T38's complementarity on the carve-out's field half, for sampled records: under every 26
/// hold the core-collapse claim on a grid record is the one shared test on the record's own marks,
/// taken afresh through its parts, plan 08's `death_in_interval` under the galaxy's stellar rules
/// on its initial mass, metallicity draw, η and draws, the site at its epoch position moved along
/// its velocity to the death, `DeathMarks::of` on its stars at that velocity and `claims` in the
/// field; and the carve-out claims it exactly then or for its window. The records are the ridge's
/// and young-disc records re-massed and re-aged to die inside the interval and past +H, so that the
/// test both claims and keeps.
///
/// Each side keys its stars' draws on its own system's ID, so a class candidate has no grid twin
/// with its marks: the test is per mark set, as P09.T38's is, and P09.T43's
/// `the_class_s_core_collapse_claim_is_the_grid_s` holds the class's claim equal to this one on
/// the same marks.
#[test]
fn the_core_collapses_claim_is_the_one_shared_test_on_sampled_records() {
    use crate::galaxy::catalogue_classes::supernova::{DeathMarks, Surroundings, claims};
    use crate::galaxy::displaced::death_in_interval_under;
    use crate::stellar::system::{draw_metallicity, primary_draws, primary_eta};
    let on = claimed();
    let shared = |r: &SystemRecord| {
        let composition = draw_metallicity(on, r);
        let Some(death) = death_in_interval_under(
            r.primary_initial_mass(),
            &composition,
            primary_eta(on, r),
            r.age_at_epoch(),
            || primary_draws(on, r),
            on.rules().stellar,
        ) else {
            return false;
        };
        let velocity = epoch_velocity(on, r);
        let elapsed = crate::units::Seconds::new(death.since_epoch().as_seconds_f64());
        let position = r
            .epoch_position()
            .translated(velocity.displacement_over(elapsed))
            .unwrap();
        let site = ExplosionSite::new(position, death);
        let stars =
            SystemStars::generate_with(on, r, &composition, MultiplicityContext::ForcedSingle);
        DeathMarks::of(on, &stars, &site, velocity)
            .is_some_and(|marks| claims(on, &site, &marks, Surroundings::Field))
    };
    let mut records = Vec::new();
    let mut cell = Vec::new();
    for key in ridge_cells(on, &[24_000.0, 26_000.0], 16) {
        generate_cell(unclaimed(), key, &mut cell);
        records.extend_from_slice(&cell);
    }
    let young: Vec<SystemRecord> = records
        .iter()
        .filter(|r| r.population() == Population::YoungThinDisc)
        .copied()
        .collect();
    for (n, base) in young.iter().take(24).enumerate() {
        let mass = 9.0 + 2.5 * f64::from(u32::try_from(n).unwrap());
        let probe = reshaped(base, mass, 3.0e7);
        let ClockDeath::At(death, _) =
            SystemStars::generate_in(on, &probe, MultiplicityContext::ForcedSingle).death_time()
        else {
            continue;
        };
        let lived = 3.0e7 + years_after_epoch(death);
        for since in [3.9e6, 1.0e6, 2.0e4, -500.0, -2_000.0] {
            records.push(reshaped(base, mass, lived + since));
        }
    }
    let (mut claimed, mut kept) = (0, 0);
    for record in &records {
        let grid =
            explosion_site(on, record).is_some_and(|site| recent_death_claims(on, &site, record));
        assert_eq!(grid, shared(record), "{:?}", record.id());
        assert_eq!(
            catalogue_claims(on, record, None),
            grid || lbv_claims(on, record),
            "{:?}",
            record.id()
        );
        if explosion_site(on, record).is_some() {
            if grid {
                claimed += 1;
            } else {
                kept += 1;
            }
        }
    }
    eprintln!(
        "{} records: {claimed} deaths claimed, {kept} kept",
        records.len()
    );
    assert!(claimed > 10 && kept > 0, "{claimed} {kept}");
}

/// Order independence under every 26 hold but plan 08's displaced classes, as
/// [`the_pre_pin_s_cells_hold_claimed_deaths`] reads them (sim-determinism, "Order independence"):
/// the pre-pin's cells and some of the ridge's, generated in any order, and the records the classes
/// claim from them, resolved in any order, give the same answers on a galaxy built fresh. With the
/// displaced classes too, at generator version 25, these cells hold no claimed record.
#[test]
fn the_claimed_cells_are_order_independent() {
    let galaxy = milky_way(field_only_rules());
    let mut keys: Vec<CellKey> = PRE_PIN_CELLS
        .iter()
        .map(|&cell| CellKey::new(Layer::E, cell).unwrap())
        .collect();
    keys.extend(ridge_cells(&galaxy, &[26_000.0], 8));
    assert_order_independent(&keys, |&key| {
        let mut cell = Vec::new();
        generate_cell(&galaxy, key, &mut cell);
        cell
    });
    let off = milky_way(GalaxyRules {
        carve_out: CarveOutRules::Unclaimed,
        ..field_only_rules()
    });
    let ids: Vec<_> = carved(&off, &galaxy, &keys)
        .iter()
        .map(|(record, _)| record.id())
        .collect();
    assert_ne!(ids.len(), 0, "the cells hold claimed records");
    assert_order_independent(&ids, |&id| resolve(&galaxy, id));
}

/// The digest of [`the_moving_galaxy_s_claims_are_pinned`]'s claimed records, as
/// [`CLAIMED_DIGEST`]'s. Blessed natively on x86-64; the wasip1 suite compares it. Re-pinned by
/// name with [`CLAIMED_DIGEST`] (P08.T12, and at generator versions 25, 26, 27 and 31; it was
/// `0xcf32_0fb1_b816_96b9` at 26 and is so again at 31, `0x7cb2_2fec_b1a6_222e` at 27 to 30).
const MOVING_CLAIMED_DIGEST: u64 = 0xcf32_0fb1_b816_96b9;

/// The claims of a galaxy with its kinematic tables, as every production galaxy has them: each
/// site is the epoch position moved along the record's drawn velocity to its death. The pre-pin's
/// cells and the ridge's at the solar circle under every 26 hold, their claimed records' digest
/// pinned, as [`the_classes_claim_their_hosts_from_the_grid_pinned`]'s is, for the WebAssembly
/// suite (D5).
#[test]
fn the_moving_galaxy_s_claims_are_pinned() {
    let off = milky_way(unclaimed_rules()).with_full_potential();
    let on = milky_way(GalaxyRules::of_version(crate::GENERATOR_VERSION)).with_full_potential();
    let mut keys: Vec<CellKey> = PRE_PIN_CELLS
        .iter()
        .map(|&cell| CellKey::new(Layer::E, cell).unwrap())
        .collect();
    keys.extend(ridge_cells(&on, &[26_000.0], 128));
    let claimed = carved(&off, &on, &keys);
    let mut values = Vec::new();
    for (record, class) in &claimed {
        assert_eq!(
            resolve(&on, record.id()),
            Err(ResolveSystemError::NoSuchSystem)
        );
        let raw = record.id().raw();
        values.push(f64::from(u32::try_from(raw >> 32).unwrap()));
        values.push(f64::from(u32::try_from(raw & 0xffff_ffff).unwrap()));
        values.push(record.primary_initial_mass().value());
        values.push(record.age_at_epoch().value());
        values.push(f64::from(u8::from(*class == Class::LuminousBlueVariable)));
    }
    let digest = f64_digest(&values);
    eprintln!(
        "{} cells: {} claimed, digest {digest:#018x}",
        keys.len(),
        claimed.len()
    );
    assert_ne!(claimed.len(), 0, "the cells hold claimed records");
    assert_eq!(digest, MOVING_CLAIMED_DIGEST, "{digest:#018x}");
}
