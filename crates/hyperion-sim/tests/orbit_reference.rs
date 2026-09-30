//! Orbits against independent solutions (plan 11, P11.T3.a; plan 14, P14.T2.a–b), recorded in
//! round 8's validation of plan 11.
//!
//! Each reference state was computed outside this crate, in 70-digit decimal arithmetic, from the
//! same elements read as exact binary values: Kepler's equation solved by bisection to 10⁻⁷⁰, the
//! hyperbolic one likewise, the parabola by Barker's equation, and the mean anomaly from the time
//! modulo the period as an exact rational. None of them shares code or method with `orbit`: no
//! starter, no Halley step, no Stumpff series. The propagation must agree with each to 10⁻¹³ of the
//! state's size; the measured worst is 1.1 × 10⁻¹⁴ for a bound orbit of e = 0.9998 a thousand
//! million periods from its epoch, where the phase's last bit is amplified by 1 ÷ (1 − e) near
//! periapsis, and 6 × 10⁻¹⁶ for every open orbit, parabola included.

use hyperion_sim::orbit::{Eccentricity, KeplerElements, OpenOrbit, Orientation};
use hyperion_sim::time::UniverseTime;
use hyperion_sim::units::{GravitationalParameter, Metres, Radians, Seconds, SolarMasses};

/// The orientation of every orbit here: i = 1.1, Ω = 2.3, ω = 4.4 rad.
fn orientation() -> Orientation {
    Orientation::new(Radians::new(1.1), Radians::new(2.3), Radians::new(4.4))
        .expect("angles inside their ranges")
}

/// μ of a pair of 1.7 M☉ in all.
fn mu() -> GravitationalParameter {
    GravitationalParameter::from_solar_masses(SolarMasses::new(1.7))
}

/// Whether `state` (position then velocity) is `reference` to 10⁻¹³ of each vector's length.
fn assert_state(what: &str, position: [f64; 3], velocity: [f64; 3], reference: [f64; 6]) {
    let norm = |v: &[f64]| v.iter().map(|x| x * x).sum::<f64>().sqrt();
    let (r, v) = (&reference[..3], &reference[3..]);
    for (got, want, scale) in [(position, r, norm(r)), (velocity, v, norm(v))] {
        for (g, w) in got.iter().zip(want) {
            assert!(
                (g - w).abs() <= 1e-13 * scale,
                "{what}: {got:?} against {want:?}"
            );
        }
    }
}

#[test]
fn an_eccentric_orbit_a_thousand_million_periods_out_is_the_independent_solution() {
    let orbit = KeplerElements::from_period(
        Seconds::new(86_400.123_456_789),
        mu(),
        Eccentricity::new(0.9998).expect("bound"),
        orientation(),
        Radians::new(0.7),
    )
    .expect("a valid orbit");
    // 10⁹ periods and a little, 2.7 million years from the epoch.
    let t = UniverseTime::new(86_400_123_456_789, 0).expect("a valid time");
    let (r, v) = orbit.relative_state_at(t);
    assert_state(
        "e = 0.9998 at 10⁹ periods",
        r.metres(),
        v.metres_per_second(),
        [
            -2.099_839_837_806_005_7e9,
            -1.731_639_621_550_654e8,
            3.303_225_664_272_815_7e9,
            -1.181_732_123_901_744_3e5,
            -1.358_087_997_999_415_9e4,
            1.909_176_510_224_101_4e5,
        ],
    );
}

#[test]
fn open_orbits_either_side_of_the_parabola_are_the_independent_solutions() {
    // (e, seconds, nanoseconds from the pericentre at the epoch, reference state)
    let cases: [(f64, i64, u32, [f64; 6]); 5] = [
        (
            0.999_999_9,
            3_155_760_000,
            0,
            [
                -1.229_699_468_737_466_4e13,
                2.129_733_189_397_826_5e11,
                1.773_791_702_067_906e13,
                -2.508_597_080_783_84e3,
                -1.110_851_448_386_777_3e2,
                3.820_843_126_908_305e3,
            ],
        ),
        (
            1.0,
            3_155_760_000,
            0,
            [
                -1.229_707_298_630_487_7e13,
                2.129_908_320_633_038_3e11,
                1.773_800_881_296_550_4e13,
                -2.508_628_717_603_381_6e3,
                -1.110_803_207_586_560_7e2,
                3.820_883_163_940_440_7e3,
            ],
        ),
        (
            1.000_000_001,
            31_557_600,
            0,
            [
                -6.692_979_735_815_526e11,
                2.529_336_092_342_133_5e11,
                6.495_011_871_820_35e11,
                -1.344_694_784_615_047_3e4,
                2.242_509_885_085_622e3,
                1.676_593_964_334_812e4,
            ],
        ),
        (
            1.2,
            -315_576_000,
            5,
            [
                -1.576_499_255_508_157e12,
                -5.718_376_019_362_723e12,
                9.795_546_091_923_264e12,
                5.049_382_030_013_243e3,
                1.705_122_087_581_752_2e4,
                -2.971_928_878_343_113e4,
            ],
        ),
        (
            3.0,
            -86_400,
            0,
            [
                2.714_203_849_179_314_4e10,
                -8.272_492_289_584_504e9,
                -2.893_736_596_076_297_8e10,
                -7.317_508_123_452_768e4,
                1.213_150_896_455_413_5e5,
                -5.159_909_923_993_912_5e4,
            ],
        ),
    ];
    for (e, seconds, nanos, reference) in cases {
        let orbit = OpenOrbit::new(
            Metres::new(1.496e11 * 0.26),
            e,
            orientation(),
            UniverseTime::EPOCH,
            mu(),
        )
        .expect("a valid open orbit");
        let t = UniverseTime::new(seconds, nanos).expect("a valid time");
        let (r, v) = orbit.relative_state_at(t);
        assert_state(
            &format!("e = {e} at {seconds} s"),
            r.metres(),
            v.metres_per_second(),
            reference,
        );
    }
}
