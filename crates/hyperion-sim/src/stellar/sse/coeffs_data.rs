//! The coefficient tables of the analytic stellar evolution formulae, as printed (plan 06,
//! P06.T4.a).
//!
//! [`A`] and [`B`] are the Appendix of Hurley, Pols and Tout (2000, MNRAS 315, 543; "HPT"): each
//! coefficient aₙ and bₙ as a polynomial in ζ = log₁₀(Z ÷ 0.02). [`ZAMS_L`] and [`ZAMS_R`] are
//! Tables 1 and 2 of Tout et al. (1996, MNRAS 281, 257) for the zero-age main sequence. Each
//! literal keeps the paper's digits (m(e) printed as `me`), and a blank entry is `0.0`.
//!
//! Transcription: the tables were read from the papers' text by a script that assigns each number
//! to its column, and every number was then compared with the data statements of the published
//! SSE package (Hurley's `sse.tar.gz`, `zdata.h` and `zcnsts.f`), which all agree to the last
//! printed digit; no code of the package is copied. The special cases and derived coefficients
//! that the Appendix gives as closed forms are evaluated by
//! [`ZCoeffs::new`](super::coeffs::ZCoeffs::new). The checksum tests pin every bit, so an
//! accidental edit fails.

/// The coefficients aₙ of Hurley, Pols and Tout (2000, Appendix) as polynomials in
/// ζ = log₁₀(Z ÷ 0.02): row n holds (α, β, γ, η, µ) of
/// aₙ = α + βζ + γζ² + ηζ³ + µζ⁴.
///
/// Row 0 is unused. A row marked `primed` in its comment holds the table's primed coefficient
/// (such as a′₁₁ or b′₁₁), from which [`ZCoeffs`](super::coeffs::ZCoeffs) derives the one the
/// formulae use; a row marked `formula` is all zeros because the paper gives that coefficient as
/// a closed form instead (in the Appendix, or for b50, which also depends on mass, in section
/// 5.4), and a row marked `unused` names no coefficient.
#[expect(
    clippy::unreadable_literal,
    reason = "the digits as the paper prints them, to be checked against it by eye"
)]
pub(crate) const A: [[f64; 5]; 82] = [
    [0.0, 0.0, 0.0, 0.0, 0.0],                                   // unused
    [1.593890e3, 2.053038e3, 1.231226e3, 2.327785e2, 0.0],       // a1
    [2.706708e3, 1.483131e3, 5.772723e2, 7.411230e1, 0.0],       // a2
    [1.466143e2, -1.048442e2, -6.795374e1, -1.391127e1, 0.0],    // a3
    [4.141960e-2, 4.564888e-2, 2.958542e-2, 5.571483e-3, 0.0],   // a4
    [3.426349e-1, 0.0, 0.0, 0.0, 0.0],                           // a5
    [1.949814e1, 1.758178, -6.008212, -4.470533, 0.0],           // a6
    [4.903830, 0.0, 0.0, 0.0, 0.0],                              // a7
    [5.212154e-2, 3.166411e-2, -2.750074e-3, -2.271549e-3, 0.0], // a8
    [1.312179, -3.294936e-1, 9.231860e-2, 2.610989e-2, 0.0],     // a9
    [8.073972e-1, 0.0, 0.0, 0.0, 0.0],                           // a10
    [1.031538, -2.434480e-1, 7.732821, 6.460705, 1.374484],      // a11 primed
    [1.043715, -1.577474, -5.168234, -5.596506, -1.299394],      // a12 primed
    [7.859573e2, -8.542048, -2.642511e1, -9.585707, 0.0],        // a13
    [
        3.858911e3,
        2.459681e3,
        -7.630093e1,
        -3.486057e2,
        -4.861703e1,
    ], // a14
    [2.888720e2, 2.952979e2, 1.850341e2, 3.797254e1, 0.0],       // a15
    [7.196580, 5.613746e-1, 3.805871e-1, 8.398728e-2, 0.0],      // a16
    [0.0, 0.0, 0.0, 0.0, 0.0],                                   // a17 formula
    [2.187715e-1, -2.154437, -3.768678, -1.975518, -3.021475e-1], // a18 primed
    [1.466440, 1.839725, 6.442199, 4.023635, 6.957529e-1],       // a19 primed
    [2.652091e1, 8.178458e1, 1.156058e2, 7.633811e1, 1.950698e1], // a20
    [1.472103, -2.947609, -3.312828, -9.945065e-1, 0.0],         // a21
    [3.071048, -5.679941, -9.745523, -3.594543, 0.0],            // a22
    [2.617890, 1.019135, -3.292551e-2, -7.445123e-2, 0.0],       // a23
    [1.075567e-2, 1.773287e-2, 9.610479e-3, 1.732469e-3, 0.0],   // a24
    [1.476246, 1.899331, 1.195010, 3.035051e-1, 0.0],            // a25
    [5.502535, -6.601663e-2, 9.968707e-2, 3.599801e-2, 0.0],     // a26
    [9.511033e1, 6.819618e1, -1.045625e1, -1.474939e1, 0.0],     // a27
    [3.113458e1, 1.012033e1, -4.650511, -2.463185, 0.0],         // a28
    [1.413057, 4.578814e-1, -6.850581e-2, -5.588658e-2, 0.0],    // a29 primed
    [3.910862e1, 5.196646e1, 2.264970e1, 2.873680, 0.0],         // a30
    [4.597479, -2.855179e-1, 2.709724e-1, 0.0, 0.0],             // a31
    [6.682518, 2.827718e-1, -7.294429e-2, 0.0, 0.0],             // a32
    [0.0, 0.0, 0.0, 0.0, 0.0],                                   // a33 formula
    [1.910302e-1, 1.158624e-1, 3.348990e-2, 2.599706e-3, 0.0],   // a34
    [3.931056e-1, 7.277637e-2, -1.366593e-1, -4.508946e-2, 0.0], // a35
    [3.267776e-1, 1.204424e-1, 9.988332e-2, 2.455361e-2, 0.0],   // a36
    [5.990212e-1, 5.570264e-2, 6.207626e-2, 1.777283e-2, 0.0],   // a37
    [7.330122e-1, 5.192827e-1, 2.316416e-1, 8.346941e-3, 0.0],   // a38
    [1.172768, -1.209262e-1, -1.193023e-1, -2.859837e-2, 0.0],   // a39
    [3.982622e-1, -2.296279e-1, -2.262539e-1, -5.219837e-2, 0.0], // a40
    [3.571038, -2.223625e-2, -2.611794e-2, -6.359648e-3, 0.0],   // a41
    [1.9848, 1.1386, 3.5640e-1, 0.0, 0.0],                       // a42
    [6.300e-2, 4.810e-2, 9.840e-3, 0.0, 0.0],                    // a43
    [1.200, 2.450, 0.0, 0.0, 0.0],                               // a44
    [2.321400e-1, 1.828075e-3, -2.232007e-2, -3.378734e-3, 0.0], // a45
    [1.163659e-2, 3.427682e-3, 1.421393e-3, -3.710666e-3, 0.0],  // a46
    [1.048020e-2, -1.231921e-2, -1.686860e-2, -4.234354e-3, 0.0], // a47
    [1.555590, -3.223927e-1, -5.197429e-1, -1.066441e-1, 0.0],   // a48
    [9.7700e-2, -2.3100e-1, -7.5300e-2, 0.0, 0.0],               // a49
    [2.4000e-1, 1.8000e-1, 5.9500e-1, 0.0, 0.0],                 // a50
    [3.3000e-1, 1.3200e-1, 2.1800e-1, 0.0, 0.0],                 // a51
    [1.1064, 4.1500e-1, 1.8000e-1, 0.0, 0.0],                    // a52
    [1.1900, 3.7700e-1, 1.7600e-1, 0.0, 0.0],                    // a53
    [3.855707e-1, -6.104166e-1, 5.676742, 1.060894e1, 5.284014], // a54
    [3.579064e-1, -6.442936e-1, 5.494644, 1.054952e1, 5.280991], // a55
    [9.587587e-1, 8.777464e-1, 2.017321e-1, 0.0, 0.0],           // a56
    [1.5135, 3.7690e-1, 0.0, 0.0, 0.0],                          // a57
    [4.907546e-1, -1.683928e-1, -3.108742e-1, -7.202918e-2, 0.0], // a58
    [4.537070, -4.465455, -1.612690, -1.623246, 0.0],            // a59
    [1.796220, 2.814020e-1, 1.423325, 3.421036e-1, 0.0],         // a60
    [2.256216, 3.773400e-1, 1.537867, 4.396373e-1, 0.0],         // a61
    [8.4300e-2, -4.7500e-2, -3.5200e-2, 0.0, 0.0],               // a62
    [7.3600e-2, 7.4900e-2, 4.4260e-2, 0.0, 0.0],                 // a63
    [1.3600e-1, 3.5200e-2, 0.0, 0.0, 0.0],                       // a64
    [
        1.564231e-3,
        1.653042e-3,
        -4.439786e-3,
        -4.951011e-3,
        -1.216530e-3,
    ], // a65
    [1.4770, 2.9600e-1, 0.0, 0.0, 0.0],                          // a66
    [5.210157, -4.143695, -2.120870, 0.0, 0.0],                  // a67
    [1.1160, 1.6600e-1, 0.0, 0.0, 0.0],                          // a68
    [1.071489, -1.164852e-1, -8.623831e-2, -1.582349e-2, 0.0],   // a69
    [7.108492e-1, 7.935927e-1, 3.926983e-1, 3.622146e-2, 0.0],   // a70
    [3.478514, -2.585474e-2, -1.512955e-2, -2.833691e-3, 0.0],   // a71
    [9.132108e-1, -1.653695e-1, 0.0, 3.636784e-2, 0.0],          // a72
    [3.969331e-3, 4.539076e-3, 1.720906e-3, 1.897857e-4, 0.0],   // a73
    [1.600, 7.640e-1, 3.322e-1, 0.0, 0.0],                       // a74
    [8.109e-1, -6.282e-1, 0.0, 0.0, 0.0],                        // a75
    [1.192334e-2, 1.083057e-2, 1.230969, 1.551656, 0.0],         // a76
    [-1.668868e-1, 5.818123e-1, -1.105027e1, -1.668070e1, 0.0],  // a77
    [7.615495e-1, 1.068243e-1, -2.011333e-1, -9.371415e-2, 0.0], // a78
    [9.409838, 1.522928, 0.0, 0.0, 0.0],                         // a79
    [-2.7110e-1, -5.7560e-1, -8.3800e-2, 0.0, 0.0],              // a80
    [2.4930, 1.1475, 0.0, 0.0, 0.0],                             // a81
];

/// The coefficients bₙ of Hurley, Pols and Tout (2000, Appendix) as polynomials in
/// ζ = log₁₀(Z ÷ 0.02): row n holds (α, β, γ, η, µ) of
/// bₙ = α + βζ + γζ² + ηζ³ + µζ⁴.
///
/// Row 0 is unused. A row marked `primed` in its comment holds the table's primed coefficient
/// (such as a′₁₁ or b′₁₁), from which [`ZCoeffs`](super::coeffs::ZCoeffs) derives the one the
/// formulae use; a row marked `formula` is all zeros because the paper gives that coefficient as
/// a closed form instead (in the Appendix, or for b50, which also depends on mass, in section
/// 5.4), and a row marked `unused` names no coefficient.
#[expect(
    clippy::unreadable_literal,
    reason = "the digits as the paper prints them, to be checked against it by eye"
)]
pub(crate) const B: [[f64; 5]; 58] = [
    [0.0, 0.0, 0.0, 0.0, 0.0],                                   // unused
    [3.9700e-1, 2.8826e-1, 5.2930e-1, 0.0, 0.0],                 // b1
    [0.0, 0.0, 0.0, 0.0, 0.0],                                   // b2 formula
    [0.0, 0.0, 0.0, 0.0, 0.0],                                   // b3 formula
    [9.960283e-1, 8.164393e-1, 2.383830, 2.223436, 8.638115e-1], // b4
    [
        2.561062e-1,
        7.072646e-2,
        -5.444596e-2,
        -5.798167e-2,
        -1.349129e-2,
    ], // b5
    [1.157338, 1.467883, 4.299661, 3.130500, 6.992080e-1],       // b6
    [
        4.022765e-1,
        3.050010e-1,
        9.962137e-1,
        7.914079e-1,
        1.728098e-1,
    ], // b7
    [0.0, 0.0, 0.0, 0.0, 0.0],                                   // b8 unused
    [2.751631e3, 3.557098e2, 0.0, 0.0, 0.0],                     // b9
    [-3.820831e-2, 5.872664e-2, 0.0, 0.0, 0.0],                  // b10
    [1.071738e2, -8.970339e1, -3.949739e1, 0.0, 0.0],            // b11 primed
    [7.348793e2, -1.531020e2, -3.793700e1, 0.0, 0.0],            // b12
    [9.219293, -2.005865, -5.561309e-1, 0.0, 0.0],               // b13 primed
    [2.917412, 1.575290, 5.751814e-1, 0.0, 0.0],                 // b14 primed
    [3.629118, -9.112722e-1, 1.042291, 0.0, 0.0],                // b15
    [4.916389, 2.862149, 7.844850e-1, 0.0, 0.0],                 // b16 primed
    [0.0, 0.0, 0.0, 0.0, 0.0],                                   // b17 formula
    [5.496045e1, -1.289968e1, 6.385758, 0.0, 0.0],               // b18
    [1.832694, -5.766608e-2, 5.696128e-2, 0.0, 0.0],             // b19
    [1.211104e2, 0.0, 0.0, 0.0, 0.0],                            // b20
    [2.214088e2, 2.187113e2, 1.170177e1, -2.635340e1, 0.0],      // b21
    [2.063983, 7.363827e-1, 2.654323e-1, -6.140719e-2, 0.0],     // b22
    [2.003160, 9.388871e-1, 9.656450e-1, 2.362266e-1, 0.0],      // b23
    [1.609901e1, 7.391573, 2.277010e1, 8.334227, 0.0],           // b24 primed
    [1.747500e-1, 6.271202e-2, -2.324229e-2, -1.844559e-2, 0.0], // b25
    [0.0, 0.0, 0.0, 0.0, 0.0],                                   // b26 formula
    [2.752869, 2.729201e-2, 4.996927e-1, 2.496551e-1, 0.0],      // b27 primed
    [3.518506, 1.112440, -4.556216e-1, -2.179426e-1, 0.0],       // b28
    [1.626062e2, -1.168838e1, -5.498343, 0.0, 0.0],              // b29
    [3.336833e-1, -1.458043e-1, -2.011751e-2, 0.0, 0.0],         // b30
    [7.425137e1, 1.790236e1, 3.033910e1, 1.018259e1, 0.0],       // b31 primed
    [9.268325e2, -9.739859e1, -7.702152e1, -3.158268e1, 0.0],    // b32
    [2.474401, 3.892972e-1, 0.0, 0.0, 0.0],                      // b33
    [1.127018e1, 1.622158, -1.443664, -9.474699e-1, 0.0],        // b34 primed
    [0.0, 0.0, 0.0, 0.0, 0.0],                                   // b35 unused
    [1.445216e-1, -6.180219e-2, 3.093878e-2, 1.567090e-2, 0.0],  // b36 primed
    [1.304129, 1.395919e-1, 4.142455e-3, -9.732503e-3, 0.0],     // b37 primed
    [5.114149e-1, -1.160850e-2, 0.0, 0.0, 0.0],                  // b38 primed
    [1.314955e2, 2.009258e1, -5.143082e-1, -1.379140, 0.0],      // b39
    [1.823973e1, -3.074559, -4.307878, 0.0, 0.0],                // b40
    [2.327037, 2.403445, 1.208407, 2.087263e-1, 0.0],            // b41 primed
    [1.997378, -8.126205e-1, 0.0, 0.0, 0.0],                     // b42
    [1.079113e-1, 1.762409e-2, 1.096601e-2, 3.058818e-3, 0.0],   // b43
    [2.327409, 6.901582e-1, -2.158431e-1, -1.084117e-1, 0.0],    // b44 primed
    [0.0, 0.0, 0.0, 0.0, 0.0],                                   // b45 formula
    [2.214315, -1.975747, 0.0, 0.0, 0.0],                        // b46
    [0.0, 0.0, 0.0, 0.0, 0.0],                                   // b47 formula
    [5.072525, 1.146189e1, 6.961724, 1.316965, 0.0],             // b48
    [5.139740, 0.0, 0.0, 0.0, 0.0],                              // b49
    [0.0, 0.0, 0.0, 0.0, 0.0],                                   // b50 formula
    [1.125124, 1.306486, 3.622359, 2.601976, 3.031270e-1],       // b51 primed
    [
        3.349489e-1,
        4.531269e-3,
        1.131793e-1,
        2.300156e-1,
        7.632745e-2,
    ], // b52
    [1.467794, 2.798142, 9.455580, 8.963904, 3.339719],          // b53 primed
    [
        4.658512e-1,
        2.597451e-1,
        9.048179e-1,
        7.394505e-1,
        1.607092e-1,
    ], // b54
    [1.0422, 1.3156e-1, 4.5000e-2, 0.0, 0.0],                    // b55
    [1.110866, 9.623856e-1, 2.735487, 2.445602, 8.826352e-1],    // b56 primed
    [
        -1.584333e-1,
        -1.728865e-1,
        -4.461431e-1,
        -3.925259e-1,
        -1.276203e-1,
    ], // b57 primed
];

/// The zero-age main-sequence luminosity coefficients of Tout et al. (1996, MNRAS 281, 257).
///
/// Their Table 1: the polynomials of their equation 3, in ζ = log₁₀(Z ÷ 0.02), that give the
/// coefficients α, β, γ, δ, ε, ζ, η of their equation 1; each row is (a, b, c, d, e) of
/// a + bζ + cζ² + dζ³ + eζ⁴.
#[expect(
    clippy::unreadable_literal,
    reason = "the digits as the paper prints them, to be checked against it by eye"
)]
pub(crate) const ZAMS_L: [[f64; 5]; 7] = [
    [0.39704170, -0.32913574, 0.34776688, 0.37470851, 0.09011915], // α
    [
        8.52762600,
        -24.41225973,
        56.43597107,
        37.06152575,
        5.45624060,
    ], // β
    [0.00025546, -0.00123461, -0.00023246, 0.00045519, 0.00016176], // γ
    [
        5.43288900,
        -8.62157806,
        13.44202049,
        14.51584135,
        3.39793084,
    ], // δ
    [
        5.56357900,
        -10.32345224,
        19.44322980,
        18.97361347,
        4.16903097,
    ], // ε
    [0.78866060, -2.90870942, 6.54713531, 4.05606657, 0.53287322], // ζ
    [0.00586685, -0.01704237, 0.03872348, 0.02570041, 0.00383376], // η
];

/// The zero-age main-sequence radius coefficients of Tout et al. (1996).
///
/// Their Table 2: the polynomials of their equation 4, in ζ, that give the coefficients θ, ι, κ,
/// λ, µ, ν, ξ, ο, π of their equation 2. ν is fixed at its Z = 0.02 value for every metallicity,
/// as the paper states.
#[expect(
    clippy::unreadable_literal,
    reason = "the digits as the paper prints them, to be checked against it by eye"
)]
pub(crate) const ZAMS_R: [[f64; 5]; 9] = [
    [
        1.71535900,
        0.62246212,
        -0.92557761,
        -1.16996966,
        -0.30631491,
    ], // θ
    [
        6.59778800,
        -0.42450044,
        -12.13339427,
        -10.73509484,
        -2.51487077,
    ], // ι
    [
        10.08855000,
        -7.11727086,
        -31.67119479,
        -24.24848322,
        -5.33608972,
    ], // κ
    [
        1.01249500,
        0.32699690,
        -0.00923418,
        -0.03876858,
        -0.00412750,
    ], // λ
    [0.07490166, 0.02410413, 0.07233664, 0.03040467, 0.00197741], // µ
    [0.01077422, 0.0, 0.0, 0.0, 0.0],                             // ν
    [
        3.08223400,
        0.94472050,
        -2.15200882,
        -2.49219496,
        -0.63848738,
    ], // ξ
    [
        17.84778000,
        -7.45345690,
        -48.96066856,
        -40.05386135,
        -9.09331816,
    ], // ο
    [0.00022582, -0.00186899, 0.00388783, 0.00142402, -0.00007671], // π
];

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::{assert_same_bits, bits};

    use super::*;

    /// FNV-1a over the bits of every entry, row by row.
    fn checksum(rows: &[[f64; 5]]) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for value in rows.iter().flatten() {
            for byte in bits(*value).to_le_bytes() {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
            }
        }
        hash
    }

    #[test]
    fn every_table_is_pinned_by_its_checksum() {
        assert_eq!(checksum(&A), A_CHECKSUM, "A");
        assert_eq!(checksum(&B), B_CHECKSUM, "B");
        assert_eq!(checksum(&ZAMS_L), ZAMS_L_CHECKSUM, "ZAMS_L");
        assert_eq!(checksum(&ZAMS_R), ZAMS_R_CHECKSUM, "ZAMS_R");
    }

    /// The rows the Appendix gives as closed forms, and the numbers it never uses, are zero; every
    /// other row has a non-zero leading coefficient.
    #[test]
    fn only_the_closed_form_and_unused_rows_are_empty() {
        let empty_a = [0, 17, 33];
        let empty_b = [0, 2, 3, 8, 17, 26, 35, 45, 47, 50];
        for (n, row) in A.iter().enumerate() {
            let empty = row.iter().all(|&c| bits(c) == 0);
            assert_eq!(empty, empty_a.contains(&n), "a{n}");
            assert_eq!(bits(row[0]) != 0, !empty, "a{n}'s leading coefficient");
        }
        for (n, row) in B.iter().enumerate() {
            let empty = row.iter().all(|&c| bits(c) == 0);
            assert_eq!(empty, empty_b.contains(&n), "b{n}");
            assert_eq!(bits(row[0]) != 0, !empty, "b{n}'s leading coefficient");
        }
    }

    /// Spot checks against the printed tables: first and last rows, a row with an interior blank
    /// (a72's γ), and Tout's fixed ν.
    #[test]
    fn spot_values_match_the_printed_tables() {
        let same = |row: [f64; 5], printed: [f64; 5]| {
            for (x, y) in row.into_iter().zip(printed) {
                assert_same_bits(x, y);
            }
        };
        same(
            A[1],
            [1.593_890e3, 2.053_038e3, 1.231_226e3, 2.327_785e2, 0.0],
        );
        same(A[72], [9.132_108e-1, -1.653_695e-1, 0.0, 3.636_784e-2, 0.0]);
        same(A[81], [2.4930, 1.1475, 0.0, 0.0, 0.0]);
        same(
            B[57],
            [
                -1.584_333e-1,
                -1.728_865e-1,
                -4.461_431e-1,
                -3.925_259e-1,
                -1.276_203e-1,
            ],
        );
        same(
            ZAMS_L[0],
            [
                0.397_041_70,
                -0.329_135_74,
                0.347_766_88,
                0.374_708_51,
                0.090_119_15,
            ],
        );
        same(ZAMS_R[5], [0.010_774_22, 0.0, 0.0, 0.0, 0.0]);
    }

    const A_CHECKSUM: u64 = 0x5ad0_e6ad_bd03_cd74;
    const B_CHECKSUM: u64 = 0xeedc_b92a_6654_935c;
    const ZAMS_L_CHECKSUM: u64 = 0x3e60_4306_3fa1_b35e;
    const ZAMS_R_CHECKSUM: u64 = 0xdb81_27c1_0787_d222;
}
