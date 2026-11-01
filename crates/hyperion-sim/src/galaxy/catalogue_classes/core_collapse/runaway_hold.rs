//! The tests of P09.T33.b, [`RUNAWAY_DEATHS_VERSION`]: from 27 the core-collapse class follows the
//! young field's runaways and walkaways. The class at 26 is the held arm verbatim (the class's own
//! tests and golden hold it at the version's rules); these hold the new arm, under every 27 hold.
//!
//! - The tripwire; the held arm's process and cells are every 26 hold's.
//! - The terms are the young field's band-E runaway and walkaway classes, each with a form; a
//!   term's largest share bounds its class's share of a star's deaths at every mass; a source's
//!   classes together take exactly the share the field's ejection marks drop, mass by mass,
//!   `RunawayModel`'s closed form; and the field's marks drop that share, at α = 10⁻³.
//! - The envelope holds over random points, and no cell comes near the index.
//! - Every runaway's or walkaway's entry is its term's: its ejection in its class's speed and age
//!   bins before its death, its velocity its component's law plus the ejection's, and the shared
//!   test's. A grid record and a class candidate with the same marks and velocity give the same
//!   death in the interval and the same claim (complementarity, for sampled runaway candidates).
//! - The cells do not depend on the order they are generated in.
//! - Slow: galaxy-wide, the candidates' ejected share against the closed form, each candidate's
//!   odds its mass's ejected share against the field's part at its age (α = 10⁻³).

use std::sync::OnceLock;

use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::lcg::Lcg;
use hyperion_testkit::order::assert_order_independent;
use hyperion_testkit::stats::{ALPHA, assert_p_value, normal_cdf};

use super::*;
use crate::GENERATOR_VERSION;
use crate::Seed;
use crate::galaxy::catalogue_classes::grid::candidate_count;
use crate::galaxy::catalogue_classes::{CatalogueClassCell, CatalogueEntry, check_class_headroom};
use crate::galaxy::displaced::explosion_site;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::CellKey;
use crate::galaxy::rules::GalaxyRules;
use crate::id::Layer;
use crate::stellar::system::{draw_metallicity, primary_draws};

/// The seed of the Milky Way fixture, P09.T33.a's tests'.
const SEED: u64 = 0x0933_0000_6363_6f6c;

/// The fixture built under every 27 hold, with its kinematic tables: built so, since the field
/// share and the carve-out are read when a galaxy is built; built once.
fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params_with_rules(
            Seed::new(SEED),
            GalaxyParams::milky_way_like(),
            GalaxyRules::every_27_hold(),
        )
        .expect("the Milky Way fixture's gas is mostly neutral")
        .with_full_potential()
    })
}

/// The class cell `cell`.
fn key(cell: [i32; 3]) -> CatalogueCellKey {
    CatalogueCellKey::new(ClassId::CORE_COLLAPSE, cell).expect("a cell of the root cube")
}

/// P09.T33.a's sample cells, a run along +y through the inner disc in the plane and the centre's,
/// with the cells just above and below the plane, where the runaways are.
fn sample_cells() -> Vec<CatalogueCellKey> {
    (8..28)
        .flat_map(|y| [-1, 0].map(|z| key([0, y, z])))
        .chain([key([0, 0, 0])])
        .collect()
}

/// The sample cells' entries under every 27 hold, generated once.
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

/// The class table of the fixture.
fn table() -> &'static ClassTable {
    galaxy()
        .class_table()
        .expect("every 27 hold installs the displaced classes")
}

/// The share of a band-E star of initial mass `m` of `source` ejected before its death at its
/// reference lifetime, the closed form, in the fixture: [`closed_form_share`].
fn ejected_share(source: BirthSource, m: SolarMasses) -> f64 {
    closed_form_share(table(), source, m)
}

/// The share of a band-E star of initial mass `m` of `source` ejected before its death at its
/// reference lifetime under `table`, the closed form: each kind's lifetime share × the weight of
/// its ejection ages below that lifetime.
fn closed_form_share(table: &ClassTable, source: BirthSource, m: SolarMasses) -> f64 {
    let model = table.runaway_model();
    let life = table.reference_lifetime(source, MassBand::E, m);
    KINDS.iter().fold(0.0, |sum, &kind| {
        let before = table
            .ejection_ages(source, kind, m)
            .iter()
            .filter(|(age, _)| *age < life)
            .fold(0.0, |s, (_, w)| s + w);
        sum + lifetime_share(model, kind, m) * before
    })
}

/// `n` log-spaced masses across band E, its edges included.
fn band_masses(n: u32) -> impl Iterator<Item = SolarMasses> {
    let (lo, hi) = (MassBand::E.lo(), MassBand::E.hi());
    (0..=n).map(move |k| {
        SolarMasses::new(
            lo * crate::math::exp(f64::from(k) / f64::from(n) * crate::math::ln(hi / lo)),
        )
    })
}

// 1. The hold.

/// The version's class keeps the held arm, every version below 27 does, and 27 and every 27 hold
/// follow the runaways. The tripwire: the bump to 27 deletes this hold.
#[test]
fn the_runaway_deaths_are_held_only_until_the_27_bump() {
    assert!(
        GENERATOR_VERSION < RUNAWAY_DEATHS_VERSION,
        "the bump to 27 deletes P09.T33.b's hold: `RUNAWAY_DEATHS_VERSION`, \
         `RunawayDeathsRules` with its `FieldOnly` arm, `GalaxyRules::runaway_deaths`, the \
         matches on it in `CoreCollapseProcess::new` (`ejecting`), and this module's tripwire"
    );
    let at = |v: u32| GalaxyRules::of_version(GeneratorVersion::new(v)).runaway_deaths;
    for v in [GENERATOR_VERSION.get(), 25, 26] {
        assert_eq!(at(v), RunawayDeathsRules::FieldOnly, "version {v}");
    }
    assert_eq!(at(27), RunawayDeathsRules::Ejected);
    assert_eq!(
        GalaxyRules::every_26_hold().runaway_deaths,
        RunawayDeathsRules::FieldOnly
    );
    assert_eq!(
        GalaxyRules::every_27_hold().runaway_deaths,
        RunawayDeathsRules::Ejected
    );
    assert_ne!(CoreCollapseProcess::new(galaxy()).ejected_terms().len(), 0);
}

/// Under the held arm the class's process and cells are every 26 hold's, bit for bit: the field
/// alone turns the new arm on.
#[test]
fn the_held_arm_is_every_26_hold_s() {
    let every_26 = Galaxy::from_params_with_rules(
        Seed::new(SEED),
        GalaxyParams::milky_way_like(),
        GalaxyRules::every_26_hold(),
    )
    .expect("the Milky Way fixture's gas is mostly neutral")
    .with_full_potential();
    let held = galaxy().clone().with_rules(GalaxyRules {
        runaway_deaths: RunawayDeathsRules::FieldOnly,
        ..GalaxyRules::every_27_hold()
    });
    let process = CoreCollapseProcess::new(&every_26);
    assert_eq!(process, CoreCollapseProcess::new(&held));
    assert_eq!(process.ejected_terms(), []);
    for cell in [key([0, 12, 0]), key([0, 20, -1]), key([0, 0, 0])] {
        let [a, b] = [&every_26, &held]
            .map(|g| CatalogueClassCell::generate(g, &CoreCollapseProcess::new(g), cell));
        assert_eq!(a.into_entries(), b.into_entries(), "{cell:?}");
    }
}

// 2. The terms and the closed form.

/// The terms are the young field's: the young thin disc's thin-disc classes and each nuclear-disc
/// component's nuclear-disc classes, each with a form, in the components' and the classes' order,
/// at the component's band budget and its budget's largest death rate; and every class of an
/// ejecting source that holds a share of a band-E star's deaths has its term, so none is lost.
#[test]
fn the_terms_are_the_young_field_s_runaway_and_walkaway_classes() {
    let galaxy = galaxy();
    let process = CoreCollapseProcess::new(galaxy);
    let displaced = galaxy.fields().displaced().expect("the displaced classes");
    let table = table();
    let fields = galaxy.fields();
    let years = process.interval().length().as_julian_years_f64();
    let mut order = Vec::new();
    for term in process.ejected_terms() {
        let component = fields.component(term.component());
        let class = displaced.class(term.class());
        assert!(class.form().is_some(), "{term:?}");
        match (component.population(), class.key()) {
            (Population::YoungThinDisc, ClassKey::Thin { speed, age }) => {
                assert_eq!((term.speed(), term.age()), (speed, Some(age)));
            }
            (Population::NuclearDisc, ClassKey::Old { source, speed }) => {
                assert_eq!(source.source(), BirthSource::NuclearDisc);
                assert_eq!((term.speed(), term.age()), (speed, None));
            }
            other => panic!("{other:?} is no young field's runaway class"),
        }
        assert_same_bits(
            term.systems(),
            component.count() * galaxy.shares().share(MassBand::E, component.population()),
        );
        let rate = component.ages().max_pdf_between(
            Years::new(-CLOCK_WINDOW_H.as_julian_years_f64()),
            RECENT_DEATH_AGES,
        );
        assert_same_bits(term.deaths_per_system(), rate * (1.0 + RATE_MARGIN) * years);
        order.push((term.component().index(), term.class().index()));
    }
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{order:?}");
    // No class with a share of some band-E star's deaths is left without a term.
    let mut parts = Vec::new();
    for young in process.components() {
        let source = table.component_source(young.id());
        for (index, class) in displaced.classes().iter().enumerate() {
            let id = DisplacedClassId::new(u16::try_from(index).unwrap());
            let (speed, age) = match class.key() {
                ClassKey::Thin { speed, age } if source == BirthSource::ThinDisc => {
                    (speed, Some(age))
                }
                ClassKey::Old { source: old, speed } if old.source() == source => (speed, None),
                ClassKey::Thin { .. } | ClassKey::Old { .. } | ClassKey::Hypervelocity => {
                    continue;
                }
            };
            let probe = EjectedTerm {
                class: id,
                component: young.id(),
                source,
                speed,
                age,
                systems: 1.0,
                largest_share: 1.0,
                deaths_per_system: 1.0,
            };
            let takes = band_masses(256).any(|m| ejected_parts(table, &probe, m, &mut parts) > 0.0);
            let termed = process
                .ejected_terms()
                .iter()
                .any(|t| t.component() == young.id() && t.class() == id);
            assert_eq!(takes, termed, "{:?} of {:?}", class.key(), young.id());
        }
    }
}

/// A term's largest share bounds its class's share of a band-E star's deaths at every mass, over
/// a fine grid and random masses; and a source's classes together take, at each mass, the closed
/// form's ejected share: the share the field's marks drop.
#[test]
fn the_classes_take_the_closed_form_s_share_mass_by_mass() {
    let process = CoreCollapseProcess::new(galaxy());
    let table = table();
    let mut lcg = Lcg::new(0x0933_b001);
    let (lo, hi) = (MassBand::E.lo(), MassBand::E.hi());
    let masses: Vec<SolarMasses> = band_masses(4_096)
        .chain((0..4_096).map(|_| {
            SolarMasses::new(lo * crate::math::exp(lcg.next_f64() * crate::math::ln(hi / lo)))
        }))
        .collect();
    let mut parts = Vec::new();
    for &m in &masses {
        for young in process.components() {
            let source = table.component_source(young.id());
            let mut taken = 0.0;
            for term in process
                .ejected_terms()
                .iter()
                .filter(|t| t.component() == young.id())
            {
                let share = ejected_parts(table, term, m, &mut parts);
                assert!(
                    share <= term.largest_share(),
                    "{m:?}: {share} over {term:?}"
                );
                let summed = parts.iter().fold(0.0, |sum, part| sum + part.share);
                assert_same_bits(summed, share);
                taken += share;
            }
            let closed = ejected_share(source, m);
            assert!(
                (taken - closed).abs() <= 1e-12,
                "{m:?} of {source:?}: the classes take {taken}, the closed form {closed}"
            );
        }
    }
}

/// The field's ejection marks drop a candidate's death with its mass's closed-form ejected share:
/// over 40,000 masses across band E, keyed as candidates are, the drops against the sum of their
/// odds, at α = 10⁻³.
#[test]
fn the_field_s_marks_drop_the_closed_form_s_share() {
    let galaxy = galaxy();
    let table = table();
    let cell = key([0, 12, 0]);
    let mut lcg = Lcg::new(0x0933_b002);
    let (lo, hi) = (MassBand::E.lo(), MassBand::E.hi());
    let (mut dropped, mut expected, mut variance) = (0_u32, 0.0, 0.0);
    for index in 0..40_000 {
        let m = SolarMasses::new(lo * crate::math::exp(lcg.next_f64() * crate::math::ln(hi / lo)));
        let id = cell.candidate_id(index, 0).unwrap();
        let mut stream = Stream::open(
            galaxy.seed(),
            tags::CLASS_CORE_COLLAPSE_EJECTION,
            ObjectKey::from(SystemId::from(id)),
        );
        dropped += u32::from(ejected_before_death(
            table,
            BirthSource::ThinDisc,
            m,
            &mut stream,
        ));
        let p = ejected_share(BirthSource::ThinDisc, m);
        expected += p;
        variance += p * (1.0 - p);
    }
    let z = (f64::from(dropped) - expected) / variance.sqrt();
    eprintln!("{dropped} of 40,000 dropped against {expected:.1}, z = {z:.2}");
    assert_p_value(
        "the field's ejected drops",
        2.0 * normal_cdf(-z.abs()),
        ALPHA,
    );
}

// 3. The envelope and the index.

/// The thinning's envelope holds: at random points of random cells, from the centre out through
/// the disc and well above it, the components' and the terms' weights together lie under the
/// cell's bound.
#[test]
fn the_envelope_holds_over_random_points_under_every_27_hold() {
    let galaxy = galaxy();
    let process = CoreCollapseProcess::new(galaxy);
    let displaced = galaxy.fields().displaced().expect("the displaced classes");
    let mut lcg = Lcg::new(0x0933_b003);
    let mut weights = [0.0; MAX_COMPONENTS];
    let mut all = Vec::new();
    let mut tested = 0;
    for _ in 0..3_000 {
        let span = |lcg: &mut Lcg, n: u64| {
            i32::try_from(lcg.next_below(2 * n)).unwrap() - i32::try_from(n).unwrap()
        };
        let cell = key([span(&mut lcg, 128), span(&mut lcg, 128), span(&mut lcg, 8)]);
        let bound = process.bound(galaxy, &cell).value();
        for _ in 0..8 {
            let p = cell.position_from_words([lcg.next_u64(), lcg.next_u64(), lcg.next_u64()]);
            let point = PointLy::from(&p);
            let n = process.weights(galaxy, &point, &mut weights);
            all.clear();
            all.extend_from_slice(&weights[..n]);
            process.ejected_weights(displaced, &point, &mut all);
            let total = all.iter().fold(0.0, |sum, w| sum + w);
            assert!(total <= bound, "{total} above {bound} in {cell:?}");
            tested += 1;
        }
    }
    assert_eq!(tested, 24_000);
}

/// No cell comes near the 24-bit index under every 27 hold: the bound over each octant of the root
/// cube, the components' and the terms' together, passes the headroom check.
#[test]
fn no_cell_comes_near_the_index_under_every_27_hold() {
    let galaxy = galaxy();
    let process = CoreCollapseProcess::new(galaxy);
    let fields = galaxy.fields();
    let displaced = fields.displaced().expect("the displaced classes");
    let half = 65_536;
    let mut largest = 0.0_f64;
    for octant in 0..8 {
        let min = [0, 1, 2].map(|axis| if octant >> axis & 1 == 1 { -half } else { 0 });
        let cube = CellBox::new(min, 65_536).expect("an octant of the root cube");
        let field = process.components().iter().fold(0.0, |sum, young| {
            sum + young.share()
                * fields.component_bound(young.id(), &cube)
                * young.deaths_per_system()
        });
        let bound = process.ejected_terms().iter().fold(field, |sum, term| {
            sum + term.systems()
                * displaced.bound(term.class(), &cube)
                * term.largest_share()
                * term.deaths_per_system()
        });
        largest = largest.max(bound);
    }
    let centre = process.bound(galaxy, &key([0, 0, 0])).value();
    assert!(centre <= largest);
    eprintln!(
        "under every 27 hold the fullest cell expects at most {:.0} candidates; the centre's {:.0}",
        largest * key([0, 0, 0]).volume_ly3(),
        centre * key([0, 0, 0]).volume_ly3()
    );
    assert_eq!(
        check_class_headroom(ClassId::CORE_COLLAPSE, PerCubicLightYear::new(largest)),
        Ok(())
    );
}

// 4. The entries.

/// Every runaway's or walkaway's entry is its term's and the shared test's: a band-E primary of
/// its term's component; its ejection a runaway's at 30 km/s or more or a walkaway's under 30,
/// at a speed inside its class's bin, at an age before its death at its source's reference
/// lifetime with the time from it to the death in its class's age bin; its velocity its
/// component's law at the site plus the ejection's; and the death in the interval, with a shell
/// the field claims.
#[test]
fn every_ejected_entry_is_its_term_s_and_the_shared_test_s() {
    let galaxy = galaxy();
    let process = CoreCollapseProcess::new(galaxy);
    let table = table();
    let displaced = galaxy.fields().displaced().expect("the displaced classes");
    let (mut field, mut ejected) = (0_u32, 0_u32);
    for (cell, entries) in sample_entries() {
        let bound = process.bound(galaxy, cell);
        for entry in entries {
            let id = entry.id();
            let value = entry.value();
            let candidate = process
                .candidate(galaxy, cell, id, bound)
                .expect("an entry's candidate");
            assert_eq!(candidate.ejection, value.ejection);
            let Some(ejection) = value.ejection() else {
                field += 1;
                continue;
            };
            ejected += 1;
            let class = displaced.class(ejection.class());
            let source = class
                .key()
                .source()
                .expect("a runaway's class has a source");
            assert_eq!(table.component_source(value.component()), source);
            let mass = value.record().primary_initial_mass();
            assert!(mass.value() >= MassBand::E.lo(), "{id:?}: {mass:?}");
            let v_ref = table.speed_scale(source).value();
            let speed = KilometresPerSecond::from(ejection.velocity().speed()).value();
            let (speed_bin, age_bin) = match class.key() {
                ClassKey::Thin { speed, age } => (speed, Some(age)),
                ClassKey::Old { speed, .. } => (speed, None),
                ClassKey::Hypervelocity => panic!("{id:?}: no runaway is a survivor"),
            };
            assert_eq!(
                SpeedBin::of(speed / v_ref),
                speed_bin,
                "{id:?}: {speed} km/s"
            );
            match ejection.kind() {
                DisplacedKind::Runaway => assert!(speed >= 30.0 - 1e-9, "{id:?}: {speed}"),
                DisplacedKind::Walkaway => assert!(speed <= 30.0 + 1e-9, "{id:?}: {speed}"),
                other => panic!("{id:?}: {other:?}"),
            }
            let life = table.reference_lifetime(source, MassBand::E, mass);
            assert!(ejection.age() < life, "{id:?}");
            if let Some(age_bin) = age_bin {
                let (lo, hi) = age_bin_years(age_bin, table.scales().tau_unit());
                let since = life.value() - ejection.age().value();
                assert!(
                    lo.value() <= since && hi.is_none_or(|hi| since < hi.value()),
                    "{id:?}: {since} yr after its ejection"
                );
            }
            let key = ObjectKey::from(SystemId::from(id));
            let law = site_velocity(galaxy, value.component(), &candidate.site, key);
            assert_eq!(law + ejection.velocity(), candidate.velocity);
            let death = RecentDeath::of(galaxy, &candidate).expect("an entry's death");
            assert!(claims(
                galaxy,
                &death.site,
                &death.marks,
                Surroundings::Field
            ));
            assert_eq!(death.site, *value.site());
        }
    }
    eprintln!("{ejected} ejected entries and {field} in the field");
    assert!(ejected > 20 && field > 2, "{ejected} {field}");
}

/// Complementarity for sampled runaway candidates: the twin grid record of each of the class's
/// runaway and walkaway candidates (a layer-E record of its component at its mass and age, placed
/// so that its explosion site is the candidate's) and a candidate with the twin's own marks and
/// velocity give the same death in the interval and the same claim, the class through its test's
/// halves and the grid through its carve-out's (P09.T35's `recent_death_claims`).
#[test]
fn a_runaway_s_twin_record_gives_the_same_death_and_claim() {
    let galaxy = galaxy();
    let process = CoreCollapseProcess::new(galaxy);
    let (mut twins, mut dying, mut claimed) = (0_u32, 0_u32, 0_u32);
    for cell in sample_cells() {
        let bound = process.bound(galaxy, &cell);
        for index in 0..candidate_count(galaxy.seed(), &cell, bound) {
            let id = cell.candidate_id(index, 0).unwrap();
            let Some(candidate) = process.candidate(galaxy, &cell, id, bound) else {
                continue;
            };
            if candidate.ejection.is_none() {
                continue;
            }
            let grid_cell = CellKey::containing(Layer::E, &candidate.site).unwrap();
            let twin = SystemRecord::from_parts(
                grid_cell.candidate_id(index).unwrap(),
                candidate.site,
                SystemOrigin::Grid(candidate.component),
                candidate.population,
                candidate.mass,
                candidate.age_at_epoch,
            );
            let grid_site = explosion_site(galaxy, &twin);
            let grid_claims =
                grid_site.is_some_and(|site| recent_death_claims(galaxy, &site, &twin));
            let marked = CoreCollapseCandidate {
                composition: draw_metallicity(galaxy, &twin),
                draws: primary_draws(galaxy, &twin),
                velocity: epoch_velocity(galaxy, &twin),
                site: grid_site.map_or(candidate.site, |site| *site.position()),
                ..candidate
            };
            // The shared test's first half on the class's side, as `RecentDeath::of_record` asks
            // it, then its second half.
            let class_death = death_in_interval_under(
                marked.mass,
                &marked.composition,
                marked.draws.eta(),
                marked.age_at_epoch,
                || marked.draws.clone(),
                galaxy.rules().stellar,
            );
            assert_eq!(class_death, grid_site.map(|site| site.death()), "{id:?}");
            let class_claims = RecentDeath::of_record(galaxy, &marked, &twin)
                .is_some_and(|d| claims(galaxy, &d.site, &d.marks, Surroundings::Field));
            assert_eq!(class_claims, grid_claims, "{id:?}");
            twins += 1;
            dying += u32::from(grid_site.is_some());
            claimed += u32::from(grid_claims);
        }
    }
    eprintln!("{twins} twins, {dying} dying in the interval, {claimed} claimed");
    assert!(claimed > 10, "{twins} {dying} {claimed}");
}

/// The cells under every 27 hold do not depend on the order they are generated in, nor on what was
/// resolved between them.
#[test]
fn the_27_cells_do_not_depend_on_order() {
    let galaxy = galaxy();
    let process = CoreCollapseProcess::new(galaxy);
    let cells: Vec<CatalogueCellKey> = sample_cells().into_iter().step_by(5).collect();
    assert_order_independent(&cells, |key| {
        let stray =
            CatalogueSystemId::new(ClassId::STELLAR_MERGER.value(), key.fine_cell(), 0, 0).unwrap();
        // Whether the stray ID names a system is beside the point: only asking it could disturb
        // what follows.
        let _ = crate::galaxy::placement::resolve(galaxy, SystemId::from(stray));
        CatalogueClassCell::generate(galaxy, &process, *key).into_entries()
    });
}

// 5. The closed form, galaxy-wide.

/// `galaxy`'s candidates over every cell of the root cube whose mean is not negligible, on 16
/// threads: their count, the ejected among them, the sum of their odds of being ejected, those
/// odds' variance, and the sum of their masses' ejected shares.
fn ejected_census(galaxy: &Galaxy) -> [f64; 5] {
    let process = CoreCollapseProcess::new(galaxy);
    let table = galaxy
        .class_table()
        .expect("every 27 hold installs the displaced classes");
    let field = galaxy.field_share();
    let column = |x: i32, y: i32, sums: &mut [f64; 5]| {
        for (start, step) in [(0, 1), (-1, -1)] {
            let mut z = start;
            while (-128..128).contains(&z) {
                let cell = key([x, y, z]);
                let bound = process.bound(galaxy, &cell);
                if bound.value() * cell.volume_ly3() < 1e-9 {
                    break;
                }
                for index in 0..candidate_count(galaxy.seed(), &cell, bound) {
                    let id = cell.candidate_id(index, 0).unwrap();
                    let Some(candidate) = process.candidate(galaxy, &cell, id, bound) else {
                        continue;
                    };
                    let source = table.component_source(candidate.component);
                    let p = closed_form_share(table, source, candidate.mass);
                    let keeps = field.map_or(1.0, |field| {
                        field.keeps(candidate.component, MassBand::E, candidate.age_at_epoch)
                    });
                    let odds = p / (p + (1.0 - p) * keeps);
                    sums[0] += 1.0;
                    sums[1] += f64::from(u8::from(candidate.ejection.is_some()));
                    sums[2] += odds;
                    sums[3] += odds * (1.0 - odds);
                    sums[4] += p;
                }
                z += step;
            }
        }
    };
    let workers: Vec<[f64; 5]> = std::thread::scope(|scope| {
        let column = &column;
        let handles: Vec<_> = (0..16)
            .map(|worker| {
                scope.spawn(move || {
                    let mut sums = [0.0; 5];
                    for x in (-128..128).filter(|x: &i32| x.rem_euclid(16) == worker) {
                        for y in -128..128 {
                            column(x, y, &mut sums);
                        }
                    }
                    sums
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("a census worker"))
            .collect()
    });
    workers.iter().fold([0.0; 5], |mut total, w| {
        for (t, v) in total.iter_mut().zip(w) {
            *t += v;
        }
        total
    })
}

/// Galaxy-wide, the candidates' ejected share is the closed form's: each candidate a runaway's
/// or a walkaway's with odds `P(m) ÷ (P(m) + (1 − P(m)) k(a))`, its mass's ejected share `P`
/// ([`ejected_share`]) against the field's, which keeps `k(a) = 1 − φ(a)` of its component's
/// systems of its age (P09.T2.c), and 1 where the field is the budget; summed over the whole root
/// cube, the ejected count against the odds' sum and their variance, at α = 10⁻³. Integrated over
/// the cube a class's form is one system's, so this holds whatever the forms. Under every 27 hold,
/// and under every 27 hold but the field share, where the odds are `P` alone.
#[test]
#[ignore = "slow: the class's candidates over the whole galaxy, twice"]
fn the_ejected_share_is_the_closed_form_galaxy_wide() {
    let budget = Galaxy::from_params_with_rules(
        Seed::new(SEED),
        GalaxyParams::milky_way_like(),
        GalaxyRules {
            field_share: crate::galaxy::ages::FieldShareRules::None,
            ..GalaxyRules::every_27_hold()
        },
    )
    .expect("the Milky Way fixture's gas is mostly neutral")
    .with_full_potential();
    for (name, galaxy) in [
        ("every 27 hold", galaxy()),
        ("every 27 hold but the field share", &budget),
    ] {
        let [n, ejected, expected, variance, closed] = ejected_census(galaxy);
        let z = (ejected - expected) / variance.sqrt();
        eprintln!(
            "{name}: {n} candidates, {ejected} of them ejected ({:.4}) against {expected:.1} \
             ({:.4}), z = {z:.2}; their masses' mean ejected share {:.4}",
            ejected / n,
            expected / n,
            closed / n
        );
        assert!(n > 10_000.0, "{name}: {n}");
        assert_p_value(
            &format!("{name}: the candidates' ejected share against the closed form"),
            2.0 * normal_cdf(-z.abs()),
            ALPHA,
        );
    }
}
