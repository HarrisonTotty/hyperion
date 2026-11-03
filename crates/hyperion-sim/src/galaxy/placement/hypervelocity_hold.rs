//! The tests of the hypervelocity survivors' weight, from generator version 27 (plan 09,
//! P09.T34.b; plan 08, Design notes 25 and 27), under every 27 hold:
//!
//! - the survivors inside the cube, some tens of thousands, at the galaxy's own Type Ia rate;
//! - layer D padded at `UNBOUND_PAD_SPEED`, the escape cut no longer holding there;
//! - every exempt class padded with the class's weight (P08.T12.d's check), the survivors alone in
//!   layer D;
//! - each survivor launched before the Type Ia class's interval, its donor a helium white dwarf
//!   of its speed's mass or a C/O one already when it was, flying at its speed from a launch site
//!   below layer D's pad, as P08.T17's debug assertion holds, and the same twice;
//! - near the Sun's circle, its mean cosine to the radial direction and the radial fallbacks;
//! - layer D's cells visited at |t| = H rising by under a third with its raised pad;
//! - and, slow, at t = ±H a 50 ly query about each of 100 centres, each with a survivor within
//!   15 ly outside it at the epoch, equal to the brute-force enumeration.

use std::sync::OnceLock;

use hyperion_testkit::float::{assert_same_bits, bits};
use hyperion_testkit::lcg::Lcg;
use hyperion_testkit::stats::{ALPHA, assert_p_value, assert_poisson_count, normal_cdf};

use super::candidate::Picked;
use super::velocity_hold::exempt_classes_padded;
use super::{
    CellKey, NoCache, SystemRecord, generate_cell, is_helium_donor, marks, survivor_flight,
};
use crate::Seed;
use crate::coords::GalacticPosition;
use crate::galaxy::catalogue_classes::supernova::SupernovaInterval;
use crate::galaxy::displaced::class_table::{ClassKey, hypervelocity_class};
use crate::galaxy::displaced::survivors::{
    DonorKind, LaunchDirection, LaunchSampler, SurvivorBand, SurvivorFlight,
};
use crate::galaxy::displaced::{DisplacedKind, PlacementClass};
use crate::galaxy::imf::MassBand;
use crate::galaxy::kinematics::draw;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::query::{
    PAD_SPEED, QuerySphere, RangeQuery, UNBOUND_PAD_SPEED, count_cells_in_sphere, epoch_velocity,
    escape_cut_holds, pad_for, pad_speed, position_at, range_query,
};
use crate::galaxy::rules::GalaxyRules;
use crate::galaxy::snr::ShellEnvironment;
use crate::galaxy::{Galaxy, PointLy};
use crate::id::Layer;
use crate::math;
use crate::planetary::testing::in_shares;
use crate::rng::{ObjectKey, Stream, tags};
use crate::stellar::Phase;
use crate::stellar::brief::BriefModel;
use crate::stellar::multiplicity::MultiplicityContext;
use crate::stellar::system::SystemStars;
use crate::time::{CLOCK_WINDOW_H, UniverseTime};
use crate::units::consts::SPEED_OF_LIGHT;
use crate::units::{KilometresPerSecond, LightYears, Years};

/// The seed of the hold's Milky Way fixture.
const SEED: u64 = 0x0934_b000_0000_0001;

/// The Milky Way fixture under every 27 hold, the survivors weighted, with its kinematic tables.
fn weighted() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params_with_rules(
            Seed::new(SEED),
            GalaxyParams::milky_way_like(),
            GalaxyRules::every_27_hold(),
        )
        .expect("the Milky Way fixture builds")
        .with_full_potential()
    })
}

/// The clock window's two ends, −H and +H.
fn window_ends() -> [UniverseTime; 2] {
    let h = UniverseTime::EPOCH
        .checked_add(CLOCK_WINDOW_H)
        .expect("H is on the clock");
    let minus_h = UniverseTime::EPOCH
        .checked_sub(CLOCK_WINDOW_H)
        .expect("−H is on the clock");
    [minus_h, h]
}

/// Whether `record` is a hypervelocity survivor.
fn is_survivor(record: &SystemRecord) -> bool {
    matches!(
        record.placement_class(),
        PlacementClass::Displaced {
            kind: DisplacedKind::HypervelocitySurvivor,
            ..
        }
    )
}

/// The survivors inside the cube are some tens of thousands at the galaxy's Type Ia rate, within
/// Li et al.'s (2011) 0.54 ± 0.12 per century for the Milky Way; they are layer D's alone.
#[test]
fn the_survivors_inside_the_cube_are_tens_of_thousands() {
    let hv = hypervelocity_class();
    let fields = weighted()
        .fields()
        .displaced()
        .expect("the classes are installed");
    let table = fields.class_table();
    let survivors = table.survivors();
    let inside = table.class_count(MassBand::D, hv);
    eprintln!(
        "{inside:.0} survivors inside the cube, {:?} by population, at {:.3} Type Ia a century, \
         the youngest launched {:.3} Myr ago",
        survivors.count(),
        survivors.rate_per_year() * 100.0,
        survivors.youngest().value() / 1e6
    );
    assert!((1e4..=1e5).contains(&inside), "{inside}");
    // None is younger than the Type Ia class's interval's start, where its entries take over.
    let start = SupernovaInterval::of_cap(weighted().window_caps(), ShellEnvironment::TypeIa)
        .start()
        .since_epoch()
        .as_julian_years_f64();
    assert!(survivors.youngest().value() >= -start, "{start}");
    assert!((inside - survivors.total()).abs() <= 1e-12 * inside);
    assert!(table.class_count(MassBand::E, hv).abs() < 1e-300);
    assert!(table.class_weight(MassBand::D, hv).abs() < 1e-300);
    let per_century = survivors.rate_per_year() * 100.0;
    assert!((0.42..=0.66).contains(&per_century), "{per_century}");
    assert!(fields.order(MassBand::D).contains(&hv));
    assert!(!fields.order(MassBand::E).contains(&hv));
}

/// Layer D pads at `UNBOUND_PAD_SPEED` with the survivors' weight, as layer E does, and the escape
/// cut no longer holds there; every other layer pads at plan 03's speed.
#[test]
fn layer_d_pads_at_the_unbound_speed_with_the_survivors() {
    for layer in Layer::ALL {
        let unbound = matches!(layer, Layer::D | Layer::E);
        let speed = if unbound {
            UNBOUND_PAD_SPEED
        } else {
            PAD_SPEED
        };
        assert_eq!(pad_speed(layer), speed, "{layer:?}");
        assert_eq!(escape_cut_holds(layer), !unbound, "{layer:?}");
    }
}

/// P08.T12.d's check with the class's weight: every exempt class with weight lies in a layer the
/// escape cut does not hold, and the survivors are layer D's only one.
#[test]
fn every_exempt_class_is_padded_with_the_survivors_weight() {
    let exempt = exempt_classes_padded(weighted());
    let in_d: Vec<ClassKey> = exempt
        .iter()
        .filter(|&&(_, layer)| layer == Layer::D)
        .map(|&(key, _)| key)
        .collect();
    assert_eq!(in_d, [ClassKey::Hypervelocity], "{exempt:?}");
    assert!(exempt.len() > 1, "{exempt:?}");
}

/// `n` survivors at `at`, their marks drawn for consecutive candidates of the layer-D cell holding
/// it.
fn survivors_at(
    galaxy: &Galaxy,
    at: &GalacticPosition,
    n: u32,
) -> Vec<(SystemRecord, Option<f64>)> {
    let fields = galaxy
        .fields()
        .displaced()
        .expect("the classes are installed");
    let key = CellKey::containing(Layer::D, at).expect("in the cube");
    (0..n)
        .map(|i| {
            let id = key.candidate_id(i).expect("under the capacity");
            let drawn = marks::draw(
                galaxy,
                fields,
                key,
                id,
                at,
                Picked::Displaced(hypervelocity_class()),
            )
            .expect("a displaced class draws marks");
            (
                SystemRecord::of_marks(galaxy, id, *at, &drawn),
                drawn.lifetime.map(Years::value),
            )
        })
        .collect()
}

/// What a test knows of a survivor's marks: the lifetime they gave, where it drew them, or nothing,
/// for a record a cell generated.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Marked {
    /// The marks' lifetime.
    Lifetime(Option<f64>),
    /// A generated record's, whose marks are not at hand.
    Generated,
}

/// Checks the survivor `record` (whose marks are `marked`) and gives its flight, its launch and its
/// star's phase at the epoch: launched no later than the Type Ia class's interval's
/// start, where the entries take over, nor before its path across the cube allows, born in a
/// component with Type Ia; its donor of its speed's mass, a helium white dwarf of that mass and
/// age, or a C/O or O/Ne one whose layer-D progenitor died before the launch, as the stellar stage
/// builds the star; and its velocity its speed along its launch's direction, under layer D's pad,
/// as `epoch_velocity` (whose debug assertion, P08.T17's, holds it below the pad) gives it.
fn check_survivor(
    galaxy: &Galaxy,
    record: &SystemRecord,
    marked: Marked,
) -> (SurvivorFlight, LaunchDirection, Phase) {
    let survivors = galaxy
        .fields()
        .displaced()
        .map(|fields| fields.class_table().survivors())
        .expect("the classes are installed");
    assert!(is_survivor(record), "{record:?}");
    assert_eq!(record.layer(), Layer::D);
    let flight = survivor_flight(galaxy, record).expect("a survivor has its flight");
    let since = flight.since_launch().value();
    let band = flight.band().population();
    let v = flight.speed().value();
    assert!((band.min_km_s..=band.max_km_s).contains(&v), "{flight:?}");
    assert!(since >= survivors.youngest().value(), "{flight:?}");
    let inside = survivors.mean_exit_ly() / (v / (SPEED_OF_LIGHT / 1_000.0));
    assert!(since <= inside * (1.0 + 1e-12), "{flight:?}");
    let born = record.component().expect("a grid record has a component");
    assert!(survivors.births_per_year()[born.index()] > 0.0, "{born:?}");
    let stars = SystemStars::generate_in(galaxy, record, MultiplicityContext::ForcedSingle);
    let primary = stars.primary();
    let state = primary
        .state_at(UniverseTime::EPOCH)
        .expect("a survivor's white dwarf is born");
    let mass = record.primary_initial_mass();
    match (flight.band(), flight.donor_kind()) {
        (SurvivorBand::Slow, DonorKind::Helium) => {
            assert_same_bits(mass.value(), flight.donor().value());
            assert!((0.15..=0.5).contains(&mass.value()), "{flight:?}");
            assert_eq!(state.phase(), Phase::HeliumWhiteDwarf, "{flight:?}");
            assert!(
                (state.mass().value() - mass.value()).abs() < 1e-9,
                "{state:?}"
            );
            assert_same_bits(record.age_at_epoch().value(), since);
            assert!(matches!(marked, Marked::Lifetime(None) | Marked::Generated));
        }
        (SurvivorBand::Fast, DonorKind::CarbonOxygen) => {
            assert!((2.5..=8.0).contains(&mass.value()), "{mass:?}");
            assert!(
                matches!(
                    state.phase(),
                    Phase::CarbonOxygenWhiteDwarf | Phase::OxygenNeonWhiteDwarf
                ),
                "{state:?}"
            );
            let death = primary.death().expect("a dead progenitor").age().value();
            match marked {
                Marked::Lifetime(Some(life)) => assert_same_bits(death, life),
                // Where the fate table read the progenitor's fate (P08.T12 at 27), the marks give
                // no lifetime.
                Marked::Lifetime(None) | Marked::Generated => {}
            }
            let age = record.age_at_epoch().value();
            assert!(age - death >= since * (1.0 - 1e-9), "{age} {death} {since}");
        }
        other => panic!("a survivor of the {other:?}: {flight:?}"),
    }
    let mut stream = Stream::open(
        galaxy.seed(),
        tags::SYSTEM_VELOCITY,
        ObjectKey::from(record.id()),
    );
    let at = PointLy::from(record.epoch_position());
    let launch = survivors.launch_direction(galaxy, &at, &flight, &mut stream);
    let velocity = draw(galaxy, record);
    let speed = velocity.velocity().speed().value() / 1_000.0;
    assert!((speed - v).abs() <= 1e-9 * v, "{speed} {v}");
    assert!(speed < pad_speed(Layer::D).value());
    assert!((velocity.cut().value() - UNBOUND_PAD_SPEED.value()).abs() < 1e-9);
    let w = velocity.velocity().metres_per_second();
    for (c, n) in w.iter().zip(launch.direction()) {
        assert!(
            (c / (speed * 1_000.0) - n).abs() < 1e-12,
            "{w:?} {launch:?}"
        );
    }
    if !launch.is_radial() {
        let d = flight.flown_ly();
        let site = PointLy {
            x: at.x - d * launch.direction()[0],
            y: at.y - d * launch.direction()[1],
            z: at.z - d * launch.direction()[2],
        };
        assert!(
            survivors.launches_per_year_per_cubic_ly(galaxy, &site) > 0.0,
            "{site:?}"
        );
    }
    assert_eq!(epoch_velocity(galaxy, record), velocity.velocity());
    (flight, launch, state.phase())
}

/// At the Sun-like point, above the bulge and in the halo, each survivor's marks, star and line
/// ([`check_survivor`]); the populations come in the counts' odds.
#[test]
fn a_survivor_is_a_white_dwarf_launched_before_the_interval() {
    let galaxy = weighted();
    let survivors = galaxy
        .fields()
        .displaced()
        .map(|fields| fields.class_table().survivors())
        .expect("the classes are installed");
    let at = |ly: [f64; 3]| GalacticPosition::from_light_years(ly).expect("in the cube");
    let (mut seen, mut fast) = (0_u32, 0_u32);
    for place in [
        at([0.0, 26_000.0, 20.0]),
        at([0.0, 2_000.0, 6_000.0]),
        at([0.0, 0.0, 30_000.0]),
    ] {
        for (record, lifetime) in survivors_at(galaxy, &place, 400) {
            let (flight, _, _) = check_survivor(galaxy, &record, Marked::Lifetime(lifetime));
            seen += 1;
            fast += u32::from(flight.band() == SurvivorBand::Fast);
        }
    }
    let p = survivors.count()[1] / survivors.total();
    let expected = p * f64::from(seen);
    let z = (f64::from(fast) - expected) / (expected * (1.0 - p)).sqrt();
    assert_p_value(
        "the fast survivors' share",
        2.0 * normal_cdf(-z.abs()),
        ALPHA,
    );
}

/// In the plane at R = 7.5–9 kpc, about the Sun's circle, survivors fly from their launch sites in
/// the old stars, not straight out from the centre: their mean cosine to the radial direction,
/// recorded against the science review's about 0.5 from a disc and bulge's convolution; and under
/// one in a hundred takes the radial direction for want of an accepted try.
#[test]
fn near_the_suns_circle_survivors_fly_from_their_launch_sites() {
    let galaxy = weighted();
    let survivors = galaxy
        .fields()
        .displaced()
        .map(|fields| fields.class_table().survivors())
        .expect("the classes are installed");
    let (mut cosines, mut radial, mut tries) = (Vec::new(), 0_u32, 0_u64);
    for k in 0..24_u32 {
        let azimuth = core::f64::consts::TAU * f64::from(k) / 24.0;
        let r = 24_500.0 + 4_800.0 * f64::from(k % 6) / 5.0;
        let (sin, cos) = math::sin_cos(azimuth);
        let place =
            GalacticPosition::from_light_years([r * cos, r * sin, 20.0]).expect("in the cube");
        for (record, _) in survivors_at(galaxy, &place, 25) {
            let flight = survivor_flight(galaxy, &record).expect("a survivor has its flight");
            let mut stream = Stream::open(
                galaxy.seed(),
                tags::SYSTEM_VELOCITY,
                ObjectKey::from(record.id()),
            );
            let at = PointLy::from(record.epoch_position());
            let launch = survivors.launch_direction(galaxy, &at, &flight, &mut stream);
            let norm = (at.x * at.x + at.y * at.y + at.z * at.z).sqrt();
            let n = launch.direction();
            cosines.push((n[0] * at.x + n[1] * at.y + n[2] * at.z) / norm);
            radial += u32::from(launch.is_radial());
            tries += u64::from(launch.tries());
        }
    }
    #[expect(clippy::cast_precision_loss, reason = "counts far below 2⁵³")]
    let (n, mean_tries) = (cosines.len() as f64, tries as f64 / cosines.len() as f64);
    let mean = cosines.iter().sum::<f64>() / n;
    eprintln!(
        "{n} survivors at R 7.5–9 kpc: mean cosine to radial {mean:.3} (the science review's \
         about 0.5), {radial} radial, {mean_tries:.1} tries each"
    );
    assert!(mean < 0.99 && mean > 0.0, "{mean}");
    assert!(f64::from(radial) < 0.01 * n, "{radial} of {n}");
}

/// A survivor's brief is its system's: the brief model takes a helium donor's white dwarf exactly,
/// as the stellar stage builds it, and a C/O donor's by its routes, at the epoch the same, or on
/// the fate table's route ([`BriefRoute::Table`]) its kind and class with its luminosity and
/// temperature within their stated error, as every table brief is. From 26, whose fate tables
/// are fitted under the enforced Humphreys–Davidson limit every 27 hold takes, most C/O donors
/// take the table's route (17 of the 21 here).
///
/// [`BriefRoute::Table`]: crate::stellar::brief::BriefRoute::Table
#[test]
fn a_survivors_brief_is_its_white_dwarfs() {
    use crate::stellar::brief::BriefRoute;
    let galaxy = weighted();
    let at = GalacticPosition::from_light_years([0.0, 26_000.0, 20.0]).expect("in the cube");
    let (mut kinds, mut tabled) = ([0_u32; 2], 0_u32);
    for (record, _) in survivors_at(galaxy, &at, 200) {
        let model = BriefModel::new(galaxy, &record);
        let brief = model.brief_at(UniverseTime::EPOCH);
        let stars = SystemStars::generate_in(galaxy, &record, MultiplicityContext::ForcedSingle);
        let exact = stars.brief_at(UniverseTime::EPOCH);
        kinds[usize::from(is_helium_donor(&record))] += 1;
        if model.route() != BriefRoute::Table {
            assert_eq!(brief, exact, "{record:?}");
            continue;
        }
        assert!(!is_helium_donor(&record), "{record:?}");
        tabled += 1;
        let (fitted, exact) = (brief.expect("a brief"), exact.expect("a brief"));
        assert_eq!(
            (fitted.kind(), fitted.class().to_string()),
            (exact.kind(), exact.class().to_string()),
            "{record:?}"
        );
        let stated = model
            .stated_error_at(UniverseTime::EPOCH)
            .expect("a table brief states its error");
        let (log_l, teff) = (
            exact.log_luminosity().expect("a luminosity").value(),
            exact.effective_temperature().value(),
        );
        assert!(
            (stated.log_luminosity.0..=stated.log_luminosity.1).contains(&log_l)
                && (stated.effective_temperature.0..=stated.effective_temperature.1)
                    .contains(&teff),
            "{record:?}: {log_l}, {teff} outside {stated:?}"
        );
    }
    eprintln!("survivors by donor (C/O, helium) {kinds:?}, {tabled} on the table's route");
    assert!(kinds[0] > 0 && kinds[1] > 0, "{kinds:?}");
}

/// The launch's bound holds the launch density over its proposal on a lattice of 20,000
/// directions on each sphere, for survivors at the Sun-like point, above the bulge and in the
/// halo: the rejection is exact where the bound is, which its probes only sample.
#[test]
fn the_launchs_bound_holds_its_ratio_over_the_sphere() {
    let galaxy = weighted();
    let survivors = galaxy
        .fields()
        .displaced()
        .map(|fields| fields.class_table().survivors())
        .expect("the classes are installed");
    let at = |ly: [f64; 3]| GalacticPosition::from_light_years(ly).expect("in the cube");
    let mut largest: f64 = 0.0;
    for place in [
        at([0.0, 26_000.0, 20.0]),
        at([0.0, 2_000.0, 6_000.0]),
        at([0.0, 0.0, 30_000.0]),
    ] {
        for (record, _) in survivors_at(galaxy, &place, 6) {
            let flight = survivor_flight(galaxy, &record).expect("a survivor has its flight");
            let sampler = LaunchSampler::new(
                survivors,
                galaxy,
                &PointLy::from(record.epoch_position()),
                &flight,
            );
            let bound = sampler.bound();
            assert!(bound > 0.0 && bound.is_finite(), "{bound}");
            let n = 20_000_u32;
            for i in 0..n {
                let z = 1.0 - (2.0 * f64::from(i) + 1.0) / f64::from(n);
                let across = (1.0 - z * z).sqrt();
                let (sin, cos) = math::sin_cos(f64::from(i) * 2.399_963_229_728_653);
                let direction = [across * cos, across * sin, z];
                let ratio = sampler.density(direction) / sampler.proposal(direction);
                largest = largest.max(ratio / bound);
                assert!(ratio <= bound, "{ratio} over {bound} at {direction:?}");
            }
        }
    }
    eprintln!("the ratio reaches {largest:.3} of the bound");
}

/// The same seed gives the same survivors twice: two galaxies built apart draw each survivor's
/// record, flight and velocity bit for bit, and their survivors' table the same. The seed keys
/// placement only, so another seed's galaxy has the same table too.
#[test]
fn the_same_seed_gives_the_same_survivors_twice() {
    let built = |seed: u64| {
        Galaxy::from_params_with_rules(
            Seed::new(seed),
            GalaxyParams::milky_way_like(),
            GalaxyRules::every_27_hold(),
        )
        .expect("the Milky Way fixture builds")
    };
    let again = built(SEED).with_full_potential();
    let table = |galaxy: &Galaxy| {
        galaxy
            .fields()
            .displaced()
            .map(|fields| fields.class_table().survivors())
            .cloned()
            .expect("the classes are installed")
    };
    assert_eq!(table(weighted()), table(&again));
    assert_eq!(table(weighted()), table(&built(7)));
    let at = GalacticPosition::from_light_years([0.0, 2_000.0, 6_000.0]).expect("in the cube");
    let first = survivors_at(weighted(), &at, 200);
    let second = survivors_at(&again, &at, 200);
    assert_eq!(first, second);
    let mut helium = 0;
    for ((a, _), (b, _)) in first.iter().zip(&second) {
        assert_eq!(survivor_flight(weighted(), a), survivor_flight(&again, b));
        let (va, vb) = (draw(weighted(), a).velocity(), draw(&again, b).velocity());
        for (x, y) in va
            .metres_per_second()
            .into_iter()
            .zip(vb.metres_per_second())
        {
            assert_same_bits(x, y);
        }
        // The star each builds, a helium donor's through its own branch, and its brief.
        let star = |galaxy: &Galaxy, record: &SystemRecord| {
            SystemStars::generate_in(galaxy, record, MultiplicityContext::ForcedSingle)
                .primary()
                .state_at(UniverseTime::EPOCH)
        };
        assert_eq!(star(weighted(), a), star(&again, b), "{a:?}");
        assert_eq!(
            BriefModel::new(weighted(), a).brief_at(UniverseTime::EPOCH),
            BriefModel::new(&again, b).brief_at(UniverseTime::EPOCH)
        );
        helium += u32::from(is_helium_donor(a));
    }
    assert!(helium > 0, "no helium donor among the survivors");
}

/// Layer D's cells visited by a sphere of 50 ly about `centre` at `t`, padded at `speed`.
fn layer_d_cells(centre: GalacticPosition, t: UniverseTime, speed: KilometresPerSecond) -> u64 {
    let sphere = QuerySphere::new(centre, LightYears::new(50.0), t, pad_for(t, speed))
        .expect("a sphere inside the cube");
    count_cells_in_sphere(Layer::D, &sphere)
}

/// Layer D's cells visited by a 50 ly sphere at |t| = H rise by under a third with its pad raised
/// to `UNBOUND_PAD_SPEED`, over 1,000 centres about the Sun-like point (Design note 27's estimate
/// for layer E's coarser cells was a cell one time in four).
#[test]
fn layer_d_cells_visited_at_the_window_ends_rise_by_under_a_third() {
    let mut lcg = Lcg::new(0x0934_b0d0);
    let (mut raised, mut held) = (0_u64, 0_u64);
    for _ in 0..1_000 {
        let offset = [0, 1, 2].map(|_| (lcg.next_f64() - 0.5) * 4_096.0);
        let centre =
            GalacticPosition::from_light_years([offset[0], 26_000.0 + offset[1], offset[2] * 0.1])
                .expect("in the cube");
        for t in window_ends() {
            raised += layer_d_cells(centre, t, UNBOUND_PAD_SPEED);
            held += layer_d_cells(centre, t, PAD_SPEED);
        }
    }
    #[expect(clippy::cast_precision_loss, reason = "counts far below 2⁵³")]
    let rise = raised as f64 / held as f64 - 1.0;
    eprintln!(
        "layer D's cells at |t| = H: {held} at 1,000 km/s, {raised} at 3,000 km/s, {rise:.3}"
    );
    assert!(rise < 1.0 / 3.0, "{rise}");
}

/// The layer-D cells the slow test sweeps for survivors, from the first corner's to before the
/// second's: a box 20,480 ly square about the z axis and 4,096 ly thick from 8,192 ly above the
/// plane, over the bulge, where the survivors' density is near its core's and layer D's thin.
/// Some 6.6 million cells at a few µs each, holding some 180 survivors (176 expected; 156 found,
/// pod, 16 vCPU, 2026-11-01).
const SCAN: ([i32; 3], [i32; 3]) = ([-160, -160, 128], [160, 160, 192]);

/// The survivors of the layer-D cells of [`SCAN`], in ID order, an x-slab a share on the test's
/// threads.
fn scanned_survivors(galaxy: &Galaxy) -> Vec<SystemRecord> {
    let (lo, hi) = SCAN;
    let slabs = usize::try_from(hi[0] - lo[0]).expect("a positive span");
    let mut found: Vec<SystemRecord> = in_shares(slabs, 16, |slab| {
        let x = lo[0] + i32::try_from(slab).expect("a slab of the span");
        let (mut cell, mut out) = (Vec::new(), Vec::new());
        for y in lo[1]..hi[1] {
            for z in lo[2]..hi[2] {
                let key = CellKey::new(Layer::D, [x, y, z]).expect("a cell of the cube");
                generate_cell(galaxy, key, &mut cell);
                out.extend(cell.iter().filter(|record| is_survivor(record)).copied());
            }
        }
        out
    })
    .into_iter()
    .flatten()
    .collect();
    found.sort_by_key(|record| record.id().raw());
    found
}

/// The class's expected survivors in [`SCAN`]: its count times its form's integral over the box,
/// by a midpoint sum of 64 × 64 × 16 nodes.
fn expected_in_scan(galaxy: &Galaxy) -> f64 {
    let fields = galaxy
        .fields()
        .displaced()
        .expect("the classes are installed");
    let class = fields.class(hypervelocity_class());
    let form = class.form().expect("a weighted class has its form");
    let (lo, hi) = SCAN;
    let size = f64::from(Layer::D.cell_size_ly());
    let low = lo.map(|c| f64::from(c) * size);
    let span = [0, 1, 2].map(|axis| f64::from(hi[axis] - lo[axis]) * size);
    let nodes = [64_u32, 64, 16];
    let step = [0, 1, 2].map(|axis| span[axis] / f64::from(nodes[axis]));
    let mut sum = 0.0;
    for i in 0..nodes[0] {
        for j in 0..nodes[1] {
            for k in 0..nodes[2] {
                let p = PointLy {
                    x: low[0] + (f64::from(i) + 0.5) * step[0],
                    y: low[1] + (f64::from(j) + 0.5) * step[1],
                    z: low[2] + (f64::from(k) + 0.5) * step[2],
                };
                sum += form.density(&p);
            }
        }
    }
    class.count(MassBand::D) * sum * step[0] * step[1] * step[2]
}

/// Every grid system of the five stellar layers within `radius` of `centre` at `t`, as its ID and
/// its distance's bits, in ID order: every cell of each layer within `radius` + 20 ly of the
/// centre generated and each system tested, 20 ly being twice the farthest any record moves in H
/// below `UNBOUND_PAD_SPEED`, so that the search knows no pad.
fn brute_force(
    galaxy: &Galaxy,
    centre: &GalacticPosition,
    radius: LightYears,
    t: UniverseTime,
) -> Vec<(u64, u64)> {
    let radius = radius.value();
    let reach = radius + 20.0;
    let c = centre.to_light_years_f64();
    let corner = |sign: f64| {
        GalacticPosition::from_light_years(c.map(|x| x + sign * reach)).expect("in the cube")
    };
    let mut cell = Vec::new();
    let mut hits = Vec::new();
    for layer in [Layer::E, Layer::D, Layer::C, Layer::B, Layer::A] {
        let size = i32::try_from(layer.cell_size_ly()).expect("a cell under 2³¹ ly");
        let index = |at: &GalacticPosition| {
            CellKey::containing(layer, at)
                .expect("in the cube")
                .origin_ly()
                .map(|o| o / size)
        };
        let (lo, hi) = (index(&corner(-1.0)), index(&corner(1.0)));
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    let key = CellKey::new(layer, [x, y, z]).expect("a cell of the cube");
                    generate_cell(galaxy, key, &mut cell);
                    for record in &cell {
                        if record.age_at(t).value() <= 0.0 {
                            continue;
                        }
                        let at = position_at(galaxy, record, t);
                        let distance = LightYears::from(centre.distance_to(&at)).value();
                        if distance <= radius {
                            hits.push((record.id().raw(), bits(distance)));
                        }
                    }
                }
            }
        }
    }
    hits.sort_unstable();
    hits
}

/// The binding test of the pad (plan 09, P09.T34.b), on the first 100 survivors of [`SCAN`] by
/// ID, the scan's count against the class's density and none launched after the Type Ia class's
/// interval's start. Each centre lies 50 ly plus half its survivor's travel in H from it, along
/// its velocity for even survivors and against it for odd, so that its survivor lies within 15 ly
/// outside the sphere at the epoch and inside it at one end of the clock window. At t = ±H the
/// 50 ly query about it is the brute-force enumeration, system for system and distance for
/// distance, and holds the survivor at its end; P08.T17's debug assertion holds over the
/// survivors (`epoch_velocity`); and layer D's cells visited rise by under a third with the raised
/// pad.
#[test]
#[ignore = "slow: sweeps 6.6 million layer-D cells for survivors and 200 queries' cells by brute \
            force, some 25 s on 16 threads"]
fn at_the_window_ends_a_query_beside_a_survivor_is_the_brute_force_enumeration() {
    let galaxy = weighted();
    let survivors = galaxy
        .fields()
        .displaced()
        .map(|fields| fields.class_table().survivors())
        .expect("the classes are installed");
    let found = scanned_survivors(galaxy);
    let expected = expected_in_scan(galaxy);
    eprintln!(
        "{} survivors in the scan, {expected:.1} expected",
        found.len()
    );
    assert_poisson_count(
        "the scan's survivors",
        u64::try_from(found.len()).expect("a count"),
        expected,
        ALPHA,
    );
    let (mut helium, mut carbon_oxygen, mut radial) = (0_u32, 0_u32, 0_u32);
    for record in &found {
        let (flight, launch, _) = check_survivor(galaxy, record, Marked::Generated);
        assert!(flight.since_launch() >= survivors.youngest(), "{flight:?}");
        match flight.donor_kind() {
            DonorKind::Helium => helium += 1,
            DonorKind::CarbonOxygen => carbon_oxygen += 1,
        }
        radial += u32::from(launch.is_radial());
    }
    eprintln!(
        "{helium} helium and {carbon_oxygen} C/O or O/Ne white dwarfs, {radial} launched radially"
    );
    assert!(
        f64::from(radial) < 0.01 * f64::from(helium + carbon_oxygen),
        "{radial}"
    );
    assert!(found.len() >= 100, "{} survivors", found.len());
    let [minus_h, plus_h] = window_ends();
    let h_years = CLOCK_WINDOW_H.as_julian_years_f64();
    let (mut raised, mut held) = (0_u64, 0_u64);
    for (i, record) in found.iter().take(100).enumerate() {
        let velocity = epoch_velocity(galaxy, record).metres_per_second();
        let speed =
            (velocity[0] * velocity[0] + velocity[1] * velocity[1] + velocity[2] * velocity[2])
                .sqrt();
        assert!(speed / 1_000.0 < pad_speed(Layer::D).value(), "{speed} m/s");
        let travel = speed / SPEED_OF_LIGHT * h_years;
        let (sign, toward) = if i % 2 == 0 {
            (1.0, plus_h)
        } else {
            (-1.0, minus_h)
        };
        let gap = 0.5 * travel;
        assert!(gap > 0.0 && gap <= 15.0, "{gap} ly");
        let x = record.epoch_position().to_light_years_f64();
        let centre = GalacticPosition::from_light_years(
            [0, 1, 2].map(|a| x[a] + sign * (50.0 + gap) * velocity[a] / speed),
        )
        .expect("in the cube");
        for t in [minus_h, plus_h] {
            let query = RangeQuery::builder(centre, LightYears::new(50.0))
                .time(t)
                .build()
                .expect("a 50 ly query in the clock window");
            let result = range_query(galaxy, &mut NoCache::new(), &[], &query);
            assert_eq!(result.census().complete_down_to(), Some(Layer::A), "{i}");
            let mut answer: Vec<(u64, u64)> = result
                .systems()
                .iter()
                .map(|hit| (hit.id().raw(), bits(hit.distance().value())))
                .collect();
            answer.sort_unstable();
            let scanned = brute_force(galaxy, &centre, LightYears::new(50.0), t);
            assert_eq!(answer, scanned, "{i} at {t}");
            if t == toward {
                assert!(
                    answer.iter().any(|&(id, _)| id == record.id().raw()),
                    "{i}: the survivor is not inside at {t}"
                );
            }
            raised += layer_d_cells(centre, t, UNBOUND_PAD_SPEED);
            held += layer_d_cells(centre, t, PAD_SPEED);
        }
    }
    #[expect(clippy::cast_precision_loss, reason = "counts far below 2⁵³")]
    let rise = raised as f64 / held as f64 - 1.0;
    eprintln!(
        "layer D's cells at |t| = H: {held} at 1,000 km/s, {raised} at 3,000 km/s, {rise:.3}"
    );
    assert!(rise < 1.0 / 3.0, "{rise}");
}
