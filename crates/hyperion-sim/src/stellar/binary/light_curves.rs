//! The light curves of the binary events (plan 11, P11.T9.a): one closed form per kind, each a
//! value built from the event's physical parameters, with its bolometric light, its light in plan
//! 07's [`Band`]s and, for the X-ray kinds, its luminosity in the 2–10 keV band:
//!
//! - classical and recurrent novae ([`NovaCurve`]), from the white dwarf's mass: a rise, the peak
//!   and a decline by t₃;
//! - dwarf novae ([`DwarfNovaCurve`]), from the white dwarf's mass, the orbital period and the mass
//!   the disc gathered: a rise, a plateau and an exponential decay;
//! - X-ray transients ([`XrayTransientCurve`]), from the accretor, its mass, the mass gathered and
//!   its hydrogen: a fast rise and an exponential decay;
//! - Type I X-ray bursts ([`XrayBurstCurve`]), from the neutron star's mass and the fuel: the same;
//! - Be/X giant (Type II) outbursts ([`BeXOutburstCurve`]), from the neutron star's mass and the
//!   accreted hydrogen: a rise and an exponential decay;
//! - luminous red novae ([`LuminousRedNovaCurve`]), from the merger's mass: a rise, a plateau of
//!   100–200 days and a fade;
//! - kilonovae ([`KilonovaCurve`]), from the ejecta's mass: a rise and a power-law fade.
//!
//! Each kind is a struct whose `of` checks its parameters, refusing masses outside what the kind
//! can have, and [`BinaryLightCurve`] holds any of them. A curve is zero before its onset
//! (`dt` < 0), rises linearly to its peak, holds a plateau where its kind has one, and then fades,
//! exponentially or as a power law, never to zero: it is continuous from the onset on, and
//! [`fades_below`](BinaryLightCurve::fades_below) gives the time after which it stays under a share
//! of its peak. The linear rise is this module's shape, as plan 06's eruptions choose theirs; the
//! sources fix the rise time, the peak and the fade.
//!
//! # Bands
//!
//! A kind's light in a band is its bolometric light times the share of a blackbody at the kind's
//! colour temperature through the band's top hat: plan 07's [`Band::wavelength`] with the band's
//! full width at half maximum (Bessell 2005, ARA&A 43, 293, Table 1, for U to I; Tokunaga 2000,
//! Allen's Astrophysical Quantities, Table 7.5, for J, H, K and the N band's 7.88–13.07 µm for the
//! mid infrared), as plan 12 takes a flare's (`alerts::local`). Radio has no thermal share. The
//! shares are a constant table, which the test
//! `the_band_shares_are_blackbodies_through_the_top_hats` holds to its integral bit for bit.
//!
//! The X-ray kinds radiate nearly all their light as X-rays: the 2–10 keV share of a blackbody at
//! the kind's X-ray temperature ([`xray_luminosity`](BinaryLightCurve::xray_luminosity)). The
//! accreting kinds' optical light is the X-rays an outer disc reprocesses, [`REPROCESSED_SHARE`] of
//! the light at [`DISC_KELVIN`]; a Be/X outburst's optical light is the Be star's disc, which the
//! system's state holds, so the outburst gives none of its own.
//!
//! A published absolute magnitude becomes a bolometric luminosity through the same blackbody: the
//! band's light is the Sun's in that band ([`SOLAR_LUMINOSITY_W`] times a 5,772 K blackbody's
//! share) scaled by the magnitude below the Sun's (Willmer 2018, ApJS 236, 47, Table 3, Vega
//! magnitudes), and the bolometric is that over the kind's share in the band. So a kind's own band
//! reproduces the magnitude its source gives.
//!
//! # Sources
//!
//! Every figure carries its source in its doc comment. The literature could not be reached when
//! this module was written, so the figures are as recalled, not read at source in this build, as
//! plan 06's eruptions note theirs; those this module chose are marked ours.
//!
//! Nothing here draws a random number or is called by the generator yet: P11.T9.b wraps the plan's
//! `light_curve(event, dt, band)` and `xray_luminosity(event, dt)` around these curves.

use std::error::Error;
use std::fmt;

use crate::galaxy::features::centre::events::luminosity::eddington_luminosity_watts;
use crate::galaxy::gas::ccm::Band;
use crate::math;
use crate::stellar::remnant::RemnantRecipe;
use crate::stellar::remnant::structure::{
    CHANDRASEKHAR_MASS, neutron_star_radius, white_dwarf_radius,
};
use crate::time::Span;
use crate::units::consts::{
    GM_SUN, SECONDS_PER_DAY, SOLAR_LUMINOSITY_W, SOLAR_MASS_KG, SOLAR_RADIUS_M, SPEED_OF_LIGHT,
};
use crate::units::{Days, SolarMasses, Watts};

/// Watts per erg per second.
const WATTS_PER_ERG_S: f64 = 1e-7;

/// Grams per solar mass.
const GRAMS_PER_SOLAR_MASS: f64 = SOLAR_MASS_KG * 1e3;

/// The colour temperatures whose blackbody shares are tabled, in [`BAND_SHARES`]' order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Colour {
    /// The Sun, 5,772 K (IAU 2015 Resolution B3's nominal effective temperature): the zero point
    /// through which a magnitude becomes a luminosity.
    Sun,
    /// A nova at maximum, [`NOVA_KELVIN`].
    Nova,
    /// An accretion disc, [`DISC_KELVIN`]: a dwarf nova's in outburst, and an X-ray binary's outer
    /// disc reprocessing its X-rays.
    Disc,
    /// A luminous red nova's plateau, [`RED_NOVA_KELVIN`].
    RedNova,
    /// A kilonova, [`KILONOVA_KELVIN`].
    Kilonova,
}

impl Colour {
    /// Every colour, in the table's order.
    #[cfg(test)]
    const ALL: [Self; 5] = [
        Self::Sun,
        Self::Nova,
        Self::Disc,
        Self::RedNova,
        Self::Kilonova,
    ];

    /// The blackbody's temperature, K.
    #[cfg(test)]
    const fn kelvin(self) -> f64 {
        match self {
            Self::Sun => crate::units::consts::SOLAR_EFFECTIVE_TEMPERATURE_K,
            Self::Nova => NOVA_KELVIN,
            Self::Disc => DISC_KELVIN,
            Self::RedNova => RED_NOVA_KELVIN,
            Self::Kilonova => KILONOVA_KELVIN,
        }
    }

    /// The blackbody's share of its light in `band`.
    #[must_use]
    fn share(self, band: Band) -> f64 {
        let row = match self {
            Self::Sun => 0,
            Self::Nova => 1,
            Self::Disc => 2,
            Self::RedNova => 3,
            Self::Kilonova => 4,
        };
        BAND_SHARES[row][band_index(band)]
    }
}

/// `band`'s place in [`Band::ALL`].
#[must_use]
const fn band_index(band: Band) -> usize {
    match band {
        Band::U => 0,
        Band::B => 1,
        Band::V => 2,
        Band::R => 3,
        Band::I => 4,
        Band::J => 5,
        Band::H => 6,
        Band::K => 7,
        Band::MidInfrared => 8,
        Band::Radio => 9,
    }
}

/// The share of a blackbody's light in each of plan 07's bands, in [`Band::ALL`]'s order, through
/// each band's top hat (module documentation), one row per [`Colour`]: the Sun's 5,772 K, a nova's
/// [`NOVA_KELVIN`], a disc's [`DISC_KELVIN`], a red nova's [`RED_NOVA_KELVIN`] and a kilonova's
/// [`KILONOVA_KELVIN`]. None in radio. Held to the test's integral by
/// `the_band_shares_are_blackbodies_through_the_top_hats`.
const BAND_SHARES: [[f64; 10]; 5] = [
    [
        0.062_504_093_471_442_44,
        0.110_445_929_477_959_07,
        0.107_506_831_658_937_22,
        0.180_927_918_836_208_82,
        0.132_837_018_668_210_77,
        0.081_023_175_368_909_28,
        0.040_522_672_194_645_4,
        0.022_916_493_036_794_078,
        0.001_108_898_516_906_551,
        0.0,
    ],
    [
        0.117_200_811_052_208_63,
        0.147_800_885_598_752_85,
        0.106_358_781_569_491_47,
        0.153_211_081_981_637_05,
        0.093_193_269_273_453_6,
        0.043_600_804_528_762_25,
        0.019_700_125_383_887_056,
        0.010_364_162_135_077_024,
        0.000_432_638_368_818_234_36,
        0.0,
    ],
    [
        0.046_695_555_611_463_4,
        0.034_514_188_219_494_18,
        0.015_474_291_118_265_23,
        0.017_719_126_765_207_83,
        0.008_090_521_683_492_499,
        0.002_540_895_095_636_543_4,
        0.000_982_578_961_196_737,
        0.000_461_810_384_608_894_37,
        1.514_370_786_325_324_8e-5,
        0.0,
    ],
    [
        0.024_117_858_864_340_425,
        0.060_263_716_015_058_14,
        0.080_029_035_956_311_4,
        0.159_315_193_124_297_46,
        0.142_710_545_142_268_2,
        0.116_170_368_026_559_5,
        0.064_928_454_531_712_27,
        0.039_730_103_048_755_66,
        0.002_250_468_696_107_815_7,
        0.0,
    ],
    [
        0.038_245_858_899_365_81,
        0.081_650_643_015_883_94,
        0.094_176_500_995_914_29,
        0.173_555_117_530_768_33,
        0.141_966_794_944_053_45,
        0.101_178_469_007_729_92,
        0.053_720_796_674_658_28,
        0.031_696_535_695_516_71,
        0.001_670_013_988_742_862_5,
        0.0,
    ],
];

/// The X-ray temperatures whose 2–10 keV shares are tabled, in [`XRAY_SHARES`]' order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum XrayColour {
    /// An X-ray transient's disc, [`TRANSIENT_KEV`].
    Transient,
    /// A Type I burst's photosphere, [`BURST_KEV`].
    Burst,
    /// An accreting pulsar's column in a Be/X outburst, [`BEX_KEV`].
    BeX,
}

impl XrayColour {
    /// Every X-ray colour, in the table's order.
    #[cfg(test)]
    const ALL: [Self; 3] = [Self::Transient, Self::Burst, Self::BeX];

    /// kT, keV.
    #[cfg(test)]
    const fn kev(self) -> f64 {
        match self {
            Self::Transient => TRANSIENT_KEV,
            Self::Burst => BURST_KEV,
            Self::BeX => BEX_KEV,
        }
    }

    /// The blackbody's share of its light between 2 and 10 keV.
    #[must_use]
    const fn share(self) -> f64 {
        match self {
            Self::Transient => XRAY_SHARES[0],
            Self::Burst => XRAY_SHARES[1],
            Self::BeX => XRAY_SHARES[2],
        }
    }
}

/// The share of a blackbody's light between 2 and 10 keV, the X-ray band of plan 12's alerts, at
/// each [`XrayColour`]'s kT: [`TRANSIENT_KEV`], [`BURST_KEV`] and [`BEX_KEV`]. Held to the test's
/// integral by `the_band_shares_are_blackbodies_through_the_top_hats`.
const XRAY_SHARES: [f64; 3] = [
    0.472_920_637_454_589_34,
    0.719_915_398_143_800_9,
    0.452_906_849_078_748_5,
];

/// A nova's colour temperature at maximum light, K: 8,000.
///
/// At maximum a nova's spectrum is an A–F supergiant's (Warner 2008, in Bode and Evans, Classical
/// Novae, 2nd ed., chapter 2); 8,000 K is ours, the middle of that range.
pub const NOVA_KELVIN: f64 = 8_000.0;

/// The colour temperature of an accretion disc's light, K: 25,000 (ours, provisional).
///
/// A disc is no one temperature: a steady disc at a dwarf nova's peak rate is some 50,000 K inside
/// and 6,000 K at its rim (Frank, King and Raine 2002, Accretion Power in Astrophysics, 3rd ed.,
/// section 5.5), and most of its light is ultraviolet. 25,000 K puts 1.5% of the light in V, so
/// that a dwarf nova at an absolute V magnitude of 4 at maximum, Warner's at 6.6 hours, radiates
/// some 6 × 10³⁴ erg s⁻¹ in all, and a bolometric luminosity read from a V magnitude is not
/// under-counted as an optical colour would leave it. An X-ray binary's irradiated outer disc,
/// 20,000–30,000 K, takes the same.
pub const DISC_KELVIN: f64 = 25_000.0;

/// A luminous red nova's colour temperature on its plateau, K: 4,500.
///
/// It is the temperature at which hydrogen in the ejecta recombines, to which Ivanova et al. (2013,
/// Science 339, 433) normalise their plateau.
pub const RED_NOVA_KELVIN: f64 = 4_500.0;

/// A kilonova's colour temperature, K: 5,000 (ours).
///
/// AT 2017gfo cooled from about 11,000 K half a day after GW170817 to 5,000 K at a day and a half
/// and 2,500–3,000 K after five days (Drout et al. 2017, Science 358, 1570; Kasliwal et al. 2017,
/// Science 358, 1559); 5,000 K stands for the days that carry most of its light.
pub const KILONOVA_KELVIN: f64 = 5_000.0;

/// The X-ray temperature that stands for an X-ray transient's disc, kT in keV: 0.55.
///
/// In an outburst's thermal state the inner disc is at 0.7–1.5 keV (Remillard and `McClintock`
/// 2006, ARA&A 44, 49). A disc whose inner edge is at 1 keV, its temperature falling as r^(−¾),
/// puts 45% of its light between 2 and 10 keV, and a single blackbody at 0.55 keV 47% (ours, both
/// integrated for this module), so the blackbody stands for the disc.
pub const TRANSIENT_KEV: f64 = 0.55;

/// A Type I X-ray burst's X-ray temperature, kT in keV: 2.
///
/// Bursts peak at 2–3 keV and cool through their tails (Galloway et al. 2008, ApJS 179, 360;
/// Lewin, van Paradijs and Taam 1993, Space Sci. Rev. 62, 223); 2 keV is ours, the burst's
/// light-weighted mean.
pub const BURST_KEV: f64 = 2.0;

/// The X-ray temperature that stands for a Be/X outburst's spectrum, kT in keV: 3 (ours,
/// provisional).
///
/// An accreting pulsar's spectrum is a hard power law, photon index about 1, cut off at 10–20 keV
/// (Coburn et al. 2002, ApJ 580, 394), which puts 0.23–0.45 of its light between 2 and 10 keV by
/// its cutoff; a blackbody at 3 keV puts 0.45, the top of that range.
pub const BEX_KEV: f64 = 3.0;

/// The share of an accreting X-ray kind's light that its outer disc reprocesses into the optical
/// and ultraviolet at [`DISC_KELVIN`]: 0.02 (ours, provisional).
///
/// The disc intercepts and absorbs a few per cent of the central X-rays (de Jong, van Paradijs and
/// Augusteijn 1996, A&A 314, 484). 0.02 gives a neutron-star transient at a fifth of its Eddington
/// luminosity on an eight-hour orbit the absolute V magnitude of van Paradijs and `McClintock`'s
/// (1994, A&A 290, 133) relation within half a magnitude, which a test holds.
pub const REPROCESSED_SHARE: f64 = 0.02;

/// The bands in which a source here quotes an absolute magnitude.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MagnitudeBand {
    /// Johnson B.
    B,
    /// Johnson V.
    V,
}

impl MagnitudeBand {
    /// Plan 07's band.
    #[must_use]
    const fn band(self) -> Band {
        match self {
            Self::B => Band::B,
            Self::V => Band::V,
        }
    }

    /// The Sun's absolute magnitude in the band: 5.44 in B and 4.81 in V (Willmer 2018, ApJS 236,
    /// 47, Table 3, Vega magnitudes).
    #[must_use]
    const fn solar_absolute_magnitude(self) -> f64 {
        match self {
            Self::B => 5.44,
            Self::V => 4.81,
        }
    }
}

/// The bolometric luminosity, W, of a source of `colour` whose absolute magnitude in `band` is
/// `magnitude` (module documentation): the Sun's light in the band, scaled by the magnitude below
/// the Sun's, over the colour's share in the band.
#[must_use]
fn luminosity_from_magnitude(band: MagnitudeBand, magnitude: f64, colour: Colour) -> f64 {
    let solar_in_band = SOLAR_LUMINOSITY_W * Colour::Sun.share(band.band());
    solar_in_band * math::exp10(0.4 * (band.solar_absolute_magnitude() - magnitude))
        / colour.share(band.band())
}

/// The Eddington luminosity of a star of `mass` whose atmosphere has the hydrogen mass fraction
/// `hydrogen`, from electron scattering in fully ionised matter.
///
/// It is `4π G M c ÷ κ` with κ = `σ_T` (1 + X) ÷ 2 `m_p`, about 0.2 (1 + X) cm² g⁻¹ (Frank, King
/// and Raine 2002, chapter 1): 1.257 × 10³¹ W per M☉ for pure hydrogen (the centre's
/// [`eddington_luminosity_watts`]), and twice that for helium. A 1.4 M☉ neutron star's helium
/// Eddington luminosity is 3.52 × 10³¹ W, 3.52 × 10³⁸ erg s⁻¹, against the 3.8 × 10³⁸ erg s⁻¹ of
/// photospheric-radius-expansion bursts (Kuulkers et al. 2003, A&A 399, 663). The luminosity is
/// linear in any mass; `hydrogen` is held to 0–1, and a NaN gives a NaN.
///
/// # Examples
///
/// ```
/// use hyperion_sim::stellar::binary::light_curves::eddington_luminosity;
/// use hyperion_sim::units::SolarMasses;
///
/// let helium = eddington_luminosity(SolarMasses::new(1.4), 0.0);
/// let solar = eddington_luminosity(SolarMasses::new(1.4), 0.7);
/// assert!((helium.value() / solar.value() - 1.7).abs() < 1e-12);
/// ```
#[must_use]
pub fn eddington_luminosity(mass: SolarMasses, hydrogen: f64) -> Watts {
    Watts::new(eddington_luminosity_watts(mass) * 2.0 / (1.0 + hydrogen.clamp(0.0, 1.0)))
}

/// The hydrogen mass fraction of a mixed burst's fuel, 0.7: a hydrogen-rich donor's, as the
/// recycling's Eddington rate takes it (Tauris, Langer and Kramer 2012, equation 6).
const MIXED_FUEL_HYDROGEN: f64 = 0.7;

/// The lightest white dwarf a curve here takes, M☉: 0.1 (ours), below the helium dwarfs, the
/// lightest made.
const MIN_WHITE_DWARF_MASS: f64 = 0.1;

/// The heaviest neutron star a curve here takes, M☉: 3.2, the most that causality allows any
/// equation of state (Rhoades and Ruffini 1974, PRL 32, 324). Above it a neutron star's surface
/// would radiate more than the accreted matter's rest energy.
const MAX_NEUTRON_STAR_MASS: f64 = 3.2;

/// The heaviest black hole or merger a curve here takes, M☉: two of the tracks' heaviest stars.
const MAX_MERGED_MASS: f64 = 2.0 * crate::stellar::sse::MAX_INITIAL_MASS.value();

/// The most mass a disc gathers between outbursts, or a kilonova ejects, that a curve here takes,
/// M☉: 1 (ours). Mergers of neutron stars eject 10⁻⁴–0.1 M☉ (Metzger 2020, Living Rev. Relativ.
/// 23, 1), and a disc gathers far less.
const MAX_GATHERED_MASS: f64 = 1.0;

/// A light curve's `of`, such as [`NovaCurve::of`], was given a parameter no event has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildLightCurveError {
    /// A mass is not finite and positive, or is outside what its kind can have.
    MassOutOfRange,
    /// The orbital period is zero, negative or not finite.
    PeriodNotPositive,
    /// The hydrogen mass fraction is outside 0–1.
    HydrogenOutOfRange,
}

impl fmt::Display for BuildLightCurveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MassOutOfRange => f.write_str("a light curve's mass is outside its kind's range"),
            Self::PeriodNotPositive => {
                f.write_str("a light curve's orbital period is not positive and finite")
            }
            Self::HydrogenOutOfRange => {
                f.write_str("a light curve's hydrogen mass fraction is outside 0 to 1")
            }
        }
    }
}

impl Error for BuildLightCurveError {}

/// `mass` in M☉ if it lies in (`least`, `most`], `least` exclusive only when it is zero.
fn mass_within(mass: SolarMasses, least: f64, most: f64) -> Result<f64, BuildLightCurveError> {
    let m = mass.value();
    if m > 0.0 && m >= least && m <= most {
        Ok(m)
    } else {
        Err(BuildLightCurveError::MassOutOfRange)
    }
}

/// A white dwarf's `mass` in M☉ if it is at least [`MIN_WHITE_DWARF_MASS`] and under the
/// Chandrasekhar mass, 1.44 M☉ as plan 06's remnants take it, at which a dwarf has no radius left.
fn white_dwarf(mass: SolarMasses) -> Result<f64, BuildLightCurveError> {
    let m = mass_within(mass, MIN_WHITE_DWARF_MASS, CHANDRASEKHAR_MASS.value())?;
    if m < CHANDRASEKHAR_MASS.value() {
        Ok(m)
    } else {
        Err(BuildLightCurveError::MassOutOfRange)
    }
}

/// `hydrogen` if it is a mass fraction, 0–1.
fn hydrogen_fraction(hydrogen: f64) -> Result<f64, BuildLightCurveError> {
    if (0.0..=1.0).contains(&hydrogen) {
        Ok(hydrogen)
    } else {
        Err(BuildLightCurveError::HydrogenOutOfRange)
    }
}

/// How a curve fades after its peak and plateau.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Fade {
    /// `L_peak e^(−t′ ÷ τ)`, t′ the time since the plateau's end.
    Exponential {
        /// τ, the e-folding time, s.
        e_fold_s: f64,
    },
    /// `L_peak (t ÷ t_end)^(−n)`, t the time since the onset and `t_end` the plateau's end.
    PowerLaw {
        /// n, above 1.
        index: f64,
    },
}

/// The shape every kind shares: zero before the onset, a linear rise to the peak, a plateau at it,
/// and a fade.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Shape {
    /// The rise, s, positive.
    rise_s: f64,
    /// The peak luminosity, W.
    peak_w: f64,
    /// The plateau, s, zero for a kind without one.
    plateau_s: f64,
    /// The fade.
    fade: Fade,
}

impl Shape {
    /// The end of the plateau, s after the onset.
    #[must_use]
    fn top_s(&self) -> f64 {
        self.rise_s + self.plateau_s
    }

    /// The luminosity `t` seconds after the onset, W: zero before it and for a NaN.
    #[must_use]
    fn luminosity(&self, t: f64) -> f64 {
        if t.is_nan() || t < 0.0 {
            return 0.0;
        }
        if t < self.rise_s {
            return self.peak_w * t / self.rise_s;
        }
        let top = self.top_s();
        if t <= top {
            return self.peak_w;
        }
        match self.fade {
            Fade::Exponential { e_fold_s } => self.peak_w * math::exp(-(t - top) / e_fold_s),
            Fade::PowerLaw { index } => self.peak_w * math::powf(t / top, -index),
        }
    }

    /// The light radiated from the onset on, J: the rise's half, the plateau and the fade's
    /// integral, τ for an exponential and `t_end ÷ (n − 1)` for a power law.
    #[must_use]
    fn radiated_joules(&self) -> f64 {
        let fade_s = match self.fade {
            Fade::Exponential { e_fold_s } => e_fold_s,
            Fade::PowerLaw { index } => self.top_s() / (index - 1.0),
        };
        self.peak_w * (self.rise_s / 2.0 + self.plateau_s + fade_s)
    }

    /// The time after the onset, s, from which the luminosity stays under `share` of the peak: the
    /// plateau's end for a share of 1 or more, and infinite for none.
    #[must_use]
    fn fades_below(&self, share: f64) -> f64 {
        if share.is_nan() || share <= 0.0 {
            return f64::INFINITY;
        }
        let top = self.top_s();
        if share >= 1.0 {
            return top;
        }
        match self.fade {
            Fade::Exponential { e_fold_s } => top - e_fold_s * math::ln(share),
            Fade::PowerLaw { index } => top * math::powf(share, -1.0 / index),
        }
    }
}

/// Livio's (1992, ApJ 393, 516) Chandrasekhar mass, the mass at which his t₃ vanishes: 1.4 M☉
/// (recalled; Hurley, Tout and Pols take 1.44).
const LIVIO_CHANDRASEKHAR_MASS: SolarMasses = SolarMasses::new(1.4);

/// The fastest decline a nova here has, t₃ in days: 2.6, U Scorpii's, the fastest of the Galactic
/// recurrent novae (Schaefer 2010, ApJS 187, 275).
///
/// Livio's t₃ vanishes at the Chandrasekhar mass: it is 12 hours at 1.35 M☉ and 3 hours at 1.38,
/// faster than any nova seen, and the floor holds from 1.26 M☉.
const FASTEST_T3_DAYS: f64 = 2.6;

/// A classical or recurrent nova's light curve, from its white dwarf's mass (Livio 1992, ApJ 393,
/// 516).
///
/// - **Peak:** `M_B` = −8.3 − 10.0 log₁₀(M ÷ M☉) (Livio's maximum magnitude), through the B share
///   of a blackbody at [`NOVA_KELVIN`] (module documentation): 9.0 × 10³¹ W, 2.3 × 10⁵ L☉, at
///   1 M☉.
/// - **Decline:** three magnitudes in t₃ = 51.3 d (M ÷ M☉)^(−⅓) [(M ÷ `M_Ch`)^(−⅔) −
///   (M ÷ `M_Ch`)^⅔]^(3⁄2), `M_Ch` = 1.4 M☉ (Livio's decline time, the envelope's mass over its
///   wind), at least U Scorpii's 2.6 days, so exponential in luminosity with an e-folding time
///   of t₃ ÷ (1.2 ln 10). A heavier dwarf peaks brighter and declines faster.
/// - **Rise:** a tenth of t₃, at least a day (ours): the final rise takes a day or two in fast
///   novae and weeks in slow ones (Warner 2008, in Bode and Evans, Classical Novae, chapter 2).
///
/// A recurrent nova is a nova on a heavy white dwarf, so the same curve covers both. The light is
/// the photosphere's at maximum, and the bolometric and radiated light are its: the
/// constant-luminosity super-soft phase that follows, in which the light moves into the
/// ultraviolet and soft X-rays, is not modelled.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NovaCurve {
    white_dwarf_mass: SolarMasses,
    t3_days: f64,
    shape: Shape,
}

impl NovaCurve {
    /// The nova of a white dwarf of `white_dwarf_mass`.
    ///
    /// # Errors
    ///
    /// [`BuildLightCurveError::MassOutOfRange`] for a mass under 0.1 M☉, or at or over 1.44 M☉, the
    /// Chandrasekhar mass as plan 06's remnants take it.
    pub fn of(white_dwarf_mass: SolarMasses) -> Result<Self, BuildLightCurveError> {
        let m = white_dwarf(white_dwarf_mass)?;
        let x = m / LIVIO_CHANDRASEKHAR_MASS.value();
        let bracket = math::powf(x, -2.0 / 3.0) - math::powf(x, 2.0 / 3.0);
        let livio = if bracket > 0.0 {
            51.3 / math::cbrt(m) * math::powf(bracket, 1.5)
        } else {
            0.0
        };
        let t3_days = livio.max(FASTEST_T3_DAYS);
        let magnitude = -8.3 - 10.0 * math::log10(m);
        let e_fold_s = t3_days * SECONDS_PER_DAY / (1.2 * core::f64::consts::LN_10);
        Ok(Self {
            white_dwarf_mass,
            t3_days,
            shape: Shape {
                rise_s: (t3_days / 10.0).max(1.0) * SECONDS_PER_DAY,
                peak_w: luminosity_from_magnitude(MagnitudeBand::B, magnitude, Colour::Nova),
                plateau_s: 0.0,
                fade: Fade::Exponential { e_fold_s },
            },
        })
    }

    /// The white dwarf's mass.
    #[must_use]
    pub const fn white_dwarf_mass(&self) -> SolarMasses {
        self.white_dwarf_mass
    }

    /// t₃, the time to fade by three magnitudes from the peak.
    #[must_use]
    pub const fn t3(&self) -> Days {
        Days::new(self.t3_days)
    }
}

/// A dwarf nova's rise, s: a day (ours).
///
/// The outbursts that start at the disc's rim rise in about a day, those that start inside it in
/// one to two (Warner 1995, Cataclysmic Variable Stars, chapter 3).
const DWARF_NOVA_RISE_S: f64 = SECONDS_PER_DAY;

/// The orbital periods, hours, over which Warner's (1987) maximum and the Bailey relation are
/// read: 1.3–10, about the range of the dwarf novae they were fitted to (recalled). A period
/// outside is held to it, so that a long-period system's outburst is not a nova's.
const DWARF_NOVA_PERIOD_HOURS: (f64, f64) = (1.3, 10.0);

/// A dwarf nova's outburst, from its white dwarf's mass, its orbital period and the mass its disc
/// gathered since the last outburst.
///
/// - **Peak:** `M_V` = 5.74 − 0.259 `P_orb`, the period in hours, 1.3–10 (Warner 1987, MNRAS 227,
///   23), through the V share of a blackbody at [`DISC_KELVIN`] (module documentation).
/// - **Decay:** `τ_d` = 0.38 `P_orb`^0.9 days per magnitude (the Bailey relation as Warner 1995,
///   Cataclysmic Variable Stars, chapter 3, gives it), exponential in luminosity with an e-folding
///   time of 2.5 `τ_d` ÷ ln 10.
/// - **Plateau:** as long as the gathered mass lasts. A limit cycle accretes per outburst what the
///   donor gave in a cycle, and a disc radiates half of its fall, G M ÷ 2R per unit mass, the other
///   half going to the boundary layer's extreme ultraviolet (Frank, King and Raine 2002, section
///   5.3), with R the white dwarf's radius (Tout et al. 1997, as plan 06's remnants take it). The
///   plateau holds the peak until the light radiated is that energy. Where the rise and decay
///   alone radiate more, there is no plateau and Warner's peak stands, so the outburst radiates
///   more than the gathered fall (a finding of P11.T9.a).
/// - **Rise:** a day (ours; Warner 1995, chapter 3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DwarfNovaCurve {
    orbital_period: Days,
    shape: Shape,
}

impl DwarfNovaCurve {
    /// The outburst of a dwarf nova whose white dwarf has `white_dwarf_mass`, on an orbit of
    /// `orbital_period`, whose disc gathered `accreted_mass`: the transfer rate times the cycle.
    ///
    /// # Errors
    ///
    /// - [`BuildLightCurveError::MassOutOfRange`] for a white dwarf under 0.1 M☉ or at or over the
    ///   Chandrasekhar mass, 1.44 M☉, or an accreted mass that is not positive or is over 1 M☉.
    /// - [`BuildLightCurveError::PeriodNotPositive`] for a period that is not positive and finite.
    pub fn of(
        white_dwarf_mass: SolarMasses,
        orbital_period: Days,
        accreted_mass: SolarMasses,
    ) -> Result<Self, BuildLightCurveError> {
        let m = white_dwarf(white_dwarf_mass)?;
        let accreted = mass_within(accreted_mass, 0.0, MAX_GATHERED_MASS)?;
        let days = orbital_period.value();
        if !(days.is_finite() && days > 0.0) {
            return Err(BuildLightCurveError::PeriodNotPositive);
        }
        let (shortest, longest) = DWARF_NOVA_PERIOD_HOURS;
        let hours = (days * 24.0).clamp(shortest, longest);
        let peak_w =
            luminosity_from_magnitude(MagnitudeBand::V, 5.74 - 0.259 * hours, Colour::Disc);
        let days_per_magnitude = 0.38 * math::powf(hours, 0.9);
        let e_fold_s = days_per_magnitude * SECONDS_PER_DAY * 2.5 / core::f64::consts::LN_10;
        let radius_m =
            white_dwarf_radius(RemnantRecipe::default(), white_dwarf_mass).value() * SOLAR_RADIUS_M;
        let joules = GM_SUN * m * accreted * SOLAR_MASS_KG / (2.0 * radius_m);
        let plateau_s = (joules / peak_w - DWARF_NOVA_RISE_S / 2.0 - e_fold_s).max(0.0);
        Ok(Self {
            orbital_period,
            shape: Shape {
                rise_s: DWARF_NOVA_RISE_S,
                peak_w,
                plateau_s,
                fade: Fade::Exponential { e_fold_s },
            },
        })
    }

    /// The orbital period.
    #[must_use]
    pub const fn orbital_period(&self) -> Days {
        self.orbital_period
    }

    /// The plateau, from the peak to the start of the decay.
    #[must_use]
    pub fn plateau(&self) -> Days {
        Days::new(self.shape.plateau_s / SECONDS_PER_DAY)
    }
}

/// What an X-ray transient accretes onto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum XrayAccretor {
    /// A neutron star, whose surface radiates the whole fall.
    ///
    /// Its efficiency is G M ÷ R c², with plan 06's 12.2 km radius: 0.17 at 1.4 M☉.
    NeutronStar,
    /// A black hole, whose disc radiates the binding at its innermost stable orbit.
    ///
    /// Its efficiency is 1 − √(8 ÷ 9), 5.7%, for no spin (Shapiro and Teukolsky 1983, Black Holes,
    /// White Dwarfs and Neutron Stars, chapter 12); spin is not modelled.
    BlackHole,
}

impl XrayAccretor {
    /// The heaviest accretor of the kind, M☉.
    #[must_use]
    const fn max_mass(self) -> f64 {
        match self {
            Self::NeutronStar => MAX_NEUTRON_STAR_MASS,
            Self::BlackHole => MAX_MERGED_MASS,
        }
    }

    /// The share of the accreted mass's rest energy that its fall onto an accretor of `mass` M☉
    /// radiates.
    #[must_use]
    pub(crate) fn efficiency(self, mass: f64) -> f64 {
        match self {
            Self::NeutronStar => {
                let radius_m =
                    neutron_star_radius(RemnantRecipe::default()).value() * SOLAR_RADIUS_M;
                GM_SUN * mass / (radius_m * SPEED_OF_LIGHT * SPEED_OF_LIGHT)
            }
            Self::BlackHole => 1.0 - (8.0_f64 / 9.0).sqrt(),
        }
    }
}

/// An X-ray transient's rise, s: five days (ours).
///
/// Soft X-ray transients rise to their peak in a few days to a week (Chen, Shrader and Livio 1997,
/// ApJ 491, 312).
const TRANSIENT_RISE_S: f64 = 5.0 * SECONDS_PER_DAY;

/// An X-ray transient's e-folding decay, s: 30 days.
///
/// It is the mean of the fast-rise, exponential-decay outbursts (Chen, Shrader and Livio 1997, ApJ
/// 491, 312), the irradiated disc's viscous time (King and Ritter 1998, MNRAS 293, L42).
const TRANSIENT_E_FOLD_S: f64 = 30.0 * SECONDS_PER_DAY;

/// An X-ray transient's outburst, a fast rise and exponential decay, from its accretor and the
/// mass the disc gathered in quiescence.
///
/// The outburst radiates the gathered mass's fall at the accretor's efficiency, ε M c²
/// ([`XrayAccretor`]), over a five-day rise and a 30-day e-folding decay (Chen, Shrader and Livio
/// 1997). Its peak is held to the accretor's Eddington luminosity for the accreted matter's
/// hydrogen ([`eddington_luminosity`]): above it the matter is not taken, so the X-ray luminosity
/// reads the accreted rate, never the donor's above it, with X from the transferred matter
/// (ruling 132.1). Its X-rays are a blackbody's at [`TRANSIENT_KEV`], and its optical light the
/// [`REPROCESSED_SHARE`] its disc reprocesses.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XrayTransientCurve {
    accretor: XrayAccretor,
    mass: SolarMasses,
    shape: Shape,
}

impl XrayTransientCurve {
    /// The outburst of an X-ray transient whose `accretor` has `mass`, and whose disc gathered
    /// `accreted_mass` of matter with the hydrogen mass fraction `hydrogen`: the accreted rate
    /// times the cycle, and the donor's surface.
    ///
    /// # Errors
    ///
    /// - [`BuildLightCurveError::MassOutOfRange`] for an accretor that is not positive or is
    ///   heavier than its kind can be (3.2 M☉ for a neutron star, 300 M☉ for a black hole), or an
    ///   accreted mass that is not positive or is over 1 M☉.
    /// - [`BuildLightCurveError::HydrogenOutOfRange`] for a fraction outside 0–1.
    pub fn of(
        accretor: XrayAccretor,
        mass: SolarMasses,
        accreted_mass: SolarMasses,
        hydrogen: f64,
    ) -> Result<Self, BuildLightCurveError> {
        let m = mass_within(mass, 0.0, accretor.max_mass())?;
        let accreted = mass_within(accreted_mass, 0.0, MAX_GATHERED_MASS)?;
        let hydrogen = hydrogen_fraction(hydrogen)?;
        let joules =
            accretor.efficiency(m) * accreted * SOLAR_MASS_KG * SPEED_OF_LIGHT * SPEED_OF_LIGHT;
        let unheld = joules / (TRANSIENT_RISE_S / 2.0 + TRANSIENT_E_FOLD_S);
        Ok(Self {
            accretor,
            mass,
            shape: Shape {
                rise_s: TRANSIENT_RISE_S,
                peak_w: unheld.min(eddington_luminosity(mass, hydrogen).value()),
                plateau_s: 0.0,
                fade: Fade::Exponential {
                    e_fold_s: TRANSIENT_E_FOLD_S,
                },
            },
        })
    }

    /// What the transient accretes onto.
    #[must_use]
    pub const fn accretor(&self) -> XrayAccretor {
        self.accretor
    }

    /// The accretor's mass.
    #[must_use]
    pub const fn mass(&self) -> SolarMasses {
        self.mass
    }
}

/// What a Type I X-ray burst burns, which P11.T9.b reads from the accreted rate and the donor
/// (Strohmayer and Bildsten 2006, in Compact Stellar X-ray Sources, chapter 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BurstFuel {
    /// Pure helium: the hydrogen burned stably between bursts, at a few per cent of the Eddington
    /// rate, or the donor had none.
    ///
    /// A short burst at the helium Eddington luminosity, with photospheric radius expansion.
    Helium,
    /// Mixed hydrogen and helium, at higher rates.
    ///
    /// A longer, fainter burst whose tail the rp process feeds.
    HydrogenHelium,
}

/// The time profile of a Type I burst of one fuel.
#[derive(Debug, Clone, Copy, PartialEq)]
struct BurstProfile {
    /// The rise, s.
    rise_s: f64,
    /// The peak, as a share of the Eddington luminosity for the burning matter.
    eddington_share: f64,
    /// The burning matter's hydrogen mass fraction.
    hydrogen: f64,
    /// The e-folding decay, s.
    e_fold_s: f64,
}

/// The profile of a Type I burst of `fuel` (ours, from the ranges of Galloway et al. 2008, ApJS
/// 179, 360, and Strohmayer and Bildsten 2006).
///
/// - **Helium:** a 1 s rise to the helium Eddington luminosity and a 5 s decay, the short bursts
///   that reach the limit (Kuulkers et al. 2003, A&A 399, 663).
/// - **Hydrogen and helium:** a 5 s rise to half the hydrogen-rich Eddington luminosity and a 30 s
///   decay, as GS 1826−24's bursts (Galloway et al. 2004, ApJ 601, 466).
#[must_use]
const fn burst_profile(fuel: BurstFuel) -> BurstProfile {
    match fuel {
        BurstFuel::Helium => BurstProfile {
            rise_s: 1.0,
            eddington_share: 1.0,
            hydrogen: 0.0,
            e_fold_s: 5.0,
        },
        BurstFuel::HydrogenHelium => BurstProfile {
            rise_s: 5.0,
            eddington_share: 0.5,
            hydrogen: MIXED_FUEL_HYDROGEN,
            e_fold_s: 30.0,
        },
    }
}

/// A Type I X-ray burst on a neutron star, from the star's mass and the fuel.
///
/// A helium burst rises in a second to the helium Eddington luminosity and decays in 5 s; a mixed
/// burst rises in 5 s to half the hydrogen-rich one and decays in 30 s (ours, from the ranges of
/// Galloway et al. 2008, ApJS 179, 360; Kuulkers et al. 2003; GS 1826−24's bursts, Galloway et al.
/// 2004, ApJ 601, 466). So a burst is never above the star's Eddington luminosity for the burning
/// matter ([`eddington_luminosity`]). Its X-rays are a blackbody's at [`BURST_KEV`], and its
/// optical light the [`REPROCESSED_SHARE`] the disc reprocesses.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XrayBurstCurve {
    mass: SolarMasses,
    fuel: BurstFuel,
    shape: Shape,
}

impl XrayBurstCurve {
    /// A burst of `fuel` on a neutron star of `mass`.
    ///
    /// # Errors
    ///
    /// [`BuildLightCurveError::MassOutOfRange`] for a mass that is not positive or is over
    /// 3.2 M☉.
    pub fn of(mass: SolarMasses, fuel: BurstFuel) -> Result<Self, BuildLightCurveError> {
        mass_within(mass, 0.0, MAX_NEUTRON_STAR_MASS)?;
        let profile = burst_profile(fuel);
        let eddington = eddington_luminosity(mass, profile.hydrogen).value();
        Ok(Self {
            mass,
            fuel,
            shape: Shape {
                rise_s: profile.rise_s,
                peak_w: profile.eddington_share * eddington,
                plateau_s: 0.0,
                fade: Fade::Exponential {
                    e_fold_s: profile.e_fold_s,
                },
            },
        })
    }

    /// The neutron star's mass.
    #[must_use]
    pub const fn mass(&self) -> SolarMasses {
        self.mass
    }

    /// What the burst burns.
    #[must_use]
    pub const fn fuel(&self) -> BurstFuel {
        self.fuel
    }
}

/// A Be/X giant outburst's peak as a share of its neutron star's Eddington luminosity: 0.2 (ours).
///
/// That is 4 × 10³⁷ erg s⁻¹ at 1.4 M☉ for hydrogen-rich matter, inside the 10³⁷–10³⁸ erg s⁻¹ of
/// Type II outbursts (Reig 2011, Ap&SS 332, 1).
const BEX_PEAK_EDDINGTON_SHARE: f64 = 0.2;

/// A Be/X giant outburst's rise, s: 10 days (ours).
const BEX_RISE_S: f64 = 10.0 * SECONDS_PER_DAY;

/// A Be/X giant outburst's e-folding decay, s: 20 days (ours).
///
/// With the rise, the outburst is over a tenth of its peak for some 55 days: Type II outbursts
/// last weeks to months, at any orbital phase (Reig 2011, Ap&SS 332, 1).
const BEX_E_FOLD_S: f64 = 20.0 * SECONDS_PER_DAY;

/// A Be/X giant (Type II) outburst, from its neutron star's mass and the accreted hydrogen.
///
/// It peaks at a fifth of the Eddington luminosity for the accreted matter's hydrogen (ours,
/// inside Reig 2011's 10³⁷–10³⁸ erg s⁻¹), after a 10-day rise, and decays with a 20-day e-folding
/// time, so that it lasts weeks to months (ours). Its X-rays are a blackbody's at [`BEX_KEV`]; its
/// optical light is the Be star's (module documentation), none of its own. The Type I outbursts at
/// periastron are part of the system's state, not events.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeXOutburstCurve {
    mass: SolarMasses,
    shape: Shape,
}

impl BeXOutburstCurve {
    /// The giant outburst of a Be/X binary whose neutron star has `mass` and accretes matter with
    /// the hydrogen mass fraction `hydrogen`, the Be star's.
    ///
    /// # Errors
    ///
    /// - [`BuildLightCurveError::MassOutOfRange`] for a mass that is not positive or is over
    ///   3.2 M☉.
    /// - [`BuildLightCurveError::HydrogenOutOfRange`] for a fraction outside 0–1.
    pub fn of(mass: SolarMasses, hydrogen: f64) -> Result<Self, BuildLightCurveError> {
        mass_within(mass, 0.0, MAX_NEUTRON_STAR_MASS)?;
        let hydrogen = hydrogen_fraction(hydrogen)?;
        Ok(Self {
            mass,
            shape: Shape {
                rise_s: BEX_RISE_S,
                peak_w: BEX_PEAK_EDDINGTON_SHARE * eddington_luminosity(mass, hydrogen).value(),
                plateau_s: 0.0,
                fade: Fade::Exponential {
                    e_fold_s: BEX_E_FOLD_S,
                },
            },
        })
    }

    /// The neutron star's mass.
    #[must_use]
    pub const fn mass(&self) -> SolarMasses {
        self.mass
    }
}

/// A luminous red nova's plateau luminosity at [`RED_NOVA_ANCHOR_MASS`], L☉: 3 × 10⁴, V1309
/// Scorpii's (Tylenda et al. 2011, A&A 528, A114).
const RED_NOVA_ANCHOR_SOLAR_LUMINOSITIES: f64 = 3.0e4;

/// The mass of V1309 Scorpii's merger: a pair of about 1.5 M☉ (Tylenda et al. 2011, A&A 528,
/// A114; Stępień 2011 has 1.52 + 0.16 M☉).
const RED_NOVA_ANCHOR_MASS: SolarMasses = SolarMasses::new(1.5);

/// The index of a luminous red nova's luminosity in its merger's mass up to
/// [`RED_NOVA_BREAK_MASS`]: 2.5.
///
/// It is the middle of the M²–M³ that Kochanek, Adams and Belczynski (2014, MNRAS 443, 1319) find
/// from V1309 Sco to V838 Mon, which it meets within a factor of two (2 × 10⁶ L☉ at 8 M☉, against
/// Tylenda 2005's 10⁶ L☉). Against the eight red novae of `MacLeod`, De and Loeb's (2022, ApJ 937,
/// 96; the citation recalled) compilation, each one's luminosity over the time in which it
/// radiates 90% of its light against its progenitor's mass (their reproduction materials'
/// `lrn.txt`, read at source), from V4332 Sgr at 1 M☉ to NGC 4490-OT2011 and `SNhunt248` at 30 M☉,
/// a power law fits M^2.43 with a scatter of 0.24 dex; this law runs 0.29 dex above them on
/// average, at most a factor of 4.1 (M101 OT2015-1), and within 10% of V1309 Sco, M31-LRN-2015 and
/// NGC 4490-OT2011. The progenitor's mass stands for the merger's, its companion unknown (lean):
/// with the companion of a third that their own models take, the law sits 0.60 dex above them.
const RED_NOVA_INDEX: f64 = 2.5;

/// The heaviest merger to which [`RED_NOVA_INDEX`] holds, M☉: 30, the heaviest progenitors of the
/// red novae seen, NGC 4490-OT2011's and `SNhunt248`'s (`MacLeod`, De and Loeb 2022's compilation, as
/// [`RED_NOVA_INDEX`]). The class's mergers reach 300 M☉, where the steep law would be some 1,500
/// times the merger's Eddington luminosity ([`eddington_luminosity`] at a hydrogen share of 0.7).
const RED_NOVA_BREAK_MASS: f64 = 30.0;

/// The index of a luminous red nova's luminosity in its merger's mass above
/// [`RED_NOVA_BREAK_MASS`]: ⅔ (lean, 2026-11-01), past the heaviest red nova seen, so an
/// extrapolation.
///
/// Matsumoto and Metzger's (2022, ApJ 938, 5; recalled) plateau grows as `M_ej`^⅔ `v_ej` where
/// recombination powers it, 4.8 × 10³⁸ erg s⁻¹ (`f_ad` ÷ 0.3) (`ρ_i` ÷ 10⁻¹¹ g cm⁻³)^⅓ (`M_ej` ÷
/// M☉)^⅔ (`v_ej` ÷ 300 km s⁻¹), and as `M_pre` `v_ej`³ `M_ej`^⅔ where the ejecta's shock does (as
/// Karambelkar et al.'s `binaryc_postprocess` implements both, read at source): M^⅔ to M^(5⁄3),
/// the ejecta a fixed share of the merger and their speed fixed. Recombination is far short of the
/// bright red novae, 1.2 × 10⁴⁰ erg s⁻¹ for all of 30 M☉ ejected at 800 km s⁻¹, an eighteenth of
/// NGC 4490-OT2011's, so they are shock-powered, and ⅔, the range's low end, is taken for the energy
/// budget. A 300 M☉ merger then peaks at 2.5 × 10⁸ L☉, `M_bol` −16.2, 1.7 times AT 2017jfs, the
/// brightest red nova seen (some 5.5 × 10⁴¹ erg s⁻¹; Pastorello et al. 2019, A&A 625, L8;
/// recalled), and radiates 1.8 × 10⁴⁹ erg, some 2% of the orbital energy of two 150 M☉ stars in
/// contact, where NGC 4490-OT2011 radiated 6% of its own; M^(5⁄3) would radiate ten times as much,
/// and the steep law 70 times, more than the orbit holds. The speed is held fixed, a
/// simplification: main-sequence radii grow as about M^0.6 there and the escape speed as M^0.2
/// (recalled), which would raise the index to 0.88 and the top 1.6 times. Ejecta far over their
/// Eddington luminosity are usual (the top is 21 times its, NGC 4490-OT2011 50 times its
/// progenitor's), so Eddington's bounds nothing here.
const RED_NOVA_HEAVY_INDEX: f64 = 2.0 / 3.0;

/// A luminous red nova's plateau at 1 M☉, days: 100, the bottom of the task's 100–200 days.
const RED_NOVA_PLATEAU_DAYS: f64 = 100.0;

/// The index of a luminous red nova's plateau in its merger's mass: ½ (ours).
///
/// A recombination plateau lasts t ∝ κ^⅙ m^½ R^⅙ E^(−⅙) T^(−⅔) (Popov 1993, ApJ 414, 712, as
/// Ivanova et al. 2013, Science 339, 433, normalise it), which is about M^0.47 where the ejecta's
/// mass and energy scale with the merger's and the radius as a main-sequence star's, M^0.8.
const RED_NOVA_PLATEAU_INDEX: f64 = 0.5;

/// The longest plateau of a luminous red nova, days: 200, the top of the task's 100–200 days,
/// reached at 4 M☉.
const RED_NOVA_MAX_PLATEAU_DAYS: f64 = 200.0;

/// A luminous red nova's rise, s: 10 days (ours). V1309 Sco rose to its peak in about ten days
/// (Tylenda et al. 2011).
const RED_NOVA_RISE_S: f64 = 10.0 * SECONDS_PER_DAY;

/// A luminous red nova's e-folding fade after its plateau, s: 15 days (ours).
const RED_NOVA_E_FOLD_S: f64 = 15.0 * SECONDS_PER_DAY;

/// A luminous red nova, the outburst of a stellar merger, from the merger's mass, the pair's
/// total.
///
/// It plateaus for 100 (M ÷ M☉)^½ days, held to 100–200 (ours, after Popov 1993), at V1309 Sco's
/// 3 × 10⁴ L☉ scaled as M^2.5 to 30 M☉, the heaviest progenitors seen, and as M^⅔ above (Tylenda
/// et al. 2011; Kochanek, Adams and Belczynski 2014; `MacLeod`, De and Loeb 2022; Matsumoto and
/// Metzger 2022: `RED_NOVA_INDEX`, `RED_NOVA_HEAVY_INDEX`), after a 10-day rise and before a
/// 15-day e-folding fade (ours), at [`RED_NOVA_KELVIN`]. Holding the peak through the plateau
/// radiates several times what V1309 Sco and V838 Mon did (a finding of P11.T9.a).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LuminousRedNovaCurve {
    merger_mass: SolarMasses,
    shape: Shape,
}

impl LuminousRedNovaCurve {
    /// The red nova of a merger of `merger_mass`.
    ///
    /// # Errors
    ///
    /// [`BuildLightCurveError::MassOutOfRange`] for a mass that is not positive or is over
    /// 300 M☉, two of the tracks' heaviest stars.
    pub fn of(merger_mass: SolarMasses) -> Result<Self, BuildLightCurveError> {
        let m = mass_within(merger_mass, 0.0, MAX_MERGED_MASS)?;
        let plateau_days = (RED_NOVA_PLATEAU_DAYS * math::powf(m, RED_NOVA_PLATEAU_INDEX))
            .clamp(RED_NOVA_PLATEAU_DAYS, RED_NOVA_MAX_PLATEAU_DAYS);
        let scale = if m <= RED_NOVA_BREAK_MASS {
            math::powf(m / RED_NOVA_ANCHOR_MASS.value(), RED_NOVA_INDEX)
        } else {
            math::powf(
                RED_NOVA_BREAK_MASS / RED_NOVA_ANCHOR_MASS.value(),
                RED_NOVA_INDEX,
            ) * math::powf(m / RED_NOVA_BREAK_MASS, RED_NOVA_HEAVY_INDEX)
        };
        Ok(Self {
            merger_mass,
            shape: Shape {
                rise_s: RED_NOVA_RISE_S,
                peak_w: RED_NOVA_ANCHOR_SOLAR_LUMINOSITIES * SOLAR_LUMINOSITY_W * scale,
                plateau_s: plateau_days * SECONDS_PER_DAY,
                fade: Fade::Exponential {
                    e_fold_s: RED_NOVA_E_FOLD_S,
                },
            },
        })
    }

    /// The merger's mass.
    #[must_use]
    pub const fn merger_mass(&self) -> SolarMasses {
        self.merger_mass
    }

    /// The plateau, from the peak to the fade.
    #[must_use]
    pub fn plateau(&self) -> Days {
        Days::new(self.shape.plateau_s / SECONDS_PER_DAY)
    }
}

/// The ejecta of GW170817's kilonova, AT 2017gfo: 0.05 M☉, for a caller with no ejecta of its own.
///
/// It is its blue and red components together, about 0.02 and 0.04 M☉ (Metzger 2020, Living Rev.
/// Relativ. 23, 1).
pub const GW170817_EJECTA_MASS: SolarMasses = SolarMasses::new(0.05);

/// The heating of r-process ejecta a day after the merger, erg g⁻¹ s⁻¹: 10¹⁰.
///
/// The heating falls as t^−1.3 ([`R_PROCESS_HEATING_INDEX`]; Metzger et al. 2010, MNRAS 406, 2650;
/// Korobkin et al. 2012, MNRAS 426, 1940, whose fit gives 9.8 × 10⁹ at a day; Metzger 2020).
const R_PROCESS_HEATING_ERG_PER_G_S: f64 = 1.0e10;

/// The index in time of the r-process ejecta's heating, 1.3 (as
/// [`R_PROCESS_HEATING_ERG_PER_G_S`]).
const R_PROCESS_HEATING_INDEX: f64 = 1.3;

/// The share of the radioactive heating the ejecta thermalise, 0.5, about its value a day after
/// the merger (Barnes et al. 2016, ApJ 829, 110).
///
/// It is held at that for all time, though it falls later, so the light after ten days is too
/// bright (a finding of P11.T9.a).
const THERMALISED_SHARE: f64 = 0.5;

/// A kilonova's peak for [`GW170817_EJECTA_MASS`], s after the merger: a day.
///
/// That is about when AT 2017gfo's bolometric light peaked (Cowperthwaite et al. 2017, ApJ 848,
/// L17). The photons' diffusion time grows as the ejecta's mass to the half at a given opacity and
/// speed (Metzger 2020).
const KILONOVA_PEAK_S: f64 = SECONDS_PER_DAY;

/// A kilonova, from its ejecta's mass.
///
/// It rises to its peak at the photons' diffusion time, a day for GW170817's ejecta and as the
/// mass's square root (Cowperthwaite et al. 2017; Metzger 2020), and then follows the ejecta's
/// thermalised radioactive heating, 0.5 × 10¹⁰ (t ÷ 1 d)^−1.3 erg g⁻¹ s⁻¹ (Metzger et al. 2010;
/// Korobkin et al. 2012; Barnes et al. 2016): the luminosity tracks the heating from the peak on,
/// Arnett's rule. At [`KILONOVA_KELVIN`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KilonovaCurve {
    ejecta_mass: SolarMasses,
    shape: Shape,
}

impl KilonovaCurve {
    /// The kilonova of a merger that ejected `ejecta_mass`.
    ///
    /// # Errors
    ///
    /// [`BuildLightCurveError::MassOutOfRange`] for a mass that is not positive or is over 1 M☉.
    pub fn of(ejecta_mass: SolarMasses) -> Result<Self, BuildLightCurveError> {
        let m = mass_within(ejecta_mass, 0.0, MAX_GATHERED_MASS)?;
        let peak_s = KILONOVA_PEAK_S * (m / GW170817_EJECTA_MASS.value()).sqrt();
        let erg_s = THERMALISED_SHARE
            * m
            * GRAMS_PER_SOLAR_MASS
            * R_PROCESS_HEATING_ERG_PER_G_S
            * math::powf(peak_s / SECONDS_PER_DAY, -R_PROCESS_HEATING_INDEX);
        Ok(Self {
            ejecta_mass,
            shape: Shape {
                rise_s: peak_s,
                peak_w: erg_s * WATTS_PER_ERG_S,
                plateau_s: 0.0,
                fade: Fade::PowerLaw {
                    index: R_PROCESS_HEATING_INDEX,
                },
            },
        })
    }

    /// The ejecta's mass.
    #[must_use]
    pub const fn ejecta_mass(&self) -> SolarMasses {
        self.ejecta_mass
    }
}

/// The light curve of one binary event, of any kind (P11.T9.a).
///
/// The [module documentation](self) gives the kinds and the bands.
///
/// # Examples
///
/// A nova on a heavy white dwarf peaks brighter and fades faster than one on a light dwarf, and
/// only the X-ray kinds have X-rays:
///
/// ```
/// use hyperion_sim::galaxy::gas::ccm::Band;
/// use hyperion_sim::stellar::binary::light_curves::{
///     BinaryLightCurve, BurstFuel, NovaCurve, XrayBurstCurve,
/// };
/// use hyperion_sim::time::Span;
/// use hyperion_sim::units::{SolarMasses, Watts};
///
/// let heavy = BinaryLightCurve::from(NovaCurve::of(SolarMasses::new(1.2))?);
/// let light = BinaryLightCurve::from(NovaCurve::of(SolarMasses::new(0.7))?);
/// assert!(heavy.peak_luminosity() > light.peak_luminosity());
/// let month = Span::from_seconds(30 * 86_400);
/// let faded = |curve: &BinaryLightCurve| curve.bolometric(month) / curve.peak_luminosity();
/// assert!(faded(&heavy) < faded(&light));
/// assert!(heavy.in_band(month, Band::V) < heavy.bolometric(month));
/// assert!(heavy.xray_luminosity(month) < Watts::new(f64::MIN_POSITIVE));
///
/// let burst = XrayBurstCurve::of(SolarMasses::new(1.4), BurstFuel::Helium)?;
/// let burst = BinaryLightCurve::from(burst);
/// assert!(burst.xray_luminosity(burst.peak()) > Watts::ZERO);
/// # Ok::<(), hyperion_sim::stellar::binary::light_curves::BuildLightCurveError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinaryLightCurve {
    /// A classical or recurrent nova.
    Nova(NovaCurve),
    /// A dwarf nova's outburst.
    DwarfNova(DwarfNovaCurve),
    /// An X-ray transient's outburst.
    XrayTransient(XrayTransientCurve),
    /// A Type I X-ray burst.
    XrayBurst(XrayBurstCurve),
    /// A Be/X giant (Type II) outburst.
    BeXOutburst(BeXOutburstCurve),
    /// A luminous red nova.
    LuminousRedNova(LuminousRedNovaCurve),
    /// A kilonova.
    Kilonova(KilonovaCurve),
}

impl BinaryLightCurve {
    /// The kind's shape.
    #[must_use]
    const fn shape(&self) -> &Shape {
        match self {
            Self::Nova(c) => &c.shape,
            Self::DwarfNova(c) => &c.shape,
            Self::XrayTransient(c) => &c.shape,
            Self::XrayBurst(c) => &c.shape,
            Self::BeXOutburst(c) => &c.shape,
            Self::LuminousRedNova(c) => &c.shape,
            Self::Kilonova(c) => &c.shape,
        }
    }

    /// The kind's optical light: its colour and the share of its light at that colour.
    #[must_use]
    const fn optical(&self) -> Option<(Colour, f64)> {
        match self {
            Self::Nova(_) => Some((Colour::Nova, 1.0)),
            Self::DwarfNova(_) => Some((Colour::Disc, 1.0)),
            Self::XrayTransient(_) | Self::XrayBurst(_) => Some((Colour::Disc, REPROCESSED_SHARE)),
            Self::BeXOutburst(_) => None,
            Self::LuminousRedNova(_) => Some((Colour::RedNova, 1.0)),
            Self::Kilonova(_) => Some((Colour::Kilonova, 1.0)),
        }
    }

    /// The kind's X-rays: their temperature and the share of its light they carry.
    #[must_use]
    const fn xrays(&self) -> Option<(XrayColour, f64)> {
        match self {
            Self::XrayTransient(_) => Some((XrayColour::Transient, 1.0 - REPROCESSED_SHARE)),
            Self::XrayBurst(_) => Some((XrayColour::Burst, 1.0 - REPROCESSED_SHARE)),
            Self::BeXOutburst(_) => Some((XrayColour::BeX, 1.0)),
            Self::Nova(_) | Self::DwarfNova(_) | Self::LuminousRedNova(_) | Self::Kilonova(_) => {
                None
            }
        }
    }

    /// The event's bolometric luminosity `dt` after its onset: zero before it.
    #[must_use]
    pub fn bolometric(&self, dt: Span) -> Watts {
        Watts::new(self.shape().luminosity(dt.as_seconds_f64()))
    }

    /// The event's light in `band` `dt` after its onset.
    ///
    /// It is the bolometric light times the kind's colour's share in the band (module
    /// documentation), so never more than the bolometric, and nothing in radio or for a Be/X
    /// outburst.
    #[must_use]
    pub fn in_band(&self, dt: Span, band: Band) -> Watts {
        self.optical().map_or(Watts::ZERO, |(colour, share)| {
            self.bolometric(dt) * (share * colour.share(band))
        })
    }

    /// The event's luminosity between 2 and 10 keV, plan 12's X-ray band, `dt` after its onset.
    ///
    /// For the X-ray kinds it is their light's share at their X-ray temperature, and for the others
    /// zero.
    #[must_use]
    pub fn xray_luminosity(&self, dt: Span) -> Watts {
        self.xrays().map_or(Watts::ZERO, |(colour, share)| {
            self.bolometric(dt) * (share * colour.share())
        })
    }

    /// The time from the onset to the peak, where the rise ends.
    ///
    /// It is the brightest instant, and the first of the plateau for a kind with one, floored to
    /// the nanosecond and held to the span's range.
    #[must_use]
    pub fn peak(&self) -> Span {
        Span::from_seconds_f64(self.shape().rise_s).unwrap_or(Span::from_seconds(i64::MAX))
    }

    /// The bolometric luminosity at the peak, the curve's greatest.
    #[must_use]
    pub fn peak_luminosity(&self) -> Watts {
        Watts::new(self.shape().peak_w)
    }

    /// The event's light in `band` at its peak, the greatest of [`in_band`](Self::in_band): the
    /// peak luminosity times the kind's colour's share in the band, as `in_band` gives it on a
    /// plateau, bit for bit.
    #[must_use]
    pub fn peak_in_band(&self, band: Band) -> Watts {
        self.optical().map_or(Watts::ZERO, |(colour, share)| {
            self.peak_luminosity() * (share * colour.share(band))
        })
    }

    /// The event's 2–10 keV luminosity at its peak, the greatest of
    /// [`xray_luminosity`](Self::xray_luminosity): zero for a kind with no X-rays.
    #[must_use]
    pub fn peak_xray_luminosity(&self) -> Watts {
        self.xrays().map_or(Watts::ZERO, |(colour, share)| {
            self.peak_luminosity() * (share * colour.share())
        })
    }

    /// The light the event radiates from its onset on, J: the bolometric luminosity's integral, a
    /// closed form.
    #[must_use]
    pub fn radiated_joules(&self) -> f64 {
        self.shape().radiated_joules()
    }

    /// The time from the onset after which the bolometric luminosity stays under `share` of the
    /// peak.
    ///
    /// It is the plateau's end for a share of 1 or more, and `None` for a share that is not
    /// positive, or a time beyond the span's range.
    #[must_use]
    pub fn fades_below(&self, share: f64) -> Option<Span> {
        Span::from_seconds_f64(self.shape().fades_below(share))
    }
}

impl From<NovaCurve> for BinaryLightCurve {
    fn from(curve: NovaCurve) -> Self {
        Self::Nova(curve)
    }
}

impl From<DwarfNovaCurve> for BinaryLightCurve {
    fn from(curve: DwarfNovaCurve) -> Self {
        Self::DwarfNova(curve)
    }
}

impl From<XrayTransientCurve> for BinaryLightCurve {
    fn from(curve: XrayTransientCurve) -> Self {
        Self::XrayTransient(curve)
    }
}

impl From<XrayBurstCurve> for BinaryLightCurve {
    fn from(curve: XrayBurstCurve) -> Self {
        Self::XrayBurst(curve)
    }
}

impl From<BeXOutburstCurve> for BinaryLightCurve {
    fn from(curve: BeXOutburstCurve) -> Self {
        Self::BeXOutburst(curve)
    }
}

impl From<LuminousRedNovaCurve> for BinaryLightCurve {
    fn from(curve: LuminousRedNovaCurve) -> Self {
        Self::LuminousRedNova(curve)
    }
}

impl From<KilonovaCurve> for BinaryLightCurve {
    fn from(curve: KilonovaCurve) -> Self {
        Self::Kilonova(curve)
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::substance::thermo::SECOND_RADIATION_CONSTANT_CM_K;

    /// keV per kelvin: Boltzmann's constant over the electronvolt (CODATA 2018, both exact).
    const KEV_PER_KELVIN: f64 = 1.380_649e-23 / 1.602_176_634e-16;

    /// keV µm: h c (CODATA 2018, exact), so that a photon of E keV has a wavelength of this ÷ E µm.
    const KEV_MICRONS: f64 = 6.626_070_15e-34 * SPEED_OF_LIGHT / 1.602_176_634e-16 * 1e6;

    /// The hydrogen mass fraction of a hydrogen-rich donor's matter, for the accreting kinds.
    const HYDROGEN_RICH: f64 = 0.7;

    /// Joules per erg.
    const ERG: f64 = 1e-7;

    /// A band's top hat, µm: plan 07's wavelength ± half of Bessell's (2005, Table 1) or
    /// Tokunaga's (2000, Table 7.5) full width at half maximum, or the N band's edges for the mid
    /// infrared; none for radio. Plan 12's `alerts::local` takes the same.
    fn top_hat(band: Band) -> Option<(f64, f64)> {
        let width = match band {
            Band::U => 0.065,
            Band::B => 0.089,
            Band::V => 0.084,
            Band::R => 0.158,
            Band::I => 0.154,
            Band::J => 0.26,
            Band::H => 0.29,
            Band::K => 0.41,
            Band::MidInfrared => return Some((7.88, 13.07)),
            Band::Radio => return None,
        };
        let centre = band.wavelength()?.value();
        Some((centre - width / 2.0, centre + width / 2.0))
    }

    /// The share of a blackbody's light between x = `low` and x = `high`, x = hν ÷ kT:
    /// (15 ÷ π⁴) ∫ x³ ÷ (eˣ − 1) dx, by Simpson's rule on `intervals` (even) intervals.
    fn share_between(low: f64, high: f64, intervals: u32) -> f64 {
        let step = (high - low) / f64::from(intervals);
        let planck = |x: f64| x * x * x / math::exp_m1(x);
        let inner = (1..intervals).fold(0.0, |sum, i| {
            let weight = if i % 2 == 1 { 4.0 } else { 2.0 };
            sum + weight * planck(low + f64::from(i) * step)
        });
        15.0 / (PI * PI * PI * PI) * (planck(low) + inner + planck(high)) * step / 3.0
    }

    /// A blackbody's share at `kelvin` between `shortest` and `longest` µm.
    fn band_share(kelvin: f64, (shortest, longest): (f64, f64), intervals: u32) -> f64 {
        let c2 = SECOND_RADIATION_CONSTANT_CM_K * 1e4;
        share_between(c2 / (longest * kelvin), c2 / (shortest * kelvin), intervals)
    }

    /// A blackbody's share at kT = `kev` between 2 and 10 keV.
    fn xray_share(kev: f64, intervals: u32) -> f64 {
        share_between(2.0 / kev, 10.0 / kev, intervals)
    }

    /// The stored shares are the blackbodies' through the top hats to the bit, and the rule is
    /// converged to 10⁻¹² of each: every colour's ten bands, none in radio and under the whole
    /// light, and the X-ray temperatures' 2–10 keV. The physical constants agree with the module's
    /// own: 1 keV is 11.6 million kelvin.
    #[test]
    fn the_band_shares_are_blackbodies_through_the_top_hats() {
        for colour in Colour::ALL {
            let row = |intervals| {
                Band::ALL.map(|band| {
                    top_hat(band).map_or(0.0, |edges| band_share(colour.kelvin(), edges, intervals))
                })
            };
            let (rule, finer) = (row(20_000), row(40_000));
            for (i, band) in Band::ALL.into_iter().enumerate() {
                assert_same_bits(colour.share(band), rule[i]);
                assert!(
                    (finer[i] - rule[i]).abs() <= 1e-12 * rule[i],
                    "{colour:?} in {band:?}: {:e} at twice the intervals",
                    finer[i]
                );
            }
            assert!(rule.iter().sum::<f64>() < 1.0, "{colour:?}");
            assert_same_bits(colour.share(Band::Radio), 0.0);
        }
        for colour in XrayColour::ALL {
            let rule = xray_share(colour.kev(), 20_000);
            let finer = xray_share(colour.kev(), 40_000);
            assert_same_bits(colour.share(), rule);
            assert!((finer - rule).abs() <= 1e-12 * rule, "{colour:?}");
            assert!(rule < 1.0, "{colour:?}");
        }
        assert!((1.0 / KEV_PER_KELVIN - 1.160_451_8e7).abs() < 10.0);
        let c2 = KEV_MICRONS / KEV_PER_KELVIN;
        assert!(
            (c2 - SECOND_RADIATION_CONSTANT_CM_K * 1e4).abs() < 1e-9 * c2,
            "{c2}"
        );
    }

    fn solar(m: f64) -> SolarMasses {
        SolarMasses::new(m)
    }

    fn transient(accretor: XrayAccretor, m: f64, accreted: f64) -> XrayTransientCurve {
        XrayTransientCurve::of(accretor, solar(m), solar(accreted), HYDROGEN_RICH).unwrap()
    }

    /// One curve of each kind at the middle of its parameters, and others at their ends, named.
    fn curves() -> Vec<(String, BinaryLightCurve)> {
        let mut out: Vec<(String, BinaryLightCurve)> = Vec::new();
        for m in [0.1, 0.6, 0.8, 1.0, 1.2, 1.35, 1.439] {
            out.push((format!("nova {m}"), NovaCurve::of(solar(m)).unwrap().into()));
        }
        for (m, hours, accreted) in [
            (0.6, 1.5, 1e-11),
            (0.8, 6.6, 1.5e-10),
            (1.2, 4.25, 3e-11),
            (1.43, 48.0, 1e-9),
        ] {
            let curve = DwarfNovaCurve::of(solar(m), Days::new(hours / 24.0), solar(accreted));
            out.push((format!("dwarf nova {m} {hours} h"), curve.unwrap().into()));
        }
        for (accretor, m, accreted) in [
            (XrayAccretor::NeutronStar, 1.4, 1e-10),
            (XrayAccretor::NeutronStar, 3.2, 1e-6),
            (XrayAccretor::BlackHole, 8.0, 5e-9),
            (XrayAccretor::BlackHole, 300.0, 1.0),
        ] {
            let curve = transient(accretor, m, accreted);
            out.push((format!("transient {accretor:?} {accreted}"), curve.into()));
        }
        for fuel in [BurstFuel::Helium, BurstFuel::HydrogenHelium] {
            let curve = XrayBurstCurve::of(solar(1.4), fuel);
            out.push((format!("burst {fuel:?}"), curve.unwrap().into()));
        }
        let bex = BeXOutburstCurve::of(solar(1.4), HYDROGEN_RICH).unwrap();
        out.push(("Be/X".into(), bex.into()));
        for m in [0.5, 1.5, 8.0, 300.0] {
            let curve = LuminousRedNovaCurve::of(solar(m));
            out.push((format!("red nova {m}"), curve.unwrap().into()));
        }
        for m in [1e-6, 0.005, 0.05, 1.0] {
            out.push((
                format!("kilonova {m}"),
                KilonovaCurve::of(solar(m)).unwrap().into(),
            ));
        }
        out
    }

    /// Times from the onset, s: a log-spaced grid from a nanosecond to a millennium, each with its
    /// negative, the joins of `shape`, and the ends of the span's range.
    fn times(shape: &Shape) -> Vec<f64> {
        let mut out = vec![0.0, shape.rise_s, shape.top_s()];
        for i in 0..=480 {
            let t = math::exp10(-9.0 + f64::from(i) * 0.04);
            out.extend([t, -t]);
        }
        out.extend([9.2e18, -9.2e18]);
        out
    }

    #[test]
    fn every_curve_is_finite_non_negative_zero_before_its_onset_and_continuous_after_it() {
        for (name, curve) in curves() {
            let shape = curve.shape();
            assert!(shape.peak_w.is_finite() && shape.peak_w > 0.0, "{name}");
            for t in times(shape) {
                let l = shape.luminosity(t);
                assert!(l.is_finite() && l >= 0.0, "{name} at {t:e} s: {l:e}");
                if t < 0.0 {
                    assert_same_bits(l, 0.0);
                }
                // The steepest slope is the rise's or the fade's at its start, so a step of a
                // billionth of the rise moves the light by a few billionths of the peak at most.
                let h = 1e-9 * shape.rise_s;
                if t >= h {
                    let step = (shape.luminosity(t + h) - shape.luminosity(t - h)).abs();
                    assert!(
                        step <= 1e-8 * shape.peak_w,
                        "{name} at {t:e} s: a step of {step:e} W"
                    );
                }
            }
            let span = |s: i64| curve.bolometric(Span::from_seconds(s)).value();
            assert_same_bits(span(-1), 0.0);
            let just_before = Span::new(-1, 999_999_999).unwrap();
            assert_same_bits(curve.bolometric(just_before).value(), 0.0);
            assert_same_bits(span(0), 0.0);
            assert!(
                span(i64::MAX).is_finite() && span(i64::MAX) >= 0.0,
                "{name}"
            );
            assert_same_bits(span(i64::MIN), 0.0);
            assert!(curve.radiated_joules().is_finite(), "{name}");
        }
    }

    #[test]
    fn each_curve_peaks_where_its_form_says() {
        for (name, curve) in curves() {
            let shape = curve.shape();
            assert_same_bits(shape.luminosity(shape.rise_s), shape.peak_w);
            assert_same_bits(shape.luminosity(shape.top_s()), shape.peak_w);
            for t in times(shape) {
                assert!(shape.luminosity(t) <= shape.peak_w, "{name} at {t:e} s");
                if t < shape.rise_s || t > shape.top_s() {
                    assert!(shape.luminosity(t) < shape.peak_w, "{name} at {t:e} s");
                }
            }
            let at_peak = curve.bolometric(curve.peak()) / curve.peak_luminosity();
            assert!((at_peak - 1.0).abs() < 1e-9, "{name}: {at_peak}");
            let top = curve.fades_below(1.0).unwrap();
            let after = curve.bolometric(top.checked_add(Span::from_seconds(1)).unwrap());
            assert!(after < curve.peak_luminosity(), "{name}");
        }
        // A kilonova peaks at its diffusion time: a day for GW170817's ejecta, half a day for a
        // quarter of them.
        let kilonova = |m| KilonovaCurve::of(solar(m)).unwrap().shape.rise_s / SECONDS_PER_DAY;
        assert!((kilonova(0.05) - 1.0).abs() < 1e-12, "{}", kilonova(0.05));
        assert!(
            (kilonova(0.0125) - 0.5).abs() < 1e-12,
            "{}",
            kilonova(0.0125)
        );
    }

    /// The radiated light is the bolometric's integral, and a fade-below time is where the light
    /// falls through the share.
    #[test]
    fn the_radiated_light_and_the_fade_times_are_the_curves_own() {
        for (name, curve) in curves() {
            let shape = curve.shape();
            let end = shape.fades_below(1e-12);
            // Trapezoids on a log grid from the rise's millionth to the far tail, with the rise
            // and the plateau's ends among the nodes.
            let first = 1e-6 * shape.rise_s;
            let mut nodes = vec![0.0, shape.rise_s, shape.top_s()];
            nodes
                .extend((0..=200_000).map(|i| first * math::powf(end / first, f64::from(i) / 2e5)));
            nodes.sort_by(f64::total_cmp);
            let integral = nodes.windows(2).fold(0.0, |sum, w| {
                sum + (w[1] - w[0]) * (shape.luminosity(w[0]) + shape.luminosity(w[1])) / 2.0
            });
            // A power law's tail beyond the grid's end carries end × L(end) ÷ (n − 1).
            let tail = match shape.fade {
                Fade::Exponential { .. } => 0.0,
                Fade::PowerLaw { index } => end * shape.luminosity(end) / (index - 1.0),
            };
            let ratio = (integral + tail) / curve.radiated_joules();
            assert!((ratio - 1.0).abs() < 1e-6, "{name}: {ratio}");
            for share in [0.5, 0.1, 1e-3] {
                let t = shape.fades_below(share);
                let at = shape.luminosity(t) / shape.peak_w;
                assert!((at / share - 1.0).abs() < 1e-9, "{name} at {share}: {at}");
                assert!(shape.luminosity(t * 1.01) < share * shape.peak_w, "{name}");
            }
            assert_eq!(curve.fades_below(0.0), None, "{name}");
            assert_eq!(curve.fades_below(f64::NAN), None, "{name}");
            assert_eq!(curve.fades_below(2.0), curve.fades_below(1.0), "{name}");
        }
    }

    #[test]
    fn a_heavier_white_dwarf_declines_faster() {
        let nova = |m: f64| NovaCurve::of(solar(m)).unwrap();
        let mut last = nova(0.5);
        for i in 1..=18 {
            let next = nova(0.5 + 0.05 * f64::from(i));
            let (m, t3) = (next.white_dwarf_mass().value(), next.t3().value());
            assert!(t3 <= last.t3().value(), "{m}: {t3} d");
            if t3 > FASTEST_T3_DAYS {
                assert!(t3 < last.t3().value(), "{m}: {t3} d");
            }
            assert!(next.shape.peak_w > last.shape.peak_w, "{m}");
            last = next;
        }
        // Livio's t₃ is some 80 days at 0.6 M☉ and 16 at 1 M☉, and the fastest from 1.26 M☉ up.
        assert!(
            (nova(0.6).t3().value() - 79.1).abs() < 0.5,
            "{}",
            nova(0.6).t3().value()
        );
        assert!(
            (nova(1.0).t3().value() - 15.6).abs() < 0.1,
            "{}",
            nova(1.0).t3().value()
        );
        assert!(nova(1.25).t3().value() > FASTEST_T3_DAYS);
        for m in [1.26, 1.3, 1.4, 1.439] {
            assert_same_bits(nova(m).t3().value(), FASTEST_T3_DAYS);
        }
    }

    #[test]
    fn a_neutron_stars_bursts_stay_under_its_eddington_luminosity() {
        for i in 0..=44 {
            let mass = solar(1.0 + 0.05 * f64::from(i));
            let helium = eddington_luminosity(mass, 0.0);
            for (fuel, hydrogen) in [
                (BurstFuel::Helium, 0.0),
                (BurstFuel::HydrogenHelium, MIXED_FUEL_HYDROGEN),
            ] {
                let burst = XrayBurstCurve::of(mass, fuel).unwrap().shape;
                let held = eddington_luminosity(mass, hydrogen).value();
                assert!(burst.peak_w <= held, "{fuel:?}");
                assert!(burst.peak_w <= helium.value(), "{fuel:?}");
            }
            for accreted in [1e-12, 1e-9, 1e-6, 1.0] {
                for hydrogen in [0.0, HYDROGEN_RICH, 1.0] {
                    let curve = XrayTransientCurve::of(
                        XrayAccretor::NeutronStar,
                        mass,
                        solar(accreted),
                        hydrogen,
                    );
                    let held = eddington_luminosity(mass, hydrogen).value();
                    assert!(curve.unwrap().shape.peak_w <= held, "{accreted} {hydrogen}");
                }
            }
        }
        // The helium Eddington luminosity of a 1.4 M☉ star is 3.52 × 10³⁸ erg s⁻¹.
        let watts = eddington_luminosity(solar(1.4), 0.0).value();
        assert!((watts / 3.52e31 - 1.0).abs() < 2e-3, "{watts:e}");
        // No neutron star a curve takes radiates more than its fall's rest energy.
        assert!(XrayAccretor::NeutronStar.efficiency(MAX_NEUTRON_STAR_MASS) < 1.0);
    }

    #[test]
    fn in_band_light_never_exceeds_the_bolometric() {
        for (name, curve) in curves() {
            for t in times(curve.shape()) {
                let Some(dt) = Span::from_seconds_f64(t) else {
                    continue;
                };
                let bolometric = curve.bolometric(dt);
                let mut total = curve.xray_luminosity(dt);
                assert!(total <= bolometric, "{name} at {t:e} s");
                for band in Band::ALL {
                    let light = curve.in_band(dt, band);
                    assert!(
                        light >= Watts::ZERO && light <= bolometric,
                        "{name} {band:?}"
                    );
                    total = total + light;
                }
                assert!(total <= bolometric, "{name} at {t:e} s");
                assert_same_bits(curve.in_band(dt, Band::Radio).value(), 0.0);
            }
        }
    }

    /// The peak in a band bounds the band's light at every time and is its light at the peak, to
    /// the nanosecond the peak is floored to, and on a red nova's plateau bit for bit.
    #[test]
    fn the_peak_in_a_band_is_the_band_s_greatest() {
        for (name, curve) in curves() {
            let peaks = Band::ALL.map(|band| curve.peak_in_band(band));
            let xray = curve.peak_xray_luminosity();
            for t in times(curve.shape()) {
                let Some(dt) = Span::from_seconds_f64(t) else {
                    continue;
                };
                for (band, peak) in Band::ALL.into_iter().zip(peaks) {
                    assert!(
                        curve.in_band(dt, band) <= peak,
                        "{name} {band:?} at {t:e} s"
                    );
                }
                assert!(curve.xray_luminosity(dt) <= xray, "{name} at {t:e} s");
            }
            let at_peak = curve.peak();
            for (band, peak) in Band::ALL.into_iter().zip(peaks) {
                let light = curve.in_band(at_peak, band).value();
                assert!(light >= peak.value() * (1.0 - 1e-9), "{name} {band:?}");
            }
            let light = curve.xray_luminosity(at_peak).value();
            assert!(light >= xray.value() * (1.0 - 1e-9), "{name}");
            if let BinaryLightCurve::LuminousRedNova(red) = curve {
                let plateau = Span::from_seconds_f64(
                    curve.shape().rise_s + red.plateau().value() * SECONDS_PER_DAY / 2.0,
                )
                .unwrap();
                for (band, peak) in Band::ALL.into_iter().zip(peaks) {
                    assert_same_bits(curve.in_band(plateau, band).value(), peak.value());
                }
            }
        }
    }

    /// Only the X-ray kinds have X-rays, and a Be/X outburst has no optical light of its own.
    #[test]
    fn the_xray_kinds_alone_have_xrays() {
        for (name, curve) in curves() {
            let dt = curve.peak();
            let xrays = curve.xray_luminosity(dt) > Watts::ZERO;
            let kind = matches!(
                curve,
                BinaryLightCurve::XrayTransient(_)
                    | BinaryLightCurve::XrayBurst(_)
                    | BinaryLightCurve::BeXOutburst(_)
            );
            assert_eq!(xrays, kind, "{name}");
            let optical = curve.in_band(dt, Band::V) > Watts::ZERO;
            assert_eq!(
                optical,
                !matches!(curve, BinaryLightCurve::BeXOutburst(_)),
                "{name}"
            );
        }
    }

    /// `curve`'s absolute magnitude in `band` at its peak, through the Sun's light in the band
    /// (module documentation): the inverse of the conversion the novae and dwarf novae are built
    /// with.
    fn absolute_magnitude(curve: &BinaryLightCurve, band: MagnitudeBand) -> f64 {
        let light = curve.in_band(curve.peak(), band.band()).value();
        let solar = SOLAR_LUMINOSITY_W * Colour::Sun.share(band.band());
        band.solar_absolute_magnitude() - 2.5 * math::log10(light / solar)
    }

    /// Days from the onset until the light stays under a tenth of its peak, 2.5 magnitudes down.
    fn days_above_a_tenth(curve: &BinaryLightCurve) -> f64 {
        curve.fades_below(0.1).unwrap().as_seconds_f64() / SECONDS_PER_DAY
    }

    /// The novae's and dwarf novae's peaks and the light they radiate against their sources'
    /// ranges, at parameters across their range. A miss is a finding (P11.T9.a's findings), pinned
    /// here as it stands.
    #[test]
    fn the_white_dwarfs_outbursts_lie_in_their_sources_ranges() {
        // Novae: −5.5 to −10 at maximum in V (Della Valle and Izzo 2020, A&ARv 28, 3), and about
        // 10⁴⁵ erg (Gallagher and Starrfield 1978, ARA&A 16, 171), taken as 10⁴⁴–10⁴⁶.
        for m in [0.6, 0.8, 1.0, 1.2, 1.35] {
            let nova = BinaryLightCurve::from(NovaCurve::of(solar(m)).unwrap());
            let v = absolute_magnitude(&nova, MagnitudeBand::V);
            assert!((-10.0..=-5.5).contains(&v), "nova {m}: M_V {v}");
            let erg = nova.radiated_joules() / ERG;
            assert!((1e44..=1e46).contains(&erg), "nova {m}: {erg:e} erg");
        }
        // Dwarf novae: Warner's maximum in V, by construction, at periods of 1.3–10 h and held
        // there beyond; normal outbursts last 2–20 days (Warner 1995, chapter 3); and an outburst
        // with a plateau radiates its gathered mass's fall onto the disc, G M ΔM ÷ 2R, so its
        // energy is the caller's, by construction.
        let fall = |m: f64, accreted: f64| {
            let radius = white_dwarf_radius(RemnantRecipe::default(), solar(m)).value();
            GM_SUN * m * accreted * SOLAR_MASS_KG / (2.0 * radius * SOLAR_RADIUS_M)
        };
        for (m, hours, accreted) in [(0.6, 1.5, 3e-12), (0.8, 4.0, 5e-11), (0.8, 6.6, 1.5e-10)] {
            let period = Days::new(hours / 24.0);
            let curve = DwarfNovaCurve::of(solar(m), period, solar(accreted)).unwrap();
            let dwarf_nova = BinaryLightCurve::from(curve);
            let v = absolute_magnitude(&dwarf_nova, MagnitudeBand::V);
            assert!(
                (v - (5.74 - 0.259 * hours)).abs() < 1e-9,
                "dwarf nova {hours} h: {v}"
            );
            let days = days_above_a_tenth(&dwarf_nova);
            assert!(
                (2.0..=20.0).contains(&days),
                "dwarf nova {hours} h: {days} d"
            );
            let ratio = dwarf_nova.radiated_joules() / fall(m, accreted);
            if curve.plateau().value() > 0.0 {
                assert!((ratio - 1.0).abs() < 1e-12, "dwarf nova {hours} h: {ratio}");
            } else {
                // A finding: with no plateau, Warner's peak radiates 5.7 times the fall.
                assert!((ratio - 5.7).abs() < 0.1, "dwarf nova {hours} h: {ratio}");
            }
        }
        let at = |hours: f64| {
            let period = Days::new(hours / 24.0);
            DwarfNovaCurve::of(solar(0.8), period, solar(1e-10))
                .unwrap()
                .shape
        };
        assert_eq!(at(48.0), at(10.0));
        assert_eq!(at(0.5), at(1.3));
    }

    /// The X-ray kinds' peaks and the light they radiate against their sources' ranges. A miss is a
    /// finding, pinned here as it stands.
    #[test]
    fn the_xray_kinds_lie_in_their_sources_ranges() {
        // X-ray transients: peaks of 10³⁷–10³⁹ erg s⁻¹ (Chen, Shrader and Livio 1997), and the
        // gathered mass's fall at the accretor's efficiency, the caller's, by construction, or
        // less where the Eddington limit holds the peak.
        for (accretor, m, accreted) in [
            (XrayAccretor::NeutronStar, 1.4, 1e-10),
            (XrayAccretor::NeutronStar, 1.4, 1e-9),
            (XrayAccretor::BlackHole, 8.0, 5e-9),
            (XrayAccretor::BlackHole, 8.0, 1e-8),
        ] {
            let curve = BinaryLightCurve::from(transient(accretor, m, accreted));
            let erg_s = curve.peak_luminosity().value() / ERG;
            assert!(
                (1e37..=1e39).contains(&erg_s),
                "{accretor:?} {accreted}: {erg_s:e}"
            );
            let rest = accreted * SOLAR_MASS_KG * SPEED_OF_LIGHT * SPEED_OF_LIGHT;
            let ratio = curve.radiated_joules() / (accretor.efficiency(m) * rest);
            assert!(
                (ratio - 1.0).abs() < 1e-12,
                "{accretor:?} {accreted}: {ratio}"
            );
        }
        // At the Eddington limit the fall is not all taken: an 8 M☉ black hole fed 10⁻⁷ M☉ peaks
        // at its own limit, 1.2 × 10³⁹ erg s⁻¹, just above the range (a finding), and radiates
        // less.
        let held = transient(XrayAccretor::BlackHole, 8.0, 1e-7).shape.peak_w;
        assert_same_bits(
            held,
            eddington_luminosity(solar(8.0), HYDROGEN_RICH).value(),
        );
        // Type I bursts: 10³⁹–10⁴⁰ erg each (Lewin, van Paradijs and Taam 1993).
        for fuel in [BurstFuel::Helium, BurstFuel::HydrogenHelium] {
            let burst = BinaryLightCurve::from(XrayBurstCurve::of(solar(1.4), fuel).unwrap());
            let erg = burst.radiated_joules() / ERG;
            assert!((1e39..=1e40).contains(&erg), "{fuel:?}: {erg:e} erg");
        }
        // Be/X giant outbursts: 10³⁷–10³⁸ erg s⁻¹ for weeks to months (Reig 2011), so some
        // 10⁴³–10⁴⁵ erg over two weeks to four months.
        let bex = BeXOutburstCurve::of(solar(1.4), HYDROGEN_RICH).unwrap();
        let bex = BinaryLightCurve::from(bex);
        let erg_s = bex.peak_luminosity().value() / ERG;
        assert!((1e37..=1e38).contains(&erg_s), "Be/X: {erg_s:e}");
        let days = days_above_a_tenth(&bex);
        assert!((14.0..=120.0).contains(&days), "Be/X: {days} d");
        let erg = bex.radiated_joules() / ERG;
        assert!((1e43..=1e45).contains(&erg), "Be/X: {erg:e} erg");
    }

    /// The red novae's luminosity against the eight of `MacLeod`, De and Loeb's (2022) compilation,
    /// each one's luminosity over the time it radiates 90% of its light (erg s⁻¹) at its
    /// progenitor's mass (their `lrn.txt`, read at source): within a factor of 4.1 of each, 0.29 dex
    /// above them on average, as measured (P11.T9.a's fix of 2026-11-01). Up to 30 M☉ the law is
    /// P11.T9.a's, bit for bit; above it the light grows as M^⅔, continuous at the break, and a 300
    /// M☉ merger peaks at 2.5 × 10⁸ L☉, 21 times its Eddington luminosity, where the steep law gave
    /// 1,464 times.
    #[test]
    fn the_red_novae_follow_the_ones_seen_and_flatten_above_the_heaviest() {
        const SEEN: [(&str, f64, f64); 8] = [
            ("V4332 Sgr", 1.0, 1.3e37),
            ("V838 Mon", 8.0, 3.6e39),
            ("V1309 Sco", 1.4, 8.9e37),
            ("NGC 4490-OT2011", 30.0, 2.2e41),
            ("M31-LRN-2015", 4.25, 1.5e39),
            ("M101 OT2015-1", 18.0, 1.4e40),
            ("SNhunt248", 30.0, 6.2e40),
            ("AT 2018bwo", 14.0, 1.3e40),
        ];
        let peak = |m: f64| {
            LuminousRedNovaCurve::of(solar(m))
                .map(|c| BinaryLightCurve::from(c).peak_luminosity().value())
                .unwrap()
        };
        let mut sum = 0.0;
        for (name, mass, erg_s) in SEEN {
            let dex = math::log10(peak(mass) / (erg_s * 1e-7));
            assert!(dex.abs() < math::log10(4.1), "{name}: {dex} dex");
            sum += dex;
        }
        let mean = sum / 8.0;
        assert!((mean - 0.29).abs() < 0.01, "{mean} dex");
        // P11.T9.a's law to the break, bit for bit, and continuous across it.
        for m in [0.5, 1.5, 8.0, 29.999, 30.0] {
            let steep = RED_NOVA_ANCHOR_SOLAR_LUMINOSITIES
                * SOLAR_LUMINOSITY_W
                * math::powf(m / RED_NOVA_ANCHOR_MASS.value(), RED_NOVA_INDEX);
            assert_same_bits(peak(m), steep);
        }
        let above = peak(RED_NOVA_BREAK_MASS * (1.0 + 1e-12));
        assert!((above / peak(RED_NOVA_BREAK_MASS) - 1.0).abs() < 1e-11);
        let index = math::log10(peak(300.0) / peak(100.0)) / math::log10(3.0);
        assert!((index - RED_NOVA_HEAVY_INDEX).abs() < 1e-12, "{index}");
        let top = peak(300.0);
        let eddington = eddington_luminosity(solar(300.0), 0.7).value();
        assert!(
            (top / SOLAR_LUMINOSITY_W / 2.49e8 - 1.0).abs() < 2e-3,
            "{top:e} W"
        );
        assert!(
            (top / eddington - 21.5).abs() < 0.1,
            "{} Eddington",
            top / eddington
        );
    }

    /// The mergers' peaks and the light they radiate against their sources' ranges. A miss is a
    /// finding, pinned here as it stands.
    #[test]
    fn the_mergers_lie_in_their_sources_ranges() {
        // Luminous red novae: V1309 Sco's 3 × 10⁴ L☉ (Tylenda et al. 2011) by construction, V838
        // Mon's 10⁶ L☉ from a pair of about 8 M☉ (Tylenda 2005, A&A 436, 1009) within a factor of
        // three, and the task's plateau of 100–200 days. The light they radiated, some 2 × 10⁴⁴
        // and 10⁴⁶ erg (the review's estimate from their peaks and durations), is missed: the
        // plateau at the peak radiates about six and twelve times as much (a finding), pinned.
        let red = |m| BinaryLightCurve::from(LuminousRedNovaCurve::of(solar(m)).unwrap());
        let solar_units = |m| red(m).peak_luminosity().value() / SOLAR_LUMINOSITY_W;
        assert!(
            (solar_units(1.5) / 3e4 - 1.0).abs() < 1e-12,
            "{}",
            solar_units(1.5)
        );
        assert!(
            (3.3e5..=3e6).contains(&solar_units(8.0)),
            "{:e}",
            solar_units(8.0)
        );
        for m in [0.5, 1.5, 8.0, 300.0] {
            let plateau = LuminousRedNovaCurve::of(solar(m))
                .unwrap()
                .plateau()
                .value();
            assert!(
                (100.0..=200.0).contains(&plateau),
                "red nova {m}: {plateau} d"
            );
        }
        let v1309 = red(1.5).radiated_joules() / ERG / 2e44;
        assert!((v1309 - 7.07).abs() < 0.01, "V1309 Sco: {v1309} times");
        let v838 = red(8.0).radiated_joules() / ERG / 1e46;
        assert!((v838 - 14.4).abs() < 0.1, "V838 Mon: {v838} times");
        // Kilonovae: 10⁴⁰–10⁴² erg s⁻¹ at their peaks (Metzger 2020), and GW170817's, some 10⁴⁷
        // erg (Cowperthwaite et al. 2017), taken as a factor of five about it.
        for m in [0.005, 0.05, 0.1] {
            let kilonova = BinaryLightCurve::from(KilonovaCurve::of(solar(m)).unwrap());
            let erg_s = kilonova.peak_luminosity().value() / ERG;
            assert!((1e40..=1e42).contains(&erg_s), "kilonova {m}: {erg_s:e}");
        }
        let gw170817 = BinaryLightCurve::from(KilonovaCurve::of(GW170817_EJECTA_MASS).unwrap());
        let erg = gw170817.radiated_joules() / ERG;
        assert!((2e46..=5e47).contains(&erg), "GW170817: {erg:e} erg");
    }

    /// An X-ray transient's optical light against van Paradijs and `McClintock`'s (1994, A&A 290,
    /// 133) relation for low-mass X-ray binaries, `M_V` = 1.57 − 2.27 log₁₀ Σ, Σ = (L ÷ `L_Edd`)^½
    /// (P ÷ 1 h)^⅔: a neutron star at a fifth of its Eddington luminosity on an eight-hour orbit,
    /// which [`REPROCESSED_SHARE`] is set by, within half a magnitude. A black hole at the same
    /// share of its own limit is brighter by its mass over the neutron star's, since the share is
    /// one figure where the relation scales with the disc: a finding, pinned.
    #[test]
    fn a_transients_optical_light_meets_van_paradijs_and_mcclintock() {
        let at_a_fifth = |accretor: XrayAccretor, m: f64| {
            let held = eddington_luminosity(solar(m), HYDROGEN_RICH).value();
            // The mass whose fall, radiated over the rise and the decay, peaks at a fifth of it.
            let fall = accretor.efficiency(m) * SOLAR_MASS_KG * SPEED_OF_LIGHT * SPEED_OF_LIGHT;
            let accreted = 0.2 * held * (TRANSIENT_RISE_S / 2.0 + TRANSIENT_E_FOLD_S) / fall;
            let curve = BinaryLightCurve::from(transient(accretor, m, accreted));
            assert!((curve.peak_luminosity().value() / held - 0.2).abs() < 1e-9);
            absolute_magnitude(&curve, MagnitudeBand::V)
        };
        let relation = 1.57 - 2.27 * math::log10(0.2_f64.sqrt() * math::powf(8.0, 2.0 / 3.0));
        let neutron_star = at_a_fifth(XrayAccretor::NeutronStar, 1.4);
        assert!(
            (neutron_star - relation).abs() < 0.5,
            "{neutron_star} against {relation}"
        );
        let black_hole = at_a_fifth(XrayAccretor::BlackHole, 8.0);
        let brighter = 2.5 * math::log10(8.0 / 1.4);
        assert!(
            (black_hole - (neutron_star - brighter)).abs() < 1e-9,
            "{black_hole}"
        );
    }

    #[test]
    fn the_constructors_refuse_what_no_event_has() {
        let e = Err(BuildLightCurveError::MassOutOfRange);
        let period = Days::new(0.2);
        let x = HYDROGEN_RICH;
        for m in [0.0, -1.0, f64::NAN, f64::INFINITY, 1e300] {
            assert_eq!(NovaCurve::of(solar(m)).map(|_| ()), e, "{m}");
            assert_eq!(
                DwarfNovaCurve::of(solar(m), period, solar(1e-10)).map(|_| ()),
                e
            );
            assert_eq!(
                DwarfNovaCurve::of(solar(0.8), period, solar(m)).map(|_| ()),
                e
            );
            for accretor in [XrayAccretor::NeutronStar, XrayAccretor::BlackHole] {
                let heavy = XrayTransientCurve::of(accretor, solar(m), solar(1e-9), x);
                assert_eq!(heavy.map(|_| ()), e);
                let fed = XrayTransientCurve::of(accretor, solar(1.4), solar(m), x);
                assert_eq!(fed.map(|_| ()), e);
            }
            assert_eq!(
                XrayBurstCurve::of(solar(m), BurstFuel::Helium).map(|_| ()),
                e
            );
            assert_eq!(BeXOutburstCurve::of(solar(m), x).map(|_| ()), e);
            assert_eq!(LuminousRedNovaCurve::of(solar(m)).map(|_| ()), e);
            assert_eq!(KilonovaCurve::of(solar(m)).map(|_| ()), e);
        }
        for days in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(
                DwarfNovaCurve::of(solar(0.8), Days::new(days), solar(1e-10)).map(|_| ()),
                Err(BuildLightCurveError::PeriodNotPositive)
            );
        }
        // Each kind's bounds: a white dwarf from 0.1 M☉ to under 1.44, a neutron star to 3.2, a
        // black hole or merger to 300, and at most a solar mass gathered or ejected.
        assert_eq!(NovaCurve::of(solar(0.09)).map(|_| ()), e);
        assert_eq!(NovaCurve::of(solar(1.44)).map(|_| ()), e);
        assert!(NovaCurve::of(solar(1.439)).is_ok());
        let heavy_neutron_star =
            XrayTransientCurve::of(XrayAccretor::NeutronStar, solar(3.3), solar(1e-9), x);
        assert_eq!(heavy_neutron_star.map(|_| ()), e);
        let black_hole =
            XrayTransientCurve::of(XrayAccretor::BlackHole, solar(3.3), solar(1e-9), x);
        assert!(black_hole.is_ok());
        assert_eq!(
            XrayBurstCurve::of(solar(3.3), BurstFuel::Helium).map(|_| ()),
            e
        );
        assert_eq!(LuminousRedNovaCurve::of(solar(301.0)).map(|_| ()), e);
        assert_eq!(KilonovaCurve::of(solar(1.01)).map(|_| ()), e);
        assert_eq!(
            DwarfNovaCurve::of(solar(0.8), period, solar(1.01)).map(|_| ()),
            e
        );
        for hydrogen in [-0.1, 1.1, f64::NAN] {
            let curve = XrayTransientCurve::of(
                XrayAccretor::NeutronStar,
                solar(1.4),
                solar(1e-9),
                hydrogen,
            );
            let h = Err(BuildLightCurveError::HydrogenOutOfRange);
            assert_eq!(curve.map(|_| ()), h);
            assert_eq!(BeXOutburstCurve::of(solar(1.4), hydrogen).map(|_| ()), h);
        }
        assert_eq!(
            BuildLightCurveError::MassOutOfRange.to_string(),
            "a light curve's mass is outside its kind's range"
        );
    }
}
