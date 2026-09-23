//! Checks shared by the galaxy tests: every parameter inside its range, and the derived
//! quantities consistent with one another (plan 02, P02.T5.a and P02.T5.b).
//!
//! The ranges are written out here from plan 02's table, independently of the sim's own
//! constants, so that a wrong constant in the sim fails a test instead of moving the bracket.

use hyperion_sim::Seed;
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::fields::MAX_COMPONENTS;
use hyperion_sim::galaxy::imf::{MassBand, MassFunctionKind};
use hyperion_sim::galaxy::params::{
    ArmCount, GalaxyParams, HaloComponentKind, HaloComponentParams, ProgenitorKind,
};
use hyperion_sim::galaxy::placement::{CellCache, CellKey, NoCache, SystemRecord};
use hyperion_sim::galaxy::query::{PAD_SPEED, SystemHit, pad_for, position_at};
use hyperion_sim::galaxy::{Galaxy, POPULATIONS, PointLy, Population};
use hyperion_sim::id::Layer;
use hyperion_sim::math;
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::{Degrees, LightYears};

const GYR: f64 = 1e9;

/// Asserts `low ≤ value ≤ high`, naming the value.
#[track_caller]
pub fn assert_within(what: &str, value: f64, low: f64, high: f64) {
    assert!(
        (low..=high).contains(&value),
        "{what} = {value} lies outside [{low}, {high}]"
    );
}

/// Asserts `|actual − expected| ≤ tolerance × |expected|`.
#[track_caller]
pub fn assert_relative(what: &str, actual: f64, expected: f64, tolerance: f64) {
    let error = ((actual - expected) / expected).abs();
    assert!(
        error <= tolerance,
        "{what}: {actual} is {error:e} from {expected}, over {tolerance:e}"
    );
}

/// Every getter of `p` lies inside its range from plan 02's table. `max_sigmas` bounds the
/// normal scatters: 9 is the builder's own limit, a smaller count suits a small sample of seeds.
#[expect(
    clippy::too_many_lines,
    reason = "one assertion per parameter of the table, in its order"
)]
pub fn assert_params_in_ranges(p: &GalaxyParams, max_sigmas: f64) {
    let share = |pop| p.population_share(pop);
    assert_within("stellar mass", p.stellar_mass().value(), 3e10, 1e11);
    assert_within("thick share", share(Population::ThickDisc), 0.08, 0.14);
    let bulge_bar = share(Population::Bulge) + share(Population::LongBar);
    assert_within("bulge and bar share", bulge_bar, 0.20, 0.35);
    assert_within("bar of bulge", p.bar_share_of_bulge(), 0.30, 0.40);
    assert_relative(
        "bar share",
        share(Population::LongBar),
        bulge_bar * p.bar_share_of_bulge(),
        1e-12,
    );
    assert_within("nuclear share", share(Population::NuclearDisc), 0.01, 0.025);
    assert_within("halo share", share(Population::Halo), 0.007, 0.014);
    let thin = share(Population::YoungThinDisc) + share(Population::OldThinDisc);
    assert_within("thin share", thin, 0.47, 0.70);
    assert_within(
        "young part of the thin disc",
        share(Population::YoungThinDisc) / thin,
        0.0030,
        0.0056,
    );
    let total: f64 = POPULATIONS.iter().map(|&pop| share(pop)).sum();
    assert_relative("shares", total, 1.0, 1e-14);
    assert_within("timescale", p.sfh_timescale().value(), 5.0 * GYR, 9.0 * GYR);

    let thin_disc = p.thin_disc();
    assert_within("thin length", thin_disc.length().value(), 7_000.0, 11_500.0);
    assert_within(
        "thin mean height",
        thin_disc.height().value(),
        850.0,
        1_150.0,
    );
    assert_within(
        "young height",
        p.young_disc().height().value(),
        225.0,
        345.0,
    );
    assert_relative(
        "young length",
        p.young_disc().length().value(),
        thin_disc.length().value(),
        0.0,
    );
    assert_within(
        "thick length ratio",
        p.thick_disc().length() / thin_disc.length(),
        0.7,
        0.9,
    );
    assert_within(
        "thick height ratio",
        p.thick_disc().height() / thin_disc.height(),
        2.7,
        3.3,
    );

    let bulge = p.bulge();
    assert_within("bulge a", bulge.scale_x().value(), 1_700.0, 3_000.0);
    assert_within("bulge b ÷ a", bulge.scale_y() / bulge.scale_x(), 0.5, 0.7);
    assert_within("bulge c ÷ a", bulge.scale_z() / bulge.scale_x(), 0.3, 0.4);
    assert_within("bulge boxiness", bulge.boxiness(), 3.0, 4.0);

    let bar = p.bar();
    assert_within(
        "bar half-length",
        bar.half_length().value(),
        10_000.0,
        18_000.0,
    );
    assert_within(
        "bar width ratio",
        bar.width() / bar.half_length(),
        0.08,
        0.12,
    );
    assert_within("bar height", bar.height().value(), 500.0, 700.0);
    assert_within("bar corotation ratio", bar.corotation_ratio(), 1.0, 1.4);

    let nuclear = p.nuclear_disc();
    assert_within("nuclear length", nuclear.length().value(), 200.0, 400.0);
    assert_within(
        "nuclear height ratio",
        nuclear.height() / nuclear.length(),
        0.3,
        0.5,
    );
    let cluster_ratio =
        p.nuclear_cluster().mass().value() / p.population_mass(Population::NuclearDisc).value();
    let spread = math::exp10(max_sigmas * 0.2);
    assert_within(
        "nuclear cluster ratio",
        cluster_ratio,
        0.024 / spread,
        0.024 * spread,
    );

    let arms = p.arms();
    assert!(matches!(arms.count(), ArmCount::Two | ArmCount::Four));
    let degrees = Degrees::from(arms.pitch()).value();
    assert_within("pitch", degrees, 10.0 - 1e-12, 18.0 + 1e-12);
    assert_within("young arm width", arms.young_width().value(), 250.0, 500.0);
    assert_within("young arm fraction", arms.young_fraction(), 0.7, 0.9);
    assert_within("old arm amplitude", arms.old_amplitude(), 0.10, 0.30);

    let thin_mass = p.population_mass(Population::YoungThinDisc).value()
        + p.population_mass(Population::OldThinDisc).value();
    let gas = p.gas_disc();
    assert_within("gas fraction", gas.mass().value() / thin_mass, 0.175, 0.35);
    assert_within(
        "gas length ratio",
        gas.length() / thin_disc.length(),
        1.5,
        2.0,
    );
    assert_within("gas height", gas.height().value(), 700.0, 700.0);

    let dark = p.dark_halo();
    assert_within("f★", dark.f_star(), 0.12, 0.45);
    assert_relative(
        "M200",
        dark.m200().value(),
        p.stellar_mass().value() / (0.157 * dark.f_star()),
        1e-14,
    );
    assert_within("concentration", dark.concentration(), 2.0, 40.0);
    assert_within("r200", dark.r200().value(), 400_000.0, 1_400_000.0);
    // The fixture's one scatter is the Milky Way's measured offset from M–σ, 1.35 times the
    // relation's 0.38 dex at the estimator's 123.8 km/s (plan 02, Risks, R13 and R22; 1.1 times
    // before P02.T11 raised σ), so this bound never falls below 1.5 of it.
    let bh_sigmas = max_sigmas.max(1.5);
    assert_within(
        "black hole scatter",
        p.black_hole().scatter().value(),
        -bh_sigmas * 0.38,
        bh_sigmas * 0.38,
    );
    assert_within(
        "metallicity gradient",
        p.metallicity_gradient().value(),
        -0.07,
        -0.04,
    );

    assert_halo_in_ranges(p);
    assert_accretion_in_ranges(p);
}

fn assert_halo_in_ranges(p: &GalaxyParams) {
    let halo = p.halo();
    let components = halo.components();
    assert_within(
        "halo components",
        f64::from(u32::try_from(components.len()).unwrap()),
        5.0,
        8.0,
    );
    let total: f64 = components.iter().map(HaloComponentParams::share).sum();
    assert_relative("halo shares", total, 1.0, 1e-14);
    assert_eq!(components[0].kind(), HaloComponentKind::InSitu);
    assert_eq!(components[1].kind(), HaloComponentKind::DominantMerger);
    assert_eq!(
        components[components.len() - 1].kind(),
        HaloComponentKind::GlobularDebris
    );
    for (n, c) in (1_u8..).zip(&components[2..components.len() - 1]) {
        assert_eq!(c.kind(), HaloComponentKind::Lesser(n));
    }
    for c in components {
        let what = format!("{:?}", c.kind());
        let (slope, flattening, core, cut) = match c.kind() {
            HaloComponentKind::InSitu => ((2.2, 2.8), (0.45, 0.55), (1_500.0, 3_000.0), 50_000.0),
            HaloComponentKind::DominantMerger => {
                ((2.2, 2.8), (0.6, 0.8), (2_000.0, 5_000.0), 65_000.0)
            }
            HaloComponentKind::Lesser(_) => ((2.2, 2.8), (0.6, 1.0), (3_000.0, 3_000.0), 65_000.0),
            HaloComponentKind::GlobularDebris => {
                ((4.0, 4.5), (1.0, 1.0), (3_000.0, 5_000.0), 65_000.0)
            }
        };
        assert_within(&what, c.slope(), slope.0, slope.1);
        assert_within(&what, c.flattening(), flattening.0, flattening.1);
        assert_within(&what, c.core().value(), core.0, core.1);
        assert_within(&what, c.cut_radius().value(), cut, cut);
        assert_within(&what, c.ages().min().value(), 10.0 * GYR, 12.0 * GYR);
        // Stars formed before the event that put them in the halo: the last major merger heated
        // the in-situ component and quenched the dominant one.
        let merger = p.accretion().last_major_merger().value();
        match c.kind() {
            HaloComponentKind::InSitu | HaloComponentKind::DominantMerger => {
                assert!(
                    c.ages().min().value() >= merger,
                    "{what}: younger than the merger"
                );
            }
            HaloComponentKind::Lesser(_) | HaloComponentKind::GlobularDebris => {}
        }
        assert_within(&what, c.ages().max().value(), 11.0 * GYR, 13.0 * GYR);
        assert_relative(&what, (c.ages().max() - c.ages().min()).value(), GYR, 1e-12);
        assert_within(&what, c.feh_mean().value(), -2.0, -0.6);
        match c.outer_break() {
            Some(b) => {
                assert_eq!(c.kind(), HaloComponentKind::DominantMerger);
                assert_within("break radius", b.radius().value(), 52_000.0, 91_000.0);
                assert_within("break steepening", b.steepening(), 1.5, 2.5);
            }
            None => assert_ne!(c.kind(), HaloComponentKind::DominantMerger),
        }
    }
    assert_within("discrete share", halo.discrete_share(), 0.02, 0.15);
}

fn assert_accretion_in_ranges(p: &GalaxyParams) {
    let accretion = p.accretion();
    let merger = accretion.last_major_merger().value();
    assert_within("last major merger", merger, 6.0 * GYR, 11.0 * GYR);
    let progenitors = accretion.progenitors();
    assert_eq!(progenitors[0].kind(), ProgenitorKind::DominantMerger);
    assert_relative(
        "dominant accreted",
        progenitors[0].accreted().value(),
        merger,
        0.0,
    );
    let lesser = p.halo().components().len() - 3;
    let halo_mass = p.population_mass(Population::Halo).value();
    for (i, progenitor) in progenitors.iter().enumerate() {
        let what = format!("{:?}", progenitor.kind());
        let accreted = progenitor.accreted().value();
        match progenitor.kind() {
            ProgenitorKind::DominantMerger => assert_eq!(i, 0),
            ProgenitorKind::Lesser(n) => {
                assert_eq!(usize::from(n), i);
                assert_within(&what, accreted, 6.0 * GYR, 12.0 * GYR);
                assert!(progenitor.mass().value() < halo_mass);
                // Its component's youngest stars predate its accretion.
                let component = &p.halo().components()[i + 1];
                assert_eq!(component.kind(), HaloComponentKind::Lesser(n));
                assert!(
                    component.ages().min().value() >= accreted,
                    "{what}: stars younger than the accretion"
                );
            }
            ProgenitorKind::Recent(j) => {
                assert_eq!(usize::try_from(j).unwrap() + lesser + 1, i);
                assert_within(&what, accreted, 0.0, 6.0 * GYR);
                assert_within(&what, progenitor.mass().value(), 1e5, 3.162_277_660_168_4e9);
            }
        }
        let orbit = progenitor.orbit();
        let apocentre = orbit.apocentre().value();
        let pericentre = orbit.pericentre().value();
        assert_within(&what, apocentre, 20_000.0, 200_000.0);
        if progenitor.kind() == ProgenitorKind::DominantMerger {
            // Radial, like the Gaia Sausage (Belokurov et al. 2018).
            let e = (apocentre - pericentre) / (apocentre + pericentre);
            assert_within("dominant eccentricity", e, 0.85 - 1e-12, 0.95 + 1e-12);
        } else {
            assert_within(&what, pericentre / apocentre, 0.05 - 1e-12, 0.6 + 1e-12);
        }
        assert_within(
            &what,
            orbit.inclination().value(),
            0.0,
            core::f64::consts::PI,
        );
        let two_pi = 2.0 * core::f64::consts::PI;
        assert_within(&what, orbit.node().value(), 0.0, two_pi);
        assert_within(&what, orbit.phase().value(), 0.0, two_pi);
    }
    let dominant = progenitors[0].mass().value();
    assert!(dominant > 0.0 && dominant < halo_mass);
    let count = accretion.globular_count();
    assert!((80..=800).contains(&count), "globular count {count}");
}

/// The derived quantities agree with one another (P02.T5.b).
pub fn assert_derived_consistent(p: &GalaxyParams) {
    let stellar = p.stellar_mass().value();
    let masses: f64 = POPULATIONS
        .iter()
        .map(|&pop| p.population_mass(pop).value())
        .sum();
    assert_relative("population masses", masses, stellar, 1e-12);
    let per_system: f64 = POPULATIONS
        .iter()
        .map(|&pop| p.population_share(pop) * p.mean_system_mass(pop).value())
        .sum();
    assert_relative(
        "system count",
        p.system_count(),
        stellar / per_system,
        1e-12,
    );
    // M★'s 3–10 × 10¹⁰ M☉ over the default's 0.55–0.58 M☉ per system (brainstorm, "Galaxy
    // parameters": 0.5–1.8 × 10¹¹); Kroupa's lighter systems reach 2.1 × 10¹¹.
    let most = match p.mass_function() {
        MassFunctionKind::Chabrier => 1.8e11,
        MassFunctionKind::Kroupa => 2.1e11,
    };
    assert_within("system count", p.system_count(), 0.5e11, most);
    let m200 = p.dark_halo().m200().value();
    // Plan 02's "1.4–5.3 × 10¹²" is this range rounded: 10¹¹ M☉ ÷ (0.157 f★) at the ends of f★'s
    // range, 0.45 and 0.12, is 1.415 and 5.308 × 10¹², so the exact ends are checked.
    let per_1e11 = |f_star: f64| 1e11 / (0.157 * f_star);
    assert_within(
        "M200 per 10¹¹ M☉ of stars",
        m200 / (stellar / 1e11),
        per_1e11(0.45) * (1.0 - 1e-12),
        per_1e11(0.12) * (1.0 + 1e-12),
    );
    let formed = p.mean_formed_mass().value();
    for pop in POPULATIONS {
        let mean = p.mean_system_mass(pop).value();
        assert!(mean < formed, "{pop:?}: {mean} ≥ {formed}");
        // Under the default the old populations hold 0.544–0.576 M☉ per system over 10⁴ seeds,
        // around the brainstorm's 0.55–0.59, and the young disc 35–45% more than the old thin disc;
        // under Kroupa's function 0.47–0.51, around its 0.48.
        let (young, old) = match p.mass_function() {
            MassFunctionKind::Chabrier => ((0.77, 0.83), (0.54, 0.58)),
            MassFunctionKind::Kroupa => ((0.65, 0.75), (0.45, 0.52)),
        };
        match pop {
            Population::YoungThinDisc => assert_within("young mean mass", mean, young.0, young.1),
            _ => assert_within(pop.name(), mean, old.0, old.1),
        }
    }
}

// --- Placement and the range query (plan 03) ---

/// A point like the Sun's: in the plane, 26,000 ly from the centre on the +y axis, well clear of the
/// bar, whose half-length reaches at most 18,000 ly.
///
/// It is where the brainstorm's reference density applies, so a 50 ly query here is the one whose
/// figures the plan quotes.
#[must_use]
pub fn sunlike_point(galaxy: &Galaxy) -> GalacticPosition {
    let bar = galaxy.params().bar().half_length().value();
    assert!(bar < 26_000.0, "the bar reaches {bar} ly, past the Sun");
    GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).expect("26,000 ly is in the root cube")
}

/// The whole light-years of a non-negative length, rounded up.
#[must_use]
fn whole_ly_up(ly: f64) -> i64 {
    assert!(
        ly.is_finite() && (0.0..=1.0e9).contains(&ly),
        "{ly} ly is not a length a test searches over"
    );
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the assertion above holds the value in [0, 10⁹], where every whole number is \
                  exact in both f64 and i64"
    )]
    let whole = ly.ceil() as i64;
    whole
}

/// Calls `sample(point, weight, [ix, iy, iz])` at the midpoint of each of `steps³` equal boxes of
/// the box of whole light-years from `min_ly` with edge `size_ly`, so that `Σ weight × f(point)` is
/// `∫ f dV` over it.
///
/// The step indices let a caller integrate over part of the box — the slab beside a cell face, say —
/// in the same pass.
pub fn for_each_box_midpoint(
    min_ly: [i32; 3],
    size_ly: u32,
    steps: u32,
    mut sample: impl FnMut(&PointLy, f64, [u32; 3]),
) {
    assert!(steps > 0, "a midpoint sum needs a step");
    let [x0, y0, z0] = min_ly.map(f64::from);
    let h = f64::from(size_ly) / f64::from(steps);
    let weight = h * h * h;
    let at = |base: f64, i: u32| base + (f64::from(i) + 0.5) * h;
    for ix in 0..steps {
        for iy in 0..steps {
            for iz in 0..steps {
                let point = PointLy::new(at(x0, ix), at(y0, iy), at(z0, iz));
                sample(&point, weight, [ix, iy, iz]);
            }
        }
    }
}

/// The integral of a layer's density over the box of whole light-years from `min_ly` with edge
/// `size_ly`, by a midpoint sum of `steps³` points: the expected number of systems the grid places
/// there (plan 03, P03.T8.a).
///
/// The sum shares no point with the thinning and none with `expected_counts`'s Gauss–Legendre rule.
/// Its relative error is about `(h ÷ L)² ÷ 24` for a density of scale `L`, with `h = size ÷ steps`,
/// so a step of an eighth of a cell is well under a per cent everywhere the layers reach.
#[must_use]
pub fn reference_box_integral(
    galaxy: &Galaxy,
    band: MassBand,
    min_ly: [i32; 3],
    size_ly: u32,
    steps: u32,
) -> f64 {
    let (fields, shares) = (galaxy.fields(), galaxy.shares());
    let mut total = 0.0;
    for_each_box_midpoint(min_ly, size_ly, steps, |point, weight, _| {
        total += weight * fields.layer_density(shares, band, point);
    });
    total
}

/// Each density component's integral over the box of whole light-years from `min_ly` with edge
/// `size_ly`, by the same midpoint sum as [`reference_box_integral`]: the odds the thinning picks a
/// component with, once weighted by the band's share (plan 03, P03.T8.b).
#[must_use]
pub fn reference_box_component_integrals(
    galaxy: &Galaxy,
    min_ly: [i32; 3],
    size_ly: u32,
    steps: u32,
) -> [f64; MAX_COMPONENTS] {
    let fields = galaxy.fields();
    let mut integrals = [0.0; MAX_COMPONENTS];
    let mut densities = [0.0; MAX_COMPONENTS];
    for_each_box_midpoint(min_ly, size_ly, steps, |point, weight, _| {
        fields.densities(point, &mut densities);
        for (integral, density) in integrals.iter_mut().zip(densities) {
            *integral += weight * density;
        }
    });
    integrals
}

/// Every system of the five stellar layers within `radius` of `centre` at `t`, by generating every
/// cell that can hold one and testing each system: the range query's answer, found without a census,
/// a sphere or a walk (plan 03, P03.T10).
///
/// Cells are enumerated over the bounding box of the query's reach on each layer's grid, clipped to
/// the root cube, so the region searched strictly contains the one `cells_in_sphere` walks: a walk
/// that misses a cell shows up as a system missing from the query. Every cell is generated through
/// `NoCache`, in whole, as a cache would hold it. The hits are ordered as the query orders them, by
/// distance at `t` and then by ID.
#[must_use]
pub fn brute_force_in_sphere(
    galaxy: &Galaxy,
    centre: &GalacticPosition,
    radius: LightYears,
    t: UniverseTime,
) -> Vec<SystemHit> {
    // The query pads its walk for motion; two light-years more here keeps this search wider than
    // the query's however the pad is rounded.
    let reach = whole_ly_up(radius.value() + pad_for(t, PAD_SPEED).value() + 2.0);
    let centre_ly = centre.cell().to_array().map(i64::from);
    let mut cache = NoCache::new();
    let mut hits = Vec::new();
    for layer in [Layer::E, Layer::D, Layer::C, Layer::B, Layer::A] {
        let cell_ly = i64::from(layer.cell_size_ly());
        let half = 65_536 / cell_ly;
        let ends = centre_ly.map(|c| {
            let first = (c - reach).div_euclid(cell_ly).max(-half);
            let last = (c + reach).div_euclid(cell_ly).min(half - 1);
            (first, last)
        });
        for x in ends[0].0..=ends[0].1 {
            for y in ends[1].0..=ends[1].1 {
                for z in ends[2].0..=ends[2].1 {
                    let cell = [x, y, z].map(|c| i32::try_from(c).expect("a cell of the cube"));
                    let key = CellKey::new(layer, cell).expect("a stellar layer inside the cube");
                    cache.with_cell(galaxy, key, |systems| {
                        collect_hits(galaxy, systems, centre, radius, t, &mut hits);
                    });
                }
            }
        }
    }
    hits.sort_by(|a, b| {
        a.distance()
            .total_cmp(&b.distance())
            .then_with(|| a.id().raw().cmp(&b.id().raw()))
    });
    hits
}

/// The systems of one cell that are inside the sphere at `t`, appended to `hits`.
fn collect_hits(
    galaxy: &Galaxy,
    systems: &[SystemRecord],
    centre: &GalacticPosition,
    radius: LightYears,
    t: UniverseTime,
    hits: &mut Vec<SystemHit>,
) {
    for record in systems {
        if record.age_at(t).value() <= 0.0 {
            continue;
        }
        let position = position_at(galaxy, record, t);
        let distance = LightYears::from(centre.distance_to(&position));
        if distance.value() <= radius.value() {
            hits.push(SystemHit::new(*record, position, distance));
        }
    }
}

/// The integral of a layer's density over a sphere, by a midpoint sum in height, radius and azimuth:
/// an independent reference for `expected_counts`, whose Gauss–Legendre rule it shares no node with.
///
/// `steps` is the number of midpoints in height; the radius takes half as many and the azimuth a
/// quarter, so the cost is `steps³ ÷ 8`. The sum covers the sphere exactly, with no boundary error,
/// and its own error falls as `steps⁻²`.
#[must_use]
pub fn reference_sphere_integral(
    galaxy: &Galaxy,
    band: MassBand,
    centre: &GalacticPosition,
    radius: LightYears,
    steps: u32,
) -> f64 {
    let (fields, shares) = (galaxy.fields(), galaxy.shares());
    let [x0, y0, z0] = centre.to_light_years_f64();
    let r = radius.value();
    let (nz, nr, na) = (steps, (steps / 2).max(1), (steps / 4).max(4));
    let dz = 2.0 * r / f64::from(nz);
    let da = std::f64::consts::TAU / f64::from(na);
    let mut total = 0.0;
    for iz in 0..nz {
        let z = z0 - r + (f64::from(iz) + 0.5) * dz;
        let rho_sq = r * r - (z - z0) * (z - z0);
        if rho_sq <= 0.0 {
            continue;
        }
        let rho = rho_sq.sqrt();
        let dr = rho / f64::from(nr);
        for ir in 0..nr {
            let radius_here = (f64::from(ir) + 0.5) * dr;
            let weight = dz * dr * radius_here * da;
            for ia in 0..na {
                let theta = (f64::from(ia) + 0.5) * da;
                let (sin, cos) = math::sin_cos(theta);
                let point = PointLy::new(x0 + radius_here * cos, y0 + radius_here * sin, z);
                total += weight * fields.layer_density(shares, band, &point);
            }
        }
    }
    total
}

// --- The gas and dust field (plan 07) ---

/// The mean of a function of the seed over an ensemble of seeds, with its standard error.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnsembleMean {
    /// The sample mean.
    pub mean: f64,
    /// The sample's own standard error: its standard deviation over the square root of its size.
    pub standard_error: f64,
    /// The sample's variance, with the `n − 1` denominator.
    pub variance: f64,
    /// The number of seeds.
    pub count: u64,
}

/// A running mean and variance by Welford's update, so that a sample of millions keeps its digits.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Running {
    count: u64,
    mean: f64,
    squares: f64,
}

impl Running {
    /// Adds one value.
    pub fn push(&mut self, value: f64) {
        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / count_as_f64(self.count);
        self.squares += delta * (value - self.mean);
    }

    /// The mean and standard error so far.
    ///
    /// # Panics
    ///
    /// With fewer than two values, which have no variance.
    #[must_use]
    pub fn summary(&self) -> EnsembleMean {
        assert!(
            self.count >= 2,
            "a sample of {} has no variance",
            self.count
        );
        let variance = self.squares / count_as_f64(self.count - 1);
        EnsembleMean {
            mean: self.mean,
            standard_error: (variance / count_as_f64(self.count)).sqrt(),
            variance,
            count: self.count,
        }
    }
}

/// `f` averaged over `seeds`, with a standard error (plan 07's `ensemble_mean`), by [`Running`].
///
/// A heavy-tailed quantity, such as the gas's log-normal factor, has a sample standard error that
/// is itself unreliable; a caller that knows the population's variance should take the error from
/// that instead.
///
/// # Panics
///
/// If `seeds` holds fewer than two seeds, which have no variance.
pub fn ensemble_mean(
    mut f: impl FnMut(Seed) -> f64,
    seeds: impl IntoIterator<Item = Seed>,
) -> EnsembleMean {
    let mut running = Running::default();
    for seed in seeds {
        running.push(f(seed));
    }
    running.summary()
}

/// A whole number of light-years inside the root cube as an `i32`.
///
/// # Panics
///
/// If `ly` is not within 10⁻⁹ of a whole number inside ±65,536.
#[must_use]
pub fn whole_ly(ly: f64) -> i32 {
    let whole = ly.round();
    assert!(
        (-65_536.0..=65_536.0).contains(&whole) && (ly - whole).abs() < 1e-9,
        "{ly} ly is not a whole light-year of the root cube"
    );
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a whole number inside ±65,536, which the assertion holds"
    )]
    let whole = whole as i32;
    whole
}

/// A count as an `f64`, exactly: every count a test reaches is far below 2⁵³.
#[must_use]
fn count_as_f64(count: u64) -> f64 {
    let exact = u32::try_from(count).expect("an ensemble of fewer than 2³² seeds");
    f64::from(exact)
}
