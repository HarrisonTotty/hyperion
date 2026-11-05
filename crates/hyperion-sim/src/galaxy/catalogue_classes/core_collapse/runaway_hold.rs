//! The tests of P09.T33.b: from generator version 27 the core-collapse class follows the young
//! field's runaways and walkaways, under every 27 hold.
//!
//! - The galaxy keeps one process, a fresh one's.
//! - The terms are the young field's band-E runaway and walkaway classes, each with a form, a term
//!   per kind, the runaways' at the component's whole budget and the walkaways' at the field's
//!   part of it; a term's largest share bounds its class's share of a star's deaths as its kind at
//!   every mass; a source's classes together take exactly each kind's share of `RunawayModel`'s
//!   closed form, mass by mass, which the field's ejection marks drop, at α = 10⁻³.
//! - A feature's list drops its runaways' deaths by the same marks on its members' own keys, at
//!   the closed form's runaway share (α = 10⁻³), and a nursery's list keeps none of them.
//! - The envelope holds over random points, and no cell comes near the index.
//! - The ejection's speed is its kind's law inside its bin.
//! - Every runaway's or walkaway's entry is its term's: its ejection in its class's speed and age
//!   bins before its death, its velocity its class's law (plan 08's), and the shared test's; a few
//!   are pinned bit for bit. A grid runaway's or walkaway's record dying in the interval and a
//!   class candidate with its marks and velocity give the same death and the same claim
//!   (complementarity), so the grid's carve-out claims the runaways the class places.
//! - The cells do not depend on the order they are generated in.
//! - Slow: galaxy-wide, the candidates' ejected share is the closed form's, each candidate's odds
//!   its mass's and age's (α = 10⁻³), under every 27 hold and with the field at the budget.

use std::sync::OnceLock;

use hyperion_testkit::float::{assert_same_bits, bits};
use hyperion_testkit::lcg::Lcg;
use hyperion_testkit::order::assert_order_independent;
use hyperion_testkit::stats::{ALPHA, assert_p_value, normal_cdf};

use super::*;
use crate::Seed;
use crate::galaxy::catalogue_classes::grid::candidate_count;
use crate::galaxy::catalogue_classes::{CatalogueClassCell, CatalogueEntry, check_class_headroom};

use crate::galaxy::displaced::PlacementClass;
use crate::galaxy::displaced::death_in_interval_under;
use crate::galaxy::displaced::explosion_site;
use crate::galaxy::features::catalogue::{FeatureCatalogue, FeatureRecord};
use crate::galaxy::features::ids::FeatureProcess;
use crate::galaxy::features::members::FeatureLevelList;
use crate::galaxy::features::members::recent::RecentClass;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{CellKey, generate_cell};
use crate::galaxy::rules::GalaxyRules;
use crate::id::Layer;
use crate::id::{FeatureMemberId, MemberSlot, SystemIdKind};
use crate::stellar::sse::lifetime_under;
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
            GalaxyRules::of_version(crate::GENERATOR_VERSION),
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
/// reference lifetime under `table`, the closed form: each kind's ([`closed_form_kind_share`]).
pub(super) fn closed_form_share(table: &ClassTable, source: BirthSource, m: SolarMasses) -> f64 {
    KINDS.iter().fold(0.0, |sum, &kind| {
        sum + closed_form_kind_share(table, source, kind, m)
    })
}

/// The share of a band-E star of initial mass `m` of `source` ejected as `kind` before its death
/// at its reference lifetime under `table`, the closed form: the kind's lifetime share × the
/// weight of its ejection ages below that lifetime.
fn closed_form_kind_share(
    table: &ClassTable,
    source: BirthSource,
    kind: Ejected,
    m: SolarMasses,
) -> f64 {
    let life = table.reference_lifetime(source, MassBand::E, m);
    let before = table
        .ejection_ages(source, kind, m)
        .iter()
        .filter(|(age, _)| *age < life)
        .fold(0.0, |s, (_, w)| s + w);
    lifetime_share(table.runaway_model(), kind, m) * before
}

/// The oldest age at the epoch, years, a death in an interval of `span` Julian years of a star of
/// band E's node interval `i` of `table` can have: the lifetime bracket's longest lifetime there,
/// widened by [`LIFETIME_MARGIN`], plus the interval.
fn oldest_death_age(table: &ClassTable, i: usize, span: f64) -> f64 {
    let m = table.nodes(MassBand::E).masses()[i];
    let (_, longest) = LifetimeBracket::shared()
        .bracket(SolarMasses::new(m))
        .unwrap();
    longest.value() * (1.0 + LIFETIME_MARGIN) + span
}

/// The node intervals of `table`'s band E whose closed mass range holds `m`: one, or two at a
/// node.
fn intervals_holding(table: &ClassTable, m: SolarMasses) -> Vec<usize> {
    let masses = table.nodes(MassBand::E).masses();
    (0..INTERVALS)
        .filter(|&i| (masses[i]..=masses[i + 1]).contains(&m.value()))
        .collect()
}

/// The young thin disc's component in `galaxy`.
fn young_disc_of(galaxy: &Galaxy) -> ComponentId {
    galaxy
        .fields()
        .component_ids()
        .find(|&id| galaxy.fields().component(id).population() == Population::YoungThinDisc)
        .expect("the fixture has a young thin disc")
}

/// What the field keeps of component `id`'s band-E systems of age `age`: `1 − φ(age)` for the
/// young disc, scaled to its field factor (`FieldShare::keeps`), and 1 where the field is the
/// budget.
fn field_keeps(galaxy: &Galaxy, id: ComponentId, age: Years) -> f64 {
    galaxy
        .field_share()
        .map_or(1.0, |field| field.keeps(id, MassBand::E, age))
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

/// The galaxy keeps the process the production lookup visits, built once and equal to a fresh
/// one.
#[test]
fn the_galaxy_keeps_one_process_per_rules() {
    let galaxy = galaxy().clone();
    let kept = galaxy.core_collapse();
    assert!(std::ptr::eq(kept, galaxy.core_collapse()), "built once");
    assert_eq!(*kept, CoreCollapseProcess::new(&galaxy));
}

// 2. The terms and the closed form.

/// The terms are the young field's: the young thin disc's thin-disc classes and each nuclear-disc
/// component's nuclear-disc classes, each with a form, a term for each kind its speed law puts in
/// the class's bin, in the components', the classes' and the kinds' order; a runaway's at the
/// component's whole band budget and the largest death rate of its ages, a walkaway's at the
/// field's part of it and the field term's largest death rate; and the field's components at
/// their band budgets taken to the field.
#[test]
fn the_terms_are_the_young_field_s_runaway_and_walkaway_classes() {
    let galaxy = galaxy();
    let process = CoreCollapseProcess::new(galaxy);
    let displaced = galaxy.fields().displaced().expect("the displaced classes");
    let fields = galaxy.fields();
    let floor = Years::new(-CLOCK_WINDOW_H.as_julian_years_f64());
    let interval = process.interval().length().as_julian_years_f64();
    let table = table();
    let intervals = MassIntervals::of(galaxy, table, interval);
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
        assert!(
            process
                .components()
                .iter()
                .any(|young| young.id() == term.component),
            "a term's component is young"
        );
        let budget = component.count() * galaxy.shares().share(MassBand::E, component.population());
        let (systems, ages) = match term.kind {
            Ejected::Runaway => (budget, component.ages()),
            Ejected::Walkaway => (
                galaxy.field_part(term.component, MassBand::E, budget),
                galaxy.field_ages(term.component),
            ),
        };
        assert_same_bits(term.systems, systems);
        // Its rates are its ages' below each mass interval's oldest death age, never above the
        // densest below 100 Myr; its odds the interval's share × its share bound × its rate.
        let densest =
            ages.max_pdf_between(floor, RECENT_DEATH_AGES) * (1.0 + RATE_MARGIN) * interval;
        let expected = intervals.rates(ages, interval, &term.shares);
        assert_eq!(term.rates, expected);
        for i in 0..INTERVALS {
            let oldest = oldest_death_age(table, i, interval);
            assert_same_bits(
                term.rates.rates[i],
                ages.max_pdf_between(floor, Years::new(oldest)) * (1.0 + RATE_MARGIN) * interval,
            );
            assert!(term.rates.rates[i] <= densest);
        }
        assert_same_bits(term.weight, term.rates.total());
        order.push((
            term.component.index(),
            term.class.index(),
            kind_slot(term.kind),
        ));
    }
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{order:?}");
    // Every walkaway is in speed bin 0, so a faster class has a runaway's term alone; the young
    // disc's runaways weigh its whole budget, more than the field's part the walkaways weigh.
    let terms = process.ejected_terms();
    assert!(
        terms
            .iter()
            .filter(|t| t.kind == Ejected::Walkaway)
            .all(|t| t.speed.index() == 0)
    );
    let young_disc = young_disc_of(galaxy);
    let systems_of = |kind| {
        terms
            .iter()
            .find(|t| t.component == young_disc && t.kind == kind)
            .expect("the young disc has both kinds' terms")
            .systems
    };
    assert!(systems_of(Ejected::Runaway) > systems_of(Ejected::Walkaway));
}

/// Every class and kind of an ejecting source that holds a share of some band-E star's deaths has
/// its term, so none is lost.
#[test]
fn every_class_and_kind_with_a_share_has_its_term() {
    let galaxy = galaxy();
    let process = CoreCollapseProcess::new(galaxy);
    let displaced = galaxy.fields().displaced().expect("the displaced classes");
    let table = table();
    let terms = process.ejected_terms();
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
            for kind in KINDS {
                let probe = EjectedTerm {
                    class: id,
                    kind,
                    component: young.id(),
                    source,
                    speed,
                    age,
                    systems: 1.0,
                    shares: [1.0; INTERVALS],
                    rates: MassRates {
                        rates: [1.0; INTERVALS],
                        odds: [1.0; INTERVALS],
                    },
                    weight: 1.0,
                };
                let takes =
                    band_masses(256).any(|m| ejected_parts(table, &probe, m, &mut parts).1 > 0.0);
                let termed = terms
                    .iter()
                    .any(|t| t.component == young.id() && t.class == id && t.kind == kind);
                assert_eq!(
                    takes,
                    termed,
                    "{:?} of {:?} as {kind:?}",
                    class.key(),
                    young.id()
                );
            }
        }
    }
}

/// A term's largest share bounds its class's share of a band-E star's deaths as its kind at every
/// mass, over a fine grid and random masses; and at each mass a source's classes together take
/// as each kind the closed form's share of that kind, so that the runaways' terms take the
/// component's whole budget's runaway deaths and the walkaways' the field's part of its walkaway
/// deaths, to 10⁻¹²: what the field's marks drop of the field's, and the features' lists of
/// their runaways.
#[test]
fn the_classes_take_the_closed_form_s_share_mass_by_mass() {
    let galaxy = galaxy();
    let process = CoreCollapseProcess::new(galaxy);
    let table = table();
    let fields = galaxy.fields();
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
            let component = fields.component(young.id());
            let budget =
                component.count() * galaxy.shares().share(MassBand::E, component.population());
            let mut taken = 0.0;
            for kind in KINDS {
                let (mut share_taken, mut deaths) = (0.0, 0.0);
                for term in process
                    .ejected_terms()
                    .iter()
                    .filter(|t| t.component == young.id() && t.kind == kind)
                {
                    let (count, share) = ejected_parts(table, term, m, &mut parts);
                    for i in intervals_holding(table, m) {
                        assert!(
                            share <= term.shares[i],
                            "{m:?}: {share} over interval {i}'s {}",
                            term.shares[i]
                        );
                    }
                    let summed = parts[..count]
                        .iter()
                        .fold(0.0, |sum, part| sum + part.share);
                    assert_same_bits(summed, share);
                    share_taken += share;
                    deaths += term.systems * share;
                }
                let closed = closed_form_kind_share(table, source, kind, m);
                assert!(
                    (share_taken - closed).abs() <= 1e-12,
                    "{m:?} of {source:?} as {kind:?}: the classes take {share_taken}, the closed \
                     form {closed}"
                );
                let systems = match kind {
                    Ejected::Runaway => budget,
                    Ejected::Walkaway => galaxy.field_part(young.id(), MassBand::E, budget),
                };
                assert!(
                    (deaths - systems * closed).abs() <= 1e-12 * systems,
                    "{m:?} of {source:?} as {kind:?}: {deaths} deaths against {}",
                    systems * closed
                );
                taken += share_taken;
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
        dropped += u32::from(
            ejection_before_death(table, BirthSource::ThinDisc, m, &mut stream).is_some(),
        );
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

/// A feature's list drops a candidate's death with its mass's closed-form runaway share, by the
/// field's ejection marks on the member's own key ([`feature_runaway`]): over 40,000 masses
/// across band E, keyed as the young disc's nurseries' list entries are, the drops against the
/// sum of their odds, at α = 10⁻³. A member's key is a feature-level member ID's, prefix `0`,
/// and never a class candidate's, prefix `111`.
#[test]
fn a_feature_s_list_drops_its_runaways_deaths() {
    let galaxy = galaxy();
    let table = table();
    let young = young_disc_of(galaxy);
    let nurseries: Vec<FeatureRecord> =
        FeatureCatalogue::walk_process(galaxy, FeatureProcess::Nursery)
            .filter(|f| f.component() == Some(young))
            .take(5)
            .collect();
    assert_eq!(nurseries.len(), 5);
    let mut lcg = Lcg::new(0x0933_b005);
    let (lo, hi) = (MassBand::E.lo(), MassBand::E.hi());
    let (mut dropped, mut expected, mut variance) = (0_u32, 0.0, 0.0);
    for index in 0..40_000_u32 {
        let feature = &nurseries[usize::try_from(index / 8_000).unwrap()];
        let member = FeatureMemberId::new(
            feature.id().feature_ref(),
            MemberSlot::FeatureLevel {
                index: u16::try_from(index % 8_000).unwrap(),
            },
        )
        .unwrap();
        let system = SystemId::from(member);
        assert!(matches!(system.kind(), SystemIdKind::FeatureMember(_)));
        let key = ObjectKey::from(system);
        let m = SolarMasses::new(lo * crate::math::exp(lcg.next_f64() * crate::math::ln(hi / lo)));
        dropped += u32::from(feature_runaway(galaxy, young, m, key));
        let p = closed_form_kind_share(table, BirthSource::ThinDisc, Ejected::Runaway, m);
        expected += p;
        variance += p * (1.0 - p);
    }
    let z = (f64::from(dropped) - expected) / variance.sqrt();
    eprintln!("{dropped} of 40,000 members' deaths dropped against {expected:.1}, z = {z:.2}");
    assert_p_value(
        "the features' runaway drops",
        2.0 * normal_cdf(-z.abs()),
        ALPHA,
    );
}

/// A nursery's list under every 27 hold keeps no core collapse whose member its ejection marks make
/// a runaway: every recent core collapse of the young disc's nurseries is a death its member's
/// marks leave in the feature, walkaways' deaths among them.
#[test]
fn a_nursery_s_list_keeps_no_runaway_s_death() {
    let galaxy = galaxy();
    let young = young_disc_of(galaxy);
    let mut kept = 0;
    for feature in FeatureCatalogue::walk_process(galaxy, FeatureProcess::Nursery)
        .filter(|f| f.component() == Some(young))
    {
        let list = FeatureLevelList::of(galaxy, &feature);
        for entry in list.recent_entries(galaxy) {
            if entry.class() != RecentClass::CoreCollapse {
                continue;
            }
            let record = entry.member().record();
            let key = ObjectKey::from(record.id());
            assert!(
                !feature_runaway(galaxy, young, record.primary_initial_mass(), key),
                "{:?} is a runaway's death",
                record.id()
            );
            kept += 1;
        }
        if kept >= 20 {
            break;
        }
    }
    eprintln!("{kept} core collapses kept");
    assert!(kept >= 20, "{kept}");
}

/// From 27 a field component's death rate is bounded by mass interval: its field ages' densest
/// below each interval's oldest death age, the odds the interval's share of band E × that rate,
/// summing to the component's deaths per system in the envelope.
/// The young disc's rate in the envelope falls to about a third of its densest below 100 Myr.
#[test]
fn the_field_s_death_rate_is_bounded_where_band_e_dies() {
    let galaxy = galaxy();
    let process = CoreCollapseProcess::new(galaxy);
    let interval = process.interval().length().as_julian_years_f64();
    let intervals = MassIntervals::of(galaxy, table(), interval);
    let floor = Years::new(-CLOCK_WINDOW_H.as_julian_years_f64());
    for young in process.components() {
        let ages = galaxy.field_ages(young.id());
        let by_mass = young
            .by_mass
            .as_ref()
            .expect("from 27 a component's rates by mass");
        assert_eq!(*by_mass, intervals.rates(ages, interval, &[1.0; INTERVALS]));
        assert_same_bits(young.deaths_per_system(), by_mass.total());
        let densest =
            ages.max_pdf_between(floor, RECENT_DEATH_AGES) * (1.0 + RATE_MARGIN) * interval;
        assert!(young.deaths_per_system() <= densest);
        eprintln!(
            "{:?}: deaths per system {:.4e} against {densest:.4e} ({:.3})",
            young.id(),
            young.deaths_per_system(),
            young.deaths_per_system() / densest
        );
        if young.id() == young_disc_of(galaxy) {
            assert!(young.deaths_per_system() < 0.5 * densest);
        }
    }
}

/// The lifetime bracket, widened by [`LIFETIME_MARGIN`], bounds the lifetimes of band E's stars
/// that the bounds by mass rest on: over 256 stars across the band, at metallicities from
/// −1 to +0.6 dex, past the bracket's own range, and their own draws, each lifetime under every
/// 27 hold's stellar rules lies below its interval's oldest death age less the interval.
#[test]
fn the_widened_bracket_bounds_band_e_s_lifetimes() {
    let galaxy = galaxy();
    let table = table();
    let rules = galaxy.rules().stellar;
    let span = CoreCollapseProcess::new(galaxy)
        .interval()
        .length()
        .as_julian_years_f64();
    let mut lcg = Lcg::new(0x0933_b006);
    let (lo, hi) = (MassBand::E.lo(), MassBand::E.hi());
    let mut widest = 0.0_f64;
    for n in 0..256_u64 {
        let m = SolarMasses::new(lo * crate::math::exp(lcg.next_f64() * crate::math::ln(hi / lo)));
        let composition =
            Composition::from_fe_h(Dex::new(-1.0 + 1.6 * lcg.next_f64()), HeliumExcess::ZERO);
        let system = key([0, 12, 0])
            .candidate_id(u32::try_from(n).unwrap(), 0)
            .unwrap();
        let draws = StarDraws::for_star(galaxy.seed(), BodyId::new(SystemId::from(system), 0));
        let life = lifetime_under(track_mass_of(m), &composition, &draws, rules).value();
        let (_, longest) = LifetimeBracket::shared().bracket(m).unwrap();
        widest = widest.max(life / longest.value());
        for i in intervals_holding(table, m) {
            let bound = oldest_death_age(table, i, span) - span;
            assert!(
                life <= bound,
                "{m:?} at {composition:?}: {life} past {bound}"
            );
        }
    }
    eprintln!("the longest lifetime against the bracket's: {widest:.4}");
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
                |sum, term| sum + term.weight_at(displaced, &FormPoint::new(&point, None)),
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
            sum + term.systems * displaced.bound(term.class, &cube) * term.weight
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
    // 15 and 12 at generator version 26.
    assert!(ejected > 10 && field > 2, "{ejected} {field}");
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
                    lifetime: life,
                    track: None,
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

/// The screen refuses no entry: in the centre's cell and the sample cells, under every 27 hold,
/// each candidate the unscreened thinning keeps and the shared test passes is screened through,
/// to the same entry bit for bit, and the screened cells hold those entries alone. At the centre
/// the screen refuses most of the candidates the test would refuse.
#[test]
fn the_screen_refuses_no_entry() {
    let galaxy = galaxy();
    let process = CoreCollapseProcess::new(galaxy);
    let cells: Vec<CatalogueCellKey> = sample_cells().into_iter().step_by(3).collect();
    let (mut tested, mut refused, mut kept) = (0_u32, 0_u32, 0_u32);
    for cell in cells.iter().chain([&key([0, 0, 0])]) {
        let bound = process.bound(galaxy, cell);
        let mut expected = Vec::new();
        for index in 0..candidate_count(galaxy.seed(), cell, bound) {
            let id = cell.candidate_id(index, 0).unwrap();
            let Some(candidate) = process.unscreened_candidate(galaxy, cell, id, bound) else {
                continue;
            };
            tested += 1;
            let screened = process.candidate(galaxy, cell, id, bound);
            refused += u32::from(screened.is_none());
            let Some(entry) = process.test(galaxy, candidate.clone()) else {
                continue;
            };
            kept += 1;
            assert_eq!(screened.as_ref(), Some(&candidate), "{id:?}");
            expected.push((id, entry));
        }
        let generated: Vec<_> = CatalogueClassCell::generate(galaxy, &process, *cell)
            .into_entries()
            .into_iter()
            .map(|entry| (entry.id(), *entry.value()))
            .collect();
        assert_eq!(generated, expected, "{cell:?}");
    }
    eprintln!("{tested} candidates tested, {refused} refused by the screen, {kept} kept");
    assert!(
        kept > 20 && refused > (tested - kept) / 2,
        "{tested} {refused} {kept}"
    );
}

/// P09.T37's bench: a candidate keeps the whole track its step 3 built, whose lifetime it carries,
/// the bits of a build that keeps no segment, and its test builds the primary's stars on it, the stars built again before, bit for bit: in
/// the centre's cell and the sample cells, under every 27 hold, for every candidate the thinning
/// keeps, of the field and of the ejected terms, each dying by the clock window's end.
#[test]
fn a_candidate_s_test_builds_its_stars_on_the_candidate_s_track() {
    let galaxy = galaxy();
    let process = CoreCollapseProcess::new(galaxy);
    let cells: Vec<CatalogueCellKey> = sample_cells().into_iter().step_by(3).collect();
    let (mut kept, mut ejected, mut dying) = (0_u32, 0_u32, 0_u32);
    for cell in cells.iter().chain([&key([0, 0, 0])]) {
        let bound = process.bound(galaxy, cell);
        for index in 0..candidate_count(galaxy.seed(), cell, bound) {
            let id = cell.candidate_id(index, 0).unwrap();
            let Some(candidate) = process.candidate(galaxy, cell, id, bound) else {
                continue;
            };
            let track = candidate
                .track
                .as_deref()
                .expect("a candidate keeps its track");
            assert_eq!(track.lifetime(), Some(candidate.lifetime), "{id:?}");
            // The exact tier reads it as the build that keeps no segment finds it.
            let unkept = lifetime_under(
                track_mass_of(candidate.mass),
                &candidate.composition,
                &candidate.draws,
                galaxy.rules().stellar,
            );
            assert_same_bits(candidate.lifetime.value(), unkept.value());
            let record = candidate.record(candidate.site);
            let end = record.age_at(ClockWindow::END).value();
            assert!(candidate.lifetime.value() <= end, "{id:?} dies by +H");
            let again = SystemStars::generate_with(
                galaxy,
                &record,
                &candidate.composition,
                MultiplicityContext::ForcedSingle,
            );
            assert_eq!(candidate.primary_stars(galaxy, &record), again, "{id:?}");
            kept += 1;
            ejected += u32::from(candidate.ejection.is_some());
            dying += u32::from(RecentDeath::of(galaxy, &candidate).is_some());
        }
    }
    eprintln!("{kept} candidates kept, {ejected} ejected, {dying} with a shell");
    assert!(
        kept > 20 && ejected > 0 && dying > 0,
        "{kept} {ejected} {dying}"
    );
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

/// The new arm's first runaway and walkaway entries of the sample cells, as measured
/// (2026-11-01, with the runaways' terms at the whole budget and the bounds by mass; at generator
/// versions 25 and 26).
const EJECTED_PIN: [[u64; 7]; 3] = [
    [
        0x4021_ee99_24f8_d9e6,
        0x4180_e6a6_65de_8fdc,
        0xbfa0_947d_748e_8bb8,
        0xc10d_0729_0ca1_e34d,
        0xc108_fa4e_6e9f_d254,
        0x40c0_a908_8b72_aba5,
        0x4023_c417_6c59_4de2,
    ],
    [
        0x4031_63ab_85b9_ea3a,
        0x4167_6e2b_bcf8_63a5,
        0x3fb1_0738_5966_72dd,
        0xc110_19eb_c306_81c9,
        0x4127_5fe8_4f43_5b16,
        0xc103_b6ef_92fa_2a4e,
        0x4040_b876_8f48_fd90,
    ],
    [
        0x4022_92de_2667_bfd4,
        0x417e_3187_77c8_ad7b,
        0x3fdb_94d2_f341_5d34,
        0xc0f6_7986_9eed_0abb,
        0x4104_6973_e5eb_a5bd,
        0x40e2_492a_8ad7_734c,
        0x404b_b4d4_ad86_6866,
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
/// threads: their count, the ejected among them, and the sum of their odds of being ejected and
/// of those odds' variances.
///
/// A death at mass m and age a is the field's at the rate `k p(a) (1 − P(m))`, a walkaway's term's
/// at `k p(a) P_w(m)` and a runaway's at `p(a) P_r(m)`, with `p` the budget's age density, `k`
/// what the field keeps at that age ([`field_keeps`]) and `P = P_r + P_w` the closed form's shares
/// by kind; so its odds of being ejected are `(k P_w + P_r) ÷ (k (1 − P_r) + P_r)`, `P(m)` where
/// the field is the budget.
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
                    let Some(candidate) = process.unscreened_candidate(galaxy, &cell, id, bound)
                    else {
                        continue;
                    };
                    let source = table.component_source(candidate.component);
                    let [runaway, walkaway] = KINDS
                        .map(|kind| closed_form_kind_share(table, source, kind, candidate.mass));
                    let k = field_keeps(galaxy, candidate.component, candidate.age_at_epoch);
                    let p = (k * walkaway + runaway) / (k * (1.0 - runaway) + runaway);
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
/// a walkaway's with odds its mass's and age's ([`ejected_census`]), the runaways at the whole
/// budget and the walkaways and the field at the field's part; summed over the whole root cube,
/// the ejected count against the odds' sum and their variance, at α = 10⁻³. Integrated over the
/// cube a class's form is one system's, so this holds whatever the forms. Under every 27 hold,
/// and under every 27 hold but the field share, where the field is the budget and the odds the
/// mass's ejected share ([`ejected_share`]).
#[test]
#[ignore = "slow: the class's candidates over the whole galaxy, twice"]
fn the_ejected_share_is_the_closed_form_galaxy_wide() {
    let budget = Galaxy::from_params_with_rules(
        Seed::new(SEED),
        GalaxyParams::milky_way_like(),
        GalaxyRules {
            field_share: crate::galaxy::ages::FieldShareRules::None,
            ..GalaxyRules::of_version(crate::GENERATOR_VERSION)
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
