//! Variability, derived and never rolled (plan 06, P06.T26.a–c).
//!
//! Every named kind of variable falls out of a region test on the star's state and a period rule,
//! so that a star switches on and off continuously as it crosses a region, and its period follows
//! its evolution:
//!
//! - **The instability strip** (P06.T26.a): δ Scuti stars, RR Lyrae stars, classical and type II
//!   Cepheids, each between a blue and a red edge that are straight lines in (log T(eff), log L),
//!   from the source of its kind, and named by phase and initial mass. The period is the mean
//!   density's, P = Q (ρ̄ ÷ ρ̄☉)^−½ with Q by kind, and the amplitude is largest mid-strip and zero
//!   at both edges.
//! - **Long-period variables** (P06.T26.b): Miras on the thermally pulsing asymptotic giant branch,
//!   semiregulars below, and the slow semiregular and irregular red supergiants, with the
//!   period–mass–radius relation of Vassiliadis and Wood (1993).
//! - **The other strips** (P06.T26.c): β Cephei and slowly pulsating B stars, γ Doradus stars, the
//!   pulsating white dwarfs (ZZ Ceti, V777 Her, GW Vir), α Cygni supergiants, the S Doradus cycles
//!   of luminous blue variables, and rotational modulation (BY Dra, α² Canum Venaticorum) from P06.T25's period
//!   and activity.
//!
//! A star is one kind at a time: the first region that holds it in the order of [`variability`].
//! The light factor at a clock time, with cycle-keyed irregularity (P06.T26.d), needs the event
//! machinery's monotone phase and is not built here.

use crate::math;
use crate::stellar::classify::{Classification, LuminosityClass, PeculiarClass, SpectralType};
use crate::stellar::remnant::wd_spectral::WhiteDwarfAtmosphere;
use crate::stellar::rotation::{Activity, ActivityLevel, Rotation};
use crate::stellar::{Composition, Phase, StarState};
use crate::units::consts::{SECONDS_PER_DAY, SECONDS_PER_JULIAN_YEAR};
use crate::units::{Days, Magnitudes, SolarMasses};

/// The kinds of variable star plan 06 names (P06.T26).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum VariableKind {
    /// δ Scuti: main-sequence and Hertzsprung-gap pulsators of 1.5–2.5 M☉.
    DeltaScuti,
    /// RR Lyrae: low-mass core-helium-burning pulsators.
    RrLyrae,
    /// A classical Cepheid: a star of 3–20 M☉ crossing the strip on its blue loop or in the gap.
    ClassicalCepheid,
    /// A type II Cepheid of the BL Herculis class, periods under 4 days.
    BlHerculis,
    /// A type II Cepheid of the W Virginis class, 4–20 days.
    WVirginis,
    /// A type II Cepheid of the RV Tauri class, beyond 20 days.
    RvTauri,
    /// A Mira: a large-amplitude long-period variable of the thermally pulsing AGB.
    Mira,
    /// A semiregular giant of class `SRa`, with persistent periodicity.
    SemiregularA,
    /// A semiregular giant of class `SRb`, with poorly defined periodicity.
    SemiregularB,
    /// A semiregular red supergiant of class `SRc`.
    SemiregularC,
    /// A slow irregular red supergiant of class `Lc`.
    SlowIrregular,
    /// A β Cephei star: an early-B main-sequence p-mode pulsator.
    BetaCephei,
    /// A slowly pulsating B star: a mid- to late-B main-sequence g-mode pulsator.
    SlowlyPulsatingB,
    /// A γ Doradus star: an early-F main-sequence g-mode pulsator.
    GammaDoradus,
    /// A ZZ Ceti star: a pulsating DA white dwarf of 10,500–12,500 K.
    ZzCeti,
    /// A V777 Herculis star: a pulsating helium-atmosphere white dwarf of 22,000–29,000 K.
    V777Herculis,
    /// A GW Virginis star: a pulsating hot pre-white dwarf of 80,000–180,000 K.
    GwVirginis,
    /// An α Cygni variable: a pulsating B or A supergiant.
    AlphaCygni,
    /// The S Doradus cycles of a luminous blue variable.
    SDoradus,
    /// A BY Draconis variable: spots on an active cool dwarf, turning with it.
    ByDraconis,
    /// An α² Canum Venaticorum variable: a magnetic chemically peculiar star turning its spots.
    Alpha2CanumVenaticorum,
}

/// How a star's light varies: its kind, the period of its main cycle and its full amplitude
/// (P06.T26).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Variability {
    kind: VariableKind,
    period: Days,
    amplitude: Magnitudes,
}

impl Variability {
    /// The kind of variable.
    #[must_use]
    pub const fn kind(&self) -> VariableKind {
        self.kind
    }

    /// The period of the main cycle: from minutes for a pulsating white dwarf to decades for the
    /// S Doradus cycles.
    #[must_use]
    pub const fn period(&self) -> Days {
        self.period
    }

    /// The full amplitude in V, peak to peak: positive inside a region and falling to zero at
    /// its edges.
    #[must_use]
    pub const fn amplitude(&self) -> Magnitudes {
        self.amplitude
    }
}

/// What [`variability`] reads of a star beyond its state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VariabilityInputs<'a> {
    /// The star's state.
    pub state: &'a StarState,
    /// Its initial mass, which names the pulsators of the strip (P06.T26.a).
    pub initial_mass: SolarMasses,
    /// Its composition, whose metal fraction moves the RR Lyrae strip.
    pub composition: &'a Composition,
    /// Its classification: a white dwarf's atmosphere, a supergiant's class, an Ap or Bp star.
    pub classification: &'a Classification,
    /// Its rotation (P06.T25), for rotational modulation.
    pub rotation: Option<&'a Rotation>,
    /// Its activity (P06.T25), for the spotted BY Draconis stars.
    pub activity: Option<&'a Activity>,
}

// --- The instability strip (P06.T26.a) ---------------------------------------------------------

/// A strip between two straight edges in (log T(eff), log L): log T(eff) = a + b log L at the blue
/// edge and at the red.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Strip {
    blue: (f64, f64),
    red: (f64, f64),
}

impl Strip {
    /// Where log T(eff) `log_t` lies across the strip at log L `log_l`: 0 at the blue edge, 1 at the
    /// red, outside (0, 1) outside the strip; `None` where the edges meet or cross.
    #[must_use]
    fn position(&self, log_l: f64, log_t: f64) -> Option<f64> {
        let blue = math::mul_add(self.blue.1, log_l, self.blue.0);
        let red = math::mul_add(self.red.1, log_l, self.red.0);
        (blue > red).then(|| (blue - log_t) / (blue - red))
    }
}

/// The Cepheid strip, classical and type II, for log L of 1.5–4.7: a least-squares line through
/// each edge of the fundamental-mode strip of Anderson et al. (2016, A&A 591, A8, table A.1, all
/// crossings, no rotation, Z = 0.014), fitted by the lane's research (rms 0.005 and 0.014 dex);
/// Anderson et al. give the edges as tables only.
const CEPHEID_STRIP: Strip = Strip {
    blue: (3.9311, -0.0441),
    red: (3.9531, -0.0710),
};

/// The RR Lyrae strip of Marconi et al. (2015, ApJ 808, 50, equations 3 and 4): from the first
/// overtone's blue edge, log T(eff) = 3.957 − 0.080 log L − 0.012 log Z, to the fundamental red edge,
/// 3.879 − 0.084 log L − 0.012 log Z; the metal term is added in [`rr_lyrae_strip`].
const RR_LYRAE_STRIP: Strip = Strip {
    blue: (3.957, -0.080),
    red: (3.879, -0.084),
};

/// The δ Scuti strip of Murphy et al. (2019, MNRAS 485, 2380, section 5), log L = −20.476 log
/// T(eff) + 82.454 at the blue edge and −12.962 log T(eff) + 50.823 at the red, rearranged; valid for
/// the main sequence and just beyond it, as they state.
const DELTA_SCUTI_STRIP: Strip = Strip {
    blue: (82.454 / 20.476, -1.0 / 20.476),
    red: (50.823 / 12.962, -1.0 / 12.962),
};

/// The initial masses of the δ Scuti stars, M☉: 1.5–2.5 (plan 06, P06.T26.a; Murphy et al. 2019
/// give 1.5–2.3 on the main sequence).
const DELTA_SCUTI_MASSES: (f64, f64) = (1.5, 2.5);

/// The lowest surface gravity of a δ Scuti star, log g (cgs): 3.5, the lane's reading of Murphy et
/// al.'s "main sequence and immediate post-main-sequence", which keeps a 2.5 M☉ star crossing
/// the gap from carrying the strip beyond where they calibrated it.
const DELTA_SCUTI_MIN_LOG_G: f64 = 3.5;

/// The initial masses of the classical Cepheids, M☉: 3–20 (plan 06, P06.T26.a).
const CEPHEID_MASSES: (f64, f64) = (3.0, 20.0);

/// The initial mass below which a core-helium-burning star is on the horizontal branch, an RR
/// Lyrae star in the strip, and an asymptotic giant crossing it a type II Cepheid: 2 M☉, about
/// the mass above which helium ignites without a flash at solar metallicity (Hurley et al. 2000's
/// M(HeF) of 1.995 M☉ at Z = 0.02; the lane's single figure for every metallicity).
const LOW_MASS_LIMIT: f64 = 2.0;

/// The pulsation constant Q of the δ Scuti stars' fundamental mode, d: 0.033 (Handler and
/// Shobbrook 2002, MNRAS 333, 251, section 3.1).
const Q_DELTA_SCUTI: f64 = 0.033;

/// The pulsation constant of RR Lyrae stars, d: 0.036, the middle of the 0.034–0.038 that the
/// periods of Marconi et al. (2015, equation 1) give across their strip.
const Q_RR_LYRAE: f64 = 0.036;

/// The pulsation constant of Cepheids, d: 0.039, the value of Anderson et al.'s (2016) models at
/// 3–5 d. Their Q rises to 0.052 at 31 d, so long-period Cepheids come out about 30% short
/// (provisional; the constant Q gives the sample the observed period–luminosity slope).
const Q_CEPHEID: f64 = 0.039;

/// The period dividing the BL Herculis from the W Virginis stars, d: 4, and W Virginis from RV
/// Tauri, 20 (Soszyński et al. 2008, Acta Astron. 58, 293, section 3).
const TYPE_II_PERIODS: (f64, f64) = (4.0, 20.0);

/// The largest full amplitudes in V mid-strip, mag, within the ranges the General Catalogue of
/// Variable Stars gives each type (Samus et al. 2017): δ Scuti 0.003–0.9, RR Lyrae 0.5–2,
/// Cepheids several hundredths to 2. The lane's choices.
const STRIP_AMPLITUDES: [(VariableKind, f64); 6] = [
    (VariableKind::DeltaScuti, 0.3),
    (VariableKind::RrLyrae, 1.0),
    (VariableKind::ClassicalCepheid, 1.0),
    (VariableKind::BlHerculis, 0.8),
    (VariableKind::WVirginis, 0.8),
    (VariableKind::RvTauri, 1.0),
];

// --- Long-period variables (P06.T26.b) ---------------------------------------------------------

/// The effective temperature below which a giant or supergiant is a long-period variable, K: the
/// late K and M types (the lane's).
const LPV_MAX_TEFF: f64 = 4_500.0;

/// The luminosity above which a first-giant-branch star near its tip is a semiregular, L☉: 10²·⁸,
/// within the last magnitude below the tip (the lane's).
const TIP_GIANT_MIN_L: f64 = 630.0;

/// The period from which a thermally pulsing AGB star is a Mira, d: 100 (the lane's rule; the
/// catalogue's Miras have periods of 80–1,000 days and amplitudes over 2.5 mag).
const MIRA_MIN_PERIOD: f64 = 100.0;

/// The full amplitudes of the long-period variables in V, mag (the lane's, within the catalogue's
/// ranges: Mira 2.5–11, `SRa` under 2.5, `SRc` and `Lc` about 1).
const LPV_AMPLITUDES: [(VariableKind, f64); 5] = [
    (VariableKind::Mira, 4.0),
    (VariableKind::SemiregularA, 1.5),
    (VariableKind::SemiregularB, 0.8),
    (VariableKind::SemiregularC, 1.0),
    (VariableKind::SlowIrregular, 1.0),
];

// --- The other strips (P06.T26.c) ---------------------------------------------------------------

/// The β Cephei region on the main sequence: log T(eff) 4.28–4.47 and log L above 3.25 (Pamyatnykh
/// 1999, Acta Astron. 49, 119, figure 3, read by eye by the lane's research, ±0.02 dex).
const BETA_CEPHEI: ((f64, f64), f64) = ((4.28, 4.47), 3.25);

/// The slowly pulsating B region on the main sequence: log T(eff) 4.05–4.30 and log L 1.75–3.5
/// (Pamyatnykh 1999, figure 3, as above).
const SPB: ((f64, f64), (f64, f64)) = ((4.05, 4.30), (1.75, 3.5));

/// The γ Doradus region on the main sequence, K: 6,900–7,550 (Kaye et al. 1999, PASP 111, 840,
/// table 2, 6,950–7,375 K; Handler and Shobbrook 2002's blue edge of 7,550 K on the zero-age main
/// sequence).
const GAMMA_DORADUS: (f64, f64) = (6_900.0, 7_550.0);

/// The pulsation constant of the g-mode pulsators (SPB and γ Doradus stars), d: 0.6, which puts a
/// 1.6 M☉ γ Doradus star near 1 d and the SPB stars at 1–2 d, inside the catalogued 0.4–3 d and
/// 0.4–5 d (Kaye et al. 1999; Pamyatnykh 1999). The lane's rule; provisional.
const Q_GRAVITY_MODE: f64 = 0.6;

/// The ZZ Ceti strip, K: 10,500–12,500, with periods of 100–1,200 s from the blue edge to the red
/// (Althaus et al. 2010, A&ARv 18, 471, sections 3 and 9.1).
const ZZ_CETI: ((f64, f64), (f64, f64)) = ((10_500.0, 12_500.0), (100.0, 1_200.0));

/// The V777 Her strip, K: 22,000–29,000, periods 100–1,100 s (Althaus et al. 2010).
const V777_HER: ((f64, f64), (f64, f64)) = ((22_000.0, 29_000.0), (100.0, 1_100.0));

/// The GW Vir region: 80,000–180,000 K and log g 5.5–7.5, periods of about 300–3,000 s (Althaus
/// et al. 2010), longer at lower gravity.
const GW_VIR: ((f64, f64), (f64, f64), (f64, f64)) =
    ((80_000.0, 180_000.0), (5.5, 7.5), (300.0, 3_000.0));

/// The full amplitudes of the white-dwarf pulsators, mag: up to 0.3 (Althaus et al. 2010).
const WHITE_DWARF_AMPLITUDE: f64 = 0.3;

/// The α Cygni supergiants, K: the B and A types, 8,000–25,000 (the catalogue's Bep–AepIa), with
/// amplitudes about 0.1 mag and periods of days to weeks (Samus et al. 2017), from the mean
/// density with Q = 0.04 d (the lane's).
const ALPHA_CYGNI: ((f64, f64), f64, f64) = ((8_000.0, 25_000.0), 0.04, 0.1);

/// The luminous blue variables' region, after Hurley et al. (2000, section 7.1, the
/// Humphreys–Davidson limit: L above 6 × 10⁵ L☉ and 10⁻⁵ R L^½ above 1) and hotter than 8,000 K
/// (plan 06, P06.T24.a). P06.T24.a's `PhasePredicate::Lbv`, when it lands, is to replace this
/// copy of its criterion.
const LBV: (f64, f64) = (6.0e5, 8_000.0);

/// The S Doradus cycles, years: 10 × (R ÷ 100 R☉), held to 3–40, so that the cycles last years to
/// decades (Humphreys and Davidson 1994, PASP 106, 1025) and lengthen as the star swells to its
/// cool phase; the full amplitude 1.5 mag, of the catalogue's 1–7 (the lane's rule; provisional).
const S_DORADUS: (f64, (f64, f64), f64) = (10.0, (3.0, 40.0), 1.5);

/// The amplitude of an α² Canum Venaticorum star's rotational modulation, mag: 0.05, within the catalogue's
/// 0.01–0.1.
const ALPHA2_CVN_AMPLITUDE: f64 = 0.05;

/// The coolest spotted dwarfs, K: the K and M dwarfs below 5,300 K (the catalogue's dKe–dMe).
const BY_DRA_MAX_TEFF: f64 = 5_300.0;

/// A BY Dra star's amplitude, mag: 0.2 when saturated and 0.05 when highly active, within the
/// catalogue's few hundredths to 0.5 (the lane's).
const BY_DRA_AMPLITUDES: (f64, f64) = (0.2, 0.05);

/// How a star in `inputs` varies, or `None` if it does not.
///
/// The regions are tested in this order, and the first that holds the star names it (the lane's
/// order: one kind per star, so there are no δ Scuti–γ Doradus hybrids): the white dwarf
/// pulsators and GW Vir stars; the S Doradus cycles; α² Canum Venaticorum stars (Ap and Bp);
/// the instability strip (classical Cepheids, type II Cepheids, RR Lyrae stars, δ Scuti stars);
/// β Cephei, SPB and γ Doradus stars; α Cygni supergiants; long-period variables; BY Dra stars.
///
/// # Examples
///
/// A 5 M☉ Cepheid on its blue loop, at 1,600 L☉ and 5,800 K, pulsates with a period of about
/// four days:
///
/// ```
/// use hyperion_sim::stellar::classify::{ClassExtras, classify};
/// use hyperion_sim::stellar::draws::StarDraws;
/// use hyperion_sim::stellar::variability::{VariabilityInputs, VariableKind, variability};
/// use hyperion_sim::stellar::{Composition, Phase, StarState, StarStateParts};
/// use hyperion_sim::units::{SolarLuminosities, SolarMasses, SolarMassesPerYear, SolarRadii, Years};
///
/// let r = 1_600_f64.sqrt() * (5_772.0_f64 / 5_800.0).powi(2);
/// let state = StarState::new(StarStateParts {
///     phase: Phase::CoreHeliumBurning,
///     age: Years::new(1.0e8),
///     mass: SolarMasses::new(5.0),
///     core_mass: SolarMasses::new(0.8),
///     luminosity: SolarLuminosities::new(1_600.0),
///     radius: SolarRadii::new(r),
///     mass_loss_rate: SolarMassesPerYear::ZERO,
///     phase_fraction: 0.5,
/// });
/// let class = classify(&state, &Composition::SOLAR, &StarDraws::median(), &ClassExtras::NONE);
/// let cepheid = variability(&VariabilityInputs {
///     state: &state,
///     initial_mass: SolarMasses::new(5.0),
///     composition: &Composition::SOLAR,
///     classification: &class,
///     rotation: None,
///     activity: None,
/// })
/// .ok_or("inside the strip")?;
/// assert_eq!(cepheid.kind(), VariableKind::ClassicalCepheid);
/// assert!((3.0..10.0).contains(&cepheid.period().value()));
/// # Ok::<(), &str>(())
/// ```
#[must_use]
pub fn variability(inputs: &VariabilityInputs<'_>) -> Option<Variability> {
    let state = inputs.state;
    if state.luminosity().value() <= 0.0 || state.radius().value() <= 0.0 {
        return None;
    }
    white_dwarf_pulsator(inputs)
        .or_else(|| s_doradus(state))
        .or_else(|| alpha2_canum_venaticorum(inputs))
        .or_else(|| instability_strip(inputs))
        .or_else(|| main_sequence_b_and_f(state))
        .or_else(|| alpha_cygni(inputs))
        .or_else(|| long_period(inputs))
        .or_else(|| by_draconis(inputs))
}

/// A variable of `kind`, period `days` and amplitude `amplitude` mag.
#[must_use]
const fn variable(kind: VariableKind, days: f64, amplitude: f64) -> Variability {
    Variability {
        kind,
        period: Days::new(days),
        amplitude: Magnitudes::new(amplitude),
    }
}

/// The mean density of the star in `state` relative to the Sun's, M ÷ R³ in solar units.
#[must_use]
fn relative_density(state: &StarState) -> f64 {
    let r = state.radius().value();
    state.mass().value() / (r * r * r)
}

/// The period from the mean density with pulsation constant `q`, d: Q (ρ̄ ÷ ρ̄☉)^−½.
#[must_use]
fn density_period(state: &StarState, q: f64) -> f64 {
    q / relative_density(state).sqrt()
}

/// The amplitude profile across a region at `x` (0 and 1 at its edges): 4x(1 − x), largest
/// mid-region and zero at both edges (plan 06, P06.T26.a; Bono et al. 2000 find the peak nearer
/// the blue edge, which is recorded as a finding).
#[must_use]
fn profile(x: f64) -> f64 {
    4.0 * x * (1.0 - x)
}

/// Where `value` lies across (`low`, `high`), 0 to 1, if strictly inside.
#[must_use]
fn across(value: f64, (low, high): (f64, f64)) -> Option<f64> {
    (value > low && value < high).then(|| (value - low) / (high - low))
}

/// `value` interpolated in log from `low` to `high` at `x`, 0 to 1.
#[must_use]
fn log_between((low, high): (f64, f64), x: f64) -> f64 {
    low * math::powf(high / low, x)
}

/// The amplitude of `kind` in a table of amplitudes, mag.
#[must_use]
fn amplitude_of(table: &[(VariableKind, f64)], kind: VariableKind) -> f64 {
    table
        .iter()
        .find(|(k, _)| *k == kind)
        .map_or(0.0, |&(_, a)| a)
}

/// ZZ Ceti, V777 Her and GW Vir stars.
#[must_use]
fn white_dwarf_pulsator(inputs: &VariabilityInputs<'_>) -> Option<Variability> {
    let state = inputs.state;
    let teff = state.effective_temperature().value();
    let atmosphere = match inputs.classification.spectral_type() {
        SpectralType::WhiteDwarf(wd) => Some(wd.atmosphere()),
        SpectralType::Sequence(_)
        | SpectralType::NeutronStar(_)
        | SpectralType::BlackHole
        | SpectralType::NoRemnant => None,
    };
    let hot_pre_white_dwarf =
        state.phase() == Phase::PostAgb || atmosphere == Some(WhiteDwarfAtmosphere::Helium);
    if hot_pre_white_dwarf {
        let ((t_lo, t_hi), gravity, periods) = GW_VIR;
        if let (Some(x), Some(log_g)) = (across(teff, (t_lo, t_hi)), state.surface_gravity())
            && let Some(g) = across(log_g.value(), gravity)
        {
            // Longer periods at lower gravity: 3,000 s at log g 5.5, 300 s at 7.5.
            let seconds = log_between((periods.1, periods.0), g);
            return Some(variable(
                VariableKind::GwVirginis,
                seconds / SECONDS_PER_DAY,
                WHITE_DWARF_AMPLITUDE * profile(x),
            ));
        }
    }
    let (kind, (temperatures, periods)) = match atmosphere? {
        WhiteDwarfAtmosphere::Hydrogen => (VariableKind::ZzCeti, ZZ_CETI),
        WhiteDwarfAtmosphere::Helium => (VariableKind::V777Herculis, V777_HER),
    };
    let x = across(teff, temperatures)?;
    // x runs from the cool edge; the periods lengthen towards it.
    let seconds = log_between(periods, 1.0 - x);
    Some(variable(
        kind,
        seconds / SECONDS_PER_DAY,
        WHITE_DWARF_AMPLITUDE * profile(x),
    ))
}

/// The S Doradus cycles of a luminous blue variable.
#[must_use]
fn s_doradus(state: &StarState) -> Option<Variability> {
    let (l, r) = (state.luminosity().value(), state.radius().value());
    let (min_l, min_teff) = LBV;
    let lbv = state.phase().is_living()
        && l > min_l
        && 1e-5 * r * l.sqrt() > 1.0
        && state.effective_temperature().value() > min_teff;
    if !lbv {
        return None;
    }
    let (scale, (low, high), amplitude) = S_DORADUS;
    let years = (scale * r / 100.0).clamp(low, high);
    Some(variable(
        VariableKind::SDoradus,
        years * SECONDS_PER_JULIAN_YEAR / SECONDS_PER_DAY,
        amplitude,
    ))
}

/// The RR Lyrae strip at metal fraction `z`.
#[must_use]
fn rr_lyrae_strip(z: f64) -> Strip {
    let shift = -0.012 * math::log10(z);
    Strip {
        blue: (RR_LYRAE_STRIP.blue.0 + shift, RR_LYRAE_STRIP.blue.1),
        red: (RR_LYRAE_STRIP.red.0 + shift, RR_LYRAE_STRIP.red.1),
    }
}

/// The pulsators of the classical instability strip, named by phase and initial mass.
#[must_use]
fn instability_strip(inputs: &VariabilityInputs<'_>) -> Option<Variability> {
    let state = inputs.state;
    let m0 = inputs.initial_mass.value();
    let log_l = math::log10(state.luminosity().value());
    let log_t = math::log10(state.effective_temperature().value());
    let low_mass = m0 < LOW_MASS_LIMIT;
    let (kind, strip, q) = match state.phase() {
        Phase::MainSequence | Phase::HertzsprungGap
            if (DELTA_SCUTI_MASSES.0..=DELTA_SCUTI_MASSES.1).contains(&m0)
                && state
                    .surface_gravity()
                    .is_some_and(|g| g.value() >= DELTA_SCUTI_MIN_LOG_G) =>
        {
            (VariableKind::DeltaScuti, DELTA_SCUTI_STRIP, Q_DELTA_SCUTI)
        }
        Phase::HertzsprungGap | Phase::CoreHeliumBurning
            if (CEPHEID_MASSES.0..=CEPHEID_MASSES.1).contains(&m0) =>
        {
            (VariableKind::ClassicalCepheid, CEPHEID_STRIP, Q_CEPHEID)
        }
        Phase::CoreHeliumBurning if low_mass => (
            VariableKind::RrLyrae,
            rr_lyrae_strip(inputs.composition.z_fit().value()),
            Q_RR_LYRAE,
        ),
        Phase::EarlyAgb | Phase::ThermallyPulsingAgb if low_mass => {
            (VariableKind::WVirginis, CEPHEID_STRIP, Q_CEPHEID)
        }
        _ => return None,
    };
    let x = strip.position(log_l, log_t)?;
    if !(x > 0.0 && x < 1.0) {
        return None;
    }
    let days = density_period(state, q);
    let kind = if kind == VariableKind::WVirginis {
        if days < TYPE_II_PERIODS.0 {
            VariableKind::BlHerculis
        } else if days < TYPE_II_PERIODS.1 {
            VariableKind::WVirginis
        } else {
            VariableKind::RvTauri
        }
    } else {
        kind
    };
    Some(variable(
        kind,
        days,
        amplitude_of(&STRIP_AMPLITUDES, kind) * profile(x),
    ))
}

/// β Cephei, SPB and γ Doradus stars on the main sequence.
#[must_use]
fn main_sequence_b_and_f(state: &StarState) -> Option<Variability> {
    if state.phase() != Phase::MainSequence {
        return None;
    }
    let teff = state.effective_temperature().value();
    let log_t = math::log10(teff);
    let log_l = math::log10(state.luminosity().value());
    let (temperatures, min_log_l) = BETA_CEPHEI;
    if log_l > min_log_l
        && let Some(x) = across(log_t, temperatures)
    {
        return Some(variable(
            VariableKind::BetaCephei,
            density_period(state, Q_DELTA_SCUTI),
            0.1 * profile(x),
        ));
    }
    let (temperatures, luminosities) = SPB;
    if let (Some(x), Some(_)) = (across(log_t, temperatures), across(log_l, luminosities)) {
        return Some(variable(
            VariableKind::SlowlyPulsatingB,
            density_period(state, Q_GRAVITY_MODE),
            0.05 * profile(x),
        ));
    }
    let x = across(teff, GAMMA_DORADUS)?;
    Some(variable(
        VariableKind::GammaDoradus,
        density_period(state, Q_GRAVITY_MODE),
        0.1 * profile(x),
    ))
}

/// Whether `classification` is a supergiant's, of class I.
#[must_use]
fn is_supergiant(classification: &Classification) -> bool {
    matches!(
        classification.luminosity_class(),
        Some(
            LuminosityClass::Hypergiant
                | LuminosityClass::LuminousSupergiant
                | LuminosityClass::Supergiant
                | LuminosityClass::LessLuminousSupergiant
        )
    )
}

/// α Cygni variables: B and A supergiants.
#[must_use]
fn alpha_cygni(inputs: &VariabilityInputs<'_>) -> Option<Variability> {
    if !inputs.state.phase().is_living() || !is_supergiant(inputs.classification) {
        return None;
    }
    let (temperatures, q, amplitude) = ALPHA_CYGNI;
    let x = across(inputs.state.effective_temperature().value(), temperatures)?;
    Some(variable(
        VariableKind::AlphaCygni,
        density_period(inputs.state, q),
        amplitude * profile(x),
    ))
}

/// The fundamental period of a long-period variable, d: log P = −2.07 + 1.94 log R − 0.9 log M
/// (Vassiliadis and Wood 1993, ApJ 413, 641, equation 4, after Wood 1990; plan 06 credits Ostlie
/// and Cox 1986, whose own relation is −1.92 + 1.86 log R − 0.73 log M, within 10% of this one).
#[must_use]
fn long_period_days(state: &StarState) -> f64 {
    math::exp10(
        -2.07 + 1.94 * math::log10(state.radius().value())
            - 0.9 * math::log10(state.mass().value()),
    )
}

/// Miras, semiregulars and the red supergiants' slow variations.
#[must_use]
fn long_period(inputs: &VariabilityInputs<'_>) -> Option<Variability> {
    let state = inputs.state;
    if state.effective_temperature().value() >= LPV_MAX_TEFF {
        return None;
    }
    let days = long_period_days(state);
    let kind = if is_supergiant(inputs.classification) {
        match state.phase() {
            Phase::CoreHeliumBurning | Phase::EarlyAgb | Phase::ThermallyPulsingAgb => {
                VariableKind::SemiregularC
            }
            Phase::HertzsprungGap | Phase::FirstGiantBranch => VariableKind::SlowIrregular,
            _ => return None,
        }
    } else {
        match state.phase() {
            Phase::ThermallyPulsingAgb if days >= MIRA_MIN_PERIOD => VariableKind::Mira,
            Phase::ThermallyPulsingAgb | Phase::EarlyAgb => VariableKind::SemiregularA,
            Phase::FirstGiantBranch if state.luminosity().value() >= TIP_GIANT_MIN_L => {
                VariableKind::SemiregularB
            }
            _ => return None,
        }
    };
    Some(variable(kind, days, amplitude_of(&LPV_AMPLITUDES, kind)))
}

/// Rotational modulation of an Ap or Bp star's spots, at its rotation period: an α² Canum
/// Venaticorum star. It is tested before the pulsators, since the fossil field of an Ap or Bp star
/// suppresses the pulsations of the δ Scuti, γ Doradus and SPB regions it may lie in (the lane's
/// order; the rapidly oscillating Ap stars are not modelled).
#[must_use]
fn alpha2_canum_venaticorum(inputs: &VariabilityInputs<'_>) -> Option<Variability> {
    let spin = inputs.rotation?;
    matches!(
        inputs.classification.peculiar_class(),
        Some(PeculiarClass::Ap | PeculiarClass::Bp)
    )
    .then(|| {
        variable(
            VariableKind::Alpha2CanumVenaticorum,
            spin.period().value(),
            ALPHA2_CVN_AMPLITUDE,
        )
    })
}

/// Rotational modulation of an active cool dwarf's spots, at its rotation period: a BY Draconis
/// star, a saturated or highly active K or M dwarf.
#[must_use]
fn by_draconis(inputs: &VariabilityInputs<'_>) -> Option<Variability> {
    let spin = inputs.rotation?;
    let state = inputs.state;
    if state.phase() != Phase::MainSequence
        || state.effective_temperature().value() >= BY_DRA_MAX_TEFF
    {
        return None;
    }
    let amplitude = match inputs.activity?.level() {
        ActivityLevel::Saturated => BY_DRA_AMPLITUDES.0,
        ActivityLevel::High => BY_DRA_AMPLITUDES.1,
        ActivityLevel::Moderate | ActivityLevel::Low => return None,
    };
    Some(variable(
        VariableKind::ByDraconis,
        spin.period().value(),
        amplitude,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stellar::StarStateParts;
    use crate::stellar::classify::{ClassExtras, classify};
    use crate::stellar::draws::StarDraws;
    use crate::stellar::sse::Track;
    use crate::units::{SolarLuminosities, SolarMassesPerYear, SolarRadii, Years};

    /// A state of `phase`, mass `m` (M☉), luminosity `l` (L☉) and temperature `t` (K).
    fn star(phase: Phase, m: f64, l: f64, t: f64) -> StarState {
        let r = l.sqrt() * (5_772.0 / t) * (5_772.0 / t);
        StarState::new(StarStateParts {
            phase,
            age: Years::new(1e8),
            mass: SolarMasses::new(m),
            core_mass: SolarMasses::new(if phase.is_remnant() { m } else { 0.1 * m }),
            luminosity: SolarLuminosities::new(l),
            radius: SolarRadii::new(r),
            mass_loss_rate: SolarMassesPerYear::ZERO,
            phase_fraction: 0.5,
        })
    }

    fn vary(state: &StarState, m0: f64, composition: &Composition) -> Option<Variability> {
        let class = classify(state, composition, &StarDraws::median(), &ClassExtras::NONE);
        variability(&VariabilityInputs {
            state,
            initial_mass: SolarMasses::new(m0),
            composition,
            classification: &class,
            rotation: None,
            activity: None,
        })
    }

    /// The plan's test: a 5 M☉ blue-loop star gets a Cepheid period of 3–10 days. Every state of
    /// the track's core helium burning that pulsates in the strip is a classical Cepheid with such
    /// a period, and the loop does enter the strip.
    ///
    /// The star is at [Fe/H] = −0.5, the Magellanic Clouds' Cepheids': at solar metallicity the
    /// backbone's 5 M☉ loop reaches only 4,660 K, short of the strip's blue edge at 6,170 K, and
    /// needs 6 M☉ to reach it (a finding of P06.T26, recorded against the backbone).
    #[test]
    fn a_five_solar_mass_blue_loop_star_is_a_three_to_ten_day_cepheid() {
        let lmc = Composition::from_fe_h(
            crate::units::Dex::new(-0.5),
            crate::units::HeliumExcess::ZERO,
        );
        let track = Track::full(SolarMasses::new(5.0), &lmc, &StarDraws::median());
        let mut found = 0;
        for i in 0..20_000 {
            let age = Years::new(9.5e7 + f64::from(i) * 1.0e3);
            let state = track.state_at(age);
            if state.phase() != Phase::CoreHeliumBurning {
                continue;
            }
            // Off the loop the red helium-burning star is a slow semiregular; on it, a Cepheid.
            if let Some(v) = vary(&state, 5.0, &lmc)
                && v.kind() != VariableKind::SemiregularC
            {
                assert_eq!(v.kind(), VariableKind::ClassicalCepheid);
                assert!(
                    (3.0..=10.0).contains(&v.period().value()),
                    "{v:?} at {age:?}"
                );
                found += 1;
            }
        }
        assert!(found > 10, "the blue loop never enters the strip ({found})");
    }

    /// The plan's test: the sample's Cepheids follow a period–luminosity slope within 15% of the
    /// observed one, log L = 2.415 + 1.148 log P (Turner 2010, Ap&SS 326, 219).
    #[test]
    fn cepheids_follow_the_period_luminosity_slope() {
        let (mut points, mut n) = (Vec::new(), 0);
        for m in [3.5, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 12.0] {
            let track = Track::full(
                SolarMasses::new(m),
                &Composition::SOLAR,
                &StarDraws::median(),
            );
            let end = track.lifetime().unwrap().value();
            for i in 0..40_000 {
                let state = track.state_at(Years::new(end * f64::from(i) / 40_000.0));
                if state.phase() != Phase::CoreHeliumBurning {
                    continue;
                }
                if let Some(v) = vary(&state, m, &Composition::SOLAR)
                    && v.kind() == VariableKind::ClassicalCepheid
                {
                    points.push((
                        math::log10(v.period().value()),
                        math::log10(state.luminosity().value()),
                    ));
                    n += 1;
                }
            }
        }
        assert!(n > 50, "{n} Cepheid states");
        #[expect(clippy::cast_precision_loss, reason = "a count of states")]
        let count = points.len() as f64;
        let (mx, my) = points.iter().fold((0.0, 0.0), |(x, y), &(px, py)| {
            (x + px / count, y + py / count)
        });
        let (sxy, sxx) = points.iter().fold((0.0, 0.0), |(sxy, sxx), &(px, py)| {
            (sxy + (px - mx) * (py - my), sxx + (px - mx) * (px - mx))
        });
        let slope = sxy / sxx;
        assert!(
            (slope / 1.148 - 1.0).abs() <= 0.15,
            "slope {slope} over {n} states"
        );
    }

    /// The plan's test: RR Lyrae periods of 0.3–0.9 days across the strip, for horizontal-branch
    /// stars of 0.65–0.75 M☉ at 40–50 L☉ and halo metallicity.
    #[test]
    fn rr_lyrae_stars_pulsate_in_a_third_to_nine_tenths_of_a_day() {
        let halo = Composition::from_fe_h(
            crate::units::Dex::new(-1.5),
            crate::units::HeliumExcess::ZERO,
        );
        let mut found = 0;
        for m in [0.65, 0.7, 0.75] {
            for l in [40.0, 45.0, 50.0] {
                for t in (5_800..=7_600).step_by(50) {
                    let state = star(Phase::CoreHeliumBurning, m, l, f64::from(t));
                    if let Some(v) = vary(&state, 0.85, &halo) {
                        assert_eq!(v.kind(), VariableKind::RrLyrae);
                        assert!((0.3..=0.9).contains(&v.period().value()), "{v:?}");
                        found += 1;
                    }
                }
            }
        }
        assert!(found > 50, "{found}");
    }

    /// The plan's test: δ Scuti periods of 0.02–0.3 days over the main sequence and gap of
    /// 1.5–2.5 M☉ stars.
    #[test]
    fn delta_scuti_stars_pulsate_in_under_a_third_of_a_day() {
        let mut found = 0;
        for m in [1.5, 1.7, 2.0, 2.3, 2.5] {
            let track = Track::full(
                SolarMasses::new(m),
                &Composition::SOLAR,
                &StarDraws::median(),
            );
            let end = track.lifetime().unwrap().value();
            for i in 0..4_000 {
                let state = track.state_at(Years::new(end * 0.95 * f64::from(i) / 4_000.0));
                if let Some(v) = vary(&state, m, &Composition::SOLAR)
                    && v.kind() == VariableKind::DeltaScuti
                {
                    assert!((0.02..=0.3).contains(&v.period().value()), "{v:?}");
                    found += 1;
                }
            }
        }
        assert!(found > 50, "{found}");
    }

    /// The plan's test: Mira periods of 150–600 days for thermally pulsing AGB stars of 1–3 M☉ at
    /// 3,000–10,000 L☉ (the brighter the heavier) and 2,800–3,300 K.
    #[test]
    fn miras_pulsate_in_one_hundred_fifty_to_six_hundred_days() {
        for (m, l) in [
            (1.0, 3_000.0),
            (1.5, 5_000.0),
            (2.0, 7_000.0),
            (3.0, 10_000.0),
        ] {
            for t in [2_800.0, 3_000.0, 3_300.0] {
                let state = star(Phase::ThermallyPulsingAgb, m, l, t);
                let v = vary(&state, m, &Composition::SOLAR).unwrap();
                assert_eq!(v.kind(), VariableKind::Mira, "{m} M☉, {l} L☉");
                assert!((150.0..=600.0).contains(&v.period().value()), "{v:?}");
            }
        }
    }

    /// The plan's test: a star just outside the strip has amplitude zero (it does not vary), and
    /// one just inside a small one, so variability switches on continuously.
    #[test]
    fn a_star_just_outside_the_strip_does_not_vary() {
        let log_l: f64 = 3.2;
        let blue = math::mul_add(CEPHEID_STRIP.blue.1, log_l, CEPHEID_STRIP.blue.0);
        let red = math::mul_add(CEPHEID_STRIP.red.1, log_l, CEPHEID_STRIP.red.0);
        let at = |log_t: f64| {
            vary(
                &star(
                    Phase::CoreHeliumBurning,
                    5.0,
                    math::exp10(log_l),
                    math::exp10(log_t),
                ),
                5.0,
                &Composition::SOLAR,
            )
        };
        assert_eq!(at(blue + 1e-6), None);
        assert_eq!(at(red - 1e-6), None);
        let inside = at(blue - 1e-6).unwrap();
        assert!(inside.amplitude().value() < 1e-3, "{inside:?}");
        let middle = at(f64::midpoint(blue, red)).unwrap();
        assert!(
            (middle.amplitude().value() - 1.0).abs() < 1e-9,
            "{middle:?}"
        );
    }

    /// Type II Cepheids take their class from their period.
    #[test]
    fn type_two_cepheids_are_named_by_period() {
        let mut kinds = std::collections::BTreeSet::new();
        for l in [100.0, 300.0, 1_000.0, 3_000.0] {
            let log_l = math::log10(l);
            let blue = math::mul_add(CEPHEID_STRIP.blue.1, log_l, CEPHEID_STRIP.blue.0);
            let red = math::mul_add(CEPHEID_STRIP.red.1, log_l, CEPHEID_STRIP.red.0);
            let state = star(
                Phase::EarlyAgb,
                0.6,
                l,
                math::exp10(f64::midpoint(blue, red)),
            );
            let v = vary(&state, 0.9, &Composition::SOLAR).unwrap();
            let p = v.period().value();
            let expected = if p < 4.0 {
                VariableKind::BlHerculis
            } else if p < 20.0 {
                VariableKind::WVirginis
            } else {
                VariableKind::RvTauri
            };
            assert_eq!(v.kind(), expected, "{p} d");
            kinds.insert(v.kind());
        }
        assert!(kinds.len() >= 2, "{kinds:?}");
    }

    /// The white-dwarf strips of helium atmospheres and the hot pre-white dwarfs: V777 Her stars of
    /// 22,000–29,000 K and GW Vir stars of 80,000–180,000 K, each with its catalogued periods.
    #[test]
    fn helium_white_dwarfs_and_pre_white_dwarfs_pulsate_in_minutes() {
        let helium = StarDraws::from_parts(crate::stellar::draws::StarDrawsParts {
            wd_atmosphere: crate::rng::Mark::from_word(0),
            ..crate::stellar::draws::StarDrawsParts::MEDIAN
        });
        let at = |state: &StarState| {
            let class = classify(state, &Composition::SOLAR, &helium, &ClassExtras::NONE);
            variability(&VariabilityInputs {
                state,
                initial_mass: SolarMasses::new(2.0),
                composition: &Composition::SOLAR,
                classification: &class,
                rotation: None,
                activity: None,
            })
        };
        let db = at(&star(Phase::CarbonOxygenWhiteDwarf, 0.6, 0.05, 25_000.0)).unwrap();
        assert_eq!(db.kind(), VariableKind::V777Herculis);
        let seconds = db.period().value() * SECONDS_PER_DAY;
        assert!((100.0..=1_100.0).contains(&seconds), "{db:?}");
        // A post-AGB star of 0.6 M☉ at 100,000 K and log g ≈ 6.5 (R ≈ 0.084 R☉).
        let hot = star(Phase::PostAgb, 0.6, 600.0, 100_000.0);
        let gw = at(&hot).unwrap();
        assert_eq!(gw.kind(), VariableKind::GwVirginis, "{hot:?}");
        let seconds = gw.period().value() * SECONDS_PER_DAY;
        assert!((300.0..=3_000.0).contains(&seconds), "{gw:?}");
    }

    /// Rotational modulation: an Ap star is an α² Canum Venaticorum star at its rotation period, even inside
    /// the δ Scuti strip, and a saturated M dwarf a BY Dra star; a quiet one is neither.
    #[test]
    fn spotted_stars_vary_at_their_rotation_period() {
        let magnetic = StarDraws::from_parts(crate::stellar::draws::StarDrawsParts {
            magnetism: crate::rng::Mark::from_word(0),
            ..crate::stellar::draws::StarDrawsParts::MEDIAN
        });
        let ap = star(Phase::MainSequence, 2.0, 16.0, 8_000.0);
        let class = classify(&ap, &Composition::SOLAR, &magnetic, &ClassExtras::NONE);
        assert_eq!(class.peculiar_class(), Some(PeculiarClass::Ap), "{class}");
        let spin =
            crate::stellar::rotation::rotation(&ap, &Composition::SOLAR, &magnetic, None).unwrap();
        let inputs = VariabilityInputs {
            state: &ap,
            initial_mass: SolarMasses::new(2.0),
            composition: &Composition::SOLAR,
            classification: &class,
            rotation: Some(&spin),
            activity: None,
        };
        let v = variability(&inputs).unwrap();
        assert_eq!(v.kind(), VariableKind::Alpha2CanumVenaticorum);
        assert_eq!(v.period(), spin.period());
        // A 0.3 M☉ dwarf at 100 Myr, still saturated.
        let young = star(Phase::MainSequence, 0.3, 0.012, 3_400.0);
        let draws = StarDraws::median();
        let class = classify(&young, &Composition::SOLAR, &draws, &ClassExtras::NONE);
        let spin =
            crate::stellar::rotation::rotation(&young, &Composition::SOLAR, &draws, None).unwrap();
        let activity = crate::stellar::rotation::activity(&young, Some(&spin)).unwrap();
        assert_eq!(activity.level(), ActivityLevel::Saturated, "{activity:?}");
        let v = variability(&VariabilityInputs {
            state: &young,
            initial_mass: SolarMasses::new(0.3),
            composition: &Composition::SOLAR,
            classification: &class,
            rotation: Some(&spin),
            activity: Some(&activity),
        })
        .unwrap();
        assert_eq!(v.kind(), VariableKind::ByDraconis);
        assert_eq!(v.period(), spin.period());
        let sun = star(Phase::MainSequence, 1.0, 1.0, 5_772.0);
        let class = classify(&sun, &Composition::SOLAR, &draws, &ClassExtras::NONE);
        assert_eq!(vary(&sun, 1.0, &Composition::SOLAR), None, "{class}");
    }

    /// A backbone track makes Miras: a 1.5 M☉ star on its thermally pulsing AGB pulsates in
    /// 150–600 days somewhere along it.
    #[test]
    fn a_thermally_pulsing_track_makes_miras() {
        let track = Track::full(
            SolarMasses::new(1.5),
            &Composition::SOLAR,
            &StarDraws::median(),
        );
        let end = track.lifetime().unwrap().value();
        let miras: Vec<f64> = (0..200_000)
            .map(|i| track.state_at(Years::new(end * (0.95 + 0.05 * f64::from(i) / 200_000.0))))
            .filter(|state| state.phase() == Phase::ThermallyPulsingAgb)
            .filter_map(|state| vary(&state, 1.5, &Composition::SOLAR))
            .filter(|v| v.kind() == VariableKind::Mira)
            .map(|v| v.period().value())
            .collect();
        assert!(!miras.is_empty(), "no Mira on the track");
        assert!(
            miras.iter().any(|p| (150.0..=600.0).contains(p)),
            "{:?}",
            &miras[..miras.len().min(5)]
        );
    }

    /// The other strips: β Cephei, SPB and γ Doradus stars on the main sequence, ZZ Ceti and V777
    /// Her white dwarfs, an α Cygni supergiant and the S Doradus cycles, each with a period in its
    /// catalogued range.
    #[test]
    fn every_other_strip_names_its_kind() {
        let solar = Composition::SOLAR;
        let beta = vary(
            &star(Phase::MainSequence, 12.0, 1.0e4, 24_000.0),
            12.0,
            &solar,
        )
        .unwrap();
        assert_eq!(beta.kind(), VariableKind::BetaCephei);
        assert!((0.05..=0.6).contains(&beta.period().value()), "{beta:?}");
        let spb = vary(
            &star(Phase::MainSequence, 4.0, 300.0, 15_000.0),
            4.0,
            &solar,
        )
        .unwrap();
        assert_eq!(spb.kind(), VariableKind::SlowlyPulsatingB);
        assert!((0.4..=5.0).contains(&spb.period().value()), "{spb:?}");
        let gamma = vary(&star(Phase::MainSequence, 1.6, 7.0, 7_200.0), 1.35, &solar).unwrap();
        assert_eq!(gamma.kind(), VariableKind::GammaDoradus);
        assert!((0.3..=3.0).contains(&gamma.period().value()), "{gamma:?}");
        let zz = vary(
            &star(Phase::CarbonOxygenWhiteDwarf, 0.6, 3e-3, 11_500.0),
            2.0,
            &solar,
        )
        .unwrap();
        assert_eq!(zz.kind(), VariableKind::ZzCeti);
        let seconds = zz.period().value() * SECONDS_PER_DAY;
        assert!((100.0..=1_200.0).contains(&seconds), "{zz:?}");
        let cygni = vary(
            &star(Phase::HertzsprungGap, 20.0, 2.0e5, 10_000.0),
            20.0,
            &solar,
        )
        .unwrap();
        assert_eq!(cygni.kind(), VariableKind::AlphaCygni);
        assert!((1.0..=60.0).contains(&cygni.period().value()), "{cygni:?}");
        let lbv = vary(
            &star(Phase::CoreHeliumBurning, 60.0, 1.0e6, 15_000.0),
            60.0,
            &solar,
        )
        .unwrap();
        assert_eq!(lbv.kind(), VariableKind::SDoradus);
        let years = lbv.period().value() * SECONDS_PER_DAY / SECONDS_PER_JULIAN_YEAR;
        assert!((1.0..=50.0).contains(&years), "{lbv:?}");
        assert_eq!(
            vary(&star(Phase::MainSequence, 1.0, 1.0, 5_772.0), 1.0, &solar),
            None
        );
    }
}
