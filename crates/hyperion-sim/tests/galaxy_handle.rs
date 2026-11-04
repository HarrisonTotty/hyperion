//! The `Galaxy` handle and its share matrix (plan 02, P02.T9): what it bundles, that it is the
//! same however and whenever it is built, that its layers add up to the fields and are pinned by
//! the golden file, and what it costs to keep, the figure plan 04 budgets its galaxy cache by.

use std::fmt::Debug;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::panic::{RefUnwindSafe, UnwindSafe};

use hyperion_sim::galaxy::fields::{Fields, MAX_COMPONENTS};
use hyperion_sim::galaxy::imf::{BandShares, MassBand, MassFunctionKind};
use hyperion_sim::galaxy::kinematics::KinematicTables;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::potential::{MassModel, PotentialTables};
use hyperion_sim::galaxy::shares::{POPULATION_COLUMNS, ShareMatrix};
use hyperion_sim::galaxy::substellar::{SubstellarAbundance, SubstellarParams};
use hyperion_sim::galaxy::{Galaxy, POPULATIONS, PointLy, Population};
use hyperion_sim::units::LightYears;
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::order::assert_order_independent;

/// The three pinned seeds of the galaxy golden files.
const PINNED: [u64; 3] = [
    0x0000_0000_0000_0001,
    0x5eed_0000_c0ff_ee00,
    0xdead_beef_cafe_f00d,
];

/// A value as its `Debug` text, which prints every float in its shortest round-tripping form and
/// so tells any two different values apart, 0 from −0 included: equal texts are equal bits.
fn text(value: &(impl Debug + ?Sized)) -> String {
    format!("{value:?}")
}

/// A short fingerprint of [`text`], so that a failing comparison does not print megabytes.
/// `DefaultHasher::new` is keyed alike on every call within a process.
fn fingerprint(galaxy: &Galaxy) -> (u64, usize) {
    let text = text(galaxy);
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    (hasher.finish(), text.len())
}

/// Asserts that two values, galaxies or their parts, are equal bit for bit, naming the first
/// difference without printing megabytes.
#[track_caller]
fn assert_same<T: Debug + ?Sized>(what: &str, a: &T, b: &T) {
    let (a, b) = (text(a), text(b));
    if let Some(at) = a.bytes().zip(b.bytes()).position(|(x, y)| x != y) {
        let context = |s: &str| {
            String::from_utf8_lossy(&s.as_bytes()[at.saturating_sub(80)..(at + 80).min(s.len())])
                .into_owned()
        };
        panic!(
            "{what}: the values differ at byte {at}:\n  {}\nagainst\n  {}",
            context(&a),
            context(&b)
        );
    }
    assert_eq!(
        a.len(),
        b.len(),
        "{what}: one value's text extends the other's"
    );
}

/// Points across the galaxy: the centre, the nuclear disc, the bulge, the bar and its end, the
/// arms and the solar circle on and off the plane, the thick disc, the halo and beyond its cut.
const POINTS: [(f64, f64, f64); 16] = [
    (0.0, 0.0, 0.0),
    (40.0, -25.0, 3.0),
    (600.0, 300.0, 150.0),
    (-900.0, 400.0, -500.0),
    (8_000.0, 1_500.0, 100.0),
    (15_800.0, -2_000.0, -40.0),
    (-12_000.0, 17_000.0, 0.5),
    (22_516.7, 13_000.0, 50.0),
    (26_000.0, 0.0, -300.0),
    (-20_000.0, -15_000.0, 1_200.0),
    (30_000.0, 25_000.0, 3_000.0),
    (5_000.0, -8_000.0, 9_000.0),
    (-40_000.0, 10_000.0, -20_000.0),
    (0.0, 0.0, 45_000.0),
    (48_000.0, 0.0, 0.0),
    (60_000.0, 30_000.0, 20_000.0),
];

fn points() -> impl Iterator<Item = PointLy> {
    POINTS.iter().map(|&(x, y, z)| PointLy::new(x, y, z))
}

#[test]
fn the_galaxy_handle_is_send_sync_and_unwind_safe() {
    // A `Cell`, `RefCell` or `OnceCell` anywhere inside would make it neither `Sync` nor
    // `RefUnwindSafe`; a `Box<dyn …>` without those bounds would fail the latter too. Locks,
    // `OnceLock` and atomics would pass; the sim holds two `OnceLock`s, the parameters' black hole,
    // a pure function of the parameters solved as the galaxy is built, and the galaxy's centre
    // motion, a pure function of the parameters and the mass model built on first read (plan 09,
    // P09.T28.b.a).
    fn shareable<T: Send + Sync + RefUnwindSafe + UnwindSafe + Clone + Debug + PartialEq>() {}
    shareable::<Galaxy>();
    shareable::<ShareMatrix>();
}

#[test]
fn a_galaxy_handle_holds_the_parts_built_one_by_one() {
    let seed = Seed::new(PINNED[1]);
    for kind in [MassFunctionKind::Chabrier, MassFunctionKind::Kroupa] {
        let galaxy = Galaxy::with_mass_function(seed, kind);
        let params = GalaxyParams::from_seed(seed, kind);
        let model = MassModel::new(&params);
        let bands = BandShares::of(kind.to_mass_function().as_ref());
        assert_eq!(galaxy.seed(), seed);
        assert_same("params", galaxy.params(), &params);
        assert_same("mass model", galaxy.mass_model(), &model);
        assert_same(
            "potential",
            galaxy.potential(),
            &PotentialTables::in_plane(&model),
        );
        assert!(!galaxy.potential().has_grid());
        // From generator version 27 the build installs plan 08's displaced classes after the
        // field's components (P08.T12.a): the components are the field's own.
        assert_same(
            "components",
            galaxy.fields().components(),
            Fields::new(&params, &model).components(),
        );
        assert!(galaxy.fields().displaced().is_some(), "{kind:?}");
        // Plan 13's two substellar rows hold the galaxy's own abundances per system.
        let substellar = SubstellarAbundance::for_galaxy(&galaxy, &SubstellarParams::default());
        assert_eq!(galaxy.substellar(), &substellar, "{kind:?}");
        // The classes' columns follow the populations', which they leave bit for bit.
        let uniform = ShareMatrix::uniform(&bands).with_substellar(
            substellar.brown_dwarfs_per_system(),
            substellar.rogue_planets_per_system(),
        );
        for band in MassBand::ALL.into_iter().chain(MassBand::SUBSTELLAR) {
            assert_same(
                "shares",
                &galaxy.shares().row(band)[..POPULATION_COLUMNS],
                &uniform.row(band)[..POPULATION_COLUMNS],
            );
        }
        assert_eq!(
            text(&galaxy.mass_function()),
            text(&kind.to_mass_function()),
            "{kind:?}"
        );
        assert_same_bits(galaxy.system_count(), params.system_count());
        assert_same_bits(
            galaxy.mean_formed_mass().value(),
            params.mean_formed_mass().value(),
        );
        for population in POPULATIONS {
            assert_same_bits(
                galaxy.mean_system_mass(population).value(),
                params.mean_system_mass(population).value(),
            );
        }
    }
    let default = Galaxy::new(seed);
    assert_same(
        "new against the default mass function",
        &default,
        &Galaxy::with_mass_function(seed, MassFunctionKind::default()),
    );
    assert_same(
        "new against its parameters",
        &default,
        &Galaxy::from_params(
            seed,
            GalaxyParams::from_seed(seed, MassFunctionKind::default()),
        )
        .expect("this galaxy's gas is mostly neutral"),
    );
}

/// `text`, a fields' `Debug` text, without its displaced classes' table's field stays
/// (`field: Some([…])`): the share of each component's budget its field keeps (plan 09,
/// P09.T2.c).
fn without_field_stays(text: &str) -> String {
    const START: &str = "field: Some([";
    const END: &str = "]]),";
    assert_eq!(text.matches(START).count(), 1, "one table's field stays");
    let start = text.find(START).expect("the class table's field stays");
    let end = start + text[start..].find(END).expect("the field stays' end") + END.len();
    format!("{}{}", &text[..start], &text[end..])
}

/// A galaxy from parameters keeps them and its seed. The seed keys placement, the gas field's
/// draws and, from generator version 27, the halo's discrete share (plan 10, P10.T3.d.b): the
/// global list's dwarfs, drawn on the seed, hold their share of the halo's budget, which the halo's
/// field gives up, 0.0254 of it at this galaxy's seed and 0.0296 at the other's. So another seed
/// gives the same galaxy otherwise: the same potential and share matrix, and the same fields but
/// for the share of each halo component's budget its field keeps (the class table's field stays,
/// P09.T2.c), which is its budget's stay share less that share.
#[test]
fn a_galaxy_handle_from_params_keeps_them_and_its_seed() {
    let params = GalaxyParams::milky_way_like();
    let seed = Seed::new(0x0209_0000_0000_0001);
    let galaxy =
        Galaxy::from_params(seed, params.clone()).expect("this galaxy's gas is mostly neutral");
    assert_eq!(galaxy.seed(), seed);
    assert_same("params", galaxy.params(), &params);
    let other =
        Galaxy::from_params(Seed::new(7), params).expect("this galaxy's gas is mostly neutral");
    assert_same(
        "fields without their field stays",
        &without_field_stays(&text(other.fields())),
        &without_field_stays(&text(galaxy.fields())),
    );
    assert_same("potential", other.potential(), galaxy.potential());
    assert_same("shares", other.shares(), galaxy.shares());
    let phi = |g: &Galaxy, band| g.feature_shares().phi(Population::Halo, band);
    let (ours, theirs) = (
        galaxy.class_table().expect("the version's classes"),
        other.class_table().expect("the version's classes"),
    );
    let mut halo_rows = 0;
    for id in galaxy.fields().component_ids() {
        let halo = galaxy.fields().component(id).population() == Population::Halo;
        for band in MassBand::ALL.into_iter().chain(MassBand::SUBSTELLAR) {
            let (a, b) = (ours.stay_share(band, id), theirs.stay_share(band, id));
            if halo {
                let moved =
                    ours.budget_stay_share(band, id) * (phi(&galaxy, band) - phi(&other, band));
                assert!(moved.abs() > 1e-4, "{id:?} {band:?}: {moved}");
                assert!(
                    (b - a - moved).abs() < 1e-14,
                    "{id:?} {band:?}: {a} {b} {moved}"
                );
            } else {
                assert_same_bits(b, a);
            }
        }
        halo_rows += usize::from(halo);
    }
    assert!(halo_rows > 0, "the fixture has a halo");
}

#[test]
fn every_share_column_of_a_galaxy_handle_sums_to_one() {
    for kind in [MassFunctionKind::Chabrier, MassFunctionKind::Kroupa] {
        let galaxy = Galaxy::with_mass_function(Seed::new(PINNED[0]), kind);
        let shares = galaxy.shares();
        let bands = BandShares::of(galaxy.mass_function());
        // The populations, then from generator version 27 plan 08's displaced classes
        // (P08.T12.a), whose columns hold their systems inside the cube in layers D and E.
        let classes = galaxy.fields().displaced().map_or(0, |d| d.classes().len());
        assert_eq!(shares.column_count(), POPULATION_COLUMNS + classes);
        for column in 0..POPULATION_COLUMNS {
            let total = MassBand::ALL
                .iter()
                .fold(0.0, |sum, &band| sum + shares.row(band)[column]);
            assert!(
                (total - 1.0).abs() < 1e-14,
                "{kind:?}, column {column}: {total}"
            );
        }
        for band in MassBand::ALL {
            assert_eq!(shares.row(band).len(), shares.column_count());
            for population in POPULATIONS {
                assert_same_bits(shares.share(band, population), bands.share(band));
                assert_same_bits(
                    shares.row(band)[population.index()],
                    shares.share(band, population),
                );
            }
        }
    }
}

#[test]
fn layer_densities_of_a_galaxy_handle_sum_to_the_total() {
    let galaxies = [
        Galaxy::from_params(Seed::new(1), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral"),
        Galaxy::new(Seed::new(PINNED[2])),
        Galaxy::with_mass_function(Seed::new(PINNED[0]), MassFunctionKind::Kroupa),
    ];
    let mut out = [0.0; MAX_COMPONENTS];
    for galaxy in &galaxies {
        let (fields, shares) = (galaxy.fields(), galaxy.shares());
        for p in points() {
            let mut total = fields.densities(&p, &mut out);
            let layers = MassBand::ALL.iter().fold(0.0, |sum, &band| {
                sum + fields.layer_density(shares, band, &p)
            });
            // From generator version 27 layers D and E give up the dead that leave their birth
            // place to plan 08's displaced classes (P08.T12.a), each class's density weighted by
            // its column: the total is what the components keep and what the classes hold.
            if let Some(displaced) = fields.displaced() {
                let kept = fields.components().iter().fold(0.0, |sum, component| {
                    let density = component.density(&p);
                    MassBand::ALL.iter().fold(sum, |sum, &band| {
                        sum + shares.component_share(band, component) * density
                    })
                });
                let moved = MassBand::ALL.iter().fold(0.0, |sum, &band| {
                    let row = shares.row(band);
                    displaced.order(band).iter().fold(sum, |sum, &id| {
                        sum + row[POPULATION_COLUMNS + id.index()] * displaced.density(id, &p)
                    })
                });
                total = kept + moved;
            }
            // Beyond every cut and underflow, nothing at all.
            if total <= 0.0 {
                assert_same_bits(layers, 0.0);
                continue;
            }
            let error = (layers / total - 1.0).abs();
            assert!(
                error < 1e-13,
                "seed {}, {p:?}: the layers give {layers:e} against {total:e}",
                galaxy.seed()
            );
            // And component by component, each population's share of each band (the share before
            // the displaced classes take their part of layers D and E).
            for component in fields.components() {
                let density = component.density(&p);
                let by_band = MassBand::ALL.iter().fold(0.0, |sum, &band| {
                    sum + shares.share(band, component.population()) * density
                });
                // A subnormal density keeps fewer bits: its sum is held to a unit in the last
                // place of the smallest normal number too (the young disc 20,000 ly above the
                // plane since its height rose to 335 ly, plan 02, P02.T12.d).
                assert!(
                    (by_band - density).abs()
                        <= 1e-14 * density + 4.0 * f64::MIN_POSITIVE * f64::EPSILON,
                    "seed {}, {p:?}, {:?}: the bands give {by_band:e} against {density:e}",
                    galaxy.seed(),
                    component.population()
                );
            }
        }
    }
}

/// Every layer's density at [`POINTS`], checked bit for bit against `Σ share × density` in
/// component order, then the displaced classes' in theirs: the sum placement thins against (plan
/// 03, P08.T12.a).
fn write_layers(w: &mut GoldenWriter, label: &str, galaxy: &Galaxy) {
    let (fields, shares) = (galaxy.fields(), galaxy.shares());
    for (i, p) in points().enumerate() {
        for band in MassBand::ALL {
            let layer = fields.layer_density(shares, band, &p);
            let field = fields.components().iter().fold(0.0, |sum, c| {
                sum + shares.component_share(band, c) * c.density(&p)
            });
            // From generator version 27 the sum runs on over plan 08's displaced classes in the
            // band's order (P08.T12.a).
            let by_component = fields.displaced().map_or(field, |displaced| {
                let row = shares.row(band);
                displaced.order(band).iter().fold(field, |sum, &id| {
                    sum + row[POPULATION_COLUMNS + id.index()] * displaced.density(id, &p)
                })
            });
            assert_same_bits(layer, by_component);
            w.f64(&format!("{label}.point[{i}].layer_{band:?}"), layer);
        }
    }
}

#[test]
fn galaxy_handle_layers_are_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    w.line("# milky_way, seed 0");
    write_layers(
        &mut w,
        "milky_way",
        &Galaxy::from_params(Seed::new(0), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral"),
    );
    for s in PINNED {
        let seed = Seed::new(s);
        w.line(&format!("# {seed}"));
        write_layers(&mut w, &seed.to_string(), &Galaxy::new(seed));
    }
    golden!("galaxy_handle", w.as_str());
}

#[test]
fn a_galaxy_handle_is_the_same_when_built_twice() {
    let seed = Seed::new(PINNED[1]);
    assert_same("new", &Galaxy::new(seed), &Galaxy::new(seed));
    assert_same(
        "Kroupa's",
        &Galaxy::with_mass_function(seed, MassFunctionKind::Kroupa),
        &Galaxy::with_mass_function(seed, MassFunctionKind::Kroupa),
    );
    assert_same(
        "the fixture",
        &Galaxy::from_params(seed, GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral"),
        &Galaxy::from_params(seed, GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral"),
    );
    let galaxy = Galaxy::new(seed);
    assert_same("a clone", &galaxy, &galaxy.clone());
}

#[test]
fn a_galaxy_handle_is_independent_of_what_was_built_before() {
    #[derive(Debug)]
    enum Build {
        Seed(u64),
        Kroupa(u64),
        Fixture,
    }
    let builds = [
        Build::Seed(PINNED[0]),
        Build::Fixture,
        Build::Kroupa(PINNED[2]),
    ];
    assert_order_independent(&builds, |build| {
        let galaxy = match *build {
            Build::Seed(s) => Galaxy::new(Seed::new(s)),
            Build::Kroupa(s) => Galaxy::with_mass_function(Seed::new(s), MassFunctionKind::Kroupa),
            Build::Fixture => Galaxy::from_params(Seed::new(0), GalaxyParams::milky_way_like())
                .expect("the Milky Way fixture's gas is mostly neutral"),
        };
        fingerprint(&galaxy)
    });
}

#[test]
fn the_galaxy_handle_reports_its_heap_size() {
    let fixture = Galaxy::from_params(Seed::new(0), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral");
    let mut sizes = vec![("fixture".to_owned(), fixture.heap_bytes())];
    for s in PINNED {
        let galaxy = Galaxy::new(Seed::new(s));
        sizes.push((format!("{s:016x}"), galaxy.heap_bytes()));
    }
    let handle = size_of::<Galaxy>();
    for (label, bytes) in &sizes {
        println!(
            "galaxy {label}: {bytes} bytes on the heap ({:.2} MiB) and {handle} in the handle",
            f64::from(u32::try_from(*bytes).expect("under 4 GiB")) / f64::from(1_u32 << 20)
        );
    }
    let smallest = sizes.iter().map(|(_, b)| *b).min().expect("four galaxies");
    let largest = sizes.iter().map(|(_, b)| *b).max().expect("four galaxies");
    // Fixed-size tables but for the halo's three to six components: the size barely varies,
    // which is what lets plan 04 bound its galaxy cache by entries (its design note 23).
    assert!(
        largest - smallest < smallest / 20,
        "{smallest} to {largest} bytes"
    );
    assert!(
        (512 << 10..4 << 20).contains(&largest),
        "{largest} bytes on the heap"
    );
    assert!(handle < 16 << 10, "{handle} bytes in the handle");
}

#[test]
#[ignore = "slow: the (R, z) grid takes seconds to build"]
fn a_galaxy_handle_with_the_full_potential_adds_the_grid_and_the_kinematics_alone() {
    let galaxy = Galaxy::new(Seed::new(PINNED[1]));
    // Built afresh rather than cloned, since a clone holds its vectors without spare capacity.
    let full = Galaxy::new(Seed::new(PINNED[1])).with_full_potential();
    assert!(full.potential().has_grid());
    assert_same(
        "potential",
        full.potential(),
        &PotentialTables::full(galaxy.mass_model()),
    );
    assert_same("params", full.params(), galaxy.params());
    assert_same("mass model", full.mass_model(), galaxy.mass_model());
    assert_same("fields", full.fields(), galaxy.fields());
    assert_same("shares", full.shares(), galaxy.shares());
    // The grid, 64 × 64 points of four values, and plan 08's kinematic tables that read it.
    let kinematics = full
        .kinematics()
        .expect("the full potential builds the kinematics");
    assert!(galaxy.kinematics().is_none());
    assert_eq!(
        full.heap_bytes() - galaxy.heap_bytes(),
        64 * 64 * 32 + size_of::<KinematicTables>() + kinematics.heap_bytes()
    );
    let sun = LightYears::new(26_000.0);
    assert!(
        full.potential()
            .potential(sun, LightYears::new(1_000.0))
            .is_some()
    );
    assert!(
        galaxy
            .potential()
            .potential(sun, LightYears::new(1_000.0))
            .is_none()
    );
    // Asking twice builds nothing more.
    assert_same("twice", &full.clone().with_full_potential(), &full);
}
