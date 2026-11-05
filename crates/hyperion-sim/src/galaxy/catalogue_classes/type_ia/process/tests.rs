//! The Type Ia class's tests (P09.T34.a): the components it follows and its bounds, its candidates
//! against the galaxy's own Type Ia density, its entries against the supernova test, their members'
//! motion and reach, resolution and order independence, what the rest of the sim reads of them,
//! its headroom, and a golden. The galaxy-wide counts, the plan's 8–20 Type Ia in ±H and their
//! share of the shells, with every member's reach, are a slow census.

use std::sync::OnceLock;
#[cfg(not(target_family = "wasm"))]
use std::thread;

use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::lcg::Lcg;
use hyperion_testkit::order::assert_order_independent;
use hyperion_testkit::stats::{ALPHA, assert_poisson_count};

use super::*;
use crate::GENERATOR_VERSION;
use crate::Seed;
use crate::galaxy::Population;
use crate::galaxy::catalogue_classes::core_collapse::CoreCollapseProcess;
use crate::galaxy::catalogue_classes::grid::candidate_count;
use crate::galaxy::catalogue_classes::supernova::{
    LIFETIME_REACH, SupernovaKind, SupernovaState, claims, lifetime_cap,
};
use crate::galaxy::catalogue_classes::{
    CatalogueClassCell, CatalogueEntry, check_class_headroom, resolve_entry, system_of,
};
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{self, ResolveSystemError};
use crate::galaxy::query::PAD_SPEED;
use crate::id::{Layer, SystemIdKind};
use crate::time::{CLOCK_WINDOW_H, ClockWindow, Span};
use crate::units::consts::METRES_PER_LIGHT_YEAR;

/// The seed of the Milky Way fixture most tests share.
const SEED: u64 = 0x0934_0000_7479_6961;

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
    CatalogueCellKey::new(ClassId::TYPE_IA, cell).expect("a cell of the root cube")
}

/// Cells with entries: the four about the centre, where the bulge, the bar and the nuclear disc
/// peak, and a run along +y through the disc, in the plane.
fn sample_cells() -> Vec<CatalogueCellKey> {
    [[0, 0, 0], [-1, -1, -1], [0, 0, -1], [-1, 0, 0]]
        .into_iter()
        .chain((1..29).map(|y| [0, y, 0]))
        .map(key)
        .collect()
}

/// The sample cells' entries, generated once.
fn sample_entries() -> &'static [(CatalogueCellKey, Vec<CatalogueEntry<TypeIaEntry>>)] {
    static ENTRIES: OnceLock<Vec<(CatalogueCellKey, Vec<CatalogueEntry<TypeIaEntry>>)>> =
        OnceLock::new();
    ENTRIES.get_or_init(|| {
        let galaxy = galaxy();
        let process = TypeIaProcess::new(galaxy);
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

/// The light-years between two positions.
fn apart(a: &GalacticPosition, b: &GalacticPosition) -> f64 {
    a.distance_to(b).value() / METRES_PER_LIGHT_YEAR
}

/// The speed of the difference of two velocities, m/s.
fn relative_speed(a: GalacticVelocity, b: GalacticVelocity) -> f64 {
    let (a, b) = (a.metres_per_second(), b.metres_per_second());
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// Each member of `entry`: its ID and its system as its class gives it.
fn members(entry: &CatalogueEntry<TypeIaEntry>) -> Vec<(SystemId, SystemRecord, CatalogueMotion)> {
    let value = entry.value();
    let mut members = vec![(SystemId::from(entry.id()), value.record, value.motion)];
    if let Some(survivor) = value.survivor() {
        let id = SystemId::from(entry.member_id(1).expect("a second member"));
        members.push((id, survivor.record, survivor.motion));
    }
    members
}

// 1. The components and their bounds.

/// The class follows every component the delay-time distribution gives a Type Ia, each at its rate
/// per system over the Type Ia's interval; summed over the components' systems, born and unborn,
/// the rates are the distribution's, and the interval is the Type Ia cap's. From generator version
/// 27 the class follows the field alone (P09.T2.c): the distribution is the field's
/// ([`DelayTimeDistribution::of_field`], over each component's budget taken to the field in layer D
/// and the young disc's field ages), and each component's Type Ia per system its rate there taken
/// to the field, so that the features' and globulars' hosts are theirs.
#[test]
fn the_class_follows_every_population_s_type_ia() {
    let galaxy = galaxy();
    let process = TypeIaProcess::new(galaxy);
    let fields = galaxy.fields();
    let years = process.interval().length().as_julian_years_f64();
    assert_eq!(
        process.interval(),
        &SupernovaInterval::of_cap(galaxy.window_caps(), ShellEnvironment::TypeIa)
    );
    assert_eq!(process.delays(), &DelayTimeDistribution::of_field(galaxy));
    let ids: Vec<ComponentId> = process.components().iter().map(IaComponent::id).collect();
    assert_eq!(ids, fields.component_ids().collect::<Vec<_>>());
    let mut total = 0.0;
    for ia in process.components() {
        let rate = process.delays().rate_per_system_per_year(ia.id());
        assert_same_bits(
            ia.explosions_per_system(),
            galaxy.field_part(ia.id(), MassBand::D, rate * years),
        );
        let systems = fields.component(ia.id()).count_with_unborn();
        total += galaxy.field_part(ia.id(), MassBand::D, systems) * rate;
    }
    let galaxy_rate = process.delays().rate_per_year();
    assert!(
        (total / galaxy_rate - 1.0).abs() < 1e-12,
        "{total} {galaxy_rate}"
    );
    // Every population has Type Ia, the young disc's from its oldest tenth of stars (ages past
    // 42.6 Myr), the nuclear disc's at a few times the old populations' per system.
    let rate = |population: Population| {
        process
            .components()
            .iter()
            .filter(|ia| fields.component(ia.id()).population() == population)
            .map(IaComponent::explosions_per_system)
            .fold(0.0_f64, f64::max)
    };
    for population in crate::galaxy::POPULATIONS {
        assert!(rate(population) > 0.0, "{population:?}");
    }
    assert!(rate(Population::NuclearDisc) > 2.0 * rate(Population::Halo));
    eprintln!(
        "{} components, {:.4} Type Ia a century over a {years:.0} yr interval",
        process.components().len(),
        galaxy_rate * 100.0
    );
}

/// The thinning's envelope holds: at random points of random cells, from the centre out through
/// the disc and far above it, the components' running total of weights lies under the cell's
/// bound.
#[test]
fn the_envelope_holds_over_random_points() {
    let galaxy = galaxy();
    let process = TypeIaProcess::new(galaxy);
    let mut lcg = Lcg::new(0x0934_0001);
    let mut weights = [0.0; MAX_COMPONENTS];
    let mut tested = 0;
    for _ in 0..3_000 {
        let span = |lcg: &mut Lcg, n: u64| {
            i32::try_from(lcg.next_below(2 * n)).unwrap() - i32::try_from(n).unwrap()
        };
        let cell = key([
            span(&mut lcg, 128),
            span(&mut lcg, 128),
            span(&mut lcg, 128),
        ]);
        let bound = process.bound(galaxy, &cell).value();
        for _ in 0..8 {
            let p = cell.position_from_words([lcg.next_u64(), lcg.next_u64(), lcg.next_u64()]);
            let n = process.weights(galaxy, &PointLy::from(&p), &mut weights);
            let total = weights[..n].iter().fold(0.0, |sum, w| sum + w);
            assert!(total <= bound, "{total} above {bound} in {cell:?}");
            tested += 1;
        }
    }
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
    let process = TypeIaProcess::new(galaxy);
    let half = 65_536;
    let mut largest = 0.0_f64;
    let mut bounds = [0.0; MAX_COMPONENTS];
    for octant in 0..8 {
        let min = [0, 1, 2].map(|axis| if octant >> axis & 1 == 1 { -half } else { 0 });
        let cube = CellBox::new(min, 65_536).expect("an octant of the root cube");
        galaxy.fields().component_bounds(&cube, &mut bounds);
        let bound = process.components().iter().fold(0.0, |sum, ia| {
            sum + bounds[ia.id().index()] * ia.explosions_per_system()
        });
        largest = largest.max(bound);
    }
    let centre = process.bound(galaxy, &key([0, 0, 0])).value();
    assert!(centre <= largest);
    eprintln!(
        "the fullest cell expects at most {:.0} candidates; the centre's {:.0}",
        largest * key([0, 0, 0]).volume_ly3(),
        centre * key([0, 0, 0]).volume_ly3()
    );
    assert_eq!(
        check_class_headroom(ClassId::TYPE_IA, PerCubicLightYear::new(largest)),
        Ok(())
    );
}

// 2. The candidates.

/// The candidates the thinning keeps are the galaxy's Type Ia over the interval: by population, as
/// many as its components' densities × their explosions per system, integrated over the cells (a
/// midpoint rule of 40³ points per cell), at α = 10⁻³.
#[test]
fn candidates_follow_the_galaxy_s_type_ia_density() {
    const STEPS: u32 = 40;
    let galaxy = galaxy();
    let process = TypeIaProcess::new(galaxy);
    let fields = galaxy.fields();
    let populations = crate::galaxy::POPULATIONS;
    let slot = |id: ComponentId| {
        populations
            .iter()
            .position(|&p| p == fields.component(id).population())
            .unwrap()
    };
    let mut observed = [0_u64; 7];
    let mut expected = [0.0; 7];
    for cell in &sample_cells() {
        let bound = process.bound(galaxy, cell);
        for index in 0..candidate_count(galaxy.seed(), cell, bound) {
            let id = cell.candidate_id(index, 0).unwrap();
            if let Some(candidate) = process.candidate(galaxy, cell, id, bound) {
                observed[slot(candidate.component)] += 1;
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
                    for ia in process.components() {
                        expected[slot(ia.id())] += fields.component(ia.id()).density(&p)
                            * ia.explosions_per_system()
                            * volume;
                    }
                }
            }
        }
    }
    let mut tested = 0;
    for (i, population) in populations.iter().enumerate() {
        eprintln!(
            "{population:?}: {} candidates kept against {:.1} expected",
            observed[i], expected[i]
        );
        if expected[i] >= 10.0 {
            assert_poisson_count(
                &format!("the {population:?}'s kept candidates"),
                observed[i],
                expected[i],
                ALPHA,
            );
            tested += 1;
        }
    }
    assert_poisson_count(
        "every kept candidate",
        observed.iter().sum(),
        expected.iter().sum(),
        ALPHA,
    );
    assert!(tested >= 3, "{observed:?} {expected:?}");
}

/// A candidate's progenitor is its component's delay-first draw on `class.type_ia.progenitor`,
/// exploding in the class's interval; its leftover is its channel's draw on
/// `class.type_ia.leftover`; and its velocity the one velocity, on `class.type_ia.velocity`.
#[test]
fn a_candidate_s_draws_are_its_own_streams() {
    let galaxy = galaxy();
    let process = TypeIaProcess::new(galaxy);
    let interval = process.interval();
    let mut checked = 0;
    for cell in sample_cells().into_iter().take(2) {
        let bound = process.bound(galaxy, &cell);
        for index in 0..candidate_count(galaxy.seed(), &cell, bound) {
            let id = cell.candidate_id(index, 0).unwrap();
            let Some(candidate) = process.candidate(galaxy, &cell, id, bound) else {
                continue;
            };
            let key = ObjectKey::from(SystemId::from(id));
            let mut stream = Stream::open(galaxy.seed(), tags::CLASS_TYPE_IA_PROGENITOR, key);
            let progenitor = IaProgenitor::draw_in(
                process.delays(),
                &mut stream,
                candidate.component,
                interval.start(),
                interval.end(),
            );
            assert_eq!(progenitor, Some(candidate.progenitor));
            assert_eq!(
                candidate.progenitor.component(),
                u32::try_from(candidate.component.index()).unwrap()
            );
            assert!(interval.contains(candidate.progenitor.explosion()));
            let mut stream = Stream::open(galaxy.seed(), tags::CLASS_TYPE_IA_LEFTOVER, key);
            assert_eq!(
                IaLeftover::draw(candidate.progenitor.channel(), &mut stream),
                candidate.leftover
            );
            assert_eq!(
                candidate.velocity,
                class_site_velocity(
                    galaxy,
                    candidate.component,
                    &candidate.site,
                    tags::CLASS_TYPE_IA_VELOCITY,
                    key
                )
            );
            checked += 1;
        }
    }
    assert!(checked >= 30, "{checked}");
}

// 3. The entries.

/// Every entry is the supernova test's: its marks are `DeathMarks::of_type_ia` of its progenitor,
/// leftover, site and one velocity, `claims` holds in the smooth gas at its site, and its window is
/// the one `claimed_window` cut, under the Type Ia's cap and its lifetime cap. Member 0's record is
/// a catalogue system's of layer D with no component; a survivor is member 1, of its own star's
/// mass, flying off at its speed from T, and `v_fast` is at least every member's speed.
#[test]
fn every_entry_is_the_supernova_test_s() {
    let galaxy = galaxy();
    let process = TypeIaProcess::new(galaxy);
    let cap = galaxy.window_caps().cap(ShellEnvironment::TypeIa);
    let (mut entries, mut survivors) = (0, 0);
    for (cell, cell_entries) in sample_entries() {
        for entry in cell_entries {
            let value = entry.value();
            let record = value.record();
            let system = SystemId::from(entry.id());
            assert_eq!(record.id(), system);
            assert_eq!(
                record.origin(),
                SystemOrigin::CatalogueSystem(ClassId::TYPE_IA)
            );
            assert_eq!(record.component(), None);
            assert_eq!(record.layer(), Layer::D);
            let progenitor = value.progenitor();
            assert_eq!(record.population(), progenitor.population());
            assert_eq!(
                record.population(),
                galaxy.fields().component(value.component()).population()
            );
            assert_eq!(record.primary_initial_mass(), progenitor.primary_mass());
            assert_same_bits(
                record.age_at_epoch().value(),
                progenitor.age_at_epoch().value(),
            );
            let site = value.site();
            assert_eq!(
                CatalogueCellKey::containing(ClassId::TYPE_IA, site.position()),
                Some(*cell)
            );
            assert_eq!(site.death(), progenitor.explosion());
            // The marks, from the one velocity.
            let velocity = value.motion().velocity_at(just_before(site.death()));
            assert_eq!(value.motion().velocity_at(site.death()), velocity);
            assert_eq!(
                velocity,
                class_site_velocity(
                    galaxy,
                    value.component(),
                    site.position(),
                    tags::CLASS_TYPE_IA_VELOCITY,
                    ObjectKey::from(system)
                )
            );
            let marks = DeathMarks::of_type_ia(
                galaxy,
                system,
                progenitor,
                value.leftover(),
                site,
                velocity,
            );
            assert_eq!(&marks, value.marks());
            assert_eq!(marks.kind(), SupernovaKind::TypeIa(progenitor.channel()));
            assert!(claims(galaxy, site, &marks, Surroundings::TypeIa));
            let window = claimed_window(galaxy, site, &marks, Surroundings::TypeIa).unwrap();
            assert_eq!(value.supernova().window(), &window);
            assert_eq!(value.supernova().explosion(), site.death());
            assert!(window.duration() <= cap, "{:?}", window.duration());
            assert!(
                window.duration().value() <= lifetime_cap(marks.fastest_member_speed()).value()
            );
            // The members.
            assert_eq!(
                process.members(value),
                1 + u8::from(value.leftover().has_survivor())
            );
            assert_eq!(entry.members(), process.members(value));
            let fastest = marks.fastest_member_speed().value();
            let speed = value
                .leftover()
                .speed()
                .map_or(0.0, |speed| MetresPerSecond::from(speed).value());
            assert_same_bits(fastest, velocity.speed().value() + speed);
            assert!(velocity.speed().value() <= fastest);
            if let Some(survivor) = value.survivor() {
                check_survivor(entry, survivor, velocity, speed, fastest);
                survivors += 1;
            }
            entries += 1;
        }
    }
    eprintln!(
        "{entries} entries, {survivors} with a survivor, in {} cells",
        sample_entries().len()
    );
    assert!(entries >= 15 && survivors >= 5, "{entries} {survivors}");
}

/// `entry`'s `survivor`, member 1: at the site at T, with the progenitor at `velocity` before it,
/// flying off at `speed` (m/s) from it, never faster than the entry's `v_fast`, `fastest` (m/s);
/// its record of its own star's initial mass, the progenitor's population and age.
fn check_survivor(
    entry: &CatalogueEntry<TypeIaEntry>,
    survivor: &TypeIaSurvivor,
    velocity: GalacticVelocity,
    speed: f64,
    fastest: f64,
) {
    let value = entry.value();
    let (site, progenitor) = (value.site(), value.progenitor());
    let motion = survivor.motion();
    assert_eq!(motion.position_at(site.death()), *site.position());
    assert_eq!(motion.velocity_at(just_before(site.death())), velocity);
    let after = motion.velocity_at(site.death());
    assert!((relative_speed(after, velocity) - speed).abs() <= 1e-9 * speed.max(1.0));
    assert!(after.speed().value() <= fastest * (1.0 + 1e-15));
    let mass = match value.leftover() {
        IaLeftover::PartlyBurntDwarf => progenitor.primary_mass(),
        IaLeftover::SurvivingDonor { .. } | IaLeftover::HydrogenDonor { .. } => {
            progenitor.secondary_mass()
        }
        IaLeftover::Nothing => unreachable!("nothing survives"),
    };
    let record = survivor.record();
    assert_eq!(record.id(), SystemId::from(entry.member_id(1).unwrap()));
    assert_eq!(record.primary_initial_mass(), mass);
    assert_eq!(record.population(), progenitor.population());
    assert_same_bits(
        record.age_at_epoch().value(),
        progenitor.age_at_epoch().value(),
    );
    assert_eq!(
        record.origin(),
        SystemOrigin::CatalogueSystem(ClassId::TYPE_IA)
    );
}

/// No member of an entry travels more than 4,096 ly from its site within the entry's lifetime, from
/// its interval's start to +H: each member's broken line is farthest from the site at one of its
/// ends. Every member's record is where its line puts it at the epoch.
#[test]
fn no_member_travels_past_the_reach() {
    let mut farthest = 0.0_f64;
    for (_, entries) in sample_entries() {
        for entry in entries {
            let value = entry.value();
            let interval = SupernovaInterval::of_window(value.supernova().window().duration());
            let site = value.site().position();
            for (id, record, motion) in members(entry) {
                for t in [interval.start(), interval.end()] {
                    let ly = apart(site, &motion.position_at(t));
                    farthest = farthest.max(ly);
                    assert!(
                        ly <= LIFETIME_REACH.value(),
                        "{id:?} is {ly} ly from its site"
                    );
                }
                assert_eq!(
                    motion.position_at(UniverseTime::EPOCH),
                    *record.epoch_position()
                );
                assert_eq!(motion.position_at(value.site().death()), *site);
            }
        }
    }
    eprintln!("the farthest a member travels from its site: {farthest:.0} ly");
}

/// Every member resolves to itself bit for bit by its ID: through the class's process, through the
/// production lookup's catalogue system, which models no stars, and through plan 03's resolve,
/// which returns its record. The member after the last names nothing, and neither does an index
/// past its cell's count.
#[test]
fn members_resolve_to_themselves() {
    let galaxy = galaxy();
    let process = TypeIaProcess::new(galaxy);
    for (cell, entries) in sample_entries() {
        for entry in entries {
            for (n, (id, record, motion)) in members(entry).into_iter().enumerate() {
                let SystemIdKind::Catalogue(member) = id.kind() else {
                    panic!("a catalogue ID");
                };
                assert_eq!(usize::from(member.member()), n);
                let resolved = resolve_entry(galaxy, &process, member).unwrap();
                assert_eq!(resolved.entry(), entry);
                let system = system_of(galaxy, member).unwrap();
                assert_eq!(system.record(), &record);
                assert_eq!(system.motion(), &motion);
                assert_eq!(system.star_model(), None);
                assert_eq!(system.stars(galaxy), None);
                assert_eq!(placement::resolve(galaxy, id), Ok(record));
            }
            let next = CatalogueSystemId::new(
                entry.id().class(),
                entry.id().cell(),
                entry.id().index(),
                entry.members(),
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

/// The rest of the sim reads a member through its class (P09.T34.a): it has no stars of the
/// stellar stage's, so `stars_of`, the range brief and the planetary context refuse it as not
/// generated and the lensing walk weighs it as nothing; plan 03's position and epoch velocity are
/// its class's line; the loss cone does not meet it; and the observer's drift refuses its broken
/// line.
#[test]
fn the_rest_of_the_sim_reads_a_member_through_its_class() {
    use crate::galaxy::features::members::NoInteriorCache;
    use crate::galaxy::motion::{EpochState, LossConeVerdict, loss_cone_verdict};
    use crate::galaxy::query::{epoch_velocity, position_at};
    use crate::lensing::lens_mass_at;
    use crate::observe::{Drift, TraceMotionError, stars_of};
    use crate::planetary::SystemContext;
    use crate::stellar::brief::BriefModel;
    let galaxy = galaxy();
    let mut checked = 0;
    for (_, entries) in sample_entries() {
        for entry in entries {
            let death = entry.value().site().death();
            for (id, record, motion) in members(entry) {
                assert_eq!(
                    stars_of(galaxy, &NoInteriorCache, &record),
                    Err(ResolveSystemError::KindNotGenerated)
                );
                assert!(matches!(
                    BriefModel::of_record(galaxy, &NoInteriorCache, &record),
                    Err(ResolveSystemError::KindNotGenerated)
                ));
                assert_eq!(
                    SystemContext::for_system(galaxy, id),
                    Err(ResolveSystemError::KindNotGenerated)
                );
                assert!(lens_mass_at(galaxy, &record, UniverseTime::EPOCH).is_none());
                for t in [
                    ClockWindow::START,
                    ClockWindow::END,
                    death,
                    just_before(death),
                ]
                .into_iter()
                .filter(|&t| ClockWindow::contains(t))
                {
                    assert_eq!(position_at(galaxy, &record, t), motion.position_at(t));
                }
                let velocity = epoch_velocity(galaxy, &record);
                assert_eq!(velocity, motion.velocity_at(UniverseTime::EPOCH));
                let at_epoch = EpochState::new(record, velocity).unwrap();
                assert_eq!(loss_cone_verdict(galaxy, &at_epoch), LossConeVerdict::Clear);
                assert_eq!(
                    Drift::of_record(galaxy, &record),
                    Err(TraceMotionError::CatalogueMotionNotTraced(id))
                );
                checked += 1;
            }
        }
    }
    assert!(checked >= 20, "{checked}");
}

/// A candidate the thinning keeps but whose explosion falls before its window's interval is no
/// entry, and its ID names nothing; such candidates outnumber the entries, whose windows are
/// mostly shorter than the cap.
#[test]
fn an_explosion_too_early_is_thinned() {
    let galaxy = galaxy();
    let process = TypeIaProcess::new(galaxy);
    let (mut too_early, mut kept) = (0, 0);
    for cell in sample_cells() {
        let bound = process.bound(galaxy, &cell);
        for index in 0..candidate_count(galaxy.seed(), &cell, bound) {
            let id = cell.candidate_id(index, 0).unwrap();
            let Some(candidate) = process.candidate(galaxy, &cell, id, bound) else {
                continue;
            };
            let site = ExplosionSite::new(candidate.site, candidate.progenitor.explosion());
            let marks = candidate.marks(galaxy, &site);
            if claimed_window(galaxy, &site, &marks, Surroundings::TypeIa).is_some() {
                kept += 1;
                continue;
            }
            too_early += 1;
            assert!(
                !SupernovaInterval::of_window(
                    crate::galaxy::catalogue_classes::supernova::effective_window(
                        galaxy,
                        &site,
                        &marks,
                        Surroundings::TypeIa
                    )
                    .duration()
                )
                .contains(site.death())
            );
            assert!(process.test(galaxy, candidate).is_none());
            assert_eq!(
                resolve_entry(galaxy, &process, id),
                Err(ResolveSystemError::NoSuchSystem)
            );
        }
    }
    eprintln!("{kept} kept, {too_early} before their window's interval");
    assert!(too_early > kept && kept > 0, "{too_early} {kept}");
}

/// The cells do not depend on the order they are generated in, nor on what was resolved between
/// them; the same seed gives the same cells in a fresh galaxy, and another seed other cells.
#[test]
fn cells_do_not_depend_on_order() {
    let galaxy = galaxy();
    let process = TypeIaProcess::new(galaxy);
    let cells: Vec<CatalogueCellKey> = [[0, 0, 0], [-1, 0, 0]]
        .into_iter()
        .chain((1..9).map(|y| [0, y, 0]))
        .map(key)
        .collect();
    assert_order_independent(&cells, |key| {
        // A stray resolve of the core collapses' first candidate in the same place first.
        let stray =
            CatalogueSystemId::new(ClassId::CORE_COLLAPSE.value(), key.fine_cell(), 0, 0).unwrap();
        let _ = placement::resolve(galaxy, SystemId::from(stray));
        CatalogueClassCell::generate(galaxy, &process, *key)
    });
    let fresh = |seed: u64| {
        let galaxy = Galaxy::new(Seed::new(seed));
        let process = TypeIaProcess::new(&galaxy);
        cells
            .iter()
            .map(|key| CatalogueClassCell::generate(&galaxy, &process, *key))
            .collect::<Vec<_>>()
    };
    let once = fresh(0x0934_0002);
    assert_eq!(once, fresh(0x0934_0002));
    let counts = |cells: &[CatalogueClassCell<TypeIaEntry>]| -> Vec<u32> {
        cells.iter().map(CatalogueClassCell::candidates).collect()
    };
    assert_ne!(counts(&once), counts(&fresh(0x0934_0003)));
}

/// A galaxy built without its kinematic tables has no velocities: its progenitors rest, and only
/// survivors move, at their own speeds from their sites.
#[test]
fn a_galaxy_without_kinematics_has_resting_progenitors() {
    let galaxy = Galaxy::new(Seed::new(0x0934_0004));
    let process = TypeIaProcess::new(&galaxy);
    let mut seen = 0;
    for cell in [[0, 0, 0], [-1, -1, -1], [0, 0, -1], [-1, 0, 0]] {
        for entry in CatalogueClassCell::generate(&galaxy, &process, key(cell)).entries() {
            let value = entry.value();
            assert_eq!(
                value.motion().velocity_at(UniverseTime::EPOCH),
                GalacticVelocity::default()
            );
            if let (Some(survivor), Some(speed)) = (value.survivor(), value.leftover().speed()) {
                let after = survivor.motion().velocity_at(value.site().death());
                let speed = MetresPerSecond::from(speed).value();
                assert!((after.speed().value() - speed).abs() <= 1e-9 * speed);
            }
            seen += 1;
        }
    }
    assert!(seen > 0);
}

// 4. The golden.

/// Pins the class on the fixture: its components' rates, its interval, and the sample cells: each
/// one's bound, its candidate count and every entry's ID, component, channel, explosion, window,
/// masses, age, energy, `v_fast`, leftover, sites and every member's positions and velocities.
#[test]
fn type_ia_class_golden() {
    let galaxy = galaxy();
    let process = TypeIaProcess::new(galaxy);
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for ia in process.components() {
        w.f64(
            &format!("component {} explosions per system", ia.id().index()),
            ia.explosions_per_system(),
        );
    }
    let interval = process.interval();
    w.line(&format!(
        "interval ({}, {}]",
        interval.start(),
        interval.end()
    ));
    let position = |w: &mut GoldenWriter, label: &str, p: &GalacticPosition| {
        w.line(&format!("{label} cell {:?}", p.cell().to_array()));
        for (axis, metres) in p.offset_metres().iter().enumerate() {
            w.f64(&format!("{label} offset {axis}"), *metres);
        }
    };
    let velocity = |w: &mut GoldenWriter, label: &str, v: GalacticVelocity| {
        for (axis, v) in v.metres_per_second().iter().enumerate() {
            w.f64(&format!("{label} {axis}"), *v);
        }
    };
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
            let progenitor = value.progenitor();
            let supernova = value.supernova();
            let id = format!("{label} {:016x}", SystemId::from(entry.id()).raw());
            w.line(&format!(
                "{id} component {} explodes {} as {:?}, members {}",
                value.component().index(),
                supernova.explosion(),
                supernova.kind(),
                entry.members()
            ));
            w.f64(
                &format!("{id} window"),
                supernova.window().duration().value(),
            );
            w.f64(&format!("{id} primary"), progenitor.primary_mass().value());
            w.f64(
                &format!("{id} secondary"),
                progenitor.secondary_mass().value(),
            );
            w.f64(&format!("{id} age"), value.record().age_at_epoch().value());
            w.f64(&format!("{id} energy"), value.marks().energy().foes());
            w.f64(
                &format!("{id} fastest"),
                value.marks().fastest_member_speed().value(),
            );
            w.line(&format!("{id} leftover {:?}", value.leftover()));
            if let Some(speed) = value.leftover().speed() {
                w.f64(&format!("{id} survivor speed"), speed.value());
            }
            position(&mut w, &format!("{id} site"), value.site().position());
            for (member, record, motion) in members(entry) {
                let label = format!("{id} member {:016x}", member.raw());
                position(&mut w, &format!("{label} epoch"), record.epoch_position());
                w.f64(
                    &format!("{label} mass"),
                    record.primary_initial_mass().value(),
                );
                velocity(
                    &mut w,
                    &format!("{label} velocity before"),
                    motion.velocity_at(just_before(motion.at())),
                );
                velocity(
                    &mut w,
                    &format!("{label} velocity after"),
                    motion.velocity_at(motion.at()),
                );
            }
        }
    }
    golden!("catalogue_classes/type_ia_class", w.as_str());
}

// 5. The census (slow).

/// What a galaxy's Type Ia class holds, counted over every cell.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Census {
    /// Candidates the thinning kept: the galaxy's Type Ia over the interval.
    candidates: u32,
    /// Of those, entries: the explosions `claims` keeps.
    entries: u32,
    /// Entries with a survivor, member 1.
    survivors: u32,
    /// Entries that are a shell at the epoch: a supernova of some age.
    shells: u32,
    /// Entries exploding in ±H.
    in_window: u32,
    /// Entries whose member 0 is not of layer D.
    not_layer_d: u32,
    /// The longest window, Julian years; the farthest any member travels from its site, ly; and
    /// the greatest `v_fast`, km/s.
    longest: f64,
    farthest: f64,
    fastest: f64,
}

impl Census {
    fn add(&mut self, other: &Self) {
        self.candidates += other.candidates;
        self.entries += other.entries;
        self.survivors += other.survivors;
        self.shells += other.shells;
        self.in_window += other.in_window;
        self.not_layer_d += other.not_layer_d;
        self.longest = self.longest.max(other.longest);
        self.farthest = self.farthest.max(other.farthest);
        self.fastest = self.fastest.max(other.fastest);
    }
}

/// A mean below which a cell's candidates are not drawn: 10⁻⁹, so that every cell skipped holds,
/// together, under 0.02 candidates of the root cube's 2²⁴ cells.
const NEGLIGIBLE_MEAN: f64 = 1e-9;

/// The workers a census splits the columns of the root cube between.
const WORKERS: i32 = 8;

/// Runs `column` over every column of 512 ly cells of the root cube, a share of them on each of
/// [`WORKERS`] threads (all on this one on wasm32-wasip1, which has no threads), and folds the
/// workers' totals with `add`: counts and maxima, whose order does not matter.
fn over_columns<T: Default + Send>(
    column: impl Fn(i32, i32, &mut T) + Sync,
    add: impl Fn(&mut T, &T),
) -> T {
    let share = |worker: i32| {
        let mut total = T::default();
        for x in (-128..128).filter(|x: &i32| x.rem_euclid(WORKERS) == worker) {
            for y in -128..128 {
                column(x, y, &mut total);
            }
        }
        total
    };
    #[cfg(not(target_family = "wasm"))]
    let shares: Vec<T> = {
        let share = &share;
        thread::scope(|scope| {
            let workers: Vec<_> = (0..WORKERS)
                .map(|worker| scope.spawn(move || share(worker)))
                .collect();
            workers
                .into_iter()
                .map(|worker| worker.join().expect("a census worker"))
                .collect()
        })
    };
    #[cfg(target_family = "wasm")]
    let shares: Vec<T> = (0..WORKERS).map(share).collect();
    shares.iter().fold(T::default(), |mut total, share| {
        add(&mut total, share);
        total
    })
}

/// The cells of the column at (`x`, `y`) of `class` whose mean under `bound` is not negligible,
/// from the plane outwards both ways (the bound never rises away from the plane), handed to `cell`.
fn column_cells(
    class: ClassId,
    x: i32,
    y: i32,
    bound: impl Fn(&CatalogueCellKey) -> PerCubicLightYear,
    mut cell: impl FnMut(CatalogueCellKey, PerCubicLightYear),
) {
    for (start, step) in [(0, 1), (-1, -1)] {
        let mut z = start;
        while (-128..128).contains(&z) {
            let key = CatalogueCellKey::new(class, [x, y, z]).expect("a cell of the root cube");
            let b = bound(&key);
            if b.value() * key.volume_ly3() < NEGLIGIBLE_MEAN {
                break;
            }
            cell(key, b);
            z += step;
        }
    }
}

/// The census of `galaxy`'s Type Ia class: every cell whose mean is not negligible. Every entry's
/// window is checked against its caps and every member's reach on the way.
fn census(galaxy: &Galaxy) -> Census {
    let process = TypeIaProcess::new(galaxy);
    let cap = galaxy.window_caps().cap(ShellEnvironment::TypeIa);
    let column = |x: i32, y: i32, census: &mut Census| {
        column_cells(
            ClassId::TYPE_IA,
            x,
            y,
            |key| process.bound(galaxy, key),
            |cell, bound| {
                for index in 0..candidate_count(galaxy.seed(), &cell, bound) {
                    let id = cell.candidate_id(index, 0).unwrap();
                    let Some(candidate) = process.candidate(galaxy, &cell, id, bound) else {
                        continue;
                    };
                    census.candidates += 1;
                    let Some(value) = process.test(galaxy, candidate) else {
                        continue;
                    };
                    census.entries += 1;
                    let duration = value.supernova().window().duration().value();
                    assert!(value.supernova().window().duration() <= cap, "{id:?}");
                    let fastest = value.marks().fastest_member_speed();
                    assert!(duration <= lifetime_cap(fastest).value(), "{id:?}");
                    census.longest = census.longest.max(duration);
                    census.fastest = census.fastest.max(fastest.value() / 1e3);
                    census.not_layer_d += u32::from(value.record().layer() != Layer::D);
                    if matches!(
                        value.supernova().state_at(UniverseTime::EPOCH),
                        SupernovaState::Supernova { .. }
                    ) {
                        census.shells += 1;
                    }
                    if ClockWindow::contains(value.site().death()) {
                        census.in_window += 1;
                    }
                    let interval =
                        SupernovaInterval::of_window(value.supernova().window().duration());
                    let site = value.site().position();
                    let mut motions = vec![*value.motion()];
                    if let Some(survivor) = value.survivor() {
                        census.survivors += 1;
                        motions.push(*survivor.motion());
                    }
                    for motion in motions {
                        for t in [interval.start(), interval.end()] {
                            let ly = apart(site, &motion.position_at(t));
                            assert!(ly <= LIFETIME_REACH.value(), "{id:?}: {ly} ly");
                            census.farthest = census.farthest.max(ly);
                        }
                    }
                }
            },
        );
    };
    over_columns(column, Census::add)
}

/// The core collapses' shells at the epoch in `galaxy`: P09.T33.a's entries that are a supernova
/// of some age, over every cell of its class whose mean is not negligible.
fn core_collapse_shells(galaxy: &Galaxy) -> u32 {
    let process = CoreCollapseProcess::new(galaxy);
    let column = |x: i32, y: i32, shells: &mut u32| {
        column_cells(
            ClassId::CORE_COLLAPSE,
            x,
            y,
            |key| process.bound(galaxy, key),
            |cell, _| {
                for entry in CatalogueClassCell::generate(galaxy, &process, cell).entries() {
                    if matches!(
                        entry.value().supernova().state_at(UniverseTime::EPOCH),
                        SupernovaState::Supernova { .. }
                    ) {
                        *shells += 1;
                    }
                }
            },
        );
    };
    over_columns(column, |total: &mut u32, share: &u32| *total += share)
}

/// P09.T34.a, over seeds: 8–20 Type Ia in ±H, a Poisson share at the galaxy's rate; the
/// explosions over the interval at that rate; every entry's window under its caps, member 0 of
/// layer D, and no member past the reach; and the Type Ia's share of the distinct shells at the
/// epoch, with P09.T33.a's core collapses', pinned as measured at about a fifth (0.193–0.211),
/// against the plan's and the brainstorm's "a third or more": a finding. At 23 every core collapse
/// is the field's (φ = 0); the brainstorm's figure counts the shells in superbubbles too, so from
/// P09.T2.c the comparison belongs with P09.T36.a's or T38's census. The Milky Way fixture and
/// three drawn galaxies natively (83 s on the pod's 16 vCPU); on wasm32-wasip1, which has no
/// threads, the fixture alone.
#[test]
#[ignore = "slow: the class's census of four galaxies, beside the core collapses'"]
fn type_ia_over_seeds() {
    let galaxies = [
        ("the fixture", None),
        ("seed 1", Some(0x0934_0010)),
        ("seed 2", Some(0x0934_0011)),
        ("seed 3", Some(0x0934_0012)),
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
        let process = TypeIaProcess::new(galaxy);
        let rate = process.delays().rate_per_year();
        let years = process.interval().length().as_julian_years_f64();
        let shells = core_collapse_shells(galaxy);
        let share = f64::from(census.shells) / f64::from(census.shells + shells);
        eprintln!(
            "{name}: {census:?}; {:.3} Type Ia a century; {shells} core-collapse shells, the Type \
             Ia's share {share:.3}",
            rate * 100.0
        );
        // The thinning keeps the galaxy's Type Ia over the interval.
        assert_poisson_count(
            &format!("{name}'s Type Ia over the interval"),
            u64::from(census.candidates),
            rate * years,
            ALPHA,
        );
        // The explosions in ±H are a Poisson share of them at the galaxy's rate.
        assert_poisson_count(
            &format!("{name}'s Type Ia in ±H"),
            u64::from(census.in_window),
            rate * 2.0 * CLOCK_WINDOW_H.as_julian_years_f64(),
            ALPHA,
        );
        assert!(
            (8..=20).contains(&census.in_window),
            "{name}: {}",
            census.in_window
        );
        assert!(
            census.survivors > 0 && census.not_layer_d == 0,
            "{census:?}"
        );
        // A survivor at up to 2,500 km/s on a progenitor under plan 08's cut.
        assert!(
            census.fastest < 2_500.0 + PAD_SPEED.value(),
            "{name}: {census:?}"
        );
        assert!((0.17..=0.25).contains(&share), "{name}: {share}");
    }
}

/// P09.T34.a's census restated under every 26 hold (plan 09, P09.T2.c; research advice §3 of the
/// holds and leans): the field gives up φ, so the class follows the field's Type Ia and the
/// core-collapse class the field's deaths, a tenth to a fifth of the galaxy's (ruling 118.2, after
/// Higdon and Lingenfelter 2005, ApJ 628, 738). The class's own checks stay: its Type Ia over the
/// interval and in ±H Poisson counts at the field's rate, 8–20 in ±H, every window under its caps,
/// member 0 of layer D and no member past the reach. The Type Ia's share of the field's distinct shells is recorded, not bracketed: the 23
/// field's 0.17–0.25 counted every core collapse, and the brainstorm's "a third or more", over the
/// shells in superbubbles too, belongs with P09.T38's census. The same four galaxies, natively; the
/// fixture alone on wasm32-wasip1.
#[test]
#[ignore = "slow: the class's census of four galaxies under every 26 hold, beside the core \
            collapses'"]
fn type_ia_over_seeds_under_every_26_hold() {
    use crate::galaxy::imf::MassFunctionKind;
    use crate::galaxy::rules::GalaxyRules;
    let galaxies = [
        ("the fixture", None),
        ("seed 1", Some(0x0934_0010)),
        ("seed 2", Some(0x0934_0011)),
        ("seed 3", Some(0x0934_0012)),
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
        // The field share is held at 27 from train 26's retarget: the census reads it with the
        // version's other rules, as it was measured under every 26 hold.
        let galaxy = &Galaxy::from_params_with_rules(
            Seed::new(seed),
            params,
            GalaxyRules::of_version(crate::GENERATOR_VERSION),
        )
        .unwrap()
        .with_full_potential();
        let census = census(galaxy);
        let process = TypeIaProcess::new(galaxy);
        let rate = process.delays().rate_per_year();
        let years = process.interval().length().as_julian_years_f64();
        let shells = core_collapse_shells(galaxy);
        let share = f64::from(census.shells) / f64::from(census.shells + shells);
        eprintln!(
            "{name}: {census:?}; {:.3} field Type Ia a century; {shells} field core-collapse \
             shells, the Type Ia's share {share:.3}",
            rate * 100.0
        );
        assert_poisson_count(
            &format!("{name}'s field Type Ia over the interval"),
            u64::from(census.candidates),
            rate * years,
            ALPHA,
        );
        assert_poisson_count(
            &format!("{name}'s field Type Ia in ±H"),
            u64::from(census.in_window),
            rate * 2.0 * CLOCK_WINDOW_H.as_julian_years_f64(),
            ALPHA,
        );
        assert!(
            (8..=20).contains(&census.in_window),
            "{name}: {}",
            census.in_window
        );
        assert!(
            census.survivors > 0 && census.not_layer_d == 0,
            "{census:?}"
        );
        assert!(
            census.fastest < 2_500.0 + PAD_SPEED.value(),
            "{name}: {census:?}"
        );
        assert!(share > 0.0 && share < 1.0, "{name}: {share}");
    }
}
