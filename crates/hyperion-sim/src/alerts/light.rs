//! A source's light in each band (plan 12, P12.T5.b): what a band's luminosity is, a blackbody's
//! share of its light in a band, and plan 09's supernova templates band by band.
//!
//! # A band's luminosity
//!
//! A band's luminosity, and a sensor's limit in it ([`BandLimits`]), are in-band powers: the
//! source's light through a top hat of the band's full width at half maximum centred on plan 07's
//! effective wavelength ([`Band::wavelength`]), W, and its flux at the observer, W m⁻². The widths
//! are Bessell's (2005, ARA&A 43, 293, Table 1) for U to I and Tokunaga's (2000, Allen's
//! Astrophysical Quantities, 4th ed., Table 7.5) for J, H and K ([`TOP_HAT_FWHM_UM`]). The mid
//! infrared takes Tokunaga's N band, 7.88–13.07 µm, whole ([`N_BAND_UM`]). Radio has no thermal
//! share, and no source here is modelled in synchrotron, so its share is zero. X-rays are the 2–10
//! keV band of Design note 14.
//!
//! # A blackbody's share
//!
//! [`band_share`] is the share of a blackbody's light between a band's edges, 15/π⁴ ∫ x³ ÷ (eˣ − 1)
//! dx in x = hc ÷ λkT (x = E ÷ kT in X-rays), by a fixed 16-point Gauss–Legendre rule across the
//! band, which matches a converged integral to a few parts in 10¹⁶ wherever the share passes
//! 10⁻¹². A 10⁴ K blackbody puts 0.0872 of its light in V and a 6,000 K one 0.110; none puts more
//! than 0.1122, at 6,700 K, through the V top hat.
//!
//! # The supernovae
//!
//! Plan 09's templates are bolometric sums of three terms ([`LightCurve::terms_at`]). Each term of
//! each template takes a spectrum, and a band's luminosity is Σₖ Lₖ(t) sₖ(band) over the terms
//! ([`supernova_luminosity`]). The spectra are constants of the generator version:
//!
//! | Spectrum             | Shares                              | Terms                           |
//! | -------------------- | ----------------------------------- | ------------------------------- |
//! | red-supergiant flash | a 2.9 × 10⁵ K blackbody's           | the IIP, IIL and IIb breakout   |
//! | stripped flash       | a 1 keV blackbody's                 | the Ib and Ic breakout          |
//! | hydrogen plateau     | V by `BC_V` +0.14, 6,000 K colours  | the IIP and IIL plateau         |
//! | Type Ia photosphere  | V by `BC_V` +0.087, 10⁴ K colours   | the Type Ia's radioactive light |
//! | warm photosphere     | a 7,000 K blackbody's               | every other radioactive light   |
//!
//! - **The breakouts.** A red supergiant's breaks out at about 2.9 × 10⁵ K (Nakar and Sari 2010,
//!   ApJ 725, 904, equation A-15). The IIb's progenitor, a yellow supergiant, takes the same, as no
//!   source for its breakout was read (lean). A Wolf–Rayet star's lies at 1–2 keV, which puts
//!   0.72–0.84 of its light in 2–10 keV; 1 keV, the range's low end, puts 0.809 there (lean). That
//!   is the core collapses' X-ray light: the red supergiants' flash, at 25 eV, puts 10⁻³⁰ there.
//! - **Observed V shares.** Where a bolometric correction is measured, the V share is the in-band
//!   power a V magnitude implies through the top hat, `F_V0` `Δλ_V` ÷ f₀ × 10^(0.4 `BC_V`). Here
//!   `F_V0` = 3.631 × 10⁻¹¹ W m⁻² nm⁻¹ is the zero point of V (Bessell, Castelli and Plez 1998, A&A
//!   333, 231, Table A2), `Δλ_V` = 84 nm, and f₀ = 2.518 021 002 × 10⁻⁸ W m⁻² the zero point of
//!   apparent bolometric magnitude (IAU 2015 Resolution B2), so the share is 0.121 13 × 10^(0.4
//!   `BC_V`). The other bands keep a blackbody's colours, its shares scaled by the V share's ratio
//!   to its own. No blackbody reaches the observed V shares of the plateau or of a Type Ia's peak.
//!   - The plateau: `BC_V` = +0.14 on the IAU scale, Bersten and Hamuy's (2009, ApJ 701, 200,
//!     §3.1) plateau, zero on their own zero point, moved to the IAU's; Lyman, Bersier and James
//!     (2014, MNRAS 437, 3848, Table 3) give +0.06 to +0.19. The V share is 0.138, against a 6,000
//!     K blackbody's 0.110.
//!   - A Type Ia's peak: `BC_V` = +0.087, from SN 2011fe's `L_max,bol` = 1.17 × 10⁴³ erg/s and
//!     `M_V,max` = −19.06 at one distance modulus (Pereira et al. 2013, A&A 554, A27), `M_bol` =
//!     −18.973 on the IAU scale (L₀ = 3.0128 × 10²⁸ W). The V share is 0.131, against a 10⁴ K
//!     blackbody's 0.087; the colours are a 10⁴ K blackbody's (lean).
//! - **The other radioactive light**, of the core collapses' ejecta and of a Type Iax, takes a
//!   7,000 K blackbody's shares (lean).
//!
//! A template's peak in a band ([`supernova_peak`]) is the greatest of its light in the band on a
//! fixed grid of ages: the explosion, every hour of the first day, and every twentieth of a day
//! from the first to 1,096 days. The rows' bounds, one per band ([`CORE_COLLAPSE_BAND_PEAKS`],
//! [`TYPE_IA_BAND_PEAKS`]), are the greatest light of their kinds' templates at any age, rounded
//! up at the fifth figure.
//!
//! [`BandLimits`]: super::BandLimits
//! [`CORE_COLLAPSE_BAND_PEAKS`]: super::CORE_COLLAPSE_BAND_PEAKS
//! [`TYPE_IA_BAND_PEAKS`]: super::TYPE_IA_BAND_PEAKS

use std::f64::consts::PI;

use crate::galaxy::catalogue_classes::supernova::{LightCurve, SupernovaKind};
use crate::galaxy::catalogue_classes::type_ia::IaChannel;
use crate::galaxy::gas::ccm::Band;
use crate::stellar::remnant::SupernovaType;
use crate::substance::thermo::SECOND_RADIATION_CONSTANT_CM_K;
use crate::units::consts::BOLTZMANN_CONSTANT;
use crate::units::{Kelvin, Watts, Years};

use super::reach::XRAY_BAND_KEV;
use super::{AlertBand, BAND_COUNT};

/// The top hats' full widths at half maximum of U, B, V, R and I, µm: Bessell's (2005, ARA&A 43,
/// 293, Table 1) Δλ of 650, 890, 840, 1,580 and 1,540 Å; and of J, H and K: Tokunaga's (2000,
/// Allen's Astrophysical Quantities, 4th ed., Table 7.5) Δλ of 0.26, 0.29 and 0.41 µm. Each is
/// centred on plan 07's [`Band::wavelength`]. Part of the generator version.
pub const TOP_HAT_FWHM_UM: [f64; 8] = [0.065, 0.089, 0.084, 0.158, 0.154, 0.26, 0.29, 0.41];

/// The mid infrared's top hat, µm: Tokunaga's (2000, Table 7.5) N band whole, λ 10.472 µm and Δλ
/// 5.19 µm, the 8–13 µm window to its rounding. Part of the generator version.
pub const N_BAND_UM: (f64, f64) = (7.88, 13.07);

/// The second radiation constant c₂ = hc ÷ `k_B`, µm K: 14,387.768 775 (CODATA 2018, exact).
const SECOND_RADIATION_CONSTANT_UM_K: f64 = SECOND_RADIATION_CONSTANT_CM_K * 1e4;

/// Joules in a kiloelectronvolt: 1.602 176 634 × 10⁻¹⁶, from the elementary charge, exact in the
/// 2019 SI.
const JOULES_PER_KILOELECTRONVOLT: f64 = 1.602_176_634e-16;

/// `k_B` in keV per kelvin, 8.617 333 × 10⁻⁸: a blackbody of kT = 1 keV is at 1.160 45 × 10⁷ K.
const KILOELECTRONVOLTS_PER_KELVIN: f64 = BOLTZMANN_CONSTANT / JOULES_PER_KILOELECTRONVOLT;

/// The 16-point Gauss–Legendre rule on [−1, 1]: each positive abscissa with its weight, the
/// negative ones mirroring them, at full precision (Newton's iteration on P₁₆, which a test
/// repeats).
const GAUSS_LEGENDRE_16: [(f64, f64); 8] = [
    (0.989_400_934_991_649_9, 0.027_152_459_411_754_096),
    (0.944_575_023_073_232_6, 0.062_253_523_938_647_894),
    (0.865_631_202_387_831_8, 0.095_158_511_682_492_79),
    (0.755_404_408_355_003, 0.124_628_971_255_533_88),
    (0.617_876_244_402_643_8, 0.149_595_988_816_576_74),
    (0.458_016_777_657_227_37, 0.169_156_519_395_002_54),
    (0.281_603_550_779_258_9, 0.182_603_415_044_923_58),
    (0.095_012_509_837_637_44, 0.189_450_610_455_068_5),
];

/// The share of a blackbody's light at `temperature` that falls in `band` (module documentation):
/// 15/π⁴ ∫ x³ ÷ (eˣ − 1) dx between the band's edges, x = hc ÷ λkT through a photometric band's
/// top hat and E ÷ kT across 2–10 keV.
///
/// Radio has none, and neither has a temperature that is not positive and finite. Shares of
/// different bands may not sum to 1: the top hats leave gaps.
///
/// # Examples
///
/// The Sun's nominal temperature puts about a ninth of its light in V, and a supernova's shock
/// breakout from a Wolf–Rayet star, at 1 keV, four-fifths in 2–10 keV X-rays:
///
/// ```
/// use hyperion_sim::alerts::{AlertBand, band_share};
/// use hyperion_sim::galaxy::gas::ccm::Band;
/// use hyperion_sim::units::Kelvin;
///
/// let sun = band_share(Kelvin::new(5_772.0), AlertBand::Photometric(Band::V));
/// assert!((sun - 0.1075).abs() < 1e-4);
/// let flash = band_share(Kelvin::new(1.160_45e7), AlertBand::XRay);
/// assert!((flash - 0.809).abs() < 1e-3);
/// assert_eq!(band_share(Kelvin::new(1e4), AlertBand::Photometric(Band::Radio)), 0.0);
/// ```
#[must_use]
pub fn band_share(temperature: Kelvin, band: AlertBand) -> f64 {
    let t = temperature.value();
    if !(t.is_finite() && t > 0.0) {
        return 0.0;
    }
    match edges(band) {
        Edges::Wavelengths { short_um, long_um } => planck_share(
            SECOND_RADIATION_CONSTANT_UM_K / (long_um * t),
            SECOND_RADIATION_CONSTANT_UM_K / (short_um * t),
        ),
        Edges::Energies { low_kev, high_kev } => {
            let kt = KILOELECTRONVOLTS_PER_KELVIN * t;
            planck_share(low_kev / kt, high_kev / kt)
        }
        Edges::None => 0.0,
    }
}

/// A band's edges, as a blackbody's x reads them.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Edges {
    /// A top hat between two wavelengths, µm.
    Wavelengths { short_um: f64, long_um: f64 },
    /// A band between two photon energies, keV.
    Energies { low_kev: f64, high_kev: f64 },
    /// No thermal light: radio.
    None,
}

/// The edges of `band` (module documentation).
#[must_use]
fn edges(band: AlertBand) -> Edges {
    let photometric = match band {
        AlertBand::Photometric(band) => band,
        AlertBand::XRay => {
            let (low_kev, high_kev) = XRAY_BAND_KEV;
            return Edges::Energies { low_kev, high_kev };
        }
    };
    let fwhm = match photometric {
        Band::U => TOP_HAT_FWHM_UM[0],
        Band::B => TOP_HAT_FWHM_UM[1],
        Band::V => TOP_HAT_FWHM_UM[2],
        Band::R => TOP_HAT_FWHM_UM[3],
        Band::I => TOP_HAT_FWHM_UM[4],
        Band::J => TOP_HAT_FWHM_UM[5],
        Band::H => TOP_HAT_FWHM_UM[6],
        Band::K => TOP_HAT_FWHM_UM[7],
        Band::MidInfrared => {
            let (short_um, long_um) = N_BAND_UM;
            return Edges::Wavelengths { short_um, long_um };
        }
        Band::Radio => return Edges::None,
    };
    let Some(centre) = photometric.wavelength() else {
        return Edges::None;
    };
    let half = fwhm / 2.0;
    Edges::Wavelengths {
        short_um: centre.value() - half,
        long_um: centre.value() + half,
    }
}

/// 15/π⁴ ∫ x³ ÷ (eˣ − 1) dx from `low` to `high` by the 16-point rule, the nodes summed in a fixed
/// order. Where eˣ − 1 overflows the integrand is 0, as it all but is.
#[must_use]
fn planck_share(low: f64, high: f64) -> f64 {
    let (middle, half) = (f64::midpoint(low, high), (high - low) / 2.0);
    let integrand = |x: f64| x * x * x / crate::math::exp_m1(x);
    let sum = GAUSS_LEGENDRE_16
        .iter()
        .fold(0.0, |sum, &(abscissa, weight)| {
            let offset = half * abscissa;
            sum + weight * (integrand(middle - offset) + integrand(middle + offset))
        });
    sum * half * 15.0 / (PI * PI * PI * PI)
}

/// One spectrum's shares of its light in each band, in [`AlertBand::ALL`]'s order.
type BandShares = [f64; BAND_COUNT];

/// What a spectrum's shares are computed from (module documentation).
#[derive(Debug, Clone, Copy)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the spectra's sources are the stored shares' record, which a test recomputes to \
                  the bit"
    )
)]
struct SpectrumSource {
    /// The blackbody whose shares, or whose colours, the spectrum takes, K.
    temperature: f64,
    /// The observed bolometric correction in V on the IAU scale that sets the V share, the other
    /// bands keeping the blackbody's colours; `None` for the blackbody's own shares.
    bc_v: Option<f64>,
}

/// The spectra of the supernova templates' terms (module documentation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Spectrum {
    /// A red supergiant's shock breakout: a 2.9 × 10⁵ K blackbody.
    RedSupergiantFlash,
    /// A stripped star's shock breakout: a 1 keV blackbody.
    StrippedFlash,
    /// A hydrogen envelope's plateau: V by `BC_V` = +0.14, 6,000 K colours.
    HydrogenPlateau,
    /// A Type Ia's photosphere: V by `BC_V` = +0.087, 10⁴ K colours.
    TypeIaPhotosphere,
    /// Every other radioactive light: a 7,000 K blackbody.
    WarmPhotosphere,
}

impl Spectrum {
    /// The spectrum's place in [`SPECTRA`].
    #[must_use]
    const fn index(self) -> usize {
        match self {
            Self::RedSupergiantFlash => 0,
            Self::StrippedFlash => 1,
            Self::HydrogenPlateau => 2,
            Self::TypeIaPhotosphere => 3,
            Self::WarmPhotosphere => 4,
        }
    }
}

/// The spectra's sources, in [`Spectrum::index`]'s order (module documentation). Part of the
/// generator version.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the spectra's sources are the stored shares' record, which a test recomputes to \
                  the bit"
    )
)]
const SPECTRUM_SOURCES: [SpectrumSource; 5] = [
    // Nakar and Sari 2010, equation A-15.
    SpectrumSource {
        temperature: 2.9e5,
        bc_v: None,
    },
    // kT = 1 keV.
    SpectrumSource {
        temperature: 1.0 / KILOELECTRONVOLTS_PER_KELVIN,
        bc_v: None,
    },
    // Bersten and Hamuy 2009, §3.1, on the IAU scale.
    SpectrumSource {
        temperature: 6_000.0,
        bc_v: Some(0.14),
    },
    // SN 2011fe (Pereira et al. 2013).
    SpectrumSource {
        temperature: 1e4,
        bc_v: Some(0.087),
    },
    SpectrumSource {
        temperature: 7_000.0,
        bc_v: None,
    },
];

/// The spectra's shares in each band, in [`Spectrum::index`]'s order: [`band_share`] of each
/// source's blackbody, scaled to its observed V share where it has one. Stored at full precision
/// and recomputed from [`SPECTRUM_SOURCES`] to the bit by a test. Part of the generator version.
const SPECTRA: [BandShares; 5] = [
    // The red-supergiant flash.
    [
        6.968_438_241_151_751e-5,
        4.361_024_894_362_991e-5,
        1.681_284_330_613_425_5e-5,
        1.790_772_022_977_919_4e-5,
        7.433_138_375_724_893e-6,
        2.034_877_598_977_547_2e-6,
        7.435_789_592_416_123e-7,
        3.349_737_435_442_941_7e-7,
        9.977_064_253_084_891e-9,
        0.0,
        1.174_359_855_924_703_8e-30,
    ],
    // The stripped flash.
    [
        1.165_134_828_153_161_6e-9,
        7.201_467_401_608_695e-10,
        2.744_025_696_862_248e-10,
        2.906_037_208_593_359_5e-10,
        1.196_924_145_995_337e-10,
        3.239_190_358_735_491_4e-11,
        1.177_890_867_718_492_3e-11,
        5.286_607_277_715_029_6e-12,
        1.561_086_930_973_360_8e-13,
        0.0,
        0.809_305_257_505_217_4,
    ],
    // The hydrogen plateau.
    [
        0.087_492_417_305_823_63,
        0.147_588_149_190_211_07,
        0.137_799_038_375_649_64,
        0.226_880_560_618_553_63,
        0.162_254_106_725_811_34,
        0.095_326_618_676_249_77,
        0.046_996_842_807_252_49,
        0.026_308_359_428_028_862,
        0.001_247_070_722_136_311,
        0.0,
        0.0,
    ],
    // The Type Ia photosphere.
    [
        0.199_208_312_364_355_35,
        0.212_044_641_161_824_3,
        0.131_233_945_787_097_57,
        0.175_431_489_361_194_76,
        0.097_343_511_077_836_29,
        0.040_090_599_492_884_57,
        0.017_246_790_934_986_385,
        0.008_760_001_209_796_935,
        0.000_339_753_994_218_141_5,
        0.0,
        0.0,
    ],
    // The warm photosphere.
    [
        0.097_479_132_544_785_98,
        0.139_137_513_568_228_65,
        0.111_842_588_471_694_94,
        0.170_403_186_190_533_63,
        0.110_974_967_100_400_13,
        0.057_121_639_372_817_085,
        0.026_770_948_695_243_28,
        0.014_456_316_478_304_558,
        0.000_636_805_701_344_134_7,
        0.0,
        0.0,
    ],
];

/// A supernova template, of [`LightCurve::of`]'s six.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Template {
    /// IIP.
    Plateau,
    /// IIL.
    Linear,
    /// IIb, partly stripped of its hydrogen.
    PartlyStripped,
    /// Ib and Ic share one.
    Stripped,
    /// The merger, double-detonation and hydrogen-donor channels share one.
    TypeIa,
    Iax,
}

impl Template {
    /// Every template, in [`index`](Self::index)'s order.
    #[cfg(test)]
    const ALL: [Self; 6] = [
        Self::Plateau,
        Self::Linear,
        Self::PartlyStripped,
        Self::Stripped,
        Self::TypeIa,
        Self::Iax,
    ];

    /// The template of `kind`, or `None` for a core collapse that made no supernova.
    #[must_use]
    const fn of(kind: SupernovaKind) -> Option<Self> {
        match kind {
            SupernovaKind::CoreCollapse(supernova) => match supernova {
                SupernovaType::IIP => Some(Self::Plateau),
                SupernovaType::IIL => Some(Self::Linear),
                SupernovaType::IIb => Some(Self::PartlyStripped),
                SupernovaType::Ib | SupernovaType::Ic => Some(Self::Stripped),
                SupernovaType::None => None,
            },
            SupernovaKind::TypeIa(IaChannel::Iax) => Some(Self::Iax),
            SupernovaKind::TypeIa(
                IaChannel::Merger | IaChannel::DoubleDetonation | IaChannel::HydrogenDonor,
            ) => Some(Self::TypeIa),
        }
    }

    /// A kind the template is of.
    #[cfg(test)]
    #[must_use]
    const fn kind(self) -> SupernovaKind {
        match self {
            Self::Plateau => SupernovaKind::CoreCollapse(SupernovaType::IIP),
            Self::Linear => SupernovaKind::CoreCollapse(SupernovaType::IIL),
            Self::PartlyStripped => SupernovaKind::CoreCollapse(SupernovaType::IIb),
            Self::Stripped => SupernovaKind::CoreCollapse(SupernovaType::Ib),
            Self::TypeIa => SupernovaKind::TypeIa(IaChannel::Merger),
            Self::Iax => SupernovaKind::TypeIa(IaChannel::Iax),
        }
    }

    /// The template's place in [`TEMPLATE_PEAKS_W`].
    #[must_use]
    const fn index(self) -> usize {
        match self {
            Self::Plateau => 0,
            Self::Linear => 1,
            Self::PartlyStripped => 2,
            Self::Stripped => 3,
            Self::TypeIa => 4,
            Self::Iax => 5,
        }
    }

    /// The spectra of the template's breakout, plateau and radioactive terms; `None` for a term
    /// the template lacks (no Type Ia breaks out, and only IIP and IIL have a plateau).
    #[must_use]
    const fn spectra(self) -> [Option<Spectrum>; 3] {
        match self {
            Self::Plateau | Self::Linear => [
                Some(Spectrum::RedSupergiantFlash),
                Some(Spectrum::HydrogenPlateau),
                Some(Spectrum::WarmPhotosphere),
            ],
            Self::PartlyStripped => [
                Some(Spectrum::RedSupergiantFlash),
                None,
                Some(Spectrum::WarmPhotosphere),
            ],
            Self::Stripped => [
                Some(Spectrum::StrippedFlash),
                None,
                Some(Spectrum::WarmPhotosphere),
            ],
            Self::TypeIa => [None, None, Some(Spectrum::TypeIaPhotosphere)],
            Self::Iax => [None, None, Some(Spectrum::WarmPhotosphere)],
        }
    }

    /// The shares of the breakout, the plateau and the radioactive light in `band`: zero for a
    /// term the template lacks.
    #[must_use]
    fn shares(self, band: AlertBand) -> [f64; 3] {
        self.spectra().map(|spectrum| {
            spectrum.map_or(0.0, |spectrum| SPECTRA[spectrum.index()][band.index()])
        })
    }
}

/// The luminosity in `band` of a supernova of `kind`, `age` after the explosion: Σₖ Lₖ(t) sₖ(band)
/// over the template's breakout, plateau and radioactive terms (module documentation), summed as
/// [`LightCurve::at`] sums them. Zero before the explosion, for a core collapse that made no
/// supernova, and in radio.
///
/// # Examples
///
/// A Type Ia at its peak puts about 0.13 of its light in V, as SN 2011fe did; a plateau supernova's
/// breakout puts most of its in the ultraviolet beyond U, and the core collapses' X-rays come from
/// the stripped stars' breakout:
///
/// ```
/// use hyperion_sim::alerts::{AlertBand, supernova_luminosity};
/// use hyperion_sim::galaxy::catalogue_classes::supernova::{LightCurve, SupernovaKind};
/// use hyperion_sim::galaxy::catalogue_classes::type_ia::IaChannel;
/// use hyperion_sim::galaxy::gas::ccm::Band;
/// use hyperion_sim::stellar::remnant::SupernovaType;
/// use hyperion_sim::units::Years;
///
/// let v = AlertBand::Photometric(Band::V);
/// let ia = SupernovaKind::TypeIa(IaChannel::Merger);
/// let peak = Years::new(15.0 / 365.25);
/// let share = supernova_luminosity(ia, peak, v).value() / LightCurve::luminosity(ia, peak).value();
/// assert!((share - 0.131).abs() < 1e-3);
/// let iip = SupernovaKind::CoreCollapse(SupernovaType::IIP);
/// let flash = supernova_luminosity(iip, Years::ZERO, v).value();
/// assert!(flash < 1e-4 * LightCurve::luminosity(iip, Years::ZERO).value());
/// let ib = SupernovaKind::CoreCollapse(SupernovaType::Ib);
/// assert!(supernova_luminosity(ib, Years::ZERO, AlertBand::XRay).value() > 8e36);
/// ```
#[must_use]
pub fn supernova_luminosity(kind: SupernovaKind, age: Years, band: AlertBand) -> Watts {
    match (Template::of(kind), LightCurve::of(kind)) {
        (Some(template), Some(curve)) => curve.terms_at(age).weighted(template.shares(band)),
        _ => Watts::ZERO,
    }
}

/// The peak in `band` of a supernova of `kind`: the greatest of its [`supernova_luminosity`] on
/// the fixed grid of ages (module documentation), which is the event's flux at its brightest that
/// a detection reports. Zero for a core collapse that made no supernova and in radio.
///
/// # Examples
///
/// ```
/// use hyperion_sim::alerts::{AlertBand, supernova_peak};
/// use hyperion_sim::galaxy::catalogue_classes::supernova::SupernovaKind;
/// use hyperion_sim::galaxy::catalogue_classes::type_ia::IaChannel;
/// use hyperion_sim::galaxy::gas::ccm::Band;
///
/// let b = AlertBand::Photometric(Band::B);
/// let normal = supernova_peak(SupernovaKind::TypeIa(IaChannel::Merger), b);
/// let iax = supernova_peak(SupernovaKind::TypeIa(IaChannel::Iax), b);
/// // A Type Iax is some ten times fainter than a normal Type Ia.
/// assert!(normal.value() > 8.0 * iax.value());
/// ```
#[must_use]
pub fn supernova_peak(kind: SupernovaKind, band: AlertBand) -> Watts {
    Template::of(kind).map_or(Watts::ZERO, |template| {
        Watts::new(TEMPLATE_PEAKS_W[template.index()][band.index()])
    })
}

/// Each template's peak in each band, W, in [`Template::index`]'s and [`AlertBand::ALL`]'s
/// orders: the greatest of its light in the band on the grid of ages (module documentation).
/// Stored at full precision and recomputed to the bit by a test.
const TEMPLATE_PEAKS_W: [BandShares; 6] = [
    // IIP.
    [
        1.046_167_442_348_397_2e34,
        1.720_189_286_938_959_6e34,
        1.574_279_534_820_585e34,
        2.567_794_263_476_234_8e34,
        1.817_193_830_586_113e34,
        1.053_410_131_453_063_5e34,
        5.168_933_123_158_326_7e33,
        2.884_194_521_099_187e33,
        1.358_642_987_406_714e32,
        0.0,
        117_435_985.592_470_36,
    ],
    // IIL.
    [
        1.416_067_519_949_300_7e34,
        2.365_291_644_619_344e34,
        2.192_161_108_004_235_4e34,
        3.597_169_196_453_226_5e34,
        2.563_013_875_310_810_5e34,
        1.498_853_317_800_257_4e34,
        7.377_636_852_365_771e33,
        4.125_431_058_266_431_6e33,
        1.951_449_139_472_673_4e32,
        0.0,
        117_435_985.592_470_36,
    ],
    // IIb.
    [
        1.538_306_943_447_513_8e34,
        2.195_713_047_791_963_5e34,
        1.764_974_984_159_866_7e34,
        2.689_113_019_979_356_5e34,
        1.751_283_151_406_502_4e34,
        9.014_300_001_894_045e33,
        4.224_692_525_002_641_3e33,
        2.281_334_623_595_817_6e33,
        1.004_935_729_762_035e32,
        0.0,
        117_435_985.592_470_36,
    ],
    // Ib and Ic.
    [
        1.881_860_899_369_716_6e34,
        2.686_087_161_262_682_6e34,
        2.159_151_283_301_39e34,
        3.289_679_389_305_721e34,
        2.142_401_619_127_398_3e34,
        1.102_748_627_704_647_7e34,
        5.168_203_724_573_027_4e33,
        2.790_830_818_784_365_4e33,
        1.229_370_552_004_708_3e32,
        0.0,
        8.093_052_575_052_174e36,
    ],
    // Type Ia: the merger, double-detonation and hydrogen-donor channels.
    [
        2.137_328_738_186_980_3e35,
        2.275_051_176_101_456_6e35,
        1.408_023_994_718_728_3e35,
        1.882_224_488_247_390_5e35,
        1.044_409_649_543_87e35,
        4.301_366_213_602_773e34,
        1.850_427_899_787_021_5e34,
        9.398_705_360_249_735e33,
        3.645_259_412_810_431e32,
        0.0,
        0.0,
    ],
    // Type Iax.
    [
        1.310_968_847_204_586_7e34,
        1.871_220_444_864_431e34,
        1.504_138_839_251_063_3e34,
        2.291_703_493_130_248e34,
        1.492_470_448_701_795_2e34,
        7.682_125_165_056_531e33,
        3.600_347_975_342_487_7e33,
        1.944_188_469_227_540_4e33,
        8.564_216_918_947_98e31,
        0.0,
        0.0,
    ],
];

/// The ages of the grid a template's peak is taken on (module documentation): the explosion,
/// every hour of the first day, and every twentieth of a day from the first to 1,096 days.
#[cfg(test)]
pub(super) fn peak_grid() -> impl Iterator<Item = Years> {
    let days = (0..24)
        .map(|hour| f64::from(hour) / 24.0)
        .chain((20..21_920).map(|twentieth| f64::from(twentieth) / 20.0));
    days.map(|day| Years::new(day / 365.25))
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::math;

    const V: AlertBand = AlertBand::Photometric(Band::V);

    /// The share of a V magnitude's in-band power through the top hat at a bolometric correction
    /// of zero: `F_V0` `Δλ_V` ÷ f₀ (module documentation).
    fn v_share_at_zero_bc() -> f64 {
        /// The V zero point, W m⁻² nm⁻¹ (Bessell, Castelli and Plez 1998, Table A2).
        const V_ZERO_POINT: f64 = 3.631e-11;
        /// The zero point of apparent bolometric magnitude, W m⁻² (IAU 2015 Resolution B2).
        const BOLOMETRIC_ZERO_POINT: f64 = 2.518_021_002e-8;
        V_ZERO_POINT * TOP_HAT_FWHM_UM[2] * 1e3 / BOLOMETRIC_ZERO_POINT
    }

    /// The spectra's shares recomputed from their sources.
    fn recomputed_spectra() -> [BandShares; 5] {
        SPECTRUM_SOURCES.map(|source| {
            let blackbody =
                AlertBand::ALL.map(|band| band_share(Kelvin::new(source.temperature), band));
            match source.bc_v {
                None => blackbody,
                Some(bc_v) => {
                    let scale =
                        v_share_at_zero_bc() * math::exp10(0.4 * bc_v) / blackbody[V.index()];
                    blackbody.map(|share| share * scale)
                }
            }
        })
    }

    /// Simpson's rule for the share between `low` and `high` in x, 20,000 steps: a converged
    /// reference.
    fn converged_share(low: f64, high: f64) -> f64 {
        let integrand = |x: f64| {
            let denominator = math::exp_m1(x);
            if denominator.is_finite() {
                x * x * x / denominator
            } else {
                0.0
            }
        };
        let steps = 20_000_u32;
        let step = (high - low) / f64::from(steps);
        let sum = (0..=steps).fold(0.0, |sum, i| {
            let factor = if i == 0 || i == steps {
                1.0
            } else if i % 2 == 1 {
                4.0
            } else {
                2.0
            };
            sum + factor * integrand(math::mul_add(step, f64::from(i), low))
        });
        sum * step / 3.0 * 15.0 / (PI * PI * PI * PI)
    }

    /// The stored rule is Newton's iteration on P₁₆ from Tricomi's first guesses, to the last
    /// place or two, its weights 2 ÷ ((1 − x²) P′₁₆(x)²); they sum to 2, and the rule integrates
    /// x³⁰ exactly.
    #[test]
    fn the_gauss_legendre_rule_is_newtons_on_p16() {
        let legendre = |x: f64| {
            let (mut previous, mut current) = (1.0, x);
            for k in 2..=16_u32 {
                let k = f64::from(k);
                let next = ((2.0 * k - 1.0) * x * current - (k - 1.0) * previous) / k;
                previous = current;
                current = next;
            }
            (current, 16.0 * (x * current - previous) / (x * x - 1.0))
        };
        for (i, &(abscissa, weight)) in (1..=8_u32).zip(&GAUSS_LEGENDRE_16) {
            let mut x = math::cos(PI * (f64::from(i) - 0.25) / 16.5);
            for _ in 0..50 {
                let (p, dp) = legendre(x);
                x -= p / dp;
            }
            let (_, dp) = legendre(x);
            let w = 2.0 / ((1.0 - x * x) * dp * dp);
            assert!(
                (x - abscissa).abs() <= 4.0 * f64::EPSILON,
                "{i}: {x} against {abscissa}"
            );
            assert!(
                (w / weight - 1.0).abs() <= 1e-14,
                "{i}: {w} against {weight}"
            );
        }
        let total = GAUSS_LEGENDRE_16
            .iter()
            .fold(0.0, |sum, &(_, w)| sum + 2.0 * w);
        assert!((total - 2.0).abs() < 1e-15, "{total}");
        let thirtieth = GAUSS_LEGENDRE_16
            .iter()
            .fold(0.0, |sum, &(x, w)| sum + 2.0 * w * math::powi(x, 30));
        assert!(
            (thirtieth / (2.0 / 31.0) - 1.0).abs() < 1e-13,
            "{thirtieth}"
        );
    }

    /// The rule's shares meet the converged integral's wherever they pass 10⁻¹², over the spectra's
    /// temperatures and others, and hold the figures the documentation quotes: a 10⁴ K
    /// blackbody's 0.132, 0.141 and 0.0872 in U, B and V, a 6,000 K one's 0.110 in V, the Sun's
    /// 0.1075, and a 1 keV flash's 0.809 in X-rays (0.835 at 1.5 keV, 0.720 at 2 keV).
    #[test]
    fn band_shares_match_a_converged_integral() {
        let kelvin_of_kev = |kev: f64| kev / KILOELECTRONVOLTS_PER_KELVIN;
        let temperatures = [
            3e5,
            2.9e5,
            1e5,
            3e4,
            1e4,
            7e3,
            6e3,
            5_772.0,
            3e3,
            kelvin_of_kev(1.0),
            kelvin_of_kev(2.0),
        ];
        for t in temperatures {
            for band in AlertBand::ALL {
                let share = band_share(Kelvin::new(t), band);
                let reference = match edges(band) {
                    Edges::Wavelengths { short_um, long_um } => converged_share(
                        SECOND_RADIATION_CONSTANT_UM_K / (long_um * t),
                        SECOND_RADIATION_CONSTANT_UM_K / (short_um * t),
                    ),
                    Edges::Energies { low_kev, high_kev } => {
                        let kt = KILOELECTRONVOLTS_PER_KELVIN * t;
                        converged_share(low_kev / kt, high_kev / kt)
                    }
                    Edges::None => 0.0,
                };
                if reference > 1e-12 {
                    assert!(
                        (share / reference - 1.0).abs() < 1e-12,
                        "{t} K in {band:?}: {share:e} against {reference:e}"
                    );
                } else {
                    assert!((share - reference).abs() < 1e-12, "{t} K in {band:?}");
                }
            }
            assert_eq!(
                band_share(Kelvin::new(t), AlertBand::Photometric(Band::Radio)),
                0.0
            );
            let photometric = Band::ALL.into_iter().fold(0.0, |sum, band| {
                sum + band_share(Kelvin::new(t), AlertBand::Photometric(band))
            });
            assert!(photometric < 1.0, "{t} K: {photometric}");
        }
        let share = |t: f64, band| band_share(Kelvin::new(t), band);
        for (t, band, expected, within) in [
            (1e4, AlertBand::Photometric(Band::U), 0.132_4, 1e-4),
            (1e4, AlertBand::Photometric(Band::B), 0.141_0, 1e-4),
            (1e4, V, 0.087_25, 1e-5),
            (6e3, V, 0.109_7, 1e-4),
            (5_772.0, V, 0.107_5, 1e-4),
            (kelvin_of_kev(1.0), AlertBand::XRay, 0.809_3, 1e-4),
            (kelvin_of_kev(1.5), AlertBand::XRay, 0.835_3, 1e-4),
            (kelvin_of_kev(2.0), AlertBand::XRay, 0.719_9, 1e-4),
        ] {
            let got = share(t, band);
            assert!((got - expected).abs() < within, "{t} K in {band:?}: {got}");
        }
        // No blackbody puts more than 0.1122 through V's top hat, at 6,700 K.
        let (most, at) = (40..=200)
            .map(|hundreds| (share(f64::from(hundreds) * 100.0, V), hundreds))
            .fold((0.0, 0), |a, b| if b.0 > a.0 { b } else { a });
        assert!(
            (0.112_2..0.112_3).contains(&most) && at == 67,
            "{most} at {at}00 K"
        );
        for nonsense in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(share(nonsense, V), 0.0);
        }
        assert!((kelvin_of_kev(1.0) / 1.160_45e7 - 1.0).abs() < 1e-5);
    }

    /// The stored shares are their sources' to the bit, and the observed V shares are as the
    /// documentation gives them: 0.121 13 at a correction of zero, the plateau's 0.138 and the Type
    /// Ia's 0.131.
    #[test]
    fn the_spectra_are_their_sources_shares_to_the_bit() {
        let recomputed = recomputed_spectra();
        for (stored, recomputed) in SPECTRA.iter().zip(&recomputed) {
            for (&a, &b) in stored.iter().zip(recomputed) {
                assert_same_bits(a, b);
            }
        }
        assert!((v_share_at_zero_bc() - 0.121_13).abs() < 1e-5);
        let v = |spectrum: Spectrum| SPECTRA[spectrum.index()][V.index()];
        assert!((v(Spectrum::HydrogenPlateau) - 0.137_8).abs() < 1e-4);
        assert!((v(Spectrum::TypeIaPhotosphere) - 0.131_2).abs() < 1e-4);
        assert!((v(Spectrum::WarmPhotosphere) - 0.111_8).abs() < 1e-4);
        assert!(
            (SPECTRA[Spectrum::StrippedFlash.index()][AlertBand::XRay.index()] - 0.809).abs()
                < 1e-3
        );
        // Pereira et al.'s figures give the Type Ia's correction: `M_bol` − M_V.
        let m_bol = -2.5 * math::log10(1.17e36 / 3.0128e28);
        assert!((m_bol + 19.06 - SPECTRUM_SOURCES[3].bc_v.unwrap()).abs() < 5e-4);
        for spectrum in SPECTRA {
            assert_eq!(spectrum[AlertBand::Photometric(Band::Radio).index()], 0.0);
            let photometric: f64 = spectrum[..Band::ALL.len()].iter().sum();
            assert!(photometric < 1.0, "{spectrum:?}");
        }
    }

    /// The stored peaks are the grid's to the bit.
    #[test]
    fn the_template_peaks_are_the_grids_to_the_bit() {
        for template in Template::ALL {
            let kind = template.kind();
            let mut peaks = [0.0_f64; BAND_COUNT];
            for age in peak_grid() {
                for (peak, band) in peaks.iter_mut().zip(AlertBand::ALL) {
                    *peak = peak.max(supernova_luminosity(kind, age, band).value());
                }
            }
            for (band, (&stored, recomputed)) in AlertBand::ALL
                .into_iter()
                .zip(TEMPLATE_PEAKS_W[template.index()].iter().zip(peaks))
            {
                assert_same_bits(stored, recomputed);
                assert_same_bits(supernova_peak(kind, band).value(), stored);
            }
        }
    }

    /// A band's light is each term weighted by its spectrum's share, summed as the bolometric light
    /// sums them: every channel and type of a template alike; nothing before the explosion, from a
    /// collapse that made no supernova, or in radio.
    #[test]
    fn a_bands_light_weights_each_term_by_its_share() {
        let kinds = [
            SupernovaType::IIP,
            SupernovaType::IIL,
            SupernovaType::IIb,
            SupernovaType::Ib,
            SupernovaType::Ic,
        ]
        .map(SupernovaKind::CoreCollapse)
        .into_iter()
        .chain(
            [
                IaChannel::Merger,
                IaChannel::DoubleDetonation,
                IaChannel::HydrogenDonor,
                IaChannel::Iax,
            ]
            .map(SupernovaKind::TypeIa),
        );
        for kind in kinds {
            let template = Template::of(kind).unwrap();
            let curve = LightCurve::of(kind).unwrap();
            for day in [0.0, 0.01, 1.0, 10.0, 50.0, 150.0, 400.0] {
                let age = Years::new(day / 365.25);
                let terms = curve.terms_at(age);
                for band in AlertBand::ALL {
                    let [b, p, r] = template.shares(band);
                    let expected = terms.breakout().value() * b
                        + terms.plateau().value() * p
                        + terms.radioactive().value() * r;
                    let got = supernova_luminosity(kind, age, band).value();
                    assert!(
                        (got - expected).abs() <= 1e-15 * expected,
                        "{kind:?} at {day} d in {band:?}"
                    );
                }
            }
            for band in AlertBand::ALL {
                assert_eq!(
                    supernova_luminosity(kind, Years::new(-1e-3), band),
                    Watts::ZERO
                );
            }
            let radio = AlertBand::Photometric(Band::Radio);
            assert_eq!(
                supernova_luminosity(kind, Years::new(0.05), radio),
                Watts::ZERO
            );
            assert_eq!(supernova_peak(kind, radio), Watts::ZERO);
        }
        let failed = SupernovaKind::CoreCollapse(SupernovaType::None);
        assert_eq!(
            supernova_luminosity(failed, Years::new(0.05), V),
            Watts::ZERO
        );
        assert_eq!(supernova_peak(failed, V), Watts::ZERO);
    }
}
