//! The tests of P06.T42.b's hold, [`HD_LIMIT_VERSION`] (decision-stellar-holds §1.2, E1′–E4′, with
//! decision-galaxy-25-train §1.5–§1.6's D2–D4): the modern winds enforce the Humphreys–Davidson
//! limit only under the rules that turn it on, read at one choke point, `wind::rate`, through
//! every track's [`TrackOptions`], and a luminous blue variable's eruptions are thinned under the
//! same arm. Each test turns this hold's arm on alone ([`enforced_rules`]), so that the other 26
//! holds move nothing it holds equal.
//!
//! - **D3.** The version's rules keep Belczynski et al.'s constant, and a default track takes them.
//! - **D2.** Under the enforced limit these readers show a mover: a track's builders (the track, its
//!   fate and remnant without segments), a star's model, the engine's pinned death, the range
//!   brief (whose table route closes and whose gate certifies no pair, its primaries built
//!   exactly), the explosion sites' deaths (`displaced::site`, the carve-out's and the catalogue
//!   classes' interval predicate), and the eruptions' thinning (with `events::lbv`'s
//!   `the_thinned_eruptions_are_the_windows_candidates_at_lbv_states`, which holds the thinned
//!   listing to the bins' candidates). A star that never passes the limit moves under none.
//! - **D4.** The same seed twice under the enforced limit gives the same output.
//! - **D1** is the goldens as they stand: no version takes the enforced arm.
//! - **The swap's three checks** (plan 06, P06.T42.b) are slow: the remnant masses
//!   ([`enforced_remnants_are_no_heavier_unless_pair_unstable`]), the envelope integrator against the term's
//!   stiffness ([`the_envelope_integrator_holds_past_the_wall`]) and R06's brightness envelope and
//!   caps ([`the_brightness_envelope_bounds_the_enforced_tracks`]). Research-p06 §8's mass budget
//!   is [`the_eruptions_ejecta_stay_near_the_tracks_loss`], and its census is `stellar::testing`'s
//!   `lbv_eruptions_galaxy_wide_under_the_enforced_limit`.
//!
//! Readers that take the version's rules, and why:
//!
//! - a main-sequence or naked helium star's wind (the engine's `main_sequence_structure`, the
//!   helium stars' tracks): the limit acts on evolved hydrogen-rich stars alone, so their rate is
//!   the same under either arm;
//! - the tables fitted offline from the tracks, which `hyperion-fit` builds under the version's
//!   rules and the bump refits: the fates (`stellar_fates_*`, and through them the galaxy's
//!   mean-mass quadrature `galaxy::fates::fates_for`, hence `GalaxyParams`' masses, system count
//!   and the potential), the stripping and reach tables (the stripping table's helium axis under
//!   the fitted helium table and the version's other rules), the pair light, `kick_rank`'s
//!   fingerprint and the sky's tables. So under the enforced limit a galaxy keeps the version's
//!   parameters and potential, and the birth populations plan 15's orbit run samples, which the
//!   freeze holds; the class table's kick bins and lifetimes keep them too. The brief reads a
//!   fitted fate per star and the pair-light and reach tables per pair, and refuses both under
//!   another arm ([`HdLimitRules::fitted_tables_hold`]);
//! - the interval predicate's mass bracket (`displaced::marks::LifetimeBracket`), built from the
//!   version's lifetimes and widened by a tenth; its exact tier and the core-collapse class's
//!   lifetimes read the galaxy's rules, and its fate-table tier is refused under the limit, as
//!   the brief's is. Under every 26 hold the carve-out puts the site on the record, where its
//!   death must be its stars';
//! - readers that are no single star's track and that nothing on the record reaches under a hold:
//!   the stripping seam's table for stars with no helium excess, which takes the enforced tracks
//!   with the bump's refits.

use std::fmt::Write as _;
use std::sync::OnceLock;

use super::*;
use crate::GENERATOR_VERSION;
use crate::events::RateModel;
use crate::galaxy::Galaxy;
use crate::galaxy::displaced::explosion_site;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{CellKey, SystemRecord, generate_cell};
use crate::galaxy::rules::GalaxyRules;
use crate::id::Layer;
use crate::orbit::{Eccentricity, KeplerElements, Orientation};
use crate::rng::Seed;
use crate::sky::envelope::{BrightnessEnvelope, FE_H_NODES, SAMPLES_PER_PHASE, raw_node_under};
use crate::stellar::binary::{BinaryInput, Component, SupernovaRecord};
use crate::stellar::brief::{BriefModel, BriefRoute};
use crate::stellar::classify::is_luminous_blue_variable;
use crate::stellar::draws::{StandardNormal, StarDraws, StarDrawsParts};
use crate::stellar::events::lbv::{EJECTED_MASS_SOLAR_MASSES, ERUPTIONS_PER_YEAR, LbvModel};
use crate::stellar::multiplicity::MultiplicityContext;
use crate::stellar::remnant::collapse::PULSATIONAL_PAIR_INSTABILITY_BLACK_HOLE;
use crate::stellar::sse::wind::HdLimitRules;
use crate::stellar::sse::{PhasePredicate, Track, TrackOptions, fate_of, remnant_of};
use crate::stellar::system::{ClockDeath, StarModel, SystemStars};
use crate::stellar::{Composition, Phase};
use crate::time::{Span, UniverseTime};
use crate::units::{
    Dex, GravitationalParameter, HeliumExcess, Radians, Seconds, SolarMasses, Years,
};
use hyperion_testkit::float::assert_same_bits;

/// The universe of the galaxy fixtures: the brief's tests' own.
const SEED: u64 = 0x6272_6965;

/// The version's stellar rules.
fn version() -> StellarRules {
    StellarRules::of_version(GENERATOR_VERSION)
}

/// The version's stellar rules with this hold's arm alone turned on, so that the other 26 holds
/// (the dwarf sequence's, the helium table's) move nothing the tests hold equal.
fn enforced_rules() -> StellarRules {
    StellarRules {
        hd_limit: HdLimitRules::Enforced,
        ..version()
    }
}

/// The version's galaxy rules with [`enforced_rules`] for the stellar stage's.
fn enforced_galaxy_rules() -> GalaxyRules {
    GalaxyRules {
        stellar: enforced_rules(),
        ..GalaxyRules::of_version(GENERATOR_VERSION)
    }
}

/// Track options under `rules`.
fn options(rules: StellarRules) -> TrackOptions {
    TrackOptions::default().with_rules(rules)
}

/// A star of the sim's \[Fe/H\] `fe_h` and no helium excess.
fn composition(fe_h: f64) -> Composition {
    Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO)
}

/// The full track of `m` M☉ at `fe_h` and the median draws under `rules`.
fn track(m: f64, fe_h: f64, rules: StellarRules) -> Track {
    Track::full_with(
        SolarMasses::new(m),
        &composition(fe_h),
        &StarDraws::median(),
        options(rules),
    )
}

/// The brightest log₁₀ L of `track`'s living states cooler than 7,000 K after its main sequence,
/// over 4,000 even ages of its life, or −∞ for none.
fn cool_ceiling(track: &Track) -> f64 {
    let life = track.lifetime().expect("a full track dies").value();
    (0..4_000)
        .map(|i| track.state_at(Years::new(life * f64::from(i) / 4_000.0)))
        .filter(|s| {
            s.phase() != Phase::MainSequence
                && !s.phase().is_remnant()
                && s.effective_temperature().value() < 7_000.0
        })
        .map(|s| crate::math::log10(s.luminosity().value()))
        .fold(f64::NEG_INFINITY, f64::max)
}

/// P06.T42.b is held until the 25 → 26 bump: the rules in force keep Belczynski et al.'s constant.
#[test]
fn the_hd_limit_is_held_only_until_the_26_bump() {
    assert!(
        GENERATOR_VERSION < HD_LIMIT_VERSION,
        "the 25 → 26 bump puts P06.T42.b's enforced Humphreys–Davidson limit on the record (every \
         evolved star past the limit moves, and the eruptions are thinned): delete \
         `HD_LIMIT_VERSION`, `HdLimitRules` with its `Belczynski` arm, `StellarRules::hd_limit`, \
         the `match` in `wind::rate` (keep `modern` for P06.T42.a's tests), the eruptions' match, \
         `HdLimitRules::fitted_tables_hold` and the brief's and `displaced::site::decide`'s uses \
         of it, this module and \
         `stellar::testing`'s enforced census; move \
         `the_thinned_eruptions_are_the_windows_candidates_at_lbv_states` (`events::lbv`) to the \
         version's rules; rewrite against `modern` \
         `beyond_the_humphreys_davidson_limit_the_modern_rate_is_belczynskis`, \
         `above_the_kink_the_rate_rises_as_gamma_to_the_3_99` and \
         `the_rules_choose_the_modern_rate_past_the_limit`, and the docs' \"the whole of its \
         wind\" and \"the one step left\" (`wind.rs`' Modern section, `MODERN_LBV`, \
         `WindRecipe::Modern`; P06.T42.a's record); refit the tables `just fit-check` flags \
         (stellar_fates_high, lifetime_bracket, kick_rank, kick_defaults, binary_reach, \
         binary_pair_light_e, sky_envelope, sky_phase_envelope) and re-derive the galaxy's \
         parameters; then rerun the LBV and giant-flare censuses (P06.T28.e, T28.c), settling \
         `ERUPTIONS_PER_YEAR` with the ejecta's mass law"
    );
    assert_eq!(version().hd_limit, HdLimitRules::Belczynski);
    assert!(version().hd_limit.fitted_tables_hold());
}

/// The enforced limit turns on at 26 and leaves the other 26 holds' arms there as they are.
#[test]
fn the_enforced_limit_turns_on_at_26() {
    let at = |v| StellarRules::of_version(GeneratorVersion::new(v)).hd_limit;
    for v in [23, 24, 25] {
        assert_eq!(at(v), HdLimitRules::Belczynski, "version {v}");
    }
    for v in [26, 27, 64] {
        assert_eq!(at(v), HdLimitRules::Enforced, "version {v}");
    }
    assert_eq!(
        StellarRules::every_25_hold().hd_limit,
        HdLimitRules::Belczynski
    );
    let at_26 = StellarRules::every_26_hold();
    assert_eq!(at_26.hd_limit, HdLimitRules::Enforced);
    assert!(!at_26.hd_limit.fitted_tables_hold());
    assert_eq!(at_26.kick_rank, KickRankRules::Committed);
    assert_eq!(at_26.dwarf_sequence, DwarfSequenceRules::Reduced);
    assert_eq!(at_26.helium, HeliumRules::Fitted);
    assert_eq!(GalaxyRules::every_26_hold().stellar, at_26);
}

/// D3: a default track's options carry the version's rules, under which a track past the limit
/// is the one it was.
#[test]
fn the_version_s_rules_keep_belczynskis_constant() {
    assert_eq!(TrackOptions::default().rules(), version());
    assert_eq!(
        track(60.0, -1.0, version()),
        Track::full(
            SolarMasses::new(60.0),
            &composition(-1.0),
            &StarDraws::median()
        )
    );
    assert!(!format!("{:?}", TrackOptions::default()).contains("rules"));
    assert!(format!("{:?}", options(enforced_rules())).contains("hd_limit: Enforced"));
}

/// D2, the track's builders: under the enforced limit a 60 M☉ star at \[Fe/H\] −1, a cool
/// supergiant up to log L 6.1 under the constant, keeps no cool state above the wall at log L
/// 5.78–5.80 (Hurley, Pols and Tout's term; ruling 29 of signoff 12), and its builds without
/// segments reach its death and remnant bit for bit; a 20 M☉ star, which never passes the limit,
/// is bit-identical under either arm.
#[test]
fn a_track_takes_its_options_hd_limit() {
    let (held, enforced) = (
        track(60.0, -1.0, version()),
        track(60.0, -1.0, enforced_rules()),
    );
    let (before, after) = (cool_ceiling(&held), cool_ceiling(&enforced));
    assert!(
        before > 6.0,
        "a cool supergiant past the limit: log L {before}"
    );
    assert!(after < 5.80, "no cool state past the wall: log L {after}");
    let (m, comp, draws) = (
        SolarMasses::new(60.0),
        composition(-1.0),
        StarDraws::median(),
    );
    let fate = fate_of(m, &comp, &draws, options(enforced_rules()));
    assert_same_bits(
        fate.death.age().value(),
        enforced.lifetime().expect("dies").value(),
    );
    let (remnant, _) = remnant_of(m, &comp, &draws, options(enforced_rules()));
    assert_eq!(Some(remnant), enforced.remnant_model());
    assert_ne!(enforced.remnant_model(), held.remnant_model());
    let (light, light_enforced) = (
        track(20.0, -1.0, version()),
        track(20.0, -1.0, enforced_rules()),
    );
    assert_eq!(light.remnant_model(), light_enforced.remnant_model());
    for i in 0..400 {
        let age = Years::new(2.5e4 * f64::from(i));
        assert_eq!(light.state_at(age), light_enforced.state_at(age));
    }
}

/// D2, a star's model: a 60 M☉ star of solar metallicity leaves a lighter black hole under the
/// enforced limit, its model built under its rules.
#[test]
fn a_star_s_model_takes_its_rules_hd_limit() {
    let model = |rules| {
        StarModel::new_under(
            SolarMasses::new(60.0),
            composition(0.0),
            StarDraws::median(),
            Years::new(5.0e7),
            rules,
        )
        .expect("in range")
    };
    let (held, enforced) = (model(version()), model(enforced_rules()));
    let mass = |s: &StarModel| s.remnant().expect("a remnant").mass().value();
    assert!(
        mass(&enforced) < mass(&held),
        "{} against {}",
        mass(&enforced),
        mass(&held)
    );
}

/// D2, the engine's pinned death (design note 16): a 60 M☉ primary's death in its pair is its own
/// model's under the same rules, and moves with them.
#[test]
fn a_pair_s_pinned_death_takes_its_input_s_rules() {
    let (m1, m2) = (60.0, 30.0);
    let orbit = KeplerElements::from_period(
        Seconds::new(3_000.0 * 86_400.0),
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
        composition(0.0),
        orbit,
        [StarDraws::median(), StarDraws::median()],
        age,
    )
    .expect("two stars in range");
    let pinned = |rules| {
        crate::stellar::binary::evolve(&input.clone().with_rules(rules), age)
            .supernovae()
            .iter()
            .find(|r| r.component() == Component::Primary)
            .map(SupernovaRecord::remnant)
            .expect("the primary collapses")
            .mass()
            .value()
    };
    let own = |rules| {
        StarModel::new_under(
            SolarMasses::new(m1),
            composition(0.0),
            StarDraws::median(),
            age,
            rules,
        )
        .expect("in range")
        .remnant()
        .expect("a remnant")
        .mass()
        .value()
    };
    let (held, enforced) = (version(), enforced_rules());
    assert_same_bits(pinned(held), own(held));
    assert_same_bits(pinned(enforced), own(enforced));
    assert!(pinned(enforced) < pinned(held));
}

/// The Milky Way fixture under the version's rules.
fn held_galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture builds")
    })
}

/// [`held_galaxy`] under the enforced limit alone ([`enforced_galaxy_rules`]).
fn enforced_galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| held_galaxy().clone().with_rules(enforced_galaxy_rules()))
}

/// Twelve records of each of the first three cells of each layer near the solar circle, and the
/// first four of layer E's there whose primary is born at 45 M☉ or more, past the limit.
fn records(galaxy: &Galaxy) -> Vec<SystemRecord> {
    let mut all = Vec::new();
    let mut cell = Vec::new();
    for layer in [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E] {
        let size = i32::try_from(layer.cell_size_ly()).expect("small");
        for step in 0..3 {
            let key = CellKey::new(layer, [step, 26_000 / size, 0]).expect("a cell");
            generate_cell(galaxy, key, &mut cell);
            all.extend(cell.drain(..).take(12));
        }
    }
    let size = i32::try_from(Layer::E.cell_size_ly()).expect("small");
    let mut massive = 0;
    for step in 0..2_000 {
        let key = CellKey::new(Layer::E, [step, 26_000 / size, 0]).expect("a cell");
        generate_cell(galaxy, key, &mut cell);
        for record in cell.drain(..) {
            if massive < 4 && record.primary_initial_mass().value() >= 45.0 {
                massive += 1;
                all.push(record);
            }
        }
        if massive == 4 {
            break;
        }
    }
    assert_eq!(
        massive, 4,
        "four primaries past the limit near the solar circle"
    );
    all
}

/// D2 and D4, the range brief: under the enforced limit its table route closes and its gate
/// certifies no pair, since the fate, pair-light and reach tables are the version's tracks', and
/// every brief is the system's own under the galaxy's rules; the galaxy built again under them
/// gives the same systems.
#[test]
fn the_brief_builds_its_primaries_under_the_galaxy_s_rules() {
    let galaxy = enforced_galaxy();
    let again = Galaxy::from_params_with_rules(
        Seed::new(SEED),
        GalaxyParams::milky_way_like(),
        enforced_galaxy_rules(),
    )
    .expect("the Milky Way fixture builds");
    let systems = records(held_galaxy());
    assert_eq!(records(galaxy), systems, "the hold moves no record");
    let held = systems
        .iter()
        .map(|r| BriefModel::new(held_galaxy(), r).route())
        .collect::<Vec<_>>();
    assert!(
        held.contains(&BriefRoute::Table),
        "the version's brief reads the fate table"
    );
    // The gate reads the rules only to refuse the version's pair tables, so a system it
    // certifies under the version's rules and refuses under the limit is a pair the hold sent
    // to the paired route.
    let mut refused = 0;
    for (record, held) in systems.iter().zip(held) {
        let model = BriefModel::new(galaxy, record);
        assert_ne!(model.route(), BriefRoute::Table, "{record:?}");
        if held == BriefRoute::Paired {
            assert_eq!(model.route(), BriefRoute::Paired, "{record:?}");
        } else if model.route() == BriefRoute::Paired {
            refused += 1;
        }
        let stars = SystemStars::generate(galaxy, record);
        assert_eq!(
            stars,
            SystemStars::generate(&again, record),
            "D4: {record:?}"
        );
        if model.route() == BriefRoute::Paired {
            continue;
        }
        for y in [-1_000, 0, 1_000] {
            let t = UniverseTime::from_julian_years(y).expect("inside the clock");
            assert_eq!(
                model.brief_at(t),
                stars
                    .brief_at(t)
                    .map(|b| b.with_star_count(model.star_count())),
                "{record:?} at {y} yr"
            );
        }
    }
    assert!(refused > 0, "the gate refused no pair under the limit");
}

/// D2, the explosion site (`displaced::site`): a grid record's site, which the carve-out passes to
/// the core-collapse claim with the record's stars, dies when those stars die under the galaxy's
/// rules, and the limit moves the death.
///
/// The records are the four primaries past the limit, each re-aged to have died about a million
/// years before the epoch.
#[test]
fn the_explosion_site_dies_with_the_galaxy_s_stars() {
    let (held, enforced) = (held_galaxy(), enforced_galaxy());
    let death = |galaxy: &Galaxy, record: &SystemRecord| {
        let stars = SystemStars::generate_in(galaxy, record, MultiplicityContext::ForcedSingle);
        let ClockDeath::At(death, _) = stars.death_time() else {
            panic!(
                "a primary past the limit dies on the clock: {:?}",
                record.id()
            );
        };
        death
    };
    let systems = records(held);
    let mut moved = 0;
    for record in &systems[systems.len() - 4..] {
        let lifetime = death(enforced, record).since_epoch().as_julian_years_f64()
            + record.age_at_epoch().value();
        let dying = SystemRecord::from_parts(
            record.id(),
            *record.epoch_position(),
            record.origin(),
            record.population(),
            record.primary_initial_mass(),
            Years::new(lifetime + 1.0e6),
        );
        let site = |galaxy| {
            explosion_site(galaxy, &dying)
                .expect("a death a million years ago is in the interval")
                .death()
        };
        assert_eq!(site(enforced), death(enforced, &dying), "{:?}", dying.id());
        assert_eq!(site(held), death(held, &dying), "{:?}", dying.id());
        moved += usize::from(site(enforced) != site(held));
    }
    assert!(moved > 0, "the limit moves no site's death");
}

/// D2, the eruptions: under the enforced limit a candidate is kept only where the star is a
/// luminous blue variable, so a 150 M☉ star at \[Fe/H\] −2, whose window under the enforced limit
/// still crosses states that are not one, has no rate there, and the constant elsewhere; under
/// the version's rules the rate is the constant across the window.
#[test]
fn the_eruptions_are_thinned_under_the_enforced_limit() {
    let model = |rules| {
        StarModel::new_under(
            SolarMasses::new(150.0),
            composition(-2.0),
            StarDraws::median(),
            Years::new(2.6e6),
            rules,
        )
        .expect("in range")
    };
    let (held, enforced) = (model(version()), model(enforced_rules()));
    // Each sampled instant's rate per second, the constant's, and whether the star is an LBV.
    let sample = |star: &StarModel| {
        let lbv = LbvModel::of(star).expect("a window");
        let (start, end) = (lbv.start(), lbv.end());
        let span = end.checked_since(start).expect("ordered").as_seconds_f64();
        (0..400)
            .map(|i| {
                let t = start
                    .checked_add(
                        Span::from_seconds_f64(span * (f64::from(i) + 0.5) / 400.0).expect("short"),
                    )
                    .expect("inside the clock");
                let state = star.state_at(t).expect("formed");
                (
                    lbv.rate(t).value(),
                    lbv.rate_inside().value(),
                    is_luminous_blue_variable(&state),
                )
            })
            .collect::<Vec<_>>()
    };
    for (rate, constant, _) in sample(&held) {
        assert_same_bits(rate, constant);
    }
    let mut thinned = 0;
    for (rate, constant, lbv) in sample(&enforced) {
        if lbv {
            assert_same_bits(rate, constant);
        } else {
            assert_same_bits(rate, 0.0);
            thinned += 1;
        }
    }
    assert!(thinned > 0, "no instant of the window was thinned");
}

/// The swap's first check, the remnant masses (plan 06, P06.T42.b): the added term undoes part of
/// Belczynski et al.'s (2010) black-hole calibration. It reports each mass's remnant under either
/// arm at five metallicities, and holds every remnant finite and below the star's own mass, and no
/// heavier than under the constant unless the constant left the star to pair instability.
#[test]
#[ignore = "slow: 100 full tracks of 25–150 M☉ under either arm"]
fn enforced_remnants_are_no_heavier_unless_pair_unstable() {
    for fe_h in [-2.0, -1.0, -0.5, 0.0, 0.18] {
        let mut line = format!("[Fe/H] {fe_h:+.2}:");
        for m in [
            25.0, 30.0, 35.0, 40.0, 50.0, 60.0, 80.0, 100.0, 120.0, 150.0,
        ] {
            let pair = [version(), enforced_rules()].map(|rules| {
                let remnant = track(m, fe_h, rules).remnant_model().expect("a remnant");
                (remnant.phase, remnant.mass.value())
            });
            let [(p0, m0), (p1, m1)] = pair;
            assert!(m1.is_finite() && m1 < m, "{m} M☉: {m1}");
            // A star the constant leaves to pair instability (no remnant, or the pulsations'
            // capped black hole) may lose enough under the limit to collapse whole, heavier.
            let pair_unstable = p0 == Phase::NoRemnant
                || m0
                    .total_cmp(&PULSATIONAL_PAIR_INSTABILITY_BLACK_HOLE.value())
                    .is_eq();
            if p0 == p1 && !pair_unstable {
                assert!(
                    m1 <= m0 + 1e-9,
                    "{m} M☉ at [Fe/H] {fe_h}: {m1} against {m0}"
                );
            }
            // Writing into a `String` cannot fail.
            let _ = write!(line, " {m}: {p0:?} {m0:.2} -> {p1:?} {m1:.2};");
        }
        eprintln!("{line}");
    }
}

/// The swap's second check, the envelope integrator against the term's stiffness, about 10⁴ M☉ a
/// year deep past the line (plan 06, P06.T42.b; the knot cap's fault of plan 06's Risks): every
/// track of 25–150 M☉ at every \[Fe/H\] node of the sky's envelope and three Reimers η builds to
/// its death under the enforced limit with finite states, its largest segment no more than 64 knots
/// above the constant's, and the report states the most knots a segment took.
#[test]
#[ignore = "slow: 1,080 full tracks of 25–150 M☉ under either arm"]
fn the_envelope_integrator_holds_past_the_wall() {
    let (mut most, mut most_held) = ((0, String::new()), 0);
    for &fe_h in &FE_H_NODES {
        for k in 0..30 {
            let m = 25.0 * crate::math::powf(6.0, f64::from(k) / 29.0);
            for eta in [-3.5, 0.0, 3.5] {
                let draws = StarDraws::from_parts(StarDrawsParts {
                    eta: StandardNormal::new(eta).expect("finite"),
                    ..StarDrawsParts::MEDIAN
                });
                let build = |rules| {
                    Track::full_with(
                        SolarMasses::new(m),
                        &composition(fe_h),
                        &draws,
                        options(rules),
                    )
                };
                let (held, enforced) = (build(version()), build(enforced_rules()));
                let knots = |t: &Track| {
                    t.segments()
                        .iter()
                        .map(|s| s.knots.len())
                        .max()
                        .unwrap_or(0)
                };
                let life = enforced.lifetime().expect("a full track dies").value();
                for i in 0..200 {
                    let s = enforced.state_at(Years::new(life * f64::from(i) / 200.0));
                    assert!(
                        s.luminosity().value().is_finite()
                            && s.radius().value().is_finite()
                            && s.mass().value() > 0.0,
                        "{m} M☉ at [Fe/H] {fe_h}, η {eta}: {s:?}"
                    );
                }
                let (k1, k0) = (knots(&enforced), knots(&held));
                assert!(
                    k1 <= k0 + 64,
                    "{m} M☉ at [Fe/H] {fe_h}, η {eta}: {k1} knots against {k0}"
                );
                most_held = most_held.max(k0);
                if k1 > most.0 {
                    most = (k1, format!("{m:.1} M☉ at [Fe/H] {fe_h}, η {eta}"));
                }
            }
        }
    }
    eprintln!(
        "the most knots in a segment under the enforced limit: {} ({}); under the constant: {most_held}",
        most.0, most.1
    );
}

/// The swap's third check, R06's brightness envelope and caps, which read the tracks with no change
/// in R06 (plan 06, P06.T42.b): the envelope's raw nodes from 25 M☉ up, rebuilt under either arm,
/// against the fitted table. It reports how far the enforced tracks' brightest V moves at each node
/// and over every age, and holds the fitted table a bound of every enforced node's bins. The caps
/// read that table, not the tracks, so they stay conservative until the bump refits it.
#[test]
#[ignore = "slow: the sky envelope's nodes from 25 M☉ up, under either arm"]
fn the_brightness_envelope_bounds_the_enforced_tracks() {
    let fitted = BrightnessEnvelope::fitted();
    let (masses, rows) = fitted.rows();
    let (mut violations, mut brighter, mut fainter, mut largest) = (0, 0, 0, (0.0_f64, 0.0_f64));
    for (j, &m) in masses.iter().enumerate().filter(|(_, m)| **m >= 25.0) {
        let raw = |rules| raw_node_under(m, &FE_H_NODES, SAMPLES_PER_PHASE, rules);
        let (held, enforced) = (raw(version()), raw(enforced_rules()));
        for k in 0..held.len() {
            if rows[j][k] > enforced[k] {
                violations += 1;
            }
            if enforced[k] < held[k] {
                brighter += 1;
            } else if enforced[k] > held[k] {
                fainter += 1;
            }
        }
        let ever = |row: &[f64]| row.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let change = ever(&enforced) - ever(&held);
        if change.abs() > largest.0.abs() {
            largest = (change, m);
        }
        eprintln!(
            "node {m:.2} M☉: brightest M_V {:.3} under the constant, {:.3} enforced; table {:.3}",
            ever(&held),
            ever(&enforced),
            ever(&rows[j])
        );
    }
    eprintln!(
        "bins brighter under the enforced limit {brighter}, fainter {fainter}; the largest change of \
         a node's brightest {:.3} mag at {:.2} M☉; bins the fitted table fails to bound {violations}",
        largest.0, largest.1
    );
    assert_eq!(
        violations, 0,
        "the fitted envelope no longer bounds the enforced tracks"
    );
}

/// Research-p06 §8's second item: each LBV window's expected ejecta, the rate × the time in the
/// window at which the star is a luminous blue variable × the marks' mean ejected mass, against
/// the track's own loss over the window (design note 14 puts the eruptions' mass in the wind),
/// under either arm. It reports the ratio per mass and metallicity; under the enforced limit the
/// wall strips tens of solar masses in each window, so the ejecta are a small share of it but for
/// the longest windows of the most massive stars, and it holds the largest ratio finite and at
/// most 1.3, the figure as it stands (design note 14 wants at most 1: a recorded finding).
#[test]
#[ignore = "slow: 126 full tracks with their LBV windows sampled"]
fn the_eruptions_ejecta_stay_near_the_tracks_loss() {
    let (lo, hi) = EJECTED_MASS_SOLAR_MASSES;
    let mean_ejecta = (hi - lo) / crate::math::ln(hi / lo);
    let mut worst = (0.0, String::new());
    for fe_h in [-2.3, -2.0, -1.5, -1.0, -0.5, 0.0, 0.18] {
        for m in [40.0, 45.0, 50.0, 60.0, 70.0, 80.0, 100.0, 120.0, 150.0] {
            // Writing into a `String` cannot fail, so each `write!`'s result is dropped.
            let mut line = format!("[Fe/H] {fe_h:+.2}, {m} M☉:");
            for (name, rules) in [("constant", version()), ("enforced", enforced_rules())] {
                let built = track(m, fe_h, rules);
                let Some(window) = built.window_where(PhasePredicate::Lbv) else {
                    let _ = write!(line, " {name}: no window;");
                    continue;
                };
                let (start, end) = (window.start().value(), window.end().value());
                let samples = 2_000;
                let lbv = (0..samples)
                    .filter(|&i| {
                        let age = start + (end - start) * (f64::from(i) + 0.5) / f64::from(samples);
                        let thinned = rules.hd_limit == HdLimitRules::Enforced;
                        !thinned || is_luminous_blue_variable(&built.state_at(Years::new(age)))
                    })
                    .count();
                #[expect(clippy::cast_precision_loss, reason = "counts below 2,001")]
                let share = lbv as f64 / f64::from(samples);
                let ejecta = ERUPTIONS_PER_YEAR * (end - start) * share * mean_ejecta;
                let lost = built.state_at(Years::new(start)).mass().value()
                    - built.state_at(Years::new(end)).mass().value();
                let ratio = ejecta / lost;
                let _ = write!(
                    line,
                    " {name}: window {:.3e} yr, LBV {share:.3}, ejecta {ejecta:.2} M☉ against \
                     {lost:.2} lost ({ratio:.3});",
                    end - start
                );
                if rules.hd_limit == HdLimitRules::Enforced && ratio.total_cmp(&worst.0).is_gt() {
                    worst = (ratio, format!("{m} M☉ at [Fe/H] {fe_h}"));
                }
            }
            eprintln!("{line}");
        }
    }
    eprintln!(
        "the largest ejecta over the track's loss under the enforced limit: {:.3} ({})",
        worst.0, worst.1
    );
    assert!(
        worst.0.is_finite() && worst.0 <= 1.3,
        "the eruptions outweigh the track's loss: {:.3} ({})",
        worst.0,
        worst.1
    );
}
