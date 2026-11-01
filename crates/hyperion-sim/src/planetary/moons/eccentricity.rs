//! A regular moon's eccentricity at its system's age (plan 14, P14.T55.j.a;
//! decision-p14-t56d-membrane §7.1): a free eccentricity damped by the moon's own tidal heat, and
//! a forced one held to its resonance's energy budget.
//!
//! # The damping time
//!
//! A synchronous moon that dissipates its tidal heat H at its eccentricity e, its orbit's angular
//! momentum kept, loses e on `τ_e` = G `M_p` m e² ÷ (a H (1 − e²)) ([`damping_time`]). At a fixed
//! Im k₂ it is (2 ÷ 21)(m ÷ `M_p`)(a ÷ R)⁵ ÷ (n Im k₂ (1 − e²)), the circularisation time of
//! Goldreich and Soter (1966, Icarus 5, 375; from memory), independent of e but for its last
//! factor. On the ocean branch Im k₂ is the moon's own at e, so `τ_e` reads its tide solve at the
//! forcing Φ(e).
//!
//! # A free eccentricity
//!
//! A moon in no resonance that holds keeps its drawn e where `τ_e(e)` reaches the system's age
//! t, and otherwise takes the largest e below it at which `τ_e(e)` ≥ t, by
//! [`ECCENTRICITY_BISECTIONS`] bisections in ln e from [`FREE_ECCENTRICITY_FLOOR`] of it
//! ([`free_eccentricity`]). Where `τ_e` falls short even there, as a cold moon's fixed Im k₂
//! gives it at any e, its orbit is circular. The rule has no free parameter.
//!
//! # A forced eccentricity
//!
//! The two moons of a 2:1 resonance, the outer at twice the inner's period, are held there by their
//! planet's tides, which push the inner moon out faster than the outer. The orbits then expand at
//! one fractional rate, so the torques Γᵢ pay for the orbits' energy and the moons' heat
//! ([`resonance_budget`]): H₁ + H₂ ≤ B = Σ Γᵢnᵢ − (Σ Γᵢ)(Σ Lᵢnᵢ) ÷ (Σ Lᵢ), with Lᵢ = mᵢnᵢaᵢ² and
//! Γᵢ = (3 ÷ 2)(k₂ ÷ Q) G mᵢ² `R_p`⁵ ÷ aᵢ⁶ (as Meyer and Wisdom 2007, Icarus 188, 535, give
//! Enceladus's and Dione's equilibrium heating; from memory). The parent's k₂ ÷ Q is the one at
//! which the pair's inner moon migrates over the system's age ([`locking_tidal_response`]): the
//! present-day rate at the inner moon's frequency, as resonance locking reads it (Fuller, Luan and
//! Quataert 2016, MNRAS 458, 3867; from memory), bounded at Q ≥ [`RESONANCE_LEAST_TIDAL_Q`], the
//! planet's own k₂ ÷ 100 ([`budget_tidal_response`]; lean, 2026-11-01, research-leans-19 and -20).
//! Both moons take one scale s ≤ 1 of their e, H₁(s e₁) + H₂(s e₂) ≤ B, by [`SCALE_BISECTIONS`]
//! bisections in ln s ([`forced_scale`]); a moon in two pairs, the middle of a 1:2:4 chain, takes
//! the smaller of its pairs' scales, so that neither pair dissipates over its budget (lean,
//! 2026-11-01, research-leans-20). Where B ≤ 0 the outer moon's tide outruns the inner's, as
//! where the outer is over some 20 times the inner's mass, no tidal resonance holds the pair, and
//! a moon in no other pair takes the free rule.
//!
//! One k₂ ÷ Q serves both moons of a pair, by design. At each moon's own locking rate both orbits
//! would expand at one fractional rate already, and B would be exactly 0 (Enceladus and Dione's
//! 13.8 GW at one shared value is 0 with one each): the shared value is what gives a pair a
//! budget.
//!
//! The rule reads the planet's present-day rate, not its age-averaged Q: the generator's
//! `GIANT_TIDAL_Q`, which the moon limit and a planet's spin read as that average, gives Enceladus
//! and Dione 0.25 GW, and nearly every porous ocean would freeze. Where the moon limit binds,
//! about a close-in giant, no 2:1 pair forms, so the two never read one system's pair. The budget
//! grows as 1 ÷ t, so in a system younger than some 300 Myr a gas giant's heavy, close inner moons
//! keep their drawn e and run hot on the heated branch, paid for by the budget (the heated
//! branch's Im k₂ is P14.T55.j's).
//!
//! # The hold
//!
//! The rule is P14.T55.j.a's, behind `ECCENTRICITY_DAMPING_VERSION` = 27. Below it every regular
//! moon keeps its drawn eccentricity, to the bit.

use crate::math;
use crate::units::consts::GRAVITATIONAL_CONSTANT;
use crate::units::{Kilograms, Metres, Seconds, Watts};
use crate::{GENERATOR_VERSION, GeneratorVersion};

/// The generator version from which a regular moon's eccentricity is the one its tides and its
/// resonance leave it at its system's age (P14.T55.j.a): 27, with P14.T56.e's porous core, whose
/// heat at a held eccentricity it bounds.
///
/// It is P14.T55.j.a's one hold. Below it [`EccentricityDampingRules::in_force`] is
/// [`EccentricityDampingRules::Held`]: every regular moon keeps the eccentricity drawn for it, a
/// forced one included where its resonance does not hold, and every figure is version 24's, 25's
/// and 26's bit for bit. 27's bump deletes this hold, [`EccentricityDampingRules`] with its `Held`
/// arm, the tests' switch and its tripwire
/// (`the_eccentricity_damping_is_held_only_until_the_27_bump`).
pub(crate) const ECCENTRICITY_DAMPING_VERSION: GeneratorVersion = GeneratorVersion::new(27);

/// Whether a regular moon's eccentricity is damped to its age (its hold,
/// [`ECCENTRICITY_DAMPING_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum EccentricityDampingRules {
    /// Version 26's and before: the drawn eccentricity, forced or free (P14.T17.a).
    Held,
    /// Version 27's: a forced eccentricity only in a resonance that holds, at its budget, and a
    /// free one at its age (see the [module](self) documentation).
    Damped,
}

impl EccentricityDampingRules {
    /// The rules this generator version gives: the damping from [`ECCENTRICITY_DAMPING_VERSION`],
    /// the drawn eccentricity below it; in the tests, the rules a test's switch sets on its thread
    /// ([`with_eccentricity_damping`]).
    #[must_use]
    pub(crate) fn in_force() -> Self {
        #[cfg(test)]
        if let Some(rules) = ECCENTRICITY_DAMPING_RULES.with(core::cell::Cell::get) {
            return rules;
        }
        if GENERATOR_VERSION >= ECCENTRICITY_DAMPING_VERSION {
            Self::Damped
        } else {
            Self::Held
        }
    }
}

#[cfg(test)]
thread_local! {
    /// The damping's rules a test sets on its thread, `None` for the version's.
    static ECCENTRICITY_DAMPING_RULES: core::cell::Cell<Option<EccentricityDampingRules>> =
        const { core::cell::Cell::new(None) };
}

/// `run`'s result with the damping's rules `rules` on this thread, for the tests to reach
/// P14.T55.j.a ahead of the bump; the version's rules are set back after, a panic in `run`
/// included. 27's bump deletes it with the hold.
#[cfg(test)]
pub(crate) fn with_eccentricity_damping<R>(
    rules: EccentricityDampingRules,
    run: impl FnOnce() -> R,
) -> R {
    /// Sets the thread's rules back as it drops.
    struct Restore(Option<EccentricityDampingRules>);
    impl Drop for Restore {
        fn drop(&mut self) {
            ECCENTRICITY_DAMPING_RULES.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(ECCENTRICITY_DAMPING_RULES.with(|cell| cell.replace(Some(rules))));
    run()
}

/// The ratio to the drawn eccentricity of the low end of [`free_eccentricity`]'s bracket:
/// 10⁻³, where the forcing Φ ∝ e² is 10⁻⁶ of the drawn one's, so the moon's Im k₂ is its
/// unforced one's: where `τ_e` falls short there, it falls short at every e.
pub const FREE_ECCENTRICITY_FLOOR: f64 = 1e-3;

/// The least tidal Q of a planet's tide in a resonance's budget: 100 ([`budget_tidal_response`];
/// lean, 2026-11-01, research-leans-19). The locking k₂ ÷ Q grows as (a ÷ `R_p`)^6.5 ÷ m without
/// bound, so about a Saturn-mass planet a 10²⁰ kg inner moon at 15 planetary radii would take
/// Q < 1, and its pair's budget would pay any heat. Q = 100 is about the strongest effective
/// dissipation measured: Titan recedes at 11.3 ± 2.0 cm yr⁻¹ (Lainey et al. 2020, Nature Astronomy
/// 4, 1053; from memory), 2.4 times the age's rate, Q ≈ 120 at Saturn's k₂ of 0.382. It does not
/// bind for Enceladus's 2.2 × 10⁻⁴ or Io's 8.5 × 10⁻⁶.
pub const RESONANCE_LEAST_TIDAL_Q: f64 = 100.0;

/// The bisections in ln e of [`free_eccentricity`]: 20, which narrow its bracket of ln 10³ to
/// 7 × 10⁻⁶ of e.
pub const ECCENTRICITY_BISECTIONS: u32 = 20;

/// The bisections in ln s of [`forced_scale`]: 20, which narrow a bracket of ln 10² to 4 × 10⁻⁶
/// of s.
pub const SCALE_BISECTIONS: u32 = 20;

/// The halvings by which [`forced_scale`] steps its bracket's low end down from its first guess
/// where that guess still dissipates over the budget: 16, to 1.5 × 10⁻⁵ of it, below which the
/// pair's orbits are circular.
pub const SCALE_HALVINGS: u32 = 16;

/// The time, s, on which a moon of mass `moon` on an orbit of semi-major axis `a` and eccentricity
/// `e` about a planet of mass `planet` loses its eccentricity while it dissipates the tidal heat
/// `heat`, its orbit's angular momentum kept: `τ_e` = G `M_p` m e² ÷ (a H (1 − e²)) (see the
/// [module](self) documentation). Infinite for a moon that dissipates nothing.
///
/// # Examples
///
/// Enceladus at 27 GW, about its 2:1 pair's equilibrium, damps its eccentricity in under a
/// million years:
///
/// ```
/// use hyperion_sim::planetary::moons::eccentricity::damping_time;
/// use hyperion_sim::units::{Kilograms, Metres, Watts};
///
/// let tau = damping_time(
///     Kilograms::new(5.683e26),
///     Kilograms::new(1.080e20),
///     Metres::new(2.3802e8),
///     0.0047,
///     Watts::new(27e9),
/// );
/// let years = tau.value() / 3.155_76e7;
/// assert!((3e5..1e6).contains(&years), "{years}");
/// ```
#[must_use]
pub fn damping_time(planet: Kilograms, moon: Kilograms, a: Metres, e: f64, heat: Watts) -> Seconds {
    let h = heat.value();
    if h <= 0.0 {
        return Seconds::new(f64::INFINITY);
    }
    Seconds::new(
        GRAVITATIONAL_CONSTANT * planet.value() * moon.value() * e * e
            / (a.value() * h * (1.0 - e * e)),
    )
}

/// One moon of a chain of resonances as its budget reads it ([`resonance_budget`]): its mass, its
/// semi-major axis and its mean motion, rad s⁻¹.
///
/// The fields are plain values with no invariant between them, so they are public.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResonantOrbit {
    /// Its mass.
    pub mass: Kilograms,
    /// Its semi-major axis.
    pub a: Metres,
    /// Its mean motion, rad s⁻¹.
    pub mean_motion_rad_per_s: f64,
}

/// The planet's tidal response k₂ ÷ Q, dimensionless, at which its tide raised by the moon `inner`
/// pushes it out over the time `age`: a ÷ ȧ = L ÷ 2Γ = `M_p` (a ÷ `R_p`)⁵ ÷ (3 (k₂ ÷ Q) m n) = t,
/// so k₂ ÷ Q = `M_p` (a ÷ `R_p`)⁵ ÷ (3 m n t), for a planet of mass `planet` and radius `radius`.
///
/// It is resonance locking's present rate, migration on the system's age (Fuller, Luan and
/// Quataert 2016, MNRAS 458, 3867; from memory), as the astrometry finds it within 2σ: Saturn's
/// at Enceladus, 2.2 × 10⁻⁴ over 4.57 Gyr against (1.59 ± 0.74) × 10⁻⁴ (Lainey et al. 2017,
/// Icarus 281, 286), and Jupiter's at Io, 8.5 × 10⁻⁶ against (1.102 ± 0.203) × 10⁻⁵ (Lainey et
/// al. 2009, Nature 459, 957), −1.2σ; both from memory, since the pod reached neither paper. It
/// is P14.T55.j.a's lean for a pair's budget (2026-11-01), which caps it
/// ([`budget_tidal_response`]).
///
/// # Examples
///
/// Saturn at Enceladus over 4.57 Gyr, inside the astrometric range:
///
/// ```
/// use hyperion_sim::planetary::moons::eccentricity::{ResonantOrbit, locking_tidal_response};
/// use hyperion_sim::units::{Kilograms, Metres, Seconds};
///
/// let enceladus = ResonantOrbit {
///     mass: Kilograms::new(1.080e20),
///     a: Metres::new(2.3802e8),
///     mean_motion_rad_per_s: 5.307e-5,
/// };
/// let saturn = (Kilograms::new(5.683e26), Metres::new(6.0268e7));
/// let response = locking_tidal_response(saturn.0, saturn.1, &enceladus, Seconds::new(1.442e17));
/// assert!((1.1e-5..3.07e-4).contains(&response), "{response}");
/// ```
#[must_use]
pub fn locking_tidal_response(
    planet: Kilograms,
    radius: Metres,
    inner: &ResonantOrbit,
    age: Seconds,
) -> f64 {
    planet.value() * math::powi(inner.a.value() / radius.value(), 5)
        / (3.0 * inner.mass.value() * inner.mean_motion_rad_per_s * age.value())
}

/// The planet's k₂ ÷ Q a pair's budget reads ([`budget_tidal_response`]): the locking value, or
/// the ceiling where that binds, which it does for about a quarter of a gas giant's pairs and half
/// an ice giant's at the Sun's age, more in a younger system.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum BudgetResponse {
    /// The locking value ([`locking_tidal_response`]), dimensionless.
    Locking(f64),
    /// The ceiling, the planet's own k₂ ÷ [`RESONANCE_LEAST_TIDAL_Q`], dimensionless, below the
    /// locking value.
    Ceiling(f64),
}

impl BudgetResponse {
    /// Its k₂ ÷ Q.
    #[must_use]
    pub const fn value(self) -> f64 {
        match self {
            Self::Locking(response) | Self::Ceiling(response) => response,
        }
    }
}

/// The k₂ ÷ Q of a planet of mass `planet`, radius `radius` and Love number `love_number` (its
/// class's) that the budget of a pair whose inner moon is `inner` reads over the time `age`: the
/// locking value ([`locking_tidal_response`]), the present-day rate at that moon's frequency,
/// capped at k₂ ÷ [`RESONANCE_LEAST_TIDAL_Q`] (P14.T55.j.a; lean, 2026-11-01, research-leans-19).
/// One value serves both moons of the pair (see the [module](self) documentation).
///
/// # Examples
///
/// A light inner moon far out about a Saturn-mass planet would lock at Q < 1; its budget takes the
/// ceiling, Q = 100:
///
/// ```
/// use hyperion_sim::planetary::moons::eccentricity::{
///     BudgetResponse, ResonantOrbit, budget_tidal_response,
/// };
/// use hyperion_sim::units::{Kilograms, Metres, Seconds};
///
/// let (saturn, radius) = (Kilograms::new(5.683e26), Metres::new(6.0268e7));
/// let a = 15.0 * radius.value();
/// let moon = ResonantOrbit {
///     mass: Kilograms::new(1e20),
///     a: Metres::new(a),
///     mean_motion_rad_per_s: (6.674e-11 * saturn.value() / (a * a * a)).sqrt(),
/// };
/// let response = budget_tidal_response(saturn, radius, 0.4, &moon, Seconds::new(1.442e17));
/// assert_eq!(response, BudgetResponse::Ceiling(0.4 / 100.0));
/// ```
#[must_use]
pub fn budget_tidal_response(
    planet: Kilograms,
    radius: Metres,
    love_number: f64,
    inner: &ResonantOrbit,
    age: Seconds,
) -> BudgetResponse {
    let locking = locking_tidal_response(planet, radius, inner, age);
    let ceiling = love_number / RESONANCE_LEAST_TIDAL_Q;
    if locking > ceiling {
        BudgetResponse::Ceiling(ceiling)
    } else {
        BudgetResponse::Locking(locking)
    }
}

/// The tidal heat a chain of moons in resonance, `chain` inside out, can dissipate in all while
/// its planet's tides, at `k2_over_q` (k₂ ÷ Q) and of a planet of radius `radius`, hold it there:
/// B = Σ Γᵢnᵢ − (Σ Γᵢ)(Σ Lᵢnᵢ) ÷ (Σ Lᵢ), with Lᵢ = mᵢnᵢaᵢ² and Γᵢ = (3 ÷ 2)(k₂ ÷ Q) G mᵢ² `R_p`⁵ ÷
/// aᵢ⁶ (see the [module](self) documentation). Not positive where the outer moons' tides outrun
/// the inner's, so that no tidal resonance holds them; zero for a chain of fewer than two moons.
/// The damping reads it pair by pair, a chain of two.
///
/// The torques do work Σ Γᵢnᵢ on the orbits. Held in resonance, the orbits expand at one
/// fractional rate, α = 2 Σ Γᵢ ÷ Σ Lᵢ, so their energy grows at Σ nᵢ L̇ᵢ = (Σ Γᵢ)(Σ Lᵢnᵢ) ÷ Σ Lᵢ,
/// and what is left is the moons' heat: for two moons, Meyer and Wisdom's (2007, Icarus 188,
/// 535) equilibrium heating, 1.1 GW for Enceladus at Q = 18,000 and k₂ = 0.341 (from memory).
///
/// # Examples
///
/// Enceladus and Dione about Saturn at Meyer and Wisdom's k₂ and Q:
///
/// ```
/// use hyperion_sim::planetary::moons::eccentricity::{ResonantOrbit, resonance_budget};
/// use hyperion_sim::units::{Kilograms, Metres};
///
/// let enceladus = ResonantOrbit {
///     mass: Kilograms::new(1.080e20),
///     a: Metres::new(2.3802e8),
///     mean_motion_rad_per_s: 5.307e-5,
/// };
/// let dione = ResonantOrbit {
///     mass: Kilograms::new(1.0955e21),
///     a: Metres::new(3.774e8),
///     mean_motion_rad_per_s: 2.657e-5,
/// };
/// let budget = resonance_budget(Metres::new(6.0268e7), 0.341 / 18_000.0, &[enceladus, dione]);
/// assert!((1.0e9..1.3e9).contains(&budget.value()), "{budget:?}");
/// ```
#[must_use]
pub fn resonance_budget(radius: Metres, k2_over_q: f64, chain: &[ResonantOrbit]) -> Watts {
    if chain.len() < 2 {
        return Watts::new(0.0);
    }
    let lever = 1.5 * k2_over_q * GRAVITATIONAL_CONSTANT * math::powi(radius.value(), 5);
    let (mut work, mut torque, mut momentum, mut weighted) = (0.0, 0.0, 0.0, 0.0);
    for moon in chain {
        let (m, a, n) = (
            moon.mass.value(),
            moon.a.value(),
            moon.mean_motion_rad_per_s,
        );
        let gamma = lever * m * m / math::powi(a, 6);
        let l = m * n * a * a;
        work += gamma * n;
        torque += gamma;
        momentum += l;
        weighted += l * n;
    }
    Watts::new(work - torque * weighted / momentum)
}

/// The eccentricity a free moon drawn at `drawn` keeps at the system's age `age`, whose damping
/// time at an eccentricity e is `damping(e)` ([`damping_time`] at its own heat at e): `drawn` where
/// `damping(drawn)` ≥ `age`; otherwise the largest e below it at which `damping(e)` ≥ `age`, by
/// [`ECCENTRICITY_BISECTIONS`] bisections in ln e between `drawn` and
/// [`FREE_ECCENTRICITY_FLOOR`] of it, the bracket's end that holds; 0, a circular orbit, where
/// even that low end damps within the
/// age (see the [module](self) documentation).
///
/// Where the damping time is not monotonic in e, as a soft shell's can be, the bisection keeps a
/// crossing of the age, not always the largest. `damping` is read at most
/// [`ECCENTRICITY_BISECTIONS`] + 2 times.
///
/// # Examples
///
/// A moon whose ocean dissipates enough to damp its eccentricity in 1 Myr above e = 0.003, and
/// which freezes, keeping it 10 Gyr, below, keeps just under 0.003 of its drawn 0.02:
///
/// ```
/// use hyperion_sim::planetary::moons::eccentricity::free_eccentricity;
/// use hyperion_sim::units::Seconds;
///
/// let gyr = |n: f64| Seconds::new(n * 3.155_76e16);
/// let damping = |e: f64| if e < 0.003 { gyr(10.0) } else { gyr(1e-3) };
/// let e = free_eccentricity(0.02, gyr(4.5), damping);
/// assert!(e < 0.003 && e > 0.002_99, "{e}");
/// ```
#[must_use]
pub fn free_eccentricity(drawn: f64, age: Seconds, mut damping: impl FnMut(f64) -> Seconds) -> f64 {
    let lasts = |tau: Seconds| tau.value() >= age.value();
    if drawn <= 0.0 || lasts(damping(drawn)) {
        return drawn;
    }
    let mut lo = drawn * FREE_ECCENTRICITY_FLOOR;
    if !lasts(damping(lo)) {
        return 0.0;
    }
    let mut hi = drawn;
    for _ in 0..ECCENTRICITY_BISECTIONS {
        let mid = (lo * hi).sqrt();
        if lasts(damping(mid)) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

/// The scale s ∈ [0, 1] of a resonant pair's eccentricities at which its moons dissipate at most
/// their budget `budget`, whose heat in all at a scale s is `heat(s)` (H₁(s e₁) + H₂(s e₂)): 1 where
/// `heat(1)` ≤ `budget`; otherwise the largest s found by [`SCALE_BISECTIONS`] bisections in ln s
/// below 1, from a first guess √(B ÷ H(1)), exact at a fixed Im k₂ where H ∝ e², halved at most
/// [`SCALE_HALVINGS`] times until it dissipates no more than the budget; 0, circular orbits, where
/// none does. At the scale returned `heat(s)` ≤ `budget`, so that a pair never dissipates over its
/// budget.
///
/// Where the heat is not monotonic in s, the bisection keeps a crossing of the budget, not always
/// the largest.
///
/// # Examples
///
/// A pair at 16 GW whose heat goes as e², against a 4 GW budget, takes half its eccentricities:
///
/// ```
/// use hyperion_sim::planetary::moons::eccentricity::forced_scale;
/// use hyperion_sim::units::Watts;
///
/// let s = forced_scale(Watts::new(4e9), |s| Watts::new(16e9 * s * s));
/// assert_eq!(s, 0.5);
/// ```
#[must_use]
pub fn forced_scale(budget: Watts, mut heat: impl FnMut(f64) -> Watts) -> f64 {
    let b = budget.value();
    let whole = heat(1.0).value();
    if whole <= b {
        return 1.0;
    }
    if b <= 0.0 {
        return 0.0;
    }
    let (mut lo, mut hi) = ((b / whole).sqrt(), 1.0);
    let mut found = false;
    for _ in 0..=SCALE_HALVINGS {
        if heat(lo).value() <= b {
            found = true;
            break;
        }
        hi = lo;
        lo *= 0.5;
    }
    if !found {
        return 0.0;
    }
    for _ in 0..SCALE_BISECTIONS {
        let mid = (lo * hi).sqrt();
        if heat(mid).value() <= b {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

#[cfg(test)]
mod tests;
