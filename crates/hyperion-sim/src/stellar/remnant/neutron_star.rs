//! Neutron stars (plan 06, P06.T21): birth spin and field, spin-down in closed form, the radio
//! pulsar's death line, magnetars, the beam, the pulse phase and the wind-nebula flag.
//!
//! A neutron star is born with a spin period and a dipole field drawn once (P06.T21.a, design note
//! 13). Its field decays as B(t) = B₀ ÷ (1 + t ÷ τ), with τ = 10⁴ yr × (10¹⁵ G ÷ B₀), to a
//! floor of 10¹² G (or B₀ if that is lower), and magnetic-dipole braking P Ṗ = k B² then gives
//! P(t)² = P₀² + 2k ∫B² dt, whose integral under this decay law is elementary (P06.T21.b). So the
//! state at any age is a closed form, continuous in age, and nothing needs replaying: mean glitch
//! activity enters as a 1% reduction of the spin-down while the characteristic age lies within
//! 10³–10⁵ years (Fuentes et al. 2017), found in closed form too.
//!
//! What the pulsar is at an age ([`PulsarState`]) decides its class: a magnetar while its field is
//! above the quantum critical field and its field's decay outshines its spin-down, a radio pulsar
//! while it lies above the death line of Bhattacharya et al. (1992), and otherwise a neutron star
//! seen as neither (P06.T21.c). Its beam ([`PulsarBeam`]) says from which directions it is seen
//! to pulse, and its [`PulseClock`] gives the pulse phase at any clock time within the clock
//! window to better than 10⁻³ cycles (P06.T21.d). A wind nebula is flagged while the spin-down
//! luminosity exceeds 10³⁶ erg/s (P06.T21.e); plan 09 draws the nebula.
//!
//! Every figure is plan 06's (design note 13 and P06.T21) and is marked provisional where the
//! lane's research has not confirmed it against its source; the doc comment of each constant says
//! which.
//!
//! The thermal luminosity of a neutron star stays HPT's photon-cooling law ([`hpt_luminosity`],
//! ruling 33), which the track's remnant stage reads.

use core::f64::consts::{FRAC_PI_2, PI};

use crate::coords::UnitVector;
use crate::math;
use crate::stellar::draws::StarDraws;
use crate::stellar::remnant::RemnantRecipe;
use crate::stellar::remnant::structure::neutron_star_radius;
use crate::time::{Span, UniverseTime};
use crate::units::consts::{RADIANS_PER_DEGREE, SECONDS_PER_JULIAN_YEAR};
use crate::units::{Gauss, Metres, Radians, Seconds, SolarLuminosities, SolarMasses, Watts, Years};

/// The mean of the birth spin period, s: 300 ms (Faucher-Giguère and Kaspi 2006, ApJ 643, 332,
/// their optimal model; plan 06, design note 13).
pub const BIRTH_PERIOD_MEAN: Seconds = Seconds::new(0.300);

/// The standard deviation of the birth spin period, s: 150 ms (Faucher-Giguère and Kaspi 2006).
pub const BIRTH_PERIOD_SIGMA: Seconds = Seconds::new(0.150);

/// The shortest birth period, s: a draw at or below 10 ms is redrawn (P06.T21.a).
pub const BIRTH_PERIOD_MIN: Seconds = Seconds::new(0.010);

/// The mean of log₁₀ of the birth dipole field in gauss: 13.25 (Popov et al. 2010, MNRAS 401,
/// 2675, their population synthesis with field decay; plan 06, design note 13).
pub const LOG_BIRTH_FIELD_MEAN: f64 = 13.25;

/// The standard deviation of log₁₀ of the birth field: 0.6 dex (Popov et al. 2010).
pub const LOG_BIRTH_FIELD_SIGMA: f64 = 0.6;

/// The field's decay time at a birth field of 10¹⁵ G, years; it scales as 1 ÷ B₀, so that B₀ τ
/// is 10¹⁹ G yr for every star (P06.T21.b, the Hall-drift scaling of the decaying-field
/// syntheses, provisional).
pub const DECAY_TIME_AT_1E15_G: Years = Years::new(1.0e4);

/// The field the decay stops at, G: 10¹², or the birth field if that is lower (P06.T21.b).
pub const FIELD_FLOOR: Gauss = Gauss::new(1.0e12);

/// The moment of inertia, g cm²: 10⁴⁵, the canonical value of the pulsar literature (P06.T21.b).
pub const MOMENT_OF_INERTIA_G_CM2: f64 = 1.0e45;

/// The share of the spin-down that glitches give back, 1% (Fuentes et al. 2017, A&A 608, A131,
/// the activity of most glitching pulsars; provisional), applied while the characteristic age
/// lies within [`GLITCH_AGES`].
pub const GLITCH_REVERSAL: f64 = 0.01;

/// The characteristic ages, years, over which mean glitch activity slows the spin-down: 10³–10⁵
/// (Fuentes et al. 2017; P06.T21.b).
pub const GLITCH_AGES: (Years, Years) = (Years::new(1.0e3), Years::new(1.0e5));

/// The death line, G s⁻²: a pulsar shines in the radio while B ÷ P² exceeds 0.17 × 10¹² G s⁻²
/// (Bhattacharya, Wijers, Hartman and Verbunt 1992, A&A 254, 198; P06.T21.c).
pub const DEATH_LINE_G_PER_S2: f64 = 0.17e12;

/// The quantum critical field B(Q) = mₑ² c³ ÷ (e ħ), G, as plan 06 rounds it: 4.4 × 10¹³. A
/// magnetar's field lies above it (P06.T21.c).
pub const QUANTUM_CRITICAL_FIELD: Gauss = Gauss::new(4.4e13);

/// The beam's half-angle at a period of one second, degrees: ρ = 5.4° × (P ÷ s)^(−½) (Rankin
/// 1993's core-cone width law as plan 06 states it; P06.T21.c). It gives a beaming fraction of
/// about 15% at 1 s over random inclinations and viewing directions.
pub const BEAM_HALF_ANGLE_AT_1_S: f64 = 5.4;

/// The spin-down luminosity above which a pulsar powers a wind nebula, W: 10³⁶ erg/s (P06.T21.e,
/// as the brainstorm's shell section expects; provisional).
pub const WIND_NEBULA_THRESHOLD: Watts = Watts::new(1.0e29);

/// The speed of light, cm/s.
const SPEED_OF_LIGHT_CM_S: f64 = 2.997_924_58e10;

/// Watts per erg per second.
const WATTS_PER_ERG_S: f64 = 1.0e-7;

/// A newly born neutron star's spin and field, and the closed forms of its spin-down (P06.T21.a–b).
///
/// # Examples
///
/// A pulsar born at 300 ms with a field of 10¹²·⁵ G spins down to its death line after 10–20
/// million years:
///
/// ```
/// use hyperion_sim::coords::UnitVector;
/// use hyperion_sim::stellar::remnant::NeutronStar;
/// use hyperion_sim::units::{Gauss, Radians, Seconds, Years};
///
/// let ns = NeutronStar::new(
///     Seconds::new(0.3),
///     Gauss::new(10f64.powf(12.5)),
///     UnitVector::NORTH,
///     Radians::new(1.0),
///     0.0,
/// );
/// assert!(ns.state_at(Years::new(1.0e6)).is_radio_alive());
/// assert!(!ns.state_at(Years::new(1.0e8)).is_radio_alive());
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NeutronStar {
    /// P₀, s.
    birth_period: f64,
    /// B₀, G.
    birth_field: f64,
    spin_axis: UnitVector,
    /// The magnetic axis's angle from the spin axis, radians, 0–π/2.
    inclination: f64,
    /// The pulse phase's rank at the pulse clock's reference, cycles in [0, 1).
    phase_rank: f64,
    /// The field's floor, G: 10¹² or B₀, whichever is lower.
    floor: f64,
    /// The decay time τ, s.
    decay_time: f64,
    /// The age at which the decay reaches the floor, s; zero where B₀ is at or below it.
    floor_age: f64,
    /// k = 8π² R⁶ ÷ (3 c³ I), s G⁻²: P Ṗ = k B².
    braking: f64,
    /// The ages, s, between which glitches slow the spin-down (from the unglitched
    /// characteristic age reaching [`GLITCH_AGES`]).
    glitch_window: (f64, f64),
}

impl NeutronStar {
    /// The neutron star of birth period `birth_period`, birth field `birth_field`, `spin_axis`,
    /// magnetic `inclination` from the spin axis (0–π/2) and pulse phase `phase_rank` at its
    /// clock's reference (cycles, 0 to 1), with the default recipe's 12.2 km radius (ruling 27).
    ///
    /// # Panics
    ///
    /// In debug builds, if the period or field is not positive and finite, the inclination is
    /// outside 0–π/2, or the phase outside [0, 1).
    #[must_use]
    pub fn new(
        birth_period: Seconds,
        birth_field: Gauss,
        spin_axis: UnitVector,
        inclination: Radians,
        phase_rank: f64,
    ) -> Self {
        let (p0, b0, alpha) = (
            birth_period.value(),
            birth_field.value(),
            inclination.value(),
        );
        debug_assert!(
            p0.is_finite() && p0 > 0.0 && b0.is_finite() && b0 > 0.0,
            "a neutron star is born spinning with a field: {birth_period:?}, {birth_field:?}"
        );
        debug_assert!(
            (0.0..=FRAC_PI_2).contains(&alpha) && (0.0..1.0).contains(&phase_rank),
            "inclination {alpha} rad, phase {phase_rank}"
        );
        let braking = 8.0 * PI * PI * math::powi(radius_cm(), 6)
            / (3.0 * math::powi(SPEED_OF_LIGHT_CM_S, 3) * MOMENT_OF_INERTIA_G_CM2);
        let floor = b0.min(FIELD_FLOOR.value());
        let decay_time = DECAY_TIME_AT_1E15_G.value() * SECONDS_PER_JULIAN_YEAR * 1.0e15 / b0;
        let floor_age = decay_time * (b0 / floor - 1.0);
        let mut star = Self {
            birth_period: p0,
            birth_field: b0,
            spin_axis,
            inclination: alpha,
            phase_rank,
            floor,
            decay_time,
            floor_age,
            braking,
            glitch_window: (0.0, 0.0),
        };
        star.glitch_window = (
            star.unglitched_age_at_characteristic(GLITCH_AGES.0),
            star.unglitched_age_at_characteristic(GLITCH_AGES.1),
        );
        star
    }

    /// The neutron star a star's draws make (P06.T21.a): its birth period the first of 300 + 150 z
    /// ms over the `star.ns.spin` tries that exceeds 10 ms (300 ms, the mean, if all eight fail,
    /// about once in 4 × 10¹² stars), its field 10^(13.25 + 0.6 z) G from `star.ns.field`, its spin
    /// axis isotropic and its magnetic inclination isotropic about it (cos α uniform) from
    /// `star.ns.geometry`, and its pulse phase from `star.ns.phase`.
    #[must_use]
    pub fn from_draws(draws: &StarDraws) -> Self {
        let period = draws
            .ns_spin()
            .iter()
            .map(|z| {
                math::mul_add(
                    BIRTH_PERIOD_SIGMA.value(),
                    z.value(),
                    BIRTH_PERIOD_MEAN.value(),
                )
            })
            .find(|&p| p > BIRTH_PERIOD_MIN.value())
            .unwrap_or(BIRTH_PERIOD_MEAN.value());
        let field = math::exp10(math::mul_add(
            LOG_BIRTH_FIELD_SIGMA,
            draws.ns_field().value(),
            LOG_BIRTH_FIELD_MEAN,
        ));
        Self::new(
            Seconds::new(period),
            Gauss::new(field),
            draws.ns_spin_axis(),
            Radians::new(math::acos(draws.ns_inclination().value())),
            draws.ns_phase().value(),
        )
    }

    /// The birth spin period P₀.
    #[must_use]
    pub const fn birth_period(&self) -> Seconds {
        Seconds::new(self.birth_period)
    }

    /// The birth dipole field B₀ at the magnetic pole's surface.
    #[must_use]
    pub const fn birth_field(&self) -> Gauss {
        Gauss::new(self.birth_field)
    }

    /// The spin axis, along the galactic axes.
    #[must_use]
    pub const fn spin_axis(&self) -> UnitVector {
        self.spin_axis
    }

    /// The magnetic axis's inclination from the spin axis, 0–π/2.
    #[must_use]
    pub const fn inclination(&self) -> Radians {
        Radians::new(self.inclination)
    }

    /// The braking constant k of P Ṗ = k B², s G⁻²: 8π² R⁶ ÷ (3 c³ I), the orthogonal vacuum
    /// dipole's, 3.2 × 10⁻³⁹ at R = 12.2 km and I = 10⁴⁵ g cm² (the classic 3.2 × 10¹⁹ G of
    /// B = 3.2 × 10¹⁹ √(P Ṗ) is the same law at 10 km).
    #[must_use]
    pub const fn braking_constant(&self) -> f64 {
        self.braking
    }

    /// The field `age` after birth: B₀ ÷ (1 + t ÷ τ), held at its floor once it reaches it.
    #[must_use]
    pub fn field_at(&self, age: Years) -> Gauss {
        Gauss::new(self.field(age_seconds(age)))
    }

    /// The pulsar `age` after birth (clamped at zero): its period and derivative, field, ages,
    /// luminosities and beam.
    #[must_use]
    pub fn state_at(&self, age: Years) -> PulsarState {
        let t = age_seconds(age);
        let field = self.field(t);
        let p2 = self.period_squared(t);
        let period = p2.sqrt();
        let rate = self.braking * field * field * self.glitch_factor(t) / period;
        let spin_down_erg_s =
            4.0 * PI * PI * MOMENT_OF_INERTIA_G_CM2 * rate / (period * period * period);
        let decay_erg_s = self.decay_luminosity_erg_s(t, field);
        PulsarState {
            period: Seconds::new(period),
            period_derivative: rate,
            field: Gauss::new(field),
            characteristic_age: Years::new(period / (2.0 * rate) / SECONDS_PER_JULIAN_YEAR),
            spin_down_luminosity: Watts::new(spin_down_erg_s * WATTS_PER_ERG_S),
            field_decay_luminosity: Watts::new(decay_erg_s * WATTS_PER_ERG_S),
            beam: PulsarBeam {
                spin_axis: self.spin_axis,
                inclination: self.inclination,
                half_angle: beam_half_angle(period),
            },
        }
    }

    /// The pulse clock of this star, whose age since birth at the epoch is `age_at_epoch` (negative
    /// for one born after the epoch): referred to the epoch for a star born by then, with ν, ν̇
    /// and ν̈ at its age then, and otherwise to its birth, with those at birth (plan 06, design
    /// note 23: nothing fast is formed from a difference of two large ages).
    ///
    /// `None` if the birth lies beyond what the clock can hold.
    #[must_use]
    pub fn pulse_clock(&self, age_at_epoch: Years) -> Option<PulseClock> {
        let (reference, age) = if age_at_epoch.value() >= 0.0 {
            (
                UniverseTime::EPOCH,
                age_at_epoch.value() * SECONDS_PER_JULIAN_YEAR,
            )
        } else {
            let birth = Span::from_seconds_f64(-age_at_epoch.value() * SECONDS_PER_JULIAN_YEAR)?;
            (UniverseTime::EPOCH.checked_add(birth)?, 0.0)
        };
        let field = self.field(age);
        let frequency = 1.0 / self.period_squared(age).sqrt();
        let torque = self.glitch_factor(age) * self.braking;
        let cube = frequency * frequency * frequency;
        // ν̇ = −g k B² ν³, and so ν̈ = −g k (2 B Ḃ ν³ + 3 B² ν² ν̇).
        let first = -torque * field * field * cube;
        let field_rate = self.field_rate(age, field);
        let second = -torque
            * (2.0 * field * field_rate * cube
                + 3.0 * field * field * frequency * frequency * first);
        Some(PulseClock {
            reference,
            phase: self.phase_rank,
            nu: frequency,
            nu_dot: first,
            nu_ddot: second,
        })
    }

    /// B at `t` seconds after birth, G.
    #[must_use]
    fn field(&self, t: f64) -> f64 {
        if t >= self.floor_age {
            self.floor
        } else {
            self.birth_field / (1.0 + t / self.decay_time)
        }
    }

    /// dB ÷ dt at `t` seconds after birth, where the field is `field`, G s⁻¹: −B² ÷ (B₀ τ)
    /// while it decays, zero on the floor.
    #[must_use]
    fn field_rate(&self, t: f64, field: f64) -> f64 {
        if t >= self.floor_age {
            0.0
        } else {
            -field * field / (self.birth_field * self.decay_time)
        }
    }

    /// ∫₀ᵗ B² dt', G² s: B₀² τ t ÷ (τ + t) while the field decays, then B(floor)² per second.
    #[must_use]
    fn field_integral(&self, t: f64) -> f64 {
        let b0 = self.birth_field;
        let decaying = |s: f64| b0 * b0 * self.decay_time * s / (self.decay_time + s);
        if t <= self.floor_age {
            decaying(t)
        } else {
            decaying(self.floor_age) + self.floor * self.floor * (t - self.floor_age)
        }
    }

    /// P² at `t` seconds after birth, s²: P₀² + 2k (∫B² − ε ∫ over the glitch window of B²).
    #[must_use]
    fn period_squared(&self, t: f64) -> f64 {
        let (start, end) = self.glitch_window;
        let glitched = self.field_integral(t.clamp(start, end)) - self.field_integral(start);
        self.birth_period * self.birth_period
            + 2.0 * self.braking * math::mul_add(GLITCH_REVERSAL, -glitched, self.field_integral(t))
    }

    /// The factor on the spin-down at `t` seconds: 1 − ε inside the glitch window, 1 outside.
    #[must_use]
    fn glitch_factor(&self, t: f64) -> f64 {
        let (start, end) = self.glitch_window;
        if (start..end).contains(&t) {
            1.0 - GLITCH_REVERSAL
        } else {
            1.0
        }
    }

    /// The age, s, at which the characteristic age P ÷ 2Ṗ of the unglitched spin-down reaches
    /// `target`, or zero if it starts above it.
    ///
    /// The characteristic age rises strictly with age, and has closed forms on both branches of
    /// the field: while the field decays, with x = t ÷ τ and A = P₀² ÷ (2k B₀²), it is
    /// A (1 + x)² + τ x (1 + x), a quadratic in x; on the floor it rises one second per second.
    #[must_use]
    fn unglitched_age_at_characteristic(&self, target: Years) -> f64 {
        let target = target.value() * SECONDS_PER_JULIAN_YEAR;
        let b0 = self.birth_field;
        let a = self.birth_period * self.birth_period / (2.0 * self.braking * b0 * b0);
        if target <= a {
            return 0.0;
        }
        let tau = self.decay_time;
        if self.floor_age > 0.0 {
            // (A + τ) x² + (2A + τ) x − (T − A) = 0, in the form that loses no digits.
            let (b, excess) = (2.0 * a + tau, target - a);
            let x = 2.0 * excess / (b + (b * b + 4.0 * (a + tau) * excess).sqrt());
            let t = x * tau;
            if t <= self.floor_age {
                return t;
            }
        }
        let at_floor = self.unglitched_characteristic_age(self.floor_age);
        self.floor_age + (target - at_floor).max(0.0)
    }

    /// The unglitched characteristic age at `t` seconds after birth, s.
    #[must_use]
    fn unglitched_characteristic_age(&self, t: f64) -> f64 {
        let field = self.field(t);
        let p2 =
            self.birth_period * self.birth_period + 2.0 * self.braking * self.field_integral(t);
        p2 / (2.0 * self.braking * field * field)
    }

    /// The luminosity of the field's decay at `t` seconds, where the field is `field`, erg/s:
    /// −d(B² R³ ÷ 6) ÷ dt = −R³ B Ḃ ÷ 3, the external dipole's energy (as the magnetar models of
    /// Colpi, Geppert and Page 2000 write it; provisional). Zero on the floor.
    #[must_use]
    fn decay_luminosity_erg_s(&self, t: f64, field: f64) -> f64 {
        let radius = radius_cm();
        -radius * radius * radius * field * self.field_rate(t, field) / 3.0
    }
}

/// The radius, cm: the default recipe's 12.2 km (ruling 27).
#[must_use]
fn radius_cm() -> f64 {
    Metres::from(neutron_star_radius(RemnantRecipe::MandelMuller2020)).value() * 100.0
}

/// A neutron star at one age: its spin and spin-down, its field and what they make it
/// (P06.T21.b–c, e).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PulsarState {
    period: Seconds,
    period_derivative: f64,
    field: Gauss,
    characteristic_age: Years,
    spin_down_luminosity: Watts,
    field_decay_luminosity: Watts,
    beam: PulsarBeam,
}

impl PulsarState {
    /// The spin period P, s.
    #[must_use]
    pub const fn period(&self) -> Seconds {
        self.period
    }

    /// The period derivative Ṗ, s s⁻¹, positive: the spin only slows.
    #[must_use]
    pub const fn period_derivative(&self) -> f64 {
        self.period_derivative
    }

    /// The dipole field at the magnetic pole's surface, G.
    #[must_use]
    pub const fn field(&self) -> Gauss {
        self.field
    }

    /// The characteristic age P ÷ 2Ṗ, years: the age a pulsar born fast with a constant field
    /// would have, which an observer quotes.
    #[must_use]
    pub const fn characteristic_age(&self) -> Years {
        self.characteristic_age
    }

    /// The spin-down luminosity Ė = 4π² I Ṗ ÷ P³.
    #[must_use]
    pub const fn spin_down_luminosity(&self) -> Watts {
        self.spin_down_luminosity
    }

    /// The luminosity of the field's decay, −R³ B Ḃ ÷ 3; zero once the field is on its floor.
    #[must_use]
    pub const fn field_decay_luminosity(&self) -> Watts {
        self.field_decay_luminosity
    }

    /// The pulsar's beam.
    #[must_use]
    pub const fn beam(&self) -> PulsarBeam {
        self.beam
    }

    /// Whether it lies above the death line, B ÷ P² > 0.17 × 10¹² G s⁻², and still shines in
    /// the radio (Bhattacharya et al. 1992).
    #[must_use]
    pub fn is_radio_alive(&self) -> bool {
        let p = self.period.value();
        self.field.value() / (p * p) > DEATH_LINE_G_PER_S2
    }

    /// Whether it is a magnetar: a field above the quantum critical field whose decay outshines the
    /// spin-down (P06.T21.c).
    #[must_use]
    pub fn is_magnetar(&self) -> bool {
        self.field > QUANTUM_CRITICAL_FIELD
            && self.field_decay_luminosity > self.spin_down_luminosity
    }

    /// Whether it powers a wind nebula: a spin-down luminosity above 10³⁶ erg/s (P06.T21.e).
    #[must_use]
    pub fn has_wind_nebula(&self) -> bool {
        self.spin_down_luminosity > WIND_NEBULA_THRESHOLD
    }

    /// How it is classified: `MAG` for a magnetar, `PSR` for a radio pulsar above the death line,
    /// and `NS` otherwise (P06.T23.c).
    #[must_use]
    pub fn class(&self) -> NeutronStarClass {
        if self.is_magnetar() {
            NeutronStarClass::Magnetar
        } else if self.is_radio_alive() {
            NeutronStarClass::Pulsar
        } else {
            NeutronStarClass::NeutronStar
        }
    }
}

pub use crate::stellar::classify::NeutronStarClass;

/// A pulsar's two beams: cones of half-angle ρ about the magnetic axis, which lies at the
/// inclination α from the spin axis (P06.T21.c).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PulsarBeam {
    spin_axis: UnitVector,
    /// α, radians.
    inclination: f64,
    /// ρ, radians.
    half_angle: f64,
}

impl PulsarBeam {
    /// The beam's half-angle ρ = 5.4° × (P ÷ s)^(−½), at most 90°.
    #[must_use]
    pub const fn half_angle(&self) -> Radians {
        Radians::new(self.half_angle)
    }

    /// Whether an observer in `direction` (from the pulsar, along the galactic axes) is swept by
    /// either beam: the angle ζ between `direction` and the spin axis lies within ρ of α or of
    /// π − α.
    #[must_use]
    pub fn sweeps(&self, direction: UnitVector) -> bool {
        let zeta = math::acos(direction.dot(&self.spin_axis).clamp(-1.0, 1.0));
        (zeta - self.inclination).abs() <= self.half_angle
            || (zeta - (PI - self.inclination)).abs() <= self.half_angle
    }
}

/// The half-angle of the beam at period `period` seconds, radians.
#[must_use]
fn beam_half_angle(period: f64) -> f64 {
    (BEAM_HALF_ANGLE_AT_1_S * RADIANS_PER_DEGREE / period.sqrt()).min(FRAC_PI_2)
}

/// A pulsar's rotation phase as a function of clock time (P06.T21.d): the phase at a reference
/// instant from one draw, then φ(t) = φ₀ + ν Δt + ½ ν̇ Δt² + ⅙ ν̈ Δt³ with ν, ν̇ and ν̈ at the
/// reference.
///
/// Δt is taken from the clock's integer seconds and nanoseconds, never from a difference of ages,
/// and ν Δt is formed in split whole and fractional parts, so that the rounding error stays below
/// 10⁻⁹ cycles at any Δt within the clock window. The phase is this series by definition: its
/// rate ν + ν̇ Δt + ½ ν̈ Δt² departs from the closed-form spin-down's 1 ÷ P by a relative
/// ν⃛ Δt³ ÷ 6ν, under 10⁻⁴ within the window even for a young pulsar (P of 0.1 s and Ṗ of 10⁻¹³,
/// ν⃛ ≈ 15 ν̇³ ÷ ν²). Integrated, that is ν⃛ Δt⁴ ÷ 24 cycles, which for such a pulsar is about
/// one cycle at 20 years and millions at 1,000: the phase is a consistent clock, not the integral
/// of [`NeutronStar::state_at`]'s period. Beyond the window the function still returns, and the
/// departure grows as Δt⁴.
///
/// # Examples
///
/// ```
/// use hyperion_sim::coords::UnitVector;
/// use hyperion_sim::stellar::remnant::NeutronStar;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::{Gauss, Radians, Seconds, Years};
///
/// let ns = NeutronStar::new(
///     Seconds::new(0.3),
///     Gauss::new(1e12),
///     UnitVector::NORTH,
///     Radians::new(0.5),
///     0.25,
/// );
/// let clock = ns.pulse_clock(Years::new(1e6)).ok_or("born within the clock's range")?;
/// assert_eq!(clock.phase_at(UniverseTime::EPOCH), 0.25);
/// let later = clock.phase_at(UniverseTime::new(10, 0)?);
/// assert!((0.0..1.0).contains(&later));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PulseClock {
    reference: UniverseTime,
    phase: f64,
    nu: f64,
    nu_dot: f64,
    nu_ddot: f64,
}

impl PulseClock {
    /// The reference instant: the epoch, or the birth of a star born after it.
    #[must_use]
    pub const fn reference(&self) -> UniverseTime {
        self.reference
    }

    /// The spin frequency ν at the reference, Hz.
    #[must_use]
    pub const fn frequency(&self) -> f64 {
        self.nu
    }

    /// Its first derivative ν̇ at the reference, Hz s⁻¹, negative.
    #[must_use]
    pub const fn frequency_derivative(&self) -> f64 {
        self.nu_dot
    }

    /// Its second derivative ν̈ at the reference, Hz s⁻².
    #[must_use]
    pub const fn frequency_second_derivative(&self) -> f64 {
        self.nu_ddot
    }

    /// The rotation phase at `t`, cycles in [0, 1).
    #[must_use]
    pub fn phase_at(&self, t: UniverseTime) -> f64 {
        let span = t.checked_since(self.reference).unwrap_or(Span::ZERO);
        #[expect(
            clippy::cast_precision_loss,
            reason = "the whole seconds of any span of the clock are below 2^53, so exact"
        )]
        let whole = span.seconds() as f64;
        let fraction = f64::from(span.subsec_nanos()) * 1e-9;
        // ν × whole exactly, as a rounded product and its error (the product of two doubles is
        // exactly the sum of the two): only the product's fractional part matters.
        let product = self.nu * whole;
        let error = math::mul_add(self.nu, whole, -product);
        let product_cycles = product - product.floor();
        let dt = whole + fraction;
        let series = dt * dt * math::mul_add(self.nu_ddot / 6.0, dt, 0.5 * self.nu_dot);
        let phase = self.phase + product_cycles + error + self.nu * fraction + series;
        let wrapped = phase - phase.floor();
        if wrapped >= 1.0 { 0.0 } else { wrapped }
    }
}

/// `age` in seconds, clamped at zero.
#[must_use]
fn age_seconds(age: Years) -> f64 {
    age.value().max(0.0) * SECONDS_PER_JULIAN_YEAR
}

/// The luminosity of a neutron star of `mass`, `age` after its birth, by HPT's photon-cooling law
/// (equation 93): L = 0.02 M^(2/3) ÷ max(t, 0.1)² L☉ with t in Myr, constant for the first 10⁵
/// years, which HPT calibrate to an effective temperature of about 2 × 10⁶ K for the Crab pulsar.
///
/// # Panics
///
/// In debug builds, if `mass` is not positive or `age` is negative.
#[must_use]
pub(crate) fn hpt_luminosity(mass: SolarMasses, age: Years) -> SolarLuminosities {
    debug_assert!(
        mass.value() > 0.0 && age.value() >= 0.0,
        "a neutron star of {mass:?} at {age:?}"
    );
    let t_myr = (age.value() * 1e-6).max(0.1);
    SolarLuminosities::new(0.02 * math::powf(mass.value(), 2.0 / 3.0) / (t_myr * t_myr))
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::lcg::Lcg;

    use super::*;
    use crate::stellar::draws::{StandardNormal, StarDrawsParts, UnitUniform};

    /// 0.02 × 1.4^⅔ ÷ 0.1² = 2.50 L☉ for the first 10⁵ years, then falling as t⁻².
    #[test]
    fn a_neutron_star_cools_as_the_inverse_square_of_its_age() {
        let m = SolarMasses::new(1.4);
        let young = hpt_luminosity(m, Years::new(5e4)).value();
        assert!(
            (young - 2.0 * math::powf(1.4, 2.0 / 3.0)).abs() < 1e-12,
            "{young}"
        );
        let same = hpt_luminosity(m, Years::new(1e5)).value();
        assert!((same / young - 1.0).abs() < 1e-15);
        let older = hpt_luminosity(m, Years::new(1e6)).value();
        assert!((older * 100.0 / young - 1.0).abs() < 1e-12, "{older}");
    }

    /// A uniform on the open interval (0, 1).
    fn open_unit(rng: &mut Lcg) -> f64 {
        (rng.next_f64() + f64::EPSILON) / (1.0 + 2.0 * f64::EPSILON)
    }

    fn star(p0: f64, log_b0: f64) -> NeutronStar {
        NeutronStar::new(
            Seconds::new(p0),
            Gauss::new(math::exp10(log_b0)),
            UnitVector::NORTH,
            Radians::new(1.0),
            0.5,
        )
    }

    /// A population drawn as `from_draws` draws, from explicit variates.
    fn population(n: usize, seed: u64) -> Vec<NeutronStar> {
        let mut rng = Lcg::new(seed);
        (0..n)
            .map(|_| {
                let mut normal = || {
                    StandardNormal::new(math::normal_quantile(open_unit(&mut rng))).expect("finite")
                };
                let ns_spin = core::array::from_fn(|_| normal());
                let ns_field = normal();
                let z = 2.0 * open_unit(&mut rng) - 1.0;
                let phi = 2.0 * PI * open_unit(&mut rng);
                let s = (1.0 - z * z).sqrt();
                let axis = UnitVector::from_components([s * math::cos(phi), s * math::sin(phi), z])
                    .expect("a point of the sphere");
                NeutronStar::from_draws(&StarDraws::from_parts(StarDrawsParts {
                    ns_spin,
                    ns_field,
                    ns_spin_axis: axis,
                    ns_inclination: UnitUniform::new(open_unit(&mut rng)).expect("open"),
                    ns_phase: UnitUniform::new(open_unit(&mut rng)).expect("open"),
                    ..StarDrawsParts::MEDIAN
                }))
            })
            .collect()
    }

    /// The median draws: 300 ms, 10^13.25 G, inclination 60°.
    #[test]
    fn the_median_star_is_born_at_the_means() {
        let ns = NeutronStar::from_draws(&StarDraws::median());
        assert!((ns.birth_period().value() - 0.3).abs() < 1e-15);
        assert!((math::log10(ns.birth_field().value()) - 13.25).abs() < 1e-12);
        assert!((ns.inclination().value() - PI / 3.0).abs() < 1e-12);
    }

    /// A birth period at or under 10 ms is redrawn from the next try.
    #[test]
    fn a_birth_period_under_ten_milliseconds_is_redrawn() {
        let z = |v| StandardNormal::new(v).expect("finite");
        let mut ns_spin = [z(0.0); crate::stellar::draws::REDRAW_TRIES];
        ns_spin[0] = z(-2.5);
        ns_spin[1] = z(-1.0);
        let ns = NeutronStar::from_draws(&StarDraws::from_parts(StarDrawsParts {
            ns_spin,
            ..StarDrawsParts::MEDIAN
        }));
        assert!((ns.birth_period().value() - 0.15).abs() < 1e-15);
    }

    /// The braking constant is the orthogonal vacuum dipole's at 12.2 km: 3.2 × 10⁻³⁹ s G⁻², the
    /// classic 1 ÷ (3.2 × 10¹⁹)² scaled by (1.22)⁶.
    #[test]
    fn the_braking_constant_is_the_vacuum_dipoles_at_twelve_kilometres() {
        let k = star(0.3, 12.0).braking_constant();
        let classic = 1.0 / (3.2e19 * 3.2e19) * math::powi(1.22, 6);
        assert!(
            (k / classic - 1.0).abs() < 0.01,
            "{k:e} against {classic:e}"
        );
    }

    /// The plan's test: P is continuous and non-decreasing in age, across the floor and the glitch
    /// window, for every star of a sample.
    #[test]
    fn the_period_is_continuous_and_never_falls() {
        for ns in population(200, 7) {
            let mut last = ns.state_at(Years::ZERO).period().value();
            assert!((last - ns.birth_period().value()).abs() < 1e-15);
            for i in 1..=2_000 {
                let age = Years::new(math::exp10(f64::from(i) * 0.004));
                let p = ns.state_at(age).period().value();
                assert!(p >= last, "{ns:?} at {age:?}: {p} after {last}");
                last = p;
            }
            // Continuity at the two edges of the glitch window and at the floor.
            for edge in [ns.glitch_window.0, ns.glitch_window.1, ns.floor_age] {
                let at = |t: f64| ns.period_squared(t).sqrt();
                let (before, after) = (at(edge * (1.0 - 1e-12)), at(edge * (1.0 + 1e-12)));
                assert!((after - before).abs() <= 1e-9 * before, "{ns:?} at {edge}");
            }
        }
    }

    /// The plan's test: with a constant field (a birth field under the floor) the closed form is
    /// √(P₀² + 2kB²t).
    #[test]
    fn with_a_constant_field_the_period_is_the_square_root_law() {
        let ns = star(0.3, 11.0);
        let k = ns.braking_constant();
        assert_eq!(ns.glitch_window, (0.0, 0.0), "τ_c starts above 10⁵ yr");
        for years in [1.0e3, 1.0e5, 1.0e7, 1.0e9] {
            let t = years * SECONDS_PER_JULIAN_YEAR;
            let expected = (0.09 + 2.0 * k * 1e22 * t).sqrt();
            let p = ns.state_at(Years::new(years)).period().value();
            assert!(
                (p / expected - 1.0).abs() < 1e-12,
                "{years} yr: {p} against {expected}"
            );
        }
    }

    /// The plan's test: a 10¹²·⁵ G pulsar born at 300 ms dies after 10⁷–10⁸ years.
    #[test]
    fn a_typical_pulsar_dies_after_ten_to_a_hundred_million_years() {
        let ns = star(0.3, 12.5);
        assert!(ns.state_at(Years::new(1.0e7)).is_radio_alive());
        assert!(!ns.state_at(Years::new(1.0e8)).is_radio_alive());
        assert_eq!(
            ns.state_at(Years::new(1.0e6)).class(),
            NeutronStarClass::Pulsar
        );
        assert_eq!(
            ns.state_at(Years::new(1.0e9)).class(),
            NeutronStarClass::NeutronStar
        );
    }

    /// The field decays as B₀ ÷ (1 + t ÷ τ) to its floor, and not below.
    #[test]
    fn the_field_decays_to_its_floor() {
        let ns = star(0.3, 15.0);
        let at = |years: f64| ns.field_at(Years::new(years)).value();
        assert!((at(0.0) - 1e15).abs() < 1.0);
        assert!((at(1.0e4) / 5e14 - 1.0).abs() < 1e-12);
        assert!((at(1.0e9) - 1e12).abs() < 1.0);
        let weak = star(0.3, 11.5);
        assert!((weak.field_at(Years::new(1.0e9)).value() / math::exp10(11.5) - 1.0).abs() < 1e-12);
    }

    /// Glitches take 1% off the spin-down while the characteristic age is 10³–10⁵ years.
    #[test]
    fn glitches_slow_the_spin_down_by_a_percent_in_the_window() {
        let ns = star(0.02, 12.5);
        let (start, end) = ns.glitch_window;
        assert!(start > 0.0 && end > start, "{:?}", ns.glitch_window);
        let tau_c = |t: f64| ns.unglitched_characteristic_age(t) / SECONDS_PER_JULIAN_YEAR;
        assert!((tau_c(start) / 1e3 - 1.0).abs() < 1e-9, "{}", tau_c(start));
        assert!((tau_c(end) / 1e5 - 1.0).abs() < 1e-9, "{}", tau_c(end));
        let mid = f64::midpoint(start, end);
        let inside = ns.state_at(Years::new(mid / SECONDS_PER_JULIAN_YEAR));
        let p = inside.period().value();
        let unglitched = ns.braking * ns.field(mid) * ns.field(mid) / p;
        assert!((inside.period_derivative() / unglitched - 0.99).abs() < 1e-12);
    }

    /// The characteristic age's closed form on the floor branch: a weak-field star's window.
    #[test]
    fn the_glitch_window_is_found_on_the_floor_too() {
        let ns = star(0.005, 11.8);
        let tau_c = |t: f64| ns.unglitched_characteristic_age(t) / SECONDS_PER_JULIAN_YEAR;
        let (start, end) = ns.glitch_window;
        assert!((tau_c(start) / 1e3 - 1.0).abs() < 1e-9);
        assert!((tau_c(end) / 1e5 - 1.0).abs() < 1e-9);
    }

    /// The plan's tests on the birth field and magnetars: 15–40% of neutron stars are born above
    /// 4.4 × 10¹³ G, none of them is a magnetar at birth (its spin-down outshines its field's
    /// decay), and at two core collapses a century the Galaxy holds 20–300 active magnetars.
    #[test]
    fn magnetars_are_the_high_tail_of_the_birth_field() {
        let stars = population(20_000, 11);
        #[expect(clippy::cast_precision_loss, reason = "counts of a few thousand")]
        let share = |k: usize| k as f64 / stars.len() as f64;
        let strong = stars
            .iter()
            .filter(|ns| ns.birth_field() > QUANTUM_CRITICAL_FIELD)
            .count();
        assert!((0.15..=0.40).contains(&share(strong)), "{}", share(strong));
        assert!(
            stars
                .iter()
                .all(|ns| !ns.state_at(Years::ZERO).is_magnetar())
        );
        // The mean time a star is a magnetar, from a log-spaced sweep of ages to 10 Myr.
        let edges: Vec<f64> = (0..=700)
            .map(|i| math::exp10(f64::from(i) * 0.01))
            .collect();
        let mean_years: f64 = stars
            .iter()
            .map(|ns| {
                edges
                    .windows(2)
                    .filter(|w| ns.state_at(Years::new((w[0] * w[1]).sqrt())).is_magnetar())
                    .map(|w| w[1] - w[0])
                    .sum::<f64>()
            })
            .sum::<f64>()
            / f64::from(u32::try_from(stars.len()).unwrap());
        // Two core collapses a century, of which 62% leave a neutron star (plan 06, P06.T31's
        // 62:38 of neutron stars to black holes; P06.T18's 38 ± 5% black holes).
        let active = 0.02 * 0.62 * mean_years;
        assert!(
            (20.0..=300.0).contains(&active),
            "{active} magnetars, {mean_years} yr each"
        );
    }

    /// Design note 13's re-check: born at a steady rate over 100 Myr, where the living radio
    /// pulsars sit in the P–Ṗ plane. The ATNF catalogue's non-recycled pulsars have a median
    /// period of about 0.6 s and a median Ṗ of about 10⁻¹⁴·⁷ (Manchester et al. 2005, AJ 129,
    /// 1993). The plan's pair gives a median of 2.0 s and 10⁻¹⁴·⁶: the derivative agrees and the
    /// period is three times long, a finding reported for a ruling (Popov et al.'s field is taken
    /// at the pole and the braking constant is the equatorial one at 12.2 km, so the braking is
    /// about thirteen times Popov et al.'s model: four from the field's definition and 3.3 from
    /// R⁶ against their 10 km). The period band is held at 0.2–3 s until the ruling.
    #[test]
    fn living_pulsars_sit_where_observed_ones_do() {
        let stars = population(20_000, 13);
        let mut rng = Lcg::new(29);
        let (mut periods, mut derivatives) = (Vec::new(), Vec::new());
        for ns in &stars {
            let age = Years::new(1.0e8 * open_unit(&mut rng));
            let pulsar = ns.state_at(age);
            if pulsar.is_radio_alive() && !pulsar.is_magnetar() {
                periods.push(pulsar.period().value());
                derivatives.push(math::log10(pulsar.period_derivative()));
            }
        }
        periods.sort_by(f64::total_cmp);
        derivatives.sort_by(f64::total_cmp);
        let (p, pdot) = (
            periods[periods.len() / 2],
            derivatives[derivatives.len() / 2],
        );
        let alive = f64::from(u32::try_from(periods.len()).unwrap()) / 20_000.0;
        assert!(
            (0.2..=3.0).contains(&p) && (-15.2..=-14.2).contains(&pdot),
            "median P {p} s, median log Pdot {pdot}, {alive} of stars alive"
        );
    }

    /// The plan's test: over random inclinations and viewing directions, a 1 s pulsar is seen
    /// by 10–20% of observers.
    #[test]
    fn a_one_second_pulsar_beams_at_ten_to_twenty_percent_of_the_sky() {
        let mut rng = Lcg::new(3);
        let n = 200_000;
        let mut seen = 0_u32;
        for _ in 0..n {
            let z = 2.0 * open_unit(&mut rng) - 1.0;
            let phi = 2.0 * PI * open_unit(&mut rng);
            let s = (1.0 - z * z).sqrt();
            let direction =
                UnitVector::from_components([s * math::cos(phi), s * math::sin(phi), z]).unwrap();
            let beam = PulsarBeam {
                spin_axis: UnitVector::NORTH,
                inclination: math::acos(open_unit(&mut rng)),
                half_angle: beam_half_angle(1.0),
            };
            seen += u32::from(beam.sweeps(direction));
        }
        let fraction = f64::from(seen) / f64::from(n);
        assert!((0.10..=0.20).contains(&fraction), "{fraction}");
    }

    /// The plan's test: the wind-nebula flag lasts 10³–10⁵·⁵ years across the birth distribution
    /// (the median over the stars born with one).
    #[test]
    fn a_wind_nebula_lasts_a_thousand_to_three_hundred_thousand_years() {
        let stars = population(5_000, 5);
        let mut durations: Vec<f64> = stars
            .iter()
            .filter(|ns| ns.state_at(Years::ZERO).has_wind_nebula())
            .map(|ns| {
                // Ė falls with age, so the flag ends at one root, found by bisection in log age.
                let (mut lo, mut hi) = (0.0_f64, 9.0_f64);
                for _ in 0..60 {
                    let mid = f64::midpoint(lo, hi);
                    if ns.state_at(Years::new(math::exp10(mid))).has_wind_nebula() {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                math::exp10(lo)
            })
            .collect();
        assert!(
            durations.len() > 100,
            "{} born with a nebula",
            durations.len()
        );
        durations.sort_by(f64::total_cmp);
        let median = durations[durations.len() / 2];
        assert!((1.0e3..=math::exp10(5.5)).contains(&median), "{median} yr");
    }

    /// The pulse phase at the reference is the drawn phase, and forward of it follows ν t with
    /// the spin-down terms.
    #[test]
    fn the_pulse_phase_follows_the_spin() {
        let ns = star(0.1, 12.0);
        let clock = ns.pulse_clock(Years::new(1.0e5)).unwrap();
        assert_eq!(clock.reference(), UniverseTime::EPOCH);
        assert!((clock.phase_at(UniverseTime::EPOCH) - 0.5).abs() < 1e-15);
        let p = ns.state_at(Years::new(1.0e5)).period().value();
        assert!((clock.frequency() * p - 1.0).abs() < 1e-12);
        // A hundred seconds on, ν t alone to 10⁻⁶ cycles (ν̇ t² is ~10⁻¹¹ Hz s⁻¹ × 10⁴ s²).
        let t = UniverseTime::new(100, 0).unwrap();
        let expected = (0.5 + clock.frequency() * 100.0).rem_euclid(1.0);
        let phase = clock.phase_at(t);
        let d = (phase - expected).abs();
        assert!(d.min(1.0 - d) < 1e-6, "{phase} against {expected}");
    }

    /// The split product keeps the phase to 10⁻³ cycles over the clock window: against the same
    /// series in exact rational arithmetic on the integer seconds (u128), a millisecond pulsar
    /// 1,000 years on.
    #[test]
    fn the_pulse_phase_is_precise_over_the_clock_window() {
        let clock = PulseClock {
            reference: UniverseTime::EPOCH,
            phase: 0.0,
            // 2⁻⁸ Hz × 2⁻³⁰: an exact binary frequency near 640 Hz so that ν × s is exact.
            nu: 640.0 + math::exp2(-30.0),
            nu_dot: 0.0,
            nu_ddot: 0.0,
        };
        let seconds: i64 = 1_000 * 31_557_600;
        let t = UniverseTime::new(seconds, 0).unwrap();
        // ν s = 640 s + s ÷ 2³⁰: the fractional part is (s mod 2³⁰) ÷ 2³⁰.
        let s = u128::try_from(seconds).unwrap();
        #[expect(clippy::cast_precision_loss, reason = "below 2^30, exact")]
        let exact = (s % (1_u128 << 30)) as f64 / f64::from(1_u32 << 30);
        let phase = clock.phase_at(t);
        let d = (phase - exact).abs();
        assert!(d.min(1.0 - d) < 1e-9, "{phase} against {exact}");
    }

    /// A star born after the epoch is clocked from its birth.
    #[test]
    fn a_star_born_after_the_epoch_is_clocked_from_its_birth() {
        let ns = star(0.3, 12.0);
        let clock = ns.pulse_clock(Years::new(-10.0)).unwrap();
        assert_eq!(
            clock.reference(),
            UniverseTime::new(10 * 31_557_600, 0).unwrap()
        );
        assert!((clock.frequency() - 1.0 / 0.3).abs() < 1e-12);
    }
}
