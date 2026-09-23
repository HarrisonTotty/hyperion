//! The gas and dust field's slow statistical tests (plan 07), run by `just test-slow`.
//!
//! Each takes thousands of galaxies or millions of draws — a galaxy's parameters cost some 17 ms
//! even optimised, for plan 02's σ estimator — so they are marked slow and run under the slow-test
//! profile only; the fast suite runs the same checks on fewer seeds, in `galaxy::gas`'s own unit
//! tests and in `tests/gas.rs`.

#[expect(
    dead_code,
    reason = "the gas statistics use only the shared accumulators"
)]
mod common;

use hyperion_sim::Seed;
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::bounds::CellBox;
use hyperion_sim::galaxy::fields::Fields;
use hyperion_sim::galaxy::gas::field::GasField;
use hyperion_sim::galaxy::gas::noise::{NoiseCache, SmoothingScale, log_normal_factor};
use hyperion_sim::galaxy::gas::params::GasParams;
use hyperion_sim::galaxy::gas::pressure::Pressure;
use hyperion_sim::galaxy::gas::smooth::{GasLayer, SmoothGas};
use hyperion_sim::galaxy::imf::MassFunctionKind;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::potential::MassModel;
use hyperion_sim::math;
use hyperion_sim::units::LightYears;
use hyperion_testkit::lcg::Lcg;

use crate::common::{Running, whole_ly};

/// The least share of the gas the neutral disc may hold: half, the ruling's "a neutral layer that is
/// not most of the gas is not this galaxy" (plan 07, ruling 19 of 2026-09-22). The Milky Way's local
/// column is 87% atomic or molecular (McKee, Parravano and Hollenbach 2015, ApJ 814, 13, Table 2:
/// 11.9 of 13.7 M☉ pc⁻²) and the fixture's disc 85%. A seed below it is a finding to report, never a
/// share to clamp.
const NEUTRAL_SHARE_FLOOR: f64 = 0.5;

/// The neutral disc keeps most of every galaxy's gas once the warm ionised layer is drawn by its own
/// density at the Sun's radius and the molecular disc by its own mass (ruling 19), over 2,000 seeds
/// each drawn against its own galaxy — so the spread of plan 02's gas masses and scale lengths is in
/// the sweep, which is what moves the share. The warm layer holds its drawn density at the Sun's
/// radius for every seed.
///
/// The distribution of the share is printed, since the lightest galaxies set its floor: their warm
/// layer weighs what a Milky Way's does, some 10⁹ M☉, against a gas disc of a few 10⁹.
#[test]
#[ignore = "slow: 2,000 galaxies' parameters, about 15 s optimised"]
fn the_neutral_disc_holds_most_of_the_gas_over_2000_galaxies() {
    let reference = GasParams::REFERENCE_RADIUS.value();
    let mut shares = Vec::with_capacity(2_000);
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
        shares.push(share);
    }
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
