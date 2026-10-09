//! A body's disc photometry (plan 14, P14.T47): the phase-curve template it is drawn with, its
//! geometric albedo in B, V and R, and the exponents that fit its law to its analogue's curve.
//!
//! # The law (R07 Design note 5)
//!
//! I ÷ F = A f(α) [L 2μ₀ ÷ (μ₀ + μ) + (1 − L) μ₀] with f(0) = 1, whose geometric albedo is
//! p = A [L + ⅔ (1 − L)] (McEwen 1991, Icarus 92, 298). Its disc-integrated phase function is
//! Φ(α) = f(α) `Φ_shape`(α; L), with `Φ_shape` the p-weighted mix of Lommel–Seeliger's
//! `Φ_LS` = 1 − sin(α ÷ 2) tan(α ÷ 2) ln cot(α ÷ 4) and Lambert's `Φ_L` = [sin α + (π − α) cos α] ÷ π
//! (weights L and ⅔ (1 − L), normalised to 1 at α = 0) ([`shape_phase`]). Per channel c,
//! `f_c` = `Φ_t^(s_c)` ÷ `Φ_shape`, clamped at [`PHASE_F_CLAMP`] and held at its value at the
//! template's last valid phase beyond it ([`channel_phase`]); the phase integral is
//! `q_c` = 2 ∫₀^π `Φ_c(α)` sin α dα ([`phase_integral`], a composite Simpson sum of 1,800 intervals in
//! a fixed order).
//!
//! # The templates (P14.T47.a)
//!
//! Each [`PhaseTemplate`] is an analogue's V phase curve `Φ_t` = 10^(−0.4 [V(α) − V(0)])
//! ([`template_phase`]): the planets' from Mallama and Hilton 2018 (Astronomy and Computing 25,
//! 10, arXiv:1808.01973, eqs. 2–17, the globe-only forms for Saturn), Earth's excepted, each
//! inside the range the paper states; Earth's from Robinson 2026 (PSJ 7, 12, arXiv:2507.22258,
//! eq. 14), a Henyey–Greenstein fit to the measured visual curve over 5°–144° (P14.T47.e,
//! decision-r07-earth-albedo); the Moon's from Krisciunas and Schaefer 1991 (PASP 103, 1033,
//! eq. 9), a fit to the lunar table of Allen 1973 (_Astrophysical Quantities_, 3rd ed., p. 143),
//! held past 150° and without an opposition surge, which `airless_ice` and `snowball` borrow. The
//! analogues' geometric albedos in B, V and R are Mallama, Krobusek and Pavlov 2017 (Icarus 282,
//! 19, Table 7), except Earth's, which are Robinson 2026's Model 07 B, V and R (§5.2) scaled by
//! the fit's f ÷ that model's visual p, 0.23 ÷ 0.242 (§5.3). The rows, their exponents s and phase
//! integrals q are [`PHASE_TEMPLATES`]. s and q are constants of each template, stored as literals
//! (decision-p14-phase-j A6): s is 1 for a template whose curve is its analogue's own, and for a
//! borrowed curve it is the s at which `q_V` is the analogue's measured q ([`exponent_for`]); a
//! test reproduces both. `s_B` = `s_V` = `s_R` everywhere: each curve is a single band.
//!
//! # The choice and the albedos (P14.T47.b; decision-phase-curves; decision-p14-phase-j)
//!
//! [`BodyPhotometry::derive`] chooses by the surface state ([`SurfaceState`]), in this order: a
//! gas envelope the giant of its class (a gas giant Jupiter's below [`SATURN_LIKE_HEAVY_ELEMENTS`]
//! of heavy elements and Saturn's at or above it, an ice giant or a sub-Neptune Neptune's); a
//! runaway greenhouse Venus's; a magma ocean `magma` below [`THIN_ATMOSPHERE_PRESSURE`] and
//! Venus's at or above it; a snowball `snowball`; an airless rock Mercury's and airless ice
//! `airless_ice`; a temperate world Mars's below [`THIN_ATMOSPHERE_PRESSURE`] and Earth's at or
//! above it. A gas envelope above [`HOT_GIANT_TEMPERATURE`] has no Solar System analogue and is
//! `provisional` (Sudarsky, Burrows and Pinto 2000, ApJ 538, 885, class II and up).
//!
//! `p_c` = `p_c,analogue` × `A_Bond` ÷ `A_ref`, capped so that `p_c` `q_c` ≤ 1, where `A_ref`
//! ([`TemplateRow::reference_bond`]) is the Bond albedo the generator gives the analogue body, so
//! that every Solar System analogue in its own state is drawn with its row's p (ruling 9). The
//! stated ratio `p_V` `q_V` ÷ `A_Bond` is a check only.

use core::f64::consts::PI;

use crate::math;
use crate::planetary::derive::PlanetClass;
use crate::planetary::derive::atmosphere::{SurfaceMaterial, SurfaceState};
use crate::planetary::derive::composition::MassFractions;
use crate::planetary::derive::irradiation::BondAlbedo;
use crate::planetary::params::{JUPITER_HEAVY_ELEMENT_FRACTION, SATURN_HEAVY_ELEMENT_FRACTION};
use crate::units::{Kelvin, MetresPerSecondSquared, Pascals, Radians};

/// The ceiling on the phase factor f: 4, R07's `PHASE_F_CLAMP` (a Lambert crescent vanishes faster
/// than a cloudy one: Venus's f would reach 61 at 170°).
pub const PHASE_F_CLAMP: f64 = 4.0;

/// The surface pressure below which the gas cannot hide the ground: 30 kPa (decision-phase-curves
/// item 4). Rayleigh optical depth at 550 nm scales with the column P ÷ g, Earth's being about
/// 0.097, so the Solar System Mars (11 kPa at 3.71 m s⁻²) has about 0.028 and a 30 kPa world at
/// Earth's gravity about 0.029. It splits Mars from Earth and a thin magma ocean from a thick one.
pub const THIN_ATMOSPHERE_PRESSURE: Pascals = Pascals::new(30_000.0);

/// The equilibrium temperature above which a gas envelope has no Solar System analogue: 150 K,
/// the top of Sudarsky, Burrows and Pinto's (2000, ApJ 538, 885) class I "Jovian" ammonia-cloud
/// giants; warmer giants are flagged `provisional` (decision-p14-phase-j A8).
pub const HOT_GIANT_TEMPERATURE: Kelvin = Kelvin::new(150.0);

/// The heavy-element fraction from which a gas giant takes Saturn's template: the midpoint of the
/// moment of inertia's blend, (`Z_J` + `Z_S`) ÷ 2 ≈ 0.237 (P14.T46.a).
pub const SATURN_LIKE_HEAVY_ELEMENTS: f64 = f64::midpoint(
    JUPITER_HEAVY_ELEMENT_FRACTION,
    SATURN_HEAVY_ELEMENT_FRACTION,
);

/// Intervals of the phase integral's composite Simpson sum over [0, π]: 1,800 (0.1°).
pub const PHASE_INTEGRAL_INTERVALS: u32 = 1_800;

/// Bisection steps of [`exponent_for`] on [`EXPONENT_BRACKET`]: 60.
const BISECTION_STEPS: u32 = 60;

/// The exponent's bracket for [`exponent_for`]: 0.1 to 10.
pub const EXPONENT_BRACKET: (f64, f64) = (0.1, 10.0);

/// A quantity in the B, V and R (Johnson) bands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bands {
    /// B.
    pub b: f64,
    /// V.
    pub v: f64,
    /// R.
    pub r: f64,
}

impl Bands {
    /// The same value in every band.
    #[must_use]
    pub const fn grey(x: f64) -> Self {
        Self { b: x, v: x, r: x }
    }
}

/// Which measured V phase curve a body's law is fitted to: R07's `PhaseTemplateId`, in its order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PhaseTemplate {
    /// The Moon's curve (a test analogue: airless rock takes Mercury's).
    Moon,
    /// Mercury's.
    Mercury,
    /// Mars's.
    Mars,
    /// Venus's.
    Venus,
    /// Earth's.
    Earth,
    /// Jupiter's.
    Jupiter,
    /// Saturn's globe.
    Saturn,
    /// Uranus's (a test analogue: an ice giant takes Neptune's).
    Uranus,
    /// Neptune's.
    Neptune,
    /// Airless ice: the Moon's curve, q from Ganymede's (provisional).
    AirlessIce,
    /// A snowball: the Moon's curve, q from Europa's (provisional).
    Snowball,
    /// A thin magma ocean: Mercury's curve (provisional).
    Magma,
}

impl PhaseTemplate {
    /// Every template, in [`PHASE_TEMPLATES`]'s order.
    pub const ALL: [Self; 12] = [
        Self::Moon,
        Self::Mercury,
        Self::Mars,
        Self::Venus,
        Self::Earth,
        Self::Jupiter,
        Self::Saturn,
        Self::Uranus,
        Self::Neptune,
        Self::AirlessIce,
        Self::Snowball,
        Self::Magma,
    ];

    /// The template's row of [`PHASE_TEMPLATES`].
    #[must_use]
    pub const fn row(self) -> &'static TemplateRow {
        let index = match self {
            Self::Moon => 0,
            Self::Mercury => 1,
            Self::Mars => 2,
            Self::Venus => 3,
            Self::Earth => 4,
            Self::Jupiter => 5,
            Self::Saturn => 6,
            Self::Uranus => 7,
            Self::Neptune => 8,
            Self::AirlessIce => 9,
            Self::Snowball => 10,
            Self::Magma => 11,
        };
        &PHASE_TEMPLATES[index]
    }

    /// Δm(α), the dimming in magnitudes against opposition at `alpha_deg` degrees, inside the
    /// valid range.
    fn dimming(self, alpha_deg: f64) -> f64 {
        match self {
            Self::Moon | Self::AirlessIce | Self::Snowball => {
                0.026 * alpha_deg + 4e-9 * math::powi(alpha_deg, 4)
            }
            Self::Mercury | Self::Magma => polynomial(
                &[
                    0.0,
                    6.328e-2,
                    -1.6336e-3,
                    3.3644e-5,
                    -3.4265e-7,
                    1.6893e-9,
                    -3.0334e-12,
                ],
                alpha_deg,
            ),
            Self::Venus => {
                if alpha_deg <= VENUS_JOIN_DEG {
                    polynomial(&[0.0, -1.044e-3, 3.687e-4, -2.814e-6, 8.938e-9], alpha_deg)
                } else {
                    polynomial(&[236.058_28 + 4.384, -2.819_14, 8.390_34e-3], alpha_deg)
                }
            }
            Self::Earth => {
                // Robinson 2026's eq. 14, Φ_t = [(1 + g)² ÷ (1 + g² + 2g cos α)]^(3/2), a
                // Henyey–Greenstein function at the scattering angle 180° − α, as a dimming:
                // 3.75 log₁₀ of the base over the same sum at α = 0, 1 + g² + 2g, which is
                // (1 + g)² up to rounding and makes the dimming at opposition 0 exactly.
                let g = EARTH_HG_ASYMMETRY;
                let base = |cos: f64| 1.0 + g * g + 2.0 * g * cos;
                3.75 * math::log10(base(math::cos(alpha_deg * DEG)) / base(1.0))
            }
            Self::Mars => polynomial(&[0.0, 2.267e-2, -1.302e-4], alpha_deg),
            Self::Jupiter => {
                if alpha_deg <= JUPITER_JOIN_DEG {
                    polynomial(&[0.0, -3.7e-4, 6.16e-4], alpha_deg)
                } else {
                    // Eq. 9's −9.428 against eq. 8's −9.395: the paper's adjustment so that the
                    // two agree at 12°.
                    let x = alpha_deg / 180.0;
                    -9.428 + 9.395
                        - 2.5
                            * math::log10(polynomial(
                                &[1.0, -1.507, -0.363, -0.062, 2.809, -1.876],
                                x,
                            ))
                }
            }
            Self::Saturn => {
                if alpha_deg <= SATURN_JOIN_DEG {
                    polynomial(&[0.0, -3.7e-4, 6.16e-4], alpha_deg)
                } else {
                    -8.94
                        + 8.95
                        + polynomial(&[0.0, 2.446e-4, 2.672e-4, -1.505e-6, 4.767e-9], alpha_deg)
                }
            }
            Self::Uranus => polynomial(&[0.0, 6.587e-3, 1.045e-4], alpha_deg),
            Self::Neptune => polynomial(&[0.0, 7.944e-3, 9.617e-5], alpha_deg),
        }
    }
}

/// Venus's join between Mallama and Hilton's eqs. 3 and 4, degrees: the forward-scattering
/// reversal (their §3.2).
const VENUS_JOIN_DEG: f64 = 163.7;

/// Jupiter's join between eqs. 8 and 9, degrees (their §3.5).
const JUPITER_JOIN_DEG: f64 = 12.0;

/// Saturn's join between eqs. 11 and 12, degrees: eq. 12's −8.94 is set to agree at 6° (§3.6).
const SATURN_JOIN_DEG: f64 = 6.0;

/// The asymmetry g of Earth's Henyey–Greenstein phase curve, dimensionless: −0.33, Robinson
/// 2026 (PSJ 7, 12, eq. 14), back-scattering towards opposition.
const EARTH_HG_ASYMMETRY: f64 = -0.33;

/// Σ cₖ xᵏ, coefficients ascending, by Horner's rule in a fixed order.
fn polynomial(coefficients: &[f64], x: f64) -> f64 {
    coefficients.iter().rev().fold(0.0, |sum, &c| sum * x + c)
}

/// One template: its analogue's albedos and curve, and the law's constants for it (P14.T47.a).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TemplateRow {
    /// The template.
    pub template: PhaseTemplate,
    /// The analogue's geometric albedo in B, V and R; the same in every band for an analogue
    /// measured in one (the Moon's V, Ganymede's and Europa's, which show no significant colour
    /// over 0.4–0.6 µm, Squyres and Veverka 1982).
    pub albedo: Bands,
    /// `A_ref`, the Bond albedo the generator gives the analogue body (decision-p14-phase-j,
    /// ruling 9): a body of that albedo is drawn with [`albedo`](Self::albedo) itself.
    ///
    /// Each row reads it from [`SurfaceState::albedo`] in the analogue's state, so that the two
    /// cannot drift apart. The material matters only to an airless state. The exception is
    /// snowball's, Europa's measured 0.68, since the generator would class Europa as airless ice.
    pub reference_bond: BondAlbedo,
    /// The exponents s in `Φ_t^s` per band.
    pub exponents: Bands,
    /// The phase integral q per band at [`exponents`](Self::exponents) and
    /// [`lunar_lambert_share`](Self::lunar_lambert_share) ([`phase_integral`]).
    pub phase_integral: Bands,
    /// The law's Lommel–Seeliger share L with this curve: 1 for an airless surface, 0.5 for
    /// Mars's thin air, 0 under a thick atmosphere or cloud (R07 Design note 5).
    pub lunar_lambert_share: f64,
    /// The end of the curve's stated range; the law holds its f beyond it.
    pub valid_to: Radians,
    /// A borrowed curve with no measured analogue, labelled wherever it is shown.
    pub provisional: bool,
    /// The citation: curve, range, albedos and the analogue's measured Bond albedo.
    pub source: &'static str,
}

/// Degrees to radians, for the rows' ranges.
const DEG: f64 = PI / 180.0;

/// The phase-curve templates, in [`PhaseTemplate`]'s order (see the [module](self) documentation).
///
/// The phase integrals are at each row's own L; s and q are reproduced by [`exponent_for`] and
/// [`phase_integral`] to 10⁻⁹ by a test.
pub const PHASE_TEMPLATES: [TemplateRow; 12] = [
    TemplateRow {
        template: PhaseTemplate::Moon,
        albedo: Bands::grey(0.12),
        reference_bond: SurfaceState::Airless.albedo(SurfaceMaterial::Rock),
        exponents: Bands::grey(1.0),
        phase_integral: Bands::grey(Q_MOON),
        lunar_lambert_share: 1.0,
        valid_to: Radians::new(150.0 * DEG),
        provisional: false,
        source: "Krisciunas and Schaefer 1991, PASP 103, 1033, eq. 9 (a fit to Allen 1973, \
                 Astrophysical Quantities, 3rd ed., p. 143), to 150°; p_V 0.12 (NASA's fact \
                 sheet); Bond albedo 0.11 (NASA's fact sheet, the generator's airless rock)",
    },
    TemplateRow {
        template: PhaseTemplate::Mercury,
        albedo: Bands {
            b: 0.105,
            v: 0.142,
            r: 0.172,
        },
        reference_bond: SurfaceState::Airless.albedo(SurfaceMaterial::Rock),
        exponents: Bands::grey(1.0),
        phase_integral: Bands::grey(Q_MERCURY),
        lunar_lambert_share: 1.0,
        valid_to: Radians::new(169.5 * DEG),
        provisional: false,
        source: "Mallama and Hilton 2018, Astronomy and Computing 25, 10, eq. 2, to 169.5°; \
                 Mallama, Krobusek and Pavlov 2017, Icarus 282, 19, Table 7; measured Bond \
                 albedo 0.088 (NASA's fact sheet), the generator's airless rock 0.11",
    },
    TemplateRow {
        template: PhaseTemplate::Mars,
        albedo: Bands {
            b: 0.088,
            v: 0.170,
            r: 0.288,
        },
        reference_bond: SurfaceState::Temperate.albedo(SurfaceMaterial::Rock),
        exponents: Bands::grey(1.0),
        phase_integral: Bands::grey(Q_MARS),
        lunar_lambert_share: 0.5,
        valid_to: Radians::new(50.0 * DEG),
        provisional: false,
        source: "Mallama and Hilton 2018, eq. 6, to 50° (eq. 7, an average of Earth and Mercury, \
                 not used: f is held past 50°); Mallama et al. 2017, Table 7; measured Bond \
                 albedo 0.250 (NASA's fact sheet), the generator's temperate 0.294",
    },
    TemplateRow {
        template: PhaseTemplate::Venus,
        albedo: Bands {
            b: 0.658,
            v: 0.689,
            r: 0.708,
        },
        reference_bond: SurfaceState::RunawayGreenhouse.albedo(SurfaceMaterial::Rock),
        exponents: Bands::grey(1.0),
        phase_integral: Bands::grey(Q_VENUS),
        lunar_lambert_share: 0.0,
        valid_to: Radians::new(179.0 * DEG),
        provisional: false,
        source: "Mallama and Hilton 2018, eqs. 3–4, joined at 163.7°, to 179°; Mallama et al. \
                 2017, Table 7; Bond albedo 0.76 (Haus et al. 2016, Icarus 272, 178)",
    },
    TemplateRow {
        template: PhaseTemplate::Earth,
        // The eq. 14 fit's f = 0.23, its geometric albedo, in Model 07's band ratios 0.277 :
        // 0.226 : 0.221 (that model's B, V and R against its visual 0.242), whose flat 0.4–0.5,
        // 0.5–0.6 and 0.6–0.7 µm bands stand for Johnson's. CERES's Bond albedo in the source is
        // EBAF Ed4.0's 99.1 ÷ 340.0 W m⁻² = 0.2915 (Loeb et al. 2018, J. Climate 31, 895,
        // Table 5).
        albedo: Bands {
            b: 0.263,
            v: 0.215,
            r: 0.210,
        },
        reference_bond: SurfaceState::Temperate.albedo(SurfaceMaterial::Rock),
        exponents: Bands::grey(1.0),
        phase_integral: Bands::grey(Q_EARTH),
        lunar_lambert_share: 0.0,
        valid_to: Radians::new(144.0 * DEG),
        provisional: false,
        source: "Robinson 2026, PSJ 7, 12, eq. 14 (g = −0.33, f = 0.23), to 144°; p from §5's \
                 band ratios at f; Bond albedo 0.294 (the temperate state; NASA's Earth fact \
                 sheet, 11 January 2024; CERES 0.2915, Loeb et al. 2018)",
    },
    TemplateRow {
        template: PhaseTemplate::Jupiter,
        albedo: Bands {
            b: 0.443,
            v: 0.538,
            r: 0.495,
        },
        reference_bond: SurfaceState::GasEnvelope.albedo(SurfaceMaterial::Rock),
        exponents: Bands::grey(1.0),
        phase_integral: Bands::grey(Q_JUPITER),
        lunar_lambert_share: 0.0,
        valid_to: Radians::new(130.0 * DEG),
        provisional: false,
        source: "Mallama and Hilton 2018, eqs. 8–9, joined at 12°, to 130°; Mallama et al. 2017, \
                 Table 7; measured Bond albedo 0.343 (Hanel et al. 1981), the generator's gas \
                 envelope 0.34",
    },
    TemplateRow {
        template: PhaseTemplate::Saturn,
        albedo: Bands {
            b: 0.339,
            v: 0.499,
            r: 0.568,
        },
        reference_bond: SurfaceState::GasEnvelope.albedo(SurfaceMaterial::Rock),
        exponents: Bands::grey(1.0),
        phase_integral: Bands::grey(Q_SATURN),
        lunar_lambert_share: 0.0,
        valid_to: Radians::new(150.0 * DEG),
        provisional: false,
        source: "Mallama and Hilton 2018, eqs. 11–12, the globe without rings, joined at 6°, to \
                 150°; Mallama et al. 2017, Table 7; measured Bond albedo 0.342 (NASA's fact \
                 sheet), the generator's gas envelope 0.34",
    },
    TemplateRow {
        template: PhaseTemplate::Uranus,
        albedo: Bands {
            b: 0.561,
            v: 0.488,
            r: 0.202,
        },
        reference_bond: SurfaceState::GasEnvelope.albedo(SurfaceMaterial::Rock),
        exponents: Bands::grey(1.0),
        phase_integral: Bands::grey(Q_URANUS),
        lunar_lambert_share: 0.0,
        valid_to: Radians::new(154.0 * DEG),
        provisional: false,
        source: "Mallama and Hilton 2018, eq. 15 at a sub-latitude of 0, to 154°; Mallama et al. \
                 2017, Table 7; measured Bond albedo 0.300 (NASA's fact sheet), the generator's \
                 gas envelope 0.34",
    },
    TemplateRow {
        template: PhaseTemplate::Neptune,
        albedo: Bands {
            b: 0.562,
            v: 0.442,
            r: 0.181,
        },
        reference_bond: SurfaceState::GasEnvelope.albedo(SurfaceMaterial::Rock),
        exponents: Bands::grey(1.0),
        phase_integral: Bands::grey(Q_NEPTUNE),
        lunar_lambert_share: 0.0,
        valid_to: Radians::new(133.0 * DEG),
        provisional: false,
        source: "Mallama and Hilton 2018, eq. 17, to 133°; Mallama et al. 2017, Table 7; \
                 measured Bond albedo 0.290 (Pearl and Conrath 1991), the generator's gas \
                 envelope 0.34",
    },
    TemplateRow {
        template: PhaseTemplate::AirlessIce,
        albedo: Bands::grey(0.43),
        reference_bond: SurfaceState::Airless.albedo(SurfaceMaterial::Ice),
        exponents: Bands::grey(S_AIRLESS_ICE),
        phase_integral: Bands::grey(0.80),
        lunar_lambert_share: 1.0,
        valid_to: Radians::new(150.0 * DEG),
        provisional: true,
        source: "PROVISIONAL: the Moon's curve shape (Krisciunas and Schaefer 1991, eq. 9), to \
                 150°, s solved to Ganymede's q of 0.80 (Squyres and Veverka 1981, Icarus 46, \
                 137; Buratti 1991, Icarus 92, 312, gives 0.78); p_V 0.43 and Bond albedo 0.35 \
                 (Squyres and Veverka 1982, Icarus 52)",
    },
    TemplateRow {
        template: PhaseTemplate::Snowball,
        albedo: Bands::grey(0.67),
        // Europa's measured Bond albedo, not a state's (ruling 9).
        reference_bond: BondAlbedo::from_fraction(0.68),
        exponents: Bands::grey(S_SNOWBALL),
        phase_integral: Bands::grey(1.01),
        lunar_lambert_share: 1.0,
        valid_to: Radians::new(150.0 * DEG),
        provisional: true,
        source: "PROVISIONAL: the Moon's curve shape (Krisciunas and Schaefer 1991, eq. 9), to \
                 150°, s solved to Europa's q of 1.01; p_V 0.67 and Bond albedo 0.68 (Grundy et \
                 al. 2007, Science 318, 234); cloud-free ice",
    },
    TemplateRow {
        template: PhaseTemplate::Magma,
        albedo: Bands {
            b: 0.105,
            v: 0.142,
            r: 0.172,
        },
        reference_bond: SurfaceState::Airless.albedo(SurfaceMaterial::Rock),
        exponents: Bands::grey(1.0),
        phase_integral: Bands::grey(Q_MERCURY),
        lunar_lambert_share: 1.0,
        valid_to: Radians::new(169.5 * DEG),
        provisional: true,
        source: "PROVISIONAL: Mercury's curve and albedos stand in (Mallama and Hilton 2018, eq. \
                 2; Mallama et al. 2017, Table 7); no magma-ocean analogue; thin branch only, \
                 P < 30 kPa",
    },
];

// The phase integrals of the templates whose curve is their analogue's own (s = 1), at their own
// L, and the solved exponents of the borrowed curves: computed by `phase_integral` and
// `exponent_for`, which a test reproduces to 10⁻⁹.
const Q_MOON: f64 = 0.626_110_411_296_755;
const Q_MERCURY: f64 = 0.479_801_866_292_407_7;
const Q_MARS: f64 = 1.084_647_566_489_643_3;
const Q_VENUS: f64 = 1.344_239_863_487_745_5;
const Q_EARTH: f64 = 1.311_572_861_542_696;
const Q_JUPITER: f64 = 1.311_718_956_968_471_3;
const Q_SATURN: f64 = 1.356_623_222_499_990_8;
const Q_URANUS: f64 = 1.301_738_407_075_694_5;
const Q_NEPTUNE: f64 = 1.242_005_972_898_866;
const S_AIRLESS_ICE: f64 = 0.821_536_503_206_686_8;
const S_SNOWBALL: f64 = 0.667_471_933_153_726_4;

/// `Φ_t(α)`, the template's V phase curve normalised to 1 at opposition, with α clamped to
/// [0, the valid range].
#[must_use]
pub fn template_phase(template: PhaseTemplate, alpha: Radians) -> f64 {
    let end = template.row().valid_to.value();
    let alpha = alpha.value().clamp(0.0, end);
    math::exp10(-0.4 * template.dimming(alpha / DEG))
}

/// Lambert sphere's disc-integrated phase function, 1 at 0 and 0 at π.
fn lambert_phase(alpha: f64) -> f64 {
    let a = alpha.clamp(0.0, PI);
    let (sin, cos) = math::sin_cos(a);
    ((sin + (PI - a) * cos) / PI).max(0.0)
}

/// Lommel–Seeliger sphere's disc-integrated phase function, 1 at 0 and 0 at π.
fn lommel_seeliger_phase(alpha: f64) -> f64 {
    if alpha <= 0.0 {
        return 1.0;
    }
    if alpha >= PI {
        return 0.0;
    }
    let half = 0.5 * alpha;
    1.0 - math::sin(half) * math::tan(half) * math::ln(1.0 / math::tan(0.25 * alpha))
}

/// `Φ_shape`(α; L), the law's disc term's disc-integrated phase function, 1 at opposition: the
/// mix of Lommel–Seeliger's (weight L) and Lambert's (weight ⅔ (1 − L)).
#[must_use]
pub fn shape_phase(lunar_lambert_share: f64, alpha: Radians) -> f64 {
    let l = lunar_lambert_share;
    let lambert = (2.0 / 3.0) * (1.0 - l);
    let a = alpha.value();
    (l * lommel_seeliger_phase(a) + lambert * lambert_phase(a)) / (l + lambert)
}

/// `Φ_c(α)` = f `Φ_shape`, the law's disc-integrated phase function on `template` with share `l` and
/// exponent `s`: `Φ_t^s` inside the template's range, f clamped at [`PHASE_F_CLAMP`], and f held
/// at its value at the range's end beyond it.
#[must_use]
pub fn channel_phase(template: PhaseTemplate, l: f64, s: f64, alpha: Radians) -> f64 {
    let end = template.row().valid_to;
    let shape = shape_phase(l, alpha);
    if alpha.value() <= end.value() {
        math::powf(template_phase(template, alpha), s).min(PHASE_F_CLAMP * shape)
    } else {
        held_factor(template, l, s) * shape
    }
}

/// f at the template's last valid phase, which the law holds beyond it.
fn held_factor(template: PhaseTemplate, l: f64, s: f64) -> f64 {
    let end = template.row().valid_to;
    let shape = shape_phase(l, end);
    if shape > 0.0 {
        (math::powf(template_phase(template, end), s) / shape).min(PHASE_F_CLAMP)
    } else {
        PHASE_F_CLAMP
    }
}

/// 2 ∫₀^π `phase`(α) sin α dα by a composite Simpson sum of [`PHASE_INTEGRAL_INTERVALS`]
/// intervals, in index order.
fn simpson(phase: impl Fn(Radians) -> f64) -> f64 {
    let n = PHASE_INTEGRAL_INTERVALS;
    let h = PI / f64::from(n);
    let sum = (0..=n).fold(0.0, |sum, i| {
        let weight = if i == 0 || i == n {
            1.0
        } else if i % 2 == 1 {
            4.0
        } else {
            2.0
        };
        let alpha = f64::from(i) * h;
        sum + weight * phase(Radians::new(alpha)) * math::sin(alpha)
    });
    2.0 * sum * h / 3.0
}

/// The phase integral q = 2 ∫₀^π `Φ_c(α)` sin α dα of the law on `template` with share `l` and
/// exponent `s` ([`channel_phase`]).
#[must_use]
pub fn phase_integral(template: PhaseTemplate, l: f64, s: f64) -> f64 {
    simpson(|alpha| channel_phase(template, l, s, alpha))
}

/// The exponent s at which the law on `template` with share `l` has phase integral `q`: 60 steps
/// of bisection on [`EXPONENT_BRACKET`], since q falls monotonically as s rises; the bracket's end
/// where `q` lies beyond what it reaches.
#[must_use]
pub fn exponent_for(template: PhaseTemplate, l: f64, q: f64) -> f64 {
    let (mut low, mut high) = EXPONENT_BRACKET;
    if q >= phase_integral(template, l, low) {
        return low;
    }
    if q <= phase_integral(template, l, high) {
        return high;
    }
    for _ in 0..BISECTION_STEPS {
        let middle = f64::midpoint(low, high);
        if phase_integral(template, l, middle) > q {
            low = middle;
        } else {
            high = middle;
        }
    }
    f64::midpoint(low, high)
}

/// What [`BodyPhotometry::derive`] reads of a body at a record's time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhotometryInputs {
    /// The surface state.
    pub state: SurfaceState,
    /// What an airless surface is made of, by the record's composition.
    pub material: SurfaceMaterial,
    /// The surface pressure; `None` for a gas envelope.
    pub surface_pressure: Option<Pascals>,
    /// The share of the disc covered by cloud (carried for R07's cloud term, which is suspended
    /// while it is a constant of the state).
    pub cloud_fraction: f64,
    /// The mean surface temperature.
    pub surface_temperature: Kelvin,
    /// The equilibrium temperature from the hosts' light, which flags a hot giant
    /// ([`HOT_GIANT_TEMPERATURE`]).
    pub equilibrium_temperature: Kelvin,
    /// The surface gravity.
    pub surface_gravity: MetresPerSecondSquared,
    /// The class.
    pub class: PlanetClass,
    /// The mass fractions, whose heavy elements choose a gas giant's template.
    pub fractions: MassFractions,
    /// The Bond albedo of the state on the material (P14.T13.c).
    pub bond: BondAlbedo,
}

/// A body's photometry at a record's time (P14.T47.c): its template, its geometric albedo per
/// band, its law's exponents and share, its Bond albedo and the stated check.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyPhotometry {
    geometric_albedo: Bands,
    template: PhaseTemplate,
    exponents: Bands,
    phase_integral: Bands,
    lunar_lambert_share: f64,
    bond_albedo: BondAlbedo,
    bond_ratio: f64,
    provisional: bool,
}

impl BodyPhotometry {
    /// The photometry of the body `inputs` describe (see the [module](self) documentation). It
    /// solves nothing: s and q are the template's.
    #[must_use]
    pub fn derive(inputs: &PhotometryInputs) -> Self {
        let template = choose(inputs);
        let row = template.row();
        let scale = inputs.bond.value() / row.reference_bond.value();
        let q = row.phase_integral;
        let p = Bands {
            b: (row.albedo.b * scale).min(1.0 / q.b),
            v: (row.albedo.v * scale).min(1.0 / q.v),
            r: (row.albedo.r * scale).min(1.0 / q.r),
        };
        let hot = inputs.state == SurfaceState::GasEnvelope
            && inputs.equilibrium_temperature > HOT_GIANT_TEMPERATURE;
        Self {
            geometric_albedo: p,
            template,
            exponents: row.exponents,
            phase_integral: q,
            lunar_lambert_share: row.lunar_lambert_share,
            bond_albedo: inputs.bond,
            bond_ratio: p.v * q.v / inputs.bond.value(),
            provisional: row.provisional || hot,
        }
    }

    /// The geometric albedo p per band, against π a c.
    #[must_use]
    pub const fn geometric_albedo(&self) -> Bands {
        self.geometric_albedo
    }

    /// The phase-curve template.
    #[must_use]
    pub const fn template(&self) -> PhaseTemplate {
        self.template
    }

    /// The exponents s per band.
    #[must_use]
    pub const fn exponents(&self) -> Bands {
        self.exponents
    }

    /// The law's Lommel–Seeliger share L, 0–1.
    #[must_use]
    pub const fn lunar_lambert_share(&self) -> f64 {
        self.lunar_lambert_share
    }

    /// The Bond albedo of the body's state (P14.T13.c).
    #[must_use]
    pub const fn bond_albedo(&self) -> BondAlbedo {
        self.bond_albedo
    }

    /// `p_V` `q_V` ÷ `A_Bond`, a check only: 1 for a law whose spherical albedo is the Bond albedo.
    #[must_use]
    pub const fn bond_ratio(&self) -> f64 {
        self.bond_ratio
    }

    /// Whether the template is a borrowed curve or the body a hot giant with no analogue.
    #[must_use]
    pub const fn provisional(&self) -> bool {
        self.provisional
    }

    /// The phase integral q per band, the template's.
    #[must_use]
    pub const fn phase_integral(&self) -> Bands {
        self.phase_integral
    }
}

/// The template of the body `inputs` describe, by its surface state (P14.T47.b).
fn choose(inputs: &PhotometryInputs) -> PhaseTemplate {
    let thin = inputs
        .surface_pressure
        .is_none_or(|p| p < THIN_ATMOSPHERE_PRESSURE);
    match inputs.state {
        SurfaceState::GasEnvelope => match inputs.class {
            PlanetClass::GasGiant
                if 1.0 - inputs.fractions.envelope() >= SATURN_LIKE_HEAVY_ELEMENTS =>
            {
                PhaseTemplate::Saturn
            }
            PlanetClass::GasGiant => PhaseTemplate::Jupiter,
            PlanetClass::Rocky
            | PlanetClass::Icy
            | PlanetClass::SubNeptune
            | PlanetClass::IceGiant => PhaseTemplate::Neptune,
        },
        SurfaceState::MagmaOcean if thin => PhaseTemplate::Magma,
        // A runaway greenhouse, and a magma ocean under a thick atmosphere (unlabelled: it has no
        // analogue either).
        SurfaceState::RunawayGreenhouse | SurfaceState::MagmaOcean => PhaseTemplate::Venus,
        SurfaceState::Snowball => PhaseTemplate::Snowball,
        SurfaceState::Airless => match inputs.material {
            SurfaceMaterial::Rock => PhaseTemplate::Mercury,
            SurfaceMaterial::Ice => PhaseTemplate::AirlessIce,
        },
        SurfaceState::Temperate if thin => PhaseTemplate::Mars,
        SurfaceState::Temperate => PhaseTemplate::Earth,
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::planetary::derive::DerivedBody;
    use crate::planetary::derive::radius::CoreComposition;
    use crate::planetary::derive::solar::{found, solar_system};

    const LS: [f64; 3] = [0.0, 0.5, 1.0];

    fn inputs_of(d: &DerivedBody) -> PhotometryInputs {
        let atmosphere = d.atmosphere();
        let material = SurfaceMaterial::of(&d.fractions());
        PhotometryInputs {
            state: atmosphere.state(),
            material,
            surface_pressure: atmosphere.surface_pressure(),
            cloud_fraction: atmosphere.cloud_fraction(),
            surface_temperature: atmosphere.surface_temperature(),
            equilibrium_temperature: d.equilibrium_temperature(),
            surface_gravity: d.surface_gravity(),
            class: d.class(),
            fractions: d.fractions(),
            bond: atmosphere.state().albedo(material),
        }
    }

    /// (a) `Φ_t(0)` = 1 for every template, and `Φ_shape(0)` = 1 for every share.
    #[test]
    fn the_curves_are_one_at_opposition() {
        for template in PhaseTemplate::ALL {
            assert_eq!(template.row().template, template);
            assert!((template_phase(template, Radians::ZERO) - 1.0).abs() < 1e-15);
        }
        for l in LS {
            assert!((shape_phase(l, Radians::ZERO) - 1.0).abs() < 1e-15);
        }
    }

    /// (a) Lambert's q is 1.5 and Lommel–Seeliger's 16 (1 − ln 2) ÷ 3, to 10⁻⁹.
    #[test]
    fn the_shapes_have_their_phase_integrals() {
        let lambert = simpson(|alpha| shape_phase(0.0, alpha));
        assert!((lambert - 1.5).abs() < 1e-9, "{lambert}");
        let lommel_seeliger = simpson(|alpha| shape_phase(1.0, alpha));
        let expected = 16.0 * (1.0 - core::f64::consts::LN_2) / 3.0;
        assert!(
            (lommel_seeliger - expected).abs() < 1e-9,
            "{lommel_seeliger}"
        );
    }

    /// (a) Inside a template's range, and where the clamp does not act, the law's phase function is
    /// `Φ_t^s` whatever L is, to 10⁻¹². (Past the range the law holds f while `Φ_shape` still moves
    /// with L, so q at another L than the template's differs: by 10⁻⁴ for the Moon's curve held
    /// past 150° and by 0.10 for Mars's held past 50°. Each template's q is taken at its own L.)
    #[test]
    fn inside_its_range_the_law_is_the_template_whatever_l() {
        for template in PhaseTemplate::ALL {
            let row = template.row();
            for s in [0.7, 1.0, 1.3] {
                for i in 0..=100 {
                    let alpha = Radians::new(row.valid_to.value() * f64::from(i) / 100.0);
                    let target = math::powf(template_phase(template, alpha), s);
                    for l in LS {
                        if target < PHASE_F_CLAMP * shape_phase(l, alpha) {
                            let phase = channel_phase(template, l, s, alpha);
                            assert!((phase - target).abs() < 1e-12, "{template:?} {s} {i} {l}");
                        }
                    }
                }
            }
        }
    }

    /// (a) The rows' s and q are what `exponent_for` and `phase_integral` give, to 10⁻⁹
    /// (decision-p14-phase-j A6); q falls monotonically in s, and `exponent_for` inverts
    /// `phase_integral`.
    #[test]
    fn the_rows_constants_are_reproduced() {
        for template in PhaseTemplate::ALL {
            let row = template.row();
            let l = row.lunar_lambert_share;
            let q = phase_integral(template, l, row.exponents.v);
            assert!((q - row.phase_integral.v).abs() < 1e-9, "{template:?}: {q}");
            assert_same_bits(row.exponents.b, row.exponents.v);
            assert_same_bits(row.exponents.r, row.exponents.v);
            assert_same_bits(row.phase_integral.b, row.phase_integral.v);
            assert_same_bits(row.phase_integral.r, row.phase_integral.v);
            if (row.exponents.v - 1.0).abs() > 0.0 {
                let s = exponent_for(template, l, row.phase_integral.v);
                assert!((s - row.exponents.v).abs() < 1e-9, "{template:?}: {s}");
            }
            let mut previous = f64::INFINITY;
            for k in 1..=40 {
                let s = 0.1 * f64::from(k);
                let q = phase_integral(template, l, s);
                assert!(q < previous, "{template:?} at s {s}");
                previous = q;
                if (0.2..=5.0).contains(&s) {
                    assert!(
                        (exponent_for(template, l, q) - s).abs() < 1e-9,
                        "{template:?} {s}"
                    );
                }
            }
        }
    }

    /// (a) Mercury's `q_V` is 0.480 (Mallama et al. 2002's 0.478) and Earth's 1.312 (eq. 2 and
    /// Robinson 2026's eq. 14, the clamp acting on Earth from 139°), to 0.5%; every template's q
    /// lies in 0.4–1.7.
    #[test]
    fn the_phase_integrals_are_the_analogues() {
        let q = |t: PhaseTemplate| t.row().phase_integral.v;
        assert!((q(PhaseTemplate::Mercury) / 0.480 - 1.0).abs() < 0.005);
        assert!((q(PhaseTemplate::Earth) / 1.312 - 1.0).abs() < 0.005);
        for template in PhaseTemplate::ALL {
            assert!((0.4..=1.7).contains(&q(template)), "{template:?}");
        }
        assert!((q(PhaseTemplate::AirlessIce) - 0.80).abs() < 1e-12);
        assert!((q(PhaseTemplate::Snowball) - 1.01).abs() < 1e-12);
    }

    /// (b) The Solar System table: each planet chooses its own template (Uranus Neptune's), and
    /// each analogue's p is Table 7's to 10⁻¹² (decision-p14-phase-j, 9).
    #[test]
    fn the_solar_system_draws_itself() {
        let planets = solar_system();
        for (name, template, l) in [
            ("Mercury", PhaseTemplate::Mercury, 1.0),
            ("Venus", PhaseTemplate::Venus, 0.0),
            ("Earth", PhaseTemplate::Earth, 0.0),
            ("Mars", PhaseTemplate::Mars, 0.5),
            ("Jupiter", PhaseTemplate::Jupiter, 0.0),
            ("Saturn", PhaseTemplate::Saturn, 0.0),
            ("Uranus", PhaseTemplate::Neptune, 0.0),
            ("Neptune", PhaseTemplate::Neptune, 0.0),
        ] {
            let photometry = BodyPhotometry::derive(&inputs_of(found(&planets, name)));
            assert_eq!(photometry.template(), template, "{name}");
            assert!((photometry.lunar_lambert_share() - l).abs() < 1e-15);
            let (p, table) = (photometry.geometric_albedo(), template.row().albedo);
            for (got, want) in [(p.b, table.b), (p.v, table.v), (p.r, table.r)] {
                assert!((got - want).abs() < 1e-12, "{name}: {got} against {want}");
            }
            assert!(!photometry.provisional(), "{name}");
        }
        // The stated ratios, findings for P14.T13.c and not failures: Mercury's 0.62
        // (decision-p14-phase-j) and Earth's 0.959 at the temperate 0.294 (decision-p11-t4k-faults;
        // 0.92 at 0.306, decision-r07-earth-albedo; Mallama's p gave 1.86).
        let ratio = |name| BodyPhotometry::derive(&inputs_of(found(&planets, name))).bond_ratio();
        assert!((ratio("Mercury") - 0.142 * 0.4798 / 0.11).abs() < 0.01);
        assert!((ratio("Earth") - 0.215 * 1.3116 / 0.294).abs() < 0.01);
    }

    /// Robinson 2026's eq. 14 written out as the paper gives it, independently of the template's
    /// dimming: his eq. 5's Henyey–Greenstein function `P_HG`(Θ) = (1 − g²) ÷ (1 + g² − 2g cos
    /// Θ)^(3/2) of g = −0.33 at the scattering angle Θ = 180° − α, over its value at α = 0,
    /// unclamped and over 0°–180°.
    fn robinson_eq14(alpha: Radians) -> f64 {
        let g: f64 = -0.33;
        let henyey_greenstein =
            |theta: f64| (1.0 - g * g) / math::powf(1.0 + g * g - 2.0 * g * math::cos(theta), 1.5);
        henyey_greenstein(PI - alpha.value()) / henyey_greenstein(PI)
    }

    /// P14.T47.e: Earth's template is Robinson 2026's eq. 14 to 144° (its dimming by hand at 30°,
    /// 90°, 120° and 144° to 10⁻⁴ mag, R07.T4.d's figures, and the paper's form to 10⁻¹²), held
    /// beyond, with Model 07's band ratios at f = 0.23.
    #[test]
    fn earth_is_robinson_2026s_fit() {
        let earth = PhaseTemplate::Earth;
        for (degrees, mag) in [
            (30.0, 0.292_82),
            (90.0, 1.472_79),
            (120.0, 1.897_05),
            (144.0, 2.112_93),
        ] {
            let dimming = earth.dimming(degrees);
            assert!((dimming - mag).abs() < 1e-4, "{degrees}°: {dimming}");
        }
        assert_same_bits(earth.dimming(0.0), 0.0);
        for i in 0..=144 {
            let alpha = Radians::new(f64::from(i) * DEG);
            let (template, paper) = (template_phase(earth, alpha), robinson_eq14(alpha));
            assert!(
                (template - paper).abs() < 1e-12,
                "{i}°: {template} against {paper}"
            );
        }
        let row = earth.row();
        assert!((row.valid_to.value() - 144.0 * DEG).abs() < 1e-15);
        assert!(row.lunar_lambert_share.abs() < 1e-15);
        assert!(!row.provisional);
        assert!(row.source.starts_with("Robinson 2026, PSJ 7, 12, eq. 14 "));
        // Model 07's B, V and R (0.277, 0.226, 0.221) scaled from its visual p of 0.242 to the
        // fit's f = 0.23, to the rounding of the third decimal.
        for (p, model) in [
            (row.albedo.b, 0.277),
            (row.albedo.v, 0.226),
            (row.albedo.r, 0.221),
        ] {
            assert!((p - model * 0.23 / 0.242).abs() < 5e-4, "{p}");
        }
    }

    /// P14.T47.e: Earth's law at L = 0 is clamped from 139.006° (R07.T4.d's onset), and past
    /// 144° the held f of 5.65 is cut to 4; eq. 14 alone over 0°–180° would integrate to 1.350;
    /// 0.23 q is within 3% of Robinson's visual spherical albedo of 0.294 (0.302).
    #[test]
    fn earths_clamp_and_phase_integral_match_the_client() {
        let earth = PhaseTemplate::Earth;
        let f = |degrees: f64| {
            let alpha = Radians::new(degrees * DEG);
            channel_phase(earth, 0.0, 1.0, alpha) / shape_phase(0.0, alpha)
        };
        assert!(f(138.9) < PHASE_F_CLAMP, "{}", f(138.9));
        assert!((f(139.1) - PHASE_F_CLAMP).abs() < 1e-12, "{}", f(139.1));
        let (mut below, mut above) = (138.9, 139.1);
        for _ in 0..60 {
            let middle = f64::midpoint(below, above);
            let alpha = Radians::new(middle * DEG);
            if template_phase(earth, alpha) < PHASE_F_CLAMP * shape_phase(0.0, alpha) {
                below = middle;
            } else {
                above = middle;
            }
        }
        assert!(
            (below - 139.006).abs() < 5e-4,
            "the clamp acts from {below}°"
        );
        let end = earth.row().valid_to;
        let unclamped = template_phase(earth, end) / shape_phase(0.0, end);
        assert!((unclamped - 5.65).abs() < 0.005, "{unclamped}");
        assert_same_bits(held_factor(earth, 0.0, 1.0), PHASE_F_CLAMP);
        let alone = simpson(robinson_eq14);
        assert!((alone - 1.350).abs() < 5e-4, "{alone}");
        let q = earth.row().phase_integral.v;
        assert!((q - 1.3116).abs() < 5e-5, "{q}");
        assert!((0.23 * q / 0.294 - 1.0).abs() < 0.03, "{}", 0.23 * q);
    }

    fn airless(material: SurfaceMaterial, state: SurfaceState, bond: f64) -> PhotometryInputs {
        PhotometryInputs {
            state,
            material,
            surface_pressure: None,
            cloud_fraction: 0.0,
            surface_temperature: Kelvin::new(100.0),
            equilibrium_temperature: Kelvin::new(100.0),
            surface_gravity: MetresPerSecondSquared::new(1.4),
            class: PlanetClass::Icy,
            fractions: MassFractions::solid(0.1, 0.4, 0.5),
            bond: BondAlbedo::new(bond).unwrap(),
        }
    }

    /// (b) An airless-ice body and a snowball state a ratio within 5% of 1 (0.98 and 0.99); the
    /// rest of the rule; two calls agree bit for bit.
    #[test]
    fn the_borrowed_curves_keep_their_ratios() {
        let ice =
            BodyPhotometry::derive(&airless(SurfaceMaterial::Ice, SurfaceState::Airless, 0.35));
        assert_eq!(ice.template(), PhaseTemplate::AirlessIce);
        assert!(ice.provisional());
        assert!((ice.bond_ratio() - 0.983).abs() < 0.05 && (ice.bond_ratio() - 1.0).abs() < 0.05);
        let snow =
            BodyPhotometry::derive(&airless(SurfaceMaterial::Ice, SurfaceState::Snowball, 0.50));
        assert_eq!(snow.template(), PhaseTemplate::Snowball);
        assert!(
            (snow.bond_ratio() - 1.0).abs() < 0.05,
            "{}",
            snow.bond_ratio()
        );
        assert!((snow.geometric_albedo().v - 0.67 * 0.50 / 0.68).abs() < 1e-12);
        let magma = |pressure: f64| {
            BodyPhotometry::derive(&PhotometryInputs {
                surface_pressure: Some(Pascals::new(pressure)),
                ..airless(SurfaceMaterial::Rock, SurfaceState::MagmaOcean, 0.10)
            })
        };
        assert_eq!(magma(1e3).template(), PhaseTemplate::Magma);
        assert!((magma(1e3).geometric_albedo().v - 0.142 * 0.10 / 0.11).abs() < 1e-12);
        assert_eq!(magma(1e7).template(), PhaseTemplate::Venus);
        let temperate = |pressure: f64| {
            BodyPhotometry::derive(&PhotometryInputs {
                surface_pressure: Some(Pascals::new(pressure)),
                ..airless(SurfaceMaterial::Rock, SurfaceState::Temperate, 0.294)
            })
            .template()
        };
        assert_eq!(temperate(29_999.0), PhaseTemplate::Mars);
        assert_eq!(temperate(30_000.0), PhaseTemplate::Earth);
        let rock = airless(SurfaceMaterial::Rock, SurfaceState::Airless, 0.11);
        let a = BodyPhotometry::derive(&rock);
        let b = BodyPhotometry::derive(&rock);
        assert_eq!(a.template(), PhaseTemplate::Mercury);
        for (x, y) in [
            (a.geometric_albedo().b, b.geometric_albedo().b),
            (a.geometric_albedo().v, b.geometric_albedo().v),
            (a.geometric_albedo().r, b.geometric_albedo().r),
            (a.bond_ratio(), b.bond_ratio()),
        ] {
            assert_same_bits(x, y);
        }
    }

    /// (b) A gas giant at an equilibrium temperature of 300 K is provisional, one at 120 K is not
    /// (decision-p14-phase-j A8); heavy elements choose Jupiter's or Saturn's template.
    #[test]
    fn hot_giants_are_provisional() {
        let giant = |z: f64, t: f64| PhotometryInputs {
            state: SurfaceState::GasEnvelope,
            class: PlanetClass::GasGiant,
            fractions: MassFractions::of(CoreComposition::new(0.3, 0.5).unwrap(), 1.0 - z),
            equilibrium_temperature: Kelvin::new(t),
            surface_temperature: Kelvin::new(t),
            bond: BondAlbedo::new(0.34).unwrap(),
            ..airless(SurfaceMaterial::Rock, SurfaceState::GasEnvelope, 0.34)
        };
        assert!(BodyPhotometry::derive(&giant(0.18, 300.0)).provisional());
        assert!(!BodyPhotometry::derive(&giant(0.18, 120.0)).provisional());
        assert_eq!(
            BodyPhotometry::derive(&giant(0.18, 120.0)).template(),
            PhaseTemplate::Jupiter
        );
        assert_eq!(
            BodyPhotometry::derive(&giant(0.29, 120.0)).template(),
            PhaseTemplate::Saturn
        );
        let neptune = PhotometryInputs {
            class: PlanetClass::SubNeptune,
            ..giant(0.9, 400.0)
        };
        let photometry = BodyPhotometry::derive(&neptune);
        assert_eq!(photometry.template(), PhaseTemplate::Neptune);
        assert!(photometry.provisional());
    }
}
