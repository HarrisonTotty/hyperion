//! The gas's phases: labels from a point's density and pressure (plan 07, Design note 12).
//!
//! The phases are in rough pressure balance, so a point's equilibrium temperature is `T = (P ÷ k)
//! ÷ (x n)`, with `x` the particles per hydrogen nucleus: 2.3 in ionised gas, where the electrons
//! and the helium count as well as the protons, and 1.1 in neutral gas. The labels follow from it:
//!
//! - **hot** above 10⁵ K, with `x` = 2.3: `n < P ÷ (2.3 × 10⁵ k)`;
//! - **warm** down to 5,000 K, with `x` = 1.1: `n < P ÷ (5,500 k)`;
//! - **molecular** above 100 cm⁻³;
//! - **cold** between.
//!
//! At a fixed pressure the label never goes back as the density rises: hot, warm, then cold, then
//! molecular, the warm limit always above the hot one. Above 550,000 K cm⁻³ the warm limit passes
//! 100 cm⁻³ and cold gas vanishes, which happens only at the centre of the most compact molecular
//! discs, whose mid-plane pressure reaches some 10⁶ K cm⁻³.
//!
//! With these thresholds a log-normal of `σ_ln` 2 to 2.5 around the Milky Way plane's density at
//! its pressure puts some 17% to 38% of the plane's volume in the hot phase, the brainstorm's "a
//! fifth to two fifths"; the corona itself comes out hot for every seed (Design note 12).
//!
//! Within the warm phase the neutral share of the hydrogen is the smooth ratio `n_neutral ÷
//! (n_neutral + n_warm)` at the point; hot gas is fully ionised; cold and molecular gas are
//! neutral. That rule is what the 21 cm column counts ([`neutral_share`]).

use crate::galaxy::gas::{IONISED_PARTICLES_PER_HYDROGEN, NEUTRAL_PARTICLES_PER_HYDROGEN};
use crate::units::{HydrogenPerCm3, Kelvin, KelvinPerCm3};

/// The temperature above which gas is hot, 10⁵ K (Design note 12).
pub const HOT_TEMPERATURE: Kelvin = Kelvin::new(1e5);

/// The temperature above which gas that is not hot is warm, 5,000 K (Design note 12).
pub const WARM_TEMPERATURE: Kelvin = Kelvin::new(5_000.0);

/// The density above which gas that is neither hot nor warm is molecular, 100 cm⁻³ (Design note
/// 12).
pub const MOLECULAR_DENSITY: HydrogenPerCm3 = HydrogenPerCm3::new(100.0);

/// The phase of the gas at a point, from its density and pressure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GasPhase {
    /// Above 10⁵ K: the corona and the rarefied gas the log-normal's low tail makes, fully ionised.
    Hot,
    /// 5,000–10⁵ K: the warm neutral and warm ionised media.
    Warm,
    /// Below 5,000 K and under 100 cm⁻³: the cold neutral medium.
    Cold,
    /// Over 100 cm⁻³ and not warm: molecular gas.
    Molecular,
}

impl GasPhase {
    /// The phase of gas of density `n` at pressure `p_over_k` (module documentation).
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::galaxy::gas::phase::GasPhase;
    /// use hyperion_sim::units::{HydrogenPerCm3, KelvinPerCm3};
    ///
    /// let plane = KelvinPerCm3::new(3_800.0);
    /// // The corona's density at the plane's pressure is far above 10⁵ K.
    /// assert_eq!(GasPhase::of(HydrogenPerCm3::new(1e-3), plane), GasPhase::Hot);
    /// // The mean neutral disc is warm, and a clump of it ten times denser is cold.
    /// assert_eq!(GasPhase::of(HydrogenPerCm3::new(0.3), plane), GasPhase::Warm);
    /// assert_eq!(GasPhase::of(HydrogenPerCm3::new(3.0), plane), GasPhase::Cold);
    /// assert_eq!(GasPhase::of(HydrogenPerCm3::new(300.0), plane), GasPhase::Molecular);
    /// ```
    #[must_use]
    pub fn of(n: HydrogenPerCm3, p_over_k: KelvinPerCm3) -> Self {
        let (n, p) = (n.value(), p_over_k.value());
        if n * IONISED_PARTICLES_PER_HYDROGEN * HOT_TEMPERATURE.value() < p {
            Self::Hot
        } else if n * NEUTRAL_PARTICLES_PER_HYDROGEN * WARM_TEMPERATURE.value() < p {
            Self::Warm
        } else if n > MOLECULAR_DENSITY.value() {
            Self::Molecular
        } else {
            Self::Cold
        }
    }

    /// The particles per hydrogen nucleus the phase's temperature is taken with: 2.3 when hot,
    /// since hot gas is ionised, and 1.1 otherwise, as its label was (Design note 12).
    #[must_use]
    pub fn particles_per_hydrogen(self) -> f64 {
        match self {
            Self::Hot => IONISED_PARTICLES_PER_HYDROGEN,
            Self::Warm | Self::Cold | Self::Molecular => NEUTRAL_PARTICLES_PER_HYDROGEN,
        }
    }

    /// The equilibrium temperature `(P ÷ k) ÷ (x n)` of gas of this phase, density `n` and pressure
    /// `p_over_k`, with the phase's own `x`: above 10⁵ K when hot, 5,000–10⁵ K when warm (above
    /// 5,000 K at least, for gas that is warm by the ionised count but not hot), and below 5,000 K
    /// when cold.
    #[must_use]
    pub fn temperature(self, n: HydrogenPerCm3, p_over_k: KelvinPerCm3) -> Kelvin {
        Kelvin::new(p_over_k.value() / (self.particles_per_hydrogen() * n.value()))
    }
}

/// The neutral share of the hydrogen at a point of phase `phase`, where the smooth neutral layer's
/// density is `neutral` and the warm ionised layer's `warm` (cm⁻³): 0 when hot, `neutral ÷
/// (neutral + warm)` when warm, 1 when cold or molecular (Design note 12).
///
/// A warm point where neither layer has any gas left, far outside the disc, is taken as ionised.
#[must_use]
pub fn neutral_share(phase: GasPhase, neutral: f64, warm: f64) -> f64 {
    match phase {
        GasPhase::Hot => 0.0,
        GasPhase::Warm => {
            let both = neutral + warm;
            if both > 0.0 { neutral / both } else { 0.0 }
        }
        GasPhase::Cold | GasPhase::Molecular => 1.0,
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::stats::normal_cdf;

    use super::*;
    use crate::Seed;
    use crate::galaxy::gas::params::GasParams;
    use crate::galaxy::gas::pressure::Pressure;
    use crate::galaxy::gas::smooth::SmoothGas;
    use crate::galaxy::imf::MassFunctionKind;
    use crate::galaxy::params::GalaxyParams;
    use crate::math;

    /// The thresholds fall where Design note 12 puts them, written out here as numbers: at
    /// 3,800 K cm⁻³ gas is hot below 3,800 ÷ (2.3 × 10⁵) = 0.016522 cm⁻³, warm below
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

    /// A phase's temperature agrees with its label.
    #[test]
    fn a_phases_temperature_agrees_with_its_label() {
        let p = KelvinPerCm3::new(3_800.0);
        for j in 0..=1_200 {
            let n = HydrogenPerCm3::new(math::exp10(-6.0 + 0.01 * f64::from(j)));
            let phase = GasPhase::of(n, p);
            let t = phase.temperature(n, p).value();
            match phase {
                GasPhase::Hot => assert!(t > HOT_TEMPERATURE.value()),
                GasPhase::Warm => assert!(t > WARM_TEMPERATURE.value()),
                GasPhase::Cold | GasPhase::Molecular => {
                    assert!(t <= WARM_TEMPERATURE.value(), "{t} K is {phase:?}");
                }
            }
        }
    }

    /// The share of the hydrogen a 21 cm column counts: none of the hot gas, all of the cold, and
    /// the neutral layer's share of the warm.
    #[test]
    fn the_neutral_share_follows_the_phase() {
        assert!(neutral_share(GasPhase::Hot, 0.8, 0.03).abs() < f64::MIN_POSITIVE);
        assert!((neutral_share(GasPhase::Cold, 0.8, 0.03) - 1.0).abs() < f64::EPSILON);
        assert!((neutral_share(GasPhase::Molecular, 0.0, 0.03) - 1.0).abs() < f64::EPSILON);
        assert!((neutral_share(GasPhase::Warm, 0.8, 0.2) - 0.8).abs() < 1e-15);
        assert!(neutral_share(GasPhase::Warm, 0.0, 0.0).abs() < f64::MIN_POSITIVE);
    }

    /// The analytic hot filling factor at `σ_ln`: the chance that `n_disc F + n_cor` is below the
    /// hot limit `P ÷ (2.3 × 10⁵ k)` when `ln F` is normal with mean `−σ² ÷ 2` and variance `σ²`.
    fn hot_filling(disc: f64, corona: f64, p: f64, sigma: f64) -> f64 {
        let limit = p / (IONISED_PARTICLES_PER_HYDROGEN * HOT_TEMPERATURE.value()) - corona;
        if limit <= 0.0 {
            return 0.0;
        }
        normal_cdf((math::ln(limit / disc) + 0.5 * sigma * sigma) / sigma)
    }

    /// The hot phase fills a fifth to two fifths of the Milky Way plane's volume at the Sun's
    /// radius: analytically, from the fixture's azimuthally averaged disc density and pressure there,
    /// 0.17–0.41 across `σ_ln` of 2.0–2.5, rising with `σ_ln` (Design note 12; P07.T12 measures it
    /// by Monte Carlo at the fixture's own `σ_ln`).
    #[test]
    fn the_hot_phase_fills_a_fifth_to_two_fifths_of_the_plane() {
        let params = GasParams::milky_way_like();
        let (gas, pressure) = (SmoothGas::new(&params), Pressure::new(&params));
        let r = 26_000.0;
        let (disc, corona) = (gas.plane_disc_mean(r), gas.corona_density());
        let p = pressure.at(&gas, r, 0.0);
        let mut previous = 0.0;
        for i in 0..=10 {
            let sigma = 2.0 + 0.05 * f64::from(i);
            let hot = hot_filling(disc, corona, p, sigma);
            assert!(
                (0.17..=0.41).contains(&hot),
                "σ {sigma}: a hot share of {hot}"
            );
            assert!(hot > previous, "σ {sigma}: {hot} is not above {previous}");
            previous = hot;
        }
    }

    /// Far above the disc, 20,000 ly up at the Sun's radius, the mean gas is hot for every seed of
    /// 2,000 drawn against the fixture's galaxy and 32 against their own: the corona and what is left
    /// of the warm layer, at what is left of the pressure.
    ///
    /// The margin is thin. At 26,000 ly the warm layer's density is its drawn one whatever the
    /// galaxy, and the coolest of the 2,000 seeds is 100,870 K; the corner of the drawn ranges — a
    /// floor of 300 K cm⁻³, a corona of 1.2 × 10⁻³ cm⁻³ and a warm layer of 0.035 cm⁻³ and 3,500
    /// ly, whose tail 20,000 ly up is a tenth of the corona — gives 99,160 K, and above the inner
    /// disc, where the warm layer is denser, one or two seeds in 2,000 fall to 92,400 K (plan 07,
    /// Risks, for P07.T12's tuning of the corona's range).
    #[test]
    fn far_above_the_disc_the_gas_is_hot_for_every_seed() {
        let check = |params: &GasParams| {
            let (gas, pressure) = (SmoothGas::new(params), Pressure::new(params));
            let r = 26_000.0;
            let n = HydrogenPerCm3::new(gas.mean_density(r, 20_000.0));
            let p = KelvinPerCm3::new(pressure.at(&gas, r, 20_000.0));
            assert_eq!(GasPhase::of(n, p), GasPhase::Hot, "{n:?} at {p:?}");
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
}
