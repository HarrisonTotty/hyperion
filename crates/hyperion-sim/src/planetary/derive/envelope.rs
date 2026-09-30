//! Hydrogen and helium envelopes: the radius of a body whose core carries a primordial envelope
//! (plan 14, P14.T11.b; design note 8).
//!
//! A planet with an envelope is its core plus the envelope's thickness, R = `R_core` + `ΔR_env`, the
//! decomposition in which Lopez and Fortney (2014, ApJ 792, 1, §3.1) describe their models. The
//! core is [`radius_zeng`] of the core's own mass and composition; the thickness comes from two
//! published grids of thermal-evolution models, read as tables:
//!
//! - **Up to 20% envelope**, Lopez and Fortney's (2014) Tables 2–4: the radius of 1–20 M⊕ planets
//!   with Earth-like cores and solar-metallicity envelopes of 0.01–20% of their mass, receiving
//!   0.1, 10 and 1,000 times Earth's flux, at 100 Myr, 1 Gyr and 10 Gyr. The thickness is their
//!   radius less their own core, whose radius they give as (`M_core` ÷ M⊕)^¼ R⊕ to about 2% (their
//!   eq. 1). Plan 14 asked for "0.1–5% by mass"; the tables' full range is kept because Chen and
//!   Kipping's Neptunian radii need it (Neptune itself comes out near 12%).
//! - **From 20% to a coreless planet**, the log of the radius runs linearly in the log of the
//!   envelope fraction from the 20% radius to the radius of a coreless planet of the same mass,
//!   flux and age, from Fortney, Marley and Barnes's (2007, ApJ 659, 1661) Tables 2–4 (17–215 M⊕,
//!   0.045–9.5 au from the Sun, 300 Myr, 1 Gyr and 4.5 Gyr). This bridge is this module's own
//!   construction, not either paper's: it stays within 10% of Fortney et al.'s models with 10–50
//!   M⊕ cores of ice and rock (tested), and it puts Saturn's radius at an envelope of 82%.
//!
//! Lopez and Fortney's models have Earth-like cores. On a core of another composition, such as
//! the icy cores of bodies formed beyond the snow line, the same thickness is laid on that core's
//! own radius; they put the error of varying the core's iron alone at about 10% (their §3.1).
//!
//! Every table is interpolated linearly in radius against the logarithms of mass, envelope
//! fraction, flux and age. Flux and age are held at the grids' edges outside them. Mass outside
//! 1–20 M⊕ scales Lopez and Fortney's thickness as M^−0.21, the mass exponent of their fitted power
//! law (their eq. 3), and outside 17–215 M⊕ the coreless radius is held at its edge. Below 0.01%
//! the thickness falls linearly to zero with the fraction, so that a body with no envelope is its
//! bare core.
//!
//! The radius rises with the envelope fraction at every age from 1 Gyr on, but for dips of at most
//! 1.1 × 10⁻⁵ of the radius where two neighbouring entries of Lopez and Fortney's tables are equal,
//! as printed to two decimals, and the core shrinks faster than their (`M_core`)^¼ over the step; the
//! bisection of [`composition`](mod@super::composition) needs only continuity. At 100 Myr their
//! radius dips between 0.01% and 0.02% for 1–8.5 M⊕ at 0.1 and 10 F⊕ (Table 2), as printed.

use crate::math;
use crate::planetary::derive::radius::{CoreComposition, radius_zeng};
use crate::units::{EarthFluxes, EarthMasses, EarthRadii, Gigayears, JupiterRadii};

/// The smallest envelope fraction Lopez and Fortney (2014) tabulate, 0.01%.
pub const SMALLEST_TABULATED_FRACTION: f64 = 1e-4;

/// The largest envelope fraction Lopez and Fortney (2014) tabulate, 20%; beyond it the radius
/// bridges to a coreless planet's.
pub const LARGEST_TABULATED_FRACTION: f64 = 0.2;

/// How the thickness of an envelope scales with the planet's mass outside Lopez and Fortney's 1–20
/// M⊕: as M^−0.21 (their eq. 3, the fit to their enhanced-opacity models; they give no separate
/// mass exponent for solar metallicity).
pub const ENVELOPE_MASS_EXPONENT: f64 = -0.21;

/// Lopez and Fortney's ages, Gyr.
const LF_AGES: [f64; 3] = [0.1, 1.0, 10.0];

/// Lopez and Fortney's fluxes, F⊕.
const LF_FLUXES: [f64; 3] = [0.1, 10.0, 1_000.0];

/// Lopez and Fortney's total planet masses, M⊕.
const LF_MASSES: [f64; 8] = [1.0, 1.5, 2.4, 3.6, 5.5, 8.5, 13.0, 20.0];

/// Lopez and Fortney's envelope mass fractions.
const LF_FRACTIONS: [f64; 11] = [
    1e-4, 2e-4, 5e-4, 1e-3, 2e-3, 5e-3, 0.01, 0.02, 0.05, 0.1, 0.2,
];

/// Lopez and Fortney's (2014) Tables 2, 3 and 4, "Low mass planet radii" at solar metallicity:
/// radius (R⊕) by age, flux, total mass and envelope fraction, in the order of [`LF_AGES`],
/// [`LF_FLUXES`], [`LF_MASSES`] and [`LF_FRACTIONS`], as printed (arXiv:1311.0329).
const LOPEZ_FORTNEY_RADII: [[[[f64; 11]; 8]; 3]; 3] = [
    // 100 Myr (Table 2)
    [
        // 0.1 F⊕
        [
            [
                1.22, 1.16, 1.18, 1.21, 1.32, 1.65, 2.17, 2.75, 4.32, 6.81, 11.7,
            ], // 1 M⊕
            [
                1.3, 1.24, 1.26, 1.3, 1.4, 1.71, 2.15, 2.65, 3.97, 6.18, 10.6,
            ], // 1.5 M⊕
            [
                1.41, 1.36, 1.4, 1.42, 1.53, 1.79, 2.17, 2.58, 3.66, 5.36, 9.05,
            ], // 2.4 M⊕
            [
                1.53, 1.49, 1.51, 1.54, 1.64, 1.89, 2.21, 2.56, 3.49, 4.93, 7.86,
            ], // 3.6 M⊕
            [
                1.66, 1.63, 1.66, 1.69, 1.79, 2.01, 2.28, 2.6, 3.37, 4.58, 6.96,
            ], // 5.5 M⊕
            [
                1.81, 1.79, 1.82, 1.85, 1.95, 2.14, 2.39, 2.67, 3.36, 4.35, 6.32,
            ], // 8.5 M⊕
            [
                1.97, 1.97, 1.98, 2.02, 2.11, 2.3, 2.52, 2.78, 3.41, 4.29, 5.94,
            ], // 13 M⊕
            [
                2.15, 2.15, 2.17, 2.2, 2.29, 2.47, 2.67, 2.93, 3.52, 4.32, 5.75,
            ], // 20 M⊕
        ],
        // 10 F⊕
        [
            [
                1.32, 1.24, 1.27, 1.31, 1.44, 1.82, 2.4, 3.06, 4.72, 7.13, 11.1,
            ], // 1 M⊕
            [
                1.36, 1.32, 1.35, 1.38, 1.5, 1.84, 2.32, 2.88, 4.31, 6.47, 10.4,
            ], // 1.5 M⊕
            [
                1.46, 1.43, 1.48, 1.5, 1.59, 1.88, 2.26, 2.71, 3.88, 5.67, 9.14,
            ], // 2.4 M⊕
            [
                1.57, 1.55, 1.58, 1.6, 1.71, 1.95, 2.27, 2.64, 3.61, 5.13, 8.11,
            ], // 3.6 M⊕
            [
                1.69, 1.68, 1.71, 1.73, 1.84, 2.05, 2.33, 2.66, 3.46, 4.7, 7.13,
            ], // 5.5 M⊕
            [
                1.84, 1.83, 1.86, 1.89, 1.98, 2.18, 2.43, 2.72, 3.42, 4.43, 6.39,
            ], // 8.5 M⊕
            [
                1.99, 2.01, 2.02, 2.05, 2.14, 2.32, 2.55, 2.82, 3.46, 4.35, 5.96,
            ], // 13 M⊕
            [
                2.17, 2.18, 2.19, 2.23, 2.31, 2.49, 2.69, 2.95, 3.56, 4.37, 5.77,
            ], // 20 M⊕
        ],
        // 1,000 F⊕
        [
            [
                1.59, 1.63, 1.7, 1.75, 1.83, 2.3, 3.12, 3.99, 6.21, 8.88, 11.3,
            ], // 1 M⊕
            [
                1.63, 1.67, 1.72, 1.77, 1.89, 2.31, 3.02, 3.83, 6.01, 9.41, 14.0,
            ], // 1.5 M⊕
            [
                1.7, 1.72, 1.77, 1.81, 1.93, 2.32, 2.9, 3.55, 5.35, 8.59, 15.4,
            ], // 2.4 M⊕
            [
                1.77, 1.79, 1.83, 1.87, 1.99, 2.34, 2.81, 3.36, 4.82, 7.27, 13.4,
            ], // 3.6 M⊕
            [
                1.87, 1.88, 1.92, 1.96, 2.08, 2.37, 2.76, 3.22, 4.39, 6.25, 10.3,
            ], // 5.5 M⊕
            [
                1.99, 2.0, 2.03, 2.08, 2.19, 2.5, 2.76, 3.15, 4.12, 5.56, 8.48,
            ], // 8.5 M⊕
            [
                2.12, 2.12, 2.15, 2.21, 2.31, 2.58, 2.81, 3.16, 3.99, 5.18, 7.43,
            ], // 13 M⊕
            [
                2.27, 2.27, 2.3, 2.35, 2.45, 2.68, 2.9, 3.21, 3.94, 4.97, 6.8,
            ], // 20 M⊕
        ],
    ],
    // 1 Gyr (Table 3)
    [
        // 0.1 F⊕
        [
            [
                1.07, 1.09, 1.12, 1.15, 1.28, 1.55, 1.79, 2.13, 2.98, 4.26, 6.74,
            ], // 1 M⊕
            [
                1.18, 1.19, 1.22, 1.26, 1.38, 1.62, 1.82, 2.13, 2.87, 3.96, 6.1,
            ], // 1.5 M⊕
            [
                1.32, 1.33, 1.36, 1.39, 1.52, 1.72, 1.9, 2.16, 2.81, 3.75, 5.52,
            ], // 2.4 M⊕
            [
                1.45, 1.46, 1.49, 1.52, 1.65, 1.82, 1.99, 2.23, 2.81, 3.65, 5.21,
            ], // 3.6 M⊕
            [
                1.6, 1.61, 1.64, 1.67, 1.79, 1.95, 2.11, 2.34, 2.87, 3.62, 5.0,
            ], // 5.5 M⊕
            [
                1.77, 1.78, 1.8, 1.83, 1.94, 2.1, 2.25, 2.47, 2.97, 3.67, 4.91,
            ], // 8.5 M⊕
            [
                1.94, 1.95, 1.97, 2.0, 2.11, 2.25, 2.4, 2.61, 3.11, 3.77, 4.92,
            ], // 13 M⊕
            [
                2.12, 2.13, 2.16, 2.19, 2.3, 2.42, 2.57, 2.78, 3.28, 3.92, 5.0,
            ], // 20 M⊕
        ],
        // 10 F⊕
        [
            [
                1.18, 1.2, 1.23, 1.27, 1.47, 1.81, 2.12, 2.58, 3.63, 5.07, 7.45,
            ], // 1 M⊕
            [
                1.27, 1.29, 1.32, 1.36, 1.52, 1.82, 2.08, 2.47, 3.4, 4.68, 6.96,
            ], // 1.5 M⊕
            [
                1.4, 1.41, 1.44, 1.48, 1.63, 1.86, 2.08, 2.41, 3.18, 4.26, 6.24,
            ], // 2.4 M⊕
            [
                1.51, 1.53, 1.55, 1.59, 1.72, 1.93, 2.12, 2.4, 3.07, 4.02, 5.73,
            ], // 3.6 M⊕
            [
                1.65, 1.66, 1.69, 1.72, 1.85, 2.02, 2.19, 2.45, 3.04, 3.86, 5.34,
            ], // 5.5 M⊕
            [
                1.81, 1.82, 1.84, 1.88, 1.99, 2.15, 2.31, 2.54, 3.08, 3.81, 5.09,
            ], // 8.5 M⊕
            [
                1.97, 1.98, 2.01, 2.04, 2.15, 2.29, 2.44, 2.67, 3.18, 3.86, 5.02,
            ], // 13 M⊕
            [
                2.15, 2.16, 2.18, 2.22, 2.32, 2.45, 2.6, 2.82, 3.33, 3.99, 5.07,
            ], // 20 M⊕
        ],
        // 1,000 F⊕
        [
            [
                1.61, 1.65, 1.71, 1.77, 1.81, 2.15, 2.5, 3.01, 4.24, 6.04, 8.75,
            ], // 1 M⊕
            [
                1.65, 1.68, 1.73, 1.78, 1.87, 2.18, 2.5, 2.98, 4.14, 5.91, 9.34,
            ], // 1.5 M⊕
            [
                1.71, 1.73, 1.78, 1.82, 1.93, 2.21, 2.5, 2.91, 3.93, 5.5, 8.76,
            ], // 2.4 M⊕
            [
                1.78, 1.8, 1.84, 1.87, 1.99, 2.24, 2.5, 2.87, 3.77, 5.11, 7.86,
            ], // 3.6 M⊕
            [
                1.87, 1.89, 1.92, 1.94, 2.1, 2.3, 2.52, 2.85, 3.65, 4.79, 7.0,
            ], // 5.5 M⊕
            [
                1.99, 2.0, 2.02, 2.05, 2.19, 2.38, 2.58, 2.88, 3.59, 4.58, 6.39,
            ], // 8.5 M⊕
            [
                2.12, 2.13, 2.15, 2.19, 2.31, 2.48, 2.66, 2.94, 3.59, 4.48, 6.05,
            ], // 13 M⊕
            [
                2.27, 2.27, 2.29, 2.34, 2.45, 2.61, 2.78, 3.04, 3.65, 4.47, 5.85,
            ], // 20 M⊕
        ],
    ],
    // 10 Gyr (Table 4)
    [
        // 0.1 F⊕
        [
            [
                1.08, 1.1, 1.13, 1.17, 1.22, 1.37, 1.53, 1.75, 2.25, 2.94, 4.14,
            ], // 1 M⊕
            [
                1.19, 1.2, 1.23, 1.27, 1.31, 1.45, 1.6, 1.81, 2.28, 2.93, 4.05,
            ], // 1.5 M⊕
            [
                1.32, 1.34, 1.37, 1.4, 1.45, 1.58, 1.71, 1.9, 2.35, 2.95, 3.98,
            ], // 2.4 M⊕
            [
                1.45, 1.47, 1.49, 1.53, 1.58, 1.7, 1.82, 2.01, 2.44, 3.01, 3.97,
            ], // 3.6 M⊕
            [
                1.6, 1.62, 1.64, 1.67, 1.75, 1.84, 1.96, 2.15, 2.56, 3.11, 4.03,
            ], // 5.5 M⊕
            [
                1.77, 1.78, 1.8, 1.84, 1.91, 2.0, 2.13, 2.31, 2.72, 3.25, 4.14,
            ], // 8.5 M⊕
            [
                1.94, 1.95, 1.97, 2.0, 2.09, 2.17, 2.3, 2.48, 2.9, 3.44, 4.31,
            ], // 13 M⊕
            [
                2.12, 2.14, 2.16, 2.19, 2.25, 2.36, 2.49, 2.68, 3.1, 3.65, 4.53,
            ], // 20 M⊕
        ],
        // 10 F⊕
        [
            [
                1.23, 1.25, 1.28, 1.31, 1.44, 1.68, 1.87, 2.17, 2.84, 3.7, 5.11,
            ], // 1 M⊕
            [
                1.31, 1.33, 1.36, 1.4, 1.49, 1.72, 1.9, 2.19, 2.83, 3.66, 5.03,
            ], // 1.5 M⊕
            [
                1.43, 1.44, 1.47, 1.51, 1.6, 1.78, 1.96, 2.21, 2.8, 3.58, 4.89,
            ], // 2.4 M⊕
            [
                1.54, 1.55, 1.58, 1.62, 1.73, 1.87, 2.03, 2.27, 2.81, 3.53, 4.75,
            ], // 3.6 M⊕
            [
                1.67, 1.69, 1.71, 1.75, 1.85, 1.98, 2.13, 2.35, 2.86, 3.52, 4.64,
            ], // 5.5 M⊕
            [
                1.82, 1.84, 1.86, 1.9, 1.98, 2.11, 2.25, 2.47, 2.95, 3.58, 4.61,
            ], // 8.5 M⊕
            [
                1.98, 1.99, 2.02, 2.05, 2.13, 2.26, 2.4, 2.61, 3.07, 3.68, 4.66,
            ], // 13 M⊕
            [
                2.16, 2.17, 2.2, 2.23, 2.32, 2.43, 2.56, 2.77, 3.23, 3.83, 4.77,
            ], // 20 M⊕
        ],
        // 1,000 F⊕
        [
            [
                1.76, 1.81, 1.88, 1.96, 2.01, 2.08, 2.18, 2.31, 2.7, 3.49, 4.88,
            ], // 1 M⊕
            [
                1.77, 1.81, 1.88, 1.94, 1.99, 2.08, 2.17, 2.33, 2.91, 3.76, 5.36,
            ], // 1.5 M⊕
            [
                1.82, 1.85, 1.9, 1.95, 2.0, 2.08, 2.22, 2.49, 3.1, 3.94, 5.55,
            ], // 2.4 M⊕
            [
                1.87, 1.9, 1.94, 1.98, 2.03, 2.12, 2.3, 2.58, 3.2, 4.03, 5.54,
            ], // 3.6 M⊕
            [
                1.95, 1.97, 2.01, 2.04, 2.1, 2.21, 2.38, 2.64, 3.26, 4.08, 5.49,
            ], // 5.5 M⊕
            [
                2.05, 2.07, 2.1, 2.12, 2.19, 2.31, 2.48, 2.73, 3.31, 4.1, 5.44,
            ], // 8.5 M⊕
            [
                2.17, 2.18, 2.21, 2.23, 2.34, 2.43, 2.59, 2.83, 3.38, 4.13, 5.4,
            ], // 13 M⊕
            [
                2.31, 2.32, 2.34, 2.36, 2.47, 2.57, 2.72, 2.95, 3.49, 4.2, 5.38,
            ], // 20 M⊕
        ],
    ],
];

/// Fortney, Marley and Barnes's ages, Gyr.
const FORTNEY_AGES: [f64; 3] = [0.3, 1.0, 4.5];

/// Fortney, Marley and Barnes's fluxes, F⊕: their orbital distances about the Sun, 9.5, 1.0, 0.1
/// and 0.045 au, as 1 ÷ a². Their fifth, 0.02 au, is left out, since its lightest coreless planet
/// did not survive ("*").
const FORTNEY_FLUXES: [f64; 4] = [
    1.0 / (9.5 * 9.5),
    1.0,
    1.0 / (0.1 * 0.1),
    1.0 / (0.045 * 0.045),
];

/// Fortney, Marley and Barnes's masses, M⊕, as their tables' second header row gives them (the
/// first, in Jupiter masses, rounds differently: 0.15 `M_J` is 47.7 M⊕ at their 317.89, printed 46).
const FORTNEY_MASSES: [f64; 6] = [17.0, 28.0, 46.0, 77.0, 129.0, 215.0];

/// Fortney, Marley and Barnes's (2007) Tables 2, 3 and 4, the rows with no core: radius (`R_J`) by
/// age, flux and mass, in the order of [`FORTNEY_AGES`], [`FORTNEY_FLUXES`] and
/// [`FORTNEY_MASSES`], as printed (arXiv:astro-ph/0612671).
const FORTNEY_CORELESS_RADII: [[[f64; 6]; 4]; 3] = [
    // 300 Myr (Table 2)
    [
        [0.929, 0.951, 0.983, 1.020, 1.070, 1.106], // 9.5 au
        [1.504, 1.325, 1.222, 1.169, 1.182, 1.182], // 1.0 au
        [1.595, 1.395, 1.270, 1.197, 1.202, 1.198], // 0.1 au
        [2.795, 1.522, 1.345, 1.255, 1.240, 1.228], // 0.045 au
    ],
    // 1 Gyr (Table 3)
    [
        [0.857, 0.877, 0.910, 0.955, 1.003, 1.044], // 9.5 au
        [1.229, 1.148, 1.095, 1.086, 1.118, 1.130], // 1.0 au
        [1.298, 1.197, 1.127, 1.105, 1.133, 1.143], // 0.1 au
        [1.490, 1.271, 1.183, 1.144, 1.163, 1.167], // 0.045 au
    ],
    // 4.5 Gyr (Table 4)
    [
        [0.798, 0.827, 0.866, 0.913, 0.957, 0.994], // 9.5 au
        [1.014, 0.993, 0.983, 1.011, 1.050, 1.074], // 1.0 au
        [1.068, 1.027, 1.005, 1.024, 1.062, 1.085], // 0.1 au
        [1.103, 1.065, 1.038, 1.049, 1.086, 1.105], // 0.045 au
    ],
];

/// Where `x` falls on the ascending grid `grid`: the lower index and the fraction of the way to
/// the next point in the logarithm, both held at the grid's edges outside it.
#[must_use]
fn locate(grid: &[f64], x: f64) -> (usize, f64) {
    let last = grid.len() - 1;
    if x <= grid[0] {
        return (0, 0.0);
    }
    if x >= grid[last] {
        return (last - 1, 1.0);
    }
    let mut lower = 0;
    while x >= grid[lower + 1] {
        lower += 1;
    }
    let share = math::ln(x / grid[lower]) / math::ln(grid[lower + 1] / grid[lower]);
    (lower, share)
}

/// Lopez and Fortney's radius (R⊕) at mass `mass` (1–20 M⊕), envelope fraction `fraction`
/// (0.01–20%), flux `flux` (F⊕) and age `age` (Gyr), each of the last two held at the grid's
/// edges: linear in radius across the sixteen neighbouring grid points, in a fixed order.
#[must_use]
fn lopez_fortney_radius(mass: f64, fraction: f64, flux: f64, age: f64) -> f64 {
    let (a, wa) = locate(&LF_AGES, age);
    let (s, ws) = locate(&LF_FLUXES, flux);
    let (m, wm) = locate(&LF_MASSES, mass);
    let (f, wf) = locate(&LF_FRACTIONS, fraction);
    let mut radius = 0.0;
    for (da, ka) in [(0, 1.0 - wa), (1, wa)] {
        for (ds, ks) in [(0, 1.0 - ws), (1, ws)] {
            for (dm, km) in [(0, 1.0 - wm), (1, wm)] {
                for (df, kf) in [(0, 1.0 - wf), (1, wf)] {
                    let value = LOPEZ_FORTNEY_RADII[a + da][s + ds][m + dm][f + df];
                    radius += ka * ks * km * kf * value;
                }
            }
        }
    }
    radius
}

/// The thickness of an envelope of `fraction` (0.01–20%) on a planet of total mass `mass` (M⊕)
/// at flux `flux` (F⊕) and age `age` (Gyr), R⊕: Lopez and Fortney's radius less their core's,
/// (`M_core`)^¼ (their eq. 1), at the mass held to 1–20 M⊕, scaled outside it by M^−0.21.
#[must_use]
fn thickness(mass: f64, fraction: f64, flux: f64, age: f64) -> f64 {
    let held = mass.clamp(LF_MASSES[0], LF_MASSES[LF_MASSES.len() - 1]);
    let radius = lopez_fortney_radius(held, fraction, flux, age);
    let core = (held * (1.0 - fraction)).sqrt().sqrt();
    (radius - core) * math::powf(mass / held, ENVELOPE_MASS_EXPONENT)
}

/// The radius of a coreless planet of mass `mass` (M⊕) at flux `flux` (F⊕) and age `age` (Gyr),
/// from Fortney, Marley and Barnes's tables, R⊕; every input held at the grid's edges.
#[must_use]
fn coreless_radius(mass: f64, flux: f64, age: f64) -> EarthRadii {
    let (a, wa) = locate(&FORTNEY_AGES, age);
    let (s, ws) = locate(&FORTNEY_FLUXES, flux);
    let (m, wm) = locate(&FORTNEY_MASSES, mass);
    let mut radius = 0.0;
    for (da, ka) in [(0, 1.0 - wa), (1, wa)] {
        for (ds, ks) in [(0, 1.0 - ws), (1, ws)] {
            for (dm, km) in [(0, 1.0 - wm), (1, wm)] {
                radius += ka * ks * km * FORTNEY_CORELESS_RADII[a + da][s + ds][m + dm];
            }
        }
    }
    EarthRadii::from(JupiterRadii::new(radius))
}

/// The radius of a planet of total mass `mass` whose hydrogen and helium envelope is `fraction`
/// of its mass (0 ≤ `fraction` < 1), on a core of composition `core`, at flux `flux` and age `age`
/// (see the [module](self) documentation for the model and its sources).
///
/// With no envelope it is [`radius_zeng`] of the whole mass. The radius is continuous in every
/// argument, and rises with `fraction` at ages of 1 Gyr and more (to the dips the [module](self)
/// documentation bounds).
///
/// # Panics
///
/// In debug builds, if `mass` is not positive and finite, `fraction` is outside 0 ≤ `fraction` <
/// 1, or `flux` or `age` is negative or NaN.
///
/// # Examples
///
/// A 5 M⊕ planet with Earth-like rock and a 2% envelope, at 100 times Earth's flux, is a
/// sub-Neptune of about 2.6 R⊕ at 5 Gyr, and puffier young:
///
/// ```
/// use hyperion_sim::planetary::derive::envelope::radius_with_envelope;
/// use hyperion_sim::planetary::derive::radius::CoreComposition;
/// use hyperion_sim::units::{EarthFluxes, EarthMasses, Gigayears};
///
/// let (mass, core, flux) = (EarthMasses::new(5.0), CoreComposition::EARTH_LIKE, EarthFluxes::new(100.0));
/// let old = radius_with_envelope(mass, core, 0.02, flux, Gigayears::new(5.0)).value();
/// assert!((2.4..2.8).contains(&old));
/// let young = radius_with_envelope(mass, core, 0.02, flux, Gigayears::new(0.1)).value();
/// assert!(young > old);
/// ```
#[must_use]
pub fn radius_with_envelope(
    mass: EarthMasses,
    core: CoreComposition,
    fraction: f64,
    flux: EarthFluxes,
    age: Gigayears,
) -> EarthRadii {
    let m = mass.value();
    debug_assert!(m.is_finite() && m > 0.0, "a mass is positive, got {m}");
    debug_assert!(
        (0.0..1.0).contains(&fraction),
        "an envelope fraction lies in 0 to 1, got {fraction}"
    );
    let (s, t) = (flux.value(), age.value());
    debug_assert!(
        s >= 0.0 && t >= 0.0,
        "flux {s} and age {t} are not negative"
    );
    let core_radius = |f: f64| radius_zeng(EarthMasses::new(m * (1.0 - f)), core).value();
    let radius = if fraction <= 0.0 {
        core_radius(0.0)
    } else if fraction < SMALLEST_TABULATED_FRACTION {
        let edge = thickness(m, SMALLEST_TABULATED_FRACTION, s, t);
        core_radius(fraction) + fraction / SMALLEST_TABULATED_FRACTION * edge
    } else if fraction <= LARGEST_TABULATED_FRACTION {
        core_radius(fraction) + thickness(m, fraction, s, t)
    } else {
        let tabulated = core_radius(LARGEST_TABULATED_FRACTION)
            + thickness(m, LARGEST_TABULATED_FRACTION, s, t);
        let coreless = coreless_radius(m, s, t).value();
        if coreless <= tabulated {
            tabulated
        } else {
            let reach = math::ln(fraction / LARGEST_TABULATED_FRACTION)
                / math::ln(1.0 / LARGEST_TABULATED_FRACTION);
            tabulated * math::powf(coreless / tabulated, reach)
        }
    };
    EarthRadii::new(radius)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn radius(m: f64, core: CoreComposition, f: f64, s: f64, t: f64) -> f64 {
        radius_with_envelope(
            EarthMasses::new(m),
            core,
            f,
            EarthFluxes::new(s),
            Gigayears::new(t),
        )
        .value()
    }

    #[test]
    fn the_tables_are_lopez_and_fortney_s_to_the_accuracy_of_their_core() {
        // With an Earth-like core the model returns Lopez and Fortney's radii, but for the 3.2% by
        // which Zeng et al.'s Earth-like curve lies above their (`M_core`)^¼ at 8–20 M⊕.
        let mut worst: f64 = 0.0;
        for (a, age) in LF_AGES.iter().enumerate() {
            for (s, flux) in LF_FLUXES.iter().enumerate() {
                for (m, mass) in LF_MASSES.iter().enumerate() {
                    for (f, fraction) in LF_FRACTIONS.iter().enumerate() {
                        let ours =
                            radius(*mass, CoreComposition::EARTH_LIKE, *fraction, *flux, *age);
                        let theirs = LOPEZ_FORTNEY_RADII[a][s][m][f];
                        worst = worst.max((ours / theirs - 1.0).abs());
                    }
                }
            }
        }
        assert!(worst < 0.035, "{worst}");
    }

    #[test]
    fn the_radius_rises_with_the_envelope_from_a_bare_core_to_a_coreless_planet() {
        let cores = [
            CoreComposition::EARTH_LIKE,
            CoreComposition::new(0.325, 0.539).unwrap(),
        ];
        let mut worst: f64 = 0.0;
        for core in cores {
            for m in [1.6, 3.0, 8.0, 17.0, 40.0, 95.0, 131.0] {
                for s in [0.001, 0.1, 1.0, 30.0, 1_000.0, 5_000.0] {
                    for t in [1.0, 5.0, 13.0] {
                        let bare = radius(m, core, 0.0, s, t);
                        let zeng = radius_zeng(EarthMasses::new(m), core).value();
                        assert!((bare - zeng).abs() < 1e-15);
                        let mut last = bare;
                        for i in 1..1_000_u32 {
                            let f = 0.99 * math::powf(f64::from(i) / 1_000.0, 3.0);
                            let r = radius(m, core, f, s, t);
                            // Where two neighbouring entries of the tables are equal, as printed to
                            // two decimals, the shrinking core takes back up to 1.1 × 10⁻⁵ of the
                            // radius (131 M⊕ at 1,000 F⊕ and 1 Gyr).
                            let dip = 1.0 - r / last;
                            assert!(
                                dip < 2e-5,
                                "{m} M⊕, {s} F⊕, {t} Gyr: {r} at {f} after {last}"
                            );
                            worst = worst.max(dip);
                            last = last.max(r);
                        }
                        assert!(last > 2.0 * bare, "{m} M⊕: {last} against {bare}");
                    }
                }
            }
        }
        assert!(worst < 2e-5, "{worst}");
    }

    #[test]
    fn the_radius_is_continuous_where_the_pieces_meet() {
        for m in [2.0, 10.0, 60.0] {
            for edge in [SMALLEST_TABULATED_FRACTION, LARGEST_TABULATED_FRACTION] {
                let core = CoreComposition::EARTH_LIKE;
                let below = radius(m, core, edge * (1.0 - 1e-12), 10.0, 5.0);
                let at = radius(m, core, edge, 10.0, 5.0);
                let above = radius(m, core, edge * (1.0 + 1e-12), 10.0, 5.0);
                assert!((at / below - 1.0).abs() < 1e-9, "{m} M⊕ at {edge}");
                assert!((above / at - 1.0).abs() < 1e-9, "{m} M⊕ at {edge}");
            }
        }
    }

    #[test]
    fn the_bridge_stays_near_fortney_s_models_with_cores() {
        // Fortney et al. (2007) Table 4, 4.5 Gyr: (distance au, total M⊕, core M⊕, `R_J`), their
        // cores half ice and half rock, which is `CoreComposition::new(0.0, 0.5)`.
        let models = [
            (9.5, 28.0, 10.0, 0.653),
            (9.5, 46.0, 10.0, 0.759),
            (9.5, 46.0, 25.0, 0.611),
            (9.5, 77.0, 10.0, 0.844),
            (9.5, 77.0, 25.0, 0.750),
            (9.5, 129.0, 10.0, 0.911),
            (9.5, 129.0, 25.0, 0.849),
            (9.5, 129.0, 50.0, 0.754),
            (1.0, 46.0, 10.0, 0.845),
            (1.0, 77.0, 25.0, 0.820),
            (1.0, 129.0, 50.0, 0.810),
            (0.1, 77.0, 10.0, 0.942),
            (0.1, 129.0, 25.0, 0.934),
        ];
        let icy = CoreComposition::new(0.0, 0.5).unwrap();
        for (a, m, core, theirs) in models {
            let ours = radius(m, icy, 1.0 - core / m, 1.0 / (a * a), 4.5);
            let theirs = EarthRadii::from(JupiterRadii::new(theirs)).value();
            assert!(
                (ours / theirs - 1.0).abs() < 0.1,
                "{m} M⊕ with {core} at {a} au: {ours} against {theirs}"
            );
        }
    }

    #[test]
    fn a_body_with_no_envelope_is_its_bare_core_at_any_age() {
        for t in [0.01, 0.1, 5.0] {
            let r = radius(3.0, CoreComposition::ROCK, 0.0, 10.0, t);
            let zeng = radius_zeng(EarthMasses::new(3.0), CoreComposition::ROCK).value();
            assert!((r - zeng).abs() < 1e-15);
        }
    }
}
