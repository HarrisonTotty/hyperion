//! The density fields (plan 02, P02.T7): discs and arms, the sub-discs, the bulge and the bar, the
//! halo's mixture, metallicity, the assembly and the golden file.
//!
//! The checks over many seeds run here on 32 seeds, and over 10³ in `galaxy_sweeps.rs` under
//! `just test-slow` (plan 02, P02.T11).

#[expect(dead_code, reason = "the field tests use only the shared assertions")]
mod common;

use common::{assert_relative, assert_within};
use hyperion_sim::galaxy::consts::{LIGHT_YEARS_PER_KILOPARSEC, LIGHT_YEARS_PER_PARSEC};
use hyperion_sim::galaxy::fields::arms::Arm;
use hyperion_sim::galaxy::fields::disc::ExponentialDisc;
use hyperion_sim::galaxy::fields::{Component, ComponentId, Fields, MAX_COMPONENTS, Shape};
use hyperion_sim::galaxy::imf::{BandShares, MassBand, MassFunctionKind};
use hyperion_sim::galaxy::params::{
    ArmCount, GalaxyParams, GalaxyParamsBuilder, HaloComponentKind, HaloComponentParams,
};
use hyperion_sim::galaxy::potential::MassModel;
use hyperion_sim::galaxy::quad::{gl_panels, gl16};
use hyperion_sim::galaxy::shares::ShareMatrix;
use hyperion_sim::galaxy::{POPULATIONS, PointLy, Population};
use hyperion_sim::math;
use hyperion_sim::units::{Degrees, KilometresPerSecond, LightYears, Years};
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::lcg::Lcg;
use hyperion_testkit::order::assert_order_independent;

const PI: f64 = core::f64::consts::PI;

/// The three pinned seeds of the golden files, as the potential's.
const PINNED: [u64; 3] = [
    0x0000_0000_0000_0001,
    0x5eed_0000_c0ff_ee00,
    0xdead_beef_cafe_f00d,
];

/// The seeds of the fast checks.
fn fast_seeds() -> impl Iterator<Item = Seed> {
    (0..32_u64).map(|n| Seed::new(0x0207_5eed_0000_0000 | n))
}

fn fields_of(params: &GalaxyParams) -> Fields {
    Fields::new(params, &MassModel::new(params))
}

fn fixture() -> (GalaxyParams, Fields) {
    let params = GalaxyParams::milky_way_like();
    let fields = fields_of(&params);
    (params, fields)
}

fn seeded(seed: Seed) -> (GalaxyParams, Fields) {
    let params = GalaxyParams::from_seed(seed, MassFunctionKind::default());
    let fields = fields_of(&params);
    (params, fields)
}

fn component(fields: &Fields, population: Population) -> &Component {
    fields
        .components()
        .iter()
        .find(|c| c.population() == population)
        .expect("every population has a component")
}

/// `∫ f` over consecutive `edges` by the 16-point rule on each panel, summed in order.
fn gl16_panels(mut f: impl FnMut(f64) -> f64, edges: &[f64]) -> f64 {
    edges
        .windows(2)
        .fold(0.0, |sum, panel| sum + gl16(&mut f, panel[0], panel[1]))
}

fn disc_of(c: &Component) -> &ExponentialDisc {
    match c.shape() {
        Shape::Disc(disc) => disc,
        other => panic!("not a disc: {other:?}"),
    }
}

/// The sample correlation of the pairs' coordinates.
fn correlation(points: &[(f64, f64)]) -> f64 {
    let n = f64::from(u32::try_from(points.len()).unwrap());
    let mean_x = points.iter().fold(0.0, |s, p| s + p.0) / n;
    let mean_y = points.iter().fold(0.0, |s, p| s + p.1) / n;
    let (sxy, sxx, syy) = points.iter().fold((0.0, 0.0, 0.0), |(xy, xx, yy), p| {
        let (dx, dy) = (p.0 - mean_x, p.1 - mean_y);
        (xy + dx * dy, xx + dx * dx, yy + dy * dy)
    });
    sxy / (sxx * syy).sqrt()
}

// P02.T7.a: discs and arms.

/// Each disc integrates to its count to 0.1% (P02.T7.a): brute force in the plane over the circle
/// that the root cube holds, arm factor and all, times the density's own vertical integral by
/// brute force, plus the analytic tail beyond the circle, where the arms average 1 and the column
/// is twice the effective height.
#[test]
fn each_disc_integrates_to_its_count() {
    let (_, fields) = fixture();
    let edge = f64::from(hyperion_sim::coords::ROOT_HALF_WIDTH_LY);
    let bar_end = fields.arms().bar_half_length().value();
    for c in fields.components() {
        let Shape::Disc(disc) = c.shape() else {
            continue;
        };
        let (l, h) = (disc.length().value(), disc.height().value());
        let azimuths = 1_024_u32;
        let ring = |r: f64| {
            let sum = (0..azimuths).fold(0.0, |sum, j| {
                let theta = 2.0 * PI * f64::from(j) / f64::from(azimuths);
                sum + c.density(&PointLy::new(
                    r * math::cos(theta),
                    r * math::sin(theta),
                    0.0,
                ))
            });
            sum / f64::from(azimuths) * 2.0 * PI * r
        };
        let mut edges: Vec<f64> = [0.0, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0]
            .iter()
            .map(|f| f * l)
            .chain([0.9 * bar_end, 1.1 * bar_end, 2.0 * bar_end, edge])
            .filter(|&e| e <= edge)
            .collect();
        edges.sort_by(f64::total_cmp);
        edges.dedup();
        let plane = gl_panels(ring, &edges);
        let at = PointLy::new(l * math::cos(0.3), l * math::sin(0.3), 0.0);
        let column =
            2.0 * gl_panels(
                |z| c.density(&PointLy::new(at.x, at.y, z)),
                &[0.0, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0].map(|f| f * h),
            ) / c.density(&at);
        let x = edge / l;
        let tail = disc.n0() * 2.0 * h * 2.0 * PI * l * l * (1.0 + x) * math::exp(-x);
        let total = plane * column + tail;
        assert_relative(
            &format!("{:?} {:?}", c.population(), c.sub_disc()),
            total,
            c.count_with_unborn(),
            1e-3,
        );
        assert_relative("disc count", disc.count(), c.count_with_unborn(), 1e-14);
    }
}

/// Each ridge leaves the bar's end: with two arms both ridges lie on the x axis at `R = L_bar`,
/// where the young disc's density peaks in azimuth; the ridges trail.
#[test]
fn the_young_disc_follows_its_ridges() {
    let params = GalaxyParamsBuilder::new()
        .arm_count(ArmCount::Two)
        .arm_pitch(Degrees::new(14.0))
        .build()
        .unwrap();
    let fields = fields_of(&params);
    let young = &fields.components()[0];
    assert!(matches!(young.arm(), Some(Arm::Sharp(_))));
    let l = params.bar().half_length().value();
    for x in [l, -l] {
        let on = young.density(&PointLy::new(x, 0.0, 0.0));
        for delta in [0.01, -0.01, 0.05, -0.05] {
            let off = young.density(&PointLy::new(
                x * math::cos(delta),
                l * math::sin(delta),
                0.0,
            ));
            assert!(on > off, "ridge at ({x}, 0) against {delta} rad");
        }
    }
    let g = fields.arms();
    let mut previous = g.ridge_azimuth(l, 0);
    for i in 1..=40 {
        let r = l * (1.0 + 0.05 * f64::from(i));
        let theta = g.ridge_azimuth(r, 0);
        assert!(theta < previous, "the ridge trails at {r}");
        previous = theta;
    }
}

// P02.T7.b: the old thin disc's sub-discs and every disc's cored profile.

/// The brainstorm's sub-disc heights, "from about 320 ly at half a gigayear to 1,700 ly at ten"
/// (brainstorm, "Orbits and time"), taken as a power of age between those two and read at a mean
/// age, Gyr.
fn brainstorm_height(age_gyr: f64) -> f64 {
    let p = math::ln(1_700.0 / 320.0) / math::ln(10.0 / 0.5);
    320.0 * math::exp(p * math::ln(age_gyr / 0.5))
}

/// The fixture's sub-discs (P02.T7.b): effective heights that rise with age, near the brainstorm's
/// at each sub-disc's mean age, whose harmonic mean is the drawn height, under Sharma et al.'s
/// heating law times one dispersion scale.
///
/// The brainstorm's heights, read at the youngest and oldest bins' mean ages of 0.56 and 8.6 Gyr,
/// are 340 and 1,560 ly; the fixture's are 371 and 1,373, each within a quarter of them. The
/// dispersion scale is 1.02: the measured heating law in the model's potential gives the drawn
/// height almost exactly, as the density rulings' research found (brainstorm, Decisions,
/// "2026-09-21: local density rulings", 1).
#[test]
fn the_fixture_s_sub_discs_follow_the_heating_law() {
    let (params, fields) = fixture();
    let sub = fields.sub_disc_heights();
    let unscaled = sub.unscaled().map(LightYears::value);
    let heights = sub.heights().map(LightYears::value);
    let ages = sub.mean_ages().map(|a| a.value() / 1e9);
    eprintln!(
        "effective heights {heights:?} (unscaled {unscaled:?}), dispersion scale {}",
        sub.scale()
    );
    for (i, bracket) in [(0, "youngest"), (4, "oldest")] {
        let expected = brainstorm_height(ages[i]);
        assert_within(
            &format!("{bracket} effective height"),
            heights[i],
            0.75 * expected,
            1.25 * expected,
        );
    }
    assert!(
        heights.windows(2).all(|w| w[0] < w[1]),
        "heights rise with age"
    );
    assert!(unscaled.windows(2).all(|w| w[0] < w[1]));
    assert_relative(
        "harmonic mean height",
        sub.mean_height().value(),
        params.thin_disc().height().value(),
        1e-9,
    );
    assert_within("dispersion scale", sub.scale(), 0.6, 1.6);
    assert!(ages.windows(2).all(|w| w[0] < w[1]));
    assert_within("youngest age", ages[0], 0.1, 1.0);
    assert_within("oldest age", ages[4], 7.0, 10.0);
    let shares: f64 = sub.shares().iter().sum();
    assert_relative("sub-disc shares", shares, 1.0, 1e-15);
    // The heating law at the mean ages (Sharma et al. 2021): 6–21 km/s, and the profiles' own
    // dispersions that law times the scale.
    let sigmas = sub.dispersions().map(KilometresPerSecond::value);
    assert_within("youngest dispersion", sigmas[0], 5.0, 8.0);
    assert_within("oldest dispersion", sigmas[4], 18.0, 21.1);
    // The components carry the profiles and the shares.
    let old = params.system_count() * params.population_share(Population::OldThinDisc);
    for (i, c) in fields.components()[1..6].iter().enumerate() {
        assert_eq!(c.population(), Population::OldThinDisc);
        let disc = disc_of(c);
        assert_same_bits(disc.height().value(), sub.heights()[i].value());
        assert_relative(
            "sub-disc dispersion",
            disc.profile().dispersion().value(),
            sub.scaled_dispersions()[i].value(),
            1e-15,
        );
        assert_relative("sub-disc count", c.count(), old * sub.shares()[i], 1e-15);
        assert!(matches!(c.arm(), Some(Arm::Gentle(_))));
        let [lo, hi] = c.sub_disc().unwrap().age_range();
        assert_same_bits(c.ages().min().value(), lo.value());
        assert_same_bits(c.ages().max().value(), hi.value());
    }
}

/// Every disc is cored (brainstorm, "Fields"): flat at the plane, where the vertical force
/// vanishes, as measured profiles are (Bovy 2017, MNRAS 470, 1360), each with the effective height
/// drawn for it. An exponential in height would fall by `z ÷ h` at once; each disc here has lost
/// under 0.05% of its mid-plane density at a thousandth of its effective height up, and its
/// logarithmic slope steepens with height all through the disc.
#[test]
fn every_disc_is_cored_with_its_drawn_effective_height() {
    let (params, fields) = fixture();
    let drawn = [
        (0, params.young_disc().height().value()),
        (6, params.thick_disc().height().value()),
        (9, params.nuclear_disc().height().value()),
    ];
    for (i, h) in drawn {
        assert_relative(
            &format!("component {i}'s effective height"),
            disc_of(&fields.components()[i]).height().value(),
            h,
            1e-12,
        );
    }
    for c in fields.components() {
        let Shape::Disc(disc) = c.shape() else {
            continue;
        };
        let profile = disc.profile();
        let h = profile.effective_height().value();
        assert!(
            profile.value(1e-3 * h) > 0.999_5,
            "{:?}: a cusp at the plane",
            c.population()
        );
        // The local scale height, −1 ÷ (d ln n ÷ dz), shrinks from the plane up to a height.
        let local = |z: f64| {
            let dz = 0.02 * z;
            -2.0 * dz / (math::ln(profile.value(z + dz)) - math::ln(profile.value(z - dz)))
        };
        let mut previous = f64::INFINITY;
        for t in [0.05, 0.1, 0.2, 0.4, 0.8] {
            let here = local(t * h);
            assert!(here < previous, "{:?} at {t} h", c.population());
            previous = here;
        }
    }
}

/// The sub-discs together have, in the mid-plane, exactly the density of one disc of the old thin
/// disc's count and the drawn effective height: the disc the potential holds (plan 02, Design
/// note 6) and the one the brainstorm's in-plane density is worked for.
#[test]
fn the_sub_discs_have_the_mid_plane_density_of_the_drawn_height() {
    for (params, fields) in [fixture(), seeded(Seed::new(PINNED[2]))] {
        let old = params.system_count() * params.population_share(Population::OldThinDisc);
        let (l, h) = (
            params.thin_disc().length().value(),
            params.thin_disc().height().value(),
        );
        let n0 = old / (4.0 * PI * l * l * h);
        for r in [0.0, 5_000.0, 26_000.0, 60_000.0] {
            let sum = fields.components()[1..6].iter().fold(0.0, |sum, c| {
                sum + c.envelope(&PointLy::new(0.6 * r, 0.8 * r, 0.0))
            });
            assert_relative(
                &format!("mid-plane at {r} ly"),
                sum,
                n0 * math::exp(-r / l),
                1e-12,
            );
        }
    }
}

/// Over 32 seeds the heights rise with age, their harmonic mean is the drawn one, the dispersion
/// scale is found inside its bracket, and it is what the physics says it is: it grows as the
/// square root of the disc's column at `R_ref` times the drawn height, which the drawn height
/// does not follow (plan 02, Risks, R2). All 32 scales lie in 0.6–1.6; 10³ seeds are in
/// `galaxy_sweeps.rs`.
#[test]
fn sub_discs_over_32_seeds() {
    let mut points = Vec::new();
    for seed in fast_seeds() {
        let params = GalaxyParams::from_seed(seed, MassFunctionKind::default());
        let model = MassModel::new(&params);
        let fields = Fields::new(&params, &model);
        let sub = fields.sub_disc_heights();
        let h = sub.heights().map(LightYears::value);
        assert!(h.windows(2).all(|w| w[0] < w[1]), "{seed}: {h:?}");
        assert_relative(
            &format!("{seed} mean height"),
            sub.mean_height().value(),
            params.thin_disc().height().value(),
            1e-9,
        );
        assert_within(&format!("{seed} dispersion scale"), sub.scale(), 0.6, 1.6);
        let column = model.vertical_force(
            sub.reference_radius(),
            LightYears::new(LIGHT_YEARS_PER_KILOPARSEC),
        );
        let drawn = params.thin_disc().height().value();
        points.push((math::ln(column * drawn), math::ln(sub.scale())));
    }
    let correlation = correlation(&points);
    assert!(
        correlation > 0.95,
        "the scale follows the column times the drawn height: r = {correlation}"
    );
}

/// A double exponential fitted to `ln_n` at heights `z` (pc): `a (e^(−z ÷ h₁) + f e^(−z ÷ h₂))`
/// with `h₂ > 1.3 h₁`, least squares in `ln n`, in the manner of Jurić et al.'s (2008, ApJ 673,
/// 864) fits that Bland-Hawthorn and Gerhard (2016) summarise. A grid over h₁ in 150–600 pc, h₂ in
/// 500–2,000 pc and f in 0.5–40%, then a compass search from its best point. Returns (h₁, h₂, f).
fn fit_double_exponential(z: &[f64], ln_n: &[f64]) -> (f64, f64, f64) {
    let n = f64::from(u32::try_from(z.len()).unwrap());
    let cost = |h1: f64, h2: f64, f: f64| {
        let shape: Vec<f64> = z
            .iter()
            .map(|z| math::ln(math::exp(-z / h1) + f * math::exp(-z / h2)))
            .collect();
        let offset = ln_n.iter().zip(&shape).fold(0.0, |s, (y, m)| s + y - m) / n;
        ln_n.iter()
            .zip(&shape)
            .fold(0.0, |s, (y, m)| s + (y - offset - m) * (y - offset - m))
    };
    let mut best = (f64::INFINITY, 0.0, 0.0, 0.0);
    for i in 0..=45 {
        let h1 = 150.0 + 10.0 * f64::from(i);
        for j in 0..=30 {
            let h2 = 500.0 + 50.0 * f64::from(j);
            if h2 <= 1.3 * h1 {
                continue;
            }
            for k in 0..=40 {
                let f = 0.005 * math::exp(f64::from(k) * math::ln(80.0) / 40.0);
                let c = cost(h1, h2, f);
                if c < best.0 {
                    best = (c, h1, h2, f);
                }
            }
        }
    }
    let (mut c, mut h1, mut h2, mut f) = best;
    let mut step = [5.0, 25.0, 0.05];
    for _ in 0..400 {
        let mut moved = false;
        for (axis, sign) in [
            (0, 1.0),
            (0, -1.0),
            (1, 1.0),
            (1, -1.0),
            (2, 1.0),
            (2, -1.0),
        ] {
            let mut trial = [h1, h2, f];
            trial[axis] = if axis == 2 {
                trial[axis] * math::exp(sign * step[axis])
            } else {
                trial[axis] + sign * step[axis]
            };
            // The thin disc stays the thinner component, as on the grid.
            if trial[1] <= 1.3 * trial[0] {
                continue;
            }
            let trial_cost = cost(trial[0], trial[1], trial[2]);
            if trial_cost < c {
                (c, h1, h2, f) = (trial_cost, trial[0], trial[1], trial[2]);
                moved = true;
            }
        }
        if !moved {
            step = step.map(|s| 0.5 * s);
        }
    }
    (h1, h2, f)
}

/// The Sun's distance from the centre, 8.178 kpc (GRAVITY Collaboration 2019, A&A 625, L10), ly.
const R0: f64 = 8.178 * LIGHT_YEARS_PER_KILOPARSEC;

/// The azimuthal mean at `(R, z)` of `f` over 360 azimuths.
fn azimuthal_mean(r: f64, z: f64, mut f: impl FnMut(&PointLy) -> f64) -> f64 {
    let azimuths = 360_u32;
    (0..azimuths).fold(0.0, |sum, j| {
        let theta = 2.0 * PI * (f64::from(j) + 0.5) / f64::from(azimuths);
        sum + f(&PointLy::new(r * math::cos(theta), r * math::sin(theta), z))
    }) / f64::from(azimuths)
}

/// Far from the plane the fixture's discs are the Milky Way's (P02.T7.b): a double exponential
/// fitted to their density at R₀ over 250–3,000 pc, as star counts are fitted, has a thin disc of
/// 300 ± 50 pc, a thick disc of 900 ± 180 pc and a thick-disc share in the plane of 4 ± 2%
/// (Bland-Hawthorn and Gerhard 2016, ARA&A 54, 529, §5.1.3), with no free height: the drawn heights
/// are effective heights, and the profiles are the Jeans equation's.
///
/// The fixture's fit is 258.6 pc, 985 pc and 2.3%: its thin disc sits near the bottom of the
/// measured range at its effective height of 1,000 ly, and would reach 286 pc at 1,100 ly and
/// 299 pc at 1,150 ly, the top of the drawn range.
#[test]
fn the_fixture_s_discs_far_from_the_plane_are_the_milky_way_s() {
    let (_, fields) = fixture();
    let discs: Vec<&Component> = fields
        .components()
        .iter()
        .filter(|c| {
            matches!(c.shape(), Shape::Disc(_)) && c.population() != Population::NuclearDisc
        })
        .collect();
    let z_pc: Vec<f64> = (0..56_u32)
        .map(|i| 250.0 + 2_750.0 * f64::from(i) / 55.0)
        .collect();
    let ln_n: Vec<f64> = z_pc
        .iter()
        .map(|&z| {
            math::ln(azimuthal_mean(R0, z * LIGHT_YEARS_PER_PARSEC, |p| {
                discs.iter().fold(0.0, |s, c| s + c.density(p))
            }))
        })
        .collect();
    let (thin, thick, share) = fit_double_exponential(&z_pc, &ln_n);
    eprintln!("far-field fit at R₀: thin {thin:.1} pc, thick {thick:.1} pc, share {share:.4}");
    assert_within("thin disc's far-field height, pc", thin, 250.0, 350.0);
    assert_within("thick disc's far-field height, pc", thick, 720.0, 1_080.0);
    assert_within("thick disc's share in the plane", share, 0.02, 0.06);
}

/// The fixture against the solar neighbourhood's measured densities, at R₀ = 8.178 kpc (GRAVITY
/// Collaboration 2019), azimuthally averaged:
///
/// - the mid-plane mass density of stars and remnants, against 0.0415 ± 0.004 M☉ pc⁻³ for main
///   sequence, giants and white dwarfs (McKee, Parravano and Hollenbach 2015, ApJ 814, 13, Table
///   1; 0.043 with brown dwarfs, which no stellar layer holds);
/// - the number density of systems with a star or white dwarf at the Sun's height, 20.8 pc
///   (Bennett and Bovy 2019, MNRAS 482, 1417), against 0.00193 ± 0.00004 per ly³ within 20 pc
///   (tallied from Kirkpatrick et al. 2024, ApJS 271, 55, Table 4: 2,240 systems in 33,510 pc³)
///   and the benchmark's 0.0018–0.0021 (brainstorm, Decisions, "2026-09-21: local density
///   rulings", 3);
/// - their ratio, the local mean present-day mass per system, against the census's 0.55–0.59
///   (tallied from the same table; McKee et al. 2015's inferred white dwarfs give the top).
///
/// The fixture gives 0.061 M☉ pc⁻³ and 0.00297 per ly³, 1.47 and 1.54 times the measurements.
/// Both scale with the stars' surface density at R₀, which is 39 M☉ pc⁻² in the fixture and needs
/// to be 25–27 for these two alone (26–30 from the census density and the effective height;
/// McKee et al.'s 33.4 ± 3); plan 02's P02.T11 tunes it and asserts both there. What does not
/// scale with it is asserted here: the local mean mass per system, 0.579 M☉, which is the default
/// mass function's.
#[test]
fn the_fixture_against_the_solar_neighbourhood() {
    let (params, fields) = fixture();
    let masses: Vec<f64> = fields
        .components()
        .iter()
        .map(|c| params.mean_system_mass(c.population()).value())
        .collect();
    let mut out = [0.0; MAX_COMPONENTS];
    let per_pc3 = LIGHT_YEARS_PER_PARSEC * LIGHT_YEARS_PER_PARSEC * LIGHT_YEARS_PER_PARSEC;
    let mass_density = azimuthal_mean(R0, 0.0, |p| {
        fields.densities(p, &mut out);
        out.iter().zip(&masses).fold(0.0, |s, (n, m)| s + n * m)
    }) * per_pc3;
    let number = azimuthal_mean(R0, 0.0, |p| fields.densities(p, &mut out));
    let sun_height = 20.8 * LIGHT_YEARS_PER_PARSEC;
    let at_sun = azimuthal_mean(R0, sun_height, |p| fields.densities(p, &mut out));
    let mean_mass = mass_density / per_pc3 / number;
    eprintln!(
        "at R₀: stars and remnants {mass_density:.4} M☉ pc⁻³ (McKee et al. 0.0415 ± 0.004), \
         systems {number:.5} per ly³ in the plane and {at_sun:.5} at the Sun's height \
         (census 0.00193, benchmark 0.0018–0.0021), {mean_mass:.3} M☉ per system"
    );
    assert_within("local mean mass per system", mean_mass, 0.55, 0.59);
    assert!(
        at_sun < number,
        "the Sun's height lies above the plane's density"
    );
}

/// Over 8 seeds the young disc's, the thick disc's and the nuclear disc's own dispersions, which
/// meet their drawn effective heights, stay physical: the young disc near the heating law's few
/// km/s for its age (Sharma et al. 2021) and the thick disc near the brainstorm's "about 35 km/s".
#[test]
fn the_other_discs_dispersions_are_physical() {
    for seed in fast_seeds().take(8) {
        let (_, fields) = seeded(seed);
        let sigma = |i: usize| {
            disc_of(&fields.components()[i])
                .profile()
                .dispersion()
                .value()
        };
        assert_within(&format!("{seed}: young"), sigma(0), 1.0, 8.0);
        assert_within(&format!("{seed}: thick"), sigma(6), 15.0, 70.0);
        assert_within(&format!("{seed}: nuclear"), sigma(9), 10.0, 80.0);
    }
}

// P02.T7.c: the bulge and the long bar.

/// Neither the bulge nor the bar rises with |x|, |y| or |z| over 10⁵ random pairs of points each
/// (P02.T7.c), nor does a halo component or any disc's envelope over 10⁴ (P02.T7.d). The halo's
/// points reach 70,000 ly, past its break and its cut; the third galaxy's dominant merger breaks
/// at 52,700 ly, inside its cut, and so does the fixture's, at 58,700 ly.
#[test]
fn no_envelope_rises_with_any_coordinate() {
    let broken = seeded(Seed::new(0x0207_5eed_0000_0005));
    let dominant = broken
        .0
        .halo()
        .components()
        .iter()
        .find_map(HaloComponentParams::outer_break)
        .unwrap();
    assert!(dominant.radius().value() < 55_000.0);
    for (_, fields) in [fixture(), seeded(Seed::new(PINNED[1])), broken] {
        let mut lcg = Lcg::new(0x7c);
        for c in fields.components() {
            let what = format!("{:?} {:?}", c.population(), c.halo_component());
            let pairs = match c.population() {
                Population::Bulge | Population::LongBar => 100_000,
                _ => 10_000,
            };
            let reach = if c.population() == Population::Halo {
                [70_000.0; 3]
            } else {
                [20_000.0, 20_000.0, 8_000.0]
            };
            for _ in 0..pairs {
                let near = reach.map(|r| r * lcg.next_f64() * lcg.next_f64());
                let axis = usize::try_from(lcg.next_below(3)).unwrap();
                let mut far = near;
                far[axis] += 3_000.0 * lcg.next_f64() * lcg.next_f64();
                let sign = |v: [f64; 3], s: u64| {
                    let flip = |i: u32| if s >> i & 1 == 1 { -1.0 } else { 1.0 };
                    PointLy::new(v[0] * flip(0), v[1] * flip(1), v[2] * flip(2))
                };
                let s = lcg.next_below(8);
                let (a, b) = (c.envelope(&sign(near, s)), c.envelope(&sign(far, s)));
                assert!(b <= a, "{what}: {b} at {far:?} above {a} at {near:?}");
            }
        }
    }
}

/// The bulge varies by at most 20% across any 128 ly cell of the fixture: its maximum is at the
/// nearest corner and its minimum at the farthest (P02.T7.c; the brainstorm says 15%).
#[test]
fn the_bulge_varies_little_across_a_coarse_cell() {
    let (_, fields) = fixture();
    let bulge = component(&fields, Population::Bulge);
    let mut worst: f64 = 0.0;
    let mut lcg = Lcg::new(0xb7);
    let corners = (0..2_000)
        .map(|_| [8_000.0, 8_000.0, 4_000.0].map(|r| (r * lcg.next_f64() / 128.0).floor() * 128.0))
        .chain(std::iter::once([0.0; 3]));
    for [x, y, z] in corners {
        let near = bulge.density(&PointLy::new(x, y, z));
        let far = bulge.density(&PointLy::new(x + 128.0, y + 128.0, z + 128.0));
        for i in 0..=4 {
            for j in 0..=4 {
                for k in 0..=4 {
                    let step = |n: i32| 128.0 * f64::from(n) / 4.0;
                    let p = PointLy::new(x + step(i), y + step(j), z + step(k));
                    let d = bulge.density(&p);
                    assert!(d <= near && d >= far, "inside ({x}, {y}, {z})");
                }
            }
        }
        worst = worst.max(1.0 - far / near);
    }
    eprintln!(
        "largest variation across a 128 ly cell: {:.1}%",
        100.0 * worst
    );
    assert!(worst <= 0.20, "{worst}");
}

/// The fixture's central bulge density is 0.17–0.30 per ly³, and over 32 seeds at least nine in
/// ten lie inside 0.12–0.55 (the brainstorm's "about 0.26 per cubic light-year (0.13–0.5 over the
/// ranges)"); 10³ seeds are in `galaxy_sweeps.rs`.
///
/// Both brackets are plan 02's 0.2–0.35 and 0.14–0.63 under Kroupa's function, scaled by the
/// bulge's mean mass per system under Kroupa's over the default's, 0.478 ÷ 0.552 = 0.866: the
/// central density is `1 ÷ (m̄ 6 V(c∥) (b ÷ a)(c ÷ a) a₀³ 10^(3s))`, free of the stellar mass, share
/// and system count (plan 02, Design note 16). Not all seeds can lie inside: the 0.06 dex scatter
/// `s` of the bulge's length enters it cubed, 0.18 dex, an unbounded tail on both sides (plan 02,
/// Risks, R16).
#[test]
fn the_bulge_centre_is_a_hundred_times_the_solar_neighbourhood() {
    let (_, fields) = fixture();
    let centre = component(&fields, Population::Bulge).density(&PointLy::new(0.0, 0.0, 0.0));
    assert_within("fixture's bulge centre", centre, 0.17, 0.30);
    let mut inside = 0;
    for seed in fast_seeds() {
        let (_, fields) = seeded(seed);
        let centre = component(&fields, Population::Bulge).density(&PointLy::default());
        if (0.12..=0.55).contains(&centre) {
            inside += 1;
        } else {
            eprintln!("{seed}'s bulge centre: {centre:.3} per ly³");
        }
    }
    assert!(inside >= 29, "{inside} of 32 bulge centres in 0.12–0.55");
}

/// The bulge's and the bar's counts by brute-force quadrature over an octant, to 0.5%.
#[test]
fn the_bulge_and_the_bar_integrate_to_their_counts() {
    let (_, fields) = fixture();
    for population in [Population::Bulge, Population::LongBar] {
        let c = component(&fields, population);
        let (sx, sy, sz, x_edges): (f64, f64, f64, &[f64]) = match c.shape() {
            Shape::Bulge(b) => (
                b.scale_x().value(),
                b.scale_y().value(),
                b.scale_z().value(),
                &[0.0, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 40.0],
            ),
            Shape::Bar(b) => (
                b.half_length().value(),
                b.width().value(),
                b.height().value(),
                &[0.0, 0.425, 0.85, 1.0, 1.3, 2.0],
            ),
            other => panic!("{other:?}"),
        };
        let y_edges = if population == Population::LongBar {
            vec![0.0, 1.0, 2.0, 4.0, 8.0]
        } else {
            vec![0.0, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 40.0]
        };
        let z_edges = [0.0, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 40.0];
        let scaled = |edges: &[f64], s: f64| edges.iter().map(|e| e * s).collect::<Vec<_>>();
        let (xe, ye, ze) = (
            scaled(x_edges, sx),
            scaled(&y_edges, sy),
            scaled(&z_edges, sz),
        );
        let octant = gl16_panels(
            |x| {
                gl16_panels(
                    |y| gl16_panels(|z| c.density(&PointLy::new(x, y, z)), &ze),
                    &ye,
                )
            },
            &xe,
        );
        assert_relative(
            &format!("{population:?} count"),
            8.0 * octant,
            c.count(),
            5e-3,
        );
    }
}

// P02.T7.d: the halo.

/// Each halo component's count by brute force in spherical coordinates, to 0.5%: every ray out to
/// where the density ends, found by bisection on the density itself, at eight azimuths.
#[test]
fn the_halo_components_integrate_to_their_counts() {
    let (_, fields) = fixture();
    for c in fields
        .components()
        .iter()
        .filter(|c| c.halo_component().is_some())
    {
        let mut total = 0.0;
        for k in 0..8_u32 {
            let phi = 2.0 * PI * (f64::from(k) + 0.5) / 8.0;
            total += gl_panels(
                |mu| {
                    let s = (1.0 - mu * mu).sqrt();
                    let at = |r: f64| {
                        PointLy::new(r * s * math::cos(phi), r * s * math::sin(phi), r * mu)
                    };
                    let end = hyperion_sim::galaxy::quad::bisect(
                        |r| if c.density(&at(r)) > 0.0 { -1.0 } else { 1.0 },
                        1.0,
                        200_000.0,
                        60,
                    );
                    let mut edges = vec![0.0, 1_000.0];
                    while edges[edges.len() - 1] * 2.0 < end {
                        let next = edges[edges.len() - 1] * 2.0;
                        edges.push(next);
                    }
                    edges.push(end);
                    gl_panels(|r| c.density(&at(r)) * r * r, &edges)
                },
                &[0.0, 0.25, 0.5, 0.75, 0.9, 1.0],
            );
        }
        // Two hemispheres, eight azimuths, 2π of them: 4π × the mean.
        let count = 2.0 * 2.0 * PI * total / 8.0;
        assert_relative(&format!("{:?}", c.halo_component()), count, c.count(), 5e-3);
    }
}

/// The spherically averaged density of the fixture's halo, `(1 ÷ 4π) ∫ ρ dΩ` at radius `r`.
fn halo_average(fields: &Fields, r: f64) -> f64 {
    let n = 2_000_u32;
    (0..n).fold(0.0, |sum, i| {
        let mu = (f64::from(i) + 0.5) / f64::from(n);
        let s = (1.0 - mu * mu).sqrt();
        let phi = 0.7;
        sum + fields.population_density(
            Population::Halo,
            &PointLy::new(r * s * math::cos(phi), r * s * math::sin(phi), r * mu),
        )
    }) / f64::from(n)
}

/// The mixture's spherically averaged slope between 20,000 and 60,000 ly is −2.1 to −3.0, and
/// nothing lies beyond 65,000 ly (P02.T7.d). The slope is the ratio of the averages at the two
/// radii: −2.97 for the fixture, whose components fall as r^−2.5 inside the dominant merger's
/// break at 58,700 ly but lose the in-situ component at 50,000 ly and the steep globular debris.
/// Star counts measure −2.1 to −2.96 inside 20–28 kpc (Xue et al. 2015; Deason, Belokurov and
/// Evans 2011; Pila-Díez et al. 2015; Iorio et al. 2018).
#[test]
fn the_halo_falls_as_a_broken_power_law() {
    let (_, fields) = fixture();
    let slope =
        math::ln(halo_average(&fields, 60_000.0) / halo_average(&fields, 20_000.0)) / math::ln(3.0);
    eprintln!("the fixture halo's slope from 20,000 to 60,000 ly: {slope:.3}");
    assert_within("halo slope", slope, -3.0, -2.1);
    let mut lcg = Lcg::new(0x4a);
    for _ in 0..20_000 {
        let r = 65_000.0 * (1.0 + 1e-9 + lcg.next_f64());
        let mu = 2.0 * lcg.next_f64() - 1.0;
        let phi = 2.0 * PI * lcg.next_f64();
        let s = (1.0 - mu * mu).sqrt();
        let p = PointLy::new(r * s * math::cos(phi), r * s * math::sin(phi), r * mu);
        assert!(fields.population_density(Population::Halo, &p).abs() < f64::MIN_POSITIVE);
    }
    // The in-situ component stops at 50,000 ly.
    let in_situ = fields
        .components()
        .iter()
        .find(|c| c.halo_component() == Some(HaloComponentKind::InSitu))
        .unwrap();
    assert!(in_situ.density(&PointLy::new(49_999.0, 0.0, 0.0)) > 0.0);
    assert!(in_situ.density(&PointLy::new(50_001.0, 0.0, 0.0)).abs() < f64::MIN_POSITIVE);
}

/// Every halo component carries its mark, and the marks come in the parameters' order.
#[test]
fn the_halo_is_a_marked_mixture() {
    for (params, fields) in [fixture(), seeded(Seed::new(PINNED[2]))] {
        let marks: Vec<HaloComponentKind> = fields
            .components()
            .iter()
            .filter_map(Component::halo_component)
            .collect();
        let expected: Vec<HaloComponentKind> = params
            .halo()
            .components()
            .iter()
            .map(HaloComponentParams::kind)
            .collect();
        assert_eq!(marks, expected);
        let halo = params.system_count() * params.population_share(Population::Halo);
        for (c, p) in fields
            .components()
            .iter()
            .filter(|c| c.population() == Population::Halo)
            .zip(params.halo().components())
        {
            assert_relative("halo component count", c.count(), halo * p.share(), 1e-15);
            assert_eq!(c.ages(), p.ages());
            assert_relative(
                "halo [Fe/H]",
                c.metallicity(&PointLy::default(), Years::new(1.1e10))
                    .mean()
                    .value(),
                p.feh_mean().value(),
                0.0,
            );
        }
    }
}

// P02.T7.e: metallicity and assembly.

/// The thin discs' \[Fe/H\] gradient at 26,000 ly is the drawn one (P02.T7.e).
#[test]
fn the_metallicity_gradient_is_the_drawn_one() {
    for (params, fields) in [fixture(), seeded(Seed::new(PINNED[0]))] {
        let drawn = params.metallicity_gradient().value();
        for c in &fields.components()[..6] {
            let age = Years::new(3e9);
            let at = |r: f64| {
                c.metallicity(&PointLy::new(0.6 * r, 0.8 * r, 20.0), age)
                    .mean()
                    .value()
            };
            let per_kpc = (at(26_100.0) - at(25_900.0)) / 200.0 * LIGHT_YEARS_PER_KILOPARSEC;
            assert_relative("gradient", per_kpc, drawn, 1e-9);
            assert_relative(
                "sigma",
                c.metallicity(&PointLy::default(), age).sigma().value(),
                0.20,
                0.0,
            );
        }
        let fixed = [
            (Population::ThickDisc, -0.55, 0.25),
            (Population::Bulge, 0.0, 0.40),
            (Population::LongBar, 0.0, 0.30),
            (Population::NuclearDisc, 0.1, 0.30),
        ];
        for (population, mean, sigma) in fixed {
            let feh = component(&fields, population).metallicity(&PointLy::default(), Years::ZERO);
            assert!((feh.mean().value() - mean).abs() < 1e-15, "{population:?}");
            assert!(
                (feh.sigma().value() - sigma).abs() < 1e-15,
                "{population:?}"
            );
        }
    }
    // The thin discs' age–metallicity relation is flat to 8 Gyr, then 0.1 dex poorer per Gyr.
    let (_, fields) = fixture();
    let young = &fields.components()[0];
    let at = |gyr: f64| {
        young
            .metallicity(&PointLy::new(26_000.0, 0.0, 0.0), Years::new(gyr * 1e9))
            .mean()
            .value()
    };
    assert_same_bits(at(6.0), at(0.01));
    assert_relative("falling beyond 8 Gyr", at(9.5) - at(8.0), -0.15, 1e-12);
}

/// The mean \[Fe/H\] over every age of the systems at `(R, z)`, azimuthally averaged: each
/// component's mean over its age distribution, by the midpoints of 2,048 equal quantiles, weighted
/// by its azimuthally averaged density.
fn local_mean_feh(fields: &Fields, r: f64, z: f64) -> f64 {
    let quantiles = 2_048_u32;
    let on_axis = PointLy::new(r, 0.0, z);
    let (weighted, systems) =
        fields
            .components()
            .iter()
            .fold((0.0, 0.0), |(weighted, systems), c| {
                let density = azimuthal_mean(r, z, |p| c.density(p));
                let mean = (0..quantiles).fold(0.0, |sum, i| {
                    let age = c
                        .ages()
                        .quantile((f64::from(i) + 0.5) / f64::from(quantiles));
                    sum + c.metallicity(&on_axis, age).mean().value()
                }) / f64::from(quantiles);
                (weighted + density * mean, systems + density)
            });
    weighted / systems
}

/// The thin discs are solar at the Sun's radius, as the youngest local stars are, and the local
/// mean over every age is near the Geneva–Copenhagen survey's (P02.T7.e; ruling 21 of 2026-09-22).
///
/// - The young disc's mean at R₀ is solar to 0.01 dex: the anchor is R₀ ÷ `R_d` = 3.8 scale lengths
///   out, and the fixture's 7,000 ly is Bovy and Rix's 2.15 kpc, so it lands 73 ly inside R₀. At
///   three scale lengths, before the ruling, it was −0.087 (−0.077 at 26,000 ly).
/// - The local mean over every age and every component at R₀ and the Sun's height, 20.8 pc
///   (Bennett and Bovy 2019), is within 0.04 dex of the survey's −0.06 (Casagrande et al. 2011,
///   A&A 530, A138, Table 1), which is a magnitude-limited sample of F and G dwarfs and so weighted
///   differently from a count of systems. It is −0.054, and was −0.135 at three scale lengths.
#[test]
fn the_sun_is_solar_and_the_local_mean_is_near_the_surveys() {
    let (_, fields) = fixture();
    let young = component(&fields, Population::YoungThinDisc);
    let young_at = |r: f64| {
        young
            .metallicity(&PointLy::new(r, 0.0, 0.0), Years::ZERO)
            .mean()
            .value()
    };
    let sun_height = 20.8 * LIGHT_YEARS_PER_PARSEC;
    let local = local_mean_feh(&fields, R0, sun_height);
    eprintln!(
        "young disc [Fe/H] {:.4} at R₀ and {:.4} at 26,000 ly; local mean over every age {local:.4} \
         at R₀ and the Sun's height (Geneva–Copenhagen −0.06)",
        young_at(R0),
        young_at(26_000.0),
    );
    assert_within("the young disc's [Fe/H] at R₀", young_at(R0), -0.01, 0.01);
    assert_within(
        "the local mean [Fe/H] over every age",
        local,
        -0.06 - 0.04,
        -0.06 + 0.04,
    );
}

/// The components come in the fixed order, at most `MAX_COMPONENTS` of them, and their counts add
/// up to the galaxy's system count, by population and in all (P02.T7.e).
#[test]
fn the_components_add_up_to_the_galaxy() {
    for seed in fast_seeds().take(8) {
        let (params, fields) = seeded(seed);
        let components = fields.components();
        assert!(components.len() <= MAX_COMPONENTS);
        assert_eq!(components.len(), 10 + params.halo().components().len());
        let order: Vec<Population> = components.iter().map(Component::population).collect();
        let mut expected = vec![Population::YoungThinDisc];
        expected.extend([Population::OldThinDisc; 5]);
        expected.extend([
            Population::ThickDisc,
            Population::Bulge,
            Population::LongBar,
            Population::NuclearDisc,
        ]);
        expected.extend(std::iter::repeat_n(
            Population::Halo,
            params.halo().components().len(),
        ));
        assert_eq!(order, expected);
        let n = params.system_count();
        let total = components.iter().fold(0.0, |sum, c| sum + c.count());
        assert_relative("Σ counts", total, n, 1e-12);
        for population in POPULATIONS {
            let count = components
                .iter()
                .filter(|c| c.population() == population)
                .fold(0.0, |sum, c| sum + c.count());
            assert_relative(
                population.name(),
                count,
                n * params.population_share(population),
                1e-12,
            );
        }
        let ids: Vec<usize> = fields.component_ids().map(ComponentId::index).collect();
        assert_eq!(ids, (0..components.len()).collect::<Vec<_>>());
        assert!(fields.component_id(components.len()).is_none());
        let last = fields.component_id(components.len() - 1).unwrap();
        assert_eq!(fields.component(last), &components[components.len() - 1]);
        // Still-forming components hold an unborn sliver beyond their count.
        let young = &components[0];
        let unborn = young.count_with_unborn() / young.count() - 1.0;
        assert_within("young unborn sliver", unborn, 5e-6, 2e-5);
    }
}

/// `densities` fills the buffer with each component's own density, bit for bit, sums them in
/// order and zeroes the rest; the population and layer densities are the matching sums.
#[test]
fn densities_population_and_layer_sums_agree() {
    let two_arms = seeded(Seed::new(PINNED[2]));
    assert_eq!(two_arms.0.arms().count(), ArmCount::Two);
    for (_, fields) in [fixture(), two_arms] {
        densities_agree(&fields);
    }
}

fn densities_agree(fields: &Fields) {
    let default = MassFunctionKind::default().to_mass_function();
    let shares = ShareMatrix::uniform(&BandShares::of(default.as_ref()));
    let mut lcg = Lcg::new(0xd5);
    let mut out = [f64::NAN; MAX_COMPONENTS];
    for i in 0..2_000 {
        let reach = if i % 4 == 0 { 3_000.0 } else { 60_000.0 };
        let p = PointLy::new(
            reach * (2.0 * lcg.next_f64() - 1.0),
            reach * (2.0 * lcg.next_f64() - 1.0),
            0.1 * reach * (2.0 * lcg.next_f64() - 1.0),
        );
        let total = fields.densities(&p, &mut out);
        let n = fields.components().len();
        let mut sum = 0.0;
        for (c, &d) in fields.components().iter().zip(&out) {
            assert_same_bits(d, c.density(&p));
            assert!(d >= 0.0 && d.is_finite());
            sum += d;
        }
        assert_same_bits(total, sum);
        for &unused in &out[n..] {
            assert_same_bits(unused, 0.0);
        }
        let by_population = POPULATIONS
            .iter()
            .fold(0.0, |s, &pop| s + fields.population_density(pop, &p));
        assert_relative("populations", by_population, total, 1e-14);
        let by_layer = MassBand::ALL
            .iter()
            .fold(0.0, |s, &band| s + fields.layer_density(&shares, band, &p));
        assert_relative("layers", by_layer, total, 1e-13);
        // A disc's density is its envelope times its arm factor.
        for c in &fields.components()[..6] {
            let envelope = c.envelope(&p);
            let arm = c.arm().unwrap();
            let factor = arm.factor(&arm.geometry().point(p.x, p.y));
            assert_same_bits(c.density(&p), envelope * factor);
        }
    }
    // The centre is finite, and the arms are exactly 1 there.
    let centre = fields.densities(&PointLy::default(), &mut out);
    assert!(centre.is_finite());
    for c in fields.components() {
        assert_same_bits(
            c.density(&PointLy::default()),
            c.envelope(&PointLy::default()),
        );
    }
}

/// The nuclear disc's central density for the fixture is 12–19 per ly³ (P02.T7.e; the
/// brainstorm's "about 16 systems per cubic light-year at its centre"): plan 02's 14–22 under
/// Kroupa's function, scaled by the nuclear disc's mean mass per system under Kroupa's over the
/// default's, 0.866. Its effective height is the drawn one, so coring it moves nothing here.
#[test]
fn the_nuclear_disc_is_dense_at_the_centre() {
    let (_, fields) = fixture();
    let centre = component(&fields, Population::NuclearDisc).density(&PointLy::default());
    assert_within("nuclear disc centre", centre, 12.0, 19.0);
    let mut out = [0.0; MAX_COMPONENTS];
    let total = fields.densities(&PointLy::default(), &mut out);
    eprintln!("fixture: nuclear disc {centre:.2}, total {total:.2} per ly³ at the centre");
    assert_within("total centre", total, 12.0, 30.0);
}

#[test]
fn fields_are_a_pure_function_of_the_parameters() {
    let params = GalaxyParams::from_seed(Seed::new(PINNED[1]), MassFunctionKind::default());
    let first = fields_of(&params);
    let _other = fields_of(&GalaxyParams::milky_way_like());
    let second = fields_of(&params);
    assert_eq!(first, second);
}

/// Every density's bits at a point, in component order.
fn density_bits(fields: &Fields, p: &PointLy) -> Vec<u64> {
    let mut out = [0.0; MAX_COMPONENTS];
    let total = fields.densities(p, &mut out);
    std::iter::once(total)
        .chain(out)
        .map(hyperion_testkit::float::bits)
        .collect()
}

/// Building the fields of one galaxy after others, and evaluating one point after others, gives
/// the same bits in any order (the sim-determinism rule for anything a cache may hold).
#[test]
fn fields_and_densities_are_order_independent() {
    let points: Vec<PointLy> = POINTS
        .iter()
        .map(|&(x, y, z)| PointLy::new(x, y, z))
        .collect();
    assert_order_independent(&PINNED, |&s| {
        let fields = fields_of(&GalaxyParams::from_seed(
            Seed::new(s),
            MassFunctionKind::default(),
        ));
        let heights = fields.sub_disc_heights().heights();
        let mut bits: Vec<u64> = heights
            .iter()
            .map(|h| hyperion_testkit::float::bits(h.value()))
            .collect();
        bits.extend(points.iter().flat_map(|p| density_bits(&fields, p)));
        bits
    });
    let (_, fields) = fixture();
    assert_order_independent(&points, |p| density_bits(&fields, p));
}

// Golden values.

/// Twenty points: the centre, the nuclear disc, the bulge, the bar's end, the arms and the solar
/// circle, the thick disc, the halo, the in-situ cut and beyond the halo.
const POINTS: [(f64, f64, f64); 20] = [
    (0.0, 0.0, 0.0),
    (40.0, -25.0, 10.0),
    (300.0, 150.0, -60.0),
    (1_200.0, 700.0, 400.0),
    (-2_500.0, 900.0, -900.0),
    (9_000.0, 800.0, 150.0),
    (15_000.0, -300.0, 40.0),
    (-17_000.0, 2_000.0, -20.0),
    (22_516.7, 13_000.0, 50.0),
    (-6_000.0, -25_300.0, 5.0),
    (26_000.0, 0.0, 0.0),
    (0.0, 26_000.0, 1_500.0),
    (31_000.0, -12_000.0, -400.0),
    (-40_000.0, 3_000.0, 250.0),
    (45_000.0, 45_000.0, 0.0),
    (5_000.0, 5_000.0, 12_000.0),
    (-20_000.0, 10_000.0, -30_000.0),
    (0.0, 0.0, 49_000.0),
    (48_000.0, -12_000.0, 20_000.0),
    (60_000.0, 30_000.0, 10_000.0),
];

fn write_fields(w: &mut GoldenWriter, label: &str, params: &GalaxyParams) {
    let fields = fields_of(params);
    let sub = fields.sub_disc_heights();
    for (i, (h, u)) in sub.heights().iter().zip(sub.unscaled()).enumerate() {
        w.f64(&format!("{label}.sub_disc[{i}].height"), h.value());
        w.f64(&format!("{label}.sub_disc[{i}].unscaled"), u.value());
    }
    w.f64(&format!("{label}.sub_disc.scale"), sub.scale());
    for (i, c) in fields.components().iter().enumerate() {
        w.f64(&format!("{label}.component[{i}].count"), c.count());
        w.f64(
            &format!("{label}.component[{i}].centre"),
            c.density(&PointLy::default()),
        );
        if let Shape::Disc(disc) = c.shape() {
            let profile = disc.profile();
            w.f64(
                &format!("{label}.component[{i}].dispersion"),
                profile.dispersion().value(),
            );
            for z in [10.0, 300.0, 3_000.0, 30_000.0] {
                w.f64(
                    &format!("{label}.component[{i}].exponent({z})"),
                    profile.exponent(z),
                );
            }
        }
    }
    let mut out = [0.0; MAX_COMPONENTS];
    for &(x, y, z) in &POINTS {
        let p = PointLy::new(x, y, z);
        let total = fields.densities(&p, &mut out);
        w.f64(&format!("{label}.total({x}, {y}, {z})"), total);
        for (i, d) in out[..fields.components().len()].iter().enumerate() {
            w.f64(&format!("{label}.density[{i}]({x}, {y}, {z})"), *d);
        }
        let feh = fields.components()[3].metallicity(&p, Years::new(2.5e9));
        w.f64(
            &format!("{label}.feh_sub_disc_2({x}, {y}, {z})"),
            feh.mean().value(),
        );
    }
}

/// The densities of three pinned seeds and the fixture at twenty points, bit for bit.
#[test]
fn galaxy_fields_are_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    w.line("# milky_way");
    write_fields(&mut w, "milky_way", &GalaxyParams::milky_way_like());
    for s in PINNED {
        let seed = Seed::new(s);
        w.line(&format!("# {seed}"));
        let params = GalaxyParams::from_seed(seed, MassFunctionKind::default());
        write_fields(&mut w, &seed.to_string(), &params);
    }
    golden!("galaxy_fields", w.as_str());
}
