//! The gas and dust field's slow statistical tests (plan 07, and P07.T12's verification of the
//! Milky Way fixture and the drawn galaxies), run by `just test-slow`.
//!
//! Each takes thousands of galaxies, lines of sight or millions of draws — a galaxy's parameters
//! cost some 17 ms even optimised, for plan 02's σ estimator — so they are marked slow and run
//! under the slow-test profile only; the fast suite runs the same checks on fewer seeds, in
//! `galaxy::gas`'s own unit tests and in `tests/gas.rs`. P07.T12's golden file and its order
//! independence are the two fast tests here.
//!
//! What P07.T12 measured at version 11 is in plan 07's task entry and Risks; each test prints its
//! own figures.

#[expect(
    dead_code,
    reason = "the gas statistics use only the shared accumulators"
)]
mod common;

use std::num::NonZeroU32;

use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::bounds::CellBox;
use hyperion_sim::galaxy::consts::LIGHT_YEARS_PER_PARSEC;
use hyperion_sim::galaxy::fields::Fields;
use hyperion_sim::galaxy::gas::ccm::Band;
use hyperion_sim::galaxy::gas::extinction::{NoiseMode, Quality, Sightline, sightline};
use hyperion_sim::galaxy::gas::field::GasField;
use hyperion_sim::galaxy::gas::noise::{NoiseCache, SmoothingScale, log_normal_factor};
use hyperion_sim::galaxy::gas::params::GasParams;
use hyperion_sim::galaxy::gas::phase::{GasPhase, Medium, PhaseMix};
use hyperion_sim::galaxy::gas::pressure::Pressure;
use hyperion_sim::galaxy::gas::smooth::{GasLayer, SmoothGas};
use hyperion_sim::galaxy::gas::{CENTIMETRES_PER_LIGHT_YEAR, SOLAR_MASSES_PER_LY3_AT_UNIT_DENSITY};
use hyperion_sim::galaxy::imf::MassFunctionKind;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::potential::{MassModel, PotentialTables};
use hyperion_sim::math;
use hyperion_sim::units::consts::{BOLTZMANN_CONSTANT, HYDROGEN_MASS_KG};
use hyperion_sim::units::{HydrogenPerCm3, KelvinPerCm3, LightYears};
use hyperion_sim::{GENERATOR_VERSION, Seed};
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::lcg::Lcg;
use hyperion_testkit::stats::normal_cdf;

use crate::common::{Running, assert_relative, assert_within, whole_ly};

/// The least share of the gas the neutral disc may hold: half, the ruling's "a neutral layer that is
/// not most of the gas is not this galaxy" (plan 07, ruling 19 of 2026-09-22). The Milky Way's local
/// column is 87% atomic or molecular (McKee, Parravano and Hollenbach 2015, ApJ 814, 13, Table 2:
/// 11.9 of 13.7 M☉ pc⁻²) and the fixture's disc 85%. Since ruling 91 the warm layer is clamped so
/// that no galaxy falls below it; no drawn galaxy comes near the clamp.
const NEUTRAL_SHARE_FLOOR: f64 = 0.5;

/// The split of the mean gas 20,000 ly above radius `r` (ly) of `params` (ruling 103): its hot
/// share and the hot phase's temperature, K.
fn corona_far_above(params: &GasParams, r: f64) -> (f64, f64) {
    let (gas, pressure) = (SmoothGas::new(params), Pressure::new(params));
    let z = 20_000.0;
    let mix = PhaseMix::split(
        HydrogenPerCm3::new(
            gas.density(GasLayer::Neutral, r, z) + gas.density(GasLayer::Molecular, r, z),
        ),
        HydrogenPerCm3::new(gas.density(GasLayer::Warm, r, z)),
        HydrogenPerCm3::new(gas.corona_density()),
        KelvinPerCm3::new(pressure.at(&gas, r, z)),
    );
    (
        mix.filling(Medium::Hot),
        mix.temperature(Medium::Hot).value(),
    )
}

/// The neutral disc keeps most of every galaxy's gas once the warm ionised layer is drawn by its own
/// density at the Sun's radius and the molecular disc by its own mass (ruling 19), over 2,000 seeds
/// each drawn against its own galaxy — so the spread of plan 02's gas masses and scale lengths is in
/// the sweep, which is what moves the share. The warm layer holds its drawn density at the Sun's
/// radius for every seed: ruling 91's clamp binds on none of them.
///
/// Rulings 91 and 103 also hold the corona hot 20,000 ly above every radius from 8,000 to
/// 40,000 ly, for every seed sampled: the mean gas there is more than 95% hot by volume, and its hot
/// phase above 10⁵ K; the least share and the coolest are printed (`gas::phase` proves the bound
/// at the draws' corners, a share of about 0.975 at some 159,000 K).
///
/// The distribution of the share is printed, since the lightest galaxies set its floor: their warm
/// layer weighs what a Milky Way's does, some 10⁹ M☉, against a gas disc of a few 10⁹.
#[test]
#[ignore = "slow: 2,000 galaxies' parameters, about 15 s optimised"]
fn the_neutral_disc_holds_most_of_the_gas_over_2000_galaxies() {
    let reference = GasParams::REFERENCE_RADIUS.value();
    let mut shares = Vec::with_capacity(2_000);
    let (mut coolest, mut least_hot) = (f64::INFINITY, f64::INFINITY);
    for n in 0..2_000_u64 {
        let seed = Seed::new(0x0700_5eed_0000_0000 | n);
        let galaxy = GalaxyParams::from_seed(seed, MassFunctionKind::default());
        let params =
            GasParams::from_galaxy(seed, &galaxy).unwrap_or_else(|error| panic!("{seed}: {error}"));
        let share = params.neutral_fraction();
        assert!(
            share >= NEUTRAL_SHARE_FLOOR,
            "{seed}: the neutral disc holds {share} of {:e} M☉ of gas, the warm layer {:e} M☉",
            params.gas_mass().value(),
            params.warm_mass().value(),
        );
        let warm = SmoothGas::new(&params).plane_density(GasLayer::Warm, reference);
        let drawn = params.warm_density().value();
        assert!(
            ((warm - drawn) / drawn).abs() < 1e-14,
            "{seed}: the warm layer reads {warm} at the Sun against its drawn {drawn}"
        );
        assert!(
            (0.025..=0.035).contains(&drawn),
            "{seed}: the clamp bound at {drawn} cm⁻³"
        );
        for i in 0..=32 {
            let r = 8_000.0 + 1_000.0 * f64::from(i);
            let (hot, t) = corona_far_above(&params, r);
            assert!(
                hot > 0.95 && t > 1e5,
                "{seed}: {hot} hot at {t} K 20,000 ly above {r} ly"
            );
            coolest = coolest.min(t);
            least_hot = least_hot.min(hot);
        }
        shares.push(share);
    }
    eprintln!(
        "the mean gas 20,000 ly up, R = 8,000–40,000 ly: least hot share {least_hot:.4}, coolest \
         hot phase {coolest:.0} K"
    );
    shares.sort_by(f64::total_cmp);
    let at = |percent: usize| shares[percent * (shares.len() - 1) / 100];
    eprintln!(
        "neutral share over 2,000 galaxies: least {:.3}, 1% {:.3}, 16% {:.3}, median {:.3}, \
         84% {:.3}, most {:.3}",
        shares[0],
        at(1),
        at(16),
        at(50),
        at(84),
        shares[shares.len() - 1],
    );
}

/// P07.T4.b: the log-normal factor is mean-preserving at a fixed point. Over 10⁶ seeds its
/// ensemble mean is 1 within four standard errors, the error taken from the log-normal's own
/// variance `e^(σ_eff²) − 1` rather than from the heavy-tailed sample, and its logarithm's variance
/// is `σ_eff²` to 2%, for `σ_ln` of 2.0 and 2.5 and for the full field and the field smoothed to
/// 250 ly, where `σ_eff² = σ_ln² Σ_kept a_k²`.
///
/// The point lies 0.23–0.81 of the way across its lattice cell on every axis at every octave, so a
/// lost normalisation or a wrong amplitude moves both figures by many standard errors.
#[test]
#[ignore = "slow: 10⁶ draws"]
fn the_log_normal_factor_keeps_its_mean_over_a_million_seeds() {
    const SEEDS: u32 = 1_000_000;
    let p = GalacticPosition::from_light_years([26_013.7, -417.3, 300.9])
        .expect("inside the root cube");
    let smoothed = SmoothingScale::AtLeast(LightYears::new(250.0));
    let cases = [
        (2.0, SmoothingScale::Full),
        (2.0, smoothed),
        (2.5, SmoothingScale::Full),
        (2.5, smoothed),
    ];
    let mut factors = [Running::default(); 4];
    let mut logs = [Running::default(); 4];
    let mut cache = NoiseCache::with_capacity(64);
    for n in 0..SEEDS {
        let seed = Seed::new(0x0704_b000_0000_0000 | u64::from(n));
        for (i, &(sigma, scale)) in cases.iter().enumerate() {
            let factor = log_normal_factor(seed, &p, sigma, scale, &mut cache);
            factors[i].push(factor);
            logs[i].push(math::ln(factor));
        }
    }
    for (i, &(sigma, scale)) in cases.iter().enumerate() {
        let variance = sigma * sigma * scale.kept_variance();
        let standard_error = (math::exp_m1(variance) / f64::from(SEEDS)).sqrt();
        let (mean, log_variance) = (factors[i].summary().mean, logs[i].summary().variance);
        eprintln!(
            "σ {sigma}, {scale:?}: mean F {mean:.5} ± {standard_error:.5}, var ln F \
             {log_variance:.5} against {variance:.5}"
        );
        assert!(
            (mean - 1.0).abs() < 4.0 * standard_error,
            "σ {sigma}, {scale:?}: the mean factor is {mean} ± {standard_error}"
        );
        assert!(
            (log_variance / variance - 1.0).abs() < 0.02,
            "σ {sigma}, {scale:?}: ln F has variance {log_variance}, not {variance}"
        );
    }
}

/// P07.T5: the thermal pressure never falls below the corona's floor, over 10⁴ random positions of
/// the root cube for each of 100 seeds' own galaxies — 10⁶ evaluations — and it never rises with
/// the height on the way up to the positions drawn. The pressure is the floor plus a term that is
/// not negative, so the floor holds bit for bit; this sweep is what would see a formula that let
/// the floor itself fall with height.
#[test]
#[ignore = "slow: 10⁶ pressure evaluations over 100 galaxies"]
fn the_pressure_never_falls_below_the_floor_over_100_galaxies() {
    let mut lcg = Lcg::new(0x0705_f100);
    let mut least_excess = f64::INFINITY;
    for n in 0..100_u64 {
        let seed = Seed::new(0x0705_f100_0000_0000 | n);
        let galaxy = GalaxyParams::from_seed(seed, MassFunctionKind::default());
        let params = GasParams::from_galaxy(seed, &galaxy).expect("a drawn galaxy's gas");
        let (gas, pressure) = (SmoothGas::new(&params), Pressure::new(&params));
        let floor = params.pressure_floor().value();
        for _ in 0..10_000 {
            let [x, y, z] = [0; 3].map(|_| 65_536.0 * (2.0 * lcg.next_f64() - 1.0));
            let r = (x * x + y * y).sqrt();
            let at = pressure.at(&gas, r, z);
            assert!(
                at >= floor,
                "{seed}: {at} K cm⁻³ at ({x}, {y}, {z}), below {floor}"
            );
            assert!(
                at <= pressure.at(&gas, r, 0.5 * z),
                "{seed}: rising with height at {r}"
            );
            least_excess = least_excess.min(at / floor - 1.0);
        }
    }
    eprintln!("the least pressure over its floor: 1 + {least_excess:e}");
}

/// The cell of `edge` ly holding the in-plane point `(x, y)` at height `z`, all in ly, moved inside
/// the root cube if it would reach past it.
fn cell_at(x: f64, y: f64, z: f64, edge: u32) -> CellBox {
    let size = f64::from(edge);
    let corner = |v: f64| {
        let low = (v / size).floor() * size;
        whole_ly(low.clamp(-65_536.0, 65_536.0 - size))
    };
    CellBox::new([corner(x), corner(y), corner(z)], edge).expect("a cell of the root cube")
}

/// P07.T6.b: `neutral_bound` holds over a cell at every point of it, as the densities compute them.
/// The hunt draws 10⁴ cells of 128 ly and 4,096 ly — a third across the lanes where they leave the
/// bar's ends, a third about the neutral disc's peak radius `√(R_m R_g)`, a third anywhere in the
/// disc — in the Milky Way fixture and three drawn galaxies, and 100 points in each, 10⁶ checks,
/// and finds no point above its cell's bound. On the cells away from the lanes, where the lane
/// factor's bound is below 1, the bound is within a factor of 1.5 of the true maximum, taken on a
/// 17 × 17 grid over the cell's face nearest the plane, where every layer is densest.
#[test]
#[ignore = "slow: 10⁶ bound checks over 10⁴ cells"]
fn the_neutral_bound_holds_over_10000_cells() {
    let mut lcg = Lcg::new(0x0706_b000);
    let mut galaxies = vec![(GalaxyParams::milky_way_like(), None)];
    for n in 0..3_u64 {
        let seed = Seed::new(0x0706_b000_0000_0000 | n);
        galaxies.push((
            GalaxyParams::from_seed(seed, MassFunctionKind::default()),
            Some(seed),
        ));
    }
    let (mut checks, mut worst_ratio, mut tight_cells) = (0_u64, 0.0_f64, 0_u32);
    for (params, seed) in &galaxies {
        let fields = Fields::new(params, &MassModel::new(params));
        let gas = match seed {
            None => GasField::with_params(Seed::new(9), GasParams::milky_way_like(), &fields),
            Some(seed) => GasField::new(*seed, params, &fields).expect("a drawn galaxy's gas"),
        };
        let arms = fields.arms();
        let bar = arms.bar_half_length().value();
        let peak = gas.smooth().neutral_peak_radius();
        let shift = gas.lanes().shift().value();
        for i in 0..2_500_u32 {
            let edge = if lcg.next_below(2) == 0 { 128 } else { 4_096 };
            let (r, theta) = match i % 3 {
                0 => {
                    let r = bar * (0.8 + 0.6 * lcg.next_f64());
                    let j = u32::try_from(lcg.next_below(u64::from(arms.count().get()))).unwrap();
                    let theta = arms.ridge_azimuth(r + shift, j) + 0.02 * (lcg.next_f64() - 0.5);
                    (r, theta)
                }
                1 => (
                    peak + 4_000.0 * (lcg.next_f64() - 0.5),
                    core::f64::consts::TAU * lcg.next_f64(),
                ),
                _ => (
                    60_000.0 * lcg.next_f64().sqrt(),
                    core::f64::consts::TAU * lcg.next_f64(),
                ),
            };
            let z = 3_000.0 * (2.0 * lcg.next_f64() - 1.0) * lcg.next_f64();
            let cell = cell_at(r * math::cos(theta), r * math::sin(theta), z, edge);
            let bound = gas.neutral_bound(&cell).value();
            let [x0, y0, z0] = cell.min_corner().map(f64::from);
            let size = f64::from(edge);
            for _ in 0..100 {
                let point = [x0, y0, z0].map(|low| low + size * lcg.next_f64());
                let p = GalacticPosition::from_light_years(point).expect("inside the cube");
                let n = gas.mean_neutral_density(&p).value();
                assert!(n <= bound, "{n} above {bound} in {cell:?} at {point:?}");
                checks += 1;
            }
            if gas.lanes().sup(&cell) < 1.0 {
                let near = cell.nearest_corner().z;
                let mut greatest = 0.0_f64;
                for a in 0..=16 {
                    for b in 0..=16 {
                        let point = [
                            x0 + size * f64::from(a) / 16.0,
                            y0 + size * f64::from(b) / 16.0,
                            near,
                        ];
                        let p = GalacticPosition::from_light_years(point).expect("in the cube");
                        greatest = greatest.max(gas.mean_neutral_density(&p).value());
                    }
                }
                if greatest > 0.0 {
                    let ratio = bound / greatest;
                    assert!(
                        ratio < 1.5,
                        "the bound of {cell:?} is {ratio} of its maximum"
                    );
                    worst_ratio = worst_ratio.max(ratio);
                    tight_cells += 1;
                }
            }
        }
    }
    assert_eq!(checks, 1_000_000);
    eprintln!(
        "no violation in 10⁶ checks; {tight_cells} cells away from lanes, worst bound over \
         maximum {worst_ratio}"
    );
}

// --- P07.T12: the Milky Way fixture and the drawn galaxies ---

/// The Milky Way fixture's stellar fields, which a gas field copies its arms and metallicity from.
fn milky_way_fields() -> Fields {
    let params = GalaxyParams::milky_way_like();
    Fields::new(&params, &MassModel::new(&params))
}

fn at(ly: [f64; 3]) -> GalacticPosition {
    GalacticPosition::from_light_years(ly).expect("inside the root cube")
}

/// The ends of P07.T12's in-plane line at azimuth `i` of 64: 3,000 ly long, tangent to the circle
/// of 26,000 ly and centred on it, in the plane.
fn in_plane_line(i: u32) -> (GalacticPosition, GalacticPosition) {
    let phi = core::f64::consts::TAU * f64::from(i) / 64.0;
    let (c, s) = (math::cos(phi), math::sin(phi));
    let centre = [26_000.0 * c, 26_000.0 * s];
    let half = [-1_500.0 * s, 1_500.0 * c];
    (
        at([centre[0] - half[0], centre[1] - half[1], 0.0]),
        at([centre[0] + half[0], centre[1] + half[1], 0.0]),
    )
}

/// The mean-mode `A_V` of P07.T12's in-plane lines, averaged over their 64 azimuths.
fn in_plane_mean(gas: &GasField, cache: &mut NoiseCache) -> f64 {
    in_plane_mean_lines(gas, cache).0
}

/// The mean-mode `A_V`, mag, and neutral hydrogen column, cm⁻², of P07.T12's in-plane lines,
/// averaged over their 64 azimuths.
fn in_plane_mean_lines(gas: &GasField, cache: &mut NoiseCache) -> (f64, f64) {
    let (a_v, neutral) = (0..64).fold((0.0, 0.0), |(a_v, neutral), i| {
        let (a, b) = in_plane_line(i);
        let line = sightline(gas, &a, &b, NoiseMode::Mean, Quality::Full, &[], cache);
        (
            a_v + line.a_v().value(),
            neutral + line.neutral_hydrogen_column().value(),
        )
    });
    (a_v / 64.0, neutral / 64.0)
}

/// The value at `percent` of a sorted sample.
fn percentile(sorted: &[f64], percent: usize) -> f64 {
    sorted[percent * (sorted.len() - 1) / 100]
}

/// P07.T12 and ruling 91.6: the in-plane extinction of the Milky Way fixture, along 3,000 ly lines
/// centred at 26,000 ly and averaged over 64 azimuths.
///
/// In mean mode it is held to 0.9–2.0 mag, replacing the brainstorm's "about one magnitude per
/// 3,000 ly" (0.8–1.3). The top is McKee, Parravano and Hollenbach's (2015, ApJ 814, 13) mean
/// mid-plane density of 1.17 cm⁻³ with its 10%, which over Bohlin et al.'s 1.87 × 10²¹ cm⁻² per
/// magnitude is 1.78 (1.60–1.96) mag per 3,000 ly; the floor is the upper end of the typical
/// stellar lines' 0.7–1.0 mag/kpc (0.64–0.92 per 3,000 ly), since a mean is never below a typical
/// line. The `Realised` mean over the same lines and 32 seeds agrees within four standard errors,
/// the error from the 32 seeds' own line averages; its median line is recorded against the typical
/// lines' 0.64–0.92, not held to it. The `Realised` neutral column, `∫ (n_n + n_mol) F` since
/// ruling 103, agrees with the mean mode's `∫ (n_n + n_mol)` within four standard errors the same
/// way: it is its expectation.
#[test]
#[ignore = "slow: 2,048 realised lines of 3,000 ly at full quality"]
fn the_in_plane_extinction_is_the_local_mean_rate() {
    let fields = milky_way_fields();
    let mut cache = NoiseCache::with_capacity(4_096);
    let gas = |seed| GasField::with_params(seed, GasParams::milky_way_like(), &fields);
    let (mean, mean_neutral) = in_plane_mean_lines(&gas(Seed::new(1)), &mut cache);
    let (mut averages, mut lines) = (Running::default(), Vec::with_capacity(2_048));
    let mut neutrals = Running::default();
    for n in 0..32_u64 {
        let field = gas(Seed::new(0x0712_a000_0000_0000 | n));
        let (mut sum, mut neutral) = (0.0, 0.0);
        for i in 0..64 {
            let (a, b) = in_plane_line(i);
            let line = sightline(
                &field,
                &a,
                &b,
                NoiseMode::Realised,
                Quality::Full,
                &[],
                &mut cache,
            );
            sum += line.a_v().value();
            neutral += line.neutral_hydrogen_column().value();
            lines.push(line.a_v().value());
        }
        averages.push(sum / 64.0);
        neutrals.push(neutral / 64.0);
    }
    let realised = averages.summary();
    let realised_neutral = neutrals.summary();
    lines.sort_by(f64::total_cmp);
    eprintln!(
        "in-plane A_V per 3,000 ly at 26,000 ly: mean mode {mean:.3} mag; realised {:.3} ± {:.3} \
         over 32 seeds; realised lines 16/50/84%: {:.3}/{:.3}/{:.3} (typical lines 0.64–0.92); \
         neutral column: mean mode {mean_neutral:.4e}, realised {:.4e} ± {:.2e} cm⁻²",
        realised.mean,
        realised.standard_error,
        percentile(&lines, 16),
        percentile(&lines, 50),
        percentile(&lines, 84),
        realised_neutral.mean,
        realised_neutral.standard_error,
    );
    assert!(
        (realised_neutral.mean - mean_neutral).abs() < 4.0 * realised_neutral.standard_error,
        "realised neutral {} ± {} against {mean_neutral}",
        realised_neutral.mean,
        realised_neutral.standard_error
    );
    assert_within("mean-mode A_V per 3,000 ly", mean, 0.9, 2.0);
    assert!(
        (realised.mean - mean).abs() < 4.0 * realised.standard_error,
        "realised {} ± {} against {mean}",
        realised.mean,
        realised.standard_error
    );
}

/// P07.T12: the centre lies behind the brainstorm's "some thirty" magnitudes in V and "about three"
/// in K. From (26,000, 0, 0) ly to the centre, in mean mode, `A_V` is 24–38 mag and `A_K` 2.6–4.5;
/// over 256 seeds in `Realised` mode the mean agrees with the mean mode's within four standard
/// errors, and the median and the 16th and 84th percentiles are recorded. A median under 10 mag is
/// a finding for the owner, not a failure: half the column is a molecular disc a few lattice cells
/// across, where one log-normal factor decides the line (plan 07, Risks).
#[test]
#[ignore = "slow: 256 realised lines of 26,000 ly at full quality"]
fn the_centre_lies_behind_thirty_magnitudes() {
    let fields = milky_way_fields();
    let mut cache = NoiseCache::with_capacity(4_096);
    let gas = |seed| GasField::with_params(seed, GasParams::milky_way_like(), &fields);
    let (sun, centre) = (at([26_000.0, 0.0, 0.0]), at([0.0, 0.0, 0.0]));
    let line = |field: &GasField, mode, cache: &mut NoiseCache| {
        sightline(field, &sun, &centre, mode, Quality::Full, &[], cache)
    };
    let mean = line(&gas(Seed::new(1)), NoiseMode::Mean, &mut cache);
    let (a_v, a_k) = (mean.a_v().value(), mean.in_band(Band::K).value());
    let mut running = Running::default();
    let mut realised = Vec::with_capacity(256);
    for n in 0..256_u64 {
        let field = gas(Seed::new(0x0712_c000_0000_0000 | n));
        let value = line(&field, NoiseMode::Realised, &mut cache).a_v().value();
        running.push(value);
        realised.push(value);
    }
    let summary = running.summary();
    realised.sort_by(f64::total_cmp);
    let median = percentile(&realised, 50);
    eprintln!(
        "to the centre: mean mode A_V {a_v:.2}, A_K {a_k:.3} mag; realised over 256 seeds mean \
         {:.2} ± {:.2}, 16/50/84% {:.2}/{median:.2}/{:.2} mag{}",
        summary.mean,
        summary.standard_error,
        percentile(&realised, 16),
        percentile(&realised, 84),
        if median < 10.0 {
            " (a median under 10 mag: a finding for the owner)"
        } else {
            ""
        },
    );
    assert_within("A_V to the centre, mean mode", a_v, 24.0, 38.0);
    assert_within("A_K to the centre, mean mode", a_k, 2.6, 4.5);
    assert!(
        (summary.mean - a_v).abs() < 4.0 * summary.standard_error,
        "realised {} ± {} against {a_v}",
        summary.mean,
        summary.standard_error
    );
}

/// A supernova shell's observable window `W` at ambient density `n` (cm⁻³) and pressure
/// `p_over_k` (K cm⁻³), Myr: the closed form plan 09's P09.T15.a will own, copied here until then
/// (plan 07, P07.T12, as ruling 98 of 2026-09-22 corrects it).
///
/// `W = t_PDS [¾ (v_PDS ÷ β c_net)^(10⁄7) + ¼]`, with Cioffi, McKee and Bertschinger's (1988, ApJ
/// 334, 252) pressure-driven snowplough at 10⁵¹ erg and solar metallicity, `t_PDS = 1.33 × 10⁴ yr ×
/// n^(−4⁄7)` (eqs. 3.10–3.11) and `v_PDS = 413 km/s × n^(1⁄7)` (eq. 3.33b). The shell merges when
/// it slows to `β = 2` times the ambient's effective sound speed, `c_net² = C₀² + (8 km/s)²`, with
/// `C₀² = P ÷ ρ` CMB88's "ambient isothermal sound speed" (p. 264), `ρ = 1.4 m_H n`, and 8 km/s of
/// turbulence. The `+ ¼` is the exact inverse of eq. 3.32b, `v = v_PDS (4⁄3 t ÷ t_PDS − 1⁄3)^(−7⁄10)`,
/// where their eq. 4.4a drops it; the plan's first copy had `− ¼` and `γ P ÷ ρ`.
fn shell_window_myr(n: f64, p_over_k: f64) -> f64 {
    let thermal = p_over_k * BOLTZMANN_CONSTANT / (1.4 * HYDROGEN_MASS_KG * n);
    let c_net = (thermal + 8e3 * 8e3).sqrt();
    let t_pds = 1.33e4 * math::powf(n, -4.0 / 7.0);
    let v_pds = 413e3 * math::powf(n, 1.0 / 7.0);
    t_pds * (0.75 * math::powf(v_pds / (2.0 * c_net), 10.0 / 7.0) + 0.25) / 1e6
}

/// The largest shell window over a fixed scan of 4,001 densities from 10⁻³ to 10 cm⁻³, evenly in
/// `log n`, at `p_over_k`: the window, Myr, and the density it falls at, cm⁻³.
fn largest_shell_window(p_over_k: f64) -> (f64, f64) {
    (0..=4_000)
        .map(|i| {
            let n = math::exp10(-3.0 + f64::from(i) / 1_000.0);
            (shell_window_myr(n, p_over_k), n)
        })
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .expect("a non-empty scan")
}

/// P07.T12: the shell window's ceiling, which is what the pressure floor is for. At the floor the
/// largest window over the scan is the brainstorm's 2–4 Myr for the Milky Way fixture and every one
/// of 200 seeds; at the Milky Way plane's pressure, the fixture's at 26,000 ly, it is 0.5–1 Myr and
/// falls at a density of 0.1–0.5 cm⁻³ (ruling 98 of 2026-09-22, which restores both windows with
/// the isothermal merge criterion and trims the floor to 300–450 K cm⁻³).
#[test]
#[ignore = "slow: 200 galaxies' parameters and 201 scans of the shell window"]
fn the_pressure_floor_caps_the_shell_window() {
    let fixture = GasParams::milky_way_like();
    let (mut least, mut most) = (f64::INFINITY, 0.0_f64);
    let mut check = |params: &GasParams| {
        let (window, _) = largest_shell_window(params.pressure_floor().value());
        least = least.min(window);
        most = most.max(window);
        assert_within("the largest window at the floor, Myr", window, 2.0, 4.0);
    };
    check(&fixture);
    for n in 0..200_u64 {
        let seed = Seed::new(0x0712_5e11_0000_0000 | n);
        let galaxy = GalaxyParams::from_seed(seed, MassFunctionKind::default());
        check(&GasParams::from_galaxy(seed, &galaxy).expect("a drawn galaxy's gas"));
    }
    let gas = SmoothGas::new(&fixture);
    let plane = Pressure::new(&fixture).at(&gas, 26_000.0, 0.0);
    let (window, density) = largest_shell_window(plane);
    let (at_floor, _) = largest_shell_window(fixture.pressure_floor().value());
    let (at_300, _) = largest_shell_window(300.0);
    let (at_450, _) = largest_shell_window(450.0);
    eprintln!(
        "largest shell window: {at_floor:.2} Myr at the fixture's floor, {least:.2}–{most:.2} Myr \
         over 200 seeds' floors ({at_300:.2} at 300 and {at_450:.2} at 450 K cm⁻³; the plan's \
         2–4); at the plane's {plane:.0} K cm⁻³ {window:.3} Myr at n = {density:.3} cm⁻³ (the \
         plan's 0.5–1 Myr at 0.1–0.5 cm⁻³)"
    );
    assert_within("the largest window in the plane, Myr", window, 0.5, 1.0);
    assert_within("the density it falls at, cm⁻³", density, 0.1, 0.5);
}

/// A point drawn uniformly over the annulus of radii 20,000–30,000 ly, at height `z` ly.
fn annulus_point(lcg: &mut Lcg, z: f64) -> GalacticPosition {
    let r = (4e8 + 5e8 * lcg.next_f64()).sqrt();
    let phi = core::f64::consts::TAU * lcg.next_f64();
    at([r * math::cos(phi), r * math::sin(phi), z])
}

/// What the drawn phases of a set of points came to (ruling 103).
#[derive(Default)]
struct Tally {
    points: u32,
    /// Points drawn in each medium, in [`Medium::ALL`]'s order.
    media: [u32; 4],
    molecular: u32,
    /// Points in parcels the split does not compress.
    uncompressed: u32,
    /// The drawn hot points' temperatures, K.
    hot_temperatures: Vec<f64>,
    /// The warm neutral and cold mass of the uncompressed parcels, and the neutral mass of the
    /// compressed ones and of all, per unit volume summed over the points, cm⁻³.
    warm_neutral_mass: f64,
    cold_mass: f64,
    compressed_neutral_mass: f64,
    neutral_mass: f64,
}

impl Tally {
    /// Adds the state of `gas` at `p` to the tally, and checks that a drawn warm point is at
    /// exactly 8,000 K: no clamp (ruling 98.4 withdrawn).
    fn add(&mut self, gas: &GasField, p: &GalacticPosition, cache: &mut NoiseCache) {
        let state = gas.state(p, SmoothingScale::Full, cache);
        let mix = state.mix();
        self.points += 1;
        let k = Medium::ALL
            .iter()
            .position(|&m| m == state.medium())
            .expect("one of the four");
        self.media[k] += 1;
        self.molecular += u32::from(state.phase() == GasPhase::Molecular);
        let neutral = mix.mass(Medium::WarmNeutral).value() + mix.mass(Medium::Cold).value();
        self.neutral_mass += neutral;
        if mix.overpressure() > 1.0 {
            self.compressed_neutral_mass += neutral;
        } else {
            self.uncompressed += 1;
            self.warm_neutral_mass += mix.mass(Medium::WarmNeutral).value();
            self.cold_mass += mix.mass(Medium::Cold).value();
        }
        match state.medium() {
            Medium::Hot => self.hot_temperatures.push(state.temperature().value()),
            Medium::WarmIonised | Medium::WarmNeutral => {
                assert!(
                    state.temperature().value().total_cmp(&8_000.0).is_eq(),
                    "{state:?}"
                );
            }
            Medium::Cold => {}
        }
    }

    /// The share of the points drawn in `medium`.
    fn share(&self, medium: Medium) -> f64 {
        let k = Medium::ALL
            .iter()
            .position(|&m| m == medium)
            .expect("one of the four");
        f64::from(self.media[k]) / f64::from(self.points)
    }
}

/// The height P07.T12 samples the gas at: `z` or `−z`, evenly.
fn either_side(lcg: &mut Lcg, z: f64) -> f64 {
    if lcg.next_below(2) == 0 { z } else { -z }
}

/// A point at radius `r` ly and height `z` ly, at a uniformly drawn azimuth.
fn ring_point(lcg: &mut Lcg, r: f64, z: f64) -> GalacticPosition {
    let phi = core::f64::consts::TAU * lcg.next_f64();
    at([r * math::cos(phi), r * math::sin(phi), z])
}

/// The warm ionised share at 26,000 ly by height, from the plane to 10,000 ly up in steps of
/// 500 ly, over 10⁴ points each: `(height in ly, share)`.
fn warm_ionised_profile(gas: &GasField, lcg: &mut Lcg, cache: &mut NoiseCache) -> Vec<(f64, f64)> {
    let mut profile = Vec::with_capacity(21);
    for step in 0..=20_u32 {
        let height = 500.0 * f64::from(step);
        let mut tally = Tally::default();
        for _ in 0..10_000 {
            let z = either_side(lcg, height);
            tally.add(gas, &ring_point(lcg, 26_000.0, z), cache);
        }
        profile.push((height, tally.share(Medium::WarmIonised)));
    }
    profile
}

/// P07.T12's filling factors by Monte Carlo, as ruling 103 (research `gas2phase`, NOTES §5) sets
/// them, for the Milky Way fixture at its `σ_ln` of 1.2, with each point's phase drawn from its
/// parcel's split:
///
/// - in the plane at radii of 20,000–30,000 ly, over 10⁵ points: hot 0.20–0.40 (the brainstorm's
///   "a fifth to two fifths"), warm neutral 0.30–0.70 (Heiles and Troland 2003's "about 0.50"),
///   cold 0.005–0.05 (McKee and Ostriker's 0.02–0.04, widened below, since the model sits under
///   it), molecular under 2%; the cold share of the uncompressed parcels' atomic mass 0.3–0.7
///   (Heiles and Troland's 0.4 to Dickey and Lockman's "most"); the hot phase's median
///   temperature, by volume, 10^5.5–10^6.5 K (Ferrière 2001's ~10⁶); every warm point at 8,000 K;
///   and at least 95% of the points in parcels the split does not compress;
/// - at |z| = 20,000 ly, over 10⁵ points: hot above 0.95;
/// - in the plane at 26,000 ly, over 10⁵ points: warm ionised 0.04–0.15 (Gaensler et al. 2008's
///   0.04 ± 0.01, Haffner et al. 2009's ~0.1, Ferrière's 5–14%);
/// - at 26,000 ly from the plane to 10,000 ly up, in steps of 500 ly, over 10⁴ points each: the
///   warm ionised share peaks at 0.15–0.40 at 1,600–6,500 ly (0.5–2 kpc) and is below the peak at
///   10,000 ly, Gaensler et al.'s "maximum of ∼ 30% at a height of ≈1–1.5 kpc, before then
///   declining". The research predicts 0.18 near 1 kpc, some 1.7 times under Gaensler's peak: a
///   finding for the owner, not tuned.
#[test]
#[ignore = "slow: 5 × 10⁵ states of the fixture's gas"]
fn the_phases_fill_the_plane_as_the_brainstorm_says() {
    let fields = milky_way_fields();
    let gas = GasField::with_params(Seed::new(0x0712_f111), GasParams::milky_way_like(), &fields);
    let mut cache = NoiseCache::with_capacity(4_096);
    let mut lcg = Lcg::new(0x0712_f111);
    let points = 100_000;
    let (mut plane, mut high, mut sun) = (Tally::default(), Tally::default(), Tally::default());
    for _ in 0..points {
        plane.add(&gas, &annulus_point(&mut lcg, 0.0), &mut cache);
        let z = either_side(&mut lcg, 20_000.0);
        high.add(&gas, &annulus_point(&mut lcg, z), &mut cache);
        sun.add(&gas, &ring_point(&mut lcg, 26_000.0, 0.0), &mut cache);
    }
    let profile = warm_ionised_profile(&gas, &mut lcg, &mut cache);
    let (peak_height, peak) = profile
        .iter()
        .copied()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .expect("21 heights");
    let top = profile[20].1;
    plane.hot_temperatures.sort_by(f64::total_cmp);
    let hot_median = percentile(&plane.hot_temperatures, 50);
    let cold_share = plane.cold_mass / (plane.cold_mass + plane.warm_neutral_mass);
    let uncompressed = f64::from(plane.uncompressed) / f64::from(plane.points);
    let profile_text: Vec<String> = profile
        .iter()
        .map(|(z, share)| format!("{z:.0} {share:.3}"))
        .collect();
    eprintln!(
        "in the plane at 20,000–30,000 ly: hot {:.3}, warm ionised {:.3}, warm neutral {:.3}, \
         cold {:.4}, molecular {:.4}; cold share of the atomic mass {cold_share:.3} (compressed \
         parcels hold {:.3} of the neutral mass); hot phase 16/50/84% by volume {:.3e}/\
         {hot_median:.3e}/{:.3e} K; uncompressed share {uncompressed:.4}; at |z| 20,000 ly hot \
         {:.4}; at 26,000 ly in the plane warm ionised {:.3}; the warm ionised share by height \
         (ly) {}: peak {peak:.3} at {peak_height:.0} ly, {top:.3} at 10,000 ly",
        plane.share(Medium::Hot),
        plane.share(Medium::WarmIonised),
        plane.share(Medium::WarmNeutral),
        plane.share(Medium::Cold),
        f64::from(plane.molecular) / f64::from(plane.points),
        plane.compressed_neutral_mass / plane.neutral_mass,
        percentile(&plane.hot_temperatures, 16),
        percentile(&plane.hot_temperatures, 84),
        high.share(Medium::Hot),
        sun.share(Medium::WarmIonised),
        profile_text.join(", "),
    );
    assert_within(
        "the hot share of the plane",
        plane.share(Medium::Hot),
        0.20,
        0.40,
    );
    assert_within(
        "the warm neutral share of the plane",
        plane.share(Medium::WarmNeutral),
        0.30,
        0.70,
    );
    assert_within(
        "the cold share of the plane",
        plane.share(Medium::Cold),
        0.005,
        0.05,
    );
    let molecular = f64::from(plane.molecular) / f64::from(plane.points);
    assert!(molecular < 0.02, "a molecular share of {molecular}");
    assert_within("the cold share of the atomic mass", cold_share, 0.3, 0.7);
    assert_within(
        "log10 of the hot phase's median temperature",
        math::log10(hot_median),
        5.5,
        6.5,
    );
    assert!(uncompressed >= 0.95, "{uncompressed} uncompressed");
    assert!(
        high.share(Medium::Hot) > 0.95,
        "a hot share of {} far above",
        high.share(Medium::Hot)
    );
    assert_within(
        "the warm ionised share at 26,000 ly in the plane",
        sun.share(Medium::WarmIonised),
        0.04,
        0.15,
    );
    assert_within("the warm ionised peak", peak, 0.15, 0.40);
    assert_within(
        "the warm ionised peak's height, ly",
        peak_height,
        1_600.0,
        6_500.0,
    );
    assert!(top < peak, "{top} at 10,000 ly against a peak of {peak}");
}

/// The mean of the log-normal factor over `points` points drawn uniformly from the cube of
/// 16,384 ly with its low corner at (10,000, −8,192, −8,192) ly, at `σ_ln` 2.3 and full detail.
fn cube_mean(seed: Seed, points: u32, lcg: &mut Lcg, cache: &mut NoiseCache) -> f64 {
    let mut running = Running::default();
    for _ in 0..points {
        let p = at([
            10_000.0 + 16_384.0 * lcg.next_f64(),
            -8_192.0 + 16_384.0 * lcg.next_f64(),
            -8_192.0 + 16_384.0 * lcg.next_f64(),
        ]);
        running.push(log_normal_factor(
            seed,
            &p,
            2.3,
            SmoothingScale::Full,
            cache,
        ));
    }
    running.summary().mean
}

/// P07.T12: the noise preserves the mean in space as well as over seeds. The volume average of the
/// log-normal factor over a cube of 16,384 ly, sampled at 10⁶ points, is 1 within four times the
/// spread the cube's own correlated variance implies.
///
/// A single cube's average is not 1 to the sampling error of 10⁶ independent points (0.014 at
/// `σ_ln` 2.3, where the factor's variance is `e^(σ²) − 1` = 197): the noise is correlated over
/// its octaves' wavelengths, up to 1,024 ly, so the cube holds only some 16³ independent coarse
/// cells. The tolerance is measured rather than modelled: the standard deviation of the same
/// cube's average over 32 other seeds at 32,768 points each, which carries the correlated variance
/// and more sampling variance than the main estimate, so it bounds that estimate's error from
/// above.
#[test]
#[ignore = "slow: 2 × 10⁶ evaluations of the log-normal factor"]
fn the_noise_keeps_its_mean_over_a_cube() {
    let mut cache = NoiseCache::with_capacity(4_096);
    let mut lcg = Lcg::new(0x0712_cbe0);
    let mean = cube_mean(Seed::new(0x0712_cbe0), 1_000_000, &mut lcg, &mut cache);
    let mut spread = Running::default();
    for n in 0..32_u64 {
        spread.push(cube_mean(
            Seed::new(0x0712_cbe1_0000_0000 | n),
            32_768,
            &mut lcg,
            &mut cache,
        ));
    }
    let across = spread.summary();
    let tolerance = 4.0 * across.variance.sqrt();
    eprintln!(
        "the factor over a 16,384 ly cube: {mean:.4} at 10⁶ points; over 32 seeds {:.4}, standard \
         deviation {:.4}",
        across.mean,
        across.variance.sqrt()
    );
    assert!(
        (mean - 1.0).abs() < tolerance,
        "{mean} against 1 ± {tolerance}"
    );
    assert!((across.mean - 1.0).abs() < 4.0 * across.standard_error);
}

/// The gas mass of the smooth field in M☉, by a midpoint sum in `ln R` over 400 radii from 0.25 ly
/// to 40 scale lengths, each layer's closed-form vertical column standing for its height integral.
/// The lanes are left out, since they move none of the mass (`tests/gas.rs` holds them to 10⁻⁹).
fn smooth_gas_mass(gas: &SmoothGas, radial_scale: f64) -> f64 {
    let radii = 400_u32;
    let (lo, hi) = (0.25, 40.0 * radial_scale);
    let step = math::ln(hi / lo) / f64::from(radii);
    let per_column = SOLAR_MASSES_PER_LY3_AT_UNIT_DENSITY / CENTIMETRES_PER_LIGHT_YEAR;
    (0..radii)
        .map(|i| {
            let r = lo * math::exp(step * (f64::from(i) + 0.5));
            core::f64::consts::TAU * r * r * step * per_column * gas.disc_column(r)
        })
        .sum()
}

/// Every drawn gas parameter of `gas` against Design note 3's ranges, as amended by rulings 91 and
/// 103: the corona's 0.5–0.8 × 10⁻³ cm⁻³ and `σ_ln`'s 1.0–1.4.
fn assert_gas_in_ranges(seed: Seed, galaxy: &GalaxyParams, gas: &GasParams) {
    let bar = galaxy.bar().half_length().value();
    let rows = [
        (
            "hole scale over the bar",
            gas.hole_scale().value() / bar,
            0.8,
            1.2,
        ),
        ("warm density", gas.warm_density().value(), 0.025, 0.035),
        ("warm height", gas.warm_height().value(), 2_500.0, 3_500.0),
        (
            "molecular mass",
            gas.molecular_disc().mass().value(),
            2e6,
            3e6,
        ),
        (
            "molecular height ratio",
            gas.molecular_disc().height() / gas.molecular_disc().length(),
            0.15,
            0.25,
        ),
        (
            "corona density",
            gas.corona_density().value(),
            0.5e-3,
            0.8e-3,
        ),
        ("pressure floor", gas.pressure_floor().value(), 300.0, 450.0),
        ("sigma_ln", gas.sigma_ln(), 1.0, 1.4),
        ("lane offset", gas.lane().offset().value(), 300.0, 600.0),
        ("lane width", gas.lane().width().value(), 150.0, 300.0),
        ("lane fraction", gas.lane().fraction(), 0.08, 0.20),
        (
            "neutral share",
            gas.neutral_fraction(),
            NEUTRAL_SHARE_FLOOR,
            1.0,
        ),
    ];
    for (what, value, lo, hi) in rows {
        assert!(
            (lo..=hi).contains(&value),
            "{seed}: {what} = {value} outside [{lo}, {hi}]"
        );
    }
    assert_eq!(gas.gas_mass(), galaxy.gas_disc().mass());
    assert_eq!(gas.radial_scale(), galaxy.gas_disc().length());
}

/// P07.T12 over 200 seeds' own galaxies: the smooth gas weighs plan 02's gas mass to 2%, every
/// parameter lies in its range, and the plane's pressure at 26,000 ly is positive and above the
/// floor.
///
/// Recorded, not held (ruling 91.6 and its research): each galaxy's mean-mode in-plane extinction
/// along the fixture's 64 lines, which T12's 0.9–2.0 mag does not bind, since it is the Milky Way's
/// rate, and the gas's surface density at 26,000 ly that goes with it, against the Milky Way's
/// 13.7 ± 1.6 M☉ pc⁻² (McKee et al. 2015); and each galaxy's escape speed at 26,000 ly to twice
/// its `r₂₀₀` (ruling 91.2), bracketed only loosely, at 300–1,100 km/s, since a drawn galaxy is
/// not the Milky Way: its stellar mass, and so its halo, is drawn over a range around it. Also
/// recorded (ruling 103): the spread of the hot share of each galaxy's plane at 26,000 ly, in the
/// split's closed form ([`hot_share_in_the_plane`]), since each galaxy's layers, pressure and
/// `σ_ln` differ.
#[test]
#[ignore = "slow: 200 galaxies' parameters, fields, potentials and 12,800 lines"]
fn the_gas_of_200_galaxies_closes_and_stays_in_range() {
    let mut cache = NoiseCache::with_capacity(4_096);
    let per_pc2 = SOLAR_MASSES_PER_LY3_AT_UNIT_DENSITY / CENTIMETRES_PER_LIGHT_YEAR
        * LIGHT_YEARS_PER_PARSEC
        * LIGHT_YEARS_PER_PARSEC;
    let mut rows = Vec::with_capacity(200);
    let mut escapes = Vec::with_capacity(200);
    let mut hot_shares = Vec::with_capacity(200);
    for n in 0..200_u64 {
        let seed = Seed::new(0x0712_0200_0000_0000 | n);
        let galaxy = GalaxyParams::from_seed(seed, MassFunctionKind::default());
        let model = MassModel::new(&galaxy);
        let fields = Fields::new(&galaxy, &model);
        let field = GasField::new(seed, &galaxy, &fields).expect("a drawn galaxy's gas");
        let params = *field.params();
        assert_gas_in_ranges(seed, &galaxy, &params);
        let smooth = SmoothGas::new(&params);
        let mass = smooth_gas_mass(&smooth, params.radial_scale().value());
        assert_relative("the gas mass", mass, params.gas_mass().value(), 0.02);
        let plane = Pressure::new(&params).at(&smooth, 26_000.0, 0.0);
        let floor = params.pressure_floor().value();
        assert!(
            floor > 0.0 && plane > floor,
            "{seed}: {plane} over a floor of {floor}"
        );
        let a_v = in_plane_mean(&field, &mut cache);
        rows.push((a_v, smooth.disc_column(26_000.0) * per_pc2));
        hot_shares.push(hot_share_in_the_plane(&params, 26_000.0));
        let tables = PotentialTables::in_plane(&model);
        let escape = tables
            .galactic_escape_speed_in_plane(LightYears::new(26_000.0))
            .value();
        escapes.push(escape);
    }
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    escapes.sort_by(f64::total_cmp);
    hot_shares.sort_by(f64::total_cmp);
    eprintln!(
        "the hot share of the plane at 26,000 ly over 200 galaxies, closed form: least {:.3}, \
         16/50/84% {:.3}/{:.3}/{:.3}, most {:.3}",
        hot_shares[0],
        percentile(&hot_shares, 16),
        percentile(&hot_shares, 50),
        percentile(&hot_shares, 84),
        hot_shares[199],
    );
    let (least, most) = (rows[0], rows[199]);
    let a_v: Vec<f64> = rows.iter().map(|row| row.0).collect();
    eprintln!(
        "in-plane A_V per 3,000 ly at 26,000 ly over 200 galaxies, mean mode: least {:.2} mag at \
         {:.1} M☉ pc⁻², 16/50/84% {:.2}/{:.2}/{:.2}, most {:.2} mag at {:.1} M☉ pc⁻² of gas; \
         escape speed to 2 r₂₀₀, 1/16/50/84/99%: {:.0}/{:.0}/{:.0}/{:.0}/{:.0} km/s",
        least.0,
        least.1,
        percentile(&a_v, 16),
        percentile(&a_v, 50),
        percentile(&a_v, 84),
        most.0,
        most.1,
        percentile(&escapes, 1),
        percentile(&escapes, 16),
        percentile(&escapes, 50),
        percentile(&escapes, 84),
        percentile(&escapes, 99),
    );
    assert_within("the least escape speed, km/s", escapes[0], 300.0, 1_100.0);
    assert_within(
        "the greatest escape speed, km/s",
        escapes[199],
        300.0,
        1_100.0,
    );
}

/// The hot share of the plane of `params`'s gas at radius `r` ly, azimuthally averaged, in the
/// four-phase split's closed form (ruling 103; `gas::phase`'s unit test holds it against a Monte
/// Carlo): `f_Hmin + room Φ(d₁) − K Φ(d₂)`, with `K` the smooth layers' balance volumes, `d₁ =
/// [ln(room ÷ K) + σ² ÷ 2] ÷ σ` and `d₂ = d₁ − σ`.
fn hot_share_in_the_plane(params: &GasParams, r: f64) -> f64 {
    let (gas, pressure) = (SmoothGas::new(params), Pressure::new(params));
    let p = pressure.at(&gas, r, 0.0);
    let warm = gas.plane_density(GasLayer::Warm, r);
    let neutral =
        gas.plane_density(GasLayer::Neutral, r) + gas.plane_density(GasLayer::Molecular, r);
    let k = warm * 2.1 * 8_000.0 / p + neutral * 1.1 * 8_000.0 / p;
    let least = 2.3 * gas.corona_density() * 1e5 / p;
    let room = 1.0 - least;
    let sigma = params.sigma_ln();
    let d1 = (math::ln(room / k) + 0.5 * sigma * sigma) / sigma;
    least + room * normal_cdf(d1) - k * normal_cdf(d1 - sigma)
}

/// The ten lines of P07.T12's golden: in the plane at the Sun, to the centre and through it, to
/// the pole, across the bar, along the outer disc, rising out of the disc, crossing the plane
/// steeply, a short line of 50 ly and one along a lane's radius.
const GOLDEN_LINES: [([f64; 3], [f64; 3]); 10] = [
    ([-1_500.0, 26_000.0, 0.0], [1_500.0, 26_000.0, 0.0]),
    ([26_000.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
    ([20_000.0, 5_000.0, 10.0], [-20_000.0, -5_000.0, -10.0]),
    ([0.0, 26_000.0, 0.0], [0.0, 26_000.0, 30_000.0]),
    ([-15_000.0, 3_000.0, -40.0], [15_000.0, -3_000.0, 40.0]),
    ([40_000.0, -10_000.0, 0.0], [30_000.0, 25_000.0, 100.0]),
    ([5_000.0, 26_000.0, 50.0], [9_000.0, 30_000.0, 4_000.0]),
    (
        [-26_000.0, 1_000.0, -2_000.0],
        [-24_000.0, -1_000.0, 2_000.0],
    ),
    ([100.0, 26_000.0, 3.0], [130.0, 26_040.0, -3.0]),
    ([12_000.0, 12_000.0, 0.0], [4_000.0, 4_000.0, 0.0]),
];

/// The two drawn galaxies P07.T12's golden pins, each with its own gas.
fn golden_gases() -> [GasField; 2] {
    [0x0712_0001_u64, 0x5eed_0712_0000_0002].map(|raw| {
        let seed = Seed::new(raw);
        let galaxy = GalaxyParams::from_seed(seed, MassFunctionKind::default());
        let fields = Fields::new(&galaxy, &MassModel::new(&galaxy));
        GasField::new(seed, &galaxy, &fields).expect("a drawn galaxy's gas")
    })
}

/// The two qualities P07.T12's golden pins.
fn golden_qualities() -> [(&'static str, Quality); 2] {
    let sixty_four = Quality::Budget(NonZeroU32::new(64).expect("sixty-four"));
    [("full", Quality::Full), ("budget64", sixty_four)]
}

/// The ten golden lines through `gas` at `quality`, in the seed's own clumpy gas, in the order
/// `order` gives, with `cache`.
fn golden_sightlines(
    gas: &GasField,
    quality: Quality,
    order: &[usize],
    cache: &mut NoiseCache,
) -> Vec<(usize, Sightline)> {
    order
        .iter()
        .map(|&i| {
            let (a, b) = GOLDEN_LINES[i];
            let line = sightline(
                gas,
                &at(a),
                &at(b),
                NoiseMode::Realised,
                quality,
                &[],
                cache,
            );
            (i, line)
        })
        .collect()
}

/// P07.T12's determinism golden (`gas/sightlines.golden`): ten realised sightlines through each of
/// two drawn galaxies' gas, at full quality and at a budget of 64 steps — their visual and
/// K-band extinction, reddening, hydrogen and neutral columns and steps.
#[test]
fn gas_sightlines_are_pinned() {
    let mut cache = NoiseCache::with_capacity(4_096);
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    let forward: Vec<usize> = (0..GOLDEN_LINES.len()).collect();
    for gas in &golden_gases() {
        for (quality_name, quality) in golden_qualities() {
            for (i, line) in golden_sightlines(gas, quality, &forward, &mut cache) {
                let label = |name: &str| format!("{}.line{i}.{quality_name}.{name}", gas.seed());
                w.f64(&label("a_v"), line.a_v().value());
                w.f64(&label("a_k"), line.in_band(Band::K).value());
                w.f64(&label("reddening"), line.reddening().value());
                w.f64(&label("hydrogen_column"), line.hydrogen_column().value());
                w.f64(
                    &label("neutral_column"),
                    line.neutral_hydrogen_column().value(),
                );
                w.line(&format!("{} = {}", label("steps"), line.steps()));
            }
        }
    }
    golden!("gas/sightlines", w.as_str());
}

/// P07.T12's order independence: the ten golden lines computed forwards with one cache, backwards
/// with another that already holds the first pass's lattice, and each alone with a fresh cache of
/// its own or of no capacity at all, agree bit for bit, in both galaxies at both qualities.
#[test]
fn gas_sightlines_do_not_depend_on_their_order_or_cache() {
    let forward: Vec<usize> = (0..GOLDEN_LINES.len()).collect();
    let backward: Vec<usize> = forward.iter().rev().copied().collect();
    for gas in &golden_gases() {
        for (_, quality) in golden_qualities() {
            let mut shared = NoiseCache::with_capacity(4_096);
            let once = golden_sightlines(gas, quality, &forward, &mut shared);
            let mut reversed = golden_sightlines(gas, quality, &backward, &mut shared);
            reversed.reverse();
            assert_eq!(once, reversed, "{quality:?}: the order moved a line");
            for &(i, line) in &once {
                let capacity = if i % 2 == 0 { 256 } else { 0 };
                let alone =
                    golden_sightlines(gas, quality, &[i], &mut NoiseCache::with_capacity(capacity));
                assert_eq!(alone, vec![(i, line)], "{quality:?}: line {i} alone");
            }
        }
    }
}
