//! Gauss–Legendre nodes and weights on `[−1, 1]`, 16 and 32 points.
//!
//! An n-point rule integrates every polynomial of degree up to 2n − 1 exactly. The nodes are the
//! roots of the Legendre polynomial Pₙ and the weights `2 ÷ ((1 − x²) Pₙ′(x)²)` (Abramowitz and
//! Stegun 1964, §25.4.29). Each literal is the correctly rounded `f64` of the value computed at 50
//! significant digits with `mpmath` by Newton iteration on Pₙ, and prints as the shortest decimal
//! that round-trips. Nodes ascend from −1 to +1, and the tables are exactly symmetric: node `i` is
//! the negation of node `n − 1 − i`, and the two weights are equal.
//!
//! The order of the entries is part of the generator version, because the quadratures in
//! [`galaxy::quad`](crate::galaxy::quad) sum in index order.

/// The 16 nodes of the Gauss–Legendre rule on `[−1, 1]`, ascending.
pub const GL16_NODES: [f64; 16] = [
    -0.989_400_934_991_649_9,
    -0.944_575_023_073_232_6,
    -0.865_631_202_387_831_8,
    -0.755_404_408_355_003,
    -0.617_876_244_402_643_8,
    -0.458_016_777_657_227_37,
    -0.281_603_550_779_258_9,
    -0.095_012_509_837_637_44,
    0.095_012_509_837_637_44,
    0.281_603_550_779_258_9,
    0.458_016_777_657_227_37,
    0.617_876_244_402_643_8,
    0.755_404_408_355_003,
    0.865_631_202_387_831_8,
    0.944_575_023_073_232_6,
    0.989_400_934_991_649_9,
];

/// The 16 weights of the Gauss–Legendre rule on `[−1, 1]`, in the order of [`GL16_NODES`].
pub const GL16_WEIGHTS: [f64; 16] = [
    0.027_152_459_411_754_096,
    0.062_253_523_938_647_894,
    0.095_158_511_682_492_79,
    0.124_628_971_255_533_88,
    0.149_595_988_816_576_74,
    0.169_156_519_395_002_54,
    0.182_603_415_044_923_58,
    0.189_450_610_455_068_5,
    0.189_450_610_455_068_5,
    0.182_603_415_044_923_58,
    0.169_156_519_395_002_54,
    0.149_595_988_816_576_74,
    0.124_628_971_255_533_88,
    0.095_158_511_682_492_79,
    0.062_253_523_938_647_894,
    0.027_152_459_411_754_096,
];

/// The 32 nodes of the Gauss–Legendre rule on `[−1, 1]`, ascending.
pub const GL32_NODES: [f64; 32] = [
    -0.997_263_861_849_481_6,
    -0.985_611_511_545_268_4,
    -0.964_762_255_587_506_4,
    -0.934_906_075_937_739_7,
    -0.896_321_155_766_052_1,
    -0.849_367_613_732_57,
    -0.794_483_795_967_942_4,
    -0.732_182_118_740_289_7,
    -0.663_044_266_930_215_2,
    -0.587_715_757_240_762_3,
    -0.506_899_908_932_229_4,
    -0.421_351_276_130_635_33,
    -0.331_868_602_282_127_67,
    -0.239_287_362_252_137_06,
    -0.144_471_961_582_796_5,
    -0.048_307_665_687_738_32,
    0.048_307_665_687_738_32,
    0.144_471_961_582_796_5,
    0.239_287_362_252_137_06,
    0.331_868_602_282_127_67,
    0.421_351_276_130_635_33,
    0.506_899_908_932_229_4,
    0.587_715_757_240_762_3,
    0.663_044_266_930_215_2,
    0.732_182_118_740_289_7,
    0.794_483_795_967_942_4,
    0.849_367_613_732_57,
    0.896_321_155_766_052_1,
    0.934_906_075_937_739_7,
    0.964_762_255_587_506_4,
    0.985_611_511_545_268_4,
    0.997_263_861_849_481_6,
];

/// The 32 weights of the Gauss–Legendre rule on `[−1, 1]`, in the order of [`GL32_NODES`].
pub const GL32_WEIGHTS: [f64; 32] = [
    0.007_018_610_009_470_096,
    0.016_274_394_730_905_67,
    0.025_392_065_309_262_06,
    0.034_273_862_913_021_43,
    0.042_835_898_022_226_68,
    0.050_998_059_262_376_175,
    0.058_684_093_478_535_544,
    0.065_822_222_776_361_85,
    0.072_345_794_108_848_5,
    0.078_193_895_787_070_31,
    0.083_311_924_226_946_75,
    0.087_652_093_004_403_81,
    0.091_173_878_695_763_89,
    0.093_844_399_080_804_57,
    0.095_638_720_079_274_86,
    0.096_540_088_514_727_8,
    0.096_540_088_514_727_8,
    0.095_638_720_079_274_86,
    0.093_844_399_080_804_57,
    0.091_173_878_695_763_89,
    0.087_652_093_004_403_81,
    0.083_311_924_226_946_75,
    0.078_193_895_787_070_31,
    0.072_345_794_108_848_5,
    0.065_822_222_776_361_85,
    0.058_684_093_478_535_544,
    0.050_998_059_262_376_175,
    0.042_835_898_022_226_68,
    0.034_273_862_913_021_43,
    0.025_392_065_309_262_06,
    0.016_274_394_730_905_67,
    0.007_018_610_009_470_096,
];

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;

    fn assert_symmetric(nodes: &[f64], weights: &[f64]) {
        let n = nodes.len();
        assert_eq!(weights.len(), n);
        for i in 0..n {
            assert_same_bits(nodes[i], -nodes[n - 1 - i]);
            assert_same_bits(weights[i], weights[n - 1 - i]);
            assert!(weights[i] > 0.0);
            if i > 0 {
                assert!(nodes[i] > nodes[i - 1], "nodes ascend");
            }
        }
    }

    #[test]
    fn tables_are_ascending_and_exactly_symmetric() {
        assert_symmetric(&GL16_NODES, &GL16_WEIGHTS);
        assert_symmetric(&GL32_NODES, &GL32_WEIGHTS);
    }

    #[test]
    fn weights_sum_to_two() {
        for weights in [&GL16_WEIGHTS[..], &GL32_WEIGHTS[..]] {
            let sum: f64 = weights.iter().sum();
            assert!((sum - 2.0).abs() < 4.0 * f64::EPSILON, "sum = {sum}");
        }
    }

    /// Each node is a root of its Legendre polynomial, evaluated by the three-term recurrence. The
    /// tolerance allows for the rounding of the node itself times the slope of P₃₂ near ±1, which
    /// is some hundreds.
    #[test]
    fn nodes_are_roots_of_the_legendre_polynomial() {
        fn legendre(n: u32, x: f64) -> f64 {
            let (mut p0, mut p1) = (1.0, x);
            for k in 2..=n {
                let k = f64::from(k);
                (p0, p1) = (p1, ((2.0 * k - 1.0) * x * p1 - (k - 1.0) * p0) / k);
            }
            p1
        }
        for &x in &GL16_NODES {
            assert!(legendre(16, x).abs() < 1e-12, "P16({x})");
        }
        for &x in &GL32_NODES {
            assert!(legendre(32, x).abs() < 1e-12, "P32({x})");
        }
    }
}
