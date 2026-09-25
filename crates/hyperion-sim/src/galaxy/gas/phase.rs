//! The gas's phases: labels from a point's density and pressure, and the ionisation and temperature
//! that go with them (plan 07, Design note 12, as ruling 91 of 2026-09-22 amends it).
//!
//! The phases are in rough pressure balance, so a point's equilibrium temperature is `T = (P ÷ k)
//! ÷ (x n)`, with `x` the particles per hydrogen nucleus. Fully ionised gas has 2.3, the electrons
//! and the helium counting as well as the protons; neutral gas has 1.1, the atoms and the helium's
//! tenth. Warm gas is partly ionised: where the smooth neutral layer's share of the hydrogen is
//! `f_n` it has `x = 1.1 + 1.2 (1 − f_n)`, which is 1.1 in the warm neutral medium and 2.3 in the
//! warm ionised medium, whose hydrogen is almost wholly ionised (Haffner et al. 2009, Rev. Mod.
//! Phys. 81, 969, §II.A: H⁺ ÷ H near unity, helium 9% by number and at most 60% singly ionised,
//! about 2.1 in all). Taking 1.1 for every warm point, as this module did until ruling 91, read an
//! ionised warm point up to 2.1 times too warm. The labels follow from the temperature, each with
//! the particle count of the gas it names:
//!
//! - **hot** above 10⁵ K, fully ionised: `n < P ÷ (2.3 × 10⁵ k)`;
//! - **cold** below 5,000 K, neutral: `n ≥ P ÷ (1.1 × 5,000 k)`, and **molecular** if also above
//!   100 cm⁻³;
//! - **warm** between.
//!
//! At a fixed pressure the label never goes back as the density rises: hot, warm, then cold, then
//! molecular, the warm limit always above the hot one. Above 550,000 K cm⁻³ the warm limit passes
//! 100 cm⁻³ and cold gas vanishes, which happens only at the centre of the most compact molecular
//! discs, whose mid-plane pressure reaches some 10⁶ K cm⁻³.
//!
//! **Warm gas reads 5,000–10,000 K** ([`ThermalState`], ruling 98 of 2026-09-22). A smooth
//! pressure over a decade of density noise would scatter `P ÷ (x n)` by a decade, where the warm
//! media are measured at 6,000–10,000 K (Haffner et al. 2009, §II.A; 7,000–10,000 K in the warm
//! ionised medium) and the warm neutral medium at 4,240–8,800 K (Wolfire et al. 2003, ApJ 587,
//! 278, Table 3); Jenkins and Tripp (2011, ApJ 734, 65) find the thermal pressure scattered by
//! only some 0.175 dex. So the density contrast within the phase is mostly turbulent support or
//! unresolved hot and cold gas, not temperature, and the warm temperature is clamped to 5,000–10⁴
//! K for readout. The particle count is the smooth share's `x` exactly, never adjusted, and so is
//! the neutral share; `T × x n = P` holds where the clamp does not bind, and where it does the
//! ratio `x n T ÷ P` is the part of the pressure the model does not resolve. Ruling 91 put the
//! smooth `x` into the warm/cold threshold; that labels as cold, and so as neutral with 1.1
//! particles, gas that then reads up to 10,450 K, so the thresholds keep each label's own count,
//! 2.3 for hot and 1.1 for cold, which is the count of the phase each boundary admits. A split of
//! each warm point into a warm ionised part near 8,000 K and a warm neutral part at Wolfire et
//! al.'s `T(P)`, each in pressure balance, would let the noise set filling factors instead; it
//! moves labels, and is a later refinement (plan 07, Risks).
//!
//! With these thresholds a log-normal of `σ_ln` 2 to 2.5 around the Milky Way plane's density at
//! its pressure puts some 17% to 38% of the plane's volume in the hot phase, the brainstorm's "a
//! fifth to two fifths"; the corona itself comes out hot for every seed (Design note 12).
//!
//! Hot gas is fully ionised and cold and molecular gas neutral. That rule is what the 21 cm column
//! counts ([`ThermalState::neutral_share`]).

use crate::galaxy::gas::{IONISED_PARTICLES_PER_HYDROGEN, NEUTRAL_PARTICLES_PER_HYDROGEN};
use crate::units::{HydrogenPerCm3, Kelvin, KelvinPerCm3};

/// The temperature above which gas is hot, 10⁵ K (Design note 12).
pub const HOT_TEMPERATURE: Kelvin = Kelvin::new(1e5);

/// The temperature above which gas that is not hot is warm, 5,000 K (Design note 12), and the
/// least a warm point reads (ruling 98 of 2026-09-22).
pub const WARM_TEMPERATURE: Kelvin = Kelvin::new(5_000.0);

/// The most a warm point reads, 10,000 K: the top of the warm media's measured 6,000–10,000 K
/// (Haffner et al. 2009, Rev. Mod. Phys. 81, 969, §II.A; ruling 98 of 2026-09-22).
pub const WARM_CEILING: Kelvin = Kelvin::new(10_000.0);

/// The density above which gas that is neither hot nor warm is molecular, 100 cm⁻³ (Design note
/// 12).
pub const MOLECULAR_DENSITY: HydrogenPerCm3 = HydrogenPerCm3::new(100.0);

/// The share of the hydrogen the smooth layers leave neutral at a point where the neutral layer's
/// density is `neutral` and the warm ionised layer's `warm` (cm⁻³): `neutral ÷ (neutral + warm)`,
/// the share a warm point there holds (Design note 12). A point where neither layer has any gas
/// left, far outside the disc, is taken as ionised.
#[must_use]
pub fn warm_neutral_share(neutral: f64, warm: f64) -> f64 {
    let both = neutral + warm;
    if both > 0.0 { neutral / both } else { 0.0 }
}

/// The particles per hydrogen nucleus of warm gas whose hydrogen is the share `neutral_share`
/// neutral: `1.1 + 1.2 (1 − f_n)`, written `2.3 − 1.2 f_n` so that it is 2.3 exactly when the gas
/// is ionised (ruling 91 of 2026-09-22).
#[must_use]
pub fn warm_particles_per_hydrogen(neutral_share: f64) -> f64 {
    IONISED_PARTICLES_PER_HYDROGEN
        - (IONISED_PARTICLES_PER_HYDROGEN - NEUTRAL_PARTICLES_PER_HYDROGEN) * neutral_share
}

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
    /// let phase = |n| GasPhase::of(HydrogenPerCm3::new(n), plane);
    /// // The corona's density at the plane's pressure is far above 10⁵ K.
    /// assert_eq!(phase(1e-3), GasPhase::Hot);
    /// // The mean neutral disc is warm, and a clump of it ten times denser is cold.
    /// assert_eq!(phase(0.3), GasPhase::Warm);
    /// assert_eq!(phase(3.0), GasPhase::Cold);
    /// assert_eq!(phase(300.0), GasPhase::Molecular);
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
}

/// The gas at a point as its phase and ionisation describe it: the phase, the share of its
/// hydrogen that is neutral, the particles per hydrogen nucleus, and the temperature it reads
/// (module documentation).
///
/// `x = 2.3 − 1.2 f` everywhere. The temperature is `(P ÷ k) ÷ (x n)` above 10⁵ K when hot and
/// at most 5,000 K when cold or molecular; when warm it is that clamped to 5,000–10,000 K, and
/// [`clamped`](Self::clamped) says whether the clamp binds.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::gas::phase::{GasPhase, ThermalState};
/// use hyperion_sim::units::{HydrogenPerCm3, KelvinPerCm3};
///
/// let (n, p) = (HydrogenPerCm3::new(0.5), KelvinPerCm3::new(3_800.0));
/// // Warm ionised gas, where the warm layer holds all the hydrogen, has 2.3 particles, and here
/// // reads 3,800 ÷ (2.3 × 0.5) = 3,304 K, which is clamped to 5,000 K.
/// let ionised = ThermalState::of(n, p, 0.0);
/// assert_eq!(ionised.phase(), GasPhase::Warm);
/// assert!(ionised.clamped());
/// assert!((ionised.temperature(n, p).value() - 5_000.0).abs() < 1e-9);
/// // Neutral gas of the same density reads 3,800 ÷ (1.1 × 0.5) = 6,909 K, unclamped.
/// let neutral = ThermalState::of(n, p, 1.0);
/// assert!(!neutral.clamped() && (neutral.neutral_share() - 1.0).abs() < f64::EPSILON);
/// assert!((neutral.temperature(n, p).value() - 3_800.0 / 0.55).abs() < 1e-9);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThermalState {
    phase: GasPhase,
    neutral_share: f64,
    particles: f64,
    /// The temperature a warm point is held at where `P ÷ (x n)` falls outside 5,000–10,000 K.
    clamp: Option<Kelvin>,
}

impl ThermalState {
    /// The state of gas of density `n` at pressure `p_over_k` where the smooth layers leave the
    /// share `neutral_share` of the hydrogen neutral ([`warm_neutral_share`]).
    #[must_use]
    pub fn of(n: HydrogenPerCm3, p_over_k: KelvinPerCm3, neutral_share: f64) -> Self {
        let phase = GasPhase::of(n, p_over_k);
        let (share, particles, clamp) = match phase {
            GasPhase::Hot => (0.0, IONISED_PARTICLES_PER_HYDROGEN, None),
            GasPhase::Warm => {
                let particles = warm_particles_per_hydrogen(neutral_share);
                let t = p_over_k.value() / (particles * n.value());
                let clamp = if t < WARM_TEMPERATURE.value() {
                    Some(WARM_TEMPERATURE)
                } else if t > WARM_CEILING.value() {
                    Some(WARM_CEILING)
                } else {
                    None
                };
                (neutral_share, particles, clamp)
            }
            GasPhase::Cold | GasPhase::Molecular => (1.0, NEUTRAL_PARTICLES_PER_HYDROGEN, None),
        };
        Self {
            phase,
            neutral_share: share,
            particles,
            clamp,
        }
    }

    /// The phase.
    #[must_use]
    pub fn phase(&self) -> GasPhase {
        self.phase
    }

    /// The share of the hydrogen that is neutral: 0 when hot, 1 when cold or molecular, and the
    /// smooth layers' share when warm. It is what the 21 cm column counts.
    #[must_use]
    pub fn neutral_share(&self) -> f64 {
        self.neutral_share
    }

    /// The particles per hydrogen nucleus `x`: 2.3 when hot, 1.1 when cold or molecular, and
    /// `2.3 − 1.2 f` when warm with `f` the [`neutral_share`](Self::neutral_share).
    #[must_use]
    pub fn particles_per_hydrogen(&self) -> f64 {
        self.particles
    }

    /// Whether the gas is warm and its temperature clamped to 5,000 or 10,000 K, so that `T × x n`
    /// is not `P` (module documentation).
    #[must_use]
    pub fn clamped(&self) -> bool {
        self.clamp.is_some()
    }

    /// The temperature of gas of density `n` at pressure `p_over_k`, which must be the ones the
    /// state was taken at: the equilibrium `(P ÷ k) ÷ (x n)`, held to 5,000–10,000 K when warm;
    /// infinite where the density is 0.
    #[must_use]
    pub fn temperature(&self, n: HydrogenPerCm3, p_over_k: KelvinPerCm3) -> Kelvin {
        self.clamp
            .unwrap_or_else(|| Kelvin::new(p_over_k.value() / (self.particles * n.value())))
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::stats::normal_cdf;

    use super::*;
    use crate::Seed;
    use crate::galaxy::gas::params::GasParams;
    use crate::galaxy::gas::pressure::Pressure;
    use crate::galaxy::gas::smooth::{GasLayer, SmoothGas};
    use crate::galaxy::imf::MassFunctionKind;
    use crate::galaxy::params::GalaxyParams;
    use crate::math;

    /// The shares the tests take the phases at: ionised, the warm layer's own mix, the plane's and
    /// neutral.
    const SHARES: [f64; 4] = [0.0, 0.5, 0.96, 1.0];

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
        // Ruling 91's particle count: 1.1 neutral, 2.3 ionised, linear between.
        assert!((warm_particles_per_hydrogen(1.0) - 1.1).abs() < 1e-15);
        assert!(warm_particles_per_hydrogen(0.0).total_cmp(&2.3).is_eq());
        assert!((warm_particles_per_hydrogen(0.5) - 1.7).abs() < 1e-15);
        assert!(warm_neutral_share(0.0, 0.0).abs() < f64::MIN_POSITIVE);
        assert!((warm_neutral_share(0.8, 0.2) - 0.8).abs() < 1e-15);
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

    /// Rulings 91 and 98: a state's temperature agrees with its label at every share — above 10⁵ K
    /// when hot, 5,000–10,000 K when warm, never above 5,000 K when cold — the warm gas's `x` is
    /// `2.3 − 1.2 f` with the smooth share `f` exactly, and `T × x n = P` wherever the warm clamp
    /// does not bind.
    #[test]
    fn a_states_temperature_agrees_with_its_label() {
        let (mut warm, mut clamped) = (0_u32, 0_u32);
        for share in SHARES {
            for pressure in [400.0, 3_800.0, 40_000.0] {
                for j in 0..=1_200 {
                    let density = math::exp10(-6.0 + 0.01 * f64::from(j));
                    let state = check_state(density, pressure, share);
                    if state.phase() == GasPhase::Warm {
                        warm += 1;
                        clamped += u32::from(state.clamped());
                    }
                }
            }
        }
        // The scan crosses the whole warm band at every share, so the clamp binds at both ends
        // and not everywhere.
        assert!(clamped > 0 && clamped < warm, "{clamped} of {warm}");
    }

    /// One point of [`a_states_temperature_agrees_with_its_label`], at `density` cm⁻³, `pressure`
    /// K cm⁻³ and the smooth share `share`.
    fn check_state(density: f64, pressure: f64, share: f64) -> ThermalState {
        let (n, p) = (HydrogenPerCm3::new(density), KelvinPerCm3::new(pressure));
        let state = ThermalState::of(n, p, share);
        let t = state.temperature(n, p).value();
        let particles = state.particles_per_hydrogen();
        let balance = t * particles * density / pressure;
        if !state.clamped() {
            assert!((balance - 1.0).abs() < 1e-15, "T x n ÷ P = {balance}");
        }
        let f = state.neutral_share();
        assert!((0.0..=1.0).contains(&f), "a share of {f}");
        assert!((warm_particles_per_hydrogen(f) - particles).abs() < 1e-14);
        match state.phase() {
            GasPhase::Hot => assert!(t > HOT_TEMPERATURE.value() && !state.clamped()),
            GasPhase::Warm => {
                assert!(f.total_cmp(&share).is_eq(), "{f} moved from {share}");
                assert!(
                    particles
                        .total_cmp(&warm_particles_per_hydrogen(share))
                        .is_eq(),
                    "{particles} is not the smooth count"
                );
                assert!(
                    (WARM_TEMPERATURE.value()..=WARM_CEILING.value()).contains(&t),
                    "{t} K warm"
                );
                let smooth = pressure / (particles * density);
                let inside = (WARM_TEMPERATURE.value()..=WARM_CEILING.value()).contains(&smooth);
                assert_eq!(state.clamped(), !inside, "{smooth} K before the clamp");
            }
            GasPhase::Cold | GasPhase::Molecular => {
                assert!(
                    t <= WARM_TEMPERATURE.value() && !state.clamped(),
                    "{t} K is {state:?}"
                );
            }
        }
        state
    }

    /// The share of the hydrogen a 21 cm column counts: none of the hot gas, all of the cold, and
    /// the smooth layers' share of the warm.
    #[test]
    fn the_neutral_share_follows_the_phase() {
        let p = KelvinPerCm3::new(3_800.0);
        let state = |n: f64, share: f64| ThermalState::of(HydrogenPerCm3::new(n), p, share);
        assert!(state(1e-3, 0.96).neutral_share().abs() < f64::MIN_POSITIVE);
        assert!((state(3.0, 0.96).neutral_share() - 1.0).abs() < f64::EPSILON);
        assert!((state(300.0, 0.0).neutral_share() - 1.0).abs() < f64::EPSILON);
        assert!((state(0.3, 0.8).neutral_share() - 0.8).abs() < 1e-15);
        assert!(state(0.1, 0.0).neutral_share().abs() < f64::MIN_POSITIVE);
        // Ruling 98: near either edge of the warm phase the share is still the smooth one; the
        // temperature is clamped instead.
        assert!(state(0.02, 0.96).neutral_share().total_cmp(&0.96).is_eq());
        assert!(state(0.02, 0.96).clamped());
        assert!(state(0.6, 0.0).neutral_share().abs() < f64::MIN_POSITIVE);
        assert!(state(0.6, 0.0).clamped());
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

    /// The radii ruling 91 holds the corona hot at, 20,000 ly above the plane: 8,000–40,000 ly.
    fn corona_radii() -> impl Iterator<Item = f64> {
        (0..=32).map(|i| 8_000.0 + 1_000.0 * f64::from(i))
    }

    /// The mean gas 20,000 ly above the plane at radius `r` of `params`, and whether it is hot.
    fn hot_far_above(params: &GasParams, r: f64) -> (f64, bool) {
        let (gas, pressure) = (SmoothGas::new(params), Pressure::new(params));
        let z = 20_000.0;
        let n = gas.mean_density(r, z);
        let p = pressure.at(&gas, r, z);
        let share = warm_neutral_share(
            gas.density(GasLayer::Neutral, r, z),
            gas.density(GasLayer::Warm, r, z),
        );
        let state = ThermalState::of(HydrogenPerCm3::new(n), KelvinPerCm3::new(p), share);
        (
            state
                .temperature(HydrogenPerCm3::new(n), KelvinPerCm3::new(p))
                .value(),
            state.phase() == GasPhase::Hot,
        )
    }

    /// Ruling 91: far above the disc, 20,000 ly up at every radius from 8,000 to 40,000 ly, the
    /// mean gas is hot for every seed of 2,000 drawn against the fixture's galaxy and 32 against
    /// their own — the corona and what is left of the warm layer, at what is left of the pressure.
    /// `gas_statistics.rs` holds it over 2,000 seeds' own galaxies, and
    /// `the_corona_is_hot_at_every_corner_of_the_draws` proves it.
    #[test]
    fn far_above_the_disc_the_gas_is_hot_for_every_seed() {
        let check = |params: &GasParams| {
            for r in corona_radii() {
                let (t, hot) = hot_far_above(params, r);
                assert!(hot, "{t} K at {r} ly");
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

    /// Ruling 91's proof that the corona is hot 20,000 ly above every radius from 8,000 to
    /// 40,000 ly, for every galaxy the draws can make.
    ///
    /// There the neutral layer (700 ly tall) and the molecular disc have fallen by more than e⁻²⁸,
    /// to under 10⁻⁹ cm⁻³ between them, which the bound adds, and the pressure is its floor plus a term that is not negative, so the temperature is at
    /// least `P_cor ÷ (2.3 (n_cor + n_w + 10⁻⁹))`, with the warm layer's density `n_w = w exp(−R_m (1 ÷ R
    /// − 1 ÷ R₀) ÷ 2 − (R − R₀) ÷ R_g − z ÷ h_w)` for its drawn density `w` at `R₀`. That is
    /// monotone in each of `w`, `h_w`, `R_m` and `1 ÷ R_g`, so its least value over the draws is at
    /// a corner: the lowest floor, 300 K cm⁻³; the densest corona, 0.8 × 10⁻³ cm⁻³; the densest and
    /// tallest warm layer, 0.035 cm⁻³ and 3,500 ly (the clamp of ruling 91 only lowers it); the
    /// hole `R_m` at 0.8–1.2 of the bar's 10,000–18,000 ly; and the gas disc 1.5–2 times the thin
    /// disc's 7,000–11,500 ly. The least is about 104,000 K, at the shortest gas disc and the
    /// smallest hole over the inner disc.
    #[test]
    fn the_corona_is_hot_at_every_corner_of_the_draws() {
        let (floor, corona, warm, height) = (300.0, 0.8e-3, 0.035, 3_500.0);
        let reference = GasParams::REFERENCE_RADIUS.value();
        let (z, mut least) = (20_000.0, f64::INFINITY);
        for hole in [0.8 * 10_000.0, 1.2 * 18_000.0] {
            for scale in [1.5 * 7_000.0, 2.0 * 11_500.0] {
                for r in corona_radii() {
                    let n_w = warm
                        * math::exp(
                            -0.5 * hole * (1.0 / r - 1.0 / reference)
                                - (r - reference) / scale
                                - z / height,
                        );
                    let t = floor / (IONISED_PARTICLES_PER_HYDROGEN * (corona + n_w + 1e-9));
                    least = least.min(t);
                }
            }
        }
        assert!(
            least > HOT_TEMPERATURE.value(),
            "{least} K at the worst corner"
        );
        assert!((103_000.0..105_000.0).contains(&least), "{least} K");
    }
}
