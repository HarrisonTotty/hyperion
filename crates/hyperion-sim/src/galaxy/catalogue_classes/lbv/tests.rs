//! The luminous blue variables' class's tests (P09.T43): the young field it follows and its
//! bounds, the windows it reads and the draws that move none, its candidates against the field's
//! own systems, its entries against the class test, the grid's claim against a brute-force read of
//! every star's window, the core collapses' disjointness, resolution and order independence, its
//! headroom, and a golden with the galaxy's hosts. The million-star sample of the plan, with the
//! galaxy-wide agreement of the class and the grid, is slow.

use std::sync::OnceLock;
#[cfg(not(target_family = "wasm"))]
use std::thread;

use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::lcg::Lcg;
use hyperion_testkit::order::assert_order_independent;
use hyperion_testkit::stats::{
    ALPHA, assert_p_value, assert_poisson_count, ks_two_sample, normal_cdf,
};

use super::*;
use crate::GENERATOR_VERSION;
use crate::Seed;
use crate::galaxy::catalogue_classes::core_collapse::{CoreCollapseProcess, recent_death_claims};
use crate::galaxy::catalogue_classes::grid::candidate_count;
use crate::galaxy::catalogue_classes::supernova::LIFETIME_REACH;
use crate::galaxy::catalogue_classes::{
    CatalogueClassCell, check_class_headroom, resolve_entry, system_of,
};
use crate::galaxy::displaced::explosion_site;
use crate::galaxy::features::members::NoInteriorCache;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{self, CellKey, ResolveSystemError};
use crate::galaxy::query::{epoch_velocity, position_at};
use crate::id::BodyId;
use crate::observe::stars_of;
use crate::stellar::draws::{StandardNormal, StarDraws, StarDrawsParts};
use crate::stellar::sse::{PhasePredicate, Track};
use crate::stellar::system::ClockDeath;
use crate::time::{ClockWindow, Span};
use crate::units::consts::METRES_PER_LIGHT_YEAR;

/// The seed of the Milky Way fixture most tests share.
const SEED: u64 = 0x0943_0000_6c62_7621;

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
    CatalogueCellKey::new(ClassId::LUMINOUS_BLUE_VARIABLE, cell).expect("a cell of the root cube")
}

/// Cells with entries: a run along +y through the disc, on both sides of the plane, and two of the
/// eight cells about the centre, where the nuclear disc's are. Each of those draws some 1.25 × 10⁶
/// candidates on the nuclear disc's peak density, about a second, so the sample holds two.
fn sample_cells() -> Vec<CatalogueCellKey> {
    (1..8)
        .flat_map(|y| [key([0, y, 0]), key([0, y, -1])])
        .chain([key([0, 0, 0]), key([-1, -1, -1])])
        .collect()
}

/// The eight cells about the centre, where the nuclear disc's candidates are: from 27 the young
/// field keeps some tenth of the young disc's stars of a few million years (P09.T2.c), so the
/// nuclear disc's, whose field is nearly its budget, are most of the class's candidates in a few
/// cells. Each costs some 0.4 s.
fn centre_cells() -> [CatalogueCellKey; 8] {
    std::array::from_fn(|o| {
        let o = i32::try_from(o).unwrap();
        key([0, 1, 2].map(|axis| -((o >> axis) & 1)))
    })
}

/// The cells of the disc within six of the centre's on both sides of the plane, but the eight about
/// the centre ([`centre_cells`]): 330 cells, some 0.2 s, of which the young field's few entries
/// are some 15.
fn disc_cells() -> Vec<CatalogueCellKey> {
    (-6..=6)
        .flat_map(|x| (-6..=6).flat_map(move |y| [[x, y, 0], [x, y, -1]]))
        .filter(|[x, y, _]| !((-1..=0).contains(x) && (-1..=0).contains(y)))
        .map(key)
        .collect()
}

/// The sample cells, generated once.
fn sample_entries() -> &'static [CatalogueClassCell<LbvEntry>] {
    static CELLS: OnceLock<Vec<CatalogueClassCell<LbvEntry>>> = OnceLock::new();
    CELLS.get_or_init(|| {
        let galaxy = galaxy();
        let process = LbvProcess::new(galaxy);
        sample_cells()
            .into_iter()
            .map(|key| CatalogueClassCell::generate(galaxy, &process, key))
            .collect()
    })
}

/// The kept candidates of `cell`, in index order.
fn kept_candidates(
    galaxy: &Galaxy,
    process: &LbvProcess,
    cell: &CatalogueCellKey,
) -> Vec<LbvCandidate> {
    let bound = process.bound(galaxy, cell);
    (0..candidate_count(galaxy.seed(), cell, bound))
        .filter_map(|index| {
            let id = cell.candidate_id(index, 0).unwrap();
            process.candidate(galaxy, cell, id, bound)
        })
        .collect()
}

/// The earliest and the latest age at the epoch any node's span holds, Julian years.
fn sampler_ages(process: &LbvProcess) -> (f64, f64) {
    process
        .nodes()
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), node| {
            (lo.min(node.ages().0.value()), hi.max(node.ages().1.value()))
        })
}

/// A grid-shaped record of layer E: the candidate `index` of the layer-E cell holding `position`,
/// of `component`'s origin and population, with `mass` and `age`.
fn grid_record(
    galaxy: &Galaxy,
    component: ComponentId,
    position: GalacticPosition,
    index: u32,
    mass: f64,
    age: f64,
) -> SystemRecord {
    let cell = CellKey::containing(Layer::E, &position).expect("a position of the root cube");
    SystemRecord::from_parts(
        cell.candidate_id(index).expect("an index of the layer"),
        position,
        SystemOrigin::Grid(component),
        galaxy.fields().component(component).population(),
        SolarMasses::new(mass),
        Years::new(age),
    )
}

/// Whether the core-collapse class claims the grid record `record`, as the carve-out asks it
/// (P09.T35): its explosion site, then the production claim, `recent_death_claims`.
fn grid_core_collapse_claims(galaxy: &Galaxy, record: &SystemRecord) -> bool {
    explosion_site(galaxy, record).is_some_and(|site| recent_death_claims(galaxy, &site, record))
}

// 1. The young field, its bounds and the windows.

/// The young field is the young thin disc and the nuclear disc's two components, every other
/// component holding no star of the sampler's ages; each one's rate bounds its age density over
/// those ages; and the nodes are the sampler's, weighted by the galaxy's mass function.
#[test]
fn the_young_field_is_the_young_disc_and_the_nuclear_disc() {
    let galaxy = galaxy();
    let process = LbvProcess::new(galaxy);
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
    let (earliest, latest) = sampler_ages(&process);
    for id in fields.component_ids() {
        if process.components().iter().all(|young| young.id() != id) {
            let ages = fields.component(id).ages();
            assert!(ages.min().value() > latest, "{id:?} is young");
        }
    }
    let mass_function = galaxy.mass_function();
    let band = mass_function.integral(MassBand::E.lo(), MassBand::E.hi());
    let mut weight = 0.0;
    for (k, node) in process.nodes().iter().enumerate() {
        let (lo, hi) = node.masses();
        assert_eq!((lo, hi), LBV_SAMPLER.masses(k));
        let (first, last) = host_ages(k);
        assert_same_bits(node.ages().0.value(), first.value());
        assert_same_bits(node.ages().1.value(), last.value());
        assert_same_bits(
            node.share(),
            mass_function.integral(lo.value(), hi.value()) / band,
        );
        assert_same_bits(
            node.weight().value(),
            node.share() * (last.value() - first.value()),
        );
        weight += node.weight().value();
    }
    // Some 0.23 Myr of ages per layer-E system, the nodes' placement's figure.
    assert!((2.0e5..2.6e5).contains(&weight), "{weight}");
    // The field's share and ages, which from 27 give most young stars to the features (P09.T2.c).
    for young in process.components() {
        let ages = galaxy.field_ages(young.id());
        assert_same_bits(
            young.share(),
            galaxy.field_component_share(MassBand::E, young.id()),
        );
        assert_same_bits(
            young.candidates_per_system(),
            young.rate_per_year() * weight,
        );
        for k in 0..=20_000 {
            let age = earliest + (latest - earliest) * f64::from(k) / 20_000.0;
            assert!(ages.pdf(Years::new(age)) <= young.rate_per_year(), "{age}");
        }
    }
    // The young disc's budget's age density is about one in 100 Myr, and from 27 its field's about
    // one in 390 Myr, the features taking most of its youngest stars (P09.T2.c); the nuclear
    // disc's a twentieth of its stars over its last gigayear, some fifty times less.
    let [young, nuclear, inner] = process.components() else {
        panic!("three components");
    };
    assert!(
        (2.3e-9..2.9e-9).contains(&young.rate_per_year()),
        "{young:?}"
    );
    assert_same_bits(nuclear.rate_per_year(), inner.rate_per_year());
    let ratio = young.rate_per_year() / nuclear.rate_per_year();
    assert!((40.0..65.0).contains(&ratio), "{ratio}");
}

/// No draw of a star's moves its window, which the class's thinning on attempt 0 rests on (the
/// module documentation's lean): Reimers η at ±1.5σ and ±3σ, and every draw of a real star at its
/// first four attempts, give the median's window bit for bit, from 36.5 to 150 M☉ and from
/// \[Fe/H\] −2.4, where the Nieuwenhuijzen–de Jager wind falls toward Reimers's, to +0.2. From
/// generator version 26, whose winds enforce the Humphreys–Davidson limit, 38 M☉ at −1 and 42 M☉
/// at −0.5 have no window; 42 and 45 M☉ stand in for them.
#[test]
fn no_draw_moves_a_window() {
    let star = BodyId::new(
        SystemId::from(key([0, 6, 0]).candidate_id(0, 0).unwrap()),
        0,
    );
    let seed = galaxy().seed();
    for (mass, fe_h) in [
        (36.5, -2.4),
        (38.0, -2.0),
        (40.0, -1.5),
        (60.0, -2.0),
        (42.0, -1.0),
        (45.0, -0.5),
        (60.0, -1.0),
        (60.0, 0.0),
        (90.0, -0.25),
        (150.0, 0.2),
    ] {
        let composition = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
        let m = SolarMasses::new(mass);
        let window = |draws: &StarDraws| {
            Track::full(m, &composition, draws).window_where(PhasePredicate::Lbv)
        };
        let median = window(&StarDraws::median());
        assert!(median.is_some(), "{mass} M☉ at {fe_h}");
        for z in [-3.0, -1.5, 1.5, 3.0] {
            let eta = StandardNormal::new(z).unwrap();
            let draws = StarDraws::from_parts(StarDrawsParts {
                eta,
                ..StarDrawsParts::MEDIAN
            });
            assert_eq!(window(&draws), median, "{mass} M☉ at {fe_h}, η at {z}σ");
        }
        for attempt in 0..4 {
            let draws = StarDraws::for_attempt(seed, star, attempt);
            assert_eq!(
                window(&draws),
                median,
                "{mass} M☉ at {fe_h}, attempt {attempt}"
            );
        }
    }
}

/// The thinning's envelope holds: at random points of random cells, from the centre out through
/// the disc and above it, each component's term and their running total lie under the cell's
/// bound.
#[test]
fn the_envelope_holds_over_random_points() {
    let galaxy = galaxy();
    let process = LbvProcess::new(galaxy);
    let mut lcg = Lcg::new(0x0943_0001);
    let mut weights = [0.0; MAX_COMPONENTS];
    let mut tested = 0;
    for _ in 0..3_000 {
        let span = |lcg: &mut Lcg, n: u64| {
            i32::try_from(lcg.next_below(2 * n)).unwrap() - i32::try_from(n).unwrap()
        };
        let cell = key([span(&mut lcg, 16), span(&mut lcg, 16), span(&mut lcg, 2)]);
        let bound = process.bound(galaxy, &cell).value();
        for _ in 0..8 {
            let p = cell.position_from_words([lcg.next_u64(), lcg.next_u64(), lcg.next_u64()]);
            let n = process.weights(galaxy, &PointLy::from(&p), &mut weights);
            let total = weights[..n].iter().fold(0.0, |sum, w| sum + w);
            assert!(total <= bound, "{total} above {bound} in {cell:?}");
            tested += 1;
        }
    }
    // The cells' corners at the centre, where the nuclear disc peaks.
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
    let process = LbvProcess::new(galaxy);
    let fields = galaxy.fields();
    let half = 65_536;
    let mut largest = 0.0_f64;
    for octant in 0..8 {
        let min = [0, 1, 2].map(|axis| if octant >> axis & 1 == 1 { -half } else { 0 });
        let cube = CellBox::new(min, 65_536).expect("an octant of the root cube");
        let bound = process.components().iter().fold(0.0, |sum, young| {
            sum + young.share()
                * fields.component_bound(young.id(), &cube)
                * young.candidates_per_system()
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
        check_class_headroom(
            ClassId::LUMINOUS_BLUE_VARIABLE,
            PerCubicLightYear::new(largest)
        ),
        Ok(())
    );
}

// 2. The candidates, the entries and the grid's claim.

/// A candidate's \[Fe/H\] is its component's law at its site and age, one normal on
/// `class.lbv.metallicity`, to the bit; its mass lies in its node's masses and its age in the
/// node's span; and its record passes the prefilter. A cell of the young disc and the eight about
/// the centre ([`centre_cells`]).
#[test]
fn a_candidate_s_marks_are_its_node_s_and_its_component_s() {
    let galaxy = galaxy();
    let process = LbvProcess::new(galaxy);
    let fields = galaxy.fields();
    let mut checked = 0;
    for cell in core::iter::once(key([0, 5, 0])).chain(centre_cells()) {
        for candidate in kept_candidates(galaxy, &process, &cell) {
            let laws = fields.component(candidate.component);
            let feh = laws.metallicity(&PointLy::from(&candidate.site), candidate.age_at_epoch);
            let mut stream = Stream::open(
                galaxy.seed(),
                tags::CLASS_LBV_METALLICITY,
                ObjectKey::from(SystemId::from(candidate.id)),
            );
            let expected = Composition::from_fe_h(
                Dex::new(stream.normal(feh.mean().value(), feh.sigma().value())),
                HeliumExcess::ZERO,
            );
            assert_eq!(candidate.composition, expected);
            let node = LBV_SAMPLER.node_of(candidate.mass).expect("a node's mass");
            let (lo, hi) = process.nodes()[node].ages();
            assert!(lo <= candidate.age_at_epoch && candidate.age_at_epoch <= hi);
            assert!(may_host(candidate.mass, candidate.age_at_epoch));
            assert_eq!(candidate.population, laws.population());
            checked += 1;
        }
    }
    assert!(checked > 50, "{checked}");
}

/// Every entry is the class test's: of layer E and a catalogue system of the class, passing the
/// prefilter, its window its stars' with the model's own multiplicity and a single star's alike,
/// overlapping the source horizon, unclaimed by the core collapses, and moving straight through
/// its site at the epoch. The sample cells' entries and the rest of the disc's ([`disc_cells`]):
/// from 27 the young field keeps some tenth of the young disc's stars of a few million years
/// (P09.T2.c), and the sample's cells hold four entries.
#[test]
fn every_entry_is_the_class_test_s() {
    let galaxy = galaxy();
    let process = LbvProcess::new(galaxy);
    let sample = sample_cells();
    let others: Vec<CatalogueClassCell<LbvEntry>> = disc_cells()
        .into_iter()
        .filter(|cell| !sample.contains(cell))
        .map(|cell| CatalogueClassCell::generate(galaxy, &process, cell))
        .collect();
    let mut entries = 0;
    for cell in sample_entries().iter().chain(&others) {
        for entry in cell.entries() {
            let value = entry.value();
            let record = value.record();
            assert_eq!(
                record.origin(),
                SystemOrigin::CatalogueSystem(ClassId::LUMINOUS_BLUE_VARIABLE)
            );
            assert_eq!(record.layer(), Layer::E);
            assert_eq!(record.id(), SystemId::from(entry.id()));
            assert!(may_host(
                record.primary_initial_mass(),
                record.age_at_epoch()
            ));
            let single = SystemStars::generate_with(
                galaxy,
                record,
                value.composition(),
                MultiplicityContext::ForcedSingle,
            );
            assert_eq!(horizon_window(&single), Some(value.window()));
            assert_eq!(horizon_window(&value.stars(galaxy)), Some(value.window()));
            let (start, end) = value.window();
            assert!(start <= SourceHorizon::END && end >= SourceHorizon::START);
            let velocity = value.motion().velocity_at(UniverseTime::EPOCH);
            assert!(!core_collapse_claims(
                galaxy,
                record,
                value.composition(),
                &single,
                velocity
            ));
            assert_eq!(
                value.motion().position_at(UniverseTime::EPOCH),
                *record.epoch_position()
            );
            assert_eq!(value.motion().velocity_at(SourceHorizon::START), velocity);
            entries += 1;
        }
    }
    assert!(entries > 10, "{entries}");
}

/// The grid's claim holds every star whose window overlaps the source horizon and nothing else,
/// over grid-shaped records of the young disc chosen to be near it: masses of 30–150 M☉ and ages of
/// 1.5–7 Myr. The truth is a brute-force read of each star's window, its stars with the model's own
/// multiplicity, which no prefilter shortens; every star it finds passes the prefilter. The nuclear
/// disc's records about the centre are checked the same way. The million-star sample of the plan,
/// at the young disc's own masses and ages, is slow.
#[test]
fn the_grid_s_claim_holds_every_host_and_nothing_else() {
    let galaxy = galaxy();
    let process = LbvProcess::new(galaxy);
    let (young_disc, nuclear_disc) = (process.components()[0].id(), process.components()[1].id());
    let mut lcg = Lcg::new(0x0943_0002);
    let mut hosts = [0, 0];
    // The young disc's records about the solar circle, then the nuclear disc's about the centre,
    // where its law is metal-rich.
    for (slot, component, centre, half, n) in [
        (
            0,
            young_disc,
            [0.0, 26_000.0, 0.0],
            [2_000.0, 6_000.0, 200.0],
            600,
        ),
        (1, nuclear_disc, [0.0, 0.0, 0.0], [300.0, 300.0, 50.0], 300),
    ] {
        for index in 0..n {
            let position = GalacticPosition::from_light_years(
                [0, 1, 2].map(|axis| centre[axis] + half[axis] * (2.0 * lcg.next_f64() - 1.0)),
            )
            .unwrap();
            let mass = 30.0 + 120.0 * lcg.next_f64();
            let age = 1.5e6 + 5.5e6 * lcg.next_f64();
            let record = grid_record(galaxy, component, position, index, mass, age);
            let truth = horizon_window(&SystemStars::generate(galaxy, &record)).is_some();
            assert_eq!(lbv_claims(galaxy, &record), truth, "{record:?}");
            if truth {
                assert!(may_host(
                    record.primary_initial_mass(),
                    record.age_at_epoch()
                ));
                hosts[slot] += 1;
            }
        }
    }
    eprintln!("hosts among the young disc's and the nuclear disc's records: {hosts:?}");
    assert!(hosts[0] > 30 && hosts[1] > 5, "{hosts:?}");
    // Another layer's record is never claimed, whatever its mass and age.
    let record = SystemRecord::from_parts(
        CellKey::containing(
            Layer::D,
            &GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap(),
        )
        .unwrap()
        .candidate_id(0)
        .unwrap(),
        GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).unwrap(),
        SystemOrigin::Grid(young_disc),
        Population::YoungThinDisc,
        SolarMasses::new(5.0),
        Years::new(4.0e6),
    );
    assert!(!lbv_claims(galaxy, &record));
}

/// The class and the grid agree on sampled candidates (P09.T38's complementarity, its field half):
/// a grid record of the candidate's component, mass and age at its site, with the candidate's
/// composition, has the candidate's window, so the grid's claim on it and the class's window test
/// give the same answer; and every kept candidate the test rejects for its window, the grid's
/// claim rejects too. Four cells of the young disc and the eight about the centre
/// ([`centre_cells`]).
#[test]
fn the_class_and_the_grid_agree_on_sampled_candidates() {
    let galaxy = galaxy();
    let process = LbvProcess::new(galaxy);
    let (mut agreed, mut hosts) = (0, 0);
    for (index, candidate) in [[0, 3, 0], [0, 4, -1], [0, 5, 0], [0, 6, -1]]
        .map(key)
        .into_iter()
        .chain(centre_cells())
        .flat_map(|cell| kept_candidates(galaxy, &process, &cell))
        .enumerate()
    {
        let record = grid_record(
            galaxy,
            candidate.component,
            candidate.site,
            u32::try_from(index).unwrap(),
            candidate.mass.value(),
            candidate.age_at_epoch.value(),
        );
        let grid = horizon_window(&SystemStars::generate_with(
            galaxy,
            &record,
            &candidate.composition,
            MultiplicityContext::ForcedSingle,
        ));
        let class = horizon_window(&SystemStars::generate_with(
            galaxy,
            &candidate.record(),
            &candidate.composition,
            MultiplicityContext::ForcedSingle,
        ));
        assert_eq!(grid, class, "{:?}", candidate.id);
        match Verdict::of(galaxy, &candidate) {
            Verdict::NoWindow => assert_eq!(grid, None),
            Verdict::CoreCollapse | Verdict::Host(_) => {
                assert!(grid.is_some());
                hosts += 1;
            }
            Verdict::Prefiltered => panic!("a candidate passes the prefilter: {:?}", candidate.id),
        }
        agreed += 1;
    }
    eprintln!("{agreed} candidates agree, {hosts} with a window");
    // 113 and 6 at generator version 26 over the first four cells and the centre's, whose windows
    // the enforced Humphreys–Davidson limit makes fewer and shorter; 22 and 1 there from 27, whose
    // young field keeps some tenth of the young disc's stars of these ages (P09.T2.c).
    assert!(agreed > 100 && hosts > 4, "{agreed} {hosts}");
}

/// No host is both a core-collapse entry and a luminous blue variable: the class's test on the
/// marks of each of the core-collapse class's entries about the Sun-like point and the centre,
/// moved back along the progenitor's line to the epoch, finds the core collapses claiming it,
/// so that any of them whose window overlaps the horizon is not the class's; and no entry of the
/// class is one the core collapses claim (the entries' test above).
#[test]
fn no_host_is_both_a_core_collapse_and_a_luminous_blue_variable() {
    let galaxy = galaxy();
    let process = CoreCollapseProcess::new(galaxy);
    let (mut claimed, mut overlapping) = (0, 0);
    let cells = (48..52)
        .map(|y| [0, y, 0])
        .chain([[0, 0, 0], [-1, -1, -1]])
        .map(|cell| CatalogueCellKey::new(ClassId::CORE_COLLAPSE, cell).unwrap());
    for cell in cells {
        for entry in CatalogueClassCell::generate(galaxy, &process, cell).entries() {
            let value = entry.value();
            let record = value.record();
            let death = value.site().death();
            let before = value
                .motion()
                .velocity_at(death.checked_sub(Span::from_seconds(1)).unwrap());
            // The progenitor's line at the epoch, the position the grid's record would have.
            let elapsed = Seconds::new(
                UniverseTime::EPOCH
                    .checked_since(death)
                    .unwrap()
                    .as_seconds_f64(),
            );
            let at_epoch = value
                .site()
                .position()
                .translated(before.displacement_over(elapsed))
                .unwrap();
            let on_line = SystemRecord::from_parts(
                record.id(),
                at_epoch,
                record.origin(),
                record.population(),
                record.primary_initial_mass(),
                record.age_at_epoch(),
            );
            let stars = SystemStars::generate_with(
                galaxy,
                &on_line,
                value.composition(),
                MultiplicityContext::ForcedSingle,
            );
            assert!(
                core_collapse_claims(galaxy, &on_line, value.composition(), &stars, before),
                "{:?}",
                entry.id()
            );
            claimed += 1;
            if horizon_window(&stars).is_some() {
                overlapping += 1;
            }
        }
    }
    eprintln!(
        "{claimed} core collapses claimed, {overlapping} of them with a window in the horizon"
    );
    assert!(claimed > 20, "{claimed}");
}

/// The class's core-collapse claim is the grid's: on grid records of the young disc about the
/// solar circle whose primaries may die in the supernova test's interval (8–40 M☉, 3–45 Myr),
/// `core_collapse_claims` at the record's own metallicity draw and epoch velocity answers as plan
/// 08's `explosion_site` and the claim P09.T35 asks of it, so the class leaves to the core
/// collapses what the grid's carve-out gives them.
#[test]
fn the_class_s_core_collapse_claim_is_the_grid_s() {
    let galaxy = galaxy();
    let young_disc = LbvProcess::new(galaxy).components()[0].id();
    let mut lcg = Lcg::new(0x0943_0006);
    let (mut dying, mut claimed) = (0, 0);
    for index in 0..400 {
        let position = GalacticPosition::from_light_years([
            4_000.0 * lcg.next_f64() - 2_000.0,
            20_000.0 + 12_000.0 * lcg.next_f64(),
            400.0 * lcg.next_f64() - 200.0,
        ])
        .unwrap();
        let mass = 8.0 + 32.0 * lcg.next_f64();
        let age = 3.0e6 + 4.2e7 * lcg.next_f64();
        let record = grid_record(galaxy, young_disc, position, index, mass, age);
        let composition = draw_metallicity(galaxy, &record);
        let stars = SystemStars::generate_with(
            galaxy,
            &record,
            &composition,
            MultiplicityContext::ForcedSingle,
        );
        let velocity = epoch_velocity(galaxy, &record);
        let class = core_collapse_claims(galaxy, &record, &composition, &stars, velocity);
        assert_eq!(
            class,
            grid_core_collapse_claims(galaxy, &record),
            "{record:?}"
        );
        dying += u32::from(explosion_site(galaxy, &record).is_some());
        claimed += u32::from(class);
    }
    eprintln!("{dying} of 400 records die in the interval, {claimed} of them claimed");
    assert!(claimed > 5, "{dying} {claimed}");
}

// 3. Resolution, the rest of the sim, order.

/// Every entry resolves to itself through the process, through `system_of` and through plan 03's
/// resolve; a second member and an index past the cell's count name nothing.
#[test]
fn entries_resolve_to_themselves() {
    let galaxy = galaxy();
    let process = LbvProcess::new(galaxy);
    for cell in sample_entries() {
        let key = cell.key();
        for entry in cell.entries() {
            assert_eq!(
                resolve_entry(galaxy, &process, entry.id()).unwrap().entry(),
                entry
            );
            let system = system_of(galaxy, entry.id()).unwrap();
            assert_eq!(system.record(), entry.value().record());
            assert_eq!(
                placement::resolve(galaxy, SystemId::from(entry.id())),
                Ok(*entry.value().record())
            );
            let second = key.candidate_id(entry.id().index(), 1).unwrap();
            assert_eq!(
                placement::resolve(galaxy, SystemId::from(second)),
                Err(ResolveSystemError::NoSuchSystem)
            );
        }
        let past = key.candidate_id(cell.candidates(), 0).unwrap();
        assert_eq!(
            placement::resolve(galaxy, SystemId::from(past)),
            Err(ResolveSystemError::NoSuchSystem)
        );
    }
}

/// The rest of the sim reads an entry through its class: its stars (`stars_of`), whose window is
/// the entry's; plan 03's position and epoch velocity on its line at the horizon's ends and the
/// epoch; and no entry travels past the class lookup's reach from its site over the horizon.
#[test]
fn the_rest_of_the_sim_reads_an_entry_through_its_class() {
    let galaxy = galaxy();
    let mut farthest = 0.0_f64;
    for cell in sample_entries() {
        for entry in cell.entries() {
            let value = entry.value();
            let record = value.record();
            let stars = stars_of(galaxy, &NoInteriorCache, record).unwrap();
            assert_eq!(horizon_window(&stars), Some(value.window()));
            assert_eq!(
                epoch_velocity(galaxy, record),
                value.motion().velocity_at(UniverseTime::EPOCH)
            );
            for t in [
                SourceHorizon::START,
                ClockWindow::START,
                UniverseTime::EPOCH,
                ClockWindow::END,
            ] {
                let p = position_at(galaxy, record, t);
                assert_eq!(p, value.motion().position_at(t));
                let ly = record.epoch_position().distance_to(&p).value() / METRES_PER_LIGHT_YEAR;
                assert!(ly <= LIFETIME_REACH.value(), "{:?}: {ly} ly", entry.id());
                farthest = farthest.max(ly);
            }
        }
    }
    eprintln!("the farthest an entry travels over the horizon: {farthest:.0} ly");
}

/// Cells come out the same in any order, after a stray resolve of another class's ID, twice from
/// the same seed, and differently from another.
#[test]
fn cells_do_not_depend_on_order() {
    let galaxy = galaxy();
    let process = LbvProcess::new(galaxy);
    let cells: Vec<CatalogueCellKey> = (4..8).map(|y| key([0, y, 0])).collect();
    // A stray resolve of another class's entry first, which runs that class's own process to its
    // test: the first X-ray binary of the cells about the centre.
    let xrb = crate::galaxy::catalogue_classes::binary::XrayBinaries::new(galaxy);
    let stray = (-4..4)
        .flat_map(|x| (-4..4).map(move |y| [x, y, 0]))
        .find_map(|cell| {
            let key = CatalogueCellKey::new(ClassId::XRAY_BINARY, cell).unwrap();
            CatalogueClassCell::generate(galaxy, &xrb, key)
                .into_entries()
                .into_iter()
                .next()
        })
        .expect("the centre has X-ray binaries");
    let stray = SystemId::from(stray.id());
    let record = placement::resolve(galaxy, stray).unwrap();
    assert_order_independent(&cells, |key| {
        assert_eq!(placement::resolve(galaxy, stray), Ok(record));
        CatalogueClassCell::generate(galaxy, &process, *key)
    });
    let fresh = |seed: u64| {
        let galaxy = Galaxy::new(Seed::new(seed));
        let process = LbvProcess::new(&galaxy);
        cells
            .iter()
            .map(|key| CatalogueClassCell::generate(&galaxy, &process, *key))
            .collect::<Vec<_>>()
    };
    let once = fresh(0x0943_0003);
    assert_eq!(once, fresh(0x0943_0003));
    let counts = |cells: &[CatalogueClassCell<LbvEntry>]| -> Vec<u32> {
        cells.iter().map(CatalogueClassCell::candidates).collect()
    };
    assert_ne!(counts(&once), counts(&fresh(0x0943_0004)));
}

// 4. The census and the golden.

/// What a galaxy's class holds, counted over every cell.
#[derive(Debug, Clone, Default, PartialEq)]
struct Census {
    /// Candidates the thinning kept, by component.
    candidates: Vec<u64>,
    /// Of those, with a window overlapping the source horizon.
    windows: u64,
    /// Of those, claimed by the core collapses.
    core_collapses: u64,
    /// Entries, by component and by node.
    entries: Vec<u64>,
    by_node: [u64; NODES],
    /// Entries whose window holds the epoch: luminous blue variables then.
    at_epoch: u64,
    /// Entries whose primary is dead by +H.
    dead: u64,
    /// The entries' initial masses, M☉, for the young disc's entries.
    young_masses: Vec<f64>,
}

impl Census {
    fn new(components: usize) -> Self {
        Self {
            candidates: vec![0; components],
            entries: vec![0; components],
            ..Self::default()
        }
    }

    fn add(&mut self, other: &Self) {
        for (a, b) in self.candidates.iter_mut().zip(&other.candidates) {
            *a += b;
        }
        for (a, b) in self.entries.iter_mut().zip(&other.entries) {
            *a += b;
        }
        for (a, b) in self.by_node.iter_mut().zip(&other.by_node) {
            *a += b;
        }
        self.windows += other.windows;
        self.core_collapses += other.core_collapses;
        self.at_epoch += other.at_epoch;
        self.dead += other.dead;
        self.young_masses.extend_from_slice(&other.young_masses);
    }

    fn hosts(&self) -> u64 {
        self.entries.iter().sum()
    }
}

/// A mean below which a cell's candidates are not drawn: 10⁻⁹, so that every cell skipped holds,
/// together, under 0.00004 candidates of the root cube's 2¹⁵ cells.
const NEGLIGIBLE_MEAN: f64 = 1e-9;

/// The census of `galaxy`'s class: every cell of the root cube whose mean is not negligible, a
/// column of cells at a time from the plane outwards (the bound never rises away from the plane),
/// on four threads natively.
fn census(galaxy: &Galaxy) -> Census {
    let process = LbvProcess::new(galaxy);
    let n = process.components().len();
    let column = |x: i32, y: i32, census: &mut Census| {
        for (start, step) in [(0, 1), (-1, -1)] {
            let mut z = start;
            while (-16..16).contains(&z) {
                let cell = key([x, y, z]);
                let bound = process.bound(galaxy, &cell);
                if bound.value() * cell.volume_ly3() < NEGLIGIBLE_MEAN {
                    break;
                }
                for candidate in kept_candidates(galaxy, &process, &cell) {
                    let slot = process
                        .components()
                        .iter()
                        .position(|young| young.id() == candidate.component)
                        .unwrap();
                    census.candidates[slot] += 1;
                    match Verdict::of(galaxy, &candidate) {
                        Verdict::Prefiltered => panic!("{:?} passes the prefilter", candidate.id),
                        Verdict::NoWindow => {}
                        Verdict::CoreCollapse => {
                            census.windows += 1;
                            census.core_collapses += 1;
                        }
                        Verdict::Host(entry) => {
                            census.windows += 1;
                            census.entries[slot] += 1;
                            census.by_node[LBV_SAMPLER.node_of(candidate.mass).unwrap()] += 1;
                            let (start, end) = entry.window();
                            if start <= UniverseTime::EPOCH && UniverseTime::EPOCH <= end {
                                census.at_epoch += 1;
                            }
                            let host = entry.stars(galaxy);
                            if matches!(host.death_time(), ClockDeath::At(t, _) if t <= ClockWindow::END)
                            {
                                census.dead += 1;
                            }
                            if slot == 0 {
                                census.young_masses.push(candidate.mass.value());
                            }
                        }
                    }
                }
                z += step;
            }
        }
    };
    let quarter = |worker: i32| {
        let mut census = Census::new(n);
        for x in (-16..16).filter(|x: &i32| x.rem_euclid(4) == worker) {
            for y in -16..16 {
                column(x, y, &mut census);
            }
        }
        census
    };
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
    #[cfg(target_family = "wasm")]
    let quarters: Vec<Census> = (0..4).map(quarter).collect();
    let mut total = Census::new(n);
    for quarter in &quarters {
        total.add(quarter);
    }
    total.young_masses.sort_by(f64::total_cmp);
    total
}

/// The fixture's census, taken once.
fn fixture_census() -> &'static Census {
    static CENSUS: OnceLock<Census> = OnceLock::new();
    CENSUS.get_or_init(|| census(galaxy()))
}

/// The census's candidates are the young field's layer-E systems of the nodes' masses and ages: in
/// each component, as many as its systems × its layer-E share × Σ over the nodes of the node's
/// share × the age distribution's mass over its span, at α = 10⁻³. The young disc's are exact; the
/// nuclear disc's two components are pooled.
#[test]
fn candidates_follow_the_young_field_s_systems() {
    let galaxy = galaxy();
    let process = LbvProcess::new(galaxy);
    let fields = galaxy.fields();
    let census = fixture_census();
    let expected: Vec<f64> = process
        .components()
        .iter()
        .map(|young| {
            let laws = fields.component(young.id());
            // The field's ages, which from 27 give most young stars to the features (P09.T2.c),
            // as its share does.
            let ages = galaxy.field_ages(young.id());
            let share = process.nodes().iter().fold(0.0, |sum, node| {
                let (lo, hi) = node.ages();
                sum + node.share() * (ages.cdf(hi) - ages.cdf(lo))
            });
            laws.count_with_unborn() * young.share() * share
        })
        .collect();
    eprintln!("candidates {:?} against {expected:?}", census.candidates);
    assert_poisson_count(
        "the young disc's kept candidates",
        census.candidates[0],
        expected[0],
        ALPHA,
    );
    assert_poisson_count(
        "the nuclear disc's kept candidates",
        census.candidates[1] + census.candidates[2],
        expected[1] + expected[2],
        ALPHA,
    );
}

/// Pins the class on the fixture: its components and rates, its nodes, a run of cells along the
/// disc and about the centre (each one's bound, its candidate count and every entry's ID, mass,
/// age, metallicity, window, site and velocity), and the galaxy's census: its hosts per galaxy,
/// by component and by node, the candidates and windows behind them, the core collapses the class
/// leaves to their own, the luminous blue variables at the epoch and the hosts dead by +H.
#[test]
fn lbv_golden() {
    let galaxy = galaxy();
    let process = LbvProcess::new(galaxy);
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for young in process.components() {
        let label = format!("component {}", young.id().index());
        w.f64(&format!("{label} share"), young.share());
        w.f64(&format!("{label} rate per year"), young.rate_per_year());
        w.f64(
            &format!("{label} candidates per system"),
            young.candidates_per_system(),
        );
    }
    for (k, node) in process.nodes().iter().enumerate() {
        let (lo, hi) = node.masses();
        let (first, last) = node.ages();
        let label = format!("node {k}");
        w.f64(&format!("{label} lightest mass"), lo.value());
        w.f64(&format!("{label} heaviest mass"), hi.value());
        w.f64(&format!("{label} first age"), first.value());
        w.f64(&format!("{label} last age"), last.value());
        w.f64(&format!("{label} share"), node.share());
        w.f64(&format!("{label} weight"), node.weight().value());
    }
    for cell in sample_entries() {
        let key = cell.key();
        let label = format!("cell {:?}", key.cell());
        w.f64(
            &format!("{label} bound"),
            process.bound(galaxy, &key).value(),
        );
        w.line(&format!(
            "{label} candidates {} entries {}",
            cell.candidates(),
            cell.entries().len()
        ));
        for entry in cell.entries() {
            let value = entry.value();
            let record = value.record();
            let id = format!("{label} {:016x}", SystemId::from(entry.id()).raw());
            let (start, end) = value.window();
            w.line(&format!(
                "{id} component {} window {start} to {end}",
                value.component().index()
            ));
            w.f64(&format!("{id} mass"), record.primary_initial_mass().value());
            w.f64(&format!("{id} age"), record.age_at_epoch().value());
            w.f64(&format!("{id} feh"), value.composition().fe_h().value());
            let site = record.epoch_position();
            w.line(&format!("{id} site cell {:?}", site.cell().to_array()));
            for (axis, metres) in site.offset_metres().iter().enumerate() {
                w.f64(&format!("{id} site offset {axis}"), *metres);
            }
            for (axis, v) in value
                .motion()
                .velocity_at(UniverseTime::EPOCH)
                .metres_per_second()
                .iter()
                .enumerate()
            {
                w.f64(&format!("{id} velocity {axis}"), *v);
            }
        }
    }
    let census = fixture_census();
    w.line(&format!("hosts per galaxy {}", census.hosts()));
    w.line(&format!("hosts by component {:?}", census.entries));
    w.line(&format!("hosts by node {:?}", census.by_node));
    w.line(&format!(
        "candidates by component {:?}, windows {}, core collapses {}",
        census.candidates, census.windows, census.core_collapses
    ));
    w.line(&format!(
        "luminous blue variables at the epoch {}, hosts dead by +H {}",
        census.at_epoch, census.dead
    ));
    golden!("catalogue_classes/lbv", w.as_str());
}

// 5. The million-star sample (slow).

/// The young disc's layer-E systems as the grid places them: `n` grid-shaped records at positions
/// drawn from the young disc's density (a cell of the class's grid picked by its density bound,
/// a point uniform in it, kept on the density ÷ the bound), with band-E masses from the galaxy's
/// mass function and ages from the component's own distribution, from `lcg`.
fn young_disc_sample(galaxy: &Galaxy, n: u32, lcg: &mut Lcg) -> Vec<SystemRecord> {
    let process = LbvProcess::new(galaxy);
    let young = process.components()[0].id();
    let laws = galaxy.fields().component(young);
    let cells: Vec<(CatalogueCellKey, f64)> = (-16..16)
        .flat_map(|x| (-16..16).flat_map(move |y| (-16..16).map(move |z| key([x, y, z]))))
        .map(|cell| {
            let cube = CellBox::new(cell.origin_ly(), cell.edge_ly()).unwrap();
            (cell, galaxy.fields().component_bound(young, &cube))
        })
        .filter(|(_, bound)| *bound > 0.0)
        .collect();
    let mut cumulative = Vec::with_capacity(cells.len());
    let mut total = 0.0;
    for (cell, bound) in &cells {
        total += bound * cell.volume_ly3();
        cumulative.push(total);
    }
    let mut records = Vec::with_capacity(usize::try_from(n).unwrap());
    let mut index = 0;
    while index < n {
        let u = lcg.next_f64() * total;
        let k = cumulative.partition_point(|c| *c <= u).min(cells.len() - 1);
        let (cell, bound) = cells[k];
        let p = cell.position_from_words([lcg.next_u64(), lcg.next_u64(), lcg.next_u64()]);
        if laws.density(&PointLy::from(&p)) <= lcg.next_f64() * bound {
            continue;
        }
        let mass =
            galaxy
                .mass_function()
                .quantile_in(MassBand::E.lo(), MassBand::E.hi(), lcg.next_f64());
        let age = laws.ages().quantile(lcg.next_f64()).value();
        records.push(grid_record(galaxy, young, p, index, mass, age));
        index += 1;
    }
    records
}

/// What the brute force finds in a sample: its records, the hosts among them, those the core
/// collapses claim too, the prefilter's passes, and the hosts' masses.
#[derive(Debug, Clone, Default)]
struct SampleCount {
    records: u64,
    passes: u64,
    hosts: u64,
    core_collapses: u64,
    masses: Vec<f64>,
}

/// P09.T43, the plan's test: over a 10⁶-star layer-E sample of the young disc the grid's claim
/// holds every star whose window overlaps the source horizon and nothing else, read by brute force
/// from every star's own stars; the prefilter's pass rate; and the class agrees with the grid over
/// the galaxy (P09.T38's complementarity, its field half): the census's young-disc hosts are the
/// sample's share of hosts the core collapses leave × the young disc's layer-E systems, within
/// their joint error at α = 10⁻³, and their masses pass a two-sample K–S test at the 1% level
/// against the sample's hosts' (P15.T10.a's acceptance). On 16 threads natively; on
/// wasm32-wasip1, which has no threads, 50,000 stars and the claim alone, since the million would
/// take hours on one.
#[test]
#[ignore = "slow: the brute-force read of a million stars' windows"]
fn the_class_test_holds_every_host_of_a_million_layer_e_stars() {
    const WASM: bool = cfg!(target_family = "wasm");
    const STARS: u32 = if WASM { 50_000 } else { 1_000_000 };
    const WORKERS: u32 = if WASM { 1 } else { 16 };
    let galaxy = galaxy();
    let mut lcg = Lcg::new(0x0943_0005);
    let records = young_disc_sample(galaxy, STARS, &mut lcg);
    let count = |chunk: &[SystemRecord]| {
        let mut count = SampleCount::default();
        for record in chunk {
            let truth = horizon_window(&SystemStars::generate(galaxy, record)).is_some();
            assert_eq!(lbv_claims(galaxy, record), truth, "{record:?}");
            count.records += 1;
            if may_host(record.primary_initial_mass(), record.age_at_epoch()) {
                count.passes += 1;
            }
            if truth {
                count.hosts += 1;
                if grid_core_collapse_claims(galaxy, record) {
                    count.core_collapses += 1;
                } else {
                    count.masses.push(record.primary_initial_mass().value());
                }
            }
        }
        count
    };
    let size = records.len().div_ceil(usize::try_from(WORKERS).unwrap());
    #[cfg(not(target_family = "wasm"))]
    let counts: Vec<SampleCount> = thread::scope(|scope| {
        let workers: Vec<_> = records
            .chunks(size)
            .map(|chunk| scope.spawn(move || count(chunk)))
            .collect();
        workers
            .into_iter()
            .map(|w| w.join().expect("a sample worker"))
            .collect()
    });
    #[cfg(target_family = "wasm")]
    let counts: Vec<SampleCount> = records.chunks(size).map(count).collect();
    let mut sample = SampleCount::default();
    for c in counts {
        sample.records += c.records;
        sample.passes += c.passes;
        sample.hosts += c.hosts;
        sample.core_collapses += c.core_collapses;
        sample.masses.extend(c.masses);
    }
    if WASM {
        eprintln!("{} stars: {} hosts", sample.records, sample.hosts);
        return;
    }
    #[expect(clippy::cast_precision_loss, reason = "counts below 2⁵³ are exact")]
    let (records, passes, hosts, left) = (
        sample.records as f64,
        sample.passes as f64,
        sample.hosts as f64,
        (sample.hosts - sample.core_collapses) as f64,
    );
    let process = LbvProcess::new(galaxy);
    let young = process.components()[0];
    let laws = galaxy.fields().component(young.id());
    let layer_e = laws.count_with_unborn() * young.share();
    let census = fixture_census();
    let expected = layer_e * left / records;
    eprintln!(
        "{} stars: {} pass the prefilter ({:.3e}), {} hosts, {} of them core collapses; the \
         young disc's {layer_e:.4e} layer-E systems hold {expected:.1} hosts; the census {}",
        sample.records,
        sample.passes,
        passes / records,
        sample.hosts,
        sample.core_collapses,
        census.entries[0]
    );
    assert!(hosts > 100.0, "{hosts}");
    // The census is a Poisson count, the sample's share a binomial one: their joint z score.
    #[expect(clippy::cast_precision_loss, reason = "counts below 2⁵³ are exact")]
    let observed = census.entries[0] as f64;
    let sigma = (expected + expected * expected / left).sqrt();
    let z = (observed - expected) / sigma;
    assert_p_value(
        "the census's young-disc hosts",
        2.0 * normal_cdf(-z.abs()),
        ALPHA,
    );
    let mut class_masses = census.young_masses.clone();
    let ks = ks_two_sample(&mut class_masses, &mut sample.masses);
    eprintln!("masses: K–S {ks:?}");
    assert_p_value("the hosts' masses", ks.p_value, 0.01);
}

/// P09.T43 over seeds: the hosts per galaxy of three drawn galaxies, each some hundreds, with the
/// luminous blue variables at their epochs, which plan 06's P06.T31 puts near 181 at Milky Way
/// values; every host's window overlaps the horizon, which the census's verdicts hold. On
/// wasm32-wasip1, which has no threads, the first galaxy alone.
#[test]
#[ignore = "slow: the class's census of three drawn galaxies"]
fn lbv_hosts_over_seeds() {
    let seeds = [0x0943_0010, 0x0943_0011, 0x0943_0012];
    let seeds = if cfg!(target_family = "wasm") {
        &seeds[..1]
    } else {
        &seeds[..]
    };
    for &seed in seeds {
        let galaxy = Galaxy::new(Seed::new(seed)).with_full_potential();
        let census = census(&galaxy);
        eprintln!(
            "seed {seed:#x}: {} hosts {:?} by node {:?}; {} at the epoch, {} dead by +H, {} \
             windows left to the core collapses",
            census.hosts(),
            census.entries,
            census.by_node,
            census.at_epoch,
            census.dead,
            census.core_collapses
        );
        assert!(
            (100..=3_000).contains(&census.hosts()),
            "{}",
            census.hosts()
        );
    }
}

/// P09.T38's fault, fixed at 27: a living runaway or walkaway of the grid is one of plan 08's
/// displaced systems, which the class, drawing the field's systems alone, never places, so the
/// grid keeps it, where up to 26 the grid's claim took it all the same when its window overlapped
/// the source horizon, so that nobody held it. A living field star so aged is claimed.
#[test]
fn a_displaced_host_is_the_grid_s_from_27() {
    use crate::galaxy::displaced::DisplacedKind;
    use crate::galaxy::rules::GalaxyRules;
    /// `base` aged a thousand years into its window at the first of a few masses at which its
    /// primary has one, read from a probe long dead, whose track covers its whole life; none if
    /// at none of them (a metal-rich star's window can close).
    fn host_of(galaxy: &Galaxy, base: &SystemRecord) -> Option<SystemRecord> {
        [60.0, 80.0, 100.0, 120.0, 150.0]
            .into_iter()
            .find_map(|mass| {
                let probe = base
                    .with_primary_initial_mass(SolarMasses::new(mass))
                    .with_age_at_epoch(Years::new(3.0e7));
                let probed =
                    SystemStars::generate_in(galaxy, &probe, MultiplicityContext::ForcedSingle);
                let (start, _) = probed.lbv_window()?;
                let since = start.since_epoch().as_julian_years_f64();
                Some(probe.with_age_at_epoch(Years::new(3.0e7 + since + 1.0e3)))
            })
    }
    for (rules, claimed) in [(GalaxyRules::of_version(crate::GENERATOR_VERSION), false)] {
        let galaxy =
            Galaxy::from_params_with_rules(Seed::new(SEED), GalaxyParams::milky_way_like(), rules)
                .expect("the Milky Way fixture builds");
        // The arms' ridges at the Sun's radius, where the young field's layer E is richest.
        let (mut cell, mut ejected, mut field) = (Vec::new(), None, None);
        let arms = galaxy.fields().arms();
        let ridges = (0..4).flat_map(|arm| {
            let (sin, cos) = crate::math::sin_cos(arms.ridge_azimuth(26_000.0, arm));
            (-16..16).map(move |step| {
                let along = 128.0 * f64::from(step);
                [
                    26_000.0 * cos - along * sin,
                    26_000.0 * sin + along * cos,
                    0.0,
                ]
            })
        });
        for at in ridges {
            let at = GalacticPosition::from_light_years(at).expect("in the cube");
            placement::generate_cell(
                &galaxy,
                CellKey::containing(Layer::E, &at).expect("a cell"),
                &mut cell,
            );
            for record in &cell {
                let displaced = match record.placement_class() {
                    PlacementClass::Displaced {
                        kind: DisplacedKind::Runaway | DisplacedKind::Walkaway,
                        ..
                    } => true,
                    PlacementClass::Alive => false,
                    PlacementClass::Retained { .. } | PlacementClass::Displaced { .. } => continue,
                };
                let slot = if displaced { &mut ejected } else { &mut field };
                if slot.is_none() {
                    *slot = host_of(&galaxy, record);
                }
            }
            if ejected.is_some() && field.is_some() {
                break;
            }
        }
        let ejected = ejected.expect("a runaway or walkaway on the ridges");
        let field = field.expect("a living field star on the ridges");
        for host in [ejected, field] {
            assert!(may_host(host.primary_initial_mass(), host.age_at_epoch()));
            assert!(
                horizon_window(&SystemStars::generate_in(
                    &galaxy,
                    &host,
                    MultiplicityContext::ForcedSingle
                ))
                .is_some()
            );
        }
        assert_eq!(lbv_claims(&galaxy, &ejected), claimed, "{rules:?}");
        assert!(lbv_claims(&galaxy, &field), "{rules:?}");
    }
}
