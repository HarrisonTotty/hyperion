//! The tests of P09.T33.b, [`RUNAWAY_DEATHS_VERSION`]: from 27 the core-collapse class follows the
//! young field's runaways and walkaways. The class at 26 is the held arm verbatim (the class's own
//! tests and golden hold it at the version's rules); these hold the new arm, under every 27 hold.
//!
//! - The tripwire; the held arm's process and cells are every 26 hold's, and its first entries of a
//!   cell are pinned bit for bit (the class's golden runs at 24, which holds no displaced class).
//! - The terms are the young field's band-E runaway and walkaway classes, each with a form, at the
//!   field's share of the component's budget; a term's largest share bounds its class's share of
//!   a star's deaths at every mass; a source's classes together take exactly the share the
//!   field's ejection marks drop, mass by mass, `RunawayModel`'s closed form; and the field's
//!   marks drop that share, at α = 10⁻³.
//! - The envelope holds over random points, and no cell comes near the index.
//! - The ejection's speed is its kind's law inside its bin.
//! - Every runaway's or walkaway's entry is its term's: its ejection in its class's speed and age
//!   bins before its death, its velocity its class's law (plan 08's), and the shared test's; a few
//!   are pinned bit for bit. A grid runaway's or walkaway's record dying in the interval and a
//!   class candidate with its marks and velocity give the same death and the same claim
//!   (complementarity), so the grid's carve-out claims the runaways the class places.
//! - The cells do not depend on the order they are generated in.
//! - Slow: galaxy-wide, the candidates' ejected share is the closed form's, each candidate's odds
//!   its mass's ejected share (α = 10⁻³), under every 27 hold and with the field at the budget.

use std::sync::OnceLock;

use hyperion_testkit::float::{assert_same_bits, bits};
use hyperion_testkit::lcg::Lcg;
use hyperion_testkit::order::assert_order_independent;
use hyperion_testkit::stats::{ALPHA, assert_p_value, normal_cdf};

use super::*;
use crate::GENERATOR_VERSION;
use crate::Seed;
use crate::galaxy::catalogue_classes::grid::candidate_count;
use crate::galaxy::catalogue_classes::{CatalogueClassCell, CatalogueEntry, check_class_headroom};
use crate::galaxy::displaced::PlacementClass;
use crate::galaxy::displaced::explosion_site;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{CellKey, generate_cell};
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
/// at the field's part of the component's band budget and the field term's largest death rate,
/// the field's components at their band budgets taken to the field; and every class of an
/// ejecting source that holds a share of a band-E star's deaths has its term, so none is lost.
#[test]
fn the_terms_are_the_young_field_s_runaway_and_walkaway_classes() {
    let galaxy = galaxy();
    let process = CoreCollapseProcess::new(galaxy);
    let displaced = galaxy.fields().displaced().expect("the displaced classes");
    let table = table();
    let fields = galaxy.fields();
    let mut order = Vec::new();
    for young in process.components() {
        let component = fields.component(young.id());
        let budget = galaxy.shares().share(MassBand::E, component.population());
        assert_same_bits(
            young.share(),
            galaxy.field_part(young.id(), MassBand::E, budget),
        );
    }
    for term in process.ejected_terms() {
        let component = fields.component(term.component);
        let class = displaced.class(term.class);
        assert!(class.form().is_some(), "{term:?}");
        match (component.population(), class.key()) {
            (Population::YoungThinDisc, ClassKey::Thin { speed, age }) => {
                assert_eq!((term.speed, term.age), (speed, Some(age)));
            }
            (Population::NuclearDisc, ClassKey::Old { source, speed }) => {
                assert_eq!(source.source(), BirthSource::NuclearDisc);
                assert_eq!((term.speed, term.age), (speed, None));
            }
            other => panic!("{other:?} is no young field's runaway class"),
        }
        assert_same_bits(
            term.systems,
            galaxy.field_part(
                term.component,
                MassBand::E,
                component.count() * galaxy.shares().share(MassBand::E, component.population()),
            ),
        );
        let young = process
            .components()
            .iter()
            .find(|young| young.id() == term.component)
            .expect("a term's component is young");
        assert_same_bits(term.deaths_per_system, young.deaths_per_system());
        order.push((term.component.index(), term.class.index()));
    }
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{order:?}");
    // No class with a share of some band-E star's deaths is left without a term.
    let mut parts = [NO_PART; MAX_PARTS];
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
            let takes =
                band_masses(256).any(|m| ejected_parts(table, &probe, m, &mut parts).1 > 0.0);
            let termed = process
                .ejected_terms()
                .iter()
                .any(|t| t.component == young.id() && t.class == id);
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
    let mut parts = [NO_PART; MAX_PARTS];
    for &m in &masses {
        for young in process.components() {
            let source = table.component_source(young.id());
            let mut taken = 0.0;
            for term in process
                .ejected_terms()
                .iter()
                .filter(|t| t.component == young.id())
            {
                let (count, share) = ejected_parts(table, term, m, &mut parts);
                assert!(share <= term.largest_share, "{m:?}: {share} over {term:?}");
                let summed = parts[..count]
                    .iter()
                    .fold(0.0, |sum, part| sum + part.share);
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

// 3. The envelope, the index and the speed law.

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
            let total = process.ejected_terms().iter().fold(
                weights[..n].iter().fold(0.0, |sum, w| sum + w),
                |sum, term| sum + term.weight_at(displaced, &point),
            );
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
            sum + term.systems
                * displaced.bound(term.class, &cube)
                * term.largest_share
                * term.deaths_per_system
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

/// The ejection's speed is its kind's law inside its bin: for each kind and each bin its law
/// reaches, at the thin disc's and the nuclear disc's speed scales, the speed at each rank lies in
/// the bin's part of the law, rises with the rank, starts at the part's lower end, and its CDF is
/// the rank's share of the part's; a bin the law misses gives its part's lower end.
#[test]
fn the_ejection_s_speed_is_its_kind_s_law_inside_its_bin() {
    let table = table();
    let model = table.runaway_model();
    for source in [BirthSource::ThinDisc, BirthSource::NuclearDisc] {
        let v_ref = table.speed_scale(source);
        for kind in KINDS {
            let shares = model.speed_bins(kind, v_ref);
            for bin in SpeedBin::all() {
                let cdf = |v: f64| model.speed_cdf(kind, KilometresPerSecond::new(v), v_ref);
                let speeds: Vec<f64> = (0..=64)
                    .map(|k| speed_in_bin(model, kind, bin, v_ref, f64::from(k) / 64.0).value())
                    .collect();
                if shares[bin.index()] <= 0.0 {
                    assert!(
                        speeds.iter().all(|&v| bits(v) == bits(speeds[0])),
                        "{kind:?} {bin:?}"
                    );
                    continue;
                }
                assert_eq!(SpeedBin::of(speeds[0] / v_ref.value()), bin, "{kind:?}");
                assert!(speeds.windows(2).all(|w| w[0] <= w[1]), "{kind:?} {bin:?}");
                let (low, high) = (cdf(speeds[0]), cdf(speeds[64]));
                assert!(
                    (high - low - shares[bin.index()]).abs() < 1e-9,
                    "{kind:?} {bin:?}"
                );
                for (k, &v) in speeds.iter().enumerate() {
                    let target = low + f64::from(u32::try_from(k).unwrap()) / 64.0 * (high - low);
                    assert!(
                        (cdf(v) - target).abs() < 1e-9,
                        "{kind:?} {bin:?} at {k}: {v}"
                    );
                }
            }
        }
    }
}

// 4. The entries.

/// Every runaway's or walkaway's entry is its term's and the shared test's: a band-E primary of
/// its term's component; its ejection a runaway's at 30 km/s or more or a walkaway's under 30,
/// at a speed inside its class's bin, at an age before its death at its source's reference
/// lifetime with the time from it to the death in its class's age bin; its velocity its class's
/// law at the site (plan 08's, the ballistic bins' with that speed); and the death in the
/// interval, with a shell the field claims.
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
            let speed = ejection.speed().value();
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
            let since = life.value() - ejection.age().value();
            if let Some(age_bin) = age_bin {
                let (lo, hi) = age_bin_years(age_bin, table.scales().tau_unit());
                assert!(
                    lo.value() <= since && hi.is_none_or(|hi| since < hi.value()),
                    "{id:?}: {since} yr after its ejection"
                );
            }
            // The class's law at the site, with the ejection's azimuth, word 2 of its stream.
            let key = ObjectKey::from(SystemId::from(id));
            let mut stream = Stream::open(galaxy.seed(), tags::CLASS_CORE_COLLAPSE_EJECTION, key);
            let _ = (stream.mark(), stream.uniform());
            let azimuth = std::f64::consts::TAU * stream.uniform();
            let ballistic = is_ballistic(class.key()).then(|| BallisticKick {
                height: LightYears::new(PointLy::from(&candidate.site).z),
                since: Years::new(since),
                speed,
                azimuth,
            });
            let law = ejected_velocity(
                galaxy,
                displaced,
                ejection.class(),
                &candidate.site,
                ballistic,
                key,
            );
            assert_eq!(law, candidate.velocity, "{id:?}");
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

/// Complementarity, on the grid's own runaways and walkaways: each layer-E grid record of a
/// runaway or walkaway class, over cells along the inner disc, moved to an age at which its
/// primary died in the interval's last 4 Myr (its own lifetime, every other mark kept: a living
/// runaway dies within the interval only in its last +H), and a class candidate with that
/// record's marks and velocity (its mass, age, metallicity, draws, epoch velocity by its class's
/// law and explosion site) give the same death in the interval and the same claim, the class
/// through its test's halves and the grid through its carve-out's (P09.T35's
/// `recent_death_claims`, which placement asks of every layer-E record, a runaway's with the
/// rest).
#[test]
fn the_grid_s_runaways_and_the_class_agree_on_their_deaths_and_claims() {
    let galaxy = galaxy();
    let rules = galaxy.rules().stellar;
    let id = key([0, 12, 0]).candidate_id(0, 0).unwrap();
    let mut lcg = Lcg::new(0x0933_b004);
    let (mut seen, mut dying, mut claimed) = (0_u32, 0_u32, 0_u32);
    let mut systems = Vec::new();
    'cells: for y in (4_000..30_000).step_by(128) {
        for x in [-128, 0, 128] {
            let place =
                GalacticPosition::from_light_years([f64::from(x), f64::from(y), 0.0]).unwrap();
            generate_cell(
                galaxy,
                CellKey::containing(Layer::E, &place).unwrap(),
                &mut systems,
            );
            for living in &systems {
                let PlacementClass::Displaced { kind, .. } = living.placement_class() else {
                    continue;
                };
                if !matches!(kind, DisplacedKind::Runaway | DisplacedKind::Walkaway) {
                    continue;
                }
                seen += 1;
                let composition = draw_metallicity(galaxy, living);
                let draws = primary_draws(galaxy, living);
                let life = lifetime_under(
                    track_mass_of(living.primary_initial_mass()),
                    &composition,
                    &draws,
                    rules,
                );
                let record =
                    living.with_age_at_epoch(Years::new(life.value() + 4.0e6 * lcg.next_f64()));
                let Some(site) = explosion_site(galaxy, &record) else {
                    continue;
                };
                dying += 1;
                let marked = CoreCollapseCandidate {
                    id,
                    site: *site.position(),
                    component: record.component().expect("a grid record has its component"),
                    population: record.population(),
                    mass: record.primary_initial_mass(),
                    composition,
                    draws,
                    age_at_epoch: record.age_at_epoch(),
                    velocity: epoch_velocity(galaxy, &record),
                    ejection: None,
                };
                let class_death = death_in_interval_under(
                    marked.mass,
                    &marked.composition,
                    marked.draws.eta(),
                    marked.age_at_epoch,
                    || marked.draws.clone(),
                    rules,
                );
                assert_eq!(class_death, Some(site.death()), "{:?}", record.id());
                let class_claims = RecentDeath::of_record(galaxy, &marked, &record)
                    .is_some_and(|d| claims(galaxy, &d.site, &d.marks, Surroundings::Field));
                let grid_claims = recent_death_claims(galaxy, &site, &record);
                assert_eq!(class_claims, grid_claims, "{:?}", record.id());
                claimed += u32::from(grid_claims);
                if claimed >= 8 {
                    break 'cells;
                }
            }
        }
    }
    eprintln!(
        "{seen} grid runaways and walkaways, {dying} dying in the interval, {claimed} claimed"
    );
    assert!(dying > 20 && claimed >= 5, "{seen} {dying} {claimed}");
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

// 5. The pins.

/// Entries' marks, bit for bit, the first `n`: each one's primary mass, age at the epoch, [Fe/H]
/// and velocity, and its ejection's speed (0 for a death in the field).
fn entry_bits<'a>(
    entries: impl IntoIterator<Item = &'a CoreCollapseEntry>,
    n: usize,
) -> Vec<[u64; 7]> {
    entries
        .into_iter()
        .take(n)
        .map(|value| {
            let record = value.record();
            let v = value
                .motion()
                .velocity_at(UniverseTime::EPOCH)
                .metres_per_second();
            [
                bits(record.primary_initial_mass().value()),
                bits(record.age_at_epoch().value()),
                bits(value.composition().fe_h().value()),
                bits(v[0]),
                bits(v[1]),
                bits(v[2]),
                bits(
                    value
                        .ejection()
                        .map_or(0.0, |ejection| ejection.speed().value()),
                ),
            ]
        })
        .collect()
}

/// The held arm's bound over the inner disc's cell (0, 12, 0) under every 26 hold, and the first
/// two entries' marks of the sample cells, as measured (2026-11-01).
const HELD_PIN: (u64, [[u64; 7]; 2]) = (
    0x3e6d_875c_4958_11ff,
    [
        [
            0x402c_e0d9_4d04_2f86,
            0x416c_2c58_ff47_ac65,
            0x3fc0_fac1_1ea3_b3df,
            0xc118_1c8e_ec6f_553a,
            0xc109_0457_c966_d060,
            0x410a_d0b2_37d0_2d19,
            0x0000_0000_0000_0000,
        ],
        [
            0x402e_5e2e_c3cc_6733,
            0x416c_35b3_7288_343d,
            0x3f73_020e_10dd_be60,
            0x40d2_f218_78ac_fec4,
            0xc0f9_dd2d_5aaf_268b,
            0xc0df_dbf6_e7c1_c7e3,
            0x0000_0000_0000_0000,
        ],
    ],
);

/// Under every 26 hold the class's bound over a cell of the inner disc and the sample cells' first
/// entries are pinned bit for bit: the held arm in a galaxy holding the displaced classes, which the class's
/// golden, at 24, never builds. The bump to 27 deletes the pin with the hold.
#[test]
fn the_held_arm_s_cell_is_pinned_under_every_26_hold() {
    let galaxy = Galaxy::from_params_with_rules(
        Seed::new(SEED),
        GalaxyParams::milky_way_like(),
        GalaxyRules::every_26_hold(),
    )
    .expect("the Milky Way fixture's gas is mostly neutral")
    .with_full_potential();
    let process = CoreCollapseProcess::new(&galaxy);
    let bound = bits(process.bound(&galaxy, &key([0, 12, 0])).value());
    let mut entries = Vec::new();
    for cell in sample_cells() {
        entries.extend(CatalogueClassCell::generate(&galaxy, &process, cell).into_entries());
        if entries.len() >= 2 {
            break;
        }
    }
    let marks = entry_bits(entries.iter().map(CatalogueEntry::value), 2);
    eprintln!("bound {bound:#018x}, entries {marks:#018x?}");
    assert_eq!((bound, marks.as_slice()), (HELD_PIN.0, &HELD_PIN.1[..]));
}

/// The new arm's first runaway and walkaway entries of the sample cells, as measured
/// (2026-11-01).
const EJECTED_PIN: [[u64; 7]; 3] = [
    [
        0x4036_83f3_3563_62aa,
        0x4162_ae36_3d34_1ccb,
        0xbfc4_a14b_a4e8_3238,
        0xc106_ed24_3a31_8c2a,
        0x40fb_9f9a_e917_b1d2,
        0x40d8_b616_9b10_a533,
        0x4042_f379_0a57_0a9a,
    ],
    [
        0x4025_9cca_6738_e99d,
        0x4177_f87d_f836_84fb,
        0xbfc6_f281_d95e_2dac,
        0xc106_0d0b_a48c_2739,
        0x40b7_7b5c_33b9_ab52,
        0xc0c2_eefd_af27_d24e,
        0x4025_44d7_6967_64f4,
    ],
    [
        0x402c_e0d9_4d04_2f86,
        0x416d_c60c_ba45_3461,
        0xbfb2_4a48_a8ad_c89e,
        0xc119_8402_92cc_4b8f,
        0xc105_2ad0_542f_05be,
        0x410f_e621_db25_404a,
        0x4054_4f94_5041_2c1c,
    ],
];

/// Under every 27 hold the first runaway and walkaway entries of the sample cells are pinned bit
/// for bit, so that the WebAssembly run checks the new arm's arithmetic (its speed's bisection,
/// its class's velocity) against the native one before the bump's golden does.
#[test]
fn the_new_arm_s_ejected_entries_are_pinned() {
    let ejected = sample_entries()
        .iter()
        .flat_map(|(_, cell)| cell.iter().map(CatalogueEntry::value))
        .filter(|value| value.ejection().is_some());
    let marks = entry_bits(ejected, EJECTED_PIN.len());
    eprintln!("{marks:#018x?}");
    assert_eq!(marks.as_slice(), &EJECTED_PIN[..]);
}

// 6. The closed form, galaxy-wide.

/// `galaxy`'s candidates over every cell of the root cube whose mean is not negligible, on 16
/// threads: their count, the ejected among them, and the sum of their masses' ejected shares and
/// of those shares' variances.
fn ejected_census(galaxy: &Galaxy) -> [f64; 4] {
    let process = CoreCollapseProcess::new(galaxy);
    let table = galaxy
        .class_table()
        .expect("every 27 hold installs the displaced classes");
    let column = |x: i32, y: i32, sums: &mut [f64; 4]| {
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
                    sums[0] += 1.0;
                    sums[1] += f64::from(u8::from(candidate.ejection.is_some()));
                    sums[2] += p;
                    sums[3] += p * (1.0 - p);
                }
                z += step;
            }
        }
    };
    let workers: Vec<[f64; 4]> = std::thread::scope(|scope| {
        let column = &column;
        let handles: Vec<_> = (0..16)
            .map(|worker| {
                scope.spawn(move || {
                    let mut sums = [0.0; 4];
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
    workers.iter().fold([0.0; 4], |mut total, w| {
        for (t, v) in total.iter_mut().zip(w) {
            *t += v;
        }
        total
    })
}

/// Galaxy-wide, the candidates' ejected share is the closed form's: each candidate a runaway's or
/// a walkaway's with odds its mass's ejected share ([`ejected_share`]), the field's part of each
/// mass's deaths split between the field term and the classes alike; summed over the whole root
/// cube, the ejected count against the shares' sum and their variance, at α = 10⁻³. Integrated
/// over the cube a class's form is one system's, so this holds whatever the forms. Under every 27
/// hold, and under every 27 hold but the field share, where the field is the budget.
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
        let [n, ejected, expected, variance] = ejected_census(galaxy);
        let z = (ejected - expected) / variance.sqrt();
        eprintln!(
            "{name}: {n} candidates, {ejected} of them ejected ({:.4}) against {expected:.1} \
             ({:.4}), z = {z:.2}",
            ejected / n,
            expected / n,
        );
        assert!(n > 10_000.0, "{name}: {n}");
        assert_p_value(
            &format!("{name}: the candidates' ejected share against the closed form"),
            2.0 * normal_cdf(-z.abs()),
            ALPHA,
        );
    }
}
