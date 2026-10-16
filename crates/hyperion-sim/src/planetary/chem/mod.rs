//! Element chemistry on the snapshot path (plan 14, Phase L, P14.T50; decision-composition §1.2).
//!
//! - [`inventory`]: a body's [`ElementInventory`](inventory::ElementInventory), its volatile and
//!   redox-active elements by mass and reservoir, of which P14.T13's
//!   [`VolatileInventory`](crate::planetary::derive::atmosphere::VolatileInventory) is the view.
//! - [`redox`]: the mantle's one redox figure, ΔIW, and its banding,
//!   [`RedoxClass`](redox::RedoxClass).
//! - [`envelope_abundances`]: a gas envelope's elements relative to hydrogen
//!   ([`EnvelopeAbundances`](envelope_abundances::EnvelopeAbundances), P14.T50.b), on the Surface
//!   path, which nothing on the record reads until P14.T54.b.
//! - [`partition`]: the condensate partition at the ground (P14.T51.a), how much of each
//!   condensable a body's air holds over its climate, how much lies on its surface, and in which
//!   phase.
//! - Here, the host's C/O and Mg/Si ([`HostAbundances`]), drawn per system on
//!   `system.abundances` ([`AbundanceDraws`]), since a star's planets share its photospheric
//!   ratios, until plan 06 carries them on the star. Plan 06 may adopt the stream without moving
//!   a value: it is keyed by the system and named for what it holds.
//!
//! Nothing on the record reads this module until P14.T54.a wires it in the 22 → 23 batch, so no
//! generated output moves with it.
//!
//! # The host's ratios
//!
//! log(C/O) and log(Mg/Si) are the element registry's protosolar values (Lodders 2010, Table 6)
//! plus a trend in \[Fe/H\] and a log-normal scatter (the science agent's ruling for P14.T50.a,
//! 2026-10-14):
//!
//! - **C/O**, from Brewer and Fischer's (2016, ApJ 831, 20, arXiv:1608.06286, read) 849 FGK dwarfs
//!   of the solar neighbourhood, rises with \[Fe/H\] to a plateau above −0.1 dex.
//!   - Their quadratic fit (Fig. 2; its coefficients are not printed) is C/O = 0.460 +
//!     0.154 \[Fe/H\] − 0.187 \[Fe/H\]². The lane digitised the arXiv figure's curve at 300 dpi, a
//!     pixel 0.0007 in C/O; the 1,676 points lie within 0.0002 rms of that quadratic, whose
//!     coefficients are rounded to 0.001.
//!   - It is laid on their Table 1's scale, whose solar C/O is Grevesse et al.'s (2007)
//!     10^(8.39 − 8.66) = 0.537, recovered from the table's rows by their eq. 1 (their text and
//!     Fig. 1 quote 0.55). The trend is log₁₀ of the fit over 0.537, so that a star of solar
//!     metallicity is 0.067 dex poorer in carbon than the Sun, as theirs are ("the Sun ... is
//!     slightly carbon rich compared to the stars in our sample").
//!   - \[Fe/H\] is held to their sample's −1.0 to +0.6, outside which the quadratic is not fitted
//!     (it falls below zero by −1.5).
//!   - The scatter, 0.035 dex, is their rms of 0.06 about the fit at C/O 0.46 (0.057 dex) with
//!     their per-star error, 0.026 dex in \[C/H\] and 0.036 in \[O/H\] (§2.3), taken out in
//!     quadrature. None of their stars reaches C/O 0.7 (0.66 ± 0.068 the largest).
//! - **Mg/Si** falls with \[Fe/H\] by Galactic chemical evolution: \[Mg/Si\] = 0.0045 −
//!   0.0873 \[Fe/H\] (Adibekyan et al. 2015, A&A 581, L2, arXiv:1508.04970, §3, read; 589 HARPS
//!   dwarfs above \[Fe/H\] −0.6).
//!   - Its zero point is Brewer and Fischer's median, 1.02, on their scale, on which the Sun's is
//!     Grevesse et al.'s 1.047 (recovered as their C/O's was; their Fig. 3), so the offset at
//!     \[Fe/H\] = 0 is log₁₀(1.02 ÷ 1.047) = −0.011 dex.
//!   - The slope is Adibekyan et al.'s, with \[Fe/H\] held to −0.6 to +0.4, the range of their
//!     sample.
//!   - The scatter, 0.032 dex, is the standard deviation of log Mg/Si over Brewer and Fischer's
//!     Fig. 3 histogram (digitised by the lane, 0.035 dex) with their 3.3% error taken out.
//!
//! Lodders's solar C/O, 0.457, is the series this plan's disc reads; Asplund, Amarsi and
//! Grevesse's (2021) photospheric 0.59 ± 0.08 is the recorded alternative (plan 14's Risks).
//! Both are scales on which a relative \[C/O\] is laid, which is what the surveys measure.

pub mod envelope_abundances;
pub mod inventory;
pub mod partition;
pub mod redox;

use crate::Seed;
use crate::id::SystemId;
use crate::math;
use crate::rng::{ObjectKey, Stream, tags};
use crate::stellar::draws::UnitUniform;
use crate::substance::Element;
use crate::units::Dex;

/// Words of `system.abundances` a system reads or reserves: word 0, the rank of its host's C/O;
/// word 1, that of its Mg/Si; words 2–7 reserved, for later stellar ratios.
pub const ABUNDANCE_WORDS: u64 = 8;

/// The ranks of a system's host abundances, as drawn from `system.abundances` by
/// [`AbundanceDraws::for_system`], or given explicitly by a test or a tool (P14.T50.a).
///
/// The fields are plain ranks with no invariant between them, so they are public, as the
/// volatiles' are.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbundanceDraws {
    /// The rank of the host's C/O about its trend. Word 0.
    pub carbon_to_oxygen: UnitUniform,
    /// The rank of its Mg/Si. Word 1.
    pub magnesium_to_silicon: UnitUniform,
}

impl AbundanceDraws {
    /// Both ranks at their medians: the trends' values.
    pub const MEDIAN: Self = Self {
        carbon_to_oxygen: UnitUniform::HALF,
        magnesium_to_silicon: UnitUniform::HALF,
    };

    /// The draws of the system `system` in the universe of `seed`: words 0 and 1 of its
    /// `system.abundances` stream.
    ///
    /// The stream is keyed by the system, so that every body of it reads one value.
    ///
    /// # Panics
    ///
    /// Never: an open uniform lies strictly between 0 and 1.
    ///
    /// # Examples
    ///
    /// Two planets of one system read one host:
    ///
    /// ```
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::id::{BodyId, SystemId};
    /// use hyperion_sim::planetary::chem::AbundanceDraws;
    ///
    /// let system = SystemId::from_raw(0x0200_0800_2000_0000)?;
    /// let (b, c) = (BodyId::new(system, 0x0100), BodyId::new(system, 0x0200));
    /// let seed = Seed::new(7);
    /// assert_eq!(
    ///     AbundanceDraws::for_system(seed, b.system()),
    ///     AbundanceDraws::for_system(seed, c.system()),
    /// );
    /// # Ok::<(), hyperion_sim::id::DecodeSystemIdError>(())
    /// ```
    #[must_use]
    pub fn for_system(seed: Seed, system: SystemId) -> Self {
        let mut stream = Stream::open(seed, tags::SYSTEM_ABUNDANCES, ObjectKey::from(system));
        let mut rank = || {
            UnitUniform::new(stream.uniform_open())
                .expect("an open uniform lies strictly between 0 and 1")
        };
        let carbon_to_oxygen = rank();
        let magnesium_to_silicon = rank();
        Self {
            carbon_to_oxygen,
            magnesium_to_silicon,
        }
    }
}

/// Brewer and Fischer's (2016) quadratic fit of C/O in \[Fe/H\], its constant, linear and
/// quadratic coefficients: C/O = 0.460 + 0.154 \[Fe/H\] − 0.187 \[Fe/H\]², digitised by the lane
/// from their Fig. 2 and rounded to 0.001 (the module's documentation).
pub const CARBON_TO_OXYGEN_FIT: [f64; 3] = [0.460, 0.154, -0.187];

/// The Sun's C/O on Brewer and Fischer's Table 1 scale, which the fit is relative to: 0.537.
///
/// It is Grevesse et al.'s (2007) 10^(8.39 − 8.66), recovered from Table 1's rows by their eq. 1;
/// their text and Fig. 1 quote 0.55.
pub const FIT_SOLAR_CARBON_TO_OXYGEN: f64 = 0.537;

/// The \[Fe/H\] to which the C/O fit is held, dex: Brewer and Fischer's sample's −1.0 to +0.6
/// (their Fig. 2).
pub const FIT_METALLICITY_RANGE: (Dex, Dex) = (Dex::new(-1.0), Dex::new(0.6));

/// The scatter of log C/O about its trend, dex: 0.035.
///
/// Brewer and Fischer's rms of 0.06 about the fit at the sample's C/O of 0.46, 0.057 dex, less
/// their per-star error, √(0.026² + 0.036²) = 0.044 dex, in quadrature.
pub const CARBON_TO_OXYGEN_SCATTER: Dex = Dex::new(0.035);

/// The median Mg/Si on Brewer and Fischer's scale, 1.02 (their §3), and the Sun's there, 1.047,
/// Grevesse et al.'s (2007) ratio, recovered from their Table 1.
pub const FIT_MAGNESIUM_TO_SILICON: (f64, f64) = (1.02, 1.047);

/// The trend of log Mg/Si in \[Fe/H\], dex per dex: −0.0873 ± 0.0071 (Adibekyan et al. 2015,
/// A&A 581, L2, §3: \[Mg/Si\] = 0.0045 − 0.0873 \[Fe/H\]).
pub const MAGNESIUM_TO_SILICON_TREND: f64 = -0.0873;

/// The \[Fe/H\] to which the Mg/Si trend is held, dex: −0.6 to +0.4, the range of Adibekyan et
/// al.'s sample.
pub const MAGNESIUM_TO_SILICON_METALLICITY_RANGE: (Dex, Dex) = (Dex::new(-0.6), Dex::new(0.4));

/// The scatter of log Mg/Si, dex: 0.032.
///
/// It is the standard deviation of log Mg/Si over Brewer and Fischer's Fig. 3 histogram (digitised
/// by the lane, 0.035 dex) less their 3.3% (0.0144 dex) error in quadrature.
pub const MAGNESIUM_TO_SILICON_SCATTER: Dex = Dex::new(0.032);

/// log₁₀ of the protosolar ratio by number of `a` to `b` (Lodders 2010, Table 6).
#[must_use]
fn protosolar_log_ratio(a: Element, b: Element) -> f64 {
    (a.protosolar_log_abundance() - b.protosolar_log_abundance()).value()
}

/// The solar system's protosolar C/O by number: 0.457 (Lodders 2010).
#[must_use]
pub fn solar_carbon_to_oxygen() -> f64 {
    math::exp10(protosolar_log_ratio(Element::C, Element::O))
}

/// The solar system's protosolar Mg/Si by number: 1.023 (Lodders 2010).
#[must_use]
pub fn solar_magnesium_to_silicon() -> f64 {
    math::exp10(protosolar_log_ratio(Element::Mg, Element::Si))
}

/// The host's carbon-to-oxygen and magnesium-to-silicon ratios by number (P14.T50.a), which the
/// gas envelopes' C/O (P14.T50.b) and the crust's mineralogy (P14.T51.c) read.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct HostAbundances {
    log_carbon_to_oxygen: Dex,
    log_magnesium_to_silicon: Dex,
}

impl HostAbundances {
    /// The ratios of a host of metallicity `fe_h` at the ranks `draws` (the module's model).
    ///
    /// # Examples
    ///
    /// A host of solar metallicity at the median rank is a little poorer in carbon than the
    /// Sun, as Brewer and Fischer's stars are, and a metal-rich one poorer in magnesium:
    ///
    /// ```
    /// use hyperion_sim::planetary::chem::{AbundanceDraws, HostAbundances, solar_carbon_to_oxygen};
    /// use hyperion_sim::units::Dex;
    ///
    /// let host = HostAbundances::of(Dex::new(0.0), &AbundanceDraws::MEDIAN);
    /// let relative = host.carbon_to_oxygen() / solar_carbon_to_oxygen();
    /// assert!((relative - 0.460 / 0.537).abs() < 1e-12);
    /// let rich = HostAbundances::of(Dex::new(0.3), &AbundanceDraws::MEDIAN);
    /// assert!(rich.magnesium_to_silicon() < host.magnesium_to_silicon());
    /// ```
    #[must_use]
    pub fn of(fe_h: Dex, draws: &AbundanceDraws) -> Self {
        let (fit_low, fit_high) = FIT_METALLICITY_RANGE;
        let metallicity = fe_h.value().clamp(fit_low.value(), fit_high.value());
        let [constant, linear, quadratic] = CARBON_TO_OXYGEN_FIT;
        let fit = constant + linear * metallicity + quadratic * metallicity * metallicity;
        let carbon_trend = math::log10(fit / FIT_SOLAR_CARBON_TO_OXYGEN);
        let (median, sun) = FIT_MAGNESIUM_TO_SILICON;
        let (trend_low, trend_high) = MAGNESIUM_TO_SILICON_METALLICITY_RANGE;
        let magnesium_offset = math::log10(median / sun)
            + MAGNESIUM_TO_SILICON_TREND
                * fe_h.value().clamp(trend_low.value(), trend_high.value());
        let normal = |rank: UnitUniform| math::normal_quantile(rank.value());
        Self {
            log_carbon_to_oxygen: Dex::new(
                protosolar_log_ratio(Element::C, Element::O)
                    + carbon_trend
                    + CARBON_TO_OXYGEN_SCATTER.value() * normal(draws.carbon_to_oxygen),
            ),
            log_magnesium_to_silicon: Dex::new(
                protosolar_log_ratio(Element::Mg, Element::Si)
                    + magnesium_offset
                    + MAGNESIUM_TO_SILICON_SCATTER.value() * normal(draws.magnesium_to_silicon),
            ),
        }
    }

    /// The Sun's own protosolar ratios, Lodders's C/O of 0.457 and Mg/Si of 1.023, off the trends:
    /// what P14.T50.b's tests compare the measured Solar System against, where a generated host of
    /// solar metallicity is [`of`](Self::of) the trends.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn protosolar() -> Self {
        Self {
            log_carbon_to_oxygen: Dex::new(protosolar_log_ratio(Element::C, Element::O)),
            log_magnesium_to_silicon: Dex::new(protosolar_log_ratio(Element::Mg, Element::Si)),
        }
    }

    /// log₁₀ of the host's C/O by number.
    #[must_use]
    pub const fn log_carbon_to_oxygen(&self) -> Dex {
        self.log_carbon_to_oxygen
    }

    /// The host's C/O by number.
    #[must_use]
    pub fn carbon_to_oxygen(&self) -> f64 {
        math::exp10(self.log_carbon_to_oxygen.value())
    }

    /// log₁₀ of the host's Mg/Si by number.
    #[must_use]
    pub const fn log_magnesium_to_silicon(&self) -> Dex {
        self.log_magnesium_to_silicon
    }

    /// The host's Mg/Si by number.
    #[must_use]
    pub fn magnesium_to_silicon(&self) -> f64 {
        math::exp10(self.log_magnesium_to_silicon.value())
    }
}

#[cfg(test)]
mod tests;
