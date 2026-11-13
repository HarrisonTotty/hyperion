//! P09.T38's budgets, part two: sampled counts of the field, the features' members and the
//! catalogue's systems against each population's budget, layer by layer, in twelve test volumes
//! and the galactic centre's (P09.T6 holds the closed forms; this samples them).
//!
//! The budget is the grid as it places a population's systems with no field share and no
//! carve-out (`FieldShareRules::None`, `CarveOutRules::Unclaimed`), every other 27 hold kept, the
//! displaced classes among them: the galaxy `B`. The galaxy under every 27 hold, `T`, keeps a
//! subset of `B`'s records bit for bit (P09.T2.c), and the features' members and the catalogue
//! classes' systems (`FeatureMemberSource`, `CatalogueClassSource`, as the server registers them)
//! must stand in for the rest. The global list's source is registered beside them, as the server
//! registers it from 27 (P10.T10.b with P09.T37.b), and its members are counted apart: the
//! centre's, the nuclear cluster's, lie outside every population's budget (plan 02, Design note
//! 15), and the streams' and the dwarf cores' stand in for the halo's discrete share, which both
//! galaxies give up (P10.T3.d.b). In each volume and layer the range query counts, by population,
//! `T`'s field `F`, its members `M` and its catalogue systems `C` against `B`'s `N`, in three
//! parts, each against its own error:
//!
//! - **The field.** Each of `B`'s field records is `T`'s with the share `T`'s field keeps of its
//!   component, band and age (the field share, and in layer D less its ancient Type Ia, P09.T35),
//!   unless the carve-out claims it; a displaced class's record always. So `F` is a sum of
//!   independent Bernoulli draws on `B`'s records, its mean and variance known exactly.
//! - **The carve-out.** The core collapses' and the luminous blue variables' systems in layer E
//!   stand in for the records `T`'s carve-out claimed, but only in part: the grid's displaced
//!   classes weigh the whole budget, the features' share among it, while the core collapses'
//!   terms take the field's part (P09.T33.b's departure, pending the owner's ruling), so a
//!   displaced record of a feature's share that the carve-out claims is stood in for by its
//!   feature's list, among the members. The counts are printed, the claims split by placement,
//!   and not tested; P09.T38's complementarity holds the claims mark set by mark set.
//! - **The features.** The members and plan 11's carved binary classes (P11.T7) stand in for the
//!   share the field share takes, but not for the ancient Type Ia, which left nothing. Members come
//!   by feature, a Poisson process of clusters, whose count's variance is each feature's count
//!   squared, summed: estimated from the volume, and so checked only where the volume holds many
//!   features; a volume without one has no estimate of what it misses.
//!
//! The Type Ia class's systems, which no grid record gives up, are counted beside them.
//!
//! The centre's volume, about the black hole, is sized by the query's own census, the cluster's
//! members with the rest, not by the budget's, since the cluster outnumbers the field there by
//! orders, and down to a twentieth of a light-year. Its cells are walked under every 27 hold, where
//! a remnant class draws its age with each attempt (P09.T37's fix of the centre's band-E draw).

use std::collections::BTreeMap;
use std::num::NonZeroU32;

use hyperion_testkit::stats::{ALPHA, assert_p_value, normal_cdf};

use super::{KeepCells, KeepClassCells, Tubes, milky_way};
use crate::coords::GalacticPosition;
use crate::galaxy::ages::{FieldShareRules, SubDisc};
use crate::galaxy::catalogue_classes::{CatalogueClassSource, ClassId};
use crate::galaxy::displaced::PlacementClass;
use crate::galaxy::displaced::site::CarveOutRules;
use crate::galaxy::features::catalogue::{FeatureCatalogue, NoFeatureCache};
use crate::galaxy::features::centre::KeepCentreOrbits;
use crate::galaxy::features::ids::{FeatureProcess, PackedFeature};
use crate::galaxy::features::members::{FeatureMemberSource, KeepInteriors};
use crate::galaxy::global_list::GlobalListSource;
use crate::galaxy::placement::{
    CandidateOutcome, CellKey, SystemOrigin, SystemRecord, catalogue_claims, evaluate_candidate,
    layer_spec,
};
use crate::galaxy::query::{
    MassFloor, RangePlan, RangeQuery, SubstellarRequest, SystemHit, SystemSource, expected_counts,
    range_query,
};
use crate::galaxy::rules::GalaxyRules;
use crate::galaxy::{Galaxy, POPULATIONS, Population};
use crate::id::{Layer, SystemId, SystemIdKind};
use crate::math;
use crate::planetary::testing::in_shares;
use crate::units::LightYears;

/// The run's shares (`in_shares`; `.config/nextest.toml`).
const SHARES: usize = 16;

/// The stellar layers, with the floor that admits each and the systems its volume aims at: layer
/// E's more, so that its few recent deaths are many enough to weigh.
const LAYERS: [(Layer, MassFloor, f64); 5] = [
    (Layer::E, MassFloor::LayerE, 100_000.0),
    (Layer::D, MassFloor::LayerD, 40_000.0),
    (Layer::C, MassFloor::LayerC, 20_000.0),
    (Layer::B, MassFloor::LayerB, 20_000.0),
    (Layer::A, MassFloor::LayerA, 20_000.0),
];

/// The radii a volume is held to, light-years.
const RADII_LY: (f64, f64) = (5.0, 3_000.0);

/// The radii the centre's volume is held to, light-years: its census reaches a layer's aim well
/// inside 5 ly for the finer layers.
const CENTRE_RADII_LY: (f64, f64) = (0.05, 3_000.0);

/// The steps the centre's volume's radius is refined in: each rescales it by the cube root of the
/// aim over the census, which a cusp's counts, rising as r^1.7, approach in a few.
const CENTRE_RADIUS_STEPS: u32 = 8;

/// The fixed test volumes, by name and centre (light-years, galactic): the Sun's place, the Sun's
/// radius a quarter turn on, the inner and outer discs, the thick disc and the halo above the Sun,
/// the bar's end, below the inner disc and the far outer disc. Four more are found in the galaxy:
/// a point of an arm's ridge at the Sun's radius, and the nursery, the oldest sub-disc's open
/// cluster and the globular nearest the Sun.
const FIXED: [(&str, [f64; 3]); 8] = [
    ("the Sun's place", [0.0, 26_000.0, 0.0]),
    ("the Sun's radius a quarter turn on", [26_000.0, 0.0, 0.0]),
    ("the inner disc", [0.0, 13_000.0, 0.0]),
    ("the outer disc", [0.0, 40_000.0, 0.0]),
    ("the thick disc above the Sun", [0.0, 26_000.0, 3_000.0]),
    ("the halo above the Sun", [0.0, 26_000.0, 20_000.0]),
    ("the bar's end", [8_000.0, 8_000.0, 0.0]),
    ("below the inner disc", [0.0, -16_000.0, -1_500.0]),
];

/// The features a volume must hold, by population and layer, for its members' variance to be
/// estimated from it.
const MANY_FEATURES: u64 = 20;

/// What one population's systems of one layer in one volume come to on each side.
#[derive(Debug, Default, Clone, Copy)]
struct Counts {
    /// `B`'s records: the budget.
    budget: u64,
    /// `T`'s field records, and their mean and variance given `B`'s.
    field: u64,
    field_mean: f64,
    field_variance: f64,
    /// `T`'s members, the sum of each feature's count squared, and the features.
    members: u64,
    member_squares: u64,
    features: u64,
    /// `T`'s catalogue systems: the core collapses' and the variables', plan 11's binary classes',
    /// and the Type Ia's.
    carved_classes: u64,
    binary_classes: u64,
    type_ia: u64,
    /// `B`'s records `T` removed: by the field's thinning, and claimed by the carve-out, a field
    /// system's or a displaced class's.
    thinned: u64,
    claimed: u64,
    claimed_displaced: u64,
    /// The share of `B`'s field records the field share takes, summed: what the members and the
    /// binary classes stand in for.
    replaced_mean: f64,
    /// `T`'s centre's members, the nuclear cluster's, outside every budget.
    centre: u64,
    /// `T`'s streams' and dwarf cores' members, which stand in for the halo's discrete share.
    listed: u64,
}

impl Counts {
    fn add(&mut self, other: &Self) {
        self.budget += other.budget;
        self.field += other.field;
        self.field_mean += other.field_mean;
        self.field_variance += other.field_variance;
        self.members += other.members;
        self.member_squares += other.member_squares;
        self.features += other.features;
        self.carved_classes += other.carved_classes;
        self.binary_classes += other.binary_classes;
        self.type_ia += other.type_ia;
        self.thinned += other.thinned;
        self.claimed += other.claimed;
        self.claimed_displaced += other.claimed_displaced;
        self.replaced_mean += other.replaced_mean;
        self.centre += other.centre;
        self.listed += other.listed;
    }
}

/// One test volume's counts, by layer and population.
type Volume = BTreeMap<(Layer, Population), Counts>;

/// A query of the sphere of `radius` about `centre`, admitting every layer down to `floor`, with
/// no limit on its census and its cells.
fn query(centre: &GalacticPosition, radius: f64, floor: MassFloor) -> RangeQuery {
    RangeQuery::builder(*centre, LightYears::new(radius))
        .limit(NonZeroU32::MAX)
        .cell_budget(NonZeroU32::MAX)
        .mass_floor(floor)
        .build()
        .expect("a sphere in the cube")
}

/// The radius about `centre` at which `budget` expects `aim` systems of `layer`, held to
/// [`RADII_LY`].
fn radius_for(budget: &Galaxy, centre: &GalacticPosition, layer: Layer, aim: f64) -> f64 {
    let probe = 100.0;
    let expected = expected_counts(
        budget,
        centre,
        LightYears::new(probe),
        SubstellarRequest::None,
    )
    .get(layer)
    .max(1e-12);
    (probe * math::cbrt(aim / expected)).clamp(RADII_LY.0, RADII_LY.1)
}

/// What a catalogue class's system stands in for: the carve-out's claims (the core collapses and
/// the luminous blue variables), the field share's binary classes (plan 11's), or nothing (the
/// Type Ia).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StandsIn {
    Claimed,
    Thinned,
    Nothing,
}

impl StandsIn {
    fn of(class: ClassId) -> Self {
        if class == ClassId::CORE_COLLAPSE || class == ClassId::LUMINOUS_BLUE_VARIABLE {
            Self::Claimed
        } else if class == ClassId::TYPE_IA {
            Self::Nothing
        } else {
            Self::Thinned
        }
    }
}

/// The radius about the centre at which `on`'s query with `sources` expects `aim` systems of
/// `layer` by its own census, admitting every layer down to `floor`, held to [`CENTRE_RADII_LY`]
/// (module documentation).
fn centre_radius_for(
    on: &Galaxy,
    sources: &[&dyn SystemSource],
    centre: &GalacticPosition,
    (layer, floor, aim): (Layer, MassFloor, f64),
) -> f64 {
    let mut radius = 100.0;
    for _ in 0..CENTRE_RADIUS_STEPS {
        let plan = RangePlan::new(on, sources, &query(centre, radius, floor));
        let expected = plan.census().expected().get(layer).max(1e-12);
        radius = (radius * math::cbrt(aim / expected)).clamp(CENTRE_RADII_LY.0, CENTRE_RADII_LY.1);
    }
    radius
}

/// How a volume's radius is chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sizing {
    /// By the budget's expected counts ([`radius_for`]).
    Budget,
    /// By the query's own census ([`centre_radius_for`]): the centre's.
    Census,
}

/// One volume's counts about `centre` in `on` (`T`) against `budget` (`B`), its radii chosen by
/// `sizing`, the global list read through `tubes`.
fn volume(
    (on, budget): (&Galaxy, &Galaxy),
    (centre, sizing): (&GalacticPosition, Sizing),
    tubes: &Tubes<'_>,
) -> Volume {
    // Each volume's queries, one a layer, share what they generate: the cells about the centre
    // take minutes cold under every 27 hold (pod, 16 vCPU).
    let interiors = KeepInteriors::new(on);
    let members = FeatureMemberSource::new(&NoFeatureCache, &interiors);
    let cells = KeepClassCells::default();
    let classes = CatalogueClassSource::registered_in(on)
        .expect("every 27 hold registers it")
        .with_cells(&cells);
    assert!(
        GlobalListSource::is_registered_in(on),
        "every 27 hold registers it"
    );
    let orbits = KeepCentreOrbits::new(on);
    let listed = GlobalListSource::new(on, tubes, &orbits).expect("the fixture's centre builds");
    let sources: [&dyn SystemSource; 3] = [&members, &listed, &classes];
    let (mut on_cells, mut budget_cells) = (KeepCells::default(), KeepCells::default());
    let mut counts = Volume::new();
    for (layer, floor, aim) in LAYERS {
        let radius = match sizing {
            Sizing::Budget => radius_for(budget, centre, layer, aim),
            Sizing::Census => centre_radius_for(on, &sources, centre, (layer, floor, aim)),
        };
        let q = query(centre, radius, floor);
        let sampled = range_query(on, &mut on_cells, &sources, &q);
        let reference = range_query(budget, &mut budget_cells, &[], &q);
        eprintln!(
            "{centre:?}, layer {}: {radius:.3} ly, {} systems and the budget's {}",
            layer.letter(),
            sampled.systems().len(),
            reference.systems().len()
        );
        assert_eq!(
            sampled.census().complete_down_to(),
            Some(layer),
            "{layer:?} at {radius} ly: the census admits the layer"
        );
        let mut field = BTreeMap::new();
        let mut per_feature: BTreeMap<(Population, PackedFeature), u64> = BTreeMap::new();
        for hit in sampled.systems() {
            let record = hit.record();
            if record.layer() != layer {
                continue;
            }
            let c = counts.entry((layer, record.population())).or_default();
            match record.origin() {
                SystemOrigin::Grid(_) => {
                    c.field += 1;
                    field.insert(hit.id(), *record);
                }
                SystemOrigin::FeatureMember { feature, .. } => {
                    c.members += 1;
                    *per_feature
                        .entry((record.population(), feature))
                        .or_default() += 1;
                }
                SystemOrigin::CatalogueSystem(class) => match StandsIn::of(class) {
                    StandsIn::Claimed => c.carved_classes += 1,
                    StandsIn::Thinned => c.binary_classes += 1,
                    StandsIn::Nothing => c.type_ia += 1,
                },
                SystemOrigin::CentreMember { .. } => c.centre += 1,
                SystemOrigin::GlobalListMember { .. } => c.listed += 1,
            }
        }
        for ((population, _), n) in per_feature {
            let c = counts.entry((layer, population)).or_default();
            c.member_squares += n * n;
            c.features += 1;
        }
        count_budget(on, layer, reference.systems(), &field, &mut counts);
    }
    counts
}

/// `B`'s records of `layer`, `hits`, counted against `T`'s (`on`'s) field records of the same
/// query, `field`, into `counts`: each record's share kept and replaced, and what became of it in
/// `T` where `T`'s field does not hold it.
fn count_budget(
    on: &Galaxy,
    layer: Layer,
    hits: &[SystemHit],
    field: &BTreeMap<SystemId, SystemRecord>,
    counts: &mut Volume,
) {
    let field_share = on.field_share().expect("every 27 hold installs the field");
    let band = layer_spec(layer).expect("a stellar layer").band();
    let mut matched = 0;
    for hit in hits {
        let record = hit.record();
        if record.layer() != layer {
            continue;
        }
        let c = counts.entry((layer, record.population())).or_default();
        c.budget += 1;
        // The share of the record the field keeps, and what layer D keeps of that past its
        // ancient Type Ia: all of a displaced class's record, which nothing thins.
        let (share, kept) = match (record.placement_class(), record.component()) {
            (PlacementClass::Displaced { .. }, _) | (_, None) => (1.0, 1.0),
            (_, Some(component)) => (
                field_share.keeps(component, band, record.age_at_epoch()),
                if layer == Layer::D {
                    on.layer_d_kept(component).unwrap_or(1.0)
                } else {
                    1.0
                },
            ),
        };
        let keeps = share * kept;
        c.replaced_mean += 1.0 - share;
        if !catalogue_claims(on, record, None) {
            c.field_mean += keeps;
            c.field_variance += keeps * (1.0 - keeps);
        }
        if let Some(held) = field.get(&hit.id()) {
            assert_eq!(
                held, record,
                "the field keeps the budget's record bit for bit"
            );
            matched += 1;
            continue;
        }
        let SystemIdKind::Grid(grid) = hit.id().kind() else {
            panic!("{:?}: the budget is the grid", hit.id());
        };
        let key = CellKey::of(hit.id()).expect("a grid record's cell");
        match evaluate_candidate(on, key, grid.index()) {
            CandidateOutcome::Thinned => c.thinned += 1,
            CandidateOutcome::ClaimedByCatalogue => {
                c.claimed += 1;
                if matches!(record.placement_class(), PlacementClass::Displaced { .. }) {
                    c.claimed_displaced += 1;
                }
            }
            outcome => panic!("{:?}: {outcome:?} outside the field", hit.id()),
        }
    }
    assert_eq!(
        matched,
        field.len(),
        "{layer:?}: every field record is the budget's"
    );
}

/// The thirteen test volumes' centres in `galaxy`, each with its sizing: [`FIXED`]'s, then an
/// arm's ridge at the Sun's radius, and the nursery, the oldest sub-disc's open cluster and the
/// globular nearest the Sun's place, each by the budget, and the galactic centre by its census.
fn volumes(galaxy: &Galaxy) -> Vec<(String, GalacticPosition, Sizing)> {
    let place = |at: [f64; 3]| GalacticPosition::from_light_years(at).expect("in the cube");
    let sun = place(FIXED[0].1);
    let mut out: Vec<(String, GalacticPosition, Sizing)> = FIXED
        .iter()
        .map(|&(name, at)| (name.to_owned(), place(at), Sizing::Budget))
        .collect();
    let (sin, cos) = math::sin_cos(galaxy.fields().arms().ridge_azimuth(26_000.0, 0));
    out.push((
        "an arm's ridge at the Sun's radius".to_owned(),
        place([26_000.0 * cos, 26_000.0 * sin, 0.0]),
        Sizing::Budget,
    ));
    for (name, process) in [
        ("the nursery nearest the Sun", FeatureProcess::Nursery),
        (
            "the open cluster nearest the Sun",
            FeatureProcess::OldOpenCluster(SubDisc::First),
        ),
        ("the globular nearest the Sun", FeatureProcess::Globular),
    ] {
        let nearest = FeatureCatalogue::walk_process(galaxy, process)
            .map(|feature| {
                (
                    feature.position().distance_to(&sun).value(),
                    *feature.position(),
                )
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .expect("the process has features");
        out.push((name.to_owned(), nearest.1, Sizing::Budget));
    }
    out.push((
        "the galactic centre".to_owned(),
        GalacticPosition::ORIGIN,
        Sizing::Census,
    ));
    out
}

/// A difference's z and its two-sided p-value, or none for a variance of zero.
fn z_of(difference: f64, variance: f64) -> Option<(f64, f64)> {
    (variance > 0.0).then(|| {
        let z = difference / variance.sqrt();
        (z, 2.0 * normal_cdf(-z.abs()))
    })
}

/// A count as a real number: exact below 2⁵³, far above any count here.
#[expect(
    clippy::cast_precision_loss,
    reason = "a count of systems in a volume is far below 2⁵³, where f64 holds every integer"
)]
fn real(n: u64) -> f64 {
    n as f64
}

/// A z for the tables, or a dash.
fn shown(z: Option<(f64, f64)>) -> String {
    z.map_or_else(|| "-".to_owned(), |(z, _)| format!("{z:.2}"))
}

/// Each population's counts summed over the volumes, printed.
fn print_totals(totals: &BTreeMap<Population, Counts>) {
    for (population, t) in totals {
        eprintln!(
            "{}: budget {}, field {} (mean {:.1}), members {} of {} features, carved classes {}, \
             binary classes {}, Type Ia {}, thinned {} (the field share's {:.1}), claimed {} ({} \
             displaced), the centre's members {}, the streams' and cores' {}",
            population.name(),
            t.budget,
            t.field,
            t.field_mean,
            t.members,
            t.features,
            t.carved_classes,
            t.binary_classes,
            t.type_ia,
            t.thinned,
            t.replaced_mean,
            t.claimed,
            t.claimed_displaced,
            t.centre,
            t.listed
        );
    }
}

/// P09.T38's budgets, part two, under every 27 hold (module documentation): in each of the
/// thirteen volumes, the centre's among them, each layer and each population, the field against
/// the budget's records it keeps; and where the volume holds many features, the members and the
/// binary classes against the share the field share takes (α = 10⁻³ over every test); the
/// carve-out's claims and the classes' systems that stand in for them, and the global list's
/// members, printed.
#[test]
#[ignore = "slow: thirteen volumes' range queries under every 27 hold and the budget (one on wasm)"]
fn the_field_members_and_catalogue_hold_each_population_s_budget() {
    let on = milky_way(GalaxyRules::of_version(crate::GENERATOR_VERSION));
    let budget = milky_way(GalaxyRules {
        field_share: FieldShareRules::None,
        carve_out: CarveOutRules::Unclaimed,
        ..GalaxyRules::of_version(crate::GENERATOR_VERSION)
    });
    let volumes = volumes(&on);
    assert_eq!(volumes.len(), 13);
    let tubes = Tubes::new(&on);
    // wasm32-wasip1 has no threads: the Sun's place alone there, whose census takes minutes on one.
    let volumes = if cfg!(target_family = "wasm") {
        &volumes[..1]
    } else {
        &volumes[..]
    };
    let counted = in_shares(volumes.len(), SHARES, |k| {
        volume((&on, &budget), (&volumes[k].1, volumes[k].2), &tubes)
    });
    let tests = 2 * counted.iter().map(BTreeMap::len).sum::<usize>();
    let alpha = ALPHA / f64::from(u32::try_from(tests).unwrap());
    let n = real;
    let mut totals: BTreeMap<Population, Counts> = BTreeMap::new();
    for ((name, _, _), counts) in volumes.iter().zip(&counted) {
        eprintln!("{name}:");
        eprintln!(
            "| layer | population | budget | field | its mean | members | features | carved \
             classes | binary classes | Type Ia | thinned | claimed | centre | listed | field z | \
             features z |"
        );
        let (mut claimed, mut displaced, mut carved) = (0, 0, 0);
        for (&(layer, population), c) in counts {
            let label = format!("{name}, layer {}, {}", layer.letter(), population.name());
            let field = z_of(n(c.field) - c.field_mean, c.field_variance);
            let features = z_of(
                n(c.members + c.binary_classes) - c.replaced_mean,
                n(c.member_squares + c.binary_classes) + c.replaced_mean,
            );
            eprintln!(
                "| {} | {} | {} | {} | {:.1} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                layer.letter(),
                population.name(),
                c.budget,
                c.field,
                c.field_mean,
                c.members,
                c.features,
                c.carved_classes,
                c.binary_classes,
                c.type_ia,
                c.thinned,
                c.claimed,
                c.centre,
                c.listed,
                shown(field),
                shown(features)
            );
            if let Some((z, p)) = field {
                assert_p_value(&format!("{label}: the field, z = {z:.2}"), p, alpha);
            }
            if let Some((z, p)) = features
                && c.features >= MANY_FEATURES
            {
                assert_p_value(&format!("{label}: the features, z = {z:.2}"), p, alpha);
            }
            totals.entry(population).or_default().add(c);
            if layer == Layer::E {
                claimed += c.claimed;
                displaced += c.claimed_displaced;
                carved += c.carved_classes;
            }
        }
        eprintln!(
            "{name}: claimed {claimed}, {displaced} of them displaced records; the core \
             collapses' and the variables' systems {carved}"
        );
    }
    print_totals(&totals);
    for population in POPULATIONS {
        assert!(
            cfg!(target_family = "wasm") || totals.contains_key(&population),
            "{population:?} is in some volume"
        );
    }
}
