//! The gas's phases: four media in pressure balance inside every parcel, and the labels a density
//! and a pressure give (plan 07, Design note 12, as ruling 103 of 2026-09-22 rebuilds it).
//!
//! # Why a parcel is split
//!
//! The field's density is a smooth layer times a log-normal factor on a lattice of 64 ly and up,
//! at a smooth pressure. A parcel whose mean density lies below what pressure-balanced warm gas
//! needs cannot be one warm phase at 6,000–10,000 K: at the Milky Way plane's 4,144 K cm⁻³ that
//! needs 0.47 cm⁻³ of warm neutral gas or 0.25 of warm ionised gas. Read as one phase, most warm
//! parcels came out between 10⁴ and 10⁵ K, or below 5,000 K, and ruling 98's clamp bound on 86–92%
//! of them. Such a parcel is instead mostly hot by volume, with its mass in a few warm clumps; a
//! parcel denser than that condenses part of its neutral gas into cold clouds. So each parcel keeps
//! its masses and splits its volume between four phases, each at the parcel's pressure and at its
//! own measured temperature ([`PhaseMix`]; research `gas2phase`, NOTES §1):
//!
//! | Phase | Medium | T | Particles per H | Source |
//! |---|---|---|---|---|
//! | hot | [`Medium::Hot`] | `P f_H ÷ (2.3 n_cor)`, at least 10⁵ K | 2.3 | Ferrière 2001, Rev. Mod. Phys. 73, 1031, Table I and §II.C: "≃ 2.3 n k T", about 10⁶ K |
//! | warm ionised | [`Medium::WarmIonised`] | 8,000 K | 2.1 | Gaensler et al. 2008, PASA 25, 184: "Te ≈ 8000 K"; Ferrière 2001 §II.C: "≃ 2.1 n k T" |
//! | warm neutral | [`Medium::WarmNeutral`] | 8,000 K | 1.1 | Wolfire et al. 2003, ApJ 587, 278, §6.4: "TWNM ≃ 8000 K" |
//! | cold | [`Medium::Cold`] | 70 K | 1.1 | Heiles and Troland 2003, ApJ 586, 1067: the column-weighted median, 70 K |
//!
//! The parcel's hydrogen is assigned by where it lives, never by a temperature: the neutral and
//! molecular layers' `(n̄_n + n̄_mol) δ` stays neutral, the warm ionised layer's `n̄_w δ` ionised,
//! and the corona's `n_cor` hot, so the smooth layers' neutral share is kept exactly and hot gas
//! fills the voids as Design note 10 says. **δ sets masses and filling factors, never a
//! temperature.**
//!
//! # The closure
//!
//! With `n_k = P ÷ (x_k k T_k)` each phase's density in balance and `V_k = M_k ÷ n_k` the volume
//! its mass would fill at it:
//!
//! 1. the corona keeps `f_Hmin = 2.3 n_cor × 10⁵ K ÷ P`, the volume that holds it at 10⁵ K, so
//!    hot gas is never cooler than that;
//! 2. the warm phases take their volumes `V_I + V_W` of the rest, the room `1 − f_Hmin`, and the
//!    hot gas the remainder;
//! 3. if they overfill it, the share `μ = (V_I + V_W − room) ÷ (V_W − V_C)` of the neutral mass
//!    condenses to the cold phase, which fits it exactly: Krolik, McKee and Tarter's lever rule, in
//!    Wolfire et al.'s eqs. 30–31;
//! 4. if even an all-cold neutral mass overfills it (`V_I + V_C > room`), every phase but the hot
//!    one is compressed by `s = (V_I + V_C) ÷ room` beyond `P`, which the parcel reports as its
//!    [`overpressure`](PhaseMix::overpressure) (H II-region-like gas or bound clouds; 1.3% of the
//!    plane's volume at the fixture's `σ_ln`).
//!
//! The branches are decided by comparisons of the volumes, never by testing `Σ f_k > 1` after the
//! lever rule, where rounding alone would mark a parcel compressed. The phases then satisfy `T_k ×
//! x_k × n_k = s P` (the hot phase `P`), their filling factors sum to 1, and `Σ f_k n_k` is the
//! parcel's mean density, so a point's drawn phase averages to it. Wolfire et al.'s two-phase
//! pressure range (their eq. 43) is not enforced (plan 07, Risks).
//!
//! # Labels
//!
//! [`GasPhase::of`] still labels an in-situ state from its density and pressure through `T = (P ÷
//! k) ÷ (x n)`, each boundary with the particle count of the phase it admits:
//!
//! - **hot** at or above 10⁵ K, fully ionised: `n ≤ P ÷ (2.3 × 10⁵ k)`;
//! - **cold** below 5,000 K, neutral: `n ≥ P ÷ (1.1 × 5,000 k)`, and **molecular** if also above
//!   100 cm⁻³;
//! - **warm** between.
//!
//! It agrees with the label of every uncompressed phase of a split, the hot boundary being
//! inclusive so that a corona at exactly `f_Hmin` is hot. At a fixed pressure the label never goes
//! back as the density rises. Warm ionised and warm neutral gas are both [`GasPhase::Warm`], told
//! apart by their [`Medium`] and its neutral share of 0 or 1.

use crate::galaxy::gas::{
    IONISED_PARTICLES_PER_HYDROGEN, MASS_PER_HYDROGEN_FACTOR, NEUTRAL_PARTICLES_PER_HYDROGEN,
};
use crate::units::consts::{BOLTZMANN_CONSTANT, HYDROGEN_MASS_KG};
use crate::units::{HydrogenPerCm3, Kelvin, KelvinPerCm3, MetresPerSecond};

/// The temperature at and above which gas is hot, 10⁵ K (Design note 12), and the least the hot
/// phase of a split reads.
pub const HOT_TEMPERATURE: Kelvin = Kelvin::new(1e5);

/// The temperature above which gas that is not hot is labelled warm, 5,000 K (Design note 12).
pub const WARM_TEMPERATURE: Kelvin = Kelvin::new(5_000.0);

/// The warm ionised medium's temperature, 8,000 K: Gaensler et al.'s (2008, PASA 25, 184) "Te ≈
/// 8000 K", inside Haffner et al.'s (2009, Rev. Mod. Phys. 81, 969) 6,000–10,000 K (ruling 103).
/// Its rise with height, which no source quantifies, is not modelled.
pub const WARM_IONISED_TEMPERATURE: Kelvin = Kelvin::new(8_000.0);

/// The warm neutral medium's temperature, 8,000 K: Wolfire et al.'s (2003, ApJ 587, 278, §6.4)
/// "TWNM ≃ 8000 K", whose Table 3 gives 5,040–8,310 K at 8.5 kpc (ruling 103). A `T(P)` along
/// their warm branch is a later refinement (plan 07, Risks).
pub const WARM_NEUTRAL_TEMPERATURE: Kelvin = Kelvin::new(8_000.0);

/// The cold neutral medium's temperature, 70 K: Heiles and Troland's (2003, ApJ 586, 1067) median
/// weighted by column density; Wolfire et al.'s Table 3 averages 85 K (ruling 103).
pub const COLD_TEMPERATURE: Kelvin = Kelvin::new(70.0);

/// Particles per hydrogen nucleus in the warm ionised medium, 2.1: the hydrogen ionised, helium at
/// a tenth by number and at most 60% singly ionised (Ferrière 2001, Rev. Mod. Phys. 73, 1031,
/// §II.C: "≃ 2.1 n k T"; Haffner et al. 2009's He⁺ ÷ He ≲ 60% gives 2.09–2.14; ruling 103). The
/// hot phase keeps [`IONISED_PARTICLES_PER_HYDROGEN`].
pub const WARM_IONISED_PARTICLES_PER_HYDROGEN: f64 = 2.1;

/// The density above which gas that is neither hot nor warm is molecular, 100 cm⁻³ (Design note
/// 12).
pub const MOLECULAR_DENSITY: HydrogenPerCm3 = HydrogenPerCm3::new(100.0);

/// The phase of the gas at a point, from its density and pressure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GasPhase {
    /// At or above 10⁵ K: the corona and the hot gas that fills a parcel's voids, fully ionised.
    Hot,
    /// 5,000–10⁵ K: the warm neutral and warm ionised media.
    Warm,
    /// Below 5,000 K and under 100 cm⁻³: the cold neutral medium.
    Cold,
    /// Over 100 cm⁻³ and not warm: molecular gas.
    Molecular,
}

impl GasPhase {
    /// The phase of gas of in-situ density `n` at pressure `p_over_k` (module documentation).
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::galaxy::gas::phase::GasPhase;
    /// use hyperion_sim::units::{HydrogenPerCm3, KelvinPerCm3};
    ///
    /// let plane = KelvinPerCm3::new(3_800.0);
    /// let phase = |n| GasPhase::of(HydrogenPerCm3::new(n), plane);
    /// // The corona's density at the plane's pressure is far above 10⁵ K.
    /// assert_eq!(phase(1e-3), GasPhase::Hot);
    /// // Warm neutral gas in balance at 8,000 K, and cold gas at 70 K.
    /// assert_eq!(phase(3_800.0 / (1.1 * 8_000.0)), GasPhase::Warm);
    /// assert_eq!(phase(3_800.0 / (1.1 * 70.0)), GasPhase::Cold);
    /// assert_eq!(phase(300.0), GasPhase::Molecular);
    /// ```
    #[must_use]
    pub fn of(n: HydrogenPerCm3, p_over_k: KelvinPerCm3) -> Self {
        let (n, p) = (n.value(), p_over_k.value());
        if n * IONISED_PARTICLES_PER_HYDROGEN * HOT_TEMPERATURE.value() <= p {
            Self::Hot
        } else if n * NEUTRAL_PARTICLES_PER_HYDROGEN * WARM_TEMPERATURE.value() < p {
            Self::Warm
        } else if n > MOLECULAR_DENSITY.value() {
            Self::Molecular
        } else {
            Self::Cold
        }
    }
}

/// One of the four media a parcel's volume is split between (ruling 103 of 2026-09-22), in the
/// order a point's phase is drawn in: McKee and Ostriker's (1977, ApJ 218, 148) clouds, cold cores
/// inside warm neutral layers inside warm ionised ones, all embedded in the hot medium.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Medium {
    /// The hot, fully ionised gas that fills the voids: the corona's mass.
    Hot,
    /// The warm ionised medium at 8,000 K: the warm layer's mass.
    WarmIonised,
    /// The warm neutral medium at 8,000 K: the neutral mass that has not condensed.
    WarmNeutral,
    /// The cold neutral medium at 70 K, molecular where it is denser than 100 cm⁻³.
    Cold,
}

impl Medium {
    /// The four, in the draw's order: hot, warm ionised, warm neutral, cold.
    pub const ALL: [Self; 4] = [Self::Hot, Self::WarmIonised, Self::WarmNeutral, Self::Cold];

    /// The medium's place in [`ALL`](Self::ALL).
    const fn index(self) -> usize {
        match self {
            Self::Hot => 0,
            Self::WarmIonised => 1,
            Self::WarmNeutral => 2,
            Self::Cold => 3,
        }
    }

    /// The particles per hydrogen nucleus `x`: 2.3 hot, 2.1 warm ionised, 1.1 warm neutral and
    /// cold.
    #[must_use]
    pub const fn particles_per_hydrogen(self) -> f64 {
        match self {
            Self::Hot => IONISED_PARTICLES_PER_HYDROGEN,
            Self::WarmIonised => WARM_IONISED_PARTICLES_PER_HYDROGEN,
            Self::WarmNeutral | Self::Cold => NEUTRAL_PARTICLES_PER_HYDROGEN,
        }
    }

    /// The share of the medium's hydrogen that is neutral, which the 21 cm column counts: 0 for
    /// the hot and the warm ionised media, 1 for the warm neutral and the cold.
    #[must_use]
    pub const fn neutral_share(self) -> f64 {
        match self {
            Self::Hot | Self::WarmIonised => 0.0,
            Self::WarmNeutral | Self::Cold => 1.0,
        }
    }
}

/// One parcel's gas split into four phases in pressure balance (module documentation; ruling 103
/// of 2026-09-22): each phase's volume filling factor, in-situ density and temperature, and how far
/// the parcel is compressed beyond its pressure.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::gas::phase::{GasPhase, Medium, PhaseMix};
/// use hyperion_sim::units::{HydrogenPerCm3, KelvinPerCm3};
///
/// let n = HydrogenPerCm3::new;
/// let p = KelvinPerCm3::new(4_144.0);
/// // A thin parcel of the Sun's plane, a fifth of the mean: mostly hot by volume.
/// let thin = PhaseMix::split(n(0.08), n(0.006), n(6e-4), p);
/// assert!(thin.filling(Medium::Hot) > 0.7);
/// assert!(thin.condensed_share().abs() < f64::MIN_POSITIVE);
/// // Four times the mean: warm gas overfills it, and part of the neutral mass condenses.
/// let dense = PhaseMix::split(n(1.6), n(0.12), n(6e-4), p);
/// assert!(dense.condensed_share() > 0.0 && dense.condensed_share() < 1.0);
/// assert_eq!(dense.phase(Medium::Cold), GasPhase::Cold);
/// // Every phase is at the parcel's pressure, and the phases hold its mass.
/// for medium in Medium::ALL {
///     let balance = dense.temperature(medium).value()
///         * medium.particles_per_hydrogen()
///         * dense.density(medium).value();
///     assert!((balance / 4_144.0 - 1.0).abs() < 1e-12);
/// }
/// let mass: f64 = Medium::ALL.iter().map(|&m| dense.mass(m).value()).sum();
/// assert!((mass / (1.6 + 0.12 + 6e-4) - 1.0).abs() < 1e-12);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhaseMix {
    /// The parcel's pressure `P ÷ k`, K cm⁻³.
    pressure: f64,
    /// Each medium's volume filling factor, in [`Medium::ALL`]'s order.
    filling: [f64; 4],
    /// Each medium's in-situ density, cm⁻³.
    density: [f64; 4],
    /// Each medium's temperature, K.
    temperature: [f64; 4],
    /// The compression `s ≥ 1` beyond the parcel's pressure of every phase but the hot one.
    compression: f64,
    /// The share `μ` of the neutral mass condensed to the cold phase.
    condensed: f64,
    /// The neutral mass's share of the parcel's hydrogen.
    neutral_mass_share: f64,
}

impl PhaseMix {
    /// The split of a parcel whose neutral and molecular gas has the mean density `neutral`, whose
    /// warm ionised gas has `ionised` and whose corona has `corona`, at pressure `p_over_k`
    /// (module documentation).
    ///
    /// `neutral` and `ionised` are the smooth layers times the parcel's log-normal factor, so the
    /// factor moves the filling factors and never a temperature. A parcel with no corona has an
    /// empty hot phase or one at infinite temperature. A hand-built parcel whose pressure cannot
    /// hold its corona at 10⁵ K even filling the whole volume (`P ≤ 2.3 n_cor × 10⁵ K`, under
    /// 184 K cm⁻³ for the densest drawn corona and below every drawn floor) reserves the corona
    /// half the volume instead, so its hot gas reads under 10⁵ K.
    #[must_use]
    pub fn split(
        neutral: HydrogenPerCm3,
        ionised: HydrogenPerCm3,
        corona: HydrogenPerCm3,
        p_over_k: KelvinPerCm3,
    ) -> Self {
        let (neutral, ionised, corona, p) = (
            neutral.value(),
            ionised.value(),
            corona.value(),
            p_over_k.value(),
        );
        debug_assert!(p > 0.0, "a split needs a positive pressure, not {p}");
        let balance = |medium: Medium, t: Kelvin| p / (medium.particles_per_hydrogen() * t.value());
        let n_i = balance(Medium::WarmIonised, WARM_IONISED_TEMPERATURE);
        let n_w = balance(Medium::WarmNeutral, WARM_NEUTRAL_TEMPERATURE);
        let n_c = balance(Medium::Cold, COLD_TEMPERATURE);
        let reserved = IONISED_PARTICLES_PER_HYDROGEN * corona * HOT_TEMPERATURE.value() / p;
        // A pressure too low to hold the corona at 10⁵ K anywhere still leaves it a volume of its
        // own, so that its mass is never dropped.
        let (least_hot, room) = if reserved < 1.0 {
            (reserved, 1.0 - reserved)
        } else {
            (0.5, 0.5)
        };
        let (v_i, v_w, v_c) = (ionised / n_i, neutral / n_w, neutral / n_c);
        let warm = v_i + v_w;
        // Decided on the volumes, so that the lever rule's `used = room`, which holds only to
        // rounding, never marks a parcel compressed.
        let (condensed, compression, hot) = if warm <= room {
            (0.0, 1.0, 1.0 - warm)
        } else if v_i + v_c < room {
            ((warm - room) / (v_w - v_c), 1.0, least_hot)
        } else {
            (1.0, (v_i + v_c) / room, least_hot)
        };
        let s = compression;
        let filling = [
            hot,
            v_i / s,
            (1.0 - condensed) * v_w / s,
            condensed * v_c / s,
        ];
        let hot_density = if hot > 0.0 { corona / hot } else { 0.0 };
        let density = [hot_density, s * n_i, s * n_w, s * n_c];
        let temperature = [
            p / (IONISED_PARTICLES_PER_HYDROGEN * hot_density),
            WARM_IONISED_TEMPERATURE.value(),
            WARM_NEUTRAL_TEMPERATURE.value(),
            COLD_TEMPERATURE.value(),
        ];
        let total = neutral + ionised + corona;
        Self {
            pressure: p,
            filling,
            density,
            temperature,
            compression,
            condensed,
            neutral_mass_share: if total > 0.0 { neutral / total } else { 0.0 },
        }
    }

    /// The parcel's pressure `P ÷ k`: the hot phase's, and the others' where the parcel is not
    /// compressed.
    #[must_use]
    pub fn pressure(&self) -> KelvinPerCm3 {
        KelvinPerCm3::new(self.pressure)
    }

    /// The share of the parcel's volume `medium` fills, in [0, 1]; the four sum to 1.
    #[must_use]
    pub fn filling(&self, medium: Medium) -> f64 {
        self.filling[medium.index()]
    }

    /// `medium`'s in-situ density: `s P ÷ (x k T)` for the warm and cold phases, `n_cor ÷ f_H`
    /// for the hot one, and 0 for a hot phase of no volume.
    #[must_use]
    pub fn density(&self, medium: Medium) -> HydrogenPerCm3 {
        HydrogenPerCm3::new(self.density[medium.index()])
    }

    /// `medium`'s temperature: 8,000 K warm, 70 K cold, and `P ÷ (2.3 k n_H)` hot, which is at
    /// least 10⁵ K to rounding (infinite for a hot phase of no gas).
    #[must_use]
    pub fn temperature(&self, medium: Medium) -> Kelvin {
        Kelvin::new(self.temperature[medium.index()])
    }

    /// `medium`'s hydrogen per unit volume of the parcel, `f × n`: its share of the parcel's mean
    /// density. The four sum to the parcel's mean density.
    #[must_use]
    pub fn mass(&self, medium: Medium) -> HydrogenPerCm3 {
        let k = medium.index();
        HydrogenPerCm3::new(self.filling[k] * self.density[k])
    }

    /// The factor `s ≥ 1` by which every phase but the hot one is compressed beyond the parcel's
    /// pressure: 1 unless even an all-cold neutral mass overfills the room the corona leaves. The
    /// compressed phases' thermal pressure is `s P`.
    #[must_use]
    pub fn overpressure(&self) -> f64 {
        self.compression
    }

    /// The share `μ` of the neutral mass condensed to the cold phase, in [0, 1] (the lever rule).
    #[must_use]
    pub fn condensed_share(&self) -> f64 {
        self.condensed
    }

    /// The share of the parcel's hydrogen that is neutral, `M_N ÷ (M_N + M_I + n_cor)`: the smooth
    /// layers' share with the corona counted, whatever the split. 0 for a parcel of no gas.
    #[must_use]
    pub fn neutral_mass_share(&self) -> f64 {
        self.neutral_mass_share
    }

    /// The label of `medium`'s in-situ state: hot, warm for both warm media, and cold, or
    /// molecular where the cold phase's in-situ density exceeds 100 cm⁻³ (Design note 12). It is
    /// [`GasPhase::of`] of the phase's density and pressure wherever the parcel is not compressed.
    #[must_use]
    pub fn phase(&self, medium: Medium) -> GasPhase {
        match medium {
            Medium::Hot => GasPhase::Hot,
            Medium::WarmIonised | Medium::WarmNeutral => GasPhase::Warm,
            Medium::Cold => {
                if self.density[Medium::Cold.index()] > MOLECULAR_DENSITY.value() {
                    GasPhase::Molecular
                } else {
                    GasPhase::Cold
                }
            }
        }
    }

    /// The isothermal sound speed of `medium`, `√(P_k ÷ ρ_k) = √(x k T ÷ 1.4 m_H)`, the phase's
    /// own pressure over its own density (the compression cancels): 7.20 km/s in the warm neutral
    /// medium, Wolfire et al.'s (2003) `σ_th` at 8,000 K, and 9.95 km/s in the warm ionised.
    #[must_use]
    pub fn isothermal_sound_speed(&self, medium: Medium) -> MetresPerSecond {
        MetresPerSecond::new(self.pressure_per_mass(medium).sqrt())
    }

    /// `P_k ÷ ρ_k` of `medium`, m² s⁻²: `x k T ÷ 1.4 m_H`.
    pub(crate) fn pressure_per_mass(&self, medium: Medium) -> f64 {
        medium.particles_per_hydrogen() * BOLTZMANN_CONSTANT * self.temperature(medium).value()
            / (MASS_PER_HYDROGEN_FACTOR * HYDROGEN_MASS_KG)
    }

    /// The medium a point lies in whose draw is `u` ∈ [0, 1]: the first in [`Medium::ALL`]'s order
    /// whose cumulative filling factor exceeds `u`, so that each is drawn with the chance of its
    /// filling factor, and neighbouring draws are neighbouring media (McKee and Ostriker's clouds).
    /// A medium of no volume is never drawn.
    #[must_use]
    pub fn draw(&self, u: f64) -> Medium {
        let mut cumulative = 0.0;
        let mut last = Medium::Hot;
        for medium in Medium::ALL {
            let filling = self.filling[medium.index()];
            if filling > 0.0 {
                cumulative += filling;
                last = medium;
                if u < cumulative {
                    return medium;
                }
            }
        }
        // The factors sum to 1 only to rounding, so a draw just below 1 may pass them all.
        last
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::lcg::Lcg;
    use hyperion_testkit::stats::normal_cdf;

    use super::*;
    use crate::Seed;
    use crate::galaxy::gas::params::GasParams;
    use crate::galaxy::gas::pressure::Pressure;
    use crate::galaxy::gas::smooth::{GasLayer, SmoothGas};
    use crate::galaxy::imf::MassFunctionKind;
    use crate::galaxy::params::GalaxyParams;
    use crate::math;
    use crate::rng::{ObjectKey, Stream, tags};

    /// The split of the given densities (cm⁻³) at `p` K cm⁻³.
    fn split(neutral: f64, ionised: f64, corona: f64, p: f64) -> PhaseMix {
        PhaseMix::split(
            HydrogenPerCm3::new(neutral),
            HydrogenPerCm3::new(ionised),
            HydrogenPerCm3::new(corona),
            KelvinPerCm3::new(p),
        )
    }

    /// A log-uniform draw on `lo`–`hi`.
    fn log_uniform(lcg: &mut Lcg, lo: f64, hi: f64) -> f64 {
        lo * math::exp(math::ln(hi / lo) * lcg.next_f64())
    }

    /// The thresholds fall where Design note 12 puts them, written out here as numbers: at
    /// 3,800 K cm⁻³ gas is hot up to 3,800 ÷ (2.3 × 10⁵) = 0.016522 cm⁻³, warm below
    /// 3,800 ÷ (1.1 × 5,000) = 0.69091 cm⁻³ and cold above it, and molecular above 100 cm⁻³.
    /// Swapping the two particle counts, or either temperature, moves a boundary and fails here.
    #[test]
    fn the_phase_boundaries_lie_where_design_note_12_puts_them() {
        let p = KelvinPerCm3::new(3_800.0);
        let phase = |n: f64| GasPhase::of(HydrogenPerCm3::new(n), p);
        let (hot, warm): (f64, f64) = (3_800.0 / 2.3e5, 3_800.0 / 5_500.0);
        assert!((hot - 0.016_522).abs() < 1e-6 && (warm - 0.690_91).abs() < 1e-5);
        for (edge, below, above) in [
            (hot, GasPhase::Hot, GasPhase::Warm),
            (warm, GasPhase::Warm, GasPhase::Cold),
            (100.0, GasPhase::Cold, GasPhase::Molecular),
        ] {
            assert_eq!(phase(edge * (1.0 - 1e-9)), below, "below {edge}");
            assert_eq!(phase(edge * (1.0 + 1e-9)), above, "above {edge}");
        }
        // The hot boundary is inclusive: 10⁵ K exactly is hot.
        assert_eq!(
            GasPhase::of(HydrogenPerCm3::new(1.0), KelvinPerCm3::new(2.3e5)),
            GasPhase::Hot
        );
    }

    /// At every pressure from 1 to 10⁸ K cm⁻³ the phase never goes back as the density rises:
    /// hot, warm, cold, molecular, in that order, each possibly absent.
    #[test]
    fn the_phases_are_ordered_at_every_pressure() {
        for i in 0..=80 {
            let p = KelvinPerCm3::new(math::exp10(0.1 * f64::from(i)));
            let mut previous = GasPhase::Hot;
            for j in 0..=1_400 {
                let n = HydrogenPerCm3::new(math::exp10(-8.0 + 0.01 * f64::from(j)));
                let phase = GasPhase::of(n, p);
                assert!(
                    phase >= previous,
                    "{phase:?} after {previous:?} at {n:?}, {p:?}"
                );
                previous = phase;
            }
            // The hot limit is always below the warm one.
            let hot = p.value() / (IONISED_PARTICLES_PER_HYDROGEN * HOT_TEMPERATURE.value());
            let warm = p.value() / (NEUTRAL_PARTICLES_PER_HYDROGEN * WARM_TEMPERATURE.value());
            assert!(hot < warm);
        }
    }

    /// Ruling 103's invariants over 10⁴ random parcels — neutral and warm layers, a log-normal
    /// factor of σ 1.4, a pressure of 250–10⁶ K cm⁻³ and a corona of 0.5–0.8 × 10⁻³ cm⁻³ or none:
    /// the filling factors lie in [0, 1] and sum to 1, and the phases hold the parcel's mean density,
    /// each to 10⁻¹²; the neutral mass and the ionised mass with the corona are each kept; the hot
    /// phase fills at least the corona's reserve and is at least 10⁵ K wherever it has volume;
    /// `T x n = P` for every phase of an uncompressed parcel to 10⁻¹³ and `s P` for the compressed
    /// warm and cold phases; and every uncompressed phase's label is [`GasPhase::of`] of its state.
    #[test]
    fn a_split_keeps_the_mass_and_the_pressure() {
        let mut lcg = Lcg::new(0x0703_5911);
        let (mut compressed, mut condensed, mut all_warm) = (0_u32, 0_u32, 0_u32);
        for i in 0..10_000 {
            let delta = math::exp(1.4 * (lcg.next_f64() * 8.0 - 4.0) - 0.98);
            let neutral = log_uniform(&mut lcg, 1e-6, 10.0) * delta;
            let ionised = log_uniform(&mut lcg, 1e-5, 0.1) * delta;
            let corona = if i % 10 == 0 {
                0.0
            } else {
                log_uniform(&mut lcg, 0.5e-3, 0.8e-3)
            };
            let p = log_uniform(&mut lcg, 250.0, 1e6);
            let mix = split(neutral, ionised, corona, p);
            let what = format!("{neutral}, {ionised}, {corona} at {p}: {mix:?}");
            let fillings = Medium::ALL.map(|m| mix.filling(m));
            assert!(fillings.iter().all(|f| (0.0..=1.0).contains(f)), "{what}");
            assert!((fillings.iter().sum::<f64>() - 1.0).abs() < 1e-12, "{what}");
            let total = neutral + ionised + corona;
            let held: f64 = Medium::ALL.iter().map(|&m| mix.mass(m).value()).sum();
            assert!((held / total - 1.0).abs() < 1e-12, "{what}");
            let neutral_held =
                mix.mass(Medium::WarmNeutral).value() + mix.mass(Medium::Cold).value();
            assert!((neutral_held - neutral).abs() <= 1e-12 * neutral, "{what}");
            let ionised_held =
                mix.mass(Medium::Hot).value() + mix.mass(Medium::WarmIonised).value();
            assert!(
                (ionised_held - ionised - corona).abs() <= 1e-12 * (ionised + corona),
                "{what}"
            );
            let reserve = 2.3 * corona * 1e5 / p;
            assert!(
                mix.filling(Medium::Hot) >= reserve * (1.0 - 1e-12),
                "{what}"
            );
            if mix.filling(Medium::Hot) > 0.0 && corona > 0.0 {
                let t = mix.temperature(Medium::Hot).value();
                assert!(t >= 1e5 * (1.0 - 1e-12), "{t} K hot: {what}");
            }
            let s = mix.overpressure();
            assert!(s >= 1.0, "{what}");
            for medium in Medium::ALL {
                if mix.filling(medium) <= 0.0 || (medium == Medium::Hot && corona <= 0.0) {
                    continue;
                }
                let n = mix.density(medium).value();
                let balance =
                    mix.temperature(medium).value() * medium.particles_per_hydrogen() * n / p;
                let expected = if medium == Medium::Hot { 1.0 } else { s };
                assert!(
                    (balance / expected - 1.0).abs() < 1e-13,
                    "{medium:?}: T x n ÷ P = {balance}, {what}"
                );
                if s <= 1.0 {
                    // The hot phase at exactly its reserve sits on the inclusive boundary, which
                    // rounding may put an ulp either side of; a part in 10¹² inside it is hot.
                    let n = if medium == Medium::Hot {
                        n * (1.0 - 1e-12)
                    } else {
                        n
                    };
                    assert_eq!(
                        GasPhase::of(HydrogenPerCm3::new(n), KelvinPerCm3::new(p)),
                        mix.phase(medium),
                        "{medium:?}: {what}"
                    );
                }
            }
            compressed += u32::from(s > 1.0);
            condensed += u32::from(mix.condensed_share() > 0.0 && s <= 1.0);
            all_warm += u32::from(mix.condensed_share() <= 0.0);
        }
        // The draws reach every branch of the closure.
        assert!(
            compressed > 100 && condensed > 100 && all_warm > 100,
            "{compressed} compressed, {condensed} condensed, {all_warm} all warm"
        );
    }

    /// A hand-built parcel whose pressure cannot hold its corona at 10⁵ K even filling the whole
    /// volume, `P ≤ 2.3 n_cor × 10⁵ K`, reserves the corona half the volume: its phases still fill
    /// the volume and hold the parcel's mass, with nothing undefined, and its hot gas reads under
    /// 10⁵ K.
    #[test]
    fn a_split_without_room_for_the_corona_still_holds_its_mass() {
        for (neutral, ionised) in [(0.0, 0.0), (1e-4, 1e-4), (0.3, 0.03), (30.0, 3.0)] {
            let mix = split(neutral, ionised, 1e-3, 150.0);
            let fillings = Medium::ALL.map(|m| mix.filling(m));
            assert!(fillings.iter().all(|f| (0.0..=1.0).contains(f)), "{mix:?}");
            assert!(
                (fillings.iter().sum::<f64>() - 1.0).abs() < 1e-12,
                "{mix:?}"
            );
            let held: f64 = Medium::ALL.iter().map(|&m| mix.mass(m).value()).sum();
            let total = neutral + ionised + 1e-3;
            assert!((held / total - 1.0).abs() < 1e-12, "{mix:?}");
            for medium in Medium::ALL {
                assert!(!mix.temperature(medium).value().is_nan(), "{mix:?}");
                assert!(!mix.density(medium).value().is_nan(), "{mix:?}");
            }
        }
        let empty = split(0.0, 0.0, 1e-3, 150.0);
        assert!(empty.temperature(Medium::Hot).value() < HOT_TEMPERATURE.value());
    }

    /// As the log-normal factor rises at a fixed site the hot phase never grows and the condensed
    /// share never falls, from a parcel almost all hot to one compressed beyond its pressure.
    #[test]
    fn denser_parcels_are_less_hot_and_more_condensed() {
        for (neutral, ionised, p) in [
            (0.35, 0.03, 4_144.0),
            (1e-3, 0.02, 1_000.0),
            (4.0, 0.05, 3e4),
        ] {
            let (mut hot, mut condensed) = (1.0_f64, 0.0_f64);
            for j in 0..=1_000 {
                let delta = math::exp10(-3.0 + 0.006 * f64::from(j));
                let mix = split(neutral * delta, ionised * delta, 6e-4, p);
                let (h, c) = (mix.filling(Medium::Hot), mix.condensed_share());
                assert!(h <= hot * (1.0 + 1e-12), "δ {delta}: hot {h} after {hot}");
                assert!(
                    c >= condensed * (1.0 - 1e-12),
                    "δ {delta}: μ {c} after {condensed}"
                );
                (hot, condensed) = (h, c);
            }
            assert!(condensed > 0.99, "μ ends at {condensed}");
        }
    }

    /// Wolfire et al.'s `σ_th`: the warm neutral medium's isothermal speed at 8,000 K is 7.20 km/s,
    /// and the warm ionised medium's, with 2.1 particles, 9.95 km/s; compression does not move it.
    #[test]
    fn the_warm_media_have_their_measured_sound_speeds() {
        for mix in [
            split(0.3, 0.03, 6e-4, 4_144.0),
            split(300.0, 3.0, 6e-4, 4_144.0),
        ] {
            let wnm = mix.isothermal_sound_speed(Medium::WarmNeutral).value();
            let wim = mix.isothermal_sound_speed(Medium::WarmIonised).value();
            assert!((wnm - 7_200.0).abs() < 20.0, "{wnm} m/s");
            assert!((wim - 9_950.0).abs() < 20.0, "{wim} m/s");
        }
        assert!(split(300.0, 3.0, 6e-4, 4_144.0).overpressure() > 1.0);
    }

    /// A draw picks each medium over its own share of [0, 1) in the order hot, warm ionised, warm
    /// neutral, cold, and never one with no volume.
    #[test]
    fn a_draw_follows_the_cumulative_filling() {
        let mix = split(1.6, 0.12, 6e-4, 4_144.0);
        let f = Medium::ALL.map(|m| mix.filling(m));
        assert!(f.iter().all(|&f| f > 0.0), "{f:?}");
        let edges = [f[0], f[0] + f[1], f[0] + f[1] + f[2]];
        assert_eq!(mix.draw(0.0), Medium::Hot);
        assert_eq!(mix.draw(edges[0] * (1.0 - 1e-12)), Medium::Hot);
        assert_eq!(mix.draw(edges[0] * (1.0 + 1e-12)), Medium::WarmIonised);
        assert_eq!(mix.draw(edges[1] * (1.0 + 1e-12)), Medium::WarmNeutral);
        assert_eq!(mix.draw(edges[2] * (1.0 + 1e-12)), Medium::Cold);
        assert_eq!(mix.draw(1.0), Medium::Cold);
        // A thin parcel has no cold phase, so the top of the range is warm neutral.
        let thin = split(0.08, 0.006, 6e-4, 4_144.0);
        assert!(thin.filling(Medium::Cold).abs() < f64::MIN_POSITIVE);
        assert_eq!(thin.draw(1.0), Medium::WarmNeutral);
    }

    /// The hot share of a parcel of mean density `δ K` of the balance densities, in closed form
    /// over `ln δ` normal with mean `−σ² ÷ 2` and variance `σ²` (ruling 103):
    /// `E[max(f_Hmin, 1 − δ K)] = f_Hmin + room Φ(d₁) − K Φ(d₂)`, with
    /// `d₁ = [ln(room ÷ K) + σ² ÷ 2] ÷ σ` and `d₂ = d₁ − σ`.
    fn hot_filling(warm: f64, neutral: f64, corona: f64, p: f64, sigma: f64) -> f64 {
        let k = warm * WARM_IONISED_PARTICLES_PER_HYDROGEN * WARM_IONISED_TEMPERATURE.value() / p
            + neutral * NEUTRAL_PARTICLES_PER_HYDROGEN * WARM_NEUTRAL_TEMPERATURE.value() / p;
        let least = IONISED_PARTICLES_PER_HYDROGEN * corona * HOT_TEMPERATURE.value() / p;
        let room = 1.0 - least;
        let d1 = (math::ln(room / k) + 0.5 * sigma * sigma) / sigma;
        least + room * normal_cdf(d1) - k * normal_cdf(d1 - sigma)
    }

    /// The hot phase fills a fifth to two fifths of the Milky Way plane's volume at the Sun's
    /// radius (ruling 103, replacing T5's single-phase test): in closed form, from the fixture's
    /// azimuthally averaged layers and pressure there, 0.20–0.40 across `σ_ln` of 1.0–1.4, rising
    /// with `σ_ln`; and at the fixture's 1.2 the closed form agrees with a Monte Carlo of the split
    /// over 10⁵ log-normal factors within four standard errors.
    #[test]
    fn the_hot_phase_fills_a_fifth_to_two_fifths_of_the_plane() {
        let params = GasParams::milky_way_like();
        let (gas, pressure) = (SmoothGas::new(&params), Pressure::new(&params));
        let r = 26_000.0;
        let warm = gas.plane_density(GasLayer::Warm, r);
        let neutral =
            gas.plane_density(GasLayer::Neutral, r) + gas.plane_density(GasLayer::Molecular, r);
        let (corona, p) = (gas.corona_density(), pressure.at(&gas, r, 0.0));
        let mut previous = 0.0;
        for i in 0..=8 {
            let sigma = 1.0 + 0.05 * f64::from(i);
            let hot = hot_filling(warm, neutral, corona, p, sigma);
            assert!(
                (0.20..=0.40).contains(&hot),
                "σ {sigma}: a hot share of {hot}"
            );
            assert!(hot > previous, "σ {sigma}: {hot} is not above {previous}");
            previous = hot;
        }
        let sigma = params.sigma_ln();
        let (mut sum, mut squares) = (0.0, 0.0);
        let points = 100_000_u32;
        for i in 0..points {
            let mut stream = Stream::open(
                Seed::new(0x0703_c105),
                tags::GAS_NOISE,
                ObjectKey::galaxy_item(u64::from(i)),
            );
            let delta = math::exp(sigma * stream.standard_normal() - 0.5 * sigma * sigma);
            let hot = split(neutral * delta, warm * delta, corona, p).filling(Medium::Hot);
            sum += hot;
            squares += hot * hot;
        }
        let n = f64::from(points);
        let mean = sum / n;
        let error = ((squares / n - mean * mean) / n).sqrt();
        let closed = hot_filling(warm, neutral, corona, p, sigma);
        assert!(
            (mean - closed).abs() < 4.0 * error,
            "Monte Carlo {mean} ± {error} against the closed form {closed}"
        );
    }

    /// The radii ruling 91 holds the corona hot at, 20,000 ly above the plane: 8,000–40,000 ly.
    fn corona_radii() -> impl Iterator<Item = f64> {
        (0..=32).map(|i| 8_000.0 + 1_000.0 * f64::from(i))
    }

    /// The split of the mean gas 20,000 ly above the plane at radius `r` of `params`.
    fn mean_gas_far_above(params: &GasParams, r: f64) -> PhaseMix {
        let (gas, pressure) = (SmoothGas::new(params), Pressure::new(params));
        let z = 20_000.0;
        split(
            gas.density(GasLayer::Neutral, r, z) + gas.density(GasLayer::Molecular, r, z),
            gas.density(GasLayer::Warm, r, z),
            gas.corona_density(),
            pressure.at(&gas, r, z),
        )
    }

    /// Rulings 91 and 103: far above the disc, 20,000 ly up at every radius from 8,000 to
    /// 40,000 ly, the mean gas is more than 95% hot by volume and its hot phase above 10⁵ K, for
    /// every seed of 2,000 drawn against the fixture's galaxy and 32 against their own — the
    /// corona and what is left of the warm layer, at what is left of the pressure.
    /// `gas_statistics.rs` holds it over 2,000 seeds' own galaxies, and
    /// `the_corona_is_hot_at_every_corner_of_the_draws` proves it.
    #[test]
    fn far_above_the_disc_the_gas_is_hot_for_every_seed() {
        let check = |params: &GasParams| {
            for r in corona_radii() {
                let mix = mean_gas_far_above(params, r);
                let (hot, t) = (
                    mix.filling(Medium::Hot),
                    mix.temperature(Medium::Hot).value(),
                );
                assert!(
                    hot > 0.95 && t > HOT_TEMPERATURE.value(),
                    "{hot} at {t} K at {r} ly"
                );
            }
        };
        let fixture = GalaxyParams::milky_way_like();
        for n in 0..2_000 {
            check(&GasParams::from_galaxy(Seed::new(0x0705_4070_0000_0000 | n), &fixture).unwrap());
        }
        for n in 0..32 {
            let seed = Seed::new(0x0705_4070_0000_0000 | n);
            let galaxy = GalaxyParams::from_seed(seed, MassFunctionKind::default());
            check(&GasParams::from_galaxy(seed, &galaxy).unwrap());
        }
    }

    /// Rulings 91 and 103: the corona is hot 20,000 ly above every radius from 8,000 to 40,000 ly
    /// for every galaxy the draws can make.
    ///
    /// The hot phase of a split is at least 10⁵ K by construction, since the corona keeps the
    /// volume that holds it there; what the draws must leave is the volume. There the neutral layer
    /// (700 ly tall) and the molecular disc have fallen by more than e⁻²⁸, to under 10⁻⁹ cm⁻³
    /// between them, which the bound adds as neutral gas, and the pressure is its floor plus a
    /// term that is not negative, so the hot share is at least `1 − (n_w × 2.1 + 10⁻⁹ × 1.1) ×
    /// 8,000 K ÷ P_cor` and its temperature `P_cor f_H ÷ (2.3 n_cor)`, with the warm layer's density
    /// `n_w = w exp(−R_m (1 ÷ R − 1 ÷ R₀) ÷ 2 − (R − R₀) ÷ R_g − z ÷ h_w)` for its drawn density `w`
    /// at `R₀`. That is monotone in each of `w`, `h_w`, `R_m` and `1 ÷ R_g`, so its least value
    /// over the draws is at a corner: the lowest floor, 300 K cm⁻³; the densest corona, 0.8 × 10⁻³
    /// cm⁻³; the densest and tallest warm layer, 0.035 cm⁻³ and 3,500 ly (the clamp of ruling 91
    /// only lowers it); the hole `R_m` at 0.8–1.2 of the bar's 10,000–18,000 ly; and the gas disc
    /// 1.5–2 times the thin disc's 7,000–11,500 ly. The least is a hot share of about 0.975 at
    /// about 159,000 K, at the shortest gas disc and the smallest hole over the inner disc.
    #[test]
    fn the_corona_is_hot_at_every_corner_of_the_draws() {
        let (floor, corona, warm, height) = (300.0, 0.8e-3, 0.035, 3_500.0);
        let reference = GasParams::REFERENCE_RADIUS.value();
        let z = 20_000.0;
        let (mut least, mut coolest) = (f64::INFINITY, f64::INFINITY);
        for hole in [0.8 * 10_000.0, 1.2 * 18_000.0] {
            for scale in [1.5 * 7_000.0, 2.0 * 11_500.0] {
                for r in corona_radii() {
                    let n_w = warm
                        * math::exp(
                            -0.5 * hole * (1.0 / r - 1.0 / reference)
                                - (r - reference) / scale
                                - z / height,
                        );
                    let mix = split(1e-9, n_w, corona, floor);
                    least = least.min(mix.filling(Medium::Hot));
                    coolest = coolest.min(mix.temperature(Medium::Hot).value());
                }
            }
        }
        assert!(least > 0.95, "a hot share of {least} at the worst corner");
        assert!(
            coolest > HOT_TEMPERATURE.value(),
            "{coolest} K at the worst corner"
        );
        assert!((0.97..0.98).contains(&least), "{least}");
        assert!((155_000.0..163_000.0).contains(&coolest), "{coolest} K");
    }
}
