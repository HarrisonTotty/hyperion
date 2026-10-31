//! The tests of P15.T7.b's hold, [`HELIUM_VERSION`] (decision-stellar-holds §1.2, E1′–E4′, with
//! decision-galaxy-25-train §1.5–§1.6's D2–D4): every reader of the helium-excess table takes it
//! from its rules, through [`HeliumTable::of`], and the rules ride a track's [`TrackOptions`].
//!
//! - **D3.** The version's rules read P06.T17's identity, and a track built without rules takes
//!   them: a default track's options are the version's, value for value and in `Debug`.
//! - **D2.** Under every 26 hold these readers show a mover: a track's builders (the track, its
//!   fate and remnant without segments, and the brief's knot-free main sequence), a star's model
//!   and its death's companion-stripped mark, the engine's own members and its pinned death, the
//!   stripping seam's exact path, the members' conditional draws, and a second-population
//!   member's stars and brief in a galaxy. A star of ΔY = 0 moves under none. The engine's tracks
//!   rebuilt in the middle of a timeline (after a common envelope, a supernova's companion) and
//!   the hierarchy's marked orbit take the same `Context::track_options` and galaxy rules with no
//!   mover of their own; the census reaches them through the members' stars.
//! - **D4.** The same seed twice under every 26 hold gives the same output.
//! - **D1** is the goldens as they stand: no version takes the fitted table, so nothing on the
//!   record moves by it.
//! - **C1** is [`census_of_the_fitted_helium_table`], slow.
//!
//! Readers that take the version's rules, and why none can move:
//!
//! - `turn_off_mass`: every turn-off the galaxy asks for is a population's own, at no helium
//!   excess (a cluster's composition, the centre's, the global list's), which no table moves;
//! - a naked helium star's track (`Track::helium_star*`, the engine's `build_from_entry`): it has
//!   no hydrogen main sequence, gap, giant branch or horizontal branch, which alone read the hook;
//! - a remnant-only track: a remnant's segment reads no hook;
//! - the engine's closed forms of a main-sequence star (`main_sequence_lifetime`,
//!   `main_sequence_structure`), which read no hook under any table (P06.T17 as built).
//!
//! No static reaches the field, so none is keyed by its arm: the fates' table refuses a helium
//! excess (`FittedFates`' `interpolate`), as do the pair light's and the reach's tables
//! (`has_helium_excess`), the stripping table (`StrippingTable::at`) and the lifetime bracket
//! (`helium_free`); the kick bins and the class table build at the layers' ΔY = 0; and a feature
//! member is `not_modelled` in every sky reply. `hyperion-fit`'s tasks build no galaxy and read
//! the version's rules by design.

use std::sync::OnceLock;

use super::*;
use crate::GENERATOR_VERSION;
use crate::coords::{GalacticPosition, GalacticVelocity};
use crate::galaxy::Galaxy;
use crate::galaxy::displaced::binarity::{stripped_share_under, stripping_node_under};
use crate::galaxy::features::FeatureProcess;
use crate::galaxy::features::catalogue::{FeatureCatalogue, FeatureMarks, FeatureRecord};
use crate::galaxy::features::cluster::ClusterModel;
use crate::galaxy::features::interior::{
    ClassKind, Generation, MemberClass, MemberClassTable, Multiplicity,
};
use crate::galaxy::features::members::{MemberRecord, draw_member, remnant_fits};
use crate::galaxy::imf::MassBand;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::rules::GalaxyRules;
use crate::id::{FeatureMemberId, MemberSlot, SystemId};
use crate::orbit::{Eccentricity, KeplerElements, Orientation};
use crate::rng::Seed;
use crate::stellar::binary::BinaryInput;
use crate::stellar::brief::BriefModel;
use crate::stellar::draws::StarDraws;
use crate::stellar::remnant::KickRankRules;
use crate::stellar::sse::{HeliumHook, HeliumTable};
use crate::stellar::sse::{Track, TrackOptions, fate_of, remnant_of};
use crate::stellar::system::{StarModel, SystemStars};
use crate::stellar::{Composition, Phase};
use crate::time::UniverseTime;
use hyperion_testkit::float::assert_same_bits;

use crate::units::{
    Dex, GravitationalParameter, HeliumExcess, Radians, Seconds, SolarMasses, Years,
};

/// The universe of the galaxy fixtures: `features::members`' own.
const SEED: u64 = 0x0910_0000;

/// The fixture under the generator version's rules.
fn held() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture builds")
    })
}

/// [`held`]'s galaxy under every 26 hold.
fn fitted() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| held().clone().with_rules(GalaxyRules::every_26_hold()))
}

/// The version's stellar rules.
fn version() -> StellarRules {
    StellarRules::of_version(GENERATOR_VERSION)
}

/// A star of \[Fe/H\] −1.5 with a helium excess of `delta_y`.
fn composition(delta_y: f64) -> Composition {
    Composition::from_fe_h(Dex::new(-1.5), HeliumExcess::new(delta_y))
}

/// Track options under every 26 hold.
fn fitted_options() -> TrackOptions {
    TrackOptions::default().with_rules(StellarRules::every_26_hold())
}

/// The state of `track` at each of `ages_yr`, compared by bits: phase, mass, luminosity, radius.
fn same_states(a: &Track, b: &Track, ages_yr: &[f64]) -> bool {
    ages_yr.iter().all(|&age| {
        let (s, t) = (a.state_at(Years::new(age)), b.state_at(Years::new(age)));
        s.phase() == t.phase()
            && s.mass().value().total_cmp(&t.mass().value()).is_eq()
            && s.luminosity()
                .value()
                .total_cmp(&t.luminosity().value())
                .is_eq()
            && s.radius().value().total_cmp(&t.radius().value()).is_eq()
    })
}

/// 200 ages even in log₁₀ from 10⁵ to 3 × 10¹⁰ years.
fn ages_yr() -> Vec<f64> {
    (0..200)
        .map(|i| crate::math::exp10(5.0 + 5.477 * f64::from(i) / 199.0))
        .collect()
}

/// P15.T7.b's swap is held until the 25 → 26 bump: the rules in force read P06.T17's identity.
#[test]
fn the_helium_table_is_held_only_until_the_26_bump() {
    assert!(
        GENERATOR_VERSION < HELIUM_VERSION,
        "the 25 → 26 bump puts P15.T7.b's fitted helium table on the record (every member with a \
         helium excess moves): delete `HELIUM_VERSION`, `HeliumRules` with its `Identity` arm, \
         `StellarRules::helium`, the `match` in `HeliumTable::of`, `HeliumTable::hb_envelope_max`'s \
         `Option` and this test; move `helium_26.toml`'s parameters into `helium.toml` at revision \
         1 with `helium_26`'s fingerprint, re-emit `tables::helium` at 26, and delete the \
         `helium_26` instance, its table and its lock entry (decision-stellar-holds §1.2)"
    );
    assert_eq!(version().helium, HeliumRules::Identity);
    assert_eq!(held().rules().stellar, version());
}

/// The fitted table turns on at 26: every 25 hold still reads the identity, every 26 hold the fit,
/// and the kick-rank field keeps its committed table.
#[test]
fn the_fitted_table_turns_on_at_26() {
    let at = |v| StellarRules::of_version(GeneratorVersion::new(v)).helium;
    for v in [23, 24, 25] {
        assert_eq!(at(v), HeliumRules::Identity, "version {v}");
    }
    for v in [26, 27, 64] {
        assert_eq!(at(v), HeliumRules::Fitted, "version {v}");
    }
    assert_eq!(StellarRules::every_25_hold().helium, HeliumRules::Identity);
    assert_eq!(StellarRules::every_26_hold().helium, HeliumRules::Fitted);
    assert_eq!(
        StellarRules::every_26_hold().kick_rank,
        KickRankRules::Committed
    );
    assert_eq!(
        GalaxyRules::every_26_hold().stellar,
        StellarRules::every_26_hold()
    );
    assert_eq!(fitted().rules(), GalaxyRules::every_26_hold());
}

/// D3: the version's rules read the identity; a default track's options carry them, name no
/// rules in `Debug`, and build a helium-rich star's track as `Track::full` does.
#[test]
fn the_version_s_rules_read_the_identity() {
    let rules = version();
    assert_eq!(HeliumTable::of(rules), HeliumTable::COMMITTED);
    assert_eq!(TrackOptions::default().rules(), rules);
    assert_eq!(
        TrackOptions::default().with_rules(rules),
        TrackOptions::default()
    );
    assert!(!format!("{:?}", TrackOptions::default()).contains("rules"));
    assert!(format!("{:?}", fitted_options()).contains("helium: Fitted"));
    let (m, rich, draws) = (SolarMasses::new(0.8), composition(0.1), StarDraws::median());
    assert_eq!(
        Track::full(m, &rich, &draws),
        Track::full_with(m, &rich, &draws, TrackOptions::default().with_rules(rules))
    );
}

/// D2, the track's builders: under the fitted table a 0.8 M☉ star with ΔY = 0.1 lives about a
/// half shorter (0.42–0.55, decision-p15-t7b-leans L3), its build without segments reaches the
/// same death and remnant bit for bit, and its knot-free main sequence, the brief's route, ends
/// sooner; at ΔY = 0 every one is bit-identical under either table.
#[test]
fn a_track_takes_its_options_helium_table() {
    let draws = StarDraws::median();
    let ages = ages_yr();
    for (m, delta_y) in [(0.8, 0.0), (3.0, 0.0), (20.0, 0.0)] {
        let (m, plain) = (SolarMasses::new(m), composition(delta_y));
        let (a, b) = (
            Track::full(m, &plain, &draws),
            Track::full_with(m, &plain, &draws, fitted_options()),
        );
        assert!(same_states(&a, &b, &ages), "{m:?} at ΔY 0");
        assert_eq!(a.lifetime(), b.lifetime());
    }
    let (m, rich) = (SolarMasses::new(0.8), composition(0.1));
    let (a, b) = (
        Track::full(m, &rich, &draws),
        Track::full_with(m, &rich, &draws, fitted_options()),
    );
    let life = |t: &Track| t.lifetime().expect("a full track dies").value();
    let shortening = 1.0 - life(&b) / life(&a);
    assert!((0.42..=0.55).contains(&shortening), "{shortening}");
    let fate = fate_of(m, &rich, &draws, fitted_options());
    assert_same_bits(fate.death.age().value(), life(&b));
    let (remnant, _) = remnant_of(m, &rich, &draws, fitted_options());
    assert_eq!(Some(remnant), b.remnant_model());
    assert_ne!(b.remnant_model(), a.remnant_model());
    let ends = |options: TrackOptions| {
        Track::knot_free_main_sequence(m, &rich, &draws, options)
            .expect("a 0.8 M☉ main sequence has no knots")
            .built_until()
            .value()
    };
    assert!(ends(fitted_options()) < 0.6 * ends(TrackOptions::default()));
}

/// D2, a star's model: its track is built under its rules, so a helium-rich model ends its main
/// sequence and dies sooner under every 26 hold, and one of ΔY = 0 is the same model's.
#[test]
fn a_star_s_model_takes_its_rules_helium_table() {
    let (mass, age) = (SolarMasses::new(0.8), Years::new(1.0e10));
    let model = |delta_y: f64, rules| {
        StarModel::new_under(mass, composition(delta_y), StarDraws::median(), age, rules)
            .expect("in range")
    };
    let life = |s: &StarModel| s.lifetime().expect("a star dies").value();
    let now = |s: &StarModel| s.state_at(UniverseTime::EPOCH).expect("formed");
    let (held, fitted) = (
        model(0.1, version()),
        model(0.1, StellarRules::every_26_hold()),
    );
    assert!(
        life(&fitted) < 0.6 * life(&held),
        "{} against {}",
        life(&fitted),
        life(&held)
    );
    assert_eq!(now(&held).phase(), Phase::MainSequence);
    assert_ne!(now(&fitted).phase(), Phase::MainSequence);
    let (plain, plain_fitted) = (
        model(0.0, version()),
        model(0.0, StellarRules::every_26_hold()),
    );
    assert_same_bits(life(&plain), life(&plain_fitted));
    assert_eq!(now(&plain), now(&plain_fitted));
}

/// A wide pair of 0.85 and 0.7 M☉ with a helium excess of `delta_y`, 12 Gyr old, on 30 years,
/// which never interacts: each star is its own track.
fn wide_pair(delta_y: f64) -> BinaryInput {
    let (m1, m2) = (0.85, 0.7);
    let orbit = KeplerElements::from_period(
        Seconds::new(30.0 * 365.25 * 86_400.0),
        GravitationalParameter::from_solar_masses(SolarMasses::new(m1 + m2)),
        Eccentricity::CIRCULAR,
        Orientation::new(Radians::new(0.3), Radians::new(0.0), Radians::new(0.0))
            .expect("a valid orientation"),
        Radians::new(0.0),
    )
    .expect("a valid orbit");
    BinaryInput::new(
        SolarMasses::new(m1),
        SolarMasses::new(m2),
        composition(delta_y),
        orbit,
        [StarDraws::median(), StarDraws::median()],
        Years::new(1.2e10),
    )
    .expect("two stars in range with a finite age")
}

/// D2, the engine: `BinaryInput::with_rules` alone moves a helium-rich pair's stars, so the
/// engine builds its tracks under its input's rules; a pair of ΔY = 0 keeps its states bit for
/// bit.
#[test]
fn a_pair_s_tracks_take_its_input_s_helium_table() {
    let until = Years::new(1.2e10);
    let run = |input: &BinaryInput| {
        let a = crate::stellar::binary::evolve(input, until);
        let b = crate::stellar::binary::evolve(
            &input.clone().with_rules(StellarRules::every_26_hold()),
            until,
        );
        let at = |t: &crate::stellar::binary::BinaryTimeline, age: f64| {
            t.state_at(Years::new(age)).stars()[0]
        };
        [6.0e9, 9.0e9, 1.1e10].map(|age| (at(&a, age), at(&b, age)))
    };
    for (a, b) in run(&wide_pair(0.0)) {
        assert_eq!(a, b);
    }
    let moved = run(&wide_pair(0.15));
    assert!(moved.iter().any(|(a, b)| a != b), "{moved:?}");
    // The helium-rich primary leaves its main sequence sooner: at 6 Gyr it has under the fit.
    let (a, b) = moved[0];
    assert_eq!(a.phase(), Phase::MainSequence);
    assert_ne!(b.phase(), Phase::MainSequence);
}

/// D2, the stripping seam: a helium-rich primary is outside the stripping table, so its share and
/// radii come from its own track, which is built under the rules of the star or the galaxy that
/// reads them; at ΔY = 0 the table answers under either.
#[test]
fn the_stripping_seam_s_exact_path_takes_its_rules() {
    let m = SolarMasses::new(12.0);
    let rich = composition(0.15);
    let (a, b) = (
        stripping_node_under(m, &rich, version()),
        stripping_node_under(m, &rich, StellarRules::every_26_hold()),
    );
    assert_ne!(a, b);
    assert_same_bits(a.share(), stripped_share_under(m, &rich, version()));
    assert_same_bits(
        b.share(),
        stripped_share_under(m, &rich, StellarRules::every_26_hold()),
    );
    let plain = composition(0.0);
    assert_eq!(
        stripping_node_under(m, &plain, version()),
        stripping_node_under(m, &plain, StellarRules::every_26_hold())
    );
}

/// D2, a star's death: its companion-stripped mark is read against the stripped share under its
/// own rules, as its track's is, so a helium-rich 10 M☉ star whose mark lies between the two
/// arms' shares is stripped at its death under one and not the other.
#[test]
fn a_star_s_stripped_mark_takes_its_rules() {
    use crate::galaxy::displaced::binarity::is_stripped_at;
    use crate::id::BodyId;
    use crate::stellar::remnant::Stripping;
    let (m, rich, age) = (SolarMasses::new(10.0), composition(0.15), Years::new(5.0e7));
    let shares = (
        stripped_share_under(m, &rich, version()),
        stripped_share_under(m, &rich, StellarRules::every_26_hold()),
    );
    assert!(shares.0.total_cmp(&shares.1).is_ne(), "{shares:?}");
    let system = SystemId::from_raw(0x8200_b2e0_0000_0000).expect("a well-formed ID");
    let draws = (0..20_000)
        .map(|attempt| StarDraws::for_attempt(Seed::new(SEED), BodyId::new(system, 0), attempt))
        .find(|d| is_stripped_at(d.stripped(), shares.0) != is_stripped_at(d.stripped(), shares.1))
        .expect("a mark between the two shares");
    let stripped = |rules| {
        let star = StarModel::new_under(m, rich, draws.clone(), age, rules).expect("in range");
        star.death().expect("a star dies").progenitor().stripping() == Stripping::Companion
    };
    assert_eq!(
        (stripped(version()), stripped(StellarRules::every_26_hold())),
        (
            is_stripped_at(draws.stripped(), shares.0),
            is_stripped_at(draws.stripped(), shares.1)
        )
    );
}

/// D2, the engine's pinned death (design note 16): a helium-rich massive primary's death in its
/// pair is its own model's under the same rules, at the fitted table's earlier age.
#[test]
fn a_pair_s_pinned_death_takes_its_input_s_rules() {
    let (m1, m2, days) = (15.0, 12.0, 300.0);
    let rich = composition(0.15);
    let orbit = KeplerElements::from_period(
        Seconds::new(days * 86_400.0),
        GravitationalParameter::from_solar_masses(SolarMasses::new(m1 + m2)),
        Eccentricity::CIRCULAR,
        Orientation::new(Radians::new(0.3), Radians::new(0.0), Radians::new(0.0))
            .expect("a valid orientation"),
        Radians::new(0.0),
    )
    .expect("a valid orbit");
    let age = Years::new(5.0e7);
    let input = BinaryInput::new(
        SolarMasses::new(m1),
        SolarMasses::new(m2),
        rich,
        orbit,
        [StarDraws::median(), StarDraws::median()],
        age,
    )
    .expect("two stars in range with a finite age");
    let primary_death = |rules| {
        let timeline = crate::stellar::binary::evolve(&input.clone().with_rules(rules), age);
        timeline
            .supernovae()
            .iter()
            .find(|r| r.component() == crate::stellar::binary::Component::Primary)
            .map(crate::stellar::binary::SupernovaRecord::age)
            .expect("the primary explodes")
            .value()
    };
    let model_death = |rules| {
        StarModel::new_under(SolarMasses::new(m1), rich, StarDraws::median(), age, rules)
            .expect("in range")
            .death()
            .expect("a star dies")
            .age()
            .value()
    };
    let (held, fitted) = (version(), StellarRules::every_26_hold());
    assert_same_bits(primary_death(held), model_death(held));
    assert_same_bits(primary_death(fitted), model_death(fitted));
    assert!(primary_death(fitted) < primary_death(held));
}

/// D2, the members' conditional draws: a neutron-star member is kept only if its kick falls below
/// the cluster's escape speed, and its star is built under the galaxy's rules, so a helium-rich
/// progenitor's verdict at an escape speed set to its kick under the identity moves under the
/// fitted table wherever the kick does.
#[test]
fn the_members_draws_read_their_galaxy_s_helium_table() {
    let system = SystemId::from_raw(0x8200_b2e0_0000_0000).expect("a well-formed ID");
    let (rich, age) = (composition(0.15), Years::new(5.0e7));
    let universe = Seed::new(SEED);
    let mut flipped = 0;
    for k in 0..20 {
        let mass = 9.0 + 0.5 * f64::from(k);
        for attempt in 0..5 {
            let draws =
                StarDraws::for_attempt(universe, crate::id::BodyId::new(system, 0), attempt);
            let star = StarModel::new_under(SolarMasses::new(mass), rich, draws, age, version())
                .expect("in range");
            let ordinary = star
                .natal_kick()
                .filter(|kick| kick.mode() == crate::stellar::remnant::KickMode::Ordinary);
            let Some(kick) = ordinary else {
                continue;
            };
            let speed = crate::units::KilometresPerSecond::from(kick.speed()).value();
            let fits = |v_eff: f64, rules| {
                remnant_fits(
                    ClassKind::NeutronStar,
                    mass,
                    &rich,
                    (universe, system, attempt),
                    age,
                    (v_eff, 1.0),
                    rules,
                )
            };
            for v_eff in [speed, speed * (1.0 + 1e-12)] {
                flipped +=
                    u32::from(fits(v_eff, version()) != fits(v_eff, StellarRules::every_26_hold()));
            }
        }
    }
    assert!(flipped > 0, "no draw moved");
}

/// The most massive globular of `galaxy`, which has a second population, its model and its class
/// table.
fn cluster(galaxy: &Galaxy) -> (FeatureRecord, ClusterModel, MemberClassTable) {
    let feature = FeatureCatalogue::walk_process(galaxy, FeatureProcess::Globular)
        .max_by(|a, b| globular_mass(a).total_cmp(&globular_mass(b)))
        .expect("the fixture has globulars");
    model_of(galaxy, feature)
}

/// A globular's initial mass, M☉, or zero for any other feature.
fn globular_mass(feature: &FeatureRecord) -> f64 {
    match feature.marks() {
        FeatureMarks::Globular(g) => g.mass().value(),
        FeatureMarks::OpenCluster(_) | FeatureMarks::Nursery(_) | FeatureMarks::Cloud(_) => 0.0,
    }
}

/// `feature`'s model and class table in `galaxy`.
fn model_of(
    galaxy: &Galaxy,
    feature: FeatureRecord,
) -> (FeatureRecord, ClusterModel, MemberClassTable) {
    let model = ClusterModel::from_record(galaxy, &feature).expect("a globular is a cluster");
    let table = MemberClassTable::new(galaxy, &model, feature.reach(), [1.0, 0.0, 0.0]);
    (feature, model, table)
}

/// The member `index` of `band`'s cell of `feature`.
fn member_id(feature: &FeatureRecord, band: MassBand, index: u32) -> FeatureMemberId {
    FeatureMemberId::new(
        feature.id().feature_ref(),
        MemberSlot::InCell {
            band: band.layer(),
            level: 0,
            cell: [8, 8, 8],
            index,
        },
    )
    .expect("a member slot of the fixture's cell")
}

/// A point one light year from `feature`'s centre.
fn near(feature: &FeatureRecord) -> GalacticPosition {
    let [x, y, z] = feature.position().to_light_years_f64();
    GalacticPosition::from_light_years([x + 1.0, y, z]).expect("inside the galaxy")
}

/// `table`'s class of `band`, `kind`, `generation` and `multiplicity`, if it has one.
fn class_of(
    table: &MemberClassTable,
    band: MassBand,
    kind: ClassKind,
    generation: Generation,
    multiplicity: Multiplicity,
) -> Option<MemberClass> {
    table
        .classes(band)
        .find(|(c, _)| {
            c.kind == kind && c.generation == generation && c.multiplicity == multiplicity
        })
        .map(|(c, _)| *c)
}

/// Member `index` of `class` of `feature`, drawn in `galaxy`.
fn draw(
    galaxy: &Galaxy,
    (feature, model): (&FeatureRecord, &ClusterModel),
    class: &MemberClass,
    index: u16,
) -> Option<MemberRecord> {
    draw_member(
        galaxy,
        feature,
        model,
        class,
        &near(feature),
        member_id(feature, class.band, u32::from(index)),
        GalacticVelocity::new([0.0; 3]),
    )
}

/// D2 and D4, a member in a galaxy: a second-population member's stars and its brief, both built
/// under the galaxy's rules, move under every 26 hold, a first-population member's do not, and
/// the same galaxy built twice under every 26 hold gives the same stars.
#[test]
fn a_second_population_member_s_stars_and_brief_take_their_galaxy_s_table() {
    let (feature, model, table) = cluster(held());
    let band = MassBand::A;
    let second = class_of(
        &table,
        band,
        ClassKind::Living,
        Generation::Second,
        Multiplicity::Single,
    )
    .expect("a massive globular has a second population of living stars");
    let first = class_of(
        &table,
        band,
        ClassKind::Living,
        Generation::First,
        Multiplicity::Single,
    )
    .expect("a globular has a first population");
    let again = Galaxy::from_params_with_rules(
        Seed::new(SEED),
        GalaxyParams::milky_way_like(),
        GalaxyRules::every_26_hold(),
    )
    .expect("the Milky Way fixture builds");
    let (mut stars_moved, mut briefs_moved) = (0, 0);
    for index in 0..40 {
        for (class, rich) in [(&first, false), (&second, true)] {
            let a = draw(held(), (&feature, &model), class, index).expect("a living member");
            let b = draw(fitted(), (&feature, &model), class, index).expect("a living member");
            assert_eq!(a, b, "a living member's draw builds no track");
            assert_eq!(a.composition().helium_excess().value() > 0.0, rich);
            let (sa, sb) = (a.stars(held()), b.stars(fitted()));
            assert_eq!(sb, b.stars(&again), "D4");
            let (ba, bb) = (
                BriefModel::of_member(held(), &a).brief_at(UniverseTime::EPOCH),
                BriefModel::of_member(fitted(), &b).brief_at(UniverseTime::EPOCH),
            );
            let now = |s: &SystemStars| s.primary().state_at(UniverseTime::EPOCH);
            if rich {
                stars_moved += u32::from(now(&sa) != now(&sb));
                briefs_moved += u32::from(ba != bb);
            } else {
                assert_eq!(now(&sa), now(&sb));
                assert_eq!(
                    sa.summary_at(UniverseTime::EPOCH),
                    sb.summary_at(UniverseTime::EPOCH)
                );
                assert_eq!(ba, bb);
            }
        }
    }
    assert!(stars_moved > 0, "no second-population star moved");
    assert!(briefs_moved > 0, "no second-population brief moved");
}

/// The census's figures, the fitted table against the identity, over one galaxy's members.
#[derive(Debug, Default)]
struct Census {
    members: u32,
    rich: u32,
    draws_moved: u32,
    stars_moved: u32,
    /// Stars whose initial mass differs between the arms: a companion the hierarchy drew otherwise.
    redrawn: u32,
    /// The fitted table's smallest timescale factor exp(s ΔY) over the second population's stars.
    factor: (f64, String),
    /// Its largest shift of the horizontal branch, dex, at no envelope.
    shift: (f64, String),
    /// The largest relative shortening of the same star's lifetime.
    shortening: (f64, String),
    /// The same star's lifetime under the fit over the identity's, where it is the largest.
    longest: (f64, String),
    /// Members of a remnant's class whose primary at the epoch is not that remnant, under the
    /// identity and under the fit: the class is judged on the primary's own model, a binary
    /// member's stars on its pair.
    unlike_class: (u32, u32),
    /// Changes of a star's kind or shown class, `(identity, fit)`, counted.
    classes: std::collections::BTreeMap<(String, String), u32>,
    changes: Vec<String>,
}

impl Census {
    /// Compares member `label`'s stars under the identity, `a`, with the fit's, `b`.
    fn compare(&mut self, label: &str, rich: bool, a: &SystemStars, b: &SystemStars) {
        let (sa, sb) = (
            a.summary_at(UniverseTime::EPOCH),
            b.summary_at(UniverseTime::EPOCH),
        );
        if !rich {
            assert_eq!(sa, sb, "{label}: a first-population member moved");
            return;
        }
        if sa != sb {
            self.stars_moved += 1;
        }
        for (i, (x, y)) in a.stars().iter().zip(b.stars()).enumerate() {
            if x.initial_mass()
                .value()
                .total_cmp(&y.initial_mass().value())
                .is_ne()
            {
                self.redrawn += 1;
                continue;
            }
            let at = format!("{label} star {i}");
            if let Some(hook) = HeliumHook::of(y.composition(), &HeliumTable::FITTED) {
                let m = x.initial_mass().value().clamp(0.1, 150.0);
                let factor = hook.timescale_factor(m);
                if self.factor.1.is_empty() || factor < self.factor.0 {
                    self.factor = (factor, at.clone());
                }
                let shift = HeliumTable::FITTED.hb_temperature_shift[0]
                    * y.composition().helium_excess().value();
                if shift > self.shift.0 {
                    self.shift = (shift, at.clone());
                }
            }
            if let (Some(p), Some(q)) = (x.lifetime(), y.lifetime()) {
                let ratio = q.value() / p.value();
                assert!(ratio.is_finite() && ratio > 0.0, "{at}: {ratio}");
                if 1.0 - ratio > self.shortening.0 {
                    self.shortening = (1.0 - ratio, at.clone());
                }
                if ratio > self.longest.0 {
                    self.longest = (ratio, at.clone());
                }
            }
            if let (Some(p), Some(q)) = (
                x.state_at(UniverseTime::EPOCH),
                y.state_at(UniverseTime::EPOCH),
            ) {
                for s in [p, q] {
                    assert!(
                        s.effective_temperature().value().is_finite()
                            && s.radius().value().is_finite(),
                        "{at}: {s:?}"
                    );
                }
            }
        }
        for (i, (x, y)) in sa.stars().iter().zip(sb.stars()).enumerate() {
            let (shown_a, shown_b) = (
                format!("{:?} {}", x.kind(), x.classification()),
                format!("{:?} {}", y.kind(), y.classification()),
            );
            if shown_a != shown_b {
                let kinds = (format!("{:?}", x.kind()), format!("{:?}", y.kind()));
                *self.classes.entry(kinds).or_insert(0) += 1;
                self.changes
                    .push(format!("{label} star {i}: {shown_a} against {shown_b}"));
            }
        }
        if sa.stars().len() != sb.stars().len() {
            self.changes.push(format!(
                "{label}: {} stars against {}",
                sa.stars().len(),
                sb.stars().len()
            ));
        }
    }
}

/// Whether `stars`' primary is, at the epoch, the remnant a member of `kind` is drawn as (any
/// state for a living or tail class).
fn primary_is(stars: &SystemStars, kind: ClassKind) -> bool {
    use crate::stellar::ObjectKind;
    let now = stars.summary_at(UniverseTime::EPOCH);
    let Some(primary) = now.stars().first() else {
        return false;
    };
    match kind {
        ClassKind::Living | ClassKind::Tail => true,
        ClassKind::WhiteDwarf => primary.kind() == ObjectKind::WhiteDwarf,
        ClassKind::NeutronStar => primary.kind() == ObjectKind::NeutronStar,
        ClassKind::BlackHole => primary.kind() == ObjectKind::BlackHole,
    }
}

/// C1, the census, the fitted table against the identity: in each of three drawn galaxies, the
/// three most massive globulars, and in each, 30 members of every class of either population in
/// every band, drawn and built under each galaxy's rules. It reports the second-population members
/// whose conditional draw or stars move, the largest change of a star's lifetime and of a
/// horizontal-branch star's log₁₀ `T_eff` at the epoch, and every change of a star's kind or class
/// by member; it holds that no first-population member moves and every value is finite. Nothing
/// on the record moves by the fitted table until 26.
#[test]
#[ignore = "slow: some 10⁴ globular members under either helium table"]
fn census_of_the_fitted_helium_table() {
    for seed in [SEED, 0x0910_0000_0000_0001, 0x0910_0000_0000_0002] {
        let held = Galaxy::new(Seed::new(seed));
        let fitted = held.clone().with_rules(GalaxyRules::every_26_hold());
        let mut globulars: Vec<FeatureRecord> =
            FeatureCatalogue::walk_process(&held, FeatureProcess::Globular).collect();
        globulars.sort_by(|a, b| globular_mass(b).total_cmp(&globular_mass(a)));
        let mut census = Census::default();
        for feature in globulars.into_iter().take(3) {
            let mass = globular_mass(&feature);
            let (feature, model, table) = model_of(&held, feature);
            for band in MassBand::ALL {
                let classes: Vec<MemberClass> = table.classes(band).map(|(c, _)| *c).collect();
                for class in &classes {
                    let rich = class.generation == Generation::Second;
                    for index in 0..30 {
                        let label = format!(
                            "{:?} ({mass:.3e} M☉) {band:?} {:?} {:?} {:?} #{index}",
                            feature.id(),
                            class.kind,
                            class.generation,
                            class.multiplicity
                        );
                        let (a, b) = (
                            draw(&held, (&feature, &model), class, index),
                            draw(&fitted, (&feature, &model), class, index),
                        );
                        census.members += 1;
                        census.rich += u32::from(rich);
                        if a != b {
                            assert!(rich, "{label}: a first-population draw moved");
                            census.draws_moved += 1;
                        }
                        if let (Some(a), Some(b)) = (a, b) {
                            let (sa, sb) = (a.stars(&held), b.stars(&fitted));
                            census.unlike_class.0 += u32::from(!primary_is(&sa, class.kind));
                            census.unlike_class.1 += u32::from(!primary_is(&sb, class.kind));
                            census.compare(&label, rich, &sa, &sb);
                        }
                    }
                }
            }
        }
        eprintln!(
            "seed {seed:#018x}: {} members, {} of the second population; {} draws and {} \
             members' stars moved, {} companions redrawn; the fit's smallest timescale factor \
             {:.4} ({}), its largest branch shift {:.4} dex ({}); the same star's lifetime \
             shortened by up to {:.4} ({}), its largest ratio fit over identity {:.4} ({}); {} \
             changes of kind or shown class",
            census.members,
            census.rich,
            census.draws_moved,
            census.stars_moved,
            census.redrawn,
            census.factor.0,
            census.factor.1,
            census.shift.0,
            census.shift.1,
            census.shortening.0,
            census.shortening.1,
            census.longest.0,
            census.longest.1,
            census.changes.len()
        );
        eprintln!(
            "  remnant-class members whose primary is not of the class at the epoch: {} under \
             the identity, {} under the fit",
            census.unlike_class.0, census.unlike_class.1
        );
        for ((from, to), n) in &census.classes {
            eprintln!("  kind {from} -> {to}: {n} stars whose kind or shown class moved");
        }
        for change in &census.changes {
            eprintln!("  {change}");
        }
        assert!(census.stars_moved > 0, "seed {seed:#018x}: no member moved");
    }
}

/// The fitted arm's values, pinned bit for bit, so that each value the 26 bump moves can be
/// traced to the swap: a 0.8 M☉ star of \[Fe/H\] −1.5 and ΔY = 0.1 dies at 7.42 Gyr under every
/// 26 hold, and is on a blue horizontal branch, near 8,600 K, at the first 10 Myr step past 1 Gyr
/// that finds it burning helium in its core. Values under a hold's arm are not generated output
/// until the bump; a later member that moves them re-pins them here by name.
#[test]
fn the_fitted_arm_s_values_are_pinned() {
    let (m, rich, draws) = (SolarMasses::new(0.8), composition(0.1), StarDraws::median());
    let track = Track::full_with(m, &rich, &draws, fitted_options());
    assert_same_bits(
        track.lifetime().expect("a full track dies").value(),
        7_419_064_629.866_532,
    );
    let age = (0..4000)
        .map(|i| 1e9 + 1e7 * f64::from(i))
        .find(|&a| track.state_at(Years::new(a)).phase() == Phase::CoreHeliumBurning)
        .expect("a horizontal branch");
    assert_same_bits(age, 7.31e9);
    assert_same_bits(
        track
            .state_at(Years::new(age))
            .effective_temperature()
            .value(),
        8_632.309_939_944_349,
    );
}
