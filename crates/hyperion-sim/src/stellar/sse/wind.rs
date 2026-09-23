//! Mass loss by stellar winds (plan 06, P06.T10.a and T10.b): the prescription of Hurley, Pols and
//! Tout (2000, MNRAS 315, 543; "HPT", section 7.1) and the modern set that current
//! population-synthesis codes use (plan 06, design note 6), behind one function, [`rate`].
//!
//! Every term is a closed form in the star's present state: bolometric luminosity L in L☉, radius R
//! in R☉, current mass M and core mass Mc in M☉, effective temperature in K, and the phase, which
//! decides which terms apply. The metal fraction is the one the backbone's formulae see,
//! [`Composition::z_fit`] (plan 06, design note 5), and enters as Z ÷ Z☉ with Z☉ = 0.02, so that
//! a star of solar \[Fe/H\] is solar to every term. The private helpers take bare `f64`s in those
//! units and return M☉ per Julian year.
//!
//! # `WindRecipe::Hurley2000`
//!
//! HPT section 7.1, as the published SSE code (`mlwind.f`) applies it to a single star. Four
//! places differ between the printed text and the code, and in each the text's form moves the rate
//! by far more than the tolerances P06.T12.b validates the backbone to (1% in mass, 0.02 dex in L
//! and R), so the code's form is used, for the owner to confirm (plan 06, ruling 10 of
//! 2026-09-22); each is named again where it acts:
//!
//! - Nieuwenhuijzen and de Jager's rate is switched on at L = 4,000 L☉ by a linear ramp over
//!   4,000–4,500 L☉ ([`nieuwenhuijzen_de_jager`]). The text switches it on in full at 4,000 L☉, a
//!   jump of the whole term (about 10⁻⁹ M☉ per year for a 7 M☉ star) that the ramp removes.
//! - Kudritzki and Reimers' rate applies from the Hertzsprung gap on (SSE types 2–9), the text
//!   says "on the GB and beyond". Below 4,000 L☉ it is the whole rate of a gap star.
//! - The Mira period is capped at 2,000 days ([`vassiliadis_wood`]), where the text writes
//!   log P₀ ≤ 3.3, the rounded logarithm: where the cap binds and the superwind's does not, the
//!   rounded form is 0.059 dex lower.
//! - The luminous-blue-variable term acts from the Hertzsprung gap to the thermally pulsing AGB
//!   (types 2–6), not on the main sequence, where the text does not restrict it (it is reached
//!   there only by the most massive stars near their terminal main sequence).
//!
//! SSE scales the naked-helium-star rate by an input `hewind`, which its distributed input file
//! sets to 0.5 while its source comment calls 1.0 normal; the paper has no such factor, and this
//! recipe uses 1.0.
//!
//! # `WindRecipe::Modern`
//!
//! Design note 6's set, as Belczynski et al. (2010, ApJ 714, 1217, section 2.2) assemble it:
//!
//! - Hot hydrogen-rich stars follow Vink, de Koter and Lamers (2001, A&A 369, 574; "Vink"),
//!   equations 24 and 25, alone: from 12,500 K Vink's rate takes the place of every HPT term,
//!   and HPT's rate is handed over to it across 11,500–12,500 K ([`hot_or_cool`]). This is
//!   design note 6's "Hurley's own choices elsewhere" and Belczynski et al.'s "for H-rich low
//!   mass stars, for which the above prescriptions do not apply, we use Hurley et al. (2000)
//!   winds": a hot star with a thin envelope loses mass at Vink's rate, not by HPT's
//!   small-envelope term. Vink fitted models of log L = 5.0–6.0, 20–60 M☉ and Z ÷ Z☉ =
//!   1/30–3 (their Table 2); like Belczynski et al., who apply the fits from about 3 M☉ at the
//!   zero-age main sequence, the recipe applies them to every hot hydrogen-rich star and to
//!   Z ÷ Z☉ down to 0.005, the floor of `z_fit`. Below log L = 5 the rates are too small to
//!   move a track (about 10⁻¹⁰ M☉ per year for a 5 M☉ B star of 800 L☉). The bi-stability jump is
//!   bridged across the fixed band of the plan, 22,500–27,500 K, which holds the jump's position
//!   by Vink's equations 14–15 (about 25,900 K at solar Z, 22,500 K at 1/30 Z☉), and the second,
//!   smaller jump near 15,000 K at high Z (their section 5.2) is left out, as Belczynski et al.
//!   leave it.
//! - An evolved hydrogen-rich star beyond the Humphreys–Davidson limit loses 1.5 × 10⁻⁴ M☉ per
//!   year, independent of metallicity, as the whole of its wind (Belczynski et al. 2010,
//!   equation 8 with `f_lbv` = 1.5).
//! - Naked helium stars follow Hamann and Koesterke (1998, A&A 335, 1003) in HPT's reduced form
//!   10⁻¹³ L^1.5, scaled as (Z ÷ Z☉)^0.86 (Vink and de Koter 2005, A&A 442, 587, for WN stars
//!   over 10⁻³ ≲ Z ÷ Z☉ ≲ 1), which is Belczynski et al. (2010) equation 9.
//! - Cooler hydrogen-rich stars keep HPT's own rate: Kudritzki and Reimers on the giant branch
//!   and beyond, Vassiliadis and Wood on the AGB, Nieuwenhuijzen and de Jager for luminous stars
//!   and the Wolf-Rayet-like rate of small envelopes, combined by the largest as HPT combine
//!   them, without HPT's luminous-blue-variable term.
//!
//! The recipe is continuous in effective temperature: the bi-stability jump and the hand-over
//! from the cool rate to Vink's are linear blends across a band (see [`vink`] and
//! [`hot_or_cool`]). The one step left is the Humphreys–Davidson limit, where Belczynski et al.'s
//! constant rate switches on by design, about six times the rate just inside it for a 60 M☉ star
//! of 10⁶ L☉. Between phases the rate may step, as in the codes the recipe follows: at low Z most
//! of all where a giant cooler than 11,500 K is stripped to a naked helium star, since HPT's
//! small-envelope term carries no Z and the helium stars' rate carries Z^0.86 (a fall of about 95
//! times at Z = 10⁻⁴ and 7 at 0.002).

// The track integrator of P06.T10.c is the first caller outside tests.
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the track integrator of P06.T10.c is the first caller"
    )
)]

use crate::math;
use crate::stellar::composition::Z_SOLAR;
use crate::stellar::{Composition, Phase, StarState};
use crate::units::{SolarLuminosities, SolarMasses, SolarMassesPerYear};

/// Which set of wind prescriptions a track integrates (plan 06, design note 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum WindRecipe {
    /// Hurley, Pols and Tout (2000, section 7.1) as the published SSE code applies it, kept so
    /// that the backbone can be validated against SSE's output (P06.T12.b).
    Hurley2000,
    /// The generator's default: Vink, de Koter and Lamers (2001) for hot hydrogen-rich stars, the
    /// luminous-blue-variable rate of Belczynski et al. (2010) beyond the Humphreys–Davidson
    /// limit, Hamann and Koesterke (1998) scaled as Z^0.86 (Vink and de Koter 2005) for naked
    /// helium stars, and Hurley, Pols and Tout's prescriptions elsewhere.
    #[default]
    Modern,
}

/// Reimers' η, the dimensionless efficiency of the Kudritzki–Reimers wind (HPT equation 106):
/// finite and non-negative.
///
/// HPT fix it at 0.5; the generator draws it per star (plan 06, design note 7).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub(crate) struct ReimersEta(f64);

impl ReimersEta {
    /// HPT's value, η = 0.5 (section 7.1), which SSE's distributed input file also uses.
    pub(crate) const HURLEY: Self = Self(0.5);

    /// Wraps a value of η.
    ///
    /// # Panics
    ///
    /// In debug builds, if `eta` is not finite or is negative.
    #[must_use]
    pub(crate) fn new(eta: f64) -> Self {
        debug_assert!(
            eta.is_finite() && eta >= 0.0,
            "Reimers η must be finite and non-negative: {eta}"
        );
        Self(eta)
    }

    /// The value of η.
    #[must_use]
    pub(crate) const fn value(self) -> f64 {
        self.0
    }
}

/// The rate at which a star in `state`, of composition `composition`, loses mass to its wind under
/// `recipe`, with Reimers' `eta`: M☉ per Julian year, non-negative, positive for loss as in
/// [`StarStateParts::mass_loss_rate`](crate::stellar::StarStateParts::mass_loss_rate).
///
/// Remnants, protostars, pre-main-sequence stars, substellar objects and the post-AGB crossing
/// have none: HPT's recipe starts on the main sequence, and the post-AGB crossing begins when the
/// envelope is gone. The terms and how they combine are those of the module documentation.
///
/// # Panics
///
/// In debug builds, if a star with a wind has no mass.
#[must_use]
pub(crate) fn rate(
    recipe: WindRecipe,
    state: &StarState,
    composition: &Composition,
    eta: ReimersEta,
) -> SolarMassesPerYear {
    let regime = Regime::of(state.phase());
    if regime == Regime::Windless {
        return SolarMassesPerYear::ZERO;
    }
    let s = Surface::of(state, composition);
    debug_assert!(s.m > 0.0, "a star with a wind has mass: {state:?}");
    SolarMassesPerYear::new(match recipe {
        WindRecipe::Hurley2000 => hurley(regime, &s, eta),
        WindRecipe::Modern => modern(regime, &s, eta),
    })
}

/// μ of HPT equation 97: ((M − Mc) ÷ M) × min(5, max(1.2, (L ÷ L₀)^κ)), with L₀ = 7 × 10⁴ L☉
/// and κ = −0.5, a measure of how small a giant's envelope is. Below 1 the envelope is small
/// enough that the star starts to show its core: the Wolf-Rayet-like wind acts (section 7.1),
/// and section 6.3 perturbs L and R towards the core's (for P06.T10.d). HPT define it for every
/// phase with a core and an envelope, not the main sequence; helium giants take equation 98
/// instead.
///
/// # Panics
///
/// In debug builds, if `mass` is not positive.
#[must_use]
pub(crate) fn small_envelope_mu(
    mass: SolarMasses,
    core_mass: SolarMasses,
    luminosity: SolarLuminosities,
) -> f64 {
    let (m, mc, l) = (mass.value(), core_mass.value(), luminosity.value());
    debug_assert!(m > 0.0, "μ needs a positive mass: {m}");
    ((m - mc) / m)
        * MU_CLAMP
            .1
            .min(MU_CLAMP.0.max(math::powf(l / MU_L0, MU_KAPPA)))
}

/// Kudritzki and Reimers' coefficient, M☉ yr⁻¹ per (L☉ R☉ ÷ M☉): `Ṁ_R` = η × 4 × 10⁻¹³ L R ÷ M
/// (HPT equation 106).
const REIMERS: f64 = 4e-13;

/// Vassiliadis and Wood's rate as HPT write it: log `Ṁ_VW` = −11.4 + 0.0125 [P₀ − 100
/// max(M − 2.5, 0)], P₀ in days.
const VW_LOG_BASE: f64 = -11.4;

/// See [`VW_LOG_BASE`]: dex per day of period.
const VW_PERIOD_SLOPE: f64 = 0.0125;

/// See [`VW_LOG_BASE`]: days of period per M☉ above 2.5 M☉.
const VW_MASS_DAYS: f64 = 100.0;

/// See [`VW_LOG_BASE`], M☉.
const VW_MASS_OFFSET: f64 = 2.5;

/// The steady superwind's rate per L☉, M☉ yr⁻¹, the cap on `Ṁ_VW` (HPT section 7.1).
const SUPERWIND_PER_LUMINOSITY: f64 = 1.36e-9;

/// The Mira pulsation period, log P₀ = −2.07 − 0.9 log M + 1.94 log R, P₀ in days (HPT section
/// 7.1, after Vassiliadis and Wood 1993).
const MIRA_LOG_PERIOD: (f64, f64, f64) = (-2.07, -0.9, 1.94);

/// The cap on P₀, days: SSE's value, which HPT print as log P₀ ≤ 3.3.
const MIRA_PERIOD_CAP_DAYS: f64 = 2_000.0;

/// Nieuwenhuijzen and de Jager's (1990) rate as HPT scale it: (Z ÷ Z☉)^½ × 9.6 × 10⁻¹⁵
/// R^0.81 L^1.24 M^0.16 M☉ yr⁻¹.
const NJ_COEFFICIENT: f64 = 9.6e-15;

/// The powers of R, L and M in [`NJ_COEFFICIENT`]'s rate.
const NJ_POWERS: (f64, f64, f64) = (0.81, 1.24, 0.16);

/// Above this luminosity, L☉, the rate of [`NJ_COEFFICIENT`] applies (HPT section 7.1).
const NJ_LUMINOSITY: f64 = 4_000.0;

/// The width, L☉, of SSE's ramp by which Nieuwenhuijzen and de Jager's rate switches on above
/// [`NJ_LUMINOSITY`].
const NJ_RAMP: f64 = 500.0;

/// HPT's reduced Wolf-Rayet rate, 10⁻¹³ L^1.5 M☉ yr⁻¹ (section 7.1, after Hamann, Koesterke and
/// Wessolowski 1995 and Hamann and Koesterke 1998).
const WOLF_RAYET: f64 = 1e-13;

/// L₀ of HPT equation 97, L☉.
const MU_L0: f64 = 7e4;

/// κ of HPT equation 97.
const MU_KAPPA: f64 = -0.5;

/// The clamp of HPT equation 97's luminosity factor, (min, max).
const MU_CLAMP: (f64, f64) = (1.2, 5.0);

/// The Humphreys–Davidson limit as HPT write it: L > 6 × 10⁵ L☉ and 10⁻⁵ R L^½ > 1 (section 7.1,
/// after Humphreys and Davidson 1994).
const HD_LUMINOSITY: f64 = 6e5;

/// See [`HD_LUMINOSITY`]: per R☉ L☉^½.
const HD_RADIUS_SCALE: f64 = 1e-5;

/// HPT's luminous-blue-variable rate, 0.1 (10⁻⁵ R L^½ − 1)³ (L ÷ 6 × 10⁵ − 1) M☉ yr⁻¹, added to
/// the others (section 7.1).
const HURLEY_LBV: f64 = 0.1;

/// The luminous-blue-variable rate beyond the Humphreys–Davidson limit, M☉ yr⁻¹: `f_lbv` × 10⁻⁴
/// with Belczynski et al.'s (2010, equation 8) standard `f_lbv` = 1.5, the whole of the star's
/// wind, independent of metallicity.
const MODERN_LBV: f64 = 1.5e-4;

/// The power of Z ÷ Z☉ in a Wolf-Rayet star's wind: Vink and de Koter (2005, section 4) find
/// m = 0.86 for WN stars over 10⁻³ ≲ Z ÷ Z☉ ≲ 1.
const WOLF_RAYET_Z_POWER: f64 = 0.86;

/// The band of effective temperature, K, over which the modern recipe hands the cool stars' rate
/// over to Vink's: the cool rate alone at the lower edge, Vink's alone from the upper, 12,500 K,
/// the lower edge of Vink's fits (equation 25), which the band evaluates up to 1,000 K below its
/// range. The 1,000 K band is our choice, after the "Dutch" scheme of the MESA code, which blends
/// its cool rate into Vink's the same way over 10,000–11,000 K (`star/private/winds.f90`) and so
/// evaluates equation 25 up to 2,500 K below its range.
const HANDOVER_K: (f64, f64) = (11_500.0, 12_500.0);

/// The band of effective temperature, K, across which Vink's cool fit (equation 25, to 22,500 K)
/// passes to the hot one (equation 24, from 27,500 K): Vink's own "critical range", in which
/// either fit applies depending on where the jump falls (their section 8).
const JUMP_K: (f64, f64) = (22_500.0, 27_500.0);

/// The highest temperature, K, at which Vink's hot fit is evaluated: hotter hydrogen-rich stars
/// take its value at 50,000 K, the edge of the fitted range, rather than the fit's quadratic
/// turn-down beyond it.
const VINK_MAX_K: f64 = 50_000.0;

/// Vink equation 24, the hot side of the bi-stability jump (27,500–50,000 K): the constant and the
/// coefficients of log(L ÷ 10⁵), log(M ÷ 30), log((v∞ ÷ `v_esc`) ÷ 2), log(T ÷ 40,000),
/// log²(T ÷ 40,000) and log(Z ÷ Z☉), with v∞ ÷ `v_esc` = 2.6, the Galactic ratio on that side.
/// Every digit checked against the paper's equation and against the same fit in Belczynski et al.
/// (2010, equation 7) and the MESA code (`eval_Vink_wind`).
const VINK_HOT: VinkFit = VinkFit {
    constant: -6.697,
    luminosity: 2.194,
    mass: -1.313,
    velocity: -1.226,
    velocity_ratio: 2.6,
    temperature: 0.933,
    temperature_squared: -10.92,
    temperature_scale: 40_000.0,
    metallicity: 0.85,
};

/// Vink equation 25, the cool side of the bi-stability jump (12,500–22,500 K), as [`VINK_HOT`],
/// with v∞ ÷ `v_esc` = 1.3 and T scaled by 20,000 K, and no quadratic term (Belczynski et al.
/// 2010, equation 6, and MESA agree digit for digit).
const VINK_COOL: VinkFit = VinkFit {
    constant: -6.688,
    luminosity: 2.210,
    mass: -1.339,
    velocity: -1.601,
    velocity_ratio: 1.3,
    temperature: 1.07,
    temperature_squared: 0.0,
    temperature_scale: 20_000.0,
    metallicity: 0.85,
};

/// One of Vink's two fits: log Ṁ = constant + a log(L ÷ 10⁵) + b log(M ÷ 30) + c log((v∞ ÷ `v_esc`)
/// ÷ 2) + d log(T ÷ T₀) + e log²(T ÷ T₀) + f log(Z ÷ Z☉).
struct VinkFit {
    constant: f64,
    luminosity: f64,
    mass: f64,
    velocity: f64,
    velocity_ratio: f64,
    temperature: f64,
    temperature_squared: f64,
    temperature_scale: f64,
    metallicity: f64,
}

impl VinkFit {
    /// Ṁ, M☉ yr⁻¹, at luminosity `l` (L☉), mass `m` (M☉), temperature `t` (K) and `z_ratio` =
    /// Z ÷ Z☉.
    #[must_use]
    fn rate(&self, l: f64, m: f64, t: f64, z_ratio: f64) -> f64 {
        let log_t = math::log10(t / self.temperature_scale);
        math::exp10(
            self.constant
                + self.luminosity * math::log10(l / 1e5)
                + self.mass * math::log10(m / 30.0)
                + self.velocity * math::log10(self.velocity_ratio / 2.0)
                + self.temperature * log_t
                + self.temperature_squared * log_t * log_t
                + self.metallicity * math::log10(z_ratio),
        )
    }
}

/// Which terms a phase can have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Regime {
    /// No wind.
    Windless,
    /// The main sequence (SSE types 0 and 1): only the wind of luminous stars.
    MainSequence,
    /// A star with a hydrogen envelope around a core, from the Hertzsprung gap to the thermally
    /// pulsing AGB (types 2–6); `agb` on the early and thermally pulsing AGB (types 5 and 6),
    /// where Vassiliadis and Wood's wind acts.
    Evolved { agb: bool },
    /// A naked helium star (types 7–9).
    NakedHelium,
}

impl Regime {
    /// The regime of `phase`.
    #[must_use]
    const fn of(phase: Phase) -> Self {
        match phase {
            Phase::MainSequence => Self::MainSequence,
            Phase::HertzsprungGap | Phase::FirstGiantBranch | Phase::CoreHeliumBurning => {
                Self::Evolved { agb: false }
            }
            Phase::EarlyAgb | Phase::ThermallyPulsingAgb => Self::Evolved { agb: true },
            Phase::HeliumMainSequence | Phase::HeliumHertzsprungGap | Phase::HeliumGiantBranch => {
                Self::NakedHelium
            }
            Phase::Protostar
            | Phase::PreMainSequence
            | Phase::PostAgb
            | Phase::Substellar
            | Phase::HeliumWhiteDwarf
            | Phase::CarbonOxygenWhiteDwarf
            | Phase::OxygenNeonWhiteDwarf
            | Phase::NeutronStar
            | Phase::BlackHole
            | Phase::NoRemnant => Self::Windless,
        }
    }
}

/// The quantities the terms read, in the units of the module documentation.
struct Surface {
    /// L, L☉.
    l: f64,
    /// R, R☉.
    r: f64,
    /// M, M☉.
    m: f64,
    /// Mc, M☉.
    mc: f64,
    /// Effective temperature, K.
    t: f64,
    /// Z ÷ Z☉, with the formulae's Z.
    z_ratio: f64,
}

impl Surface {
    #[must_use]
    fn of(state: &StarState, composition: &Composition) -> Self {
        Self {
            l: state.luminosity().value(),
            r: state.radius().value(),
            m: state.mass().value(),
            mc: state.core_mass().value(),
            t: state.effective_temperature().value(),
            z_ratio: composition.z_fit() / Z_SOLAR,
        }
    }
}

/// HPT section 7.1, as SSE applies it: the largest of the terms the phase has, and on the
/// Hertzsprung gap to the AGB the luminous-blue-variable term added on top.
#[must_use]
fn hurley(regime: Regime, s: &Surface, eta: ReimersEta) -> f64 {
    match regime {
        Regime::Windless => 0.0,
        Regime::MainSequence => nieuwenhuijzen_de_jager(s),
        Regime::Evolved { agb } => hurley_evolved(s, eta, agb) + hurley_lbv(s),
        Regime::NakedHelium => reimers(s, eta).max(WOLF_RAYET * s.l * s.l.sqrt()),
    }
}

/// HPT's rate for a hydrogen-rich star with a core, before the luminous-blue-variable term: the
/// largest of Kudritzki and Reimers, Vassiliadis and Wood on the AGB (`agb`), Nieuwenhuijzen and
/// de Jager, and the small-envelope Wolf-Rayet-like term.
#[must_use]
fn hurley_evolved(s: &Surface, eta: ReimersEta, agb: bool) -> f64 {
    let giant = if agb {
        reimers(s, eta).max(vassiliadis_wood(s))
    } else {
        reimers(s, eta)
    };
    giant
        .max(nieuwenhuijzen_de_jager(s))
        .max(small_envelope_wolf_rayet(s))
}

/// Design note 6's modern recipe (module documentation).
#[must_use]
fn modern(regime: Regime, s: &Surface, eta: ReimersEta) -> f64 {
    match regime {
        Regime::Windless => 0.0,
        Regime::MainSequence => hot_or_cool(s, nieuwenhuijzen_de_jager(s)),
        Regime::Evolved { agb } => {
            if beyond_humphreys_davidson(s) {
                MODERN_LBV
            } else {
                hot_or_cool(s, hurley_evolved(s, eta, agb))
            }
        }
        Regime::NakedHelium => reimers(s, eta)
            .max(WOLF_RAYET * s.l * s.l.sqrt() * math::powf(s.z_ratio, WOLF_RAYET_Z_POWER)),
    }
}

/// Kudritzki and Reimers (1978) as HPT write it (equation 106): η × 4 × 10⁻¹³ L R ÷ M, with no
/// dependence on Z.
#[must_use]
fn reimers(s: &Surface, eta: ReimersEta) -> f64 {
    eta.value() * REIMERS * s.r * s.l / s.m
}

/// Vassiliadis and Wood (1993) on the AGB as HPT write it: log Ṁ = −11.4 + 0.0125 [P₀ − 100
/// max(M − 2.5, 0)], capped by the superwind, 1.36 × 10⁻⁹ L. The Mira period P₀ is capped at
/// 2,000 days, SSE's value, where HPT print log P₀ ≤ 3.3 (module documentation).
#[must_use]
fn vassiliadis_wood(s: &Surface) -> f64 {
    let (a, b, c) = MIRA_LOG_PERIOD;
    let period =
        math::exp10(a + b * math::log10(s.m) + c * math::log10(s.r)).min(MIRA_PERIOD_CAP_DAYS);
    let log_rate =
        VW_LOG_BASE + VW_PERIOD_SLOPE * (period - VW_MASS_DAYS * (s.m - VW_MASS_OFFSET).max(0.0));
    math::exp10(log_rate).min(SUPERWIND_PER_LUMINOSITY * s.l)
}

/// Nieuwenhuijzen and de Jager (1990) for luminous stars, scaled by (Z ÷ Z☉)^½ (Kudritzki et al.
/// 1989) as HPT do: 9.6 × 10⁻¹⁵ R^0.81 L^1.24 M^0.16 above 4,000 L☉. It switches on through
/// SSE's ramp, x = min(1, (L − 4,000) ÷ 500) times the rate, where HPT's text switches it on in
/// full (module documentation).
#[must_use]
fn nieuwenhuijzen_de_jager(s: &Surface) -> f64 {
    if s.l <= NJ_LUMINOSITY {
        return 0.0;
    }
    let ramp = ((s.l - NJ_LUMINOSITY) / NJ_RAMP).min(1.0);
    let (pr, pl, pm) = NJ_POWERS;
    NJ_COEFFICIENT
        * ramp
        * math::powf(s.r, pr)
        * math::powf(s.l, pl)
        * math::powf(s.m, pm)
        * s.z_ratio.sqrt()
}

/// HPT's Wolf-Rayet-like rate for small hydrogen envelopes, 10⁻¹³ L^1.5 (1 − μ) while μ of
/// equation 97 is below 1, and zero otherwise.
#[must_use]
fn small_envelope_wolf_rayet(s: &Surface) -> f64 {
    let mu = small_envelope_mu(
        SolarMasses::new(s.m),
        SolarMasses::new(s.mc),
        SolarLuminosities::new(s.l),
    );
    if mu < 1.0 {
        WOLF_RAYET * s.l * s.l.sqrt() * (1.0 - mu)
    } else {
        0.0
    }
}

/// 10⁻⁵ R L^½, which exceeds 1 beyond the Humphreys–Davidson limit.
#[must_use]
fn humphreys_davidson_x(s: &Surface) -> f64 {
    HD_RADIUS_SCALE * s.r * s.l.sqrt()
}

/// Whether the star lies beyond the Humphreys–Davidson limit as HPT write it: L > 6 × 10⁵ L☉ and
/// 10⁻⁵ R L^½ > 1.
#[must_use]
fn beyond_humphreys_davidson(s: &Surface) -> bool {
    s.l > HD_LUMINOSITY && humphreys_davidson_x(s) > 1.0
}

/// HPT's luminous-blue-variable rate beyond the Humphreys–Davidson limit, 0.1 (x − 1)³ (L ÷ 6 ×
/// 10⁵ − 1) with x = 10⁻⁵ R L^½, and zero within it; it vanishes on the limit itself.
#[must_use]
fn hurley_lbv(s: &Surface) -> f64 {
    if !beyond_humphreys_davidson(s) {
        return 0.0;
    }
    let excess = humphreys_davidson_x(s) - 1.0;
    HURLEY_LBV * excess * excess * excess * (s.l / HD_LUMINOSITY - 1.0)
}

/// The modern recipe's rate for a hydrogen-rich star inside the Humphreys–Davidson limit whose
/// HPT rate is `cool`: `cool` up to 11,500 K, Vink's rate alone from 12,500 K, and between them
/// (1 − w) × `cool` + w × Vink with w rising linearly in T from 0 to 1 across the band, both
/// evaluated at the star's own state ([`HANDOVER_K`]).
#[must_use]
fn hot_or_cool(s: &Surface, cool: f64) -> f64 {
    let (low, high) = HANDOVER_K;
    if s.t <= low {
        cool
    } else if s.t >= high {
        vink(s)
    } else {
        let w = (s.t - low) / (high - low);
        (1.0 - w) * cool + w * vink(s)
    }
}

/// Vink's rate at the star's L, M, T and Z (their equations 24 and 25). Below 22,500 K it is the
/// cool fit and from 27,500 K the hot one, evaluated at no more than 50,000 K ([`VINK_MAX_K`]);
/// across the bi-stability jump in between it is (1 − w) × cool + w × hot, both at the star's T,
/// with w rising linearly from 0 at 22,500 K to 1 at 27,500 K, so the rate is continuous where
/// Vink's own recipe picks one fit or the other by the jump's position (their section 8).
#[must_use]
fn vink(s: &Surface) -> f64 {
    let t = s.t.min(VINK_MAX_K);
    let (low, high) = JUMP_K;
    if t <= low {
        VINK_COOL.rate(s.l, s.m, t, s.z_ratio)
    } else if t >= high {
        VINK_HOT.rate(s.l, s.m, t, s.z_ratio)
    } else {
        let w = (t - low) / (high - low);
        (1.0 - w) * VINK_COOL.rate(s.l, s.m, t, s.z_ratio)
            + w * VINK_HOT.rate(s.l, s.m, t, s.z_ratio)
    }
}

#[cfg(test)]
mod tests {
    use super::super::continuity::assert_continuous_over;
    use super::*;
    use crate::units::{Dex, HeliumExcess, SolarRadii, Years};
    use hyperion_testkit::float::assert_same_bits;

    /// A star in `phase` of mass `m` and core mass `mc` (M☉), luminosity `l` (L☉) and radius `r`
    /// (R☉).
    fn star(phase: Phase, m: f64, mc: f64, l: f64, r: f64) -> StarState {
        StarState::new(crate::stellar::StarStateParts {
            phase,
            age: Years::new(1e7),
            mass: SolarMasses::new(m),
            core_mass: SolarMasses::new(mc),
            luminosity: SolarLuminosities::new(l),
            radius: SolarRadii::new(r),
            mass_loss_rate: SolarMassesPerYear::ZERO,
            phase_fraction: 0.5,
        })
    }

    /// The same star at effective temperature `t` (K) instead of a radius.
    fn star_at(phase: Phase, m: f64, mc: f64, l: f64, t: f64) -> StarState {
        let ratio = 5_772.0 / t;
        star(phase, m, mc, l, ratio * ratio * l.sqrt())
    }

    /// A composition of metal fraction `z`.
    fn metals(z: f64) -> Composition {
        Composition::from_fe_h(Dex::new(math::log10(z / 0.02)), HeliumExcess::ZERO)
    }

    fn surface(state: &StarState, z: f64) -> Surface {
        Surface::of(state, &metals(z))
    }

    fn hurley_rate(state: &StarState, z: f64) -> f64 {
        rate(
            WindRecipe::Hurley2000,
            state,
            &metals(z),
            ReimersEta::HURLEY,
        )
        .value()
    }

    fn modern_rate(state: &StarState, z: f64) -> f64 {
        rate(WindRecipe::Modern, state, &metals(z), ReimersEta::HURLEY).value()
    }

    #[track_caller]
    fn assert_close(what: &str, actual: f64, expected: f64, relative: f64) {
        assert!(
            (actual / expected - 1.0).abs() < relative,
            "{what}: {actual:e} against {expected:e}"
        );
    }

    /// Each term of HPT section 7.1 at a state chosen so that it can be checked by hand; the
    /// values are Python's double-precision arithmetic on the printed formulae.
    #[test]
    fn each_hurley_term_matches_a_hand_computed_value() {
        // Reimers: 0.5 × 4e-13 × 100 R☉ × 1,000 L☉ ÷ 0.9 M☉.
        let giant = surface(&star(Phase::FirstGiantBranch, 0.9, 0.3, 1e3, 100.0), 0.02);
        assert_close(
            "Reimers",
            reimers(&giant, ReimersEta::HURLEY),
            2.222_222_222_222_222_4e-8,
            1e-14,
        );
        assert_same_bits(reimers(&giant, ReimersEta::new(0.0)), 0.0);

        // Vassiliadis and Wood: P₀ = 10^(−2.07 + 1.94 log 272) = 450 d at 1 M☉, below both caps.
        let mira = surface(
            &star(Phase::ThermallyPulsingAgb, 1.0, 0.55, 5e3, 272.0),
            0.02,
        );
        assert_close(
            "VW",
            vassiliadis_wood(&mira),
            1.671_369_794_341_256_8e-6,
            1e-12,
        );
        // The superwind: 1.36e-9 × 8,000 L☉.
        let superwind = surface(
            &star(Phase::ThermallyPulsingAgb, 1.5, 0.6, 8e3, 400.0),
            0.02,
        );
        assert_close("superwind", vassiliadis_wood(&superwind), 1.088e-5, 1e-14);
        // The period cap: P₀ would be 4,311 d; at 2,000 d log Ṁ = −11.4 + 0.0125 (2,000 − 1,750).
        let long = surface(&star(Phase::EarlyAgb, 20.0, 6.0, 1e5, 3.5e3), 0.02);
        assert_close(
            "P₀ cap",
            vassiliadis_wood(&long),
            5.308_844_442_309_879_5e-9,
            1e-12,
        );

        // Nieuwenhuijzen and de Jager, at solar Z and scaled by (0.001 ÷ 0.02)^½.
        let luminous = star(Phase::MainSequence, 20.0, 0.0, 1e5, 8.0);
        let nj = nieuwenhuijzen_de_jager(&surface(&luminous, 0.02));
        assert_close("NdJ", nj, 1.324_155_295_901_92e-7, 1e-12);
        let nj_poor = nieuwenhuijzen_de_jager(&surface(&luminous, 0.001));
        assert_close(
            "NdJ at Z = 0.001",
            nj_poor,
            2.960_901_254_403_041_3e-8,
            1e-12,
        );
        // Half-way up SSE's ramp at 4,250 L☉, and nothing at 4,000.
        let ramp = surface(&star(Phase::MainSequence, 7.0, 0.0, 4_250.0, 3.0), 0.02);
        assert_close(
            "NdJ ramp",
            nieuwenhuijzen_de_jager(&ramp),
            5.036_483_475_229_968e-10,
            1e-12,
        );
        let below = surface(&star(Phase::MainSequence, 7.0, 0.0, 4_000.0, 3.0), 0.02);
        assert_same_bits(nieuwenhuijzen_de_jager(&below), 0.0);

        // Small envelope: μ = 0.05 × 1.2 = 0.06, Ṁ = 1e-13 × (2e5)^1.5 × 0.94.
        let stripped = star(Phase::CoreHeliumBurning, 10.0, 9.5, 2e5, 30.0);
        let mu = small_envelope_mu(stripped.mass(), stripped.core_mass(), stripped.luminosity());
        assert!((mu - 0.06).abs() < 1e-15, "μ = {mu}");
        let wr = small_envelope_wolf_rayet(&surface(&stripped, 0.02));
        assert_close("Wolf-Rayet-like", wr, 8.407_615_595_399_21e-6, 1e-12);
        // A full envelope (μ = 3.3) has none.
        assert_same_bits(small_envelope_wolf_rayet(&giant), 0.0);

        // HPT's LBV term: x = 1e-5 × 150 × 1,000 = 1.5, 0.1 × 0.5³ × (1e6 ÷ 6e5 − 1).
        let lbv = surface(&star(Phase::HertzsprungGap, 60.0, 20.0, 1e6, 150.0), 0.02);
        assert_close("LBV", hurley_lbv(&lbv), 8.333_333_333_333_335e-3, 1e-14);
        // Inside the limit (x = 0.9) and below its luminosity there is none.
        let inside = surface(&star(Phase::HertzsprungGap, 60.0, 20.0, 1e6, 90.0), 0.02);
        assert_same_bits(hurley_lbv(&inside), 0.0);
        let faint = surface(&star(Phase::HertzsprungGap, 60.0, 20.0, 5e5, 1_000.0), 0.02);
        assert_same_bits(hurley_lbv(&faint), 0.0);
    }

    /// HPT combine the terms by the largest and add the luminous-blue-variable term; which terms a
    /// star has depends on its phase. The totals are hand-computed as above and the published SSE
    /// code's `mlwind` gives the same numbers (run of 2026-09-23 on the package of
    /// [`sse`](super::super), η = 0.5, `hewind` = 1, a single star).
    #[test]
    fn the_hurley_rate_is_the_largest_term_that_applies_plus_the_lbv_term() {
        // Reimers alone on the giant branch: no Vassiliadis and Wood before the AGB.
        let giant = star(Phase::FirstGiantBranch, 0.9, 0.3, 1e3, 100.0);
        assert_close(
            "GB",
            hurley_rate(&giant, 0.02),
            2.222_222_222_222_222_4e-8,
            1e-14,
        );
        // One state in four phases: Vassiliadis and Wood lead on the AGB, Reimers elsewhere, and
        // the main sequence has only Nieuwenhuijzen and de Jager; a naked helium star has Reimers
        // or 1e-13 L^1.5, whichever is larger.
        let at = |phase| star(phase, 1.0, 0.55, 5e3, 272.0);
        let s = surface(&at(Phase::ThermallyPulsingAgb), 0.02);
        let (vw, kr, nj) = (
            vassiliadis_wood(&s),
            reimers(&s, ReimersEta::HURLEY),
            nieuwenhuijzen_de_jager(&s),
        );
        assert!(
            vw > kr && kr > nj && nj > 0.0,
            "VW {vw:e}, Reimers {kr:e}, NdJ {nj:e}"
        );
        assert_same_bits(hurley_rate(&at(Phase::ThermallyPulsingAgb), 0.02), vw);
        assert_same_bits(hurley_rate(&at(Phase::EarlyAgb), 0.02), vw);
        assert_same_bits(hurley_rate(&at(Phase::FirstGiantBranch), 0.02), kr);
        assert_same_bits(hurley_rate(&at(Phase::HertzsprungGap), 0.02), kr);
        assert_same_bits(hurley_rate(&at(Phase::MainSequence), 0.02), nj);
        assert_same_bits(hurley_rate(&at(Phase::HeliumGiantBranch), 0.02), kr);
        let helium = star(Phase::HeliumMainSequence, 5.0, 0.0, 1e4, 0.8);
        assert_close("He star", hurley_rate(&helium, 0.02), 1e-7, 1e-14);
        // The superwind leads a TPAGB star; NdJ a massive MS star; the small-envelope term a
        // stripped giant.
        let superwind = star(Phase::ThermallyPulsingAgb, 1.5, 0.6, 8e3, 400.0);
        assert_close("TPAGB", hurley_rate(&superwind, 0.02), 1.088e-5, 1e-14);
        let luminous = star(Phase::MainSequence, 20.0, 0.0, 1e5, 8.0);
        assert_close(
            "MS",
            hurley_rate(&luminous, 0.02),
            1.324_155_295_901_92e-7,
            1e-12,
        );
        let stripped = star(Phase::CoreHeliumBurning, 10.0, 9.5, 2e5, 30.0);
        assert_close(
            "CHeB",
            hurley_rate(&stripped, 0.02),
            8.407_615_595_399_21e-6,
            1e-12,
        );
        // Beyond the Humphreys–Davidson limit the LBV term adds to the largest of the others,
        // here Nieuwenhuijzen and de Jager's 2.9e-5 over the small-envelope term's 2e-5 (μ = 0.8)
        // and Reimers' 5e-7.
        let lbv = star(Phase::HertzsprungGap, 60.0, 20.0, 1e6, 150.0);
        let s = surface(&lbv, 0.02);
        let largest = nieuwenhuijzen_de_jager(&s);
        let wr = small_envelope_wolf_rayet(&s);
        let kr = reimers(&s, ReimersEta::HURLEY);
        assert!(
            largest > wr && wr > kr && wr > 0.0,
            "NdJ {largest:e}, WR {wr:e}, Reimers {kr:e}"
        );
        assert_same_bits(hurley_rate(&lbv, 0.02), largest + hurley_lbv(&s));
        assert_close(
            "HG LBV",
            hurley_rate(&lbv, 0.02),
            8.362_805_315_989_177e-3,
            1e-12,
        );
    }

    /// Stars without a wind in either recipe.
    #[test]
    fn remnants_and_stars_before_the_main_sequence_have_no_wind() {
        let mut windless = 0;
        for phase in Phase::ALL {
            let s = if phase == Phase::NoRemnant {
                star(phase, 0.0, 0.0, 0.0, 0.0)
            } else if phase.is_remnant() {
                star(phase, 1.0, 1.0, 1e-3, 0.01)
            } else {
                star(phase, 1.0, 0.5, 1e5, 100.0)
            };
            let none = Regime::of(phase) == Regime::Windless;
            windless += usize::from(none);
            for recipe in [WindRecipe::Hurley2000, WindRecipe::Modern] {
                let mdot = rate(recipe, &s, &metals(0.02), ReimersEta::HURLEY).value();
                if none {
                    assert_same_bits(mdot, 0.0);
                } else {
                    assert!(mdot > 0.0, "{phase:?} under {recipe:?}: {mdot:e}");
                }
            }
        }
        // Six remnants, the protostar, the pre-main sequence, the post-AGB crossing and
        // substellar objects.
        assert_eq!(windless, 10);
    }

    /// The published SSE code's `mlwind` (see [`sse`](super::super) for the package; run of
    /// 2026-09-23 with η = 0.5, `bwind` = 0, `hewind` = 1 and no Roche lobe) at twelve states of
    /// a grid of 131,040 over SSE types 1–9, four metallicities and every term, to 10⁻⁹; the whole
    /// grid agrees to 8 × 10⁻¹⁵.
    #[test]
    fn matches_the_published_sse_code() {
        for &(kw, z, l, r, m, mc, sse) in SSE_MLWIND {
            let phase = match kw {
                1 => Phase::MainSequence,
                2 => Phase::HertzsprungGap,
                3 => Phase::FirstGiantBranch,
                4 => Phase::CoreHeliumBurning,
                5 => Phase::EarlyAgb,
                6 => Phase::ThermallyPulsingAgb,
                7 => Phase::HeliumMainSequence,
                9 => Phase::HeliumGiantBranch,
                _ => unreachable!("no row of type {kw}"),
            };
            let ours = hurley_rate(&star(phase, m, mc, l, r), z);
            assert_close(&format!("type {kw}, Z = {z}, L = {l}"), ours, sse, 1e-9);
        }
    }

    /// (SSE type, Z, L, R, M, Mc, Ṁ from `mlwind`).
    const SSE_MLWIND: &[(u8, f64, f64, f64, f64, f64, f64)] = &[
        (1, 0.02, 4_300.0, 3.0, 9.0, 0.45, 6.383_669_380_373_348e-10),
        (
            1,
            0.001,
            150_000.0,
            30.0,
            20.0,
            1.0,
            1.428_046_706_872_803_2e-7,
        ),
        (2, 0.02, 900.0, 30.0, 4.0, 1.6, 1.35e-9),
        (
            2,
            0.02,
            1_200_000.0,
            150.0,
            45.0,
            18.0,
            0.026_642_380_289_698_41,
        ),
        (3, 0.001, 900.0, 150.0, 1.5, 0.6, 1.8e-8),
        (3, 0.0001, 4_600.0, 150.0, 4.0, 1.6, 3.45e-8),
        (
            4,
            0.02,
            150_000.0,
            30.0,
            20.0,
            18.6,
            5.321_479_117_688_992e-6,
        ),
        (
            5,
            0.03,
            20_000.0,
            900.0,
            9.0,
            6.75,
            8.896_297_370_481_784e-7,
        ),
        (6, 0.02, 20_000.0, 900.0, 1.5, 0.6, 2.72e-5),
        (6, 0.02, 4_600.0, 400.0, 1.5, 0.6, 6.256e-6),
        (7, 0.02, 20_000.0, 0.3, 4.0, 0.2, 2.828_427_124_746_19e-7),
        (
            9,
            0.001,
            20_000.0,
            400.0,
            4.0,
            1.6,
            4.000_000_000_000_000_3e-7,
        ),
    ];

    /// SSE's ramp keeps the Hurley rate continuous where Nieuwenhuijzen and de Jager's term
    /// switches on, at 4,000 and 4,500 L☉.
    #[test]
    fn the_hurley_rate_is_continuous_where_the_luminous_term_switches_on() {
        let log_l: Vec<f64> = (0..=400)
            .map(|i| math::log10(3_000.0) + f64::from(i) * 0.001)
            .collect();
        // In units of 10⁻¹⁰ M☉ per year: the rate starts from zero on the main sequence, so its
        // logarithm would not do. HPT's printed switch would jump by about 9 of these units.
        for (phase, mc) in [(Phase::MainSequence, 0.0), (Phase::HertzsprungGap, 1.5)] {
            let f = |x: f64| hurley_rate(&star(phase, 7.0, mc, math::exp10(x), 3.0), 0.02) * 1e10;
            assert_continuous_over(&format!("{phase:?}"), f, &log_l, 0.1, 1e-6);
        }
    }

    /// The plan's check of Vink's hot fit: a 40 M☉ main-sequence star at 40,000 K and 10^5.7 L☉
    /// loses 10⁻⁶–10⁻⁵ M☉ per year at solar Z; equation 24 gives 3.43 × 10⁻⁶ (the pinned value is
    /// Python's double-precision arithmetic on the printed equation).
    #[test]
    fn a_40_solar_mass_o_star_loses_a_few_millionths_of_a_solar_mass_a_year() {
        let o_star = star_at(Phase::MainSequence, 40.0, 0.0, math::exp10(5.7), 40_000.0);
        let mdot = modern_rate(&o_star, 0.02);
        assert!((1e-6..=1e-5).contains(&mdot), "{mdot:e}");
        assert_close("Vink hot", mdot, 3.428_158_466_053_071_7e-6, 1e-9);
    }

    /// Equation 25 on the cool side of the jump, for a blue supergiant with a full envelope: it
    /// replaces Nieuwenhuijzen and de Jager's 1.3 × 10⁻⁶ and exceeds Reimers' 9.2 × 10⁻⁸ (the
    /// pinned value is Python's double-precision arithmetic on the printed equation).
    #[test]
    fn a_blue_supergiant_follows_vinks_cool_fit() {
        let supergiant = star_at(Phase::CoreHeliumBurning, 20.0, 2.0, 2e5, 18_000.0);
        assert_close(
            "Vink cool",
            modern_rate(&supergiant, 0.02),
            2.908_222_883_631_956_5e-6,
            1e-9,
        );
    }

    /// A hot star with a thin envelope loses mass at Vink's rate alone: HPT's small-envelope term
    /// (8.4 × 10⁻⁶ M☉ per year here, μ = 0.06) and Reimers' no longer compete above 12,500 K, and
    /// hold again at 11,500 K and below.
    #[test]
    fn a_hot_stripped_giant_loses_mass_at_vinks_rate_alone() {
        let hot = star_at(Phase::CoreHeliumBurning, 10.0, 9.5, 2e5, 30_000.0);
        let s = surface(&hot, 0.02);
        let (vink_rate, hurley_rate_here) =
            (vink(&s), hurley_evolved(&s, ReimersEta::HURLEY, false));
        assert!(
            hurley_rate_here > 5.0 * vink_rate,
            "HPT {hurley_rate_here:e} against Vink {vink_rate:e}"
        );
        assert_same_bits(modern_rate(&hot, 0.02), vink_rate);
        let cool = star_at(Phase::CoreHeliumBurning, 10.0, 9.5, 2e5, 11_000.0);
        assert_same_bits(modern_rate(&cool, 0.02), hurley_rate(&cool, 0.02));
    }

    /// Vink's rates scale as (Z ÷ Z☉)^0.85 on both sides of the jump, a naked helium star's as
    /// Z^0.86, and a luminous blue variable's not at all; Z beyond the fitted range is clamped.
    /// The helium star's pinned rate is 10⁻¹³ × (10⁴)^1.5 × 0.1^0.86 in Python's double
    /// precision.
    #[test]
    fn modern_rates_scale_with_metallicity_as_their_sources_state() {
        let tenth = |s: &StarState| modern_rate(s, 0.002) / modern_rate(s, 0.02);
        for t in [15_000.0, 25_000.0, 40_000.0] {
            let o_star = star_at(Phase::MainSequence, 40.0, 0.0, math::exp10(5.7), t);
            assert_close(
                &format!("Vink at {t} K"),
                tenth(&o_star),
                math::powf(0.1, 0.85),
                1e-12,
            );
        }
        let helium = star(Phase::HeliumMainSequence, 5.0, 0.0, 1e4, 0.8);
        assert_close("He star", tenth(&helium), math::powf(0.1, 0.86), 1e-12);
        assert_close(
            "He star at Z = 0.002",
            modern_rate(&helium, 0.002),
            1.380_384_264_602_885_1e-8,
            1e-12,
        );
        let lbv = star(Phase::HertzsprungGap, 60.0, 20.0, 1e6, 150.0);
        assert_same_bits(tenth(&lbv), 1.0);
        let o_star = star_at(Phase::MainSequence, 40.0, 0.0, math::exp10(5.7), 40_000.0);
        assert_same_bits(modern_rate(&o_star, 0.06), modern_rate(&o_star, 0.03));
        assert_same_bits(modern_rate(&o_star, 1e-5), modern_rate(&o_star, 1e-4));
    }

    /// Beyond the Humphreys–Davidson limit an evolved hydrogen-rich star loses 1.5 × 10⁻⁴ M☉ per
    /// year and nothing else; inside it, and on the main sequence, the other terms apply.
    #[test]
    fn beyond_the_humphreys_davidson_limit_the_modern_rate_is_belczynskis() {
        for phase in [
            Phase::HertzsprungGap,
            Phase::CoreHeliumBurning,
            Phase::EarlyAgb,
        ] {
            let lbv = star(phase, 60.0, 20.0, 1e6, 150.0);
            assert_same_bits(modern_rate(&lbv, 0.02), 1.5e-4);
        }
        let inside = star(Phase::HertzsprungGap, 60.0, 20.0, 1e6, 90.0);
        let within = modern_rate(&inside, 0.02);
        assert!(within < 1e-4, "inside the limit: {within:e}");
        let main_sequence = star(Phase::MainSequence, 60.0, 0.0, 1e6, 150.0);
        let s = surface(&main_sequence, 0.02);
        assert_same_bits(
            modern_rate(&main_sequence, 0.02),
            hot_or_cool(&s, nieuwenhuijzen_de_jager(&s)),
        );
    }

    /// Cool stars keep HPT's rates under the modern recipe: below 11,500 K and inside the
    /// Humphreys–Davidson limit the two recipes agree bit for bit, naked helium stars at solar Z
    /// included.
    #[test]
    fn cool_stars_keep_hurleys_rates_under_the_modern_recipe() {
        let cool = [
            star(Phase::FirstGiantBranch, 0.9, 0.3, 1e3, 100.0),
            star(Phase::ThermallyPulsingAgb, 1.0, 0.55, 5e3, 272.0),
            star(Phase::ThermallyPulsingAgb, 1.5, 0.6, 8e3, 400.0),
            star(Phase::EarlyAgb, 20.0, 6.0, 1e5, 3.5e3),
            star_at(Phase::CoreHeliumBurning, 12.0, 3.0, 5e4, 11_000.0),
            star_at(Phase::MainSequence, 1.0, 0.0, 1.0, 5_772.0),
        ];
        for s in &cool {
            assert!(s.effective_temperature().value() <= 11_500.0, "{s:?}");
            for z in [0.001, 0.02] {
                assert_same_bits(modern_rate(s, z), hurley_rate(s, z));
            }
        }
        let helium = star(Phase::HeliumMainSequence, 5.0, 0.0, 1e4, 0.8);
        assert_same_bits(modern_rate(&helium, 0.02), hurley_rate(&helium, 0.02));
    }

    /// The modern rate has no jump in effective temperature across the hand-over band
    /// (11,500–12,500 K), the bi-stability jump (22,500–27,500 K) or the top of Vink's fits
    /// (50,000 K), for main-sequence stars and giants of several masses and luminosities, one of
    /// them stripped to a thin envelope, at three metallicities. Luminosities stay below the
    /// Humphreys–Davidson limit, whose step is Belczynski et al.'s by design.
    #[test]
    fn the_modern_rate_is_continuous_across_every_temperature_boundary() {
        let log_t: Vec<f64> = (0..=2_000)
            .map(|i| {
                math::log10(8_000.0)
                    + f64::from(i) * (math::log10(60_000.0) - math::log10(8_000.0)) / 2_000.0
            })
            .collect();
        let stars: [(Phase, f64, f64, f64); 7] = [
            (Phase::MainSequence, 40.0, 0.0, math::exp10(5.7)),
            (Phase::CoreHeliumBurning, 10.0, 9.5, 2e5),
            (Phase::CoreHeliumBurning, 5.0, 1.0, 800.0),
            (Phase::MainSequence, 12.0, 0.0, 1.5e4),
            (Phase::HertzsprungGap, 20.0, 5.0, 1e5),
            (Phase::CoreHeliumBurning, 8.0, 2.0, 5e3),
            (Phase::CoreHeliumBurning, 25.0, 9.0, 4e5),
        ];
        for z in [0.0002, 0.004, 0.02] {
            for (phase, m, mc, l) in stars {
                let f = |x: f64| {
                    let s = star_at(phase, m, mc, l, math::exp10(x));
                    math::log10(modern_rate(&s, z).max(1e-30))
                };
                assert_continuous_over(
                    &format!("{phase:?} {m} M☉ {l} L☉ Z = {z}"),
                    f,
                    &log_t,
                    0.005,
                    1e-6,
                );
            }
            // A faint main-sequence B star has no cool wind at all, so Vink's rate rises from
            // zero across the hand-over band; in units of its rate at 12,500 K.
            let faint = |t: f64| star_at(Phase::MainSequence, 5.0, 0.0, 800.0, t);
            let unit = modern_rate(&faint(12_500.0), z);
            let f = |x: f64| modern_rate(&faint(math::exp10(x)), z) / unit;
            assert_continuous_over(&format!("5 M☉ MS, Z = {z}"), f, &log_t, 0.005, 1e-9);
        }
    }

    /// Where only Vink's fits act, the blends reproduce each fit at the edges of their bands.
    #[test]
    fn the_blends_meet_each_fit_at_the_edges_of_their_bands() {
        let (l, m) = (math::exp10(5.5), 30.0);
        let at = |t: f64| Surface {
            l,
            r: 10.0,
            m,
            mc: 0.0,
            t,
            z_ratio: 1.0,
        };
        let (low, high) = JUMP_K;
        assert_same_bits(vink(&at(low)), VINK_COOL.rate(l, m, low, 1.0));
        assert_same_bits(vink(&at(high)), VINK_HOT.rate(l, m, high, 1.0));
        let mid = f64::midpoint(low, high);
        let blend = 0.5 * VINK_COOL.rate(l, m, mid, 1.0) + 0.5 * VINK_HOT.rate(l, m, mid, 1.0);
        assert_close("25,000 K", vink(&at(mid)), blend, 1e-15);
        assert_same_bits(vink(&at(60_000.0)), VINK_HOT.rate(l, m, VINK_MAX_K, 1.0));
        let (low, high) = HANDOVER_K;
        assert_same_bits(hot_or_cool(&at(low), 0.25), 0.25);
        assert_same_bits(hot_or_cool(&at(high), 0.25), vink(&at(high)));
        let mid = f64::midpoint(low, high);
        assert_close(
            "12,000 K",
            hot_or_cool(&at(mid), 0.25),
            0.125 + 0.5 * vink(&at(mid)),
            1e-15,
        );
    }
}
