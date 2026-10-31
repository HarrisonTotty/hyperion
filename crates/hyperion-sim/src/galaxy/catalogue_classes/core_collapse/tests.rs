//! The core-collapse class's tests (P09.T33.a): the young field it follows and its bounds, its
//! candidates against the field's own intensity, its entries against the one shared test, their
//! motion and reach, resolution and order independence, its headroom, plan 03's resolve, and a
//! golden. The galaxy-wide counts, the brainstorm's 20–160 core collapses in ±H over seeds and the
//! entries beyond the shells, with every entry's window and reach, are a slow census; under every
//! 26 hold, where the field gives up φ (P09.T2.c), it is restated on the class's own rate and the
//! budget's deaths weighed by `1 − φ`.

use std::sync::OnceLock;
#[cfg(not(target_family = "wasm"))]
use std::thread;

use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::lcg::Lcg;
use hyperion_testkit::order::assert_order_independent;
use hyperion_testkit::stats::{ALPHA, assert_p_value, assert_poisson_count, normal_cdf};

use super::*;
use crate::GENERATOR_VERSION;
use crate::Seed;
use crate::galaxy::ages::FieldShareRules;
use crate::galaxy::catalogue_classes::grid::candidate_count;
use crate::galaxy::catalogue_classes::supernova::{
    LIFETIME_REACH, SupernovaKind, SupernovaState, claims, lifetime_cap,
};
use crate::galaxy::catalogue_classes::{
    CatalogueClassCell, CatalogueEntry, check_class_headroom, resolve_entry, system_of,
};
use crate::galaxy::displaced::marks::LifetimeBracket;
use crate::galaxy::field_share::FieldShare;
use crate::galaxy::imf::MassFunctionKind;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{self, ResolveSystemError};
use crate::galaxy::rules::GalaxyRules;
use crate::id::Layer;
use crate::stellar::remnant::SupernovaType;
use crate::time::ClockWindow;
use crate::units::consts::METRES_PER_LIGHT_YEAR;

/// The seed of the Milky Way fixture most tests share.
const SEED: u64 = 0x0933_0000_6363_6f6c;

/// The fixture, with its kinematic tables, so that candidates move; built once.
fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| milky_way(SEED))
}

/// The Milky Way fixture of `seed`, with its kinematic tables.
fn milky_way(seed: u64) -> Galaxy {
    Galaxy::from_params(Seed::new(seed), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral")
        .with_full_potential()
}

/// The class cell `cell`.
fn key(cell: [i32; 3]) -> CatalogueCellKey {
    CatalogueCellKey::new(ClassId::CORE_COLLAPSE, cell).expect("a cell of the root cube")
}

/// Cells with entries: a run along +y through the inner disc, in the plane, and one at the centre,
/// where the nuclear disc's are.
fn sample_cells() -> Vec<CatalogueCellKey> {
    (8..28)
        .map(|y| key([0, y, 0]))
        .chain([key([0, 0, 0])])
        .collect()
}

/// The sample cells' entries, generated once.
fn sample_entries() -> &'static [(CatalogueCellKey, Vec<CatalogueEntry<CoreCollapseEntry>>)] {
    static ENTRIES: OnceLock<Vec<(CatalogueCellKey, Vec<CatalogueEntry<CoreCollapseEntry>>)>> =
        OnceLock::new();
    ENTRIES.get_or_init(|| {
        let galaxy = galaxy();
        let process = CoreCollapseProcess::new(galaxy);
        sample_cells()
            .into_iter()
            .map(|key| {
                let entries = CatalogueClassCell::generate(galaxy, &process, key).into_entries();
                (key, entries)
            })
            .collect()
    })
}

/// The instant a second before `t`: on a broken line's first leg when `t` is its break.
fn just_before(t: UniverseTime) -> UniverseTime {
    t.checked_sub(Span::from_seconds(1))
        .expect("inside the clock's range")
}

/// −H, the young window's lower end, Julian years.
fn minus_h() -> Years {
    Years::new(-CLOCK_WINDOW_H.as_julian_years_f64())
}

// 1. The young field and its bounds.

/// The class follows the young thin disc and the nuclear disc's two components, every component
/// with layer-E systems younger than [`RECENT_DEATH_AGES`], each with its own largest death rate;
/// no star of band E lives within the interval of that limit.
#[test]
fn the_young_field_is_the_young_disc_and_the_nuclear_disc() {
    let galaxy = galaxy();
    let process = CoreCollapseProcess::new(galaxy);
    let fields = galaxy.fields();
    let populations: Vec<Population> = process
        .components()
        .iter()
        .map(|young| fields.component(young.id()).population())
        .collect();
    assert_eq!(
        populations,
        [
            Population::YoungThinDisc,
            Population::NuclearDisc,
            Population::NuclearDisc
        ]
    );
    for id in fields.component_ids() {
        if process.components().iter().all(|young| young.id() != id) {
            let ages = fields.component(id).ages();
            assert!(ages.min() >= RECENT_DEATH_AGES, "{id:?} is young");
        }
    }
    let years = process.interval().length().as_julian_years_f64();
    let (_, longest) = LifetimeBracket::shared()
        .bracket(SolarMasses::new(8.0))
        .expect("the bracket holds 8 M☉");
    assert!(
        longest.value() + years < RECENT_DEATH_AGES.value(),
        "an 8 M☉ star lives up to {} yr",
        longest.value()
    );
    for young in process.components() {
        let ages = fields.component(young.id()).ages();
        let rate = ages.max_pdf_between(minus_h(), RECENT_DEATH_AGES);
        assert_same_bits(
            young.deaths_per_system(),
            rate * (1.0 + RATE_MARGIN) * years,
        );
        assert_same_bits(
            young.share(),
            galaxy
                .shares()
                .component_share(MassBand::E, fields.component(young.id())),
        );
        // No age the window reaches has a density above the bound.
        for k in 0..=20_000 {
            let age = minus_h().value()
                + (RECENT_DEATH_AGES.value() - minus_h().value()) * f64::from(k) / 20_000.0;
            assert!(
                ages.pdf(Years::new(age)) * years <= young.deaths_per_system(),
                "{age}"
            );
        }
    }
    // The young disc's death rate: about one death per young system per 100 Myr, so some 3.5% of
    // them over the fixture's 3.43 Myr interval; the nuclear disc's a twentieth of its stars over
    // its last gigayear, some two hundred times less.
    let [young, nuclear, inner] = process.components() else {
        panic!("three components");
    };
    assert!(
        (0.03..0.05).contains(&young.deaths_per_system()),
        "{young:?}"
    );
    assert_same_bits(nuclear.deaths_per_system(), inner.deaths_per_system());
    let ratio = young.deaths_per_system() / nuclear.deaths_per_system();
    assert!((150.0..250.0).contains(&ratio), "{ratio}");
}

/// The thinning's envelope holds: at random points of random cells, from the centre out through
/// the disc and above it, each component's term and their running total lie under the cell's
/// bound.
#[test]
fn the_envelope_holds_over_random_points() {
    let galaxy = galaxy();
    let process = CoreCollapseProcess::new(galaxy);
    let mut lcg = Lcg::new(0x0933_0001);
    let mut weights = [0.0; MAX_COMPONENTS];
    let mut tested = 0;
    for _ in 0..3_000 {
        let span = |lcg: &mut Lcg, n: u64| {
            i32::try_from(lcg.next_below(2 * n)).unwrap() - i32::try_from(n).unwrap()
        };
        let cell = key([span(&mut lcg, 128), span(&mut lcg, 128), span(&mut lcg, 4)]);
        let bound = process.bound(galaxy, &cell).value();
        for _ in 0..8 {
            let p = cell.position_from_words([lcg.next_u64(), lcg.next_u64(), lcg.next_u64()]);
            let n = process.weights(galaxy, &PointLy::from(&p), &mut weights);
            let total = weights[..n].iter().fold(0.0, |sum, w| sum + w);
            assert!(total <= bound, "{total} above {bound} in {cell:?}");
            tested += 1;
        }
    }
    // The cells about the centre, where the nuclear disc peaks.
    for cell in [[0, 0, 0], [-1, -1, -1], [0, -1, 0], [-1, 0, -1]] {
        let cell = key(cell);
        let bound = process.bound(galaxy, &cell).value();
        let corner = cell.position_from_words([0, 0, 0]);
        let n = process.weights(galaxy, &PointLy::from(&corner), &mut weights);
        assert!(weights[..n].iter().fold(0.0, |sum, w| sum + w) <= bound);
        assert!(bound > 0.0);
        tested += 1;
    }
    assert_eq!(tested, 24_004);
}

/// No cell of the class comes near the 24-bit index: the bound over each octant of the root cube,
/// whose nearest corner is the centre, bounds every cell in it, and passes the headroom check.
#[test]
fn no_cell_comes_near_the_index() {
    let galaxy = galaxy();
    let process = CoreCollapseProcess::new(galaxy);
    let fields = galaxy.fields();
    let half = 65_536;
    let mut largest = 0.0_f64;
    for octant in 0..8 {
        let min = [0, 1, 2].map(|axis| if octant >> axis & 1 == 1 { -half } else { 0 });
        let cube = CellBox::new(min, 65_536).expect("an octant of the root cube");
        let bound = process.components().iter().fold(0.0, |sum, young| {
            sum + young.share()
                * fields.component_bound(young.id(), &cube)
                * young.deaths_per_system()
        });
        largest = largest.max(bound);
    }
    // The centre's cell is the fullest, and the octants' bound is its own.
    let centre = process.bound(galaxy, &key([0, 0, 0])).value();
    assert!(centre <= largest);
    eprintln!(
        "the fullest cell expects at most {:.0} candidates; the centre's {:.0}",
        largest * key([0, 0, 0]).volume_ly3(),
        centre * key([0, 0, 0]).volume_ly3()
    );
    assert_eq!(
        check_class_headroom(ClassId::CORE_COLLAPSE, PerCubicLightYear::new(largest)),
        Ok(())
    );
}

// 2. The candidates follow the field's own intensity.

/// The candidates the thinning keeps are the young field's recent deaths: in each component, as
/// many as its layer-E systems × its deaths per system over the interval, integrated over the
/// cells (a midpoint rule of 40³ points per cell), at α = 10⁻³. The nuclear disc's age density is
/// flat over its last gigayear, so its expectation is exact; the young disc's rises by under 1%
/// over its stars' death ages and is taken at 25 Myr.
#[test]
fn candidates_follow_the_young_field_s_recent_deaths() {
    const STEPS: u32 = 40;
    let galaxy = galaxy();
    let process = CoreCollapseProcess::new(galaxy);
    let fields = galaxy.fields();
    let years = process.interval().length().as_julian_years_f64();
    let cells: Vec<CatalogueCellKey> = (8..28)
        .map(|y| key([0, y, 0]))
        .chain([[0, 0, 0], [-1, -1, -1], [0, -1, -1], [-1, 0, 0]].map(key))
        .collect();
    let n = process.components().len();
    let mut observed = vec![0_u64; n];
    let mut expected = vec![0.0; n];
    for cell in &cells {
        let bound = process.bound(galaxy, cell);
        for index in 0..candidate_count(galaxy.seed(), cell, bound) {
            let id = cell.candidate_id(index, 0).unwrap();
            if let Some(candidate) = process.candidate(galaxy, cell, id, bound) {
                let slot = process
                    .components()
                    .iter()
                    .position(|young| young.id() == candidate.component)
                    .unwrap();
                observed[slot] += 1;
            }
        }
        let origin = cell.origin_ly();
        let step = f64::from(cell.edge_ly()) / f64::from(STEPS);
        let volume = step * step * step;
        for i in 0..STEPS {
            for j in 0..STEPS {
                for k in 0..STEPS {
                    let at =
                        |axis: usize, m: u32| f64::from(origin[axis]) + (f64::from(m) + 0.5) * step;
                    let p = PointLy::new(at(0, i), at(1, j), at(2, k));
                    for (slot, young) in process.components().iter().enumerate() {
                        let laws = fields.component(young.id());
                        let deaths = laws.ages().pdf(Years::new(2.5e7)) * years;
                        expected[slot] += young.share() * laws.density(&p) * deaths * volume;
                    }
                }
            }
        }
    }
    for (slot, young) in process.components().iter().enumerate() {
        eprintln!(
            "{:?}: {} candidates kept against {:.1} expected",
            young.id(),
            observed[slot],
            expected[slot]
        );
    }
    assert_poisson_count(
        "the young disc's kept candidates",
        observed[0],
        expected[0],
        ALPHA,
    );
    let nuclear = observed[1] + observed[2];
    assert_poisson_count(
        "the nuclear disc's kept candidates",
        nuclear,
        expected[1] + expected[2],
        ALPHA,
    );
    assert!(observed[0] > 50 && nuclear > 500, "{observed:?}");
}

/// A candidate's \[Fe/H\] is its component's law at its site read at age zero, one normal on
/// `class.core_collapse.metallicity` (the module documentation's step 2, P09.T33.a's lean), to the
/// bit; and both young components' laws are flat in age, so that read gives the bits a read at the
/// candidate's age at the epoch would, the age plan 06's `draw_metallicity` reads a grid record's
/// law at, and a read at [`RECENT_DEATH_AGES`] too (determinism audit). A slope in either law fails
/// the second half: the class must then read its candidate's age, which moves its draws. The
/// candidates of a cell of the young disc and of the cell at the centre, every component's.
#[test]
fn a_candidate_s_metallicity_is_read_at_age_zero() {
    let galaxy = galaxy();
    let process = CoreCollapseProcess::new(galaxy);
    let fields = galaxy.fields();
    let mut checked = vec![0_u32; process.components().len()];
    for cell in [key([0, 16, 0]), key([0, 0, 0])] {
        let bound = process.bound(galaxy, &cell);
        for index in 0..candidate_count(galaxy.seed(), &cell, bound) {
            let id = cell.candidate_id(index, 0).unwrap();
            let Some(candidate) = process.candidate(galaxy, &cell, id, bound) else {
                continue;
            };
            let laws = fields.component(candidate.component);
            let site = PointLy::from(&candidate.site);
            let at_zero = laws.metallicity(&site, Years::ZERO);
            let object = ObjectKey::from(SystemId::from(id));
            let mut stream =
                Stream::open(galaxy.seed(), tags::CLASS_CORE_COLLAPSE_METALLICITY, object);
            let feh = stream.normal(at_zero.mean().value(), at_zero.sigma().value());
            assert_eq!(
                candidate.composition,
                Composition::from_fe_h(Dex::new(feh), HeliumExcess::ZERO),
                "{id:?}"
            );
            for age in [candidate.age_at_epoch, RECENT_DEATH_AGES] {
                let at_age = laws.metallicity(&site, age);
                assert_same_bits(at_age.mean().value(), at_zero.mean().value());
                assert_same_bits(at_age.sigma().value(), at_zero.sigma().value());
            }
            let slot = process
                .components()
                .iter()
                .position(|young| young.id() == candidate.component)
                .unwrap();
            checked[slot] += 1;
        }
    }
    eprintln!("candidates checked, by component: {checked:?}");
    assert!(checked.iter().all(|&n| n > 0), "{checked:?}");
}

// 3. The entries.

/// Every entry is the one shared test's: plan 08's interval predicate puts its death at its T, its
/// marks are `DeathMarks::of` its stars at its site with its one velocity, `claims` holds in the
/// field's gas and its window is the one `claimed_window` cut, under the field's cap and its
/// lifetime cap. Its record is a catalogue system's, counted in layer E, with no component.
#[test]
fn every_entry_is_the_shared_test_s() {
    let galaxy = galaxy();

    let cap = galaxy.window_caps().cap(ShellEnvironment::Field);
    let mut entries = 0;
    for (cell, cell_entries) in sample_entries() {
        for entry in cell_entries {
            let value = entry.value();
            let record = value.record();
            let system = SystemId::from(entry.id());
            assert_eq!(record.id(), system);
            assert_eq!(
                record.origin(),
                SystemOrigin::CatalogueSystem(ClassId::CORE_COLLAPSE)
            );
            assert_eq!(record.component(), None);
            assert_eq!(record.layer(), Layer::E);
            assert_eq!(
                record.population(),
                galaxy.fields().component(value.component()).population()
            );
            let site = value.site();
            assert_eq!(
                CatalogueCellKey::containing(ClassId::CORE_COLLAPSE, site.position()),
                Some(*cell)
            );
            // The predicate, on the record's own marks.
            let draws = StarDraws::for_star(galaxy.seed(), BodyId::new(system, 0));
            let death = death_in_interval(
                record.primary_initial_mass(),
                value.composition(),
                draws.eta(),
                record.age_at_epoch(),
                || draws.clone(),
            );
            assert_eq!(death, Some(site.death()));
            assert_eq!(value.supernova().explosion(), site.death());
            // The marks, from the full system's stars and the one velocity.
            let stars = value.stars(galaxy);
            let before = value.motion().velocity_at(just_before(site.death()));
            let velocity = site_velocity(
                galaxy,
                value.component(),
                site.position(),
                ObjectKey::from(system),
            );
            assert_eq!(before, velocity, "one velocity");
            let marks = DeathMarks::of(galaxy, &stars, site, velocity).expect("a shell");
            assert_eq!(&marks, value.marks());
            assert!(matches!(
                marks.kind(),
                SupernovaKind::CoreCollapse(kind) if kind != SupernovaType::None
            ));
            assert!(claims(galaxy, site, &marks, Surroundings::Field));
            let window = claimed_window(galaxy, site, &marks, Surroundings::Field).unwrap();
            assert_eq!(value.supernova().window(), &window);
            assert!(window.duration() <= cap, "{:?}", window.duration());
            assert!(
                window.duration().value() <= lifetime_cap(marks.fastest_member_speed()).value()
            );
            // The remnant takes the kick at T.
            let kick = stars.natal_kick().map_or([0.0; 3], |kick| kick.velocity());
            assert_eq!(
                value.motion().velocity_at(site.death()),
                velocity + GalacticVelocity::new(kick)
            );
            entries += 1;
        }
    }
    eprintln!("{entries} entries in {} cells", sample_entries().len());
    assert!(entries >= 20, "{entries}");
}

/// No entry's system travels more than 4,096 ly from its site within its lifetime, from its
/// interval's start to +H: the broken line is farthest from its site at one of its ends.
#[test]
fn no_entry_travels_past_the_reach() {
    let mut farthest = 0.0_f64;
    for (_, entries) in sample_entries() {
        for entry in entries {
            let value = entry.value();
            let interval = SupernovaInterval::of_window(value.supernova().window().duration());
            let site = value.site().position();
            for t in [interval.start(), interval.end()] {
                let p = value.motion().position_at(t);
                let ly = site.distance_to(&p).value() / METRES_PER_LIGHT_YEAR;
                farthest = farthest.max(ly);
                assert!(
                    ly <= LIFETIME_REACH.value(),
                    "{:?} is {ly} ly from its site",
                    entry.id()
                );
            }
            // The record is where the line puts it at the epoch.
            assert_eq!(
                value.motion().position_at(UniverseTime::EPOCH),
                *value.record().epoch_position()
            );
            assert_eq!(value.motion().position_at(value.site().death()), *site);
        }
    }
    eprintln!("the farthest an entry travels from its site: {farthest:.0} ly");
}

/// An entry resolves to itself bit for bit by its ID: through the class's process, through the
/// production lookup's catalogue system, and through plan 03's resolve, which returns its record;
/// its next member names nothing, and neither does an index past its cell's count.
#[test]
fn entries_resolve_to_themselves() {
    let galaxy = galaxy();
    let process = CoreCollapseProcess::new(galaxy);
    for (cell, entries) in sample_entries() {
        for entry in entries {
            let member = resolve_entry(galaxy, &process, entry.id()).unwrap();
            assert_eq!(member.entry(), entry);
            let system = system_of(galaxy, entry.id()).unwrap();
            assert_eq!(system.record(), entry.value().record());
            assert_eq!(system.motion(), entry.value().motion());
            assert_eq!(
                system.star_model(),
                Some((entry.value().composition(), MultiplicityContext::Free))
            );
            assert_eq!(
                placement::resolve(galaxy, SystemId::from(entry.id())),
                Ok(*entry.value().record())
            );
            let next = CatalogueSystemId::new(
                entry.id().class(),
                entry.id().cell(),
                entry.id().index(),
                1,
            )
            .unwrap();
            assert_eq!(
                resolve_entry(galaxy, &process, next),
                Err(ResolveSystemError::NoSuchSystem)
            );
            assert_eq!(
                placement::resolve(galaxy, SystemId::from(next)),
                Err(ResolveSystemError::NoSuchSystem)
            );
        }
        let count = CatalogueClassCell::generate(galaxy, &process, *cell).candidates();
        let past = cell.candidate_id(count, 0).unwrap();
        assert_eq!(
            placement::resolve(galaxy, SystemId::from(past)),
            Err(ResolveSystemError::NoSuchSystem)
        );
    }
}

/// The rest of the sim reads an entry through its class (P09.T33.a): plan 06's stars, through
/// `stars_of`, and their range brief, which is theirs on every route; the planetary context; the
/// lensing mass; plan 03's position and epoch velocity, its class's line; the loss cone, which its
/// line does not meet; and the observer's drift, which refuses a broken line.
#[test]
fn the_rest_of_the_sim_reads_an_entry_through_its_class() {
    use crate::galaxy::features::members::NoInteriorCache;
    use crate::galaxy::motion::{EpochState, LossConeVerdict, loss_cone_verdict};
    use crate::galaxy::query::{epoch_velocity, position_at};
    use crate::lensing::lens_mass_at;
    use crate::observe::{Drift, TraceMotionError, stars_of};
    use crate::planetary::SystemContext;
    use crate::stellar::brief::{BriefModel, BriefRoute};
    let galaxy = galaxy();
    let mut checked = 0;
    for (_, entries) in sample_entries() {
        for entry in entries.iter().take(2) {
            let record = entry.value().record();
            let system = system_of(galaxy, entry.id()).unwrap();
            let stars = system.stars(galaxy).unwrap();
            assert_eq!(
                stars_of(galaxy, &NoInteriorCache, record),
                Ok(stars.clone())
            );
            let brief = BriefModel::of_record(galaxy, &NoInteriorCache, record).unwrap();
            assert_eq!(brief.star_count(), stars.star_count());
            let death = entry.value().site().death();
            let times = [
                ClockWindow::START,
                UniverseTime::EPOCH,
                ClockWindow::END,
                death,
            ];
            for t in times.into_iter().filter(|&t| ClockWindow::contains(t)) {
                let (ranged, exact) = (brief.brief_at(t), stars.brief_at(t));
                if brief.route() == BriefRoute::Table {
                    assert_eq!(
                        ranged.map(|b| (b.kind(), b.class().to_string())),
                        exact.map(|b| (b.kind(), b.class().to_string()))
                    );
                } else {
                    assert_eq!(ranged, exact, "{:?} at {t}", entry.id());
                }
            }
            assert_eq!(
                SystemContext::for_system(galaxy, record.id()),
                Ok(SystemContext::from_stars(galaxy, &stars))
            );
            assert!(lens_mass_at(galaxy, record, UniverseTime::EPOCH).is_some());
            for t in [ClockWindow::START, ClockWindow::END] {
                assert_eq!(position_at(galaxy, record, t), system.position_at(t));
            }
            let velocity = epoch_velocity(galaxy, record);
            assert_eq!(velocity, system.motion().velocity_at(UniverseTime::EPOCH));
            let at_epoch = EpochState::new(*record, velocity).unwrap();
            assert_eq!(loss_cone_verdict(galaxy, &at_epoch), LossConeVerdict::Clear);
            assert_eq!(
                Drift::of_record(galaxy, record),
                Err(TraceMotionError::CatalogueMotionNotTraced(record.id()))
            );
            checked += 1;
        }
    }
    assert!(checked >= 10, "{checked}");
}

/// A candidate the thinning keeps but whose death makes no shell, or falls before its window's
/// interval, is no entry, and its ID names nothing; both kinds occur.
#[test]
fn a_death_without_a_shell_or_too_early_is_thinned() {
    let galaxy = galaxy();
    let process = CoreCollapseProcess::new(galaxy);
    let (mut no_shell, mut too_early, mut kept) = (0, 0, 0);
    for cell in sample_cells() {
        let bound = process.bound(galaxy, &cell);
        let count = CatalogueClassCell::generate(galaxy, &process, cell).candidates();
        for index in 0..count {
            let id = cell.candidate_id(index, 0).unwrap();
            let Some(candidate) = process.candidate(galaxy, &cell, id, bound) else {
                continue;
            };
            match RecentDeath::of(galaxy, &candidate) {
                None => no_shell += 1,
                Some(death) => {
                    if claimed_window(galaxy, &death.site, &death.marks, Surroundings::Field)
                        .is_some()
                    {
                        kept += 1;
                        continue;
                    }
                    too_early += 1;
                }
            }
            assert!(process.test(galaxy, candidate).is_none());
            assert_eq!(
                resolve_entry(galaxy, &process, id),
                Err(ResolveSystemError::NoSuchSystem)
            );
        }
    }
    eprintln!(
        "{kept} kept, {no_shell} without a shell, {too_early} before their window's interval"
    );
    assert!(
        no_shell > 0 && too_early > kept,
        "{no_shell} {too_early} {kept}"
    );
}

/// The cells do not depend on the order they are generated in, nor on what was resolved between
/// them; the same seed gives the same cells in a fresh galaxy, and another seed other cells.
#[test]
fn cells_do_not_depend_on_order() {
    let galaxy = galaxy();
    let process = CoreCollapseProcess::new(galaxy);
    let cells: Vec<CatalogueCellKey> = (8..16).map(|y| key([0, y, 0])).collect();
    assert_order_independent(&cells, |key| {
        // A stray resolve of another class's ID first, whose process is not built yet.
        let stray =
            CatalogueSystemId::new(ClassId::XRAY_BINARY.value(), key.fine_cell(), 0, 0).unwrap();
        assert_eq!(
            placement::resolve(galaxy, SystemId::from(stray)),
            Err(ResolveSystemError::KindNotGenerated)
        );
        CatalogueClassCell::generate(galaxy, &process, *key)
    });
    let fresh = |seed: u64| {
        let galaxy = Galaxy::new(Seed::new(seed));
        let process = CoreCollapseProcess::new(&galaxy);
        cells
            .iter()
            .map(|key| CatalogueClassCell::generate(&galaxy, &process, *key))
            .collect::<Vec<_>>()
    };
    let once = fresh(0x0933_0002);
    assert_eq!(once, fresh(0x0933_0002));
    let counts = |cells: &[CatalogueClassCell<CoreCollapseEntry>]| -> Vec<u32> {
        cells.iter().map(CatalogueClassCell::candidates).collect()
    };
    assert_ne!(counts(&once), counts(&fresh(0x0933_0003)));
}

/// A galaxy built without its kinematic tables has no velocities: its candidates rest, and its
/// entries move only by their kicks, as plan 08's `epoch_velocity` has it.
#[test]
fn a_galaxy_without_kinematics_has_resting_progenitors() {
    let galaxy = Galaxy::new(Seed::new(0x0933_0004));
    let process = CoreCollapseProcess::new(&galaxy);
    let mut seen = 0;
    for y in 8..28 {
        for entry in CatalogueClassCell::generate(&galaxy, &process, key([0, y, 0])).entries() {
            let motion = entry.value().motion();
            assert_eq!(
                motion.velocity_at(just_before(motion.at())),
                GalacticVelocity::default()
            );
            seen += 1;
        }
    }
    assert!(seen > 0);
}

// 4. The golden.

/// Pins the class on the fixture: its components and rates, its interval, and a run of cells
/// across the inner disc and at the centre: each one's bound, its candidate count and every
/// entry's ID, death, kind, window, mass, age, metallicity, sites and velocities.
#[test]
fn core_collapse_golden() {
    let galaxy = galaxy();
    let process = CoreCollapseProcess::new(galaxy);
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for young in process.components() {
        let label = format!("component {}", young.id().index());
        w.f64(&format!("{label} share"), young.share());
        w.f64(
            &format!("{label} deaths per system"),
            young.deaths_per_system(),
        );
    }
    let interval = process.interval();
    w.line(&format!(
        "interval ({}, {}]",
        interval.start(),
        interval.end()
    ));
    for (cell, entries) in sample_entries() {
        let label = format!("cell {:?}", cell.cell());
        w.f64(
            &format!("{label} bound"),
            process.bound(galaxy, cell).value(),
        );
        let count = CatalogueClassCell::generate(galaxy, &process, *cell).candidates();
        w.line(&format!(
            "{label} candidates {count} entries {}",
            entries.len()
        ));
        for entry in entries {
            let value = entry.value();
            let record = value.record();
            let supernova = value.supernova();
            let id = format!("{label} {:016x}", SystemId::from(entry.id()).raw());
            w.line(&format!(
                "{id} component {} explodes {} as {:?}",
                value.component().index(),
                supernova.explosion(),
                supernova.kind()
            ));
            w.f64(
                &format!("{id} window"),
                supernova.window().duration().value(),
            );
            w.f64(&format!("{id} mass"), record.primary_initial_mass().value());
            w.f64(&format!("{id} age"), record.age_at_epoch().value());
            w.f64(&format!("{id} feh"), value.composition().fe_h().value());
            w.f64(&format!("{id} energy"), value.marks().energy().foes());
            w.f64(
                &format!("{id} fastest"),
                value.marks().fastest_member_speed().value(),
            );
            // A position exactly: its whole light-years and its offset in metres, each bit.
            for (name, p) in [
                ("site", value.site().position()),
                ("epoch", record.epoch_position()),
            ] {
                w.line(&format!("{id} {name} cell {:?}", p.cell().to_array()));
                for (axis, metres) in p.offset_metres().iter().enumerate() {
                    w.f64(&format!("{id} {name} offset {axis}"), *metres);
                }
            }
            for (name, t) in [
                ("before", just_before(value.motion().at())),
                ("after", value.motion().at()),
            ] {
                for (axis, v) in value
                    .motion()
                    .velocity_at(t)
                    .metres_per_second()
                    .iter()
                    .enumerate()
                {
                    w.f64(&format!("{id} velocity {name} {axis}"), *v);
                }
            }
        }
    }
    golden!("catalogue_classes/core_collapse", w.as_str());
}

// 5. The census (slow).

/// What a galaxy's class holds, counted over every cell.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Census {
    /// Candidates the thinning kept.
    candidates: u32,
    /// Of those, deaths with a shell in the field's interval.
    deaths: u32,
    /// Of those, entries: the deaths `claims` keeps.
    entries: u32,
    /// Entries that are a shell at the epoch: a supernova of some age.
    shells: u32,
    /// Entries exploding in ±H.
    in_window: u32,
    /// The longest window, Julian years, and the farthest any system travels from its site, ly.
    longest: f64,
    farthest: f64,
    /// The deaths weighted by the share of their component's systems of their age that another
    /// galaxy's field keeps ([`census_weighed`]): the field's deaths that galaxy's class expects.
    kept: f64,
    /// The sum of those weights' squares: the weighed count's own sampling variance, less its
    /// share of the deaths' Poisson variance.
    kept_squares: f64,
}

impl Census {
    fn add(&mut self, other: &Self) {
        self.candidates += other.candidates;
        self.deaths += other.deaths;
        self.entries += other.entries;
        self.shells += other.shells;
        self.in_window += other.in_window;
        self.longest = self.longest.max(other.longest);
        self.farthest = self.farthest.max(other.farthest);
        self.kept += other.kept;
        self.kept_squares += other.kept_squares;
    }
}

/// A mean below which a cell's candidates are not drawn: 10⁻⁹, so that every cell skipped holds,
/// together, under 0.02 candidates of the root cube's 2²⁴ cells.
const NEGLIGIBLE_MEAN: f64 = 1e-9;

/// The census of `galaxy`'s class: every cell of the root cube whose mean is not negligible, a
/// column of cells at a time from the plane outwards (the bound never rises away from the plane),
/// on four threads. Every entry's window is checked against its caps and its reach on the way.
fn census(galaxy: &Galaxy) -> Census {
    census_weighed(galaxy, None)
}

/// [`census`], with each death weighed by the share of its component's systems of layer E and
/// its age that `field`'s field keeps, if a field is given (P09.T2.c).
fn census_weighed(galaxy: &Galaxy, field: Option<&FieldShare>) -> Census {
    let process = CoreCollapseProcess::new(galaxy);
    let cap = galaxy.window_caps().cap(ShellEnvironment::Field);
    let column = |x: i32, y: i32, census: &mut Census| {
        for (start, step) in [(0, 1), (-1, -1)] {
            let mut z = start;
            while (-128..128).contains(&z) {
                let cell = key([x, y, z]);
                let bound = process.bound(galaxy, &cell);
                if bound.value() * cell.volume_ly3() < NEGLIGIBLE_MEAN {
                    break;
                }
                for index in 0..candidate_count(galaxy.seed(), &cell, bound) {
                    let id = cell.candidate_id(index, 0).unwrap();
                    let Some(candidate) = process.candidate(galaxy, &cell, id, bound) else {
                        continue;
                    };
                    census.candidates += 1;
                    let Some(death) = RecentDeath::of(galaxy, &candidate) else {
                        continue;
                    };
                    census.deaths += 1;
                    if let Some(field) = field {
                        let keeps =
                            field.keeps(candidate.component, MassBand::E, candidate.age_at_epoch);
                        census.kept += keeps;
                        census.kept_squares += keeps * keeps;
                    }
                    let Some(window) =
                        claimed_window(galaxy, &death.site, &death.marks, Surroundings::Field)
                    else {
                        continue;
                    };
                    let entry = death.into_entry(&candidate, window);
                    census.entries += 1;
                    let duration = window.duration().value();
                    assert!(window.duration() <= cap, "{id:?}: {duration} yr");
                    assert!(duration <= lifetime_cap(entry.marks.fastest_member_speed()).value());
                    census.longest = census.longest.max(duration);
                    if matches!(
                        entry.supernova.state_at(UniverseTime::EPOCH),
                        SupernovaState::Supernova { .. }
                    ) {
                        census.shells += 1;
                    }
                    if ClockWindow::contains(entry.site.death()) {
                        census.in_window += 1;
                    }
                    let interval = SupernovaInterval::of_window(window.duration());
                    for t in [interval.start(), interval.end()] {
                        let ly = entry
                            .site
                            .position()
                            .distance_to(&entry.motion.position_at(t))
                            .value()
                            / METRES_PER_LIGHT_YEAR;
                        assert!(ly <= LIFETIME_REACH.value(), "{id:?}: {ly} ly");
                        census.farthest = census.farthest.max(ly);
                    }
                }
                z += step;
            }
        }
    };
    // A quarter of the columns, by x; the totals are counts and maxima, so their order does not
    // matter.
    let quarter = |worker: i32| {
        let mut census = Census::default();
        for x in (-128..128).filter(|x: &i32| x.rem_euclid(4) == worker) {
            for y in -128..128 {
                column(x, y, &mut census);
            }
        }
        census
    };
    // Four threads of a quarter each.
    #[cfg(not(target_family = "wasm"))]
    let quarters: Vec<Census> = {
        let quarter = &quarter;
        thread::scope(|scope| {
            let workers: Vec<_> = (0..4)
                .map(|worker| scope.spawn(move || quarter(worker)))
                .collect();
            workers
                .into_iter()
                .map(|worker| worker.join().expect("a census worker"))
                .collect()
        })
    };
    // wasm32-wasip1 has no threads, so all of them on this one.
    #[cfg(target_family = "wasm")]
    let quarters: Vec<Census> = (0..4).map(quarter).collect();
    quarters
        .iter()
        .fold(Census::default(), |mut total, quarter| {
            total.add(quarter);
            total
        })
}

/// P09.T33.a, over seeds: within ±H a galaxy has 20–160 core collapses, about 40 at two a
/// century (the brainstorm, "Events in time"), and some 3,000–21,000 more entries than shells,
/// its progenitors and bare remnants; every entry's window is under its caps, and no system
/// travels past the reach. The Milky Way fixture and three drawn galaxies natively; on
/// wasm32-wasip1, which has no threads, the fixture alone, whose census takes minutes on one.
#[test]
#[ignore = "slow: the class's census of four galaxies"]
fn core_collapses_over_seeds() {
    // The fixture, then three galaxies drawn from their seeds.
    let galaxies = [
        ("the fixture", None),
        ("seed 1", Some(0x0933_0010)),
        ("seed 2", Some(0x0933_0011)),
        ("seed 3", Some(0x0933_0012)),
    ];
    let galaxies = if cfg!(target_family = "wasm") {
        &galaxies[..1]
    } else {
        &galaxies[..]
    };
    for &(name, drawn) in galaxies {
        let galaxy = &drawn.map_or_else(
            || milky_way(SEED),
            |seed| Galaxy::new(Seed::new(seed)).with_full_potential(),
        );
        let census = census(galaxy);
        let years = CoreCollapseProcess::new(galaxy)
            .interval()
            .length()
            .as_julian_years_f64();
        let rate = f64::from(census.deaths) / years;
        eprintln!(
            "{name}: {census:?}; {:.2} core collapses a century, {} entries beyond the shells",
            rate * 100.0,
            census.entries - census.shells
        );
        assert!(
            (20..=160).contains(&census.in_window),
            "{name}: {}",
            census.in_window
        );
        let beyond = census.entries - census.shells;
        assert!((3_000..=21_000).contains(&beyond), "{name}: {beyond}");
        // The deaths in ±H are a Poisson share of the deaths at the galaxy's rate.
        assert_poisson_count(
            &format!("{name}'s core collapses in ±H"),
            u64::from(census.in_window),
            rate * 2.0 * CLOCK_WINDOW_H.as_julian_years_f64(),
            ALPHA,
        );
    }
}

/// P09.T33.a's census restated under every 26 hold (plan 09, P09.T2.c; research item 7 of the
/// galaxy leans): the field keeps `1 − φ(a)` of the young disc, a tenth to a fifth of its core
/// collapses (ruling 118.2, after Higdon and Lingenfelter 2005, ApJ 628, 738), so the galaxy-wide
/// brackets of [`core_collapses_over_seeds`], 20–160 in ±H and 3,000–21,000 entries beyond the
/// shells, are the whole galaxy's, every class's, and move to P09.T38's census. The class's own
/// checks stay: the deaths in ±H a Poisson share of its deaths at its rate, every window under its
/// caps and no system past the reach. And its deaths match the budget's deaths, each weighed by
/// the share of its component's systems of its age that the field keeps, the closed form
/// `s_c F_c n_c(x) p(m) f_c(a) (1 − φ(a)) ÷ (1 − φ̄)` the class follows: the difference against
/// the variance of two independent counts, the field's Poisson and the weighed sum's own, which
/// errs lax, since the two arms share their candidates' streams. The budget is every 26 hold but
/// the field share. The same four galaxies, natively; the fixture alone on wasm32-wasip1.
#[test]
#[ignore = "slow: the class's census of four galaxies under either arm"]
fn core_collapses_over_seeds_under_every_26_hold() {
    let galaxies = [
        ("the fixture", None),
        ("seed 1", Some(0x0933_0010)),
        ("seed 2", Some(0x0933_0011)),
        ("seed 3", Some(0x0933_0012)),
    ];
    let galaxies = if cfg!(target_family = "wasm") {
        &galaxies[..1]
    } else {
        &galaxies[..]
    };
    for &(name, drawn) in galaxies {
        let (seed, params) = drawn.map_or_else(
            || (SEED, GalaxyParams::milky_way_like()),
            |seed| {
                (
                    seed,
                    GalaxyParams::from_seed(Seed::new(seed), MassFunctionKind::default()),
                )
            },
        );
        let off = GalaxyRules {
            field_share: FieldShareRules::None,
            ..GalaxyRules::every_26_hold()
        };
        let budget = Galaxy::from_params_with_rules(Seed::new(seed), params.clone(), off)
            .unwrap()
            .with_full_potential();
        let field =
            Galaxy::from_params_with_rules(Seed::new(seed), params, GalaxyRules::every_26_hold())
                .unwrap()
                .with_full_potential();
        let share = field
            .field_share()
            .expect("every 26 hold installs the field");
        let weighed = census_weighed(&budget, Some(share));
        let census = census(&field);
        let years = CoreCollapseProcess::new(&field)
            .interval()
            .length()
            .as_julian_years_f64();
        let rate = f64::from(census.deaths) / years;
        eprintln!(
            "{name}: {census:?}; {:.2} core collapses a century, {} entries beyond the shells; \
             the budget's {} deaths, {:.1} of them the field's",
            rate * 100.0,
            census.entries - census.shells,
            weighed.deaths,
            weighed.kept
        );
        let z = (f64::from(census.deaths) - weighed.kept)
            / (weighed.kept + weighed.kept_squares).sqrt();
        assert_p_value(
            &format!("{name}'s field deaths against the budget's weighed by 1 − φ, z = {z:.2}"),
            2.0 * normal_cdf(-z.abs()),
            ALPHA,
        );
        assert_poisson_count(
            &format!("{name}'s core collapses in ±H"),
            u64::from(census.in_window),
            rate * 2.0 * CLOCK_WINDOW_H.as_julian_years_f64(),
            ALPHA,
        );
    }
}
