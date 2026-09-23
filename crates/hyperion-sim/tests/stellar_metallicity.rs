//! The per-system metallicity draw (plan 06, P06.T3): the radial gradient it reproduces over the
//! thin disc, the separation of the halo's two main components, the distribution at one place,
//! and golden values for pinned systems.
//!
//! Every statistical check is two-sided at [`ALPHA`] = 10⁻³ on a fixed seed, and its bracket comes
//! from the field's own scatter and the sample, not from a tolerance chosen to pass.

use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::fields::metallicity::THIN_DISC_FLAT_AGE;
use hyperion_sim::galaxy::fields::{Component, ComponentId};
use hyperion_sim::galaxy::params::{GalaxyParams, HaloComponentKind};
use hyperion_sim::galaxy::placement::{CellKey, SystemOrigin, SystemRecord, generate_cell};
use hyperion_sim::galaxy::{Galaxy, PointLy, Population};
use hyperion_sim::id::{Layer, SystemId};
use hyperion_sim::stellar::system::draw_metallicity;
use hyperion_sim::units::{SolarMasses, Years};
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::stats::{ALPHA, assert_p_value, ks_one_sample, ks_two_sample, normal_cdf};

/// The seed of the galaxy every test here draws in.
const SEED: u64 = 0x0600_0003_0000_5eed;

/// The two-sided standard-normal quantile at [`ALPHA`]: |z| exceeds it with probability 10⁻³.
const Z_ALPHA: f64 = 3.290_526_731_491_926;

/// Light-years in one kiloparsec (IAU 2015 parsec, Julian light-year).
const LY_PER_KPC: f64 = 3_261.563_777_167_433_6;

fn milky_way() -> Galaxy {
    Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
        .expect("the Milky Way fixture's gas is mostly neutral")
}

/// `n` candidate IDs of layer C, running through consecutive cells along +x from the Sun-like
/// cell once a cell's index field is full: IDs only, for records built from parts.
fn candidate_ids(n: u32) -> Vec<SystemId> {
    let first = CellKey::new(Layer::C, [0, 812, 0]).expect("a cell near the solar circle");
    let capacity = first.index_capacity();
    (0..n)
        .map(|i| {
            let along = i32::try_from(i / capacity).expect("a few cells");
            let key = CellKey::new(Layer::C, [along, 812, 0]).expect("a cell near the first");
            key.candidate_id(i % capacity)
                .expect("inside the index field")
        })
        .collect()
}

/// The ID of the first component for which `pick` holds.
fn component_where(galaxy: &Galaxy, pick: impl Fn(&Component) -> bool) -> ComponentId {
    let fields = galaxy.fields();
    let index = fields
        .components()
        .iter()
        .position(pick)
        .expect("the fixture has the component");
    fields.component_id(index).expect("an index of the fields")
}

/// A grid record of `component` at `position` with `age`, under `id`.
fn record(
    galaxy: &Galaxy,
    id: SystemId,
    component: ComponentId,
    position: [f64; 3],
    age: Years,
) -> SystemRecord {
    SystemRecord::from_parts(
        id,
        GalacticPosition::from_light_years(position).expect("inside the root cube"),
        SystemOrigin::Grid(component),
        galaxy.fields().component(component).population(),
        SolarMasses::new(1.0),
        age,
    )
}

/// Mean and standard deviation of `xs`, the deviation with n − 1.
fn mean_and_sd(xs: &[f64]) -> (f64, f64) {
    let n = f64::from(u32::try_from(xs.len()).expect("a test sample"));
    let mean = xs.iter().sum::<f64>() / n;
    let ss = xs.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>();
    (mean, (ss / (n - 1.0)).sqrt())
}

/// Over 10⁵ thin-disc systems placed by the grid at the Milky Way fixture's parameters, the
/// least-squares slope of the drawn \[Fe/H\] against galactic radius is the galaxy's drawn gradient
/// (`GalaxyParams::metallicity_gradient`, −0.05 dex per kpc for the fixture), and the scatter about
/// the line is the field's 0.20 dex.
///
/// The sample is every young- and old-thin-disc record of layer E in a strip of cells in the plane
/// along +y, clear of the bar, from 12,800 to 44,800 ly, whose age at the epoch is within the thin
/// discs' flat age–metallicity relation (at most `THIN_DISC_FLAT_AGE`, 8 Gyr). Over that radial
/// range the field's mean is exactly linear in radius (its clamp at −1.0 and +0.5 dex binds only
/// beyond 90,000 ly and inside the centre for this gradient), so the slope's only error is the
/// sampling error of a line fitted to normal scatter. The bracket is the slope's standard error
/// from the sample's own residuals and radii, times the two-sided quantile at α = 10⁻³; the
/// scatter's bracket is its standard error under normality, σ ÷ √(2(n − 2)), likewise.
#[test]
#[ignore = "slow: places a strip of layer-E cells across the thin disc and draws 10⁵ systems"]
fn the_thin_discs_radial_gradient_is_the_galaxys_drawn_one() {
    let galaxy = milky_way();
    let drawn = galaxy.params().metallicity_gradient().value();
    let mut radii_kpc = Vec::new();
    let mut fe_h = Vec::new();
    let mut cell = Vec::new();
    for iy in 100..350 {
        for ix in -4..4 {
            for iz in -2..2 {
                let key = CellKey::new(Layer::E, [ix, iy, iz]).expect("inside the root cube");
                cell.clear();
                generate_cell(&galaxy, key, &mut cell);
                for r in &cell {
                    let thin = matches!(
                        r.population(),
                        Population::YoungThinDisc | Population::OldThinDisc
                    );
                    if !thin || r.age_at_epoch().value() > THIN_DISC_FLAT_AGE.value() {
                        continue;
                    }
                    let p = PointLy::from(r.epoch_position());
                    radii_kpc.push((p.x * p.x + p.y * p.y).sqrt() / LY_PER_KPC);
                    fe_h.push(draw_metallicity(&galaxy, r).fe_h().value());
                }
            }
        }
    }
    let n = radii_kpc.len();
    assert!(n >= 100_000, "only {n} thin-disc systems in the strip");
    let (r_mean, _) = mean_and_sd(&radii_kpc);
    let (f_mean, _) = mean_and_sd(&fe_h);
    let sxx: f64 = radii_kpc.iter().map(|r| (r - r_mean) * (r - r_mean)).sum();
    let sxy: f64 = radii_kpc
        .iter()
        .zip(&fe_h)
        .map(|(r, f)| (r - r_mean) * (f - f_mean))
        .sum();
    let slope = sxy / sxx;
    let intercept = f_mean - slope * r_mean;
    let residual_ss: f64 = radii_kpc
        .iter()
        .zip(&fe_h)
        .map(|(r, f)| {
            let e = f - intercept - slope * r;
            e * e
        })
        .sum();
    let dof = f64::from(u32::try_from(n - 2).expect("a test sample"));
    let scatter = (residual_ss / dof).sqrt();
    let slope_error = scatter / sxx.sqrt();
    eprintln!(
        "{n} systems, radius {r_mean:.2} kpc on average (spread {:.2} kpc): slope {slope:.5} ± \
         {slope_error:.5} dex/kpc against the drawn {drawn:.5}; scatter {scatter:.4} dex",
        (sxx / dof).sqrt()
    );
    assert!(
        (slope - drawn).abs() <= Z_ALPHA * slope_error,
        "slope {slope} against the drawn gradient {drawn}, standard error {slope_error}"
    );
    let sigma = 0.20;
    assert!(
        (scatter - sigma).abs() <= Z_ALPHA * sigma / (2.0 * dof).sqrt(),
        "scatter {scatter} against the field's {sigma}"
    );
}

/// The halo's two main components, the in-situ halo (\[Fe/H\] −0.6) and the dominant merger
/// (−1.2), both with the field's 0.3 dex scatter, draw populations that separate in \[Fe/H\]: each
/// sample's mean is its component's, the difference of the means is the fields' 0.6 dex, and a
/// two-sample Kolmogorov–Smirnov test rejects a common distribution by far.
#[test]
fn the_halos_two_main_components_separate_in_iron() {
    let galaxy = milky_way();
    let n = 10_000_u32;
    let ids = candidate_ids(2 * n);
    let age = Years::new(1.2e10);
    let draw = |kind: HaloComponentKind, ids: &[SystemId]| {
        let c = component_where(&galaxy, |c| c.halo_component() == Some(kind));
        let field = galaxy
            .fields()
            .component(c)
            .metallicity(&PointLy::default(), age);
        let sample: Vec<f64> = ids
            .iter()
            .map(|&id| {
                let r = record(&galaxy, id, c, [0.0, 26_000.0, 3_000.0], age);
                draw_metallicity(&galaxy, &r).fe_h().value()
            })
            .collect();
        (field, sample)
    };
    let (split_a, split_b) = ids.split_at(ids.len() / 2);
    let (in_situ, mut a) = draw(HaloComponentKind::InSitu, split_a);
    let (merger, mut b) = draw(HaloComponentKind::DominantMerger, split_b);
    let nf = f64::from(n);
    for (name, field, sample) in [("in situ", in_situ, &a), ("dominant merger", merger, &b)] {
        let (mean, sd) = mean_and_sd(sample);
        let sigma = field.sigma().value();
        eprintln!(
            "{name}: [Fe/H] {mean:.4} ± {sd:.4} against the field's {:.2} ± {sigma:.2}",
            field.mean().value()
        );
        assert!(
            (mean - field.mean().value()).abs() <= Z_ALPHA * sigma / nf.sqrt(),
            "{name}: mean {mean}"
        );
        assert!(
            (sd - sigma).abs() <= Z_ALPHA * sigma / (2.0 * (nf - 1.0)).sqrt(),
            "{name}: scatter {sd}"
        );
    }
    let separation = mean_and_sd(&a).0 - mean_and_sd(&b).0;
    let expected = in_situ.mean().value() - merger.mean().value();
    let (sa, sb) = (in_situ.sigma().value(), merger.sigma().value());
    let error = (sa * sa / nf + sb * sb / nf).sqrt();
    eprintln!("separation {separation:.4} dex against the fields' {expected:.2} (± {error:.4})");
    assert!((expected - 0.6).abs() < 1e-12, "the fixture's means moved");
    assert!(
        (separation - expected).abs() <= Z_ALPHA * error,
        "separation {separation}"
    );
    let ks = ks_two_sample(&mut a, &mut b);
    eprintln!(
        "two-sample K–S: D = {:.4}, p = {:e}",
        ks.statistic, ks.p_value
    );
    assert!(
        ks.p_value < ALPHA,
        "the two components' abundances are indistinguishable: {ks:?}"
    );
}

/// At one place and age the drawn \[Fe/H\] of 10⁴ systems of one component follows the field's
/// normal distribution there (one-sample Kolmogorov–Smirnov at α = 10⁻³).
#[test]
fn the_draw_follows_the_fields_normal_at_one_place() {
    let galaxy = milky_way();
    let c = component_where(&galaxy, |c| c.population() == Population::OldThinDisc);
    let (place, age) = ([9_000.0, 21_000.0, -120.0], Years::new(3.2e9));
    let field = galaxy
        .fields()
        .component(c)
        .metallicity(&PointLy::new(place[0], place[1], place[2]), age);
    let mut sample: Vec<f64> = candidate_ids(10_000)
        .into_iter()
        .map(|id| {
            draw_metallicity(&galaxy, &record(&galaxy, id, c, place, age))
                .fe_h()
                .value()
        })
        .collect();
    let (mean, sigma) = (field.mean().value(), field.sigma().value());
    let ks = ks_one_sample(&mut sample, |x| normal_cdf((x - mean) / sigma));
    eprintln!(
        "[Fe/H] against N({mean:.4}, {sigma:.2}): D = {:.5}, p = {:.4}",
        ks.statistic, ks.p_value
    );
    assert_p_value("metallicity at one place", ks.p_value, ALPHA);
}

/// Writes a record's component, age, \[Fe/H\] and Z under its ID.
fn write_record(w: &mut GoldenWriter, galaxy: &Galaxy, r: &SystemRecord) {
    let label = format!("{:016x}", r.id().raw());
    let component = r.component().expect("a grid record");
    let composition = draw_metallicity(galaxy, r);
    w.u64_hex(
        &format!("{label}.component"),
        u64::try_from(component.index()).expect("a small index"),
    );
    w.f64(&format!("{label}.age"), r.age_at_epoch().value());
    w.f64(&format!("{label}.fe_h"), composition.fe_h().value());
    w.f64(&format!("{label}.z"), composition.z().value());
}

/// The first `n` records for which `pick` holds in the layer-E cells `cells`, in their order.
fn first_where(
    galaxy: &Galaxy,
    pick: impl Fn(&SystemRecord) -> bool,
    cells: impl IntoIterator<Item = [i32; 3]>,
    n: usize,
) -> Vec<SystemRecord> {
    let mut found = Vec::with_capacity(n);
    let mut cell = Vec::new();
    for index in cells {
        cell.clear();
        let key = CellKey::new(Layer::E, index).expect("inside the root cube");
        generate_cell(galaxy, key, &mut cell);
        found.extend(cell.iter().filter(|r| pick(r)).copied());
        if found.len() >= n {
            found.truncate(n);
            return found;
        }
    }
    panic!("fewer than {n} records found in the cells scanned");
}

/// Golden values for pinned systems: the first records of cells in each stellar layer, from the
/// solar circle to the galactic centre, then the first young-disc systems along the solar circle,
/// one not yet born, and the first halo systems above the Sun, each with its component,
/// age, \[Fe/H\] and Z.
///
/// A changed line means the stream, its key, the field or the composition's arithmetic moved,
/// which is a generator-version change.
#[test]
fn system_metallicities_are_pinned() {
    let galaxy = milky_way();
    let cells: [(Layer, [i32; 3]); 7] = [
        (Layer::C, [0, 812, 0]),
        (Layer::A, [3, 3_250, -2]),
        (Layer::B, [-40, 1_600, 1]),
        (Layer::D, [10, 150, 0]),
        (Layer::E, [2, 24, 0]),
        (Layer::E, [0, 0, 0]),
        (Layer::E, [0, 30, 30]),
    ];
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    let mut cell = Vec::new();
    for (layer, index) in cells {
        let key = CellKey::new(layer, index).expect("inside the root cube");
        cell.clear();
        generate_cell(&galaxy, key, &mut cell);
        assert!(!cell.is_empty(), "{layer:?} cell {index:?} is empty");
        for r in cell.iter().take(3) {
            write_record(&mut w, &galaxy, r);
        }
    }
    let solar_circle = || (-64..64).flat_map(|ix| (-2..2).map(move |iz| [ix, 203, iz]));
    let young = |r: &SystemRecord| r.population() == Population::YoungThinDisc;
    let mut pinned = first_where(&galaxy, young, solar_circle(), 3);
    // A system the young disc forms within the clock window after the epoch, 500 years from now:
    // one in some 10⁵ of the disc's, too rare to find by placement, so built from parts.
    let unborn = CellKey::new(Layer::A, [375, 3_250, 2])
        .map(|key| key.candidate_id(7))
        .expect("inside the root cube")
        .expect("inside the index field");
    let young_disc = pinned[0].component().expect("a grid record");
    pinned.push(record(
        &galaxy,
        unborn,
        young_disc,
        [3_000.0, 26_000.0, 20.0],
        Years::new(-500.0),
    ));
    pinned.extend(first_where(
        &galaxy,
        |r| r.population() == Population::Halo,
        (8..400).map(|iz| [0, 203, iz]),
        3,
    ));
    for r in &pinned {
        write_record(&mut w, &galaxy, r);
    }
    golden!("stellar/system_metallicity", w.as_str());
}
